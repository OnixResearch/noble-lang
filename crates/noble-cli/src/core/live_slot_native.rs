//! Byte-derived, bounded native-body witness for the *selected installed* donors.
//! Exact direct-opcode checks and a complete reachable-body snapshot are not a
//! Wasm semantic refinement proof; proof-required admission remains closed.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use serde_json::{Value, json};
use wasmparser::{ElementItems, KnownCustom, Name, Operator, Parser, Payload, ValType, Validator};

#[path = "live_slot_native_storage.rs"]
mod storage;

fn denied(reason: &str) -> String { format!("selected native body witness refused: {reason}") }

struct Module<'a> {
    bytes: &'a [u8],
    sha256: &'a str,
    imported: u32,
    imports: BTreeMap<u32, (String, String)>,
    names: BTreeMap<u32, String>,
    by_name: BTreeMap<String, u32>,
    globals: BTreeMap<u32, String>,
    segments: BTreeMap<String, u32>,
    elements: Vec<Vec<u32>>,
    bodies: Vec<wasmparser::FunctionBody<'a>>,
    function_types: Vec<u32>,
    types: Vec<wasmparser::FuncType>,
}

impl<'a> Module<'a> {
    fn parse(bytes: &'a [u8], expected_sha256: &'a str) -> Result<Self, String> {
        if crate::workflow::intrinsic::sha256(bytes) != expected_sha256 {
            return Err(denied("original installed Wasm bytes differ from selected donor SHA"));
        }
        Validator::new().validate_all(bytes).map_err(|_| denied("invalid installed Wasm binary"))?;
        let mut module = Self {
            bytes, sha256:expected_sha256, imported:0, imports:BTreeMap::new(),
            names:BTreeMap::new(), by_name:BTreeMap::new(), globals:BTreeMap::new(),
            segments:BTreeMap::new(),
            elements:Vec::new(), bodies:Vec::new(), function_types:Vec::new(), types:Vec::new(),
        };
        for payload in Parser::new(0).parse_all(bytes) {
            match payload.map_err(|_| denied("invalid installed Wasm section"))? {
                Payload::TypeSection(section) => for ty in section.into_iter_err_on_gc_types() {
                    module.types.push(ty.map_err(|_| denied("invalid installed function signature"))?);
                },
                Payload::ImportSection(section) => for import in section {
                    let import = import.map_err(|_| denied("invalid installed import"))?;
                    if let wasmparser::TypeRef::Func(ty) | wasmparser::TypeRef::FuncExact(ty) = import.ty {
                        module.function_types.push(ty);
                        if module.imports.insert(module.imported,
                            (import.module.to_owned(), import.name.to_owned())).is_some() {
                            return Err(denied("duplicate imported function index"));
                        }
                        module.imported += 1;
                    }
                },
                Payload::FunctionSection(section) => for ty in section {
                    module.function_types.push(ty.map_err(|_| denied("invalid installed function type"))?);
                },
                Payload::CodeSectionEntry(body) => module.bodies.push(body),
                Payload::ElementSection(section) => for element in section {
                    let element = element.map_err(|_| denied("invalid installed element segment"))?;
                    let ElementItems::Functions(items) = element.items else {
                        return Err(denied("native element segment is not a function list"));
                    };
                    module.elements.push(items.into_iter().collect::<Result<Vec<_>, _>>()
                        .map_err(|_| denied("invalid native function entries"))?);
                },
                Payload::CustomSection(section) => if let KnownCustom::Name(names) = section.as_known() {
                    for entry in names {
                        match entry.map_err(|_| denied("invalid installed name section"))? {
                            Name::Function(names) => for row in names {
                                let row = row.map_err(|_| denied("invalid installed function name"))?;
                                if module.names.insert(row.index, row.name.to_owned()).is_some()
                                    || module.by_name.insert(row.name.to_owned(), row.index).is_some() {
                                    return Err(denied("duplicate or ambiguous installed function name"));
                                }
                            },
                            Name::Global(names) => for row in names {
                                let row = row.map_err(|_| denied("invalid installed global name"))?;
                                if module.globals.insert(row.index, row.name.to_owned()).is_some() {
                                    return Err(denied("ambiguous installed global name"));
                                }
                            },
                            Name::Element(names) => for row in names {
                                let row = row.map_err(|_| denied("invalid installed element name"))?;
                                if module.segments.insert(row.name.to_owned(), row.index).is_some() {
                                    return Err(denied("duplicate installed element name"));
                                }
                            },
                            _ => {},
                        }
                    }
                },
                _ => {},
            }
        }
        if module.bodies.len() + module.imported as usize != module.function_types.len()
            || module.function_types.len() > 4096 {
            return Err(denied("incomplete or unbounded installed function map"));
        }
        // This bounded returned target is pure. A changed/duplicated host
        // import is not part of its semantic closure, even if signatures
        // happen to match and the altered binary remains valid WebAssembly.
        let expected_imports: &[(&str, &[ValType])] = &[
            ("live_select", &[ValType::I32, ValType::I32]),
            ("live_bind_validate", &[ValType::I32, ValType::I32]),
            ("live_frame_enter", &[ValType::I32, ValType::I32, ValType::I32]),
            ("live_frame_leave", &[ValType::I32]),
            ("resource_validate", &[ValType::I32, ValType::I32]),
            ("resource_bind_validate", &[ValType::I32, ValType::I32, ValType::I32,
                ValType::I32, ValType::I32, ValType::I32]),
        ];
        if module.imports.len() != expected_imports.len() {
            return Err(denied("selected pure target has a different host-function import set"));
        }
        for (index, (name, params)) in expected_imports.iter().enumerate() {
            let imported = module.imports.get(&(index as u32))
                .ok_or_else(|| denied("selected host-function import order is incomplete"))?;
            if imported.0 != "noble" || imported.1 != *name {
                return Err(denied("selected host-function import changed identity"));
            }
            module.signature(index as u32, params, &[ValType::I32])?;
        }
        Ok(module)
    }

