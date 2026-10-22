//! Local-only, no-capture worker package. A manifest is a claim about one
//! independently checked, exported named Program; it is never code authority.

use super::worker_value::{Refusal, Schema};
use serde_json::{json, Value};

const BYTE_LIMIT: usize = 262_144;
const NODE_LIMIT: usize = 4_096;
const DEPTH_LIMIT: usize = 32;
const MEMORY_LIMIT: usize = 1_048_576;
const PARSER_WORK: usize = 1_000_000;
pub(super) const REVISION: &str = "0.1.0-draft.5";

pub(crate) struct Package {
    document: Value,
    pub(crate) memory: usize,
    pub(crate) remaining_work: usize,
}

/// Why a declared worker interface differs from the host-selected dispatch
/// interface `State Event -- State List<Action> ! {}` (WORKER-02). Each class is
/// decided from the package claim against the independently checked schema
/// catalog; none guesses arguments, converts schemas or falls back to `Any`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InterfaceReject {
    /// Stack arity, order or list element shape differs.
    Interface,
    /// A declared schema identity is not a checked host catalog row, even if
    /// its layout digest matches one.
    Schema,
    /// A declared latent effect lies outside the empty dispatch ceiling.
    Effect,
    /// The interface claim is not the selected wire shape.
    Malformed,
}

/// Every schema wire named by a declared stack entry: a bare wire, or each
/// element wire of a `{"list": ...}` entry whose element may be one wire or an
/// array of wires (a heterogeneous list claim).
fn declared_wires(entry: &Value) -> Result<Vec<&Value>, InterfaceReject> {
    let Some(fields) = entry.as_object() else {
        return Err(InterfaceReject::Malformed);
    };
    if fields.len() != 1 || !fields.contains_key("list") {
        return Ok(vec![entry]);
    }
    match &entry["list"] {
        Value::Array(elements) if !elements.is_empty() => Ok(elements.iter().collect()),
        element @ Value::Object(_) => Ok(vec![element]),
        _ => Err(InterfaceReject::Malformed),
    }
}

/// Resource search over a claimed capture value at every depth. The worklist
/// holds at most the package's node count, which NODE_LIMIT bounds before this
/// walk runs.
fn hides_resource(value: &Value) -> bool {
    let mut pending = vec![value];
    while let Some(next) = pending.pop() {
        match next {
            Value::Object(fields) => {
                if fields.get("tag").and_then(Value::as_str) == Some("resource") {
                    return true;
                }
                pending.extend(fields.values());
            }
            Value::Array(items) => pending.extend(items),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    false
}

fn object<'a>(
    value: &'a Value,
    keys: &[&str],
) -> Result<&'a serde_json::Map<String, Value>, Refusal> {
    let fields = value.as_object().ok_or(Refusal::Invalid)?;
    if fields.len() != keys.len() || keys.iter().any(|key| !fields.contains_key(*key)) {
        return Err(Refusal::Invalid);
    }
    Ok(fields)
}

fn decimal(value: &Value) -> Result<u64, Refusal> {
    let text = value.as_str().ok_or(Refusal::Invalid)?;
    let number = text.parse::<u64>().map_err(|_| Refusal::Invalid)?;
    if number.to_string() != text {
        return Err(Refusal::Invalid);
    }
    Ok(number)
}

fn ordinal(value: &Value) -> Result<u32, Refusal> {
    let number = value.as_u64().ok_or(Refusal::Invalid)?;
    u32::try_from(number).map_err(|_| Refusal::Invalid)
}

fn digest(value: &Value) -> Result<&str, Refusal> {
    let text = value.as_str().ok_or(Refusal::Invalid)?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Refusal::Invalid);
    }
    Ok(text)
}

fn source(value: &Value) -> Result<&str, Refusal> {
    let text = value.as_str().ok_or(Refusal::Invalid)?;
    if text.is_empty() || text.len() > 65_536 {
        return Err(Refusal::Exhausted);
    }
    Ok(text)
}

