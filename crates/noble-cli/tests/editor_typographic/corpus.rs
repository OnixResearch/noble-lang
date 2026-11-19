//! The frozen corpus loader: documents, edits, hole states and negative
//! controls as committed before the measurement that they govern.

use noble_contracts::source::editor::Node;

use super::document::decode;
use super::view::{Fill, Gesture};

pub const CORPUS: &str = "verification/editor-typographic/corpus.json";
pub const CRITERIA: &str = "verification/editor-typographic/criteria.json";

pub struct Corpus {
    pub fixtures: Vec<Fixture>,
    pub hole_states: Vec<HoleState>,
    pub negative_controls: Vec<NegativeControl>,
    pub documented_edits: Vec<DocumentedEdit>,
    pub hostile_transport: Vec<String>,
}

pub struct Fixture {
    pub name: String,
    pub nodes: Vec<Node>,
    pub kinds: Vec<String>,
    pub expect: Expect,
    pub edits: Vec<Edit>,
}

pub struct Expect {
    pub analyze_exit: i32,
    pub holes: usize,
    pub program_effect_known: Vec<u32>,
    pub program_effect_unresolved: bool,
    pub admit_exit: i32,
    pub admit_stage: String,
    pub admit_outcome: String,
    pub admit_category: Option<String>,
}

pub struct Edit {
    pub name: String,
    pub gesture: Gesture,
    pub nodes: Vec<Node>,
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

impl Corpus {
    pub fn load() -> Result<Self, String> {
        let document_text = std::fs::read_to_string(super::lane(CORPUS))
            .map_err(|error| format!("read corpus: {error}"))?;
        let value: serde_json::Value = serde_json::from_str(&document_text)
            .map_err(|error| format!("parse corpus: {error}"))?;
        if value.get("schema").and_then(serde_json::Value::as_str)
            != Some("noble-editor-typographic-corpus/v1")
        {
            return Err(String::from("corpus schema is not frozen v1"));
        }
        let mut fixtures = Vec::new();
        let Some(items) = value.get("fixtures").and_then(serde_json::Value::as_array) else {
            return Err(String::from("corpus fixtures must be an array"));
        };
        for item in items {
            fixtures.push(fixture(item)?);
        }
        let mut hole_states = Vec::new();
        let Some(items) = value
            .get("hole_states")
            .and_then(serde_json::Value::as_array)
        else {
            return Err(String::from("corpus hole_states must be an array"));
        };
        for item in items {
            hole_states.push(HoleState {
                state: text(item, "state")?,
                fixture: text(item, "fixture")?,
            });
        }
        let mut negative_controls = Vec::new();
        let Some(items) = value
            .get("negative_controls")
            .and_then(serde_json::Value::as_array)
        else {
            return Err(String::from("corpus negative_controls must be an array"));
        };
        for item in items {
            negative_controls.push(NegativeControl {
                name: text(item, "name")?,
                fixture: text(item, "fixture")?,
                gesture: gesture(field(item, "gesture")?)?,
                code: text(item, "code")?,
            });
        }
        let mut documented_edits = Vec::new();
        let Some(items) = value
            .get("documented_edits")
            .and_then(serde_json::Value::as_array)
        else {
            return Err(String::from("corpus documented_edits must be an array"));
        };
        for item in items {
            let mut edits = Vec::new();
            let Some(names) = item.get("edits").and_then(serde_json::Value::as_array) else {
                return Err(String::from("documented edit legs must be an array"));
            };
            for name in names {
                edits.push(
                    name.as_str()
                        .ok_or("documented edit leg names must be text")?
                        .to_owned(),
                );
            }
            documented_edits.push(DocumentedEdit {
                name: text(item, "name")?,
                edits,
            });
        }
        let mut hostile_transport = Vec::new();
        let Some(items) = value
            .get("hostile_transport")
            .and_then(serde_json::Value::as_array)
        else {
            return Err(String::from("corpus hostile_transport must be an array"));
        };
        for item in items {
            hostile_transport.push(
                item.as_str()
                    .ok_or("hostile transport documents must be text")?
                    .to_owned(),
            );
        }
        Ok(Corpus {
            fixtures,
            hole_states,
            negative_controls,
            documented_edits,
            hostile_transport,
        })
    }

