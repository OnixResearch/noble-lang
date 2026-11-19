//! The frozen fixtures: documents, their frozen expectations and edits.

use noble_contracts::source::editor::Node;

use super::apply::Gesture;
use super::document::decode;

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

pub(super) fn fixture(value: &serde_json::Value) -> Result<Fixture, String> {
    let name = text(value, "name")?;
    let document = field(value, "document")?;
    let compact = serde_json::to_string(document)
        .map_err(|error| format!("serialize fixture document: {error}"))?;
    let nodes = decode(&compact).map_err(|refusal| format!("{name}: {}", refusal.detail))?;
    let kinds = strings(field(value, "kinds")?)?;
    let expect = expect(field(value, "expect")?)?;
    let Some(items) = value.get("edits").and_then(serde_json::Value::as_array) else {
        return Err(format!("{name}: edits must be an array"));
    };
    let mut edits = Vec::with_capacity(items.len());
    for item in items {
        let edit_name = text(item, "name")?;
        let gesture = super::frozen::gesture(field(item, "gesture")?)?;
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

pub(super) fn field<'a>(
    value: &'a serde_json::Value,
    key: &str,
) -> Result<&'a serde_json::Value, String> {
    value
        .get(key)
        .ok_or_else(|| format!("missing corpus field {key}"))
}

pub(super) fn text(value: &serde_json::Value, key: &str) -> Result<String, String> {
    field(value, key)?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| format!("corpus field {key} must be text"))
}

pub(super) fn number(value: &serde_json::Value, key: &str) -> Result<i32, String> {
    let raw = field(value, key)?
        .as_i64()
        .ok_or_else(|| format!("corpus field {key} must be a number"))?;
    i32::try_from(raw).map_err(|_| format!("corpus field {key} is outside i32"))
}

pub(super) fn boolean(value: &serde_json::Value, key: &str) -> Result<bool, String> {
    field(value, key)?
        .as_bool()
        .ok_or_else(|| format!("corpus field {key} must be a boolean"))
}

pub(super) fn numbers(value: &serde_json::Value, key: &str) -> Result<Vec<u32>, String> {
    let Some(items) = field(value, key)?.as_array() else {
        return Err(format!("corpus field {key} must be an array"));
    };
    let mut numbers = Vec::with_capacity(items.len());
    for item in items {
        let raw = item
            .as_u64()
            .ok_or_else(|| format!("corpus field {key} entries must be unsigned"))?;
        numbers.push(u32::try_from(raw).map_err(|_| format!("{key} entry outside u32"))?);
    }
    Ok(numbers)
}

pub(super) fn strings(value: &serde_json::Value) -> Result<Vec<String>, String> {
    let Some(items) = value.as_array() else {
        return Err(String::from("kinds must be an array"));
    };
    let mut strings = Vec::with_capacity(items.len());
    for item in items {
        strings.push(
            item.as_str()
                .ok_or("kinds entries must be text")?
                .to_owned(),
        );
    }
    Ok(strings)
}

pub(super) fn path(value: &serde_json::Value) -> Result<Vec<u32>, String> {
    numbers(value, "path")
}