fn schema_wire(value: &Value) -> Result<(), Refusal> {
    object(value, &["module_id", "version", "ordinal", "shape_sha256"])?;
    decimal(&value["module_id"])?;
    ordinal(&value["version"])?;
    ordinal(&value["ordinal"])?;
    digest(&value["shape_sha256"])?;
    Ok(())
}

fn count_nodes(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), Refusal> {
    if depth > DEPTH_LIMIT {
        return Err(Refusal::Exhausted);
    }
    *nodes = nodes.checked_add(1).ok_or(Refusal::Exhausted)?;
    if *nodes > NODE_LIMIT {
        return Err(Refusal::Exhausted);
    }
    if let Some(object) = value.as_object() {
        for entry in object.values() {
            count_nodes(entry, depth + 1, nodes)?;
        }
    } else if let Some(array) = value.as_array() {
        for entry in array {
            count_nodes(entry, depth + 1, nodes)?;
        }
    }
    Ok(())
}

impl Package {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, Refusal> {
        if bytes.len() > BYTE_LIMIT {
            return Err(Refusal::Exhausted);
        }
        let mut depth = 0usize;
        let mut quote = false;
        let mut escape = false;
        let mut tokens = 0usize;
        for byte in bytes {
            if quote {
                if escape {
                    escape = false;
                } else if *byte == b'\\' {
                    escape = true;
                } else if *byte == b'"' {
                    quote = false;
                }
                continue;
            }
            match byte {
                b'"' => {
                    quote = true;
                    tokens += 1;
                }
                b'[' | b'{' => {
                    depth += 1;
                    tokens += 1;
                }
                b']' | b'}' => {
                    if depth == 0 {
                        return Err(Refusal::Invalid);
                    }
                    depth -= 1;
                }
                b',' => tokens += 1,
                _ => {}
            }
            if tokens > NODE_LIMIT * 8 || depth > DEPTH_LIMIT {
                return Err(Refusal::Exhausted);
            }
        }
        if quote || depth != 0 {
            return Err(Refusal::Invalid);
        }
        // Raw bytes, decoded strings and the canonical reserialization are
        // simultaneously live until admission finishes.
        let memory = bytes
            .len()
            .checked_mul(3)
            .and_then(|count| count.checked_add(tokens.checked_mul(128)?))
            .ok_or(Refusal::Exhausted)?;
        let work = bytes
            .len()
            .checked_mul(3)
            .and_then(|count| count.checked_add(tokens.checked_mul(8)?))
            .ok_or(Refusal::Exhausted)?;
        if memory > MEMORY_LIMIT || work > PARSER_WORK {
            return Err(Refusal::Exhausted);
        }
        let document: Value = serde_json::from_slice(bytes).map_err(|_| Refusal::Invalid)?;
        if serde_json::to_vec(&document).map_err(|_| Refusal::Invalid)? != bytes {
            return Err(Refusal::Invalid);
        }
        let mut nodes = 0;
        count_nodes(&document, 0, &mut nodes)?;
        object(
            &document,
            &[
                "format",
                "semantic_revision",
                "root",
                "captures",
                "interface",
                "definitions",
                "schemas",
                "builtins",
                "host_contracts",
                "artifact",
            ],
        )?;
        if document["format"] != "noble-worker-local-package/v1"
            || document["semantic_revision"] != REVISION
        {
            return Err(Refusal::Unsupported);
        }
        if document["artifact"] != Value::Null {
            return Err(Refusal::Unsupported);
        }
        let captures = document["captures"]
            .as_array()
            .ok_or(Refusal::Unsupported)?;
        // Local v1 admits no captures. A resource hidden at any depth is still
        // refused for eligibility first, so a wrapper cannot launder it into
        // a merely unsupported capture.
        if captures.iter().any(hides_resource) {
            return Err(Refusal::Ineligible);
        }
        if !captures.is_empty()
            || document["host_contracts"]
                .as_array()
                .filter(|value| value.is_empty())
                .is_none()
        {
            return Err(Refusal::Unsupported);
        }
        let root = &document["root"];
        object(
            root,
            &[
                "source_utf8",
                "module_id",
                "version",
                "definition_ordinal",
                "definition_identity",
            ],
        )?;
        let root_source = source(&root["source_utf8"])?;
        let root_id = decimal(&root["module_id"])?;
        let root_version = ordinal(&root["version"])?;
        let root_ordinal = ordinal(&root["definition_ordinal"])?;
        let root_identity = decimal(&root["definition_identity"])?;
        if root_version == 0 {
            return Err(Refusal::Invalid);
        }
        let definitions = document["definitions"]
            .as_array()
            .filter(|rows| !rows.is_empty() && rows.len() <= 64)
            .ok_or(Refusal::Exhausted)?;
        let mut previous = None;
        let mut root_found = false;
        for row in definitions {
            object(
                row,
                &[
                    "module_id",
                    "version",
                    "definition_ordinal",
                    "definition_identity",
                    "source_utf8",
                    "source_sha256",
                ],
            )?;
            let key = (
                decimal(&row["module_id"])?,
                ordinal(&row["version"])?,
                ordinal(&row["definition_ordinal"])?,
            );
            if previous.is_some_and(|prior| prior >= key) {
                return Err(Refusal::Invalid);
            }
            previous = Some(key);
            let body = source(&row["source_utf8"])?;
            let identity = decimal(&row["definition_identity"])?;
            if digest(&row["source_sha256"])? != crate::workflow::intrinsic::sha256(body.as_bytes())
            {
                return Err(Refusal::Invalid);
            }
            if key == (root_id, root_version, root_ordinal) {
                if body != root_source || identity != root_identity {
                    return Err(Refusal::Invalid);
                }
                root_found = true;
            }
        }
        if !root_found {
            return Err(Refusal::Invalid);
        }
        let schemas = document["schemas"]
            .as_array()
            .filter(|rows| rows.len() <= 64)
            .ok_or(Refusal::Exhausted)?;
        previous = None;
        for row in schemas {
            object(
                row,
                &[
                    "module_id",
                    "version",
                    "ordinal",
                    "shape_sha256",
                    "source_sha256",
                ],
            )?;
            let key = (
                decimal(&row["module_id"])?,
                ordinal(&row["version"])?,
                ordinal(&row["ordinal"])?,
            );
            if previous.is_some_and(|prior| prior >= key) {
                return Err(Refusal::Invalid);
            }
            previous = Some(key);
            digest(&row["shape_sha256"])?;
            digest(&row["source_sha256"])?;
        }
        let builtins = document["builtins"]
            .as_array()
            .filter(|rows| rows.len() == 22)
            .ok_or(Refusal::Unsupported)?;
        for (identity, row) in builtins.iter().enumerate() {
            object(row, &["ordinal", "name", "semantic_revision"])?;
            if ordinal(&row["ordinal"])? != u32::try_from(identity).map_err(|_| Refusal::Invalid)?
                || row["name"]
                    != noble_contracts::source::pure_builtin_spelling(identity as u32)
                        .ok_or(Refusal::Invalid)?
                || row["semantic_revision"] != REVISION
            {
                return Err(Refusal::Invalid);
            }
        }
        let interface = &document["interface"];
        object(interface, &["inputs", "outputs", "effects"])?;
        // Arity, order, schema identity and effects are classified by
        // check_interface against the checked host catalog (WORKER-02).
        if ["inputs", "outputs", "effects"]
            .iter()
            .any(|key| !interface[*key].is_array())
        {
            return Err(Refusal::Invalid);
        }
        Ok(Self {
            document,
            memory,
            remaining_work: PARSER_WORK - work,
        })
    }

