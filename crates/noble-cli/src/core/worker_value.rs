//! Host-selected, source-resolved data ingress for the typed worker service.
//! Wire schema names are claims; only the independently supplied catalog and
//! expected kernel types decide admission. No Program or live owner is decoded.

use noble_kernel::types::{NominalShape, NominalTypeId, Ty};
use serde_json::{Value, json};

const BYTE_LIMIT: usize = 65_536;
const NODE_LIMIT: usize = 4_096;
const DEPTH_LIMIT: usize = 32;
const MEMORY_LIMIT: usize = 1_048_576;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Invalid,
    Unsupported,
    Exhausted,
}

/// Construct these entries only from independently checked ModuleSession
/// declarations. Visibility is checked against exported constructor words.
pub(crate) struct Schema {
    pub(crate) ty: Ty,
    opaque_public: bool,
    left_public: bool,
    right_public: bool,
    wire: Value,
}

impl Schema {
    pub(crate) fn checked(selected: &noble_contracts::source::CheckedNominal<'_>) -> Result<Self, Refusal> {
        let declaration = selected.declaration();
        if selected.module_version() == 0 || declaration.id.module != selected.module_identity()
            || !declaration.exported { return Err(Refusal::Invalid) }
        let ty = Ty::Nominal(declaration.id, Box::new(declaration.shape.clone()));
        if !ty.is_data() { return Err(Refusal::Unsupported) }
        let descriptor = shape_descriptor(&declaration.shape, 0)?;
        let bytes = serde_json::to_vec(&descriptor).map_err(|_| Refusal::Invalid)?;
        let wire = json!({ "module_id": declaration.id.module.to_string(), "version": selected.module_version(),
            "ordinal": declaration.id.ordinal,
            "shape_sha256": crate::workflow::intrinsic::sha256(&bytes) });
        Ok(Self { ty, opaque_public: declaration.public[0],
            left_public: declaration.public[0], right_public: declaration.public[1], wire })
    }

    pub(crate) fn id(&self) -> NominalTypeId {
        let Ty::Nominal(id, _) = &self.ty else { unreachable!("checked schema has nominal identity") };
        *id
    }

    pub(crate) fn wire(&self) -> &Value {
        &self.wire
    }

    pub(crate) fn package_row(&self, source_sha256: &str) -> Value {
        let mut row = self.wire.as_object().expect("checked schema record").clone();
        row.insert("source_sha256".to_owned(), json!(source_sha256));
        Value::Object(row)
    }

    pub(crate) fn runtime_row(&self) -> Result<Value, Refusal> {
        let Ty::Nominal(_, shape) = &self.ty else { return Err(Refusal::Unsupported) };
        Ok(json!({ "schema":self.wire(), "shape":shape_descriptor(shape, 0)?,
            "constructors": {
                "opaque":self.opaque_public,"left":self.left_public,"right":self.right_public
            } }))
    }
}

fn shape_descriptor(shape: &NominalShape, depth: usize) -> Result<Value, Refusal> {
    if depth >= DEPTH_LIMIT { return Err(Refusal::Exhausted) }
    match shape {
        NominalShape::Opaque(value) => Ok(json!(["opaque", type_descriptor(value, depth + 1)?])),
        NominalShape::Variant(left, right) => Ok(json!(["variant",
            type_descriptor(left, depth + 1)?, type_descriptor(right, depth + 1)?])),
    }
}

fn type_descriptor(ty: &Ty, depth: usize) -> Result<Value, Refusal> {
    if depth >= DEPTH_LIMIT { return Err(Refusal::Exhausted) }
    match ty {
        Ty::I64 => Ok(json!(["I64"])),
        Ty::Bool => Ok(json!(["Bool"])),
        Ty::Unit => Ok(json!(["Unit"])),
        Ty::Text => Ok(json!(["Text"])),
        Ty::Pair(left, right) => Ok(json!(["Pair",type_descriptor(left, depth + 1)?,
            type_descriptor(right, depth + 1)?])),
        Ty::Sum(left, right) => Ok(json!(["Sum",type_descriptor(left, depth + 1)?,
            type_descriptor(right, depth + 1)?])),
        Ty::List(item) => Ok(json!(["List",type_descriptor(item, depth + 1)?])),
        Ty::Nominal(id, shape) => Ok(json!(["Nominal",id.module.to_string(),id.ordinal,
            shape_descriptor(shape, depth + 1)?])),
        _ => Err(Refusal::Unsupported),
    }
}

