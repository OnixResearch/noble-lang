//! Independent binary correspondence for the bounded, source-classified saved-Program grammar.
//! A name section locates functions, but never supplies authority: every selected
//! table entry and its *entire* decoded function body must match the source plan.
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
            "code_span":{"start":span.start,"length":span.length},
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
        Ok(())
    }
}