    pub(crate) fn source(&self) -> &str {
        self.document["root"]["source_utf8"]
            .as_str()
            .expect("parsed source")
    }

    pub(crate) fn bind_owner(
        &self,
        owner: &noble_contracts::source::CheckedProgramOwner,
        closure: &[noble_contracts::source::CheckedProgramOwner],
    ) -> Result<(), Refusal> {
        let root = &self.document["root"];
        if decimal(&root["module_id"])? != owner.module_identity()
            || ordinal(&root["version"])? != owner.module_version()
            || ordinal(&root["definition_ordinal"])? != owner.definition_ordinal()
            || decimal(&root["definition_identity"])? != owner.definition_identity()
        {
            return Err(Refusal::Invalid);
        }
        let rows = self.document["definitions"]
            .as_array()
            .ok_or(Refusal::Invalid)?;
        if rows.len() != closure.len() {
            return Err(Refusal::Invalid);
        }
        for (row, definition) in rows.iter().zip(closure) {
            if decimal(&row["module_id"])? != definition.module_identity()
                || ordinal(&row["version"])? != definition.module_version()
                || ordinal(&row["definition_ordinal"])? != definition.definition_ordinal()
                || decimal(&row["definition_identity"])? != definition.definition_identity()
                || row["source_utf8"] != self.document["root"]["source_utf8"]
            {
                return Err(Refusal::Invalid);
            }
        }
        Ok(())
    }

