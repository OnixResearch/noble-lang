//! The typographic surface: one `Control` per editor node.
//!
//! Integers are I64 range-checked numeric controls, booleans are checkboxes,
//! words are text controls, quotations are groups and holes stay visible with
//! the constraints `analyze` reported for them. The surface is built from the
//! public `noble_contracts` editor nodes and re-encodes to the same node tree.

use noble_contracts::source::editor::Node;

use super::Refusal;

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

/// The next reported hole fact to bind, in tree order.
struct Cursor<'a> {
    facts: &'a [HoleFacts],
    next: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self) -> Result<HoleFacts, Refusal> {
        let Some(facts) = self.facts.get(self.next) else {
            return Err(Refusal::new(
                "hole-count",
                "the rendered tree carries more holes than the report",
            ));
        };
        self.next = self.next.saturating_add(1);
        Ok(facts.clone())
    }
}

impl View {
    pub fn from_nodes(nodes: &[Node]) -> Result<Self, Refusal> {
        let mut path = Vec::new();
        Ok(View {
            root: controls(nodes, &mut path)?,
            program_effect: None,
        })
    }

    /// Bind the constraints `analyze` reported, in tree order. A report whose
    /// hole count disagrees with the rendered holes is refused: the view never
    /// invents a constraint or hides an unreported hole.
    pub fn bind_report(&mut self, holes: &[HoleFacts]) -> Result<(), Refusal> {
        let mut cursor = Cursor {
            facts: holes,
            next: 0,
        };
        let root = std::mem::take(&mut self.root);
        self.root = bind_controls(root, &mut cursor)?;
        if cursor.next != holes.len() {
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

    pub fn to_nodes(&self) -> Vec<Node> {
        nodes_of(&self.root)
    }

    /// The same node tree with the control at `target` replaced, without ever
    /// mutating the surface.
    pub fn nodes_with(&self, target: &[u32], replacement: &Node) -> Vec<Node> {
        list_with(&self.root, target, replacement)
    }

    /// Every rendered hole's bound facts, in tree order.
    pub fn hole_facts(&self) -> Vec<HoleFacts> {
        let mut facts = Vec::new();
        let mut stack: Vec<&Control> = self.root.iter().rev().collect();
        while let Some(control) = stack.pop() {
            match &control.widget {
                Widget::Hole { facts: Some(bound) } => facts.push(bound.clone()),
                Widget::Hole { facts: None } => {}
                Widget::Group { children } => stack.extend(children.iter().rev()),
                Widget::Number { .. } | Widget::Checkbox { .. } | Widget::Word { .. } => {}
            }
        }
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

    /// The typographic rendering: structure, widgets and visible hole facts.
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

fn bind_controls(controls: Vec<Control>, cursor: &mut Cursor<'_>) -> Result<Vec<Control>, Refusal> {
    controls
        .into_iter()
        .map(|control| bind_control(control, cursor))
        .collect()
}

fn bind_control(control: Control, cursor: &mut Cursor<'_>) -> Result<Control, Refusal> {
    let Control { path, widget } = control;
    let widget = match widget {
        Widget::Hole { .. } => Widget::Hole {
            facts: Some(cursor.take()?),
        },
        Widget::Group { children } => Widget::Group {
            children: bind_controls(children, cursor)?,
        },
        other => other,
    };
    Ok(Control { path, widget })
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

fn list_with(controls: &[Control], target: &[u32], replacement: &Node) -> Vec<Node> {
    controls
        .iter()
        .map(|control| node_with(control, target, replacement))
        .collect()
}

fn node_with(control: &Control, target: &[u32], replacement: &Node) -> Node {
    if control.path == target {
        return replacement.clone();
    }
    match &control.widget {
        Widget::Group { children } => Node::Quotation(list_with(children, target, replacement)),
        Widget::Number { value } => Node::Integer(*value),
        Widget::Checkbox { checked } => Node::Boolean(*checked),
        Widget::Word { text } => Node::Word(text.clone()),
        Widget::Hole { .. } => Node::Hole,
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