    fn index(&self, name: &str) -> Result<u32, String> {
        let index = *self.by_name.get(name)
            .ok_or_else(|| denied("required installed native function is unnamed"))?;
        if index < self.imported { return Err(denied("installed native function resolves to an import")); }
        Ok(index)
    }

    fn body(&self, index: u32) -> Result<&wasmparser::FunctionBody<'a>, String> {
        let local = index.checked_sub(self.imported)
            .ok_or_else(|| denied("expected native body resolves to an import"))?;
        self.bodies.get(local as usize)
            .ok_or_else(|| denied("installed native function has no body"))
    }

    fn signature(&self, index: u32, params: &[ValType], results: &[ValType]) -> Result<(), String> {
        let ty = *self.function_types.get(index as usize)
            .ok_or_else(|| denied("native function has no installed type"))?;
        let ty = self.types.get(ty as usize)
            .ok_or_else(|| denied("native function type is out of range"))?;
        if ty.params() != params || ty.results() != results {
            return Err(denied("native function signature differs from reviewed entry"));
        }
        Ok(())
    }

    // Exact operator order, named call targets, local layout, and expected
    // Wasm primitive i64.add are independently decoded from installed bytes.
    // Helper functions are *not* declared semantically correct by these plans.
    fn direct_body(&self, name: &str, expected: &[&str], locals: &[ValType]) -> Result<(), String> {
        let body = self.body(self.index(name)?)?;
        let reader = body.get_locals_reader().map_err(|_| denied("native locals are invalid"))?;
        let mut observed_locals = Vec::new();
        for row in reader {
            let (count, ty) = row.map_err(|_| denied("native locals cannot be decoded"))?;
            for _ in 0..count {
                if observed_locals.len() >= 8 { return Err(denied("native body has too many locals")); }
                observed_locals.push(ty);
            }
        }
        if observed_locals != locals { return Err(denied("native local types differ from reviewed body")); }
        let mut actual = Vec::new();
        for op in body.get_operators_reader().map_err(|_| denied("native body is invalid"))? {
            let token = match op.map_err(|_| denied("native operator cannot be decoded"))? {
                Operator::GlobalGet { global_index } => format!("global.get {}", self.globals.get(&global_index)
                    .ok_or_else(|| denied("native global read has no installed name"))?),
                Operator::GlobalSet { global_index } => format!("global.set {}", self.globals.get(&global_index)
                    .ok_or_else(|| denied("native global write has no installed name"))?),
                Operator::LocalGet { local_index } => format!("local.get {local_index}"),
                Operator::LocalSet { local_index } => format!("local.set {local_index}"),
                Operator::I32Const { value } => format!("i32.const {value}"),
                Operator::I64Const { value } => format!("i64.const {value}"),
                Operator::Call { function_index } => format!("call {}", self.names.get(&function_index)
                    .ok_or_else(|| denied("native direct call target has no installed name"))?),
                Operator::CallIndirect { type_index, table_index, .. } => {
                    let ty = self.types.get(type_index as usize)
                        .ok_or_else(|| denied("native indirect call type is invalid"))?;
                    if table_index != 0 || ty.params() != [ValType::I32] || !ty.results().is_empty() {
                        return Err(denied("native dispatch calls a different table or entry type"));
                    }
                    "call_indirect entry".to_owned()
                }
                Operator::I32Add => "i32.add".to_owned(),
                Operator::I32Sub => "i32.sub".to_owned(),
                Operator::I32Mul => "i32.mul".to_owned(),
                Operator::I32Or => "i32.or".to_owned(),
                Operator::I32Eqz => "i32.eqz".to_owned(),
                Operator::I64Add => "i64.add".to_owned(),
                Operator::I32Load { memarg } if memarg.memory == 0 && memarg.align == 2 =>
                    format!("i32.load {}", memarg.offset),
                Operator::I32Store { memarg } if memarg.memory == 0 && memarg.align == 2 =>
                    format!("i32.store {}", memarg.offset),
                Operator::I64Store { memarg } if memarg.memory == 0 && memarg.align == 3 =>
                    format!("i64.store {}", memarg.offset),
                Operator::Block { blockty: wasmparser::BlockType::Empty } => "block".to_owned(),
                Operator::Loop { blockty: wasmparser::BlockType::Empty } => "loop".to_owned(),
                Operator::Br { relative_depth } => format!("br {relative_depth}"),
                Operator::BrIf { relative_depth } => format!("br_if {relative_depth}"),
                Operator::If { blockty: wasmparser::BlockType::Empty } => "if".to_owned(),
                Operator::Else => "else".to_owned(),
                Operator::Drop => "drop".to_owned(),
                Operator::Return => "return".to_owned(),
                Operator::End => "end".to_owned(),
                _ => return Err(denied("unexpected native direct-body opcode")),
            };
            actual.push(token);
            if actual.len() > 96 { return Err(denied("native direct body exceeds finite instruction bound")); }
        }
        // Global numbers are resolved against actual imported global names by
        // the original source/byte validator. Only the quote counter is read;
        // its identity is immaterial to the pure returned I64 execution law.
        let expected = expected.iter().map(|token| (*token).to_owned()).collect::<Vec<_>>();
        if actual != expected { return Err(denied("installed native operator body differs from exact reviewed opcode plan")); }
        Ok(())
    }

    fn closure(&self, roots: &[&str], table_roots: &[u32]) -> Result<Value, String> {
        let mut pending = VecDeque::new();
        for name in roots { pending.push_back(self.index(name)?); }
        for index in table_roots { pending.push_back(*index); }
        let mut seen = BTreeSet::new();
        let mut rows = Vec::new();
        let mut total_bytes = 0usize;
        while let Some(index) = pending.pop_front() {
            if !seen.insert(index) { continue; }
            if seen.len() > 256 { return Err(denied("native reachable function closure exceeds bound")); }
            if index < self.imported {
                let (module, name) = self.imports.get(&index)
                    .ok_or_else(|| denied("native reachable import is missing"))?;
                rows.push(json!({"function_index":index,"import_module":module,"import_name":name,
                    "type_index":self.function_types[index as usize]}));
                continue;
            }
            let body = self.body(index)?;
            let bytes = self.bytes.get(body.range())
                .ok_or_else(|| denied("native body extends beyond installed module bytes"))?;
            total_bytes = total_bytes.checked_add(bytes.len())
                .ok_or_else(|| denied("native body byte count overflows"))?;
            if total_bytes > 1_048_576 { return Err(denied("native body closure exceeds byte bound")); }
            let mut calls = Vec::new();
            let mut indirect_calls = Vec::new();
            for op in body.get_operators_reader().map_err(|_| denied("native closure body is invalid"))? {
                match op.map_err(|_| denied("native closure opcode is invalid"))? {
                    Operator::Call { function_index } => {
                        pending.push_back(function_index);
                        calls.push(function_index);
                    }
                    Operator::CallIndirect { type_index, table_index, .. } => {
                        indirect_calls.push(json!({"type_index":type_index,"table_index":table_index}));
                    }
                    Operator::ReturnCall { .. } | Operator::ReturnCallIndirect { .. } => {
                        return Err(denied("native closure contains an unreviewed tail call"));
                    }
                    _ => {},
                }
            }
            rows.push(json!({"function_index":index,
                "name":self.names.get(&index).ok_or_else(|| denied("reachable native helper is unnamed"))?,
                "type_index":self.function_types[index as usize],
                "body_sha256":crate::workflow::intrinsic::sha256(bytes),
                "body_length":bytes.len(),"calls":calls,"indirect_calls":indirect_calls}));
        }
        rows.sort_by_key(|row| row["function_index"].as_u64().unwrap_or(u64::MAX));
        Ok(json!({"module_sha256":self.sha256,"roots":roots,
            "selected_table_function_indices":table_roots,"functions":rows}))
    }
}

