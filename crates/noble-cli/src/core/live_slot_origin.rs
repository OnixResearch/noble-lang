//! Binary checks for the bounded, compiler-emitted, source-classified saved-Program grammar.
//! The name section locates functions; every selected table entry and its
//! entire decoded function body must match the source plan. Runtime helpers
//! and installer control flow are trusted compiler emission, not an
//! arbitrary-binary authenticity proof derived from custom names alone.
use std::collections::{BTreeMap, BTreeSet};
use serde_json::Value;
use wasmparser::{ElementItems, ElementKind, KnownCustom, Name, Operator, Parser, Payload, Validator};

#[path = "live_slot_returned_dag.rs"]
mod returned_dag;
pub(super) use returned_dag::{
    checked_code_donor_manifest, anonymous_target_identity, supports_anonymous_target,
};

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
        "add" | "sub" | "mul" | "dup" | "swap" | "drop" | "dip" | "run" | "compose" => {
            expected.push(format!("call op_{kind}"));
        }
        "quote-i64" | "quote-program" => {
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

fn lexical_operation(node: &Value, operation: &Value) -> Result<(), String> {
    let kind = text(operation, "kind")?;
    let builtin = match kind {
        "dup" => Some(0), "drop" => Some(1), "swap" => Some(2),
        "dip" => Some(3), "add" => Some(4), "sub" => Some(5), "mul" => Some(6),
        "quote-i64" | "quote-program" => Some(8), "compose" => Some(9),
        "run" => Some(10),
        "i64" => {
            let value = text(operation, "value")?.parse::<i64>()
                .map_err(|_| denied("source I64 literal is out of range"))?;
            if node["kind"] != "i64" || node["value"] != value.to_string()
                || operation["value"] != value.to_string() {
                return Err(denied("I64 instruction differs from replayed literal node"));
            }
            None
        }
        "static-program" => {
            if node["kind"] != "quotation" || !node["body"].is_array() {
                return Err(denied("static Program does not originate at a replayed quotation"));
            }
            None
        }
        _ => return Err(denied("selected instruction has no replayed lexical action")),
    };
    if let Some(expected) = builtin {
        if node["kind"] != "builtin" || node["definition"].as_u64() != Some(expected) {
            return Err(denied("selected instruction differs from replayed builtin node"));
        }
    }
    Ok(())
}

fn reachable_quotations(body: &[Value], nodes: &[Value], depth: usize,
    found: &mut BTreeSet<usize>) -> Result<(), String> {
    if depth >= 32 { return Err(denied("replayed lexical quotation exceeds depth bound")); }
    for lexical in body {
        let index = lexical.as_u64().and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| denied("replayed body has an invalid lexical NodeId"))?;
        let node = nodes.get(index).ok_or_else(|| denied("replayed body is truncated"))?;
        if node["kind"] == "quotation" {
            let children = rows(node, "body")?;
            if !found.insert(index) {
                return Err(denied("replayed quotation body has duplicate reachability"));
            }
            reachable_quotations(children, nodes, depth+1, found)?;
        }
    }
    Ok(())
}