struct Budget {
    nodes: usize,
    memory: usize,
    work: usize,
}

impl Budget {
    fn preflight(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        if bytes.len() > BYTE_LIMIT { return Err(Refusal::Exhausted) }
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escape = false;
        let mut tokens = 0usize;
        for byte in bytes {
            if in_string {
                if escape { escape = false; }
                else if *byte == b'\\' { escape = true; }
                else if *byte == b'"' { in_string = false; }
                continue;
            }
            match byte {
                b'"' => { in_string = true; tokens += 1; }
                b'{' | b'[' => { depth += 1; tokens += 1; }
                b'}' | b']' => {
                    if depth == 0 { return Err(Refusal::Invalid) }
                    depth -= 1;
                }
                b',' => tokens += 1,
                _ => {}
            }
            if depth > DEPTH_LIMIT + 4 {
                return Err(Refusal::Exhausted);
            }
        }
        if depth != 0 || in_string { return Err(Refusal::Invalid) }
        let work = bytes.len().checked_mul(3)
            .and_then(|amount| amount.checked_add(tokens.checked_mul(8)?))
            .ok_or(Refusal::Exhausted)?;
        self.work = self.work.checked_sub(work).ok_or(Refusal::Exhausted)?;
        let charged = bytes.len().checked_mul(8)
            .and_then(|n| n.checked_add(tokens.checked_mul(256)?))
            .and_then(|n| self.memory.checked_add(n)).ok_or(Refusal::Exhausted)?;
        if charged > MEMORY_LIMIT { return Err(Refusal::Exhausted) }
        self.memory = charged;
        Ok(())
    }

    fn value(&mut self) -> Result<(), Refusal> {
        self.nodes = self.nodes.checked_add(1).ok_or(Refusal::Exhausted)?;
        if self.nodes > NODE_LIMIT { return Err(Refusal::Exhausted) }
        Ok(())
    }
}

fn fields<'a>(node: &'a Value, keys: &[&str], tag: &str) -> Result<&'a serde_json::Map<String, Value>, Refusal> {
    let object = node.as_object().ok_or(Refusal::Invalid)?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key))
        || object.get("tag").and_then(Value::as_str) != Some(tag) {
        return Err(Refusal::Invalid);
    }
    Ok(object)
}

fn schema_for<'a>(ty: &Ty, catalog: &'a [Schema]) -> Result<&'a Schema, Refusal> {
    let Ty::Nominal(id, shape) = ty else { return Err(Refusal::Unsupported) };
    let schema = catalog.iter().find(|entry| entry.id() == *id).ok_or(Refusal::Invalid)?;
    if &schema.ty != ty || !matches!(shape.as_ref(), NominalShape::Opaque(_) | NominalShape::Variant(_, _)) {
        return Err(Refusal::Invalid);
    }
    Ok(schema)
}

