//! Binary checks for the bounded, compiler-emitted, source-classified saved-Program grammar.
//! The name section locates functions; every selected table entry and its
//! entire decoded function body must match the source plan. Runtime helpers
//! and installer control flow are trusted compiler emission, not an
//! arbitrary-binary authenticity proof derived from custom names alone.
use std::collections::{BTreeMap, BTreeSet};
use serde_json::Value;
use wasmparser::{ElementItems, ElementKind, KnownCustom, Name, Operator, Parser, Payload, Validator};

fn denied(reason: &str) -> String { format!("selected origin binary correspondence refused: {reason}") }
fn number(row: &Value, key: &str) -> Result<u32, String> {
    row.get(key).and_then(Value::as_u64).and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| denied(&format!("invalid {key}")))
}
fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row.get(key).and_then(Value::as_str).ok_or_else(|| denied(&format!("missing {key}")))
}
fn rows<'a>(row: &'a Value, key: &str) -> Result<&'a [Value], String> {
    row.get(key).and_then(Value::as_array).map(Vec::as_slice)
        .ok_or_else(|| denied(&format!("missing {key}")))
}
fn named(names: &BTreeMap<u32, String>, index: u32) -> Result<&str, String> {
    names.get(&index).map(String::as_str).ok_or_else(|| denied("unnamed contributing instruction"))
}

// A deliberately closed opcode alphabet. A new emitter instruction requires
// explicit review, rather than silently accepting it as an ignorable opcode.
fn decode(body: &wasmparser::FunctionBody<'_>, functions: &BTreeMap<u32, String>,
    globals: &BTreeMap<u32, String>) -> Result<Vec<String>, String> {
    if body.get_locals_reader().map_err(|_| denied("invalid selected locals"))?.get_count() != 0 {
        return Err(denied("selected instruction has unexpected local declarations"));
    }
    let mut ops = Vec::new();
    for op in body.get_operators_reader().map_err(|_| denied("invalid code body"))? {
        let token = match op.map_err(|_| denied("invalid decoded instruction"))? {
            Operator::GlobalGet { global_index } => format!("global.get {}", named(globals, global_index)?),
            Operator::LocalGet { local_index } => format!("local.get {local_index}"),
            Operator::I32Const { value } => format!("i32.const {value}"),
            Operator::I64Const { value } => format!("i64.const {value}"),
            Operator::Call { function_index } => format!("call {}", named(functions, function_index)?),
            Operator::If { blockty: wasmparser::BlockType::Empty } => "if".to_owned(),
            Operator::End => "end".to_owned(),
            Operator::Return => "return".to_owned(),
            Operator::I32Eqz => "i32.eqz".to_owned(),
            _ => return Err(denied("unexpected contributing function opcode")),
        };
        ops.push(token);
        if ops.len() > 64 { return Err(denied("contributing function exceeds instruction bound")); }
    }
    Ok(ops)
}

fn function_plan(operation: &Value, next: Option<u32>, selected: &Value) -> Result<Vec<String>, String> {
    let mut expected = vec!["global.get failure".to_owned(), "if".to_owned(),
        "return".to_owned(), "end".to_owned()];
    if let Some(next) = next {
        expected.extend([format!("i32.const {next}"), "local.get 0".to_owned(),
            "call enqueue".to_owned()]);
    }
    expected.extend(["global.get failure".to_owned(), "i32.eqz".to_owned(), "if".to_owned()]);
    let kind = text(operation, "kind")?;
    match kind {
        "i64" => {
            let value = text(operation, "value")?.parse::<i64>().map_err(|_| denied("invalid I64 source literal"))?;
            expected.extend([format!("i64.const {value}"), "call push_i64".to_owned()]);
        }
        "add" | "sub" | "mul" | "dup" | "swap" | "drop" | "compose" => {
            expected.push(format!("call op_{kind}"));
        }
        "quote-i64" => {
            expected.extend([format!("i32.const {}", number(operation, "quote_input_signature")?),
                format!("i32.const {}", number(operation, "quote_output_signature")?),
                format!("i32.const {}", number(operation, "quote_witness")?),
                "call op_quote_typed".to_owned()]);
        }
        "static-program" => {
            expected.extend([format!("global.get s{}", number(operation, "static_program_index")?),
                "call push_ref".to_owned()]);
        }
        "call-selected" => {
            let target = number(operation, "static_program_index")?;
            if target != number(selected, "named_program_index")? {
                return Err(denied("root calls a different selected program"));
            }
            expected.extend([format!("global.get s{target}"),
                "call enqueue_program".to_owned()]);
        }
        _ => return Err(denied("unsupported selected-origin operation")),
    }
    expected.extend(["end".to_owned(), "end".to_owned()]);
    Ok(expected)
}

