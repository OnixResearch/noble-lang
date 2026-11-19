//! Widget gestures: the only way an edit enters the experiment.
//!
//! Every gesture decodes to one of the five existing editor node kinds and the
//! edited tree is re-encoded as the strict versioned document. A gesture whose
//! payload is not the kind its focused control owns refuses before a document
//! exists, so nothing illegal can reach `admit`.

use noble_contracts::source::editor::Node;

use super::document::{boolean_value, encode, integer_value, source_word};
use super::surface::{View, Widget};
use super::Refusal;

/// Deepest quotation nesting the editor JSON transport can carry.
///
/// The ingress reader parses the envelope object at depth 0, its `nodes` array
/// at depth 1 and every node object two levels deeper per enclosing quotation.
/// It refuses a value parsed at depth 32 or deeper, so a terminal node inside
/// fourteen quotations is readable (depth 30). The bound is conservative: a
/// fifteen-level chain whose innermost quotation is empty would still be
/// readable, and it is refused anyway.
pub const MAX_QUOTATION_DEPTH: u32 = 14;

/// One widget edit, addressed with the existing bounded `node_path` scheme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gesture {
    SetNumber { path: Vec<u32>, text: String },
    SetCheckbox { path: Vec<u32>, text: String },
    Toggle { path: Vec<u32> },
    SetWord { path: Vec<u32>, text: String },
    LeaveHole { path: Vec<u32> },
    FillHole { path: Vec<u32>, fill: Fill },
}

/// A hole fill payload: only the five existing node kinds, each decoded through
/// the same control grammar its widget would use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fill {
    Number(String),
    Boolean(String),
    Word(String),
    Hole,
    Quotation(Vec<Fill>),
}

/// One accepted gesture: the edited node tree and its re-encoded document.
#[derive(Debug)]
pub struct Document {
    pub nodes: Vec<Node>,
    pub text: String,
}

impl Gesture {
    /// Apply one widget gesture to the surface, returning the edited tree and
    /// its re-encoded document. The surface is never mutated.
    pub fn apply(&self, view: &View) -> Result<Document, Refusal> {
        let path = self.path();
        let Some(control) = view.control(path) else {
            return Err(Refusal::new(
                "path-not-found",
                "the node path addresses no control",
            ));
        };
        let replacement = decode_gesture(self, &control.widget)?;
        if let Gesture::FillHole { fill, .. } = self {
            guard_fill_depth(path, fill)?;
        }
        let nodes = view.nodes_with(path, &replacement);
        let text = encode(&nodes)?;
        Ok(Document { nodes, text })
    }

    pub fn path(&self) -> &[u32] {
        match self {
            Gesture::SetNumber { path, .. }
            | Gesture::SetCheckbox { path, .. }
            | Gesture::Toggle { path }
            | Gesture::SetWord { path, .. }
            | Gesture::LeaveHole { path }
            | Gesture::FillHole { path, .. } => path,
        }
    }
}

impl Fill {
    pub fn decode(&self) -> Result<Node, Refusal> {
        match self {
            Fill::Number(text) => Ok(Node::Integer(integer_value(text)?)),
            Fill::Boolean(text) => Ok(Node::Boolean(boolean_value(text)?)),
            Fill::Word(text) => {
                source_word(text)?;
                Ok(Node::Word(text.clone()))
            }
            Fill::Hole => Ok(Node::Hole),
            Fill::Quotation(children) => {
                let decoded: Result<Vec<Node>, Refusal> =
                    children.iter().map(Fill::decode).collect();
                Ok(Node::Quotation(decoded?))
            }
        }
    }

    /// Quotation levels this payload would add below the filled hole.
    pub fn quotation_depth(&self) -> u32 {
        match self {
            Fill::Quotation(children) => children
                .iter()
                .map(Fill::quotation_depth)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
            _ => 0,
        }
    }
}

fn decode_gesture(gesture: &Gesture, widget: &Widget) -> Result<Node, Refusal> {
    match gesture {
        Gesture::SetNumber { text, .. } => match widget {
            Widget::Number { .. } => Ok(Node::Integer(integer_value(text)?)),
            _ => Err(wrong_widget("not-a-number-control", "an integer control")),
        },
        Gesture::SetCheckbox { text, .. } => match widget {
            Widget::Checkbox { .. } => Ok(Node::Boolean(boolean_value(text)?)),
            _ => Err(wrong_widget("not-a-checkbox", "a checkbox")),
        },
        Gesture::Toggle { .. } => match widget {
            Widget::Checkbox { checked } => Ok(Node::Boolean(!checked)),
            _ => Err(wrong_widget("not-a-checkbox", "a checkbox")),
        },
        Gesture::SetWord { text, .. } => match widget {
            Widget::Word { .. } => {
                source_word(text)?;
                Ok(Node::Word(text.clone()))
            }
            _ => Err(wrong_widget("not-a-word-control", "a word control")),
        },
        Gesture::LeaveHole { .. } => match widget {
            Widget::Hole { .. } => Ok(Node::Hole),
            _ => Err(wrong_widget("not-a-hole", "a hole")),
        },
        Gesture::FillHole { fill, .. } => match widget {
            Widget::Hole { .. } => fill.decode(),
            _ => Err(wrong_widget("not-a-hole", "a hole")),
        },
    }
}

fn wrong_widget(code: &'static str, expected: &str) -> Refusal {
    Refusal::new(code, format!("the focused control is not {expected}"))
}

fn guard_fill_depth(path: &[u32], fill: &Fill) -> Result<(), Refusal> {
    let enclosing = u32::try_from(path.len().saturating_sub(1))
        .map_err(|_| Refusal::new("path-limit", "the node path exceeds a u32 quotation depth"))?;
    let added = fill.quotation_depth();
    let total = enclosing.saturating_add(added);
    if total > MAX_QUOTATION_DEPTH {
        return Err(Refusal::new(
            "depth-limit",
            format!(
                "filling at depth {enclosing} with a quotation of depth {added} reaches {total} levels; the editor JSON transport carries at most {MAX_QUOTATION_DEPTH}"
            ),
        ));
    }
    Ok(())
}