pub(super) fn probe_storage(
    core_bytes: &[u8], core_sha: &str, static_bytes: &[u8], static_sha: &str,
    memory: &[u8], cursor: u32, handles: &[u32],
) -> Result<Value, String> {
    let core = Module::parse(core_bytes, core_sha)?;
    let static_donor = Module::parse(static_bytes, static_sha)?;
    storage::probe(&core, memory, cursor, handles)?;
    storage::probe(&static_donor, memory, cursor, handles)?;
    Ok(json!({"schema":"noble-live-slot-installed-storage-one-state/v1",
        "scope":"exact selected storage reads and stale-epoch negatives only; writes/refcounts/future calls unproved",
        "native_module_sha256":core_sha,"static_module_sha256":static_sha,
        "selected_handles":handles,"heap_cursor":cursor}))
}

pub(super) fn attest(
    core_bytes: &[u8], core_sha: &str, static_bytes: &[u8], static_sha: &str,
    static_span_start: u32, add_entries: &[u32],
) -> Result<Value, String> {
    let core = Module::parse(core_bytes, core_sha)?;
    let static_donor = Module::parse(static_bytes, static_sha)?;
    let segment = *core.segments.get("core")
        .ok_or_else(|| denied("first-generation native core segment is missing"))?;
    let entries = core.elements.get(segment as usize)
        .ok_or_else(|| denied("first-generation native core entries are absent"))?;
    if entries.len() != 4 || entries[1] != core.index("quote_entry")?
        || entries[2] != core.index("compose_entry")? {
        return Err(denied("native quote/compose table entries differ from installed bodies"));
    }
    core.signature(entries[1], &[ValType::I32], &[])?;
    core.signature(entries[2], &[ValType::I32], &[])?;
    static_donor.signature(static_donor.index("op_add")?, &[], &[])?;
    // Locate the imported quote counter by its checked name; do not allow a
    // renamed failure/phase/global write in the native entry side branch.
    let quote = core.body(entries[1])?;
    let mut globals = Vec::new();
    for op in quote.get_operators_reader().map_err(|_| denied("quote body cannot be decoded"))? {
        if let Operator::GlobalGet { global_index } = op.map_err(|_| denied("quote opcode cannot be decoded"))? {
            globals.push(global_index);
        }
    }
    if globals.len() != 1 || core.globals.get(&globals[0]).map(String::as_str) != Some("quote_invocations") {
        return Err(denied("quote counter has an ambiguous installed global source"));
    }
    core.direct_body("quote_entry", &[
        "global.get quote_invocations", "i32.const 1", "i32.add",
        "global.set quote_invocations", "local.get 0", "call a", "if",
        "local.get 0", "call a", "call unbox", "else", "local.get 0",
        "call payload", "call push_i64", "end", "end",
    ], &[])?;
    core.direct_body("compose_entry", &[
        "local.get 0", "call b", "call enqueue_program",
        "local.get 0", "call a", "call enqueue_program", "end",
    ], &[])?;
    static_donor.direct_body("op_add", &[
        "i32.const 1", "call pop_kind", "local.set 0",
        "i32.const 1", "call pop_kind", "local.set 1",
        "local.get 1", "local.get 0", "i64.add", "call push_i64", "end",
    ], &[ValType::I64, ValType::I64])?;
    static_donor.signature(static_donor.index("dispatch")?, &[], &[])?;
    static_donor.direct_body("dispatch", &[
        "i32.const 57352", "i32.load 0", "if", "i32.const 4", "call fail",
        "return", "end", "i32.const 57352", "i32.const 1", "i32.store 0",
        "block", "loop", "global.get failure", "global.get cp", "i32.eqz",
        "i32.or", "br_if 1", "call runtime_tick", "i32.eqz", "if",
        "br 2", "end", "global.get cp", "i32.const 1", "i32.sub",
        "global.set cp", "i32.const 4096", "global.get cp", "i32.const 8",
        "i32.mul", "i32.add", "local.set 2", "local.get 2", "i32.load 0",
        "local.set 0", "local.get 2", "i32.load 4", "local.set 1",
        "i32.const 1048576", "global.get cp", "i32.const 4", "i32.mul",
        "i32.add", "i32.load 0", "local.set 3", "local.get 2",
        "i64.const 0", "i64.store 0", "i32.const 1048576",
        "global.get cp", "i32.const 4", "i32.mul", "i32.add",
        "i32.const 0", "i32.store 0", "local.get 3",
        "call live_frame_restore", "global.get failure", "br_if 1",
        "local.get 1", "local.get 0", "call_indirect entry",
        "local.get 1", "call storage_dec_ref", "drop", "br 0",
        "end", "end", "i32.const 57352", "i32.const 0",
        "i32.store 0", "end",
    ], &[ValType::I32, ValType::I32, ValType::I32, ValType::I32])?;
    // Do not infer that a checked helper is what the live table actually
    // enters: resolve every selected static index through its installed
    // passive element segment and check all three source operation wrappers.
    let code_segment = *static_donor.segments.get("code")
        .ok_or_else(|| denied("static native code element segment is missing"))?;
    let code = static_donor.elements.get(code_segment as usize)
        .ok_or_else(|| denied("static native code element functions are missing"))?;
    if add_entries.len() != 3 { return Err(denied("expected three selected add table entries")); }
    let mut static_table_roots = Vec::with_capacity(3);
    for (ordinal, entry) in add_entries.iter().enumerate() {
        let offset = entry.checked_sub(static_span_start)
            .ok_or_else(|| denied("selected add table entry precedes installed code span"))?;
        let index = *code.get(offset as usize)
            .ok_or_else(|| denied("selected add table entry has no installed element"))?;
        let name = format!("f{entry}");
        if static_donor.index(&name)? != index {
            return Err(denied("selected table function differs from installed source operation"));
        }
        static_donor.signature(index, &[ValType::I32], &[])?;
        let mut plan = vec!["global.get failure".to_owned(), "if".to_owned(),
            "return".to_owned(), "end".to_owned()];
        if ordinal < 2 {
            let next = add_entries[ordinal + 1];
            if *entry == u32::MAX || next != *entry + 1 {
                return Err(denied("selected add continuations are not sequential"));
            }
            plan.extend([format!("i32.const {next}"), "local.get 0".to_owned(),
                "call enqueue".to_owned()]);
        }
        plan.extend(["global.get failure".to_owned(), "i32.eqz".to_owned(),
            "if".to_owned(), "call op_add".to_owned(), "end".to_owned(), "end".to_owned()]);
        let expected = plan.iter().map(String::as_str).collect::<Vec<_>>();
        static_donor.direct_body(&name, &expected, &[])?;
        static_table_roots.push(index);
    }
    // Every transitive helper byte is tied to the exact installed module.
    // This witness does not prove that storage, stack, dispatcher or helpers
    // refine the pure Lean model; that must be discharged separately.
    Ok(json!({"schema":"noble-live-slot-native-byte-closure/v1",
        "native":core.closure(&["quote_entry","compose_entry"], &[entries[1],entries[2]])?,
        "static":static_donor.closure(&[
            "op_add","op_quote_typed","op_compose","dispatch"], &static_table_roots)?,
        "checked_direct_bodies":["quote_entry","compose_entry","op_add","dispatch",
            "selected_add_table_wrappers"],
        "semantic_refinement":"not-established"}))
}