pub(super) fn verify(bytes: &[u8], origin: &Value, install: &Value) -> Result<(), String> {
    Validator::new().validate_all(bytes).map_err(|_| denied("invalid WebAssembly module"))?;
    let selected = install.get("selected_target").ok_or_else(|| denied("no checked selected target"))?;
    for key in ["definition_index", "definition_identity", "source_generation",
        "selected_source_sha256", "named_program_index"] {
        if origin.get(key) != selected.get(key) { return Err(denied("selected definition identity differs")); }
    }
    let root_index = number(origin, "root_program_index")?;
    if root_index != number(&install["target_metadata"], "root_program_index")? {
        return Err(denied("root program index differs"));
    }
    let programs = rows(origin, "programs")?;
    let metadata = rows(install, "program_metadata")?;
    if programs.is_empty() || programs.len() > 128 || programs.len() != metadata.len() {
        return Err(denied("incomplete selected program map"));
    }
    let span = &install["code_span"];
    let span_start = number(span, "start")?;
    let span_end = span_start.checked_add(number(span, "length")?)
        .ok_or_else(|| denied("code span overflow"))?;
    let mut functions = BTreeMap::<u32, String>::new();
    let mut globals = BTreeMap::<u32, String>::new();
    let mut function_bodies = Vec::new();
    let mut imported_functions = 0u32;
    let mut function_types = Vec::new();
    let mut types = Vec::new();
    let mut code_elements = None;
    let mut core_elements = None;
    let mut code_initializers = Vec::new();
    let mut named_segments = BTreeMap::new();
    let mut table_imports = 0;
    let mut exported_installer = None;
    for section in Parser::new(0).parse_all(bytes) {
        match section.map_err(|_| denied("unparseable WebAssembly section"))? {
            Payload::TypeSection(section) => for ty in section.into_iter_err_on_gc_types() {
                types.push(ty.map_err(|_| denied("invalid selected function type"))?);
            },
            Payload::ImportSection(imports) => for import in imports {
                let import = import.map_err(|_| denied("invalid import"))?;
                match import.ty {
                    wasmparser::TypeRef::Func(index) | wasmparser::TypeRef::FuncExact(index) => {
                        function_types.push(index);
                        imported_functions += 1;
                    }
                    wasmparser::TypeRef::Table(_) => {
                        if import.module != "noble" || import.name != "table" || table_imports != 0 {
                            return Err(denied("unexpected dispatch table import"));
                        }
                        table_imports += 1;
                    }
                    _ => {}
                }
            },
            Payload::TableSection(_) => return Err(denied("selected module defines an alternate table")),
            Payload::StartSection { .. } => return Err(denied("selected module has an unchecked start function")),
            Payload::ExportSection(exports) => for export in exports {
                let export = export.map_err(|_| denied("invalid export"))?;
                if export.kind == wasmparser::ExternalKind::Table {
                    return Err(denied("selected module exports its dispatch table"));
                }
                if export.name == "install_target" {
                    if export.kind != wasmparser::ExternalKind::Func
                        || exported_installer.replace(export.index).is_some() {
                        return Err(denied("ambiguous target installer export"));
                    }
                }
            },
            Payload::FunctionSection(section) => for index in section {
                function_types.push(index.map_err(|_| denied("invalid function type index"))?);
            },
            Payload::CodeSectionEntry(body) => function_bodies.push(body),
            Payload::ElementSection(elements) => for (index, element) in elements.into_iter().enumerate() {
                let element = element.map_err(|_| denied("invalid element segment"))?;
                if !matches!(element.kind, ElementKind::Passive) {
                    return Err(denied("unchecked active or declared element segment"));
                }
                let ElementItems::Functions(items) = element.items else { continue; };
                let entries = items.into_iter().collect::<Result<Vec<_>, _>>()
                    .map_err(|_| denied("invalid element function list"))?;
                code_initializers.push((index as u32, entries));
            },
            Payload::CustomSection(section) => if let KnownCustom::Name(names) = section.as_known() {
                for name in names {
                    match name.map_err(|_| denied("invalid name section"))? {
                        Name::Function(entries) => for entry in entries {
                            let entry = entry.map_err(|_| denied("invalid function name"))?;
                            if functions.insert(entry.index, entry.name.to_owned()).is_some() {
                                return Err(denied("ambiguous function name"));
                            }
                        },
                        Name::Global(entries) => for entry in entries {
                            let entry = entry.map_err(|_| denied("invalid global name"))?;
                            if globals.insert(entry.index, entry.name.to_owned()).is_some() {
                                return Err(denied("ambiguous global name"));
                            }
                        },
                        Name::Element(entries) => for entry in entries {
                            let entry = entry.map_err(|_| denied("invalid element name"))?;
                            if named_segments.insert(entry.name.to_owned(), entry.index).is_some() {
                                return Err(denied("ambiguous element name"));
                            }
                        },
                        _ => {}
                    }
                }
            },
            _ => {}
        }
    }
    let unique_functions = functions.values().collect::<BTreeSet<_>>();
    if table_imports != 1 { return Err(denied("selected module lacks the host dispatch table")); }
    if unique_functions.len() != functions.len() || globals.values().collect::<BTreeSet<_>>().len() != globals.len() {
        return Err(denied("ambiguous function or global name"));
    }
    let code_segment = named_segments.get("code").copied()
        .ok_or_else(|| denied("missing code element segment"))?;
    let core_segment = named_segments.get("core").copied();
    // Prepared.code_span.generation names the *next* committed compiler
    // generation; emit::module received the preceding generation.
    if core_segment.is_some() != (number(span, "generation")? == 1) {
        return Err(denied("native core segment differs from compiler generation"));
    }
    for (segment, entries) in code_initializers {
        if segment == code_segment { code_elements = Some(entries); }
        else if Some(segment) == core_segment { core_elements = Some(entries); }
        else { return Err(denied("unrecognized passive element segment")); }
    }
    if core_segment.is_some() != core_elements.is_some() {
        return Err(denied("missing native core element segment"));
    }
    if let Some(core) = &core_elements {
        if core.iter().map(|index| named(&functions, *index)).collect::<Result<Vec<_>, _>>()?
            != ["empty_entry", "quote_entry", "compose_entry", "restore_entry"] {
            return Err(denied("native core element entries differ"));
        }
    }
    let elements = code_elements.ok_or_else(|| denied("missing passive code segment"))?;
    if elements.len() != usize::try_from(span_end - span_start).unwrap_or(usize::MAX) {
        return Err(denied("code element count differs from installed span"));
    }
    for (offset, function) in elements.iter().enumerate() {
        if named(&functions, *function)? != format!("f{}", span_start + offset as u32) {
            return Err(denied("code segment resolves to a different instruction function"));
        }
    }
    let initialize = functions.iter().find(|(_, name)| name.as_str() == "initialize")
        .map(|(index, _)| *index).ok_or_else(|| denied("missing initializer"))?;
    let installer = exported_installer.ok_or_else(|| denied("selected target installer not exported"))?;
    let install_body = function_bodies.get(installer.checked_sub(imported_functions)
        .ok_or_else(|| denied("target installer resolves to an import"))? as usize)
        .ok_or_else(|| denied("target installer lacks a code body"))?;
    let mut calls_initialize = 0;
    for operator in install_body.get_operators_reader().map_err(|_| denied("invalid installer"))? {
        if matches!(operator.map_err(|_| denied("invalid installer instruction"))?,
            Operator::Call { function_index } if function_index == initialize) {
            calls_initialize += 1;
        }
    }
    if calls_initialize != 1 { return Err(denied("exported installer does not call checked initializer exactly once")); }
    let mut verified_entries = BTreeSet::new();
    for (index, program) in programs.iter().enumerate() {
        if number(program, "program_index")? != index as u32
            || number(program, "entry")? != number(&metadata[index], "entry")? {
            return Err(denied("selected program entry/index differs from installed metadata"));
        }
        for key in ["input_signature", "output_signature", "effect_mask"] {
            if program.get(key) != metadata[index].get(key) { return Err(denied("selected program signature differs")); }
        }
        let operations = rows(program, "operations")?;
        if operations.is_empty() || operations.len() > 128 { return Err(denied("unbounded or empty selected program")); }
        let mut source_nodes = BTreeSet::new();
        for (ordinal, operation) in operations.iter().enumerate() {
            let entry = number(program, "entry")?.checked_add(ordinal as u32)
                .ok_or_else(|| denied("operation entry overflow"))?;
            if !source_nodes.insert(number(operation, "node")?)
                || number(operation, "ordinal")? != ordinal as u32
                || number(operation, "table_entry")? != entry
                || entry < span_start || entry >= span_end || !verified_entries.insert(entry) {
                return Err(denied("selected operation order/table entry differs"));
            }
            if text(program, "source_artifact")? == "selection" {
                if index as u32 != root_index || operations.len() != 1
                    || number(operation, "node")? != 0
                    || text(operation, "kind")? != "call-selected" || !operation["source_span"].is_null() {
                    return Err(denied("ambiguous selected caller wrapper"));
                }
            } else if text(program, "source_artifact")? == "definition" {
                let range = &operation["source_span"];
                if number(range, "start")? >= number(range, "end")? { return Err(denied("missing selected source span")); }
            } else { return Err(denied("unknown selected source artifact")); }
            let index_in_code = *elements.get((entry - span_start) as usize)
                .ok_or_else(|| denied("unresolved code table entry"))?;
            let ty = function_types.get(index_in_code as usize)
                .and_then(|index| types.get(*index as usize))
                .ok_or_else(|| denied("code entry lacks selected function type"))?;
            if ty.params() != [wasmparser::ValType::I32] || !ty.results().is_empty() {
                return Err(denied("selected code entry differs from checked entry signature"));
            }
            let body = function_bodies.get(index_in_code.checked_sub(imported_functions)
                .ok_or_else(|| denied("code table resolves to imported function"))? as usize)
                .ok_or_else(|| denied("code table resolves outside code section"))?;
            let next = (ordinal + 1 < operations.len()).then_some(entry + 1);
            if decode(body, &functions, &globals)? != function_plan(operation, next, origin)? {
                return Err(denied("complete decoded operation body differs from selected source action"));
            }
        }
    }
    if verified_entries.len() != elements.len() {
        return Err(denied("selected source omits a contributing code-table instruction"));
    }
    let init = function_bodies.get(initialize.checked_sub(imported_functions)
        .ok_or_else(|| denied("initializer resolves to imported function"))? as usize)
        .ok_or_else(|| denied("initializer lacks code body"))?;
    let mut previous = Vec::new();
    let mut found = 0;
    let mut found_core = 0;
    for operator in init.get_operators_reader().map_err(|_| denied("invalid initializer"))? {
        match operator.map_err(|_| denied("invalid initializer instruction"))? {
            Operator::I32Const { value } => { previous.push(value); if previous.len() > 3 { previous.remove(0); } },
            Operator::TableInit { elem_index, table } => {
                if elem_index == code_segment && table == 0 && previous == [span_start as i32, 0, elements.len() as i32] {
                    found += 1;
                } else if Some(elem_index) == core_segment && table == 0
                    && previous == [0, 0, 4] && found_core == 0 && found == 0 {
                    found_core += 1;
                } else { return Err(denied("alternate table initialization in initializer")); }
                previous.clear();
            }
            Operator::TableSet { .. } | Operator::TableCopy { .. } => return Err(denied("alternate table mutation in initializer")),
            _ => { previous.clear(); }
        }
    }
    if found != 1 || found_core != u32::from(core_segment.is_some()) {
        return Err(denied("code and native core segments not initialized at checked table spans"));
    }
    // The compiler's live source runtime has no other table writer. Reject a
    // binary that mutates dispatch after the checked initializer, even if all
    // selected function bodies and their element entries look correct.
    let mut table_initializers = 0;
    for body in &function_bodies {
        for operator in body.get_operators_reader().map_err(|_| denied("invalid code body"))? {
            match operator.map_err(|_| denied("invalid decoded code body"))? {
                Operator::TableSet { .. } | Operator::TableCopy { .. } | Operator::TableGrow { .. }
                | Operator::TableFill { .. } => return Err(denied("alternate code table writer")),
                Operator::TableInit { elem_index, table } => {
                    if (elem_index != code_segment && Some(elem_index) != core_segment)
                        || table != 0 || body.range() != init.range() {
                        return Err(denied("alternate table initialization outside checked initializer"));
                    }
                    table_initializers += 1;
                }
                _ => {}
            }
        }
    }
    if table_initializers != 1 + u32::from(core_segment.is_some()) {
        return Err(denied("ambiguous code table initialization"));
    }
    let outputs = rows(origin, "outputs")?;
    let checked_output = rows(&install["target_metadata"], "stack_out")?;
    let program_outputs = checked_output.iter().filter(|value| value.as_str().is_some_and(|s| s.starts_with("Program("))).count();
    if outputs.len() != program_outputs || outputs.is_empty() { return Err(denied("missing selected Program output lineage")); }
    let mut positions = BTreeSet::new();
    for row in outputs {
        let position = number(row, "stack_position")? as usize;
        if !checked_output.get(position).and_then(Value::as_str).is_some_and(|s| s.starts_with("Program("))
            || !positions.insert(position) { return Err(denied("ambiguous selected output position")); }
    }
    Ok(())
}

// Preserve the existing byte-for-byte local ID path for one returned
// `quote(I64) [ + ] compose` target. SelectedOriginMetadata is more general
// than this shape or the two-quotation alias extension below. A missing
// binding or lexical-owner witness never becomes an anonymous target ID.
fn anonymous_shape<'a>(
    origin: &'a Value,
    contracts: &[super::ProgramContract],
    position: usize,
    descriptor: &str,
    root_inputs: &[Value],
) -> Result<Option<(&'a Value, &'a Value, &'a Value)>, String> {
    let outputs = rows(origin, "outputs")?;
    if outputs.len() != 1 || root_inputs.len() > 1
        || descriptor != "Program([I64]->[I64]!{})" {
        return Ok(None);
    }
    let selected = &outputs[0];
    if number(selected, "stack_position")? as usize != position {
        return Ok(None);
    }
    let result = &selected["result"];
    if result["kind"] != "compose" || result["left"]["kind"] != "quote-i64"
        || result["right"]["kind"] != "static-program" {
        return Ok(None);
    }
    let right = number(&result["right"], "program_index")? as usize;
    let static_program = rows(origin, "programs")?.get(right)
        .ok_or_else(|| denied("anonymous target static dependency is missing"))?;
    let static_operations = rows(static_program, "operations")?;
    let static_contract = contracts.get(right)
        .ok_or_else(|| denied("anonymous target static contract is missing"))?;
    if static_operations.len() != 1 || static_operations[0]["kind"] != "add"
        || static_program["source_artifact"] != "definition"
        || static_contract.input != ["I64", "I64"]
        || static_contract.output != ["I64"] || !static_contract.effects.is_empty()
        || static_program["effect_mask"] != 0 || static_operations[0]["effect_mask"] != 0 {
        return Ok(None);
    }
    let operand = &result["left"]["operand"];
    if !(operand["kind"] == "fixed-i64" || operand["kind"] == "root-i64"
        && operand["position"] == 0 && root_inputs.len() == 1) {
        return Ok(None);
    }
    Ok(Some((result, static_program, operand)))
}

