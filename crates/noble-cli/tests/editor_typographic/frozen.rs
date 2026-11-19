//! The frozen corpus loader: documents, edits, hole states and negative
//! controls as committed before the measurement that they govern.

use super::apply::{Fill, Gesture};
use super::cases::{field, fixture, path, text, Fixture};

pub const CORPUS_PATH: &str = "verification/editor-typographic/corpus.json";
pub const CRITERIA_PATH: &str = "verification/editor-typographic/criteria.json";

pub struct Snapshot {
    pub fixtures: Vec<Fixture>,
    pub hole_states: Vec<HoleState>,
    pub negative_controls: Vec<NegativeControl>,
    pub documented_edits: Vec<DocumentedEdit>,
    pub hostile_transport: Vec<String>,
}

pub struct HoleState {
    pub state: String,
    pub fixture: String,
}

pub struct NegativeControl {
    pub name: String,
    pub fixture: String,
    pub gesture: Gesture,
    pub code: String,
}

pub struct DocumentedEdit {
    pub name: String,
    pub edits: Vec<String>,
}

impl Snapshot {
    pub fn load() -> Result<Self, String> {
        let document_text = super::read(CORPUS_PATH)?;
        let value: serde_json::Value = serde_json::from_str(&document_text)
            .map_err(|error| format!("parse corpus: {error}"))?;
        if value.get("schema").and_then(serde_json::Value::as_str)
            != Some("noble-editor-typographic-corpus/v1")
        {
            return Err(String::from("corpus schema is not frozen v1"));
        }
        Ok(Snapshot {
            fixtures: fixtures(&value)?,
            hole_states: hole_states(&value)?,
            negative_controls: negative_controls(&value)?,
            documented_edits: documented_edits(&value)?,
            hostile_transport: hostile_transport(&value)?,
        })
    }

    pub fn fixture(&self, name: &str) -> Option<&Fixture> {
        self.fixtures.iter().find(|fixture| fixture.name == name)
    }
}

fn array<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a [serde_json::Value], String> {
    let Some(items) = value.get(key).and_then(serde_json::Value::as_array) else {
        return Err(format!("corpus field {key} must be an array"));
    };
    Ok(items.as_slice())
}

fn fixtures(value: &serde_json::Value) -> Result<Vec<Fixture>, String> {
    let items = array(value, "fixtures")?;
    let mut fixtures = Vec::with_capacity(items.len());
    for item in items {
        fixtures.push(fixture(item)?);
    }
    Ok(fixtures)
}

fn hole_states(value: &serde_json::Value) -> Result<Vec<HoleState>, String> {
    let items = array(value, "hole_states")?;
    let mut states = Vec::with_capacity(items.len());
    for item in items {
        states.push(HoleState {
            state: text(item, "state")?,
            fixture: text(item, "fixture")?,
        });
    }
    Ok(states)
}

fn negative_controls(value: &serde_json::Value) -> Result<Vec<NegativeControl>, String> {
    let items = array(value, "negative_controls")?;
    let mut controls = Vec::with_capacity(items.len());
    for item in items {
        controls.push(NegativeControl {
            name: text(item, "name")?,
            fixture: text(item, "fixture")?,
            gesture: gesture(field(item, "gesture")?)?,
            code: text(item, "code")?,
        });
    }
    Ok(controls)
}

fn documented_edits(value: &serde_json::Value) -> Result<Vec<DocumentedEdit>, String> {
    let items = array(value, "documented_edits")?;
    let mut documented = Vec::with_capacity(items.len());
    for item in items {
        let legs = array(item, "edits")?;
        let mut edits = Vec::with_capacity(legs.len());
        for leg in legs {
            edits.push(
                leg.as_str()
                    .ok_or("documented edit leg names must be text")?
                    .to_owned(),
            );
        }
        documented.push(DocumentedEdit {
            name: text(item, "name")?,
            edits,
        });
    }
    Ok(documented)
}

fn hostile_transport(value: &serde_json::Value) -> Result<Vec<String>, String> {
    let items = array(value, "hostile_transport")?;
    let mut documents = Vec::with_capacity(items.len());
    for item in items {
        documents.push(
            item.as_str()
                .ok_or("hostile transport documents must be text")?
                .to_owned(),
        );
    }
    Ok(documents)
}

pub(super) fn gesture(value: &serde_json::Value) -> Result<Gesture, String> {
    let kind = text(value, "kind")?;
    let path = path(value)?;
    Ok(match kind.as_str() {
        "set-number" => Gesture::SetNumber {
            path,
            text: text(value, "text")?,
        },
        "set-checkbox" => Gesture::SetCheckbox {
            path,
            text: text(value, "text")?,
        },
        "toggle" => Gesture::Toggle { path },
        "set-word" => Gesture::SetWord {
            path,
            text: text(value, "text")?,
        },
        "leave-hole" => Gesture::LeaveHole { path },
        "fill-hole" => Gesture::FillHole {
            path,
            fill: fill(field(value, "fill")?)?,
        },
        other => return Err(format!("unsupported gesture kind {other}")),
    })
}

/// Decode a fill payload with an explicit work stack: nested quotation fills
/// are folded bottom-up, bounded by the document's own nesting.
fn fill(value: &serde_json::Value) -> Result<Fill, String> {
    if text(value, "kind")? != "quotation" {
        return leaf_fill(value);
    }
    struct Frame<'a> {
        items: &'a [serde_json::Value],
        next: usize,
        built: Vec<Fill>,
    }
    let mut current = quotation_children(value)?;
    let mut next = 0usize;
    let mut built = Vec::with_capacity(current.len());
    let mut stack: Vec<Frame<'_>> = Vec::new();
    loop {
        if next == current.len() {
            let folded = Fill::Quotation(built);
            let Some(parent) = stack.pop() else {
                return Ok(folded);
            };
            current = parent.items;
            next = parent.next;
            built = parent.built;
            built.push(folded);
            continue;
        }
        let children = {
            let item = &current[next];
            if text(item, "kind")? != "quotation" {
                built.push(leaf_fill(item)?);
                next = next.saturating_add(1);
                continue;
            }
            quotation_children(item)?
        };
        next = next.saturating_add(1);
        stack.push(Frame {
            items: current,
            next,
            built,
        });
        current = children;
        next = 0;
        built = Vec::with_capacity(current.len());
    }
}

fn leaf_fill(value: &serde_json::Value) -> Result<Fill, String> {
    match text(value, "kind")?.as_str() {
        "number" => Ok(Fill::Number(text(value, "text")?)),
        "boolean" => Ok(Fill::Boolean(text(value, "text")?)),
        "word" => Ok(Fill::Word(text(value, "text")?)),
        "hole" => Ok(Fill::Hole),
        other => Err(format!("unsupported fill kind {other}")),
    }
}

fn quotation_children(value: &serde_json::Value) -> Result<&[serde_json::Value], String> {
    let Some(items) = value.get("children").and_then(serde_json::Value::as_array) else {
        return Err(String::from("quotation fill children must be an array"));
    };
    Ok(items.as_slice())
}