    pub(crate) fn bind_schemas(
        &self,
        schemas: &[Schema],
        schema_source: &[u8],
    ) -> Result<(), Refusal> {
        let rows = self.document["schemas"]
            .as_array()
            .ok_or(Refusal::Invalid)?;
        if rows.len() != schemas.len() {
            return Err(Refusal::Invalid);
        }
        let digest = crate::workflow::intrinsic::sha256(schema_source);
        for (row, schema) in rows.iter().zip(schemas) {
            if row != &schema.package_row(&digest) {
                return Err(Refusal::Invalid);
            }
        }
        Ok(())
    }

    /// Check the declared worker interface claim against the checked host
    /// catalog and the selected dispatch interface, in this order: the empty
    /// effect ceiling, then exact schema identities, then the exact stack.
    pub(crate) fn check_interface(
        &self,
        catalog: &[Schema],
        state: &Schema,
        event: &Schema,
        action: &Schema,
    ) -> Result<(), InterfaceReject> {
        let declared = &self.document["interface"];
        let Some(fields) = declared.as_object() else {
            return Err(InterfaceReject::Malformed);
        };
        let (Some(inputs), Some(outputs), Some(effects)) = (
            declared["inputs"].as_array(),
            declared["outputs"].as_array(),
            declared["effects"].as_array(),
        ) else {
            return Err(InterfaceReject::Malformed);
        };
        if fields.len() != 3 {
            return Err(InterfaceReject::Malformed);
        }
        if !effects.is_empty() {
            return Err(InterfaceReject::Effect);
        }
        for entry in inputs.iter().chain(outputs) {
            for wire in declared_wires(entry)? {
                if schema_wire(wire).is_err() {
                    return Err(InterfaceReject::Malformed);
                }
                if !catalog.iter().any(|schema| schema.wire() == wire) {
                    return Err(InterfaceReject::Schema);
                }
            }
        }
        if inputs.as_slice() != [state.wire().clone(), event.wire().clone()]
            || outputs.as_slice() != [state.wire().clone(), json!({"list":action.wire()})]
        {
            return Err(InterfaceReject::Interface);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> Value {
        let source = "module worker@1 [ export transition def transition [ drop nil ] ]";
        let schema = json!({"module_id":"42","version":1,"ordinal":0,
            "shape_sha256":"0000000000000000000000000000000000000000000000000000000000000000"});
        json!({
            "format":"noble-worker-local-package/v1",
            "semantic_revision":REVISION,
            "root":{"source_utf8":source,"module_id":"7","version":1,
                "definition_ordinal":0,"definition_identity":"19"},
            "definitions":[{"module_id":"7","version":1,"definition_ordinal":0,
                "definition_identity":"19","source_utf8":source,
                "source_sha256":crate::workflow::intrinsic::sha256(source.as_bytes())}],
            "schemas":[],"captures":[],"host_contracts":[],"artifact":null,
            "interface":{"inputs":[schema.clone(),schema.clone()],
                "outputs":[schema.clone(),{"list":schema}],"effects":[]},
            "builtins":(0..22).map(|id| json!({"ordinal":id,
                "name":noble_contracts::source::pure_builtin_spelling(id).unwrap(),
                "semantic_revision":REVISION})).collect::<Vec<_>>(),
        })
    }

    fn parse(document: &Value) -> Result<Package, Refusal> {
        Package::parse(&serde_json::to_vec(document).expect("JSON test case"))
    }

    #[test]
    fn package_refuses_conflicting_root_identity_duplicate_dependencies_and_artifact() {
        let accepted = document();
        assert!(parse(&accepted).is_ok());

        let mut wrong_version = accepted.clone();
        wrong_version["root"]["version"] = json!(2);
        assert!(matches!(parse(&wrong_version), Err(Refusal::Invalid)));

        let mut wrong_owner = accepted.clone();
        wrong_owner["root"]["definition_identity"] = json!("20");
        assert!(matches!(parse(&wrong_owner), Err(Refusal::Invalid)));

        let mut duplicate = accepted.clone();
        let mut row = duplicate["definitions"][0].clone();
        let alternate = "module worker@1 [ export transition def transition [ drop unit ] ]";
        row["source_utf8"] = json!(alternate);
        row["source_sha256"] = json!(crate::workflow::intrinsic::sha256(alternate.as_bytes()));
        duplicate["definitions"].as_array_mut().unwrap().push(row);
        assert!(matches!(parse(&duplicate), Err(Refusal::Invalid)));

        let mut artifact = accepted.clone();
        artifact["artifact"] = json!({"wasm":"preapproved"});
        assert!(matches!(parse(&artifact), Err(Refusal::Unsupported)));

        let mut capture = accepted.clone();
        capture["captures"] = json!([{"tag":"resource","value":"hidden"}]);
        assert!(matches!(parse(&capture), Err(Refusal::Ineligible)));

        let mut wrapped = accepted.clone();
        wrapped["captures"] = json!([{"tag":"pair","left":{"tag":"i64","value":"1"},
            "right":{"tag":"opaque","schema":{},"value":{"tag":"resource","value":"hidden"}}}]);
        assert!(matches!(parse(&wrapped), Err(Refusal::Ineligible)));

        let mut data_capture = accepted.clone();
        data_capture["captures"] = json!([{"tag":"i64","value":"1"}]);
        assert!(matches!(parse(&data_capture), Err(Refusal::Unsupported)));

        let mut host_contract = accepted.clone();
        host_contract["host_contracts"] = json!([{"operation":"test.emit","adapter":"claimed"}]);
        assert!(matches!(parse(&host_contract), Err(Refusal::Unsupported)));

        let mut builtin = accepted;
        builtin["builtins"][0]["ordinal"] = json!(1);
        assert!(matches!(parse(&builtin), Err(Refusal::Invalid)));
    }

    #[test]
    fn package_refuses_overbudget_and_unsupported_wire_before_source_check() {
        assert!(matches!(
            Package::parse(&vec![b' '; BYTE_LIMIT + 1]),
            Err(Refusal::Exhausted)
        ));
        let mut document = document();
        document["semantic_revision"] = json!("future");
        assert!(matches!(parse(&document), Err(Refusal::Unsupported)));
        document["semantic_revision"] = json!(REVISION);
        document["opaque_extra"] = json!(true);
        assert!(matches!(parse(&document), Err(Refusal::Invalid)));
    }
}