// The only additional identity-bearing shape is two separately checked
// quotations of root I64 bindings composed before a pure three-add body.
// Duplicating one root binding and using two independent equal-valued root
// bindings have different source alias classes, regardless of VM sharing.
fn alias_shape<'a>(
    origin: &'a Value,
    contracts: &[super::ProgramContract],
    position: usize,
    descriptor: &str,
    root_inputs: &[Value],
) -> Result<Option<(&'a Value, &'a Value, &'a Value, &'a Value, &'a Value)>, String> {
    if root_inputs.len() != 2 || descriptor != "Program([I64,I64]->[I64]!{})" {
        return Ok(None);
    }
    let outputs = rows(origin, "outputs")?;
    if outputs.len() != 1 || number(&outputs[0], "stack_position")? as usize != position {
        return Ok(None);
    }
    let outer = &outputs[0]["result"];
    let inner = &outer["left"];
    let left = &inner["left"];
    let right = &inner["right"];
    if outer["kind"] != "compose" || inner["kind"] != "compose-dynamic"
        || left["kind"] != "quote-i64" || right["kind"] != "quote-i64"
        || outer["right"]["kind"] != "static-program"
        || left["operand"]["kind"] != "root-i64"
        || right["operand"]["kind"] != "root-i64"
        || number(&left["operand"], "position")? >= 2
        || number(&right["operand"], "position")? >= 2 {
        return Ok(None);
    }
    let named_index = number(origin, "named_program_index")? as usize;
    let named = rows(origin, "programs")?.get(named_index)
        .ok_or_else(|| denied("selected alias source builder is missing"))?;
    if named["source_artifact"] != "definition" || named["effect_mask"] != 0
        || rows(named, "operations")?.iter().any(|op|
            !matches!(op["kind"].as_str(),
                Some("dup" | "drop" | "swap" | "quote-i64" | "compose" | "static-program"))
                || op["effect_mask"] != 0) {
        return Ok(None);
    }
    let static_index = number(&outer["right"], "program_index")? as usize;
    let static_program = rows(origin, "programs")?.get(static_index)
        .ok_or_else(|| denied("selected alias dependency is missing"))?;
    let contract = contracts.get(static_index)
        .ok_or_else(|| denied("selected alias dependency contract is missing"))?;
    let operations = rows(static_program, "operations")?;
    if static_program["source_artifact"] != "definition"
        || contract.input != ["I64", "I64", "I64", "I64"]
        || contract.output != ["I64"] || !contract.effects.is_empty()
        || static_program["effect_mask"] != 0 || operations.len() != 3
        || operations.iter().any(|op| op["kind"] != "add" || op["effect_mask"] != 0) {
        return Ok(None);
    }
    Ok(Some((outer, inner, left, right, static_program)))
}

pub(super) fn supports_anonymous_target(
    origin: &Value,
    contracts: &[super::ProgramContract],
    position: usize,
    descriptor: &str,
    root_inputs: &[Value],
) -> Result<bool, String> {
    Ok(anonymous_shape(origin, contracts, position, descriptor, root_inputs)?.is_some()
        || alias_shape(origin, contracts, position, descriptor, root_inputs)?.is_some())
}

pub(super) fn checked_saved_owner_inspection<'a>(
    owner: &str, saved_handle: u32, receipt: &'a Value,
) -> Result<&'a Value, String> {
    if receipt["outcome"] != "saved-program-inspected"
        || receipt["owner"] != owner || receipt["saved_handle"] != saved_handle
        || receipt["guest_requests"] != 0 || receipt["protected_operations"] != 0 {
        return Err(denied("retained owner or backend handle changed before graph inspection"));
    }
    receipt.get("selected_graph")
        .ok_or_else(|| denied("retained owner has no complete graph observation"))
}

pub(super) fn anonymous_target_identity(
    origin: &Value,
    contracts: &[super::ProgramContract],
    position: usize,
    descriptor: &str,
    saved: &Value,
    inspected_graph: &Value,
    root_inputs: &[Value],
) -> Result<Option<Value>, String> {
    use serde_json::json;
    let Some((result, static_program, operand)) =
        anonymous_shape(origin, contracts, position, descriptor, root_inputs)? else {
        return alias_target_identity(origin, contracts, position, descriptor,
            saved, inspected_graph, root_inputs);
    };
    let (varying, actual) = match operand["kind"].as_str() {
        Some("root-i64") if operand["position"] == 0 && root_inputs.len() == 1 => {
            let input = &root_inputs[0];
            if input["kind"] != "i64" {
                return Err(denied("anonymous root capture is not checked I64"));
            }
            (true, text(input, "value")?)
        }
        Some("fixed-i64") => (false, text(operand, "value")?),
        _ => return Ok(None),
    };
    let value = actual.parse::<i64>()
        .map_err(|_| denied("anonymous I64 operand is out of range"))?;
    let actual = value.to_string();
    if operand["kind"] == "fixed-i64" && text(operand, "value")? != actual {
        return Err(denied("checked fixed I64 operand lacks a canonical representation"));
    }

    // The trusted worker re-reads the complete immutable VM graph from the
    // retained saved owner after the root stack is cleared. Check its entire
    // topology, node payloads, packed interfaces and effects, and inert recipe
    // against the separately checked source/compiled-site plan. This is not
    // attestation against a malicious worker or arbitrary replacement helpers.
    let named = rows(origin, "programs")?.get(number(origin, "named_program_index")? as usize)
        .ok_or_else(|| denied("anonymous builder operation map is missing"))?;
    let operations = rows(named, "operations")?;
    let quote_op = operations.iter().find(|op|
        op["kind"] == "quote-i64" && op["node"] == result["left"]["node"]
            && op["table_entry"] == result["left"]["table_entry"])
        .ok_or_else(|| denied("anonymous quote lacks checked source site"))?;
    let compose_op = operations.iter().find(|op|
        op["kind"] == "compose" && op["node"] == result["node"]
            && op["table_entry"] == result["table_entry"])
        .ok_or_else(|| denied("anonymous compose lacks checked source site"))?;
    if quote_op["effect_mask"] != 0 || compose_op["effect_mask"] != 0 {
        return Err(denied("anonymous target is not pure"));
    }
    let add = &rows(static_program, "operations")?[0];
    let expected = json!([
        {"kind":4,"payload":"0","x":2,"y":quote_op["quote_input_signature"],
            "z":static_program["output_signature"],"w":2,"n":2,
            "a":1,"b":2,"c":3,"textBytes":null},
        {"kind":4,"payload":"0","x":1,"y":quote_op["quote_input_signature"],
            "z":quote_op["quote_output_signature"],"w":1,"n":1,
            "a":4,"b":null,"c":5,"textBytes":null},
        {"kind":4,"payload":"0","x":static_program["entry"],
            "y":static_program["input_signature"],"z":static_program["output_signature"],
            "w":1,"n":1,"a":null,"b":null,"c":6,"textBytes":null},
        {"kind":9,"payload":"0","x":0,"y":0,"z":0,"w":0,"n":0,
            "a":5,"b":6,"c":null,"textBytes":null},
        {"kind":1,"payload":actual,"x":0,"y":0,"z":0,"w":0,"n":0,
            "a":null,"b":null,"c":null,"textBytes":null},
        {"kind":8,"payload":actual,"x":1,"y":0,"z":0,"w":0,"n":0,
            "a":null,"b":null,"c":null,"textBytes":null},
        {"kind":8,"payload":"4","x":2,"y":add["input_signature"],
            "z":add["output_signature"],"w":0,"n":0,
            "a":null,"b":null,"c":null,"textBytes":null}
    ]);
    if inspected_graph != &expected {
        return Err(denied("retained anonymous owner graph differs from checked source and interface"));
    }
    // Existing public child observations must agree with the separately
    // re-read full owner graph; they never substitute for it.
    let captures = rows(saved, "capture_values")?;
    if captures.len() != 2 || captures[0]["field"] != "cell_a"
        || captures[1]["field"] != "cell_b" {
        return Err(denied("anonymous target capture graph is truncated"));
    }
    let left = captures[0]["value"].as_array()
        .ok_or_else(|| denied("anonymous quote graph is missing"))?;
    let right_graph = captures[1]["value"].as_array()
        .ok_or_else(|| denied("anonymous static child graph is missing"))?;
    let expected = expected.as_array()
        .ok_or_else(|| denied("internal anonymous graph projection is missing"))?;
    let rebased = |observed: &Value, original: &Value, a: Option<u32>, c: u32| {
        observed.as_object().zip(original.as_object()).is_some_and(|(node, checked)| {
            node.len() == checked.len() && checked.iter().all(|(field, value)|
                match field.as_str() {
                    "a" if a.is_some() => node[field] == a.unwrap_or_default(),
                    "c" => node[field] == c,
                    _ => node[field] == *value,
                })
        })
    };
    if left.len() != 3 || right_graph.len() != 2
        || !rebased(&left[0], &expected[1], Some(1), 2)
        || left[1] != expected[4] || left[2] != expected[5]
        || !rebased(&right_graph[0], &expected[2], None, 1)
        || right_graph[1] != expected[6] {
        return Err(denied("public capture graphs differ from retained owner graph"));
    }

    // This is an explicitly local, strict I64 quote/add/compose projection,
    // NOT the portable G-03 encoding. Domain separation prevents a template
    // digest from being mistaken for the captured program-value digest.
    // The checked source lineage decides the operand tag; the identical VM
    // literal alone cannot distinguish a fixed source constant from an input.
    let mut template = blake3::Hasher::new_derive_key(
        "noble experimental anonymous definition template v2");
    template.update(b"core-bootstrap:i64-quote-add-compose\0");
    template.update(b"I64--I64!{}\0builtin:i64.add:4\0");
    if varying {
        template.update(b"root-input:0:I64");
    } else {
        template.update(b"fixed:I64");
        template.update(&value.to_le_bytes());
    }
    let definition_digest = template.finalize();
    let definition_id = format!("anonymous-template-experimental-v2-blake3:{}",
        definition_digest.to_hex());
    let interface = json!({"input":["I64"],"output":["I64"],"effects":[]});
    let dependencies = json!(["builtin:i64.add"]);
    let captured = json!([{"type":"I64","value":actual}]);
    let mut program = blake3::Hasher::new_derive_key(
        "noble experimental anonymous program value v2");
    program.update(b"template-blake3:32\0capture:I64:1\0");
    program.update(definition_digest.as_bytes());
    program.update(&value.to_le_bytes());
    let program_value_id = format!("anonymous-program-experimental-v2-blake3:{}",
        program.finalize().to_hex());
    Ok(Some(json!({"definition_id":definition_id,
        "program_value_id":program_value_id,"captures":captured,
        "interface":interface,"effects":[],"dependencies":dependencies,
        "artifact_sha256":null})))
}