    pub fn fixture(&self, name: &str) -> Option<&Fixture> {
        self.fixtures.iter().find(|fixture| fixture.name == name)
    }
}

impl Fixture {
    /// The exact bytes the CLI reads for this fixture.
    pub fn document(&self) -> Result<String, String> {
        super::document::encode(&self.nodes).map_err(|refusal| refusal.detail)
    }
}

impl Edit {
    pub fn document(&self) -> Result<String, String> {
        super::document::encode(&self.nodes).map_err(|refusal| refusal.detail)
    }
}

fn fixture(value: &serde_json::Value) -> Result<Fixture, String> {
    let name = text(value, "name")?;
    let document = field(value, "document")?;
    let compact = serde_json::to_string(document)
        .map_err(|error| format!("serialize fixture document: {error}"))?;
    let nodes = decode(&compact).map_err(|refusal| format!("{name}: {}", refusal.detail))?;
    let kinds = strings(field(value, "kinds")?)?;
    let expect = expect(field(value, "expect")?)?;
    let mut edits = Vec::new();
    let Some(items) = value.get("edits").and_then(serde_json::Value::as_array) else {
        return Err(format!("{name}: edits must be an array"));
    };
    for item in items {
        let edit_name = text(item, "name")?;
        let gesture = gesture(field(item, "gesture")?)?;
        let Some(nodes) = item.get("json_nodes").and_then(serde_json::Value::as_array) else {
            return Err(format!("{name}/{edit_name}: json_nodes must be an array"));
        };
        let envelope = serde_json::json!({ "format": 1, "nodes": nodes });
        let text = serde_json::to_string(&envelope)
            .map_err(|error| format!("serialize {name}/{edit_name}: {error}"))?;
        let nodes =
            decode(&text).map_err(|refusal| format!("{name}/{edit_name}: {}", refusal.detail))?;
        edits.push(Edit {
            name: edit_name,
            gesture,
            nodes,
        });
    }
    Ok(Fixture {
        name,
        nodes,
        kinds,
        expect,
        edits,
    })
}

fn expect(value: &serde_json::Value) -> Result<Expect, String> {
    Ok(Expect {
        analyze_exit: number(value, "analyze_exit")?,
        holes: usize::try_from(number(value, "holes")?)
            .map_err(|_| String::from("hole count must be non-negative"))?,
        program_effect_known: numbers(value, "program_effect_known")?,
        program_effect_unresolved: boolean(value, "program_effect_unresolved")?,
        admit_exit: number(value, "admit_exit")?,
        admit_stage: text(value, "admit_stage")?,
        admit_outcome: text(value, "admit_outcome")?,
        admit_category: match value.get("admit_category") {
            Some(serde_json::Value::String(category)) => Some(category.clone()),
            Some(serde_json::Value::Null) | None => None,
            Some(_) => return Err(String::from("admit_category must be text or null")),
        },
    })
}

fn gesture(value: &serde_json::Value) -> Result<Gesture, String> {
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

fn fill(value: &serde_json::Value) -> Result<Fill, String> {
    let kind = text(value, "kind")?;
    Ok(match kind.as_str() {
        "number" => Fill::Number(text(value, "text")?),
        "boolean" => Fill::Boolean(text(value, "text")?),
        "word" => Fill::Word(text(value, "text")?),
        "hole" => Fill::Hole,
        "quotation" => {
            let Some(items) = value.get("children").and_then(serde_json::Value::as_array) else {
                return Err(String::from("quotation fill children must be an array"));
            };
            let mut children = Vec::new();
            for item in items {
                children.push(fill(item)?);
            }
            Fill::Quotation(children)
        }
        other => return Err(format!("unsupported fill kind {other}")),
    })
}

fn strings(value: &serde_json::Value) -> Result<Vec<String>, String> {
    let Some(items) = value.as_array() else {
        return Err(String::from("kinds must be an array"));
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_owned)
                .ok_or_else(|| String::from("kinds entries must be text"))
        })
        .collect()
}

fn field<'a>(value: &'a serde_json::Value, key: &str) -> Result<&'a serde_json::Value, String> {
    value
        .get(key)
        .ok_or_else(|| format!("missing corpus field {key}"))
}

fn text(value: &serde_json::Value, key: &str) -> Result<String, String> {
    field(value, key)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("corpus field {key} must be text"))
}

fn number(value: &serde_json::Value, key: &str) -> Result<i32, String> {
    let raw = field(value, key)?
        .as_i64()
        .ok_or_else(|| format!("corpus field {key} must be a number"))?;
    i32::try_from(raw).map_err(|_| format!("corpus field {key} is outside i32"))
}

fn boolean(value: &serde_json::Value, key: &str) -> Result<bool, String> {
    field(value, key)?
        .as_bool()
        .ok_or_else(|| format!("corpus field {key} must be a boolean"))
}

fn numbers(value: &serde_json::Value, key: &str) -> Result<Vec<u32>, String> {
    let Some(items) = field(value, key)?.as_array() else {
        return Err(format!("corpus field {key} must be an array"));
    };
    let mut numbers = Vec::new();
    for item in items {
        let raw = item
            .as_u64()
            .ok_or_else(|| format!("corpus field {key} entries must be unsigned"))?;
        numbers.push(u32::try_from(raw).map_err(|_| format!("{key} entry outside u32"))?);
    }
    Ok(numbers)
}

fn path(value: &serde_json::Value) -> Result<Vec<u32>, String> {
    numbers(value, "path")
}
