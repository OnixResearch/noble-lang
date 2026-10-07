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

// Experimental local IDs for exactly one returned `quote(I64) [ + ] compose`
// target. This is deliberately narrower than SelectedOriginMetadata: its
// general quote/compose grammar has no checked logical-alias or lexical-owner
// witness. A missing witness never becomes an anonymous target ID.
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

pub(super) fn supports_anonymous_target(
    origin: &Value,
    contracts: &[super::ProgramContract],
    position: usize,
    descriptor: &str,
    root_inputs: &[Value],
) -> Result<bool, String> {
    Ok(anonymous_shape(origin, contracts, position, descriptor, root_inputs)?.is_some())
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
        return Ok(None);
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