// Reify the worker's bounded, retained graph independently of its public
// capture summaries. Cell indices and allocation sharing authenticate the
// actual graph but never become source-binding identities.
struct ObservedGraph<'a> {
    cells: &'a [Value],
    seen: [bool; 32],
    seen_count: usize,
}

struct ProgramShape {
    input: u32,
    output: u32,
    depth: u32,
    leaves: u32,
    recipe: usize,
}

impl<'a> ObservedGraph<'a> {
    fn new(cells: &'a [Value]) -> Result<Self, String> {
        if cells.is_empty() || cells.len() > 32 {
            return Err(denied("selected alias graph exceeds bounded retained observation"));
        }
        Ok(Self { cells, seen: [false; 32], seen_count: 0 })
    }

    fn pointer(row: &Value, key: &str) -> Result<usize, String> {
        row.get(key).and_then(Value::as_u64).and_then(|index| usize::try_from(index).ok())
            .ok_or_else(|| denied("selected alias graph has a missing or invalid edge"))
    }

    fn absent(row: &Value, key: &str) -> bool {
        row.get(key).is_some_and(Value::is_null)
    }

    fn cell(&mut self, index: usize, kind: u64, payload: &str,
        x: u32, y: u32, z: u32, w: u32, n: u32) -> Result<&'a Value, String> {
        let row = self.cells.get(index)
            .ok_or_else(|| denied("selected alias graph has a truncated edge"))?;
        if !row.as_object().is_some_and(|object| object.len() == 11)
            || row["kind"].as_u64() != Some(kind)
            || row["payload"] != payload
            || row["x"].as_u64() != Some(u64::from(x))
            || row["y"].as_u64() != Some(u64::from(y))
            || row["z"].as_u64() != Some(u64::from(z))
            || row["w"].as_u64() != Some(u64::from(w))
            || row["n"].as_u64() != Some(u64::from(n))
            || !row.get("textBytes").is_some_and(Value::is_null)
            || ["a", "b", "c"].iter().any(|key| row.get(*key).is_none()) {
            return Err(denied("selected alias graph node differs from checked source shape"));
        }
        if !self.seen[index] {
            self.seen[index] = true;
            self.seen_count += 1;
        }
        Ok(row)
    }

    fn atom(&mut self, index: usize, x: u32, payload: &str,
        input: u32, output: u32, effects: u32) -> Result<(), String> {
        let row = self.cell(index, 8, payload, x, input, output, effects, 0)?;
        if !["a", "b", "c"].iter().all(|key| Self::absent(row, key)) {
            return Err(denied("selected alias recipe atom has an unchecked child"));
        }
        Ok(())
    }

    fn concat(&mut self, index: usize, left: usize, right: usize) -> Result<(), String> {
        let row = self.cell(index, 9, "0", 0, 0, 0, 0, 0)?;
        if Self::pointer(row, "a")? != left || Self::pointer(row, "b")? != right
            || !Self::absent(row, "c") {
            return Err(denied("selected alias recipe has an unchecked edge"));
        }
        Ok(())
    }

    fn quote(&mut self, index: usize, operation: &Value, value: &str) -> Result<ProgramShape, String> {
        let input = number(operation, "quote_input_signature")?;
        let output = number(operation, "quote_output_signature")?;
        let row = self.cell(index, 4, "0", 1, input, output, 1, 1)?;
        if !Self::absent(row, "b") {
            return Err(denied("selected alias quotation has an unchecked capture"));
        }
        let capture = Self::pointer(row, "a")?;
        let recipe = Self::pointer(row, "c")?;
        let scalar = self.cell(capture, 1, value, 0, 0, 0, 0, 0)?;
        if !["a", "b", "c"].iter().all(|key| Self::absent(scalar, key)) {
            return Err(denied("selected alias scalar has an unchecked child"));
        }
        self.atom(recipe, 1, value, 0, 0, 0)?;
        Ok(ProgramShape { input, output, depth: 1, leaves: 1, recipe })
    }

    fn static_recipe(&mut self, index: usize, operations: &[Value]) -> Result<(), String> {
        let (last, earlier) = operations.split_last()
            .ok_or_else(|| denied("selected alias dependency has no source operations"))?;
        if earlier.is_empty() {
            return self.atom(index, 2, "4", number(last, "input_signature")?,
                number(last, "output_signature")?, number(last, "effect_mask")?);
        }
        let row = self.cell(index, 9, "0", 0, 0, 0, 0, 0)?;
        if !Self::absent(row, "c") {
            return Err(denied("selected alias dependency recipe has an unchecked edge"));
        }
        let left = Self::pointer(row, "a")?;
        let right = Self::pointer(row, "b")?;
        self.static_recipe(left, earlier)?;
        self.atom(right, 2, "4", number(last, "input_signature")?,
            number(last, "output_signature")?, number(last, "effect_mask")?)
    }

    fn static_program(&mut self, index: usize, source: &Value) -> Result<ProgramShape, String> {
        let operations = rows(source, "operations")?;
        let input = number(source, "input_signature")?;
        let output = number(source, "output_signature")?;
        let length = u32::try_from(operations.len())
            .map_err(|_| denied("selected alias dependency is unbounded"))?;
        // The installer counts nested Program children for depth, not the
        // left-associated concatenation depth of this three-atom recipe.
        let row = self.cell(index, 4, "0", number(source, "entry")?,
            input, output, 1, length)?;
        if !Self::absent(row, "a") || !Self::absent(row, "b") {
            return Err(denied("selected alias dependency has an unchecked capture"));
        }
        let recipe = Self::pointer(row, "c")?;
        self.static_recipe(recipe, operations)?;
        Ok(ProgramShape { input, output, depth: 1, leaves: length, recipe })
    }

    fn compose(&mut self, index: usize, left: ProgramShape,
        right: ProgramShape) -> Result<ProgramShape, String> {
        if left.output != right.input {
            return Err(denied("selected alias children have incompatible interfaces"));
        }
        let depth = left.depth.max(right.depth).checked_add(1)
            .ok_or_else(|| denied("selected alias graph depth overflow"))?;
        let leaves = left.leaves.checked_add(right.leaves)
            .ok_or_else(|| denied("selected alias recipe leaf overflow"))?;
        let row = self.cell(index, 4, "0", 2, left.input, right.output, depth, leaves)?;
        let recipe = Self::pointer(row, "c")?;
        self.concat(recipe, left.recipe, right.recipe)?;
        Ok(ProgramShape { input: left.input, output: right.output,
            depth, leaves, recipe })
    }

    fn capture_matches(&self, root: usize, reported: &Value) -> bool {
        let Some(summary) = reported.as_array() else { return false; };
        let mut global = [0usize; 32];
        let mut mapped = [None; 32];
        if root >= self.cells.len() { return false; }
        global[0] = root;
        mapped[root] = Some(0);
        let mut local = 0;
        let mut pending = 1;
        while local < pending {
            let Some(actual) = self.cells.get(global[local]) else { return false; };
            let Some(claimed) = summary.get(local) else { return false; };
            if !claimed.as_object().is_some_and(|object| object.len() == 11)
                || ["kind", "payload", "x", "y", "z", "w", "n", "textBytes"]
                    .iter().any(|key| claimed.get(*key) != actual.get(*key)) {
                return false;
            }
            for key in ["a", "b", "c"] {
                let Some(actual_edge) = actual.get(key) else { return false; };
                let Some(claimed_edge) = claimed.get(key) else { return false; };
                if actual_edge.is_null() {
                    if !claimed_edge.is_null() { return false; }
                } else {
                    let Some(child) = actual_edge.as_u64()
                        .and_then(|value| usize::try_from(value).ok()) else { return false; };
                    let Some(slot) = mapped.get_mut(child) else { return false; };
                    let place = match slot {
                        Some(place) => *place,
                        None => {
                            if pending == global.len() { return false; }
                            let next = pending;
                            *slot = Some(next);
                            global[pending] = child;
                            pending += 1;
                            next
                        }
                    };
                    if claimed_edge.as_u64() != Some(place as u64) { return false; }
                }
            }
            local += 1;
        }
        pending == summary.len()
    }
}

fn selected_site<'a>(operations: &'a [Value], result: &Value, kind: &str) -> Result<&'a Value, String> {
    let mut matches = operations.iter().filter(|operation|
        operation["kind"] == kind && operation["node"] == result["node"]
            && operation["table_entry"] == result["table_entry"]);
    let selected = matches.next()
        .ok_or_else(|| denied("selected alias source site is missing"))?;
    if matches.next().is_some() || selected["effect_mask"] != 0 {
        return Err(denied("selected alias source site is ambiguous or effectful"));
    }
    Ok(selected)
}