fn validate(value: &Value, ty: &Ty, catalog: &[Schema], budget: &mut Budget, depth: usize) -> Result<(), Refusal> {
    if depth >= DEPTH_LIMIT { return Err(Refusal::Exhausted) }
    budget.value()?;
    match ty {
        Ty::I64 => {
            let object = fields(value, &["tag", "value"], "i64")?;
            let text = object["value"].as_str().ok_or(Refusal::Invalid)?;
            let parsed = text.parse::<i64>().map_err(|_| Refusal::Invalid)?;
            if parsed.to_string() != text { return Err(Refusal::Invalid) }
        }
        Ty::Bool => { fields(value, &["tag", "value"], "bool")?["value"]
            .as_bool().ok_or(Refusal::Invalid)?; }
        Ty::Unit => {
            if !fields(value, &["tag", "value"], "unit")?["value"].is_null() {
                return Err(Refusal::Invalid)
            }
        }
        Ty::Text => { fields(value, &["tag", "value"], "text")?["value"]
            .as_str().ok_or(Refusal::Invalid)?; }
        Ty::Pair(left, right) => {
            let object = fields(value, &["tag", "left", "right"], "pair")?;
            validate(&object["left"], left, catalog, budget, depth + 1)?;
            validate(&object["right"], right, catalog, budget, depth + 1)?;
        }
        Ty::Sum(left, right) => {
            let object = fields(value, &["tag", "arm", "value"], "sum")?;
            let selected = match object["arm"].as_str() {
                Some("left") => left, Some("right") => right, _ => return Err(Refusal::Invalid),
            };
            validate(&object["value"], selected, catalog, budget, depth + 1)?;
        }
        Ty::List(item) => {
            let object = fields(value, &["tag", "items"], "list")?;
            let items = object["items"].as_array().ok_or(Refusal::Invalid)?;
            if items.len() > NODE_LIMIT { return Err(Refusal::Exhausted) }
            for value in items { validate(value, item, catalog, budget, depth + 1)?; }
        }
        Ty::Nominal(_, shape) => {
            let schema = schema_for(ty, catalog)?;
            match shape.as_ref() {
                NominalShape::Opaque(inner) => {
                    let object = fields(value, &["tag", "schema", "value"], "opaque")?;
                    if !schema.opaque_public || &object["schema"] != schema.wire() {
                        return Err(Refusal::Invalid)
                    }
                    validate(&object["value"], inner, catalog, budget, depth + 1)?;
                }
                NominalShape::Variant(left, right) => {
                    let object = fields(value, &["tag", "schema", "arm", "value"], "variant")?;
                    if &object["schema"] != schema.wire() { return Err(Refusal::Invalid) }
                    let selected = match object["arm"].as_str() {
                        Some("left") if schema.left_public => left,
                        Some("right") if schema.right_public => right,
                        _ => return Err(Refusal::Invalid),
                    };
                    validate(&object["value"], selected, catalog, budget, depth + 1)?;
                }
            }
        }
        _ => return Err(Refusal::Unsupported),
    }
    Ok(())
}

/// Two complete frames consume one budget. The caller must obtain `expected`
/// and `catalog` from an independently selected ModuleSession, never from either
/// frame or a package manifest. The returned value tree is only data, not an
/// accepted program or guest-visible authorization witness.
pub(crate) fn admit(frames: [&[u8]; 2], expected: [&Ty; 2], catalog: &[Schema],
                    initial_memory: usize, remaining_work: usize)
    -> Result<[Value; 2], Refusal> {
    if catalog.len() > 64 || expected.iter().any(|ty| !ty.is_data())
        || catalog.iter().enumerate().any(|(at, row)| catalog[..at].iter()
            .any(|previous| previous.id() == row.id())) {
        return Err(Refusal::Unsupported);
    }
    let mut budget = Budget { nodes: 0, memory: initial_memory, work: remaining_work };
    let mut results = Vec::with_capacity(2);
    for (bytes, ty) in frames.into_iter().zip(expected) {
        budget.preflight(bytes)?;
        let mut value: Value = serde_json::from_slice(bytes).map_err(|_| Refusal::Invalid)?;
        if serde_json::to_vec(&value).map_err(|_| Refusal::Invalid)? != bytes {
            return Err(Refusal::Invalid);
        }
        let frame = value.as_object().ok_or(Refusal::Invalid)?;
        if frame.len() != 3 || frame["format"] != "noble-worker-value/v1"
            || !frame.contains_key("expected_schema") || !frame.contains_key("value") {
            return Err(Refusal::Invalid);
        }
        if &frame["expected_schema"] != schema_for(ty, catalog)?.wire() {
            return Err(Refusal::Invalid);
        }
        validate(&frame["value"], ty, catalog, &mut budget, 0)?;
        results.push(value.as_object_mut().and_then(|frame| frame.remove("value"))
            .ok_or(Refusal::Invalid)?);
    }
    results.try_into().map_err(|_| Refusal::Invalid)
}

/// Both inert post-submit observations share one decoded-memory and node
/// budget, before handle-free values are exposed to the caller.
pub(crate) fn returned(observed: [&Value; 2], types: [&Ty; 2], catalog: &[Schema])
    -> Result<[Value; 2], Refusal> {
    let mut budget = Budget { nodes: 0, memory: 0, work: 1_000_000 };
    let first = decode(observed[0], types[0], catalog, &mut budget, 0)?;
    let second = decode(observed[1], types[1], catalog, &mut budget, 0)?;
    budget.nodes = 0;
    validate(&first, types[0], catalog, &mut budget, 0)?;
    validate(&second, types[1], catalog, &mut budget, 0)?;
    Ok([first, second])
}

