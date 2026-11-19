//! The default-off typographic view over the five editor node kinds.
//!
//! One `Control` per editor node: integers are I64 range-checked numeric
//! controls, booleans are checkboxes, words are text controls, quotations are
//! groups and holes stay visible with the constraints `analyze` reported for
//! them. Gestures decode only to the five existing node kinds; a gesture that
//! cannot decode refuses before any document exists, so no illegal node ever
//! reaches `admit`.

use noble_contracts::source::editor::Node;

use super::document::{boolean_value, encode, integer_value, source_word};
use super::Refusal;

/// Deepest quotation nesting the editor JSON transport can carry.
///
/// The ingress reader parses the envelope object at depth 0, its `nodes` array
/// at depth 1 and every node object two levels deeper per enclosing quotation
/// (the quotation object plus its `nodes` array). It refuses any value parsed at
/// depth 32 or deeper, so a node inside `depth` quotations is readable only
/// while `2 + 2 * depth < 32`, which bounds the renderer at 14.
pub const MAX_QUOTATION_DEPTH: usize = 14;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffectFacts {
    pub known: Vec<u32>,
    pub unresolved: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoleFacts {
    pub input_stack: String,
    pub output_stack: String,
    pub required_output_suffix: Vec<String>,
    pub effect: EffectFacts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Widget {
    Number { value: i64 },
    Checkbox { checked: bool },
    Word { text: String },
    Group { children: Vec<Control> },
    Hole { facts: Option<HoleFacts> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Control {
    pub path: Vec<u32>,
    pub widget: Widget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub root: Vec<Control>,
    pub program_effect: Option<EffectFacts>,
}

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

impl View {
    pub fn from_nodes(nodes: &[Node]) -> Result<Self, Refusal> {
        let mut path = Vec::new();
        Ok(View {
            root: controls(nodes, &mut path)?,
            program_effect: None,
        })
    }

    pub fn to_nodes(&self) -> Vec<Node> {
        nodes_of(&self.root)
    }

    /// Bind the constraints `analyze` reported, in tree order. A report whose
    /// hole count disagrees with the rendered holes is refused: the view never
    /// invents a constraint or hides an unreported hole.
    pub fn bind_report(&mut self, holes: &[HoleFacts]) -> Result<(), Refusal> {
        let mut queue = holes.iter();
        let mut assigned = 0_usize;
        bind_holes(&mut self.root, &mut queue, &mut assigned)?;
        if queue.next().is_some() {
            return Err(Refusal::new(
                "hole-count",
                "the report carries more holes than the rendered tree",
            ));
        }
        Ok(())
    }

    pub fn set_program_effect(&mut self, effect: EffectFacts) {
        self.program_effect = Some(effect);
    }

    /// Every rendered hole's bound facts, in tree order.
    pub fn hole_facts(&self) -> Vec<HoleFacts> {
        let mut facts = Vec::new();
        collect_holes(&self.root, &mut facts);
        facts
    }

    pub fn control(&self, path: &[u32]) -> Option<&Control> {
        let mut controls = self.root.as_slice();
        let mut found: Option<&Control> = None;
        for index in path {
            let at = usize::try_from(*index).ok()?;
            let control = controls.get(at)?;
            found = Some(control);
            controls = match &control.widget {
                Widget::Group { children } => children.as_slice(),
                _ => &[],
            };
        }
        found
    }

    /// The typographic rendering: structure, widgets and the visible hole
    /// constraints. Allocations here are the experiment's own, not the CLI's.
    pub fn render(&self) -> String {
        let mut text = String::new();
        if let Some(effect) = &self.program_effect {
            text.push_str("effects(");
            push_effect(effect, &mut text);
            text.push_str(")\n");
        }
        text.push_str("group(");
        push_controls(&self.root, &mut text);
        text.push(')');
        text
    }
}

impl Gesture {
    /// Apply one widget gesture to the view, returning the edited tree and its
    /// re-encoded document. Any gesture whose payload is not the node kind the
    /// focused control owns is refused with a stable code, and no document is
    /// produced.
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
        let mut nodes = view.to_nodes();
        replace(&mut nodes, path, replacement)?;
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
    pub fn quotation_depth(&self) -> usize {
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

fn controls(nodes: &[Node], path: &mut Vec<u32>) -> Result<Vec<Control>, Refusal> {
    let mut built = Vec::with_capacity(nodes.len());
    for (index, node) in nodes.iter().enumerate() {
        let at = u32::try_from(index)
            .map_err(|_| Refusal::new("path-limit", "the node index exceeds a u32 path"))?;
        path.push(at);
        built.push(Control {
            path: path.clone(),
            widget: widget_of(node, path)?,
        });
        path.pop();
    }
    Ok(built)
}

fn widget_of(node: &Node, path: &mut Vec<u32>) -> Result<Widget, Refusal> {
    Ok(match node {
        Node::Integer(value) => Widget::Number { value: *value },
        Node::Boolean(checked) => Widget::Checkbox { checked: *checked },
        Node::Word(text) => Widget::Word { text: text.clone() },
        Node::Hole => Widget::Hole { facts: None },
        Node::Quotation(children) => Widget::Group {
            children: controls(children, path)?,
        },
    })
}

fn nodes_of(controls: &[Control]) -> Vec<Node> {
    controls.iter().map(node_of).collect()
}

fn node_of(control: &Control) -> Node {
    match &control.widget {
        Widget::Number { value } => Node::Integer(*value),
        Widget::Checkbox { checked } => Node::Boolean(*checked),
        Widget::Word { text } => Node::Word(text.clone()),
        Widget::Group { children } => Node::Quotation(nodes_of(children)),
        Widget::Hole { .. } => Node::Hole,
    }
}

fn bind_holes(
    controls: &mut [Control],
    queue: &mut std::slice::Iter<'_, HoleFacts>,
    assigned: &mut usize,
) -> Result<(), Refusal> {
    for control in controls {
        match &mut control.widget {
            Widget::Hole { facts } => {
                let Some(next) = queue.next() else {
                    return Err(Refusal::new(
                        "hole-count",
                        "the rendered tree carries more holes than the report",
                    ));
                };
                *facts = Some(next.clone());
                *assigned = assigned.saturating_add(1);
            }
            Widget::Group { children } => bind_holes(children, queue, assigned)?,
            Widget::Number { .. } | Widget::Checkbox { .. } | Widget::Word { .. } => {}
        }
    }
    Ok(())
}

fn collect_holes(controls: &[Control], facts: &mut Vec<HoleFacts>) {
    for control in controls {
        match &control.widget {
            Widget::Hole { facts: Some(bound) } => facts.push(bound.clone()),
            Widget::Hole { facts: None } => {}
            Widget::Group { children } => collect_holes(children, facts),
            Widget::Number { .. } | Widget::Checkbox { .. } | Widget::Word { .. } => {}
        }
    }
}

fn push_controls(controls: &[Control], text: &mut String) {
    for (index, control) in controls.iter().enumerate() {
        if index > 0 {
            text.push_str(", ");
        }
        push_widget(&control.widget, text);
    }
}

fn push_widget(widget: &Widget, text: &mut String) {
    match widget {
        Widget::Number { value } => text.push_str(&format!("num({value})")),
        Widget::Checkbox { checked } => text.push_str(&format!("bool({checked})")),
        Widget::Word { text: word } => text.push_str(&format!("word({word})")),
        Widget::Group { children } => {
            text.push_str("group(");
            push_controls(children, text);
            text.push(')');
        }
        Widget::Hole { facts } => match facts {
            Some(facts) => {
                text.push_str("hole{");
                push_hole(facts, text);
                text.push('}');
            }
            None => text.push_str("hole{unbound}"),
        },
    }
}

fn push_hole(facts: &HoleFacts, text: &mut String) {
    text.push_str(&format!(
        "input={}; output={}; suffix=[{}]; effect=known[{}] unresolved={}",
        facts.input_stack,
        facts.output_stack,
        facts.required_output_suffix.join(","),
        known_text(&facts.effect.known),
        facts.effect.unresolved,
    ));
}

fn push_effect(effect: &EffectFacts, text: &mut String) {
    text.push_str(&format!(
        "known[{}] unresolved={}",
        known_text(&effect.known),
        effect.unresolved
    ));
}

fn known_text(known: &[u32]) -> String {
    let mut text = String::new();
    for (index, id) in known.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        text.push_str(&format!("{id}"));
    }
    text
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
    let enclosing = path.len().saturating_sub(1);
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

fn replace(nodes: &mut [Node], path: &[u32], replacement: Node) -> Result<(), Refusal> {
    let Some((index, rest)) = path.split_first() else {
        return Err(Refusal::new(
            "path-not-found",
            "an editor node path is empty",
        ));
    };
    let at = usize::try_from(*index)
        .map_err(|_| Refusal::new("path-not-found", "the node index exceeds usize"))?;
    let Some(slot) = nodes.get_mut(at) else {
        return Err(Refusal::new(
            "path-not-found",
            "the node path addresses no node",
        ));
    };
    if rest.is_empty() {
        *slot = replacement;
        return Ok(());
    }
    match slot {
        Node::Quotation(children) => replace(children.as_mut_slice(), rest, replacement),
        _ => Err(Refusal::new(
            "path-not-found",
            "the node path descends through a node that is not a quotation",
        )),
    }
}