fn alias_target_identity(
    origin: &Value,
    contracts: &[super::ProgramContract],
    position: usize,
    descriptor: &str,
    saved: &Value,
    inspected_graph: &Value,
    root_inputs: &[Value],
) -> Result<Option<Value>, String> {
    use serde_json::json;
    let Some((outer, inner, left, right, static_program)) =
        alias_shape(origin, contracts, position, descriptor, root_inputs)? else {
        return Ok(None);
    };
    if saved.get("lexical_owner").is_some()
        || saved["capture_status"] != "checked-selected-origin" {
        return Err(denied("selected alias has an unverified lexical owner or capture"));
    }
    let named_index = number(origin, "named_program_index")? as usize;
    let programs = rows(origin, "programs")?;
    let named = programs.get(named_index)
        .ok_or_else(|| denied("selected alias has no source builder"))?;
    let operations = rows(named, "operations")?;
    let outer_site = selected_site(operations, outer, "compose")?;
    let inner_site = selected_site(operations, inner, "compose")?;
    let left_site = selected_site(operations, left, "quote-i64")?;
    let right_site = selected_site(operations, right, "quote-i64")?;
    if outer_site["node"] == inner_site["node"]
        || left_site["node"] == right_site["node"]
        || left_site["table_entry"] == right_site["table_entry"] {
        return Err(denied("selected alias has ambiguous compiler source sites"));
    }
    let binding = |quote: &Value| -> Result<(usize, i64), String> {
        let source_position = number(&quote["operand"], "position")? as usize;
        let input = root_inputs.get(source_position)
            .ok_or_else(|| denied("selected alias root binding is missing"))?;
        if input["kind"] != "i64" {
            return Err(denied("selected alias root binding is not checked I64"));
        }
        let value = text(input, "value")?.parse::<i64>()
            .map_err(|_| denied("selected alias root I64 is out of range"))?;
        Ok((source_position, value))
    };
    let first = binding(left)?;
    let second_position = number(&right["operand"], "position")? as usize;
    let second = if second_position == first.0 { first } else { binding(right)? };
    let shared = first.0 == second.0;
    let first_literal = first.1.to_string();
    let second_literal = (!shared).then(|| second.1.to_string());
    let cells = inspected_graph.as_array()
        .ok_or_else(|| denied("retained alias owner has no complete graph"))?;
    let mut graph = ObservedGraph::new(cells)?;
    let root = cells.first().ok_or_else(|| denied("retained alias root is missing"))?;
    let inner_index = ObservedGraph::pointer(root, "a")?;
    let static_index = ObservedGraph::pointer(root, "b")?;
    let inner_cell = cells.get(inner_index)
        .ok_or_else(|| denied("retained alias compose is truncated"))?;
    let left_index = ObservedGraph::pointer(inner_cell, "a")?;
    let right_index = ObservedGraph::pointer(inner_cell, "b")?;
    let left_shape = graph.quote(left_index, left_site, &first_literal)?;
    let right_shape = graph.quote(right_index, right_site,
        second_literal.as_deref().unwrap_or(&first_literal))?;
    let inner_shape = graph.compose(inner_index, left_shape, right_shape)?;
    let static_shape = graph.static_program(static_index, static_program)?;
    graph.compose(0, inner_shape, static_shape)?;
    if graph.seen_count != cells.len() {
        return Err(denied("retained alias owner has extra unchecked graph cells"));
    }
    let captures = rows(saved, "capture_values")?;
    if captures.len() != 2 || captures[0]["field"] != "cell_a"
        || captures[1]["field"] != "cell_b"
        || !graph.capture_matches(inner_index, &captures[0]["value"])
        || !graph.capture_matches(static_index, &captures[1]["value"]) {
        return Err(denied("public alias captures differ from retained owner graph"));
    }

    // The checked source contains only root I64 binding uses, quote/compose
    // and a pure add body: no lexical recursive self-reference is admitted.
    // Physical code/module owners and source NodeIds never enter the identity.
    let mut template = blake3::Hasher::new_derive_key(
        "noble experimental anonymous definition template v2");
    template.update(b"core-bootstrap:two-quote-compose-add3\0");
    template.update(b"I64,I64--I64!{}\0builtin:i64.add:4\0");
    template.update(b"lexical-recursive-owner:none\0logical-source-alias:");
    template.update(&[0, u8::from(!shared)]);
    let definition_digest = template.finalize();
    let definition_id = format!("anonymous-template-experimental-v2-blake3:{}",
        definition_digest.to_hex());
    let mut program = blake3::Hasher::new_derive_key(
        "noble experimental anonymous program value v2");
    program.update(b"template-blake3:32\0capture:I64:");
    program.update(&[if shared { 1 } else { 2 }]);
    program.update(definition_digest.as_bytes());
    program.update(&first.1.to_le_bytes());
    let mut captured = vec![json!({"type":"I64","value":first_literal})];
    if let Some(second_literal) = second_literal {
        program.update(&second.1.to_le_bytes());
        captured.push(json!({"type":"I64","value":second_literal}));
    }
    Ok(Some(json!({"definition_id":definition_id,
        "program_value_id":format!("anonymous-program-experimental-v2-blake3:{}",
            program.finalize().to_hex()),
        "captures":captured,"interface":{"input":["I64","I64"],
            "output":["I64"],"effects":[]},"effects":[],
        "dependencies":["builtin:i64.add"],"artifact_sha256":null})))
}

// This is a digest of the complete *composite descriptor*, never a claim that
// either donor Wasm SHA alone is the returned Program's executable artifact.
// Both callers derive the same sorted-key JSON independently; the worker also
// checks actual live table function identity before admitting the candidate.
fn retained_recipe_digest(graph: &Value) -> Result<String, String> {
    let cells = graph.as_array().ok_or_else(|| denied("retained graph is not an array"))?;
    let root = cells.first().ok_or_else(|| denied("retained root is missing"))?;
    let recipe = ObservedGraph::pointer(root, "c")?;
    let mut global = [0usize; 32];
    let mut mapped = [None; 32];
    global[0] = recipe;
    *mapped.get_mut(recipe).ok_or_else(|| denied("retained recipe root is truncated"))? = Some(0);
    let mut pending = 1;
    let mut local = 0;
    let mut normalized = Vec::new();
    while local < pending {
        let mut cell = cells.get(global[local])
            .ok_or_else(|| denied("retained recipe edge is truncated"))?.clone();
        for field in ["a", "b", "c"] {
            if cell[field].is_null() { continue; }
            let child = cell[field].as_u64().and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| denied("retained recipe has an invalid edge"))?;
            let mapped_child = mapped.get_mut(child)
                .ok_or_else(|| denied("retained recipe has an out-of-bounds child"))?;
            let index = if let Some(index) = mapped_child { *index } else {
                if pending == global.len() { return Err(denied("retained recipe exceeds bound")); }
                let index = pending;
                *mapped_child = Some(index);
                global[pending] = child;
                pending += 1;
                index
            };
            cell[field] = serde_json::json!(index);
        }
        normalized.push(cell);
        local += 1;
    }
    let bytes = serde_json::to_vec(&normalized)
        .map_err(|_| denied("retained recipe is not serializable"))?;
    Ok(crate::workflow::intrinsic::sha256(&bytes))
}