fn verify_lexical_replay(origin: &Value, expected_replay: &Value,
    programs: &[Value], root_index: u32) -> Result<(), String> {
    let replay = origin.get("lexical_replay")
        .ok_or_else(|| denied("selected origin lacks independent lexical replay"))?;
    if replay != expected_replay {
        return Err(denied("serialized lexical replay differs from independently checked source"));
    }
    let source = text(replay, "source")?;
    let digest = crate::workflow::intrinsic::sha256(source.as_bytes());
    if replay["source_sha256"] != digest || origin["selected_source_sha256"] != digest
        || text(replay, "definition_name")?.is_empty() {
        return Err(denied("replayed lexical source bytes or name differ from checked selection"));
    }
    let nodes = rows(replay, "nodes")?;
    let generated_start = number(replay, "generated_start")? as usize;
    let generated_end = number(replay, "generated_end")? as usize;
    if nodes.is_empty() || nodes.len() > 512
        || number(replay, "original_nodes")? as usize != generated_start
        || generated_start > generated_end || generated_end.checked_add(1) != Some(nodes.len()) {
        return Err(denied("replayed original/generated lexical node bounds differ"));
    }
    let candidate = rows(replay, "candidate_to_lexical")?;
    if candidate.len() > 128 {
        return Err(denied("replayed candidate source occurrence exceeds selected bound"));
    }
    for (index, node) in nodes.iter().enumerate() {
        if number(node, "node")? as usize != index {
            return Err(denied("replayed lexical NodeIds are not complete and ordered"));
        }
        let generated = (generated_start..generated_end).contains(&index);
        let span = &node["authored_span"];
        let witness = &node["generated_witness"];
        if generated {
            if !span.is_null() || !witness.is_object() {
                return Err(denied("generated lexical node lacks exact replay witness"));
            }
            let generation = text(witness, "generation")?;
            if !matches!(generation, "binding-swap" | "projection-dup"
                | "projection-swap" | "capture-quote" | "capture-compose"
                | "wrap-quotation" | "wrap-dip" | "cleanup-drop") {
                return Err(denied("unknown generated lexical occurrence witness"));
            }
            for field in ["occurrence_node", "binder_node"] {
                if !witness[field].is_null() && number(witness, field)? as usize >= nodes.len() {
                    return Err(denied("generated binder/occurrence anchor is outside lexical AST"));
                }
            }
        } else if !witness.is_null() || span.is_null()
            || number(span, "start")? >= number(span, "end")?
            || number(span, "end")? as usize > source.len() {
            return Err(denied("authored lexical node lacks exact bounded source bytes"));
        }
        match text(node, "kind")? {
            "i64" => {
                let value = text(node, "value")?.parse::<i64>()
                    .map_err(|_| denied("replayed I64 literal is out of range"))?;
                if generated || node["value"] != value.to_string() {
                    return Err(denied("generated or noncanonical replayed I64 literal"));
                }
            }
            "builtin" => {
                if !matches!(number(node, "definition")?,
                    0 | 1 | 2 | 3 | 4 | 5 | 6 | 8 | 9 | 10) {
                    return Err(denied("unknown replayed builtin instruction"));
                }
            }
            "quotation" => for child in rows(node, "body")? {
                if child.as_u64().and_then(|id| usize::try_from(id).ok())
                    .is_none_or(|id| id >= nodes.len()) {
                    return Err(denied("replayed quotation has a foreign child"));
                }
            },
            _ => return Err(denied("unknown replayed lexical node kind")),
        }
    }
    let named = number(origin, "named_program_index")? as usize;
    if named >= programs.len() || root_index as usize >= programs.len()
        || named == root_index as usize {
        return Err(denied("selected lexical named/root Program indices are ambiguous"));
    }
    let mut seen_candidate = vec![false; candidate.len()];
    let mut seen_bodies = BTreeSet::new();
    let mut quoted = BTreeMap::new();
    for (index, program) in programs.iter().enumerate() {
        let selection = index as u32 == root_index;
        let named_program = index == named;
        let body = if selection {
            if program["source_artifact"] != "selection" || !program["body_node"].is_null() {
                return Err(denied("selected caller wrapper has a lexical quotation owner"));
            }
            continue;
        } else if named_program {
            if program["source_artifact"] != "definition" || !program["body_node"].is_null() {
                return Err(denied("selected named definition has a foreign quotation owner"));
            }
            rows(replay, "body")?
        } else {
            let body_node = number(program, "body_node")? as usize;
            let source = nodes.get(body_node)
                .ok_or_else(|| denied("selected static Program has a foreign quotation node"))?;
            if source["kind"] != "quotation" || !seen_bodies.insert(body_node)
                || program["source_artifact"] != "definition" {
                return Err(denied("selected static Program has an ambiguous lexical body"));
            }
            quoted.insert(body_node, index);
            rows(source, "body")?
        };
        let operations = rows(program, "operations")?;
        if operations.len() != body.len() {
            return Err(denied("selected Program omits or adds lexical body operations"));
        }
        for (operation, expected) in operations.iter().zip(body) {
            let lexical = number(operation, "lexical_node")? as usize;
            if expected.as_u64() != Some(lexical as u64) {
                return Err(denied("selected Program operation order differs from replayed body"));
            }
            let source = nodes.get(lexical)
                .ok_or_else(|| denied("compiled instruction has no replayed lexical node"))?;
            let checked = number(operation, "node")? as usize;
            let slot = seen_candidate.get_mut(checked)
                .ok_or_else(|| denied("compiled candidate NodeId lacks independent source binding"))?;
            if *slot || candidate[checked].as_u64() != Some(lexical as u64) {
                return Err(denied("compiled candidate NodeId or lexical binding was reused"));
            }
            *slot = true;
            if operation["generated"].as_bool() != Some((generated_start..generated_end).contains(&lexical))
                || operation["authored_span"] != source["authored_span"]
                || operation["generated_witness"] != source["generated_witness"] {
                return Err(denied("compiled operation differs from replayed source/generation witness"));
            }
            lexical_operation(source, operation)?;
        }
    }
    if seen_candidate.iter().any(|seen| !seen) {
        return Err(denied("selected source omits a checked candidate operation"));
    }
    for program in programs.iter().filter(|program| program["source_artifact"] == "definition") {
        for operation in rows(program, "operations")? {
            if operation["kind"] == "static-program"
                && quoted.get(&(number(operation, "lexical_node")? as usize))
                    != Some(&(number(operation, "static_program_index")? as usize)) {
                return Err(denied("source quotation body differs from installed static Program"));
            }
        }
    }
    let mut reachable = BTreeSet::new();
    reachable_quotations(rows(replay, "body")?, nodes, 0, &mut reachable)?;
    if quoted.len() + 2 != programs.len()
        || reachable != quoted.keys().copied().collect::<BTreeSet<_>>() {
        return Err(denied("selected source has an orphan or duplicated quotation Program"));
    }
    Ok(())
}

