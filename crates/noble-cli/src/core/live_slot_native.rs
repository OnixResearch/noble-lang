//! Byte-derived, bounded native-body witness for the *selected installed* donors.
//! Exact direct-opcode checks and a complete reachable-body snapshot are not a
//! Wasm semantic refinement proof; proof-required admission remains closed.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use serde_json::{Value, json};
use wasmparser::{ElementItems, KnownCustom, Name, Operator, Parser, Payload, ValType, Validator};

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
                Operator::GlobalGet { global_index } => format!("global.get {global_index}"),
                Operator::GlobalSet { global_index } => format!("global.set {global_index}"),
                Operator::LocalGet { local_index } => format!("local.get {local_index}"),
                Operator::LocalSet { local_index } => format!("local.set {local_index}"),
                Operator::I32Const { value } => format!("i32.const {value}"),
                Operator::Call { function_index } => format!("call {}", self.names.get(&function_index)
                    .ok_or_else(|| denied("native direct call target has no installed name"))?),
                Operator::I32Add => "i32.add".to_owned(),
                Operator::I64Add => "i64.add".to_owned(),
                Operator::If { blockty: wasmparser::BlockType::Empty } => "if".to_owned(),
                Operator::Else => "else".to_owned(),
                Operator::End => "end".to_owned(),
                _ => return Err(denied("unexpected native direct-body opcode")),
            };
            actual.push(token);
            if actual.len() > 32 { return Err(denied("native direct body exceeds finite instruction bound")); }
        }
        // Global numbers are resolved against actual imported global names by
        // the original source/byte validator. Only the quote counter is read;
        // its identity is immaterial to the pure returned I64 execution law.
        let expected = expected.iter().map(|token| (*token).to_owned()).collect::<Vec<_>>();
        if actual != expected { return Err(denied("installed native operator body differs from exact reviewed opcode plan")); }
        Ok(())
    }

    fn closure(&self, roots: &[&str]) -> Result<Value, String> {
        let mut pending = VecDeque::new();
        for name in roots { pending.push_back(self.index(name)?); }
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
        Ok(json!({"module_sha256":self.sha256,"roots":roots,"functions":rows}))
    }
}

pub(super) fn attest(
    core_bytes: &[u8], core_sha: &str, static_bytes: &[u8], static_sha: &str,
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
    let counter = globals[0];
    core.direct_body("quote_entry", &[
        &format!("global.get {counter}"), "i32.const 1", "i32.add",
        &format!("global.set {counter}"), "local.get 0", "call a", "if",
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
    // Every transitive helper byte is tied to the exact installed module.
    // This witness does not prove that storage, stack, dispatcher or helpers
    // refine the pure Lean model; that must be discharged separately.
    Ok(json!({"schema":"noble-live-slot-native-byte-closure/v1",
        "native":core.closure(&["quote_entry","compose_entry"])?,
        "static":static_donor.closure(&[
            "op_add","op_quote_typed","op_compose","dispatch"])?,
        "checked_direct_bodies":["quote_entry","compose_entry","op_add"],
        "semantic_refinement":"not-established"}))
}