pub(super) fn alias_code_donor_manifest(
    owner: &str, handle: u32, caller_id: &str, caller: &super::Candidate,
    core_id: &str, core: &super::Candidate, position: usize, descriptor: &str,
    identity: &Value, graph: &Value, inputs: &[Value],
) -> Result<Option<Value>, String> {
    use serde_json::json;
    let Some(origin) = caller.selected_origin.as_ref() else { return Ok(None); };
    let Some((outer, inner, left, right, static_program)) =
        alias_shape(&origin.checked_metadata, &caller.programs, position, descriptor, inputs)?
    else { return Ok(None); };
    if core.span.generation != 1 || core.selected_origin.is_none()
        || handle < 4096 || identity["artifact_sha256"] != Value::Null
        || identity["interface"] != json!({"input":["I64","I64"],
            "output":["I64"],"effects":[]}) {
        return Err(denied("selected alias has no independently checked native core or target"));
    }
    let named = &origin.checked_metadata["programs"][number(&origin.checked_metadata,
        "named_program_index")? as usize];
    let operations = rows(named, "operations")?;
    let quote_sites = [left, right].iter().map(|site|
        selected_site(operations, site, "quote-i64").and_then(|operation| {
            Ok(json!({"node":operation.get("node").ok_or_else(||
                    denied("selected quote site has no source node"))?,
                "table_entry":number(operation, "table_entry")?}))
        })).collect::<Result<Vec<_>, String>>()?;
    let compose_sites = [inner, outer].iter().map(|site|
        selected_site(operations, site, "compose").and_then(|operation| {
            Ok(json!({"node":operation.get("node").ok_or_else(||
                    denied("selected compose site has no source node"))?,
                "table_entry":number(operation, "table_entry")?}))
        })).collect::<Result<Vec<_>, String>>()?;
    let static_result = &outer["right"];
    let static_index = number(static_result, "program_index")? as usize;
    if number(static_program, "program_index")? as usize != static_index {
        return Err(denied("selected static dependency differs from source result"));
    }
    let mut static_sites = operations.iter().filter(|operation|
        operation["kind"] == "static-program"
            && operation["node"] == static_result["node"]
            && operation["static_program_index"] == static_index
            && operation["effect_mask"] == 0);
    let static_site = static_sites.next()
        .ok_or_else(|| denied("selected static add child lacks a checked source site"))?;
    if static_sites.next().is_some() {
        return Err(denied("selected static add child has ambiguous source sites"));
    }
    let static_site = json!({"node":static_site["node"],
        "table_entry":number(static_site, "table_entry")?});
    let static_source = &origin.checked_metadata["programs"][static_index];
    let start = number(static_source, "entry")?;
    let end = caller.span.start.checked_add(caller.span.length)
        .ok_or_else(|| denied("selected static donor code span overflows"))?;
    let add_entries = rows(static_source, "operations")?
        .iter().enumerate().map(|(ordinal, operation)| {
            let entry = number(operation, "table_entry")?;
            if operation["kind"] != "add" || operation["effect_mask"] != 0
                || entry != start + ordinal as u32 || entry < caller.span.start || entry >= end {
                return Err(denied("selected static add entry differs from checked code donor"));
            }
            Ok(entry)
        }).collect::<Result<Vec<_>, String>>()?;
    if add_entries.len() != 3 || static_index >= caller.programs.len() {
        return Err(denied("selected static add lacks exact three-entry dependency"));
    }
    let graph_bytes = serde_json::to_vec(graph)
        .map_err(|_| denied("selected retained graph is not canonically serializable"))?;
    let manifest = json!({
        "schema":"noble-live-slot-composite-code-donors/v1",
        "owner":{"token":owner,"handle":handle},
        "identity":{"definition_id":identity["definition_id"],
            "program_value_id":identity["program_value_id"],
            "captures":identity["captures"],"interface":identity["interface"],
            "lexical_recursive_owner":null},
        "source":{"caller_id":caller_id,"source_sha256":caller.source_sha256,
            "selected_source_sha256":origin.checked_metadata["selected_source_sha256"],
            "definition_identity":origin.definition_identity,
            "source_generation":origin.source_generation,
            "quote_sites":quote_sites,"compose_sites":compose_sites,
            "static_program_index":static_index,"static_site":static_site},
        "donors":{
            "native":{"module_id":core_id,"artifact_sha256":core.artifact_sha256,
                "source_sha256":core.source_sha256,
                "span":{"start":core.span.start,"length":core.span.length,
                    "generation":core.span.generation},
                "entries":[1,2]},
            "static":{"module_id":caller_id,"artifact_sha256":caller.artifact_sha256,
                "source_sha256":caller.source_sha256,
                "span":{"start":caller.span.start,"length":caller.span.length,
                    "generation":caller.span.generation},
                "entries":add_entries}
        },
        "graph_sha256":crate::workflow::intrinsic::sha256(&graph_bytes),
        "recipe_sha256":retained_recipe_digest(graph)?
    });
    let mut bytes = b"noble live-slot composite code donors v1\0".to_vec();
    bytes.extend_from_slice(&serde_json::to_vec(&manifest)
        .map_err(|_| denied("selected composite descriptor is not serializable"))?);
    Ok(Some(json!({"descriptor":manifest,
        "manifest_sha256":crate::workflow::intrinsic::sha256(&bytes)})))
}

// The Lean key is deliberately neither the builder Wasm SHA nor D/P nor the
// composite descriptor's SHA. Host-only bytes are retained from the exact
// selected binaries sent at installation; the worker never supplies this key.
// The proof is of the pure returned code below. The separate selected source,
// binary-body, and installed-table checks justify applying it to this owner.
pub(super) fn alias_exact_target_subject(
    manifest: &Value, caller: &super::Candidate, core: &super::Candidate,
    native_bodies: &Value,
) -> Result<Value, String> {
    let descriptor = &manifest["descriptor"];
    if descriptor["schema"] != "noble-live-slot-composite-code-donors/v1"
        || descriptor["donors"]["native"]["artifact_sha256"] != core.artifact_sha256
        || descriptor["donors"]["static"]["artifact_sha256"] != caller.artifact_sha256
        || descriptor["identity"]["lexical_recursive_owner"] != Value::Null
        || descriptor["identity"]["interface"] != serde_json::json!({
            "input":["I64","I64"],"output":["I64"],"effects":[]})
    {
        return Err(denied("exact target theorem subject differs from checked composite descriptor"));
    }
    let native_bytes = core.proof_bytes.as_ref()
        .ok_or_else(|| denied("selected native code donor has no retained original binary"))?;
    let static_bytes = caller.proof_bytes.as_ref()
        .ok_or_else(|| denied("selected static code donor has no retained original binary"))?;
    if native_bodies["native"]["module_sha256"] != core.artifact_sha256
        || native_bodies["static"]["module_sha256"] != caller.artifact_sha256 {
        return Err(denied("selected installed donor bytes differ from native body witness"));
    }
    let captures = rows(&descriptor["identity"], "captures")?;
    if !(captures.len() == 1 || captures.len() == 2) {
        return Err(denied("exact target has an unsupported capture alias class"));
    }
    let physical = captures.iter().map(|capture| {
        if capture["type"] != "I64" {
            return Err(denied("exact target captured value has a different type"));
        }
        text(capture, "value")?.parse::<i64>()
            .map_err(|_| denied("exact target captured I64 is out of range"))
    }).collect::<Result<Vec<_>, String>>()?;
    let literals = [physical[0], *physical.get(1).unwrap_or(&physical[0])];
    let literal_values = [literals[0].to_string(), literals[1].to_string()];
    // This exact claim names the two-quote/three-add target, never the builder.
    // Do not assert that Lean proves Wasm correctness: the pure arithmetic
    // fragment has no installed-runtime premise, and its byte witness alone
    // cannot establish the separate native operator semantic refinement.
    // In particular, a checked native core *entry name* does not prove the
    // quote/compose Wasm body implements the pure model.
    let subject = serde_json::json!({
        "schema":"noble-live-slot-returned-target-lean/v1",
        "claim":"pure-i64-i64-to-i64-two-literal-three-wrapping-add/v1",
        "assumptions":[],
        "manifest_sha256":manifest["manifest_sha256"],
        "manifest":descriptor,
        "installed_donor_bytes":{
            "native":{"sha256":core.artifact_sha256,"length":native_bytes.len()},
            "static":{"sha256":caller.artifact_sha256,"length":static_bytes.len()}},
        "native_body_witness":native_bodies,
        "literal_values":[literal_values[0].as_str(),literal_values[1].as_str()],
        "target_code":[
            {"op":"lit-i64","value":literal_values[0].as_str()},
            {"op":"lit-i64","value":literal_values[1].as_str()},
            {"op":"word","index":4},
            {"op":"word","index":4},
            {"op":"word","index":4}
        ]
    });
    Ok(subject)
}