pub(super) fn verify(bytes: &[u8], origin: &Value, install: &Value,
    expected_replay: &Value) -> Result<(), String> {
    let root_index = number(origin, "root_program_index")?;
    let programs = rows(origin, "programs")?;
    if programs.is_empty() || programs.len() > 128 {
        return Err(denied("incomplete selected program map"));
    }
    verify_lexical_replay(origin, expected_replay, programs, root_index)?;
    Validator::new().validate_all(bytes).map_err(|_| denied("invalid WebAssembly module"))?;
    let selected = install.get("selected_target").ok_or_else(|| denied("no checked selected target"))?;
    for key in ["definition_index", "definition_identity", "source_generation",
        "selected_source_sha256", "named_program_index"] {
        if origin.get(key) != selected.get(key) { return Err(denied("selected definition identity differs")); }
    }
    if root_index != number(&install["target_metadata"], "root_program_index")? {
        return Err(denied("root program index differs"));
    }
    let metadata = rows(install, "program_metadata")?;
    if programs.len() != metadata.len() {
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
            let generated = operation.get("generated").and_then(Value::as_bool)
                .ok_or_else(|| denied("selected operation lacks checked generation classification"))?;
            if text(program, "source_artifact")? == "selection" {
                if index as u32 != root_index || operations.len() != 1
                    || number(operation, "node")? != 0
                    || text(operation, "kind")? != "call-selected" || generated
                    || !operation["lexical_node"].is_null()
                    || !operation["authored_span"].is_null()
                    || !operation["generated_witness"].is_null() {
                    return Err(denied("ambiguous selected caller wrapper"));
                }
            } else if text(program, "source_artifact")? == "definition" {
                let kind = text(operation, "kind")?;
                if generated {
                    if !operation["authored_span"].is_null()
                        || !matches!(kind, "dup" | "swap" | "drop" | "dip"
                            | "static-program" | "quote-i64" | "quote-program" | "compose") {
                        return Err(denied("generated operation has an authored span or foreign action"));
                    }
                } else if kind == "dip" || operation["authored_span"].is_null()
                    || number(&operation["authored_span"], "start")?
                        >= number(&operation["authored_span"], "end")?
                    || !operation["generated_witness"].is_null() {
                    return Err(denied("authored operation lacks an exact nonempty source span"));
                }
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
        let expected_replay = super::super::lexical_replay_json(&selected)?;
        let origin = super::super::selected_origin_json(
            prepared.selected_origin().ok_or("missing classified selected source")?,
            &target, &expected_replay)?;
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
        verify(&binary, &origin, &install, &expected_replay)?;

        let mut wrong_site = origin.clone();
        wrong_site["programs"][0]["operations"][0]["table_entry"] = serde_json::json!(span.start + 1);
        if verify(&binary, &wrong_site, &install, &expected_replay).is_ok() {
            return Err("wrong selected quote site acquired origin".into());
        }
        let mut wrong_node = origin.clone();
        wrong_node["programs"][0]["operations"][0]["node"] = serde_json::json!(
            number(&origin["programs"][0]["operations"][0], "node")? + 1000);
        if verify(&binary, &wrong_node, &install, &expected_replay).is_ok() {
            return Err("changed lexical source NodeId acquired checked origin".into());
        }
        let mut wrong_lexical = origin.clone();
        wrong_lexical["programs"][0]["operations"][0]["lexical_node"] = serde_json::json!(
            number(&origin["programs"][0]["operations"][0], "lexical_node")? + 1);
        if verify(&binary, &wrong_lexical, &install, &expected_replay).is_ok() {
            return Err("rebound kernel-to-lexical NodeId acquired checked origin".into());
        }
        let mut wrong_bytes = origin.clone();
        wrong_bytes["lexical_replay"]["source"] = serde_json::json!(
            format!("{} ", text(&origin["lexical_replay"], "source")?));
        if verify(&binary, &wrong_bytes, &install, &expected_replay).is_ok() {
            return Err("changed retained lexical source bytes acquired checked origin".into());
        }
        let mut wrong_span = origin.clone();
        let authored = &origin["programs"][0]["operations"][0]["authored_span"];
        wrong_span["programs"][0]["operations"][0]["authored_span"]["start"] =
            serde_json::json!(number(authored,"start")? + 1);
        wrong_span["programs"][0]["operations"][0]["authored_span"]["end"] =
            serde_json::json!(number(authored,"end")? + 1);
        if verify(&binary, &wrong_span, &install, &expected_replay).is_ok() {
            return Err("shifted authored source span acquired checked origin".into());
        }
        let mut forged_generated = origin.clone();
        forged_generated["programs"][0]["operations"][0]["generated"] = serde_json::json!(true);
        forged_generated["programs"][0]["operations"][0]["authored_span"] = Value::Null;
        if verify(&binary, &forged_generated, &install, &expected_replay).is_ok() {
            return Err("authored quote relabeled as generated acquired checked origin".into());
        }
        let mut wrong_action = origin.clone();
        wrong_action["programs"][0]["operations"][2]["kind"] = serde_json::json!("drop");
        if verify(&binary, &wrong_action, &install, &expected_replay).is_ok() {
            return Err("wrong selected compose operation acquired origin".into());
        }
        let mut extra = origin.clone();
        let duplicate = extra["programs"][0]["operations"][0].clone();
        extra["programs"][0]["operations"].as_array_mut()
            .ok_or("origin operations missing")?.push(duplicate);
        if verify(&binary, &extra, &install, &expected_replay).is_ok() {
            return Err("extra ambiguous selected instruction acquired origin".into());
        }

        let wat = std::str::from_utf8(prepared.wat()).map_err(|error| error.to_string())?;
        let changed_code = wat.replacen("(call $op_compose)", "(call $op_drop)", 1);
        if changed_code == wat || verify(&super::super::assemble_slot(changed_code.as_bytes())?, &origin, &install, &expected_replay).is_ok() {
            return Err("changed decoded operation body acquired checked origin".into());
        }
        let before = format!("(table.init $code (i32.const {}) ", span.start);
        let after = format!("(table.init $code (i32.const {}) ", span.start + 1);
        let changed_table = wat.replacen(&before, &after, 1);
        if changed_table == wat {
            return Err("test could not relocate code table initializer".into());
        }
        if verify(&super::super::assemble_slot(changed_table.as_bytes())?, &origin, &install, &expected_replay).is_ok() {
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
        if verify(&super::super::assemble_slot(forged_initializer.as_bytes())?, &origin, &install, &expected_replay).is_ok() {
            return Err("second passive element overwrote checked table entries".into());
        }
        let closing = wat.rfind(')').ok_or("module closing parenthesis missing")?;
        let forged_helper = format!("{}{}(func $overwrite_checked_table {} )\n{}",
            &wat[..closing], evil_segment,
            format_args!("(table.init $evil (i32.const {}) (i32.const 0) (i32.const 1))", span.start),
            &wat[closing..]);
        if verify(&super::super::assemble_slot(forged_helper.as_bytes())?, &origin, &install, &expected_replay).is_ok() {
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
            later.selected_origin().ok_or("later selected source lacks classified origin")?,
            &target, &expected_replay)?;
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
        verify(&super::super::assemble_slot(later_wat.as_bytes())?, &later_origin, &later_install, &expected_replay)?;
        let late_init = format!("(table.init $core (i32.const 0) (i32.const 0) (i32.const 4))\n\
            (table.init $code (i32.const {})", later_span.start);
        let real_init = format!("(table.init $code (i32.const {})", later_span.start);
        let forged = later_wat.replacen(&real_init, &late_init, 1);
        if forged == later_wat { return Err("later initializer lacks checked code entry".into()); }
        let closing = forged.rfind(')').ok_or("later module closing parenthesis missing")?;
        let forged = format!("{}(elem $core func $empty_entry $quote_entry $compose_entry $restore_entry)\n{}",
            &forged[..closing], &forged[closing..]);
        if verify(&super::super::assemble_slot(forged.as_bytes())?, &later_origin, &later_install, &expected_replay).is_ok() {
            return Err("later-generation forged native core segment acquired selected origin".into());
        }
        Ok(())
    }

    #[test]
    fn selected_binary_rejects_forged_generated_dip_source_witness() -> Result<(), String> {
        use serde_json::json;
        let environment = noble_kernel::contracts::environment()
            .map_err(|error| format!("environment: {error:?}"))?.enable_live_slots();
        let mut source = noble_contracts::source::Session::new_live_slots(environment)
            .map_err(|error| error.diagnostic().message.clone())?;
        let definition = source.prepare(
            b"def builder [ quote [ + ] compose let q q q compose [ 0 + ] compose ]", &[], LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        source.commit(definition).map_err(|error| error.diagnostic().message.clone())?;
        let call = source.prepare(b"builder", &[Ty::I64], LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        let selected = source.checked_selected_target(&call, "builder")
            .map_err(|error| format!("selected source: {error:?}"))?;
        let prepared = noble_wasm::source::Compiler::new_live_slots()
            .prepare_checked_selected(&selected)
            .map_err(|_| "generated source compilation refused".to_owned())?;
        let bound = prepared.selected_target().ok_or("missing checked generated selection")?;
        let target = json!({"definition_index":bound.definition_id.index(),
            "definition_identity":bound.definition_id.identity().to_string(),
            "source_generation":bound.source_generation.to_string(),
            "named_program_index":bound.named_program_index,
            "selected_source_sha256":crate::workflow::intrinsic::sha256(selected.source())});
        let expected_replay = super::super::lexical_replay_json(&selected)?;
        let origin = super::super::selected_origin_json(
            prepared.selected_origin().ok_or("missing generated selected origin")?,
            &target,&expected_replay)?;
        let metadata = prepared.program_metadata().iter().map(|program| json!({
            "entry":program.entry,"program_index":program.program_index,
            "input_signature":program.input_signature,
            "output_signature":program.output_signature,"effect_mask":program.effect_mask
        })).collect::<Vec<_>>();
        let selected_target = prepared.target_metadata()
            .ok_or("missing generated root metadata")?;
        let span = prepared.code_span();
        let install = json!({"selected_target":target,
            "target_metadata":{"root_program_index":selected_target.root_program_index,
                "stack_out":super::super::canonical_stack(&selected_target.stack_out,0)?},
            "code_span":{"start":span.start,"length":span.length,"generation":span.generation},
            "program_metadata":metadata});
        let binary = super::super::assemble_slot(prepared.wat())?;
        verify(&binary,&origin,&install,&expected_replay)?;

        let mut dip = None;
        for (program_index,program) in rows(&origin,"programs")?.iter().enumerate() {
            for (operation_index,operation) in rows(program,"operations")?.iter().enumerate() {
                if operation["kind"] == "dip" && operation["generated"] == true
                    && operation["generated_witness"]["binder_node"].as_u64().is_some() {
                    dip = Some((program_index,operation_index));
                    break;
                }
            }
            if dip.is_some() { break; }
        }
        let (program_index,operation_index) = dip
            .ok_or("selected source omitted witnessed generated dip")?;
        let op = &origin["programs"][program_index]["operations"][operation_index];
        let mut forged_span = origin.clone();
        forged_span["programs"][program_index]["operations"][operation_index]
            ["authored_span"] = json!({"start":0,"end":1});
        if verify(&binary,&forged_span,&install,&expected_replay).is_ok() {
            return Err("generated dip gained forged authored source bytes".into());
        }
        let mut forged_binder = origin.clone();
        let binder = number(&op["generated_witness"],"binder_node")?;
        forged_binder["programs"][program_index]["operations"][operation_index]
            ["generated_witness"]["binder_node"] = json!(if binder == 0 {1} else {0});
        if verify(&binary,&forged_binder,&install,&expected_replay).is_ok() {
            return Err("generated dip rebound its lexical binder".into());
        }
        let mut forged_candidate = origin.clone();
        let lexical = number(op,"lexical_node")?;
        let mut forged_lexical_witness = origin.clone();
        forged_lexical_witness["lexical_replay"]["nodes"][lexical as usize]
            ["generated_witness"]["binder_node"] = json!(if binder == 0 {1} else {0});
        if verify(&binary,&forged_lexical_witness,&install,&expected_replay).is_ok() {
            return Err("generated dip accepted a forged independent binder witness".into());
        }
        forged_candidate["lexical_replay"]["candidate_to_lexical"]
            [number(op,"node")? as usize] = json!(if lexical == 0 {1} else {0});
        if verify(&binary,&forged_candidate,&install,&expected_replay).is_ok() {
            return Err("generated dip rebound its checked candidate NodeId".into());
        }
        let mut forged_source = origin.clone();
        forged_source["lexical_replay"]["source"] =
            json!(format!("{} ",text(&expected_replay,"source")?));
        if verify(&binary,&forged_source,&install,&expected_replay).is_ok() {
            return Err("generated dip accepted changed lexical source bytes".into());
        }
        let authored = rows(&expected_replay,"nodes")?.iter()
            .position(|node| node["authored_span"].is_object())
            .ok_or("selected source lacks authored AST span")?;
        let mut forged_ast_span = origin.clone();
        let original_end = number(&expected_replay["nodes"][authored]["authored_span"],"end")?;
        forged_ast_span["lexical_replay"]["nodes"][authored]["authored_span"]["start"] =
            json!(original_end);
        if verify(&binary,&forged_ast_span,&install,&expected_replay).is_ok() {
            return Err("generated dip accepted a shifted original AST span".into());
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
        let expected_replay = super::super::lexical_replay_json(&selected)?;
        let origin = super::super::selected_origin_json(
            prepared.selected_origin().ok_or("missing classified alias source")?,
            &target, &expected_replay)?;
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
        verify(&binary, &origin, &install, &expected_replay)?;
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
        if verify(&binary, &substituted_quote, &install, &expected_replay).is_ok() {
            return Err("second alias quotation borrowed the first checked code site".into());
        }
        let mut substituted_compose = origin.clone();
        substituted_compose["programs"][named]["operations"][composes[0]]["kind"] = json!("drop");
        if verify(&binary, &substituted_compose, &install, &expected_replay).is_ok() {
            return Err("inner alias composition borrowed a different compiled action".into());
        }
        let wat = std::str::from_utf8(prepared.wat())
            .map_err(|error| error.to_string())?;
        let native = "(elem $core func $empty_entry $quote_entry $compose_entry $restore_entry)";
        if !wat.contains(native) { return Err("selected native donor has no core entry list".into()); }
        let swapped = wat.replacen(native,
            "(elem $core func $empty_entry $compose_entry $quote_entry $restore_entry)", 1);
        if verify(&super::super::assemble_slot(swapped.as_bytes())?, &origin, &install, &expected_replay).is_ok() {
            return Err("rewritten native quote/compose table entry acquired code donor identity".into());
        }
        let selected_entry = format!("(table.init $code (i32.const {})", span.start);
        if !wat.contains(&selected_entry) {
            return Err("selected static donor has no checked table initializer".into());
        }
        let moved = wat.replacen(&selected_entry,
            &format!("(table.init $code (i32.const {})", span.start + 1), 1);
        if verify(&super::super::assemble_slot(moved.as_bytes())?, &origin, &install, &expected_replay).is_ok() {
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
        let expected_replay = super::super::lexical_replay_json(&selected)?;
        let origin = super::super::selected_origin_json(
            prepared.selected_origin().ok_or("missing checked origin")?,
            &target, &expected_replay)?;
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
        let saved = json!({"capture_status":"checked-selected-origin","capture_values":[
            {"field":"cell_a","value":[left, graph[4], graph[5]]},
            {"field":"cell_b","value":[static_child, graph[6]]}
        ]});
        let input = [json!({"kind":"i64","value":"2"})];
        let admitted = |graph: &Value| anonymous_target_identity(&origin, &contracts, 0,
            "Program([I64]->[I64]!{})", &saved, graph, &input);
        if admitted(&graph)?.is_none() {
            return Err("checked bounded graph fixture did not reach the anonymous target".into());
        }
        let mut zero_graph = graph.clone();
        zero_graph[4]["payload"] = json!("0");
        zero_graph[5]["payload"] = json!("0");
        let mut zero_saved = saved.clone();
        zero_saved["capture_values"][0]["value"][1]["payload"] = json!("0");
        zero_saved["capture_values"][0]["value"][2]["payload"] = json!("0");
        let zero = |spelling| anonymous_target_identity(&origin, &contracts, 0,
            "Program([I64]->[I64]!{})", &zero_saved, &zero_graph,
            &[json!({"kind":"i64","value":spelling})]);
        let zero_id = zero("0")?.ok_or("canonical zero acquired no checked identity")?;
        if zero("-0")? != Some(zero_id) {
            return Err("same signed I64 root value changed returned Program identity".into());
        }
        for (name, node, field, forged) in [
            ("extra recipe edge", 3, "c", json!(4)),
            ("wrong installed static entry", 2, "x", json!(number(static_program, "entry")? + 1)),
            ("wrong captured value", 4, "payload", json!("3")),
            ("wrong static recipe opcode", 6, "payload", json!("5")),
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
        let mut extra = graph.clone();
        extra.as_array_mut().ok_or("graph is not an array")?.push(graph[4].clone());
        if admitted(&extra).is_ok() {
            return Err("extra retained graph cell acquired an anonymous target identity".into());
        }
        let mut cyclic = graph.clone();
        cyclic[3]["a"] = json!(3);
        if admitted(&cyclic).is_ok() {
            return Err("cyclic retained recipe acquired an anonymous target identity".into());
        }
        let mut forged_capture = saved.clone();
        forged_capture["capture_values"][0]["value"][1]["payload"] = json!("3");
        if anonymous_target_identity(&origin, &contracts, 0,
            "Program([I64]->[I64]!{})", &forged_capture, &graph, &input).is_ok() {
            return Err("forged public child capture acquired an anonymous target identity".into());
        }
        for (name, replacement) in [
            ("forward Program alias", json!({"kind":"reference","alias":1})),
            ("wrong Program operand", json!({"kind":"quote-program","alias":1,
                "node":result["left"]["node"],"table_entry":result["left"]["table_entry"],
                "operand":result["right"]})),
            ("fabricated fixed expression for root capture",
                json!({"kind":"quote-i64","alias":1,
                    "node":result["left"]["node"],
                    "table_entry":result["left"]["table_entry"],
                    "operand":{"kind":"fixed-i64","value":"2",
                        "recipe":{"kind":"literal","value":"2"}}})),
        ] {
            let mut forged = origin.clone();
            forged["outputs"][0]["result"]["left"] = replacement;
            if supports_anonymous_target(&forged, &contracts, 0,
                "Program([I64]->[I64]!{})", &input)? {
                return Err(format!("{name} acquired a checked source identity"));
            }
        }
        let mut effectful = origin.clone();
        effectful["programs"][right]["effect_mask"] = json!(1);
        if supports_anonymous_target(&effectful, &contracts, 0,
            "Program([I64]->[I64]!{})", &input)? {
            return Err("effectful static continuation acquired pure returned identity".into());
        }
        let mut unchecked_capture = saved.clone();
        unchecked_capture["capture_status"] = json!("backend-observed-source-occurrence-unavailable");
        if anonymous_target_identity(&origin, &contracts, 0,
            "Program([I64]->[I64]!{})", &unchecked_capture, &graph, &input).is_ok() {
            return Err("unchecked capture provenance acquired an anonymous target identity".into());
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