fn decode(observed: &Value, ty: &Ty, catalog: &[Schema],
          budget: &mut Budget, depth: usize) -> Result<Value, Refusal> {
    if depth >= DEPTH_LIMIT { return Err(Refusal::Exhausted) }
    budget.value()?;
    budget.memory = budget.memory.checked_add(256).ok_or(Refusal::Exhausted)?;
    if budget.memory > MEMORY_LIMIT { return Err(Refusal::Exhausted) }
    let kind = observed.get("type").and_then(Value::as_str).ok_or(Refusal::Invalid)?;
    match (ty, kind) {
        (Ty::I64, "I64") => Ok(json!({"tag":"i64","value":observed["value"]})),
        (Ty::Bool, "Bool") => Ok(json!({"tag":"bool","value":observed["value"]})),
        (Ty::Unit, "Unit") => Ok(json!({"tag":"unit","value":observed["value"]})),
        (Ty::Text, "Text") => {
            let text = observed["value"].as_str().ok_or(Refusal::Invalid)?;
            let charge = text.len().checked_mul(12).ok_or(Refusal::Exhausted)?;
            budget.memory = budget.memory.checked_add(charge).ok_or(Refusal::Exhausted)?;
            if budget.memory > MEMORY_LIMIT { return Err(Refusal::Exhausted) }
            Ok(json!({"tag":"text","value":text}))
        }
        (Ty::Pair(left, right), "Pair") => {
            let fields = observed["value"].as_array().filter(|values| values.len() == 2)
                .ok_or(Refusal::Invalid)?;
            Ok(json!({"tag":"pair",
                "left":decode(&fields[0], left, catalog, budget, depth + 1)?,
                "right":decode(&fields[1], right, catalog, budget, depth + 1)?}))
        }
        (Ty::Sum(left, right), "Sum") => {
            let arm = observed["variant"].as_str().ok_or(Refusal::Invalid)?;
            let inner = match arm { "left" => left, "right" => right, _ => return Err(Refusal::Invalid) };
            Ok(json!({"tag":"sum","arm":arm,
                "value":decode(&observed["value"], inner, catalog, budget, depth + 1)?}))
        }
        (Ty::List(item), "List") => {
            let items = observed["value"].as_array().filter(|items| items.len() <= NODE_LIMIT)
                .ok_or(Refusal::Invalid)?;
            if items.len() > NODE_LIMIT - budget.nodes { return Err(Refusal::Exhausted) }
            let mut result = Vec::with_capacity(items.len());
            for entry in items { result.push(decode(entry, item, catalog, budget, depth + 1)?); }
            Ok(json!({"tag":"list","items":result}))
        }
        (Ty::Nominal(id, shape), "Nominal") => {
            let schema = schema_for(ty, catalog)?;
            if observed["module"] != id.module.to_string() || observed["ordinal"] != id.ordinal {
                return Err(Refusal::Invalid);
            }
            match shape.as_ref() {
                NominalShape::Opaque(inner) if observed["shape"] == "opaque"
                    && observed["variant"].is_null() =>
                    Ok(json!({"tag":"opaque","schema":schema.wire(),
                        "value":decode(&observed["value"], inner, catalog, budget, depth + 1)?})),
                NominalShape::Variant(left, right) if observed["shape"] == "variant" => {
                    let arm = observed["variant"].as_str().ok_or(Refusal::Invalid)?;
                    let inner = match arm {
                        "left" => left, "right" => right, _ => return Err(Refusal::Invalid),
                    };
                    Ok(json!({"tag":"variant","schema":schema.wire(),"arm":arm,
                        "value":decode(&observed["value"], inner, catalog, budget, depth + 1)?}))
                }
                _ => Err(Refusal::Invalid),
            }
        }
        _ => Err(Refusal::Invalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected(source: &[u8], type_name: &str) -> Result<(Schema, Ty), String> {
        let session = noble_contracts::source::ModuleSession::new(&[],
            noble_contracts::source::RetainedSourceLimit::new(crate::core::SOURCE_LIMITS.bytes)
                .expect("Worker source limit"))
            .map_err(|error| format!("{error:?}"))?;
        let checked = session.prepare(source, &[], crate::core::SOURCE_LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        let ty = checked.resolve_type(type_name).map_err(|error| format!("{error:?}"))?;
        let id = if let Ty::Nominal(id, _) = &ty { *id }
            else { return Err("selected type is not nominal".into()) };
        let declaration = checked.checked_nominals().ok_or("missing checked schema")?
            .into_iter().find(|row| row.declaration().id == id)
            .ok_or("selected declaration absent")?;
        let schema = Schema::checked(&declaration).map_err(|error| format!("{error:?}"))?;
        Ok((schema, ty))
    }

    fn frame(schema: &Schema, value: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({"format":"noble-worker-value/v1",
            "expected_schema":schema.wire(),"value":value})).expect("JSON frame")
    }

    #[test]
    fn selected_public_shape_admits_pair_text_and_refuses_schema_spoof() -> Result<(), String> {
        let (schema, ty) = selected(
            b"module WorkerData@1 [ opaque State Pair<I64,Text> public export State export State.new ]",
            "WorkerData@1.State")?;
        let value = json!({"tag":"opaque","schema":schema.wire(),
            "value":{"tag":"pair","left":{"tag":"i64","value":"7"},
                "right":{"tag":"text","value":"ping"}}});
        let bytes = frame(&schema, value.clone());
        let inputs = admit([&bytes,&bytes], [&ty,&ty], &[schema], 0, 1_000_000)
            .map_err(|error| format!("{error:?}"))?;
        if inputs != [value.clone(),value.clone()] { return Err("admitted values changed".into()) }

        let (schema, ty) = selected(
            b"module WorkerData@1 [ opaque State Pair<I64,Text> public export State export State.new ]",
            "WorkerData@1.State")?;
        let mut spoof = value;
        spoof["schema"]["version"] = json!(2);
        let wrong_version = frame(&schema, spoof);
        if !matches!(admit([&wrong_version,&bytes], [&ty,&ty], &[schema], 0, 1_000_000),
            Err(Refusal::Invalid)) { return Err("version spoof passed host ingress".into()) }
        Ok(())
    }

    #[test]
    fn private_constructor_resource_tag_and_shared_budget_refuse() -> Result<(), String> {
        let (private, private_ty) = selected(
            b"module WorkerData@1 [ opaque State Text private export State ]",
            "WorkerData@1.State")?;
        let hidden = frame(&private,json!({"tag":"opaque","schema":private.wire(),
            "value":{"tag":"text","value":"secret"}}));
        if !matches!(admit([&hidden,&hidden], [&private_ty,&private_ty], &[private],0,1_000_000),
            Err(Refusal::Invalid)) { return Err("private constructor admitted".into()) }
        let (public, ty) = selected(
            b"module WorkerData@1 [ opaque State Text public export State export State.new ]",
            "WorkerData@1.State")?;
        let smuggled = frame(&public,json!({"tag":"opaque","schema":public.wire(),
            "value":{"tag":"resource","kind":"private-handle"}}));
        if !matches!(admit([&smuggled,&smuggled], [&ty,&ty], &[public],0,1_000_000),
            Err(Refusal::Invalid)) { return Err("resource tag admitted".into()) }
        let (public, ty) = selected(
            b"module WorkerData@1 [ opaque State Text public export State export State.new ]",
            "WorkerData@1.State")?;
        let valid = frame(&public,json!({"tag":"opaque","schema":public.wire(),
            "value":{"tag":"text","value":"bounded"}}));
        if !matches!(admit([&valid,&valid], [&ty,&ty], &[public],MEMORY_LIMIT - 15_000,1_000_000),
            Err(Refusal::Exhausted)) { return Err("shared memory budget restored per frame".into()) }
        Ok(())
    }

    #[test]
    fn repeated_text_outputs_share_a_bounded_decoding_budget() {
        let state = json!({"type":"I64","value":"0"});
        let one = json!({"type":"Text","value":"x".repeat(16_384)});
        let repeated = json!({"type":"List","value":vec![one;8]});
        let actions = Ty::List(Box::new(Ty::Text));
        assert!(matches!(returned([&state,&repeated],[&Ty::I64,&actions],&[]),
            Err(Refusal::Exhausted)));
    }
}