pub(super) fn key_exact_target_subject(subject: &Value) -> Result<Value, String> {
    let mut keyed = b"noble live-slot exact returned target lean theorem v1\0".to_vec();
    keyed.extend_from_slice(&serde_json::to_vec(subject)
        .map_err(|_| denied("exact target theorem subject is not serializable"))?);
    Ok(serde_json::json!({
        "theorem_key":crate::workflow::intrinsic::sha256(&keyed),
        "subject":subject
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use noble_kernel::types::Ty;
    const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
        bytes: 65_536, nodes: 16_384, depth: 64, work: 2_000_000,
    };

    #[test]
    fn selected_binary_rejects_wrong_operation_body_and_table_binding() -> Result<(), String> {
        let environment = noble_kernel::contracts::environment()
            .map_err(|error| format!("environment: {error:?}"))?.enable_live_slots();
        let mut source = noble_contracts::source::Session::new_live_slots(environment)
            .map_err(|error| error.diagnostic().message.clone())?;
        let definition = source.prepare(b"def builder [ quote [ + ] compose ]", &[], LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        source.commit(definition).map_err(|error| error.diagnostic().message.clone())?;
        let call = source.prepare(b"builder", &[Ty::I64], LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        let selected = source.checked_selected_target(&call, "builder")
            .map_err(|error| format!("selected source: {error:?}"))?;
        let prepared = noble_wasm::source::Compiler::new_live_slots()
            .prepare_checked_selected(&selected)
            .map_err(|_| "checked source compilation refused".to_owned())?;
        let bound = prepared.selected_target().ok_or("missing checked selection")?;
        let target = serde_json::json!({
            "definition_index":bound.definition_id.index(),
            "definition_identity":bound.definition_id.identity().to_string(),
            "source_generation":bound.source_generation.to_string(),
            "named_program_index":bound.named_program_index,
            "selected_source_sha256":crate::workflow::intrinsic::sha256(selected.source())
        });
        let origin = super::super::selected_origin_json(
            prepared.selected_origin().ok_or("missing classified selected source")?, &target)?;
        let metadata = prepared.program_metadata().iter().map(|program| serde_json::json!({
            "entry":program.entry, "program_index":program.program_index,
            "input_signature":program.input_signature,
            "output_signature":program.output_signature, "effect_mask":program.effect_mask,
        })).collect::<Vec<_>>();
        let span = prepared.code_span();
        let install = serde_json::json!({
            "selected_target":target,
            "target_metadata":{"root_program_index":prepared.target_metadata()
                .ok_or("missing root metadata")?.root_program_index,
                "stack_out":["Program([I64]->[I64]!{})"]},
            "code_span":{"start":span.start,"length":span.length,"generation":span.generation},
            "program_metadata":metadata
        });
        let binary = super::super::assemble_slot(prepared.wat())?;
        verify(&binary, &origin, &install)?;

        let mut wrong_site = origin.clone();
        wrong_site["programs"][0]["operations"][0]["table_entry"] = serde_json::json!(span.start + 1);
        if verify(&binary, &wrong_site, &install).is_ok() {
            return Err("wrong selected quote site acquired origin".into());
        }
        let mut wrong_action = origin.clone();
        wrong_action["programs"][0]["operations"][2]["kind"] = serde_json::json!("drop");
        if verify(&binary, &wrong_action, &install).is_ok() {
            return Err("wrong selected compose operation acquired origin".into());
        }
        let mut extra = origin.clone();
        let duplicate = extra["programs"][0]["operations"][0].clone();
        extra["programs"][0]["operations"].as_array_mut()
            .ok_or("origin operations missing")?.push(duplicate);
        if verify(&binary, &extra, &install).is_ok() {
            return Err("extra ambiguous selected instruction acquired origin".into());
        }

        let wat = std::str::from_utf8(prepared.wat()).map_err(|error| error.to_string())?;
        let changed_code = wat.replacen("(call $op_compose)", "(call $op_drop)", 1);
        if changed_code == wat || verify(&super::super::assemble_slot(changed_code.as_bytes())?,
            &origin, &install).is_ok() {
            return Err("changed decoded operation body acquired checked origin".into());
        }
        let before = format!("(table.init $code (i32.const {}) ", span.start);
        let after = format!("(table.init $code (i32.const {}) ", span.start + 1);
        let changed_table = wat.replacen(&before, &after, 1);
        if changed_table == wat {
            return Err("test could not relocate code table initializer".into());
        }
        if verify(&super::super::assemble_slot(changed_table.as_bytes())?,
            &origin, &install).is_ok() {
            return Err("changed passive-element table destination acquired checked origin".into());
        }
        let init_code = format!("(table.init $code (i32.const {}) (i32.const 0) (i32.const {}))",
            span.start, span.length);
        if !wat.contains(&init_code) { return Err("test could not locate checked table initializer".into()); }
        let extra_init = format!("{init_code}\n(table.init $evil (i32.const {}) (i32.const 0) (i32.const 1))",
            span.start);
        let forged_initializer = wat.replacen(&init_code, &extra_init, 1);
        let evil_segment = "(elem $evil func $restore_entry)\n";
        let closing = forged_initializer.rfind(')').ok_or("module closing parenthesis missing")?;
        let forged_initializer = format!("{}{}{}",
            &forged_initializer[..closing], evil_segment, &forged_initializer[closing..]);
        if verify(&super::super::assemble_slot(forged_initializer.as_bytes())?,
            &origin, &install).is_ok() {
            return Err("second passive element overwrote checked table entries".into());
        }
        let closing = wat.rfind(')').ok_or("module closing parenthesis missing")?;
        let forged_helper = format!("{}{}(func $overwrite_checked_table {} )\n{}",
            &wat[..closing], evil_segment,
            format_args!("(table.init $evil (i32.const {}) (i32.const 0) (i32.const 1))", span.start),
            &wat[closing..]);
        if verify(&super::super::assemble_slot(forged_helper.as_bytes())?,
            &origin, &install).is_ok() {
            return Err("other function's passive element initializer acquired checked origin".into());
        }
        // The first prepared code span is labeled generation 1 even though
        // emit::module used generation 0. A later compiler emission must not
        // accept a newly forged native-core segment and its table writer.
        let mut compiler = noble_wasm::source::Compiler::new_live_slots();
        let first = compiler.prepare_checked_selected(&selected)
            .map_err(|_| "first checked compiler emission refused".to_owned())?;
        compiler.commit(first).map_err(|_| "first checked emission did not commit".to_owned())?;
        let later = compiler.prepare_checked_selected(&selected)
            .map_err(|_| "later checked compiler emission refused".to_owned())?;
        let later_span = later.code_span();
        let later_origin = super::super::selected_origin_json(
            later.selected_origin().ok_or("later selected source lacks classified origin")?, &target)?;
        let mut later_install = install.clone();
        later_install["code_span"] = serde_json::json!({
            "start":later_span.start,"length":later_span.length,"generation":later_span.generation
        });
        later_install["program_metadata"] = serde_json::json!(later.program_metadata().iter().map(|program|
            serde_json::json!({
                "entry":program.entry,"program_index":program.program_index,
                "input_signature":program.input_signature,
                "output_signature":program.output_signature,"effect_mask":program.effect_mask
            })).collect::<Vec<_>>());
        let later_wat = std::str::from_utf8(later.wat()).map_err(|error| error.to_string())?;
        verify(&super::super::assemble_slot(later_wat.as_bytes())?, &later_origin, &later_install)?;
        let late_init = format!("(table.init $core (i32.const 0) (i32.const 0) (i32.const 4))\n\
            (table.init $code (i32.const {})", later_span.start);
        let real_init = format!("(table.init $code (i32.const {})", later_span.start);
        let forged = later_wat.replacen(&real_init, &late_init, 1);
        if forged == later_wat { return Err("later initializer lacks checked code entry".into()); }
        let closing = forged.rfind(')').ok_or("later module closing parenthesis missing")?;
        let forged = format!("{}(elem $core func $empty_entry $quote_entry $compose_entry $restore_entry)\n{}",
            &forged[..closing], &forged[closing..]);
        if verify(&super::super::assemble_slot(forged.as_bytes())?,
            &later_origin, &later_install).is_ok() {
            return Err("later-generation forged native core segment acquired selected origin".into());
        }
        Ok(())
    }

    #[test]
    fn alias_binary_rejects_second_quote_and_nested_compose_site_substitution() -> Result<(), String> {
        use serde_json::json;
        let environment = noble_kernel::contracts::environment()
            .map_err(|error| format!("environment: {error:?}"))?.enable_live_slots();
        let mut source = noble_contracts::source::Session::new_live_slots(environment)
            .map_err(|error| error.diagnostic().message.clone())?;
        let definition = source.prepare(
            b"def aliased [ drop dup quote swap quote compose [ + + + ] compose ]",
            &[], LIMITS).map_err(|error| error.diagnostic().message.clone())?;
        source.commit(definition).map_err(|error| error.diagnostic().message.clone())?;
        let call = source.prepare(b"aliased", &[Ty::I64, Ty::I64], LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        let selected = source.checked_selected_target(&call, "aliased")
            .map_err(|error| format!("selected source: {error:?}"))?;
        let prepared = noble_wasm::source::Compiler::new_live_slots()
            .prepare_checked_selected(&selected)
            .map_err(|_| "checked alias compilation refused".to_owned())?;
        let bound = prepared.selected_target().ok_or("missing selected alias")?;
        let target = json!({"definition_index":bound.definition_id.index(),
            "definition_identity":bound.definition_id.identity().to_string(),
            "source_generation":bound.source_generation.to_string(),
            "named_program_index":bound.named_program_index,
            "selected_source_sha256":crate::workflow::intrinsic::sha256(selected.source())});
        let origin = super::super::selected_origin_json(
            prepared.selected_origin().ok_or("missing classified alias source")?, &target)?;
        let span = prepared.code_span();
        let install = json!({"selected_target":target,
            "target_metadata":{"root_program_index":prepared.target_metadata()
                .ok_or("missing alias root metadata")?.root_program_index,
                "stack_out":["Program([I64,I64]->[I64]!{})"]},
            "code_span":{"start":span.start,"length":span.length,"generation":span.generation},
            "program_metadata":prepared.program_metadata().iter().map(|program| json!({
                "entry":program.entry,"program_index":program.program_index,
                "input_signature":program.input_signature,
                "output_signature":program.output_signature,"effect_mask":program.effect_mask
            })).collect::<Vec<_>>()});
        let binary = super::super::assemble_slot(prepared.wat())?;
        verify(&binary, &origin, &install)?;
        let contracts = prepared.program_metadata().iter().map(|program|
            Ok(super::super::ProgramContract {
                input: super::super::canonical_stack(&program.stack_in, 0)?,
                output: super::super::canonical_stack(&program.stack_out, 0)?,
                effects: super::super::effects(&program.effects)?,
            })).collect::<Result<Vec<_>, String>>()?;
        let root_inputs = [json!({"kind":"i64","value":"2"}), json!({"kind":"i64","value":"2"})];
        if !supports_anonymous_target(&origin, &contracts, 0,
            "Program([I64,I64]->[I64]!{})", &root_inputs)? {
            return Err("two checked root-binding quotations remained opaque".into());
        }
        let named = number(&origin, "named_program_index")? as usize;
        let source_ops = rows(&origin["programs"][named], "operations")?;
        let quotes = source_ops.iter().enumerate().filter(|(_, op)| op["kind"] == "quote-i64")
            .map(|(at, _)| at).collect::<Vec<_>>();
        let composes = source_ops.iter().enumerate().filter(|(_, op)| op["kind"] == "compose")
            .map(|(at, _)| at).collect::<Vec<_>>();
        if quotes.len() != 2 || composes.len() != 2 {
            return Err("alias source lost distinct quote and compose sites".into());
        }
        let mut owner_claim = origin.clone();
        owner_claim["programs"][named]["operations"][0]["kind"] = json!("recursive-self");
        if supports_anonymous_target(&owner_claim, &contracts, 0,
            "Program([I64,I64]->[I64]!{})", &root_inputs)? {
            return Err("unverified lexical self-reference acquired anonymous identity".into());
        }
        let mut substituted_quote = origin.clone();
        substituted_quote["programs"][named]["operations"][quotes[1]]["table_entry"] =
            origin["programs"][named]["operations"][quotes[0]]["table_entry"].clone();
        if verify(&binary, &substituted_quote, &install).is_ok() {
            return Err("second alias quotation borrowed the first checked code site".into());
        }
        let mut substituted_compose = origin.clone();
        substituted_compose["programs"][named]["operations"][composes[0]]["kind"] = json!("drop");
        if verify(&binary, &substituted_compose, &install).is_ok() {
            return Err("inner alias composition borrowed a different compiled action".into());
        }
        let wat = std::str::from_utf8(prepared.wat())
            .map_err(|error| error.to_string())?;
        let binary_sha = crate::workflow::intrinsic::sha256(&binary);
        let static_add = rows(&origin, "programs")?.iter().find(|program|
            program["operations"].as_array().is_some_and(|operations|
                operations.len() == 3 && operations.iter().all(|operation|
                    operation["kind"] == "add")))
            .ok_or("selected alias has no three-add source program")?;
        let add_start = number(static_add, "entry")?;
        let add_entries = [add_start, add_start + 1, add_start + 2];
        let checked_native = super::super::native::attest(
            &binary, &binary_sha, &binary, &binary_sha, span.start, &add_entries)?;
        for (old, replacement, label) in [
            ("(i64.add (local.get $left) (local.get $right))",
                "(i64.sub (local.get $left) (local.get $right))", "add"),
            ("(else (call $push_i64 (call $payload (local.get $environment))))",
                "(else (call $push_i64 (call $measure_data (local.get $environment))))", "quote"),
            ("(call $enqueue_program (call $b (local.get $environment)))\n (call $enqueue_program (call $a (local.get $environment)))",
                "(call $enqueue_program (call $a (local.get $environment)))\n (call $enqueue_program (call $b (local.get $environment)))", "compose"),
            ("(call $op_add)", "(call $op_sub)", "selected-add-table-wrapper"),
            ("(call_indirect (type $entry) (local.get $environment) (local.get $index))",
                "(call_indirect (type $entry) (local.get $environment) (i32.const 1))",
                "dispatch-table-body"),
            ("(import \"noble\" \"live_select\" (func $host_live_select (param i32 i32) (result i32)))",
                "(import \"noble\" \"live_bind_validate\" (func $host_live_select (param i32 i32) (result i32)))",
                "host-import"),
        ] {
            let changed = wat.replacen(old, replacement, 1);
            if changed == wat { return Err(format!("{label} native mutation did not match selected Wasm source")); }
            let binary = super::super::assemble_slot(changed.as_bytes())?;
            let sha = crate::workflow::intrinsic::sha256(&binary);
            if super::super::native::attest(
                &binary, &sha, &binary, &sha, span.start, &add_entries).is_ok() {
                return Err(format!("rewritten installed native {label} body acquired exact proof witness"));
            }
        }
        let old_unbox = "(if (i32.lt_u (local.get $tag) (i32.const 4))";
        let changed_unbox = wat.replacen(old_unbox,
            "(if (i32.lt_u (local.get $tag) (i32.const 2))", 1);
        if changed_unbox == wat { return Err("native unbox helper mutation did not match".into()); }
        let changed_binary = super::super::assemble_slot(changed_unbox.as_bytes())?;
        let changed_sha = crate::workflow::intrinsic::sha256(&changed_binary);
        let changed_native = super::super::native::attest(
            &changed_binary, &changed_sha, &changed_binary, &changed_sha,
            span.start, &add_entries)?;
        let unbox_sha = |witness: &Value| witness["native"]["functions"].as_array()
            .and_then(|rows| rows.iter().find(|row| row["name"] == "unbox"))
            .and_then(|row| row["body_sha256"].as_str())
            .map(str::to_owned);
        if unbox_sha(&checked_native).is_none()
            || unbox_sha(&checked_native) == unbox_sha(&changed_native) {
            return Err("changed native unbox helper did not change exact reachable body witness".into());
        }
        let native = "(elem $core func $empty_entry $quote_entry $compose_entry $restore_entry)";
        if !wat.contains(native) { return Err("selected native donor has no core entry list".into()); }
        let swapped = wat.replacen(native,
            "(elem $core func $empty_entry $compose_entry $quote_entry $restore_entry)", 1);
        if verify(&super::super::assemble_slot(swapped.as_bytes())?, &origin, &install).is_ok() {
            return Err("rewritten native quote/compose table entry acquired code donor identity".into());
        }
        let selected_entry = format!("(table.init $code (i32.const {})", span.start);
        if !wat.contains(&selected_entry) {
            return Err("selected static donor has no checked table initializer".into());
        }
        let moved = wat.replacen(&selected_entry,
            &format!("(table.init $code (i32.const {})", span.start + 1), 1);
        if verify(&super::super::assemble_slot(moved.as_bytes())?, &origin, &install).is_ok() {
            return Err("rewritten static add table entry acquired code donor identity".into());
        }
        Ok(())
    }

    #[test]
    fn returned_target_refuses_forged_complete_graph_and_wrong_saved_owner() -> Result<(), String> {
        use serde_json::json;
        let environment = noble_kernel::contracts::environment()
            .map_err(|error| format!("environment: {error:?}"))?.enable_live_slots();
        let mut source = noble_contracts::source::Session::new_live_slots(environment)
            .map_err(|error| error.diagnostic().message.clone())?;
        let definition = source.prepare(b"def builder [ quote [ + ] compose ]", &[], LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        source.commit(definition).map_err(|error| error.diagnostic().message.clone())?;
        let call = source.prepare(b"builder", &[Ty::I64], LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        let selected = source.checked_selected_target(&call, "builder")
            .map_err(|error| format!("selected source: {error:?}"))?;
        let prepared = noble_wasm::source::Compiler::new_live_slots()
            .prepare_checked_selected(&selected)
            .map_err(|_| "selected compiler refused checked source".to_owned())?;
        let bound = prepared.selected_target().ok_or("missing checked selection")?;
        let target = json!({
            "definition_index":bound.definition_id.index(),
            "definition_identity":bound.definition_id.identity().to_string(),
            "source_generation":bound.source_generation.to_string(),
            "named_program_index":bound.named_program_index,
            "selected_source_sha256":crate::workflow::intrinsic::sha256(selected.source())
        });
        let origin = super::super::selected_origin_json(
            prepared.selected_origin().ok_or("missing checked origin")?, &target)?;
        let contracts = prepared.program_metadata().iter().map(|program|
            Ok(super::super::ProgramContract {
                input: super::super::canonical_stack(&program.stack_in, 0)?,
                output: super::super::canonical_stack(&program.stack_out, 0)?,
                effects: super::super::effects(&program.effects)?,
            })).collect::<Result<Vec<_>, String>>()?;
        let result = &origin["outputs"][0]["result"];
        let right = number(&result["right"], "program_index")? as usize;
        let static_program = &origin["programs"][right];
        let add = &static_program["operations"][0];
        let named = number(&origin, "named_program_index")? as usize;
        let quote = origin["programs"][named]["operations"].as_array()
            .ok_or("checked named operations missing")?.iter()
            .find(|op| op["kind"] == "quote-i64")
            .ok_or("checked quote operation missing")?;
        let graph = json!([
            {"kind":4,"payload":"0","x":2,"y":quote["quote_input_signature"],
                "z":static_program["output_signature"],"w":2,"n":2,
                "a":1,"b":2,"c":3,"textBytes":null},
            {"kind":4,"payload":"0","x":1,"y":quote["quote_input_signature"],
                "z":quote["quote_output_signature"],"w":1,"n":1,
                "a":4,"b":null,"c":5,"textBytes":null},
            {"kind":4,"payload":"0","x":static_program["entry"],
                "y":static_program["input_signature"],"z":static_program["output_signature"],
                "w":1,"n":1,"a":null,"b":null,"c":6,"textBytes":null},
            {"kind":9,"payload":"0","x":0,"y":0,"z":0,"w":0,"n":0,
                "a":5,"b":6,"c":null,"textBytes":null},
            {"kind":1,"payload":"2","x":0,"y":0,"z":0,"w":0,"n":0,
                "a":null,"b":null,"c":null,"textBytes":null},
            {"kind":8,"payload":"2","x":1,"y":0,"z":0,"w":0,"n":0,
                "a":null,"b":null,"c":null,"textBytes":null},
            {"kind":8,"payload":"4","x":2,"y":add["input_signature"],
                "z":add["output_signature"],"w":0,"n":0,
                "a":null,"b":null,"c":null,"textBytes":null}
        ]);
        let mut left = graph[1].clone();
        left["a"] = json!(1);
        left["c"] = json!(2);
        let mut static_child = graph[2].clone();
        static_child["c"] = json!(1);
        let saved = json!({"capture_values":[
            {"field":"cell_a","value":[left, graph[4], graph[5]]},
            {"field":"cell_b","value":[static_child, graph[6]]}
        ]});
        let input = [json!({"kind":"i64","value":"2"})];
        let admitted = |graph: &Value| anonymous_target_identity(&origin, &contracts, 0,
            "Program([I64]->[I64]!{})", &saved, graph, &input);
        if admitted(&graph)?.is_none() {
            return Err("checked bounded graph fixture did not reach the anonymous target".into());
        }
        for (name, node, field, forged) in [
            ("extra recipe edge", 3, "c", json!(4)),
            ("wrong installed static entry", 2, "x", json!(number(static_program, "entry")? + 1)),
            ("wrong captured value", 4, "payload", json!("3")),
            ("wrong static output signature", 2, "z", json!(123456)),
            ("wrong effect mask", 2, "payload", json!("1")),
            ("wrong outer interface", 0, "y", json!(123456)),
        ] {
            let mut wrong = graph.clone();
            wrong[node][field] = forged;
            if admitted(&wrong).is_ok() {
                return Err(format!("{name} acquired an anonymous target identity"));
            }
        }
        let mut truncated = graph.clone();
        truncated.as_array_mut().ok_or("graph is not an array")?.pop();
        if admitted(&truncated).is_ok() {
            return Err("truncated retained graph acquired an anonymous target identity".into());
        }
        let receipt = json!({"outcome":"saved-program-inspected","owner":"program-1",
            "saved_handle":4096,"guest_requests":0,"protected_operations":0,
            "selected_graph":graph});
        checked_saved_owner_inspection("program-1", 4096, &receipt)?;
        let mut wrong_owner = receipt.clone();
        wrong_owner["owner"] = json!("program-2");
        let mut wrong_handle = receipt.clone();
        wrong_handle["saved_handle"] = json!(4097);
        let mut protected = receipt.clone();
        protected["protected_operations"] = json!(1);
        for (name, row) in [
            ("different retained owner", wrong_owner),
            ("different backend handle", wrong_handle),
            ("inspection with protected effects", protected),
        ] {
            if checked_saved_owner_inspection("program-1", 4096, &row).is_ok() {
                return Err(format!("{name} acquired a saved owner identity"));
            }
        }
        Ok(())
    }
}
