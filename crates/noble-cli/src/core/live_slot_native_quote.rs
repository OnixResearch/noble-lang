//! A single, idle, direct scalar quote-entry step over decoded original Wasm.
//! This is not a dispatcher, a future-state invariant, or a Wasm refinement proof.
use super::{Module, denied, storage};
use serde_json::{Value as Json, json};
use wasmparser::{BlockType, Operator, ValType};

fn refused() -> String { denied("unsupported scalar quote instruction, effect or state") }

#[derive(Clone, Copy)]
enum Value { I32(u32), I64(u64) }

fn pop(stack: &[Value; 16], top: &mut usize) -> Result<Value, String> {
    *top = top.checked_sub(1).ok_or_else(refused)?;
    Ok(stack[*top])
}
fn pop32(stack: &[Value; 16], top: &mut usize) -> Result<u32, String> {
    match pop(stack, top)? { Value::I32(n) => Ok(n), Value::I64(_) => Err(refused()) }
}
fn pop64(stack: &[Value; 16], top: &mut usize) -> Result<u64, String> {
    match pop(stack, top)? { Value::I64(n) => Ok(n), Value::I32(_) => Err(refused()) }
}

struct Function<'a> {
    ops: Vec<Operator<'a>>,
    params: usize,
    locals: usize,
}

struct Runner<'a, 'm, 's> {
    module: &'m Module<'a>,
    storage: &'s storage::Machine<'a, 'm>,
    functions: [(u32, Option<Function<'a>>); 3],
    globals: [u32; 9],
    slot: [u8; 16],
    stores: u8,
}

impl<'a, 'm, 's> Runner<'a, 'm, 's> {
    fn decode(module: &Module<'a>, name: &str, params: &[ValType],
        locals: usize) -> Result<(u32, Option<Function<'a>>), String> {
        let index = module.index(name)?;
        module.signature(index, params, &[])?;
        let body = module.body(index)?;
        let mut count = 0usize;
        for row in body.get_locals_reader().map_err(|_| refused())? {
            let (n, ty) = row.map_err(|_| refused())?;
            if ty != ValType::I32 { return Err(refused()); }
            count = count.checked_add(n as usize).filter(|n| *n <= 2)
                .ok_or_else(refused)?;
        }
        if count != locals { return Err(refused()); }
        let ops = body.get_operators_reader().map_err(|_| refused())?
            .collect::<Result<Vec<_>, _>>().map_err(|_| refused())?;
        if ops.is_empty() || ops.len() > 96 { return Err(refused()); }
        Ok((index, Some(Function { ops, params:params.len(), locals })))
    }

    fn new(module: &'m Module<'a>, storage: &'s storage::Machine<'a, 'm>,
        memory: &[u8], imported: &Json, cursor: u32) -> Result<Self, String> {
        if memory.len() != 20 * 65536 || cursor == 0 || cursor > 4096 {
            return Err(refused());
        }
        let functions = [
            Self::decode(module, "quote_entry", &[ValType::I32], 0)?,
            Self::decode(module, "unbox", &[ValType::I32], 1)?,
            Self::decode(module, "push_tag", &[ValType::I32, ValType::I64], 2)?,
        ];
        let rows = imported.as_array().ok_or_else(refused)?;
        let mut globals = [0u32; 9];
        for (at, name) in ["failure", "sp", "cp", "rp", "live_frame", "heap_cursor",
            "operand_limit", "operand_peak", "quote_invocations"].iter().enumerate() {
            let mut installed = module.globals.values().filter(|found| found.as_str() == *name);
            if installed.next().is_none() || installed.next().is_some() {
                return Err(refused());
            }
            let mut matching = rows.iter().filter(|row| row["name"].as_str() == Some(*name));
            let row = matching.next().ok_or_else(refused)?;
            if matching.next().is_some() || row["type"] != "i32" {
                return Err(refused());
            }
            let value = row["value"].as_str().ok_or_else(refused)?;
            let parsed = value.parse::<i32>().map_err(|_| refused())?;
            if (parsed == 0 && value != "0") || value.starts_with('+')
                || value.trim_start_matches('-').starts_with('0') && parsed != 0 {
                return Err(refused());
            }
            globals[at] = parsed as u32;
        }
        for name in ["failure", "sp", "cp", "rp", "live_frame"] {
            if Self::get_global(&globals, name)? != 0 { return Err(refused()); }
        }
        if Self::get_global(&globals, "heap_cursor")? != cursor
            || Self::get_global(&globals, "operand_limit")? < 16
            || Self::get_global(&globals, "operand_limit")? > 2048
            || Self::get_global(&globals, "operand_peak")?
                > Self::get_global(&globals, "operand_limit")?
            || memory.get(57352..57356).ok_or_else(refused)? != [0u8; 4].as_slice() {
            return Err(refused());
        }
        let slot: [u8; 16] = memory[..16].try_into().map_err(|_| refused())?;
        Ok(Self { module, storage, functions, globals, slot, stores:0 })
    }

    fn global_slot(name: &str) -> Result<usize, String> {
        match name {
            "failure" => Ok(0), "sp" => Ok(1), "cp" => Ok(2),
            "rp" => Ok(3), "live_frame" => Ok(4), "heap_cursor" => Ok(5),
            "operand_limit" => Ok(6), "operand_peak" => Ok(7),
            "quote_invocations" => Ok(8), _ => Err(refused()),
        }
    }
    fn get_global(globals: &[u32; 9], name: &str) -> Result<u32, String> {
        Ok(globals[Self::global_slot(name)?])
    }
    fn global(&self, name: &str) -> Result<u32, String> {
        Self::get_global(&self.globals, name)
    }

    fn skip(ops: &[Operator<'_>], at: usize, stop_at_else: bool) -> Result<usize, String> {
        let mut nesting = 0usize;
        for (next, op) in ops.iter().enumerate().skip(at + 1) {
            match op {
                Operator::If { blockty: BlockType::Empty } => nesting += 1,
                Operator::End if nesting == 0 => return Ok(next + 1),
                Operator::End => nesting -= 1,
                Operator::Else if nesting == 0 && stop_at_else => return Ok(next + 1),
                _ => {},
            }
        }
        Err(refused())
    }

    fn run(&mut self, index: u32, args: [Value; 2], argc: usize, depth: u8) -> Result<(), String> {
        if depth > 3 { return Err(refused()); }
        // This selected path invokes each void helper once. Removing the
        // function also refuses recursive/repeated calls instead of silently
        // broadening the finite semantics to an unmodelled control path.
        let function = self.functions.iter_mut().find(|(at, _)| *at == index)
            .and_then(|(_, function)| function.take()).ok_or_else(refused)?;
        if function.params != argc { return Err(refused()); }
        let mut locals = [Value::I32(0); 4];
        locals[..argc].copy_from_slice(&args[..argc]);
        let mut stack = [Value::I32(0); 16];
        let mut top = 0usize;
        let mut at = 0usize;
        while let Some(op) = function.ops.get(at) {
            let value = match op {
                Operator::LocalGet { local_index } => Some(*locals.get(*local_index as usize)
                    .filter(|_| (*local_index as usize) < argc + function.locals)
                    .ok_or_else(refused)?),
                Operator::LocalSet { local_index } => {
                    let next = pop(&stack, &mut top)?;
                    *locals.get_mut(*local_index as usize)
                        .filter(|_| (*local_index as usize) < argc + function.locals)
                        .ok_or_else(refused)? = next;
                    None
                }
                Operator::GlobalGet { global_index } => {
                    let name = self.module.globals.get(global_index).ok_or_else(refused)?;
                    if !["failure", "sp", "operand_limit", "operand_peak", "quote_invocations"]
                        .contains(&name.as_str()) { return Err(refused()); }
                    Some(Value::I32(self.globals[Self::global_slot(name)?]))
                }
                Operator::GlobalSet { global_index } => {
                    let name = self.module.globals.get(global_index).ok_or_else(refused)?;
                    if !["sp", "operand_peak", "quote_invocations"].contains(&name.as_str()) {
                        return Err(refused());
                    }
                    let next = pop32(&stack, &mut top)?;
                    self.globals[Self::global_slot(name)?] = next;
                    None
                }
                Operator::I32Const { value } => Some(Value::I32(*value as u32)),
                Operator::I64Const { value } => Some(Value::I64(*value as u64)),
                Operator::Call { function_index } => {
                    let target: &'static str = match self.module.names.get(function_index)
                        .map(String::as_str).ok_or_else(refused)? {
                        "a" => "a", "kind" => "kind", "payload" => "payload",
                        "unbox" => "unbox", "push_tag" => "push_tag",
                        _ => return Err(refused()),
                    };
                    if self.module.index(target)? != *function_index { return Err(refused()); }
                    match target {
                        "a" | "kind" | "payload" => {
                            let handle = pop32(&stack, &mut top)?;
                            let result = self.storage.named(target, handle)?;
                            Some(if target == "payload" { Value::I64(result) }
                                else { Value::I32(result as u32) })
                        }
                        "unbox" => {
                            let handle = pop32(&stack, &mut top)?;
                            self.run(*function_index, [Value::I32(handle); 2], 1, depth + 1)?;
                            None
                        }
                        "push_tag" => {
                            let payload = pop64(&stack, &mut top)?;
                            let tag = pop32(&stack, &mut top)?;
                            self.run(*function_index,
                                [Value::I32(tag), Value::I64(payload)], 2, depth + 1)?;
                            None
                        }
                        // Refcount mutation, quota, imports, continuation or any
                        // other call is outside the closed scalar transition.
                        _ => return Err(refused()),
                    }
                }
                Operator::I32Add | Operator::I32Mul | Operator::I32GtU
                | Operator::I32GeU | Operator::I32LtU => {
                    let right = pop32(&stack, &mut top)?;
                    let left = pop32(&stack, &mut top)?;
                    let result = match op {
                        Operator::I32Add => left.wrapping_add(right),
                        Operator::I32Mul => left.wrapping_mul(right),
                        Operator::I32GtU => u32::from(left > right),
                        Operator::I32GeU => u32::from(left >= right),
                        Operator::I32LtU => u32::from(left < right),
                        _ => return Err(refused()),
                    };
                    Some(Value::I32(result))
                }
                Operator::I32Eqz => Some(Value::I32(u32::from(pop32(&stack, &mut top)? == 0))),
                Operator::I32Store { memarg }
                    if memarg.memory == 0 && memarg.align == 2 && memarg.offset == 0 => {
                    let word = pop32(&stack, &mut top)?;
                    if pop32(&stack, &mut top)? != 0 { return Err(refused()); }
                    self.slot[..4].copy_from_slice(&word.to_le_bytes());
                    self.stores = self.stores.checked_add(1).ok_or_else(refused)?;
                    None
                }
                Operator::I64Store { memarg }
                    if memarg.memory == 0 && memarg.align == 3 && memarg.offset == 8 => {
                    let word = pop64(&stack, &mut top)?;
                    if pop32(&stack, &mut top)? != 0 { return Err(refused()); }
                    self.slot[8..16].copy_from_slice(&word.to_le_bytes());
                    self.stores = self.stores.checked_add(1).ok_or_else(refused)?;
                    None
                }
                Operator::If { blockty: BlockType::Empty } => {
                    at = if pop32(&stack, &mut top)? == 0 {
                        Self::skip(&function.ops, at, true)?
                    } else { at + 1 };
                    continue;
                }
                Operator::Else => { at = Self::skip(&function.ops, at, false)?; continue; }
                Operator::End if at == function.ops.len() - 1 => {
                    return if top == 0 { Ok(()) } else { Err(refused()) };
                }
                Operator::End => None,
                Operator::Return => return if top == 0 { Ok(()) } else { Err(refused()) },
                Operator::Unreachable => return Err(refused()),
                _ => return Err(refused()),
            };
            if let Some(value) = value {
                if top == stack.len() { return Err(refused()); }
                stack[top] = value;
                top += 1;
            }
            at += 1;
        }
        Err(refused())
    }
}

/// Execute one selected, source-bound captured-I64 quote directly, not the
/// continuation dispatcher or the complete returned composite.
pub(super) fn probe<'a, 'm>(module: &'m Module<'a>, storage: &storage::Machine<'a, 'm>,
    memory: &[u8], cursor: u32, globals: &Json, environment: u32,
    captured: i64, memory_sha256: &str) -> Result<Json, String> {
    let segment = *module.segments.get("core").ok_or_else(refused)?;
    let entries = module.elements.get(segment as usize).ok_or_else(refused)?;
    if entries.len() != 4 || entries[1] != module.index("quote_entry")? {
        return Err(refused());
    }
    if storage.named("kind", environment)? != 4 || storage.named("x", environment)? != 1 {
        return Err(refused());
    }
    let child = storage.named("a", environment)? as u32;
    if child == 0 || storage.named("kind", child)? != 1
        || storage.named("payload", child)? != captured as u64 {
        return Err(refused());
    }
    let mut runner = Runner::new(module, storage, memory, globals, cursor)?;
    let before = runner.slot;
    let peak = runner.global("operand_peak")?;
    let calls = runner.global("quote_invocations")?;
    runner.run(entries[1], [Value::I32(environment); 2], 1, 0)?;
    let mut expected = before;
    expected[..4].copy_from_slice(&1u32.to_le_bytes());
    expected[8..16].copy_from_slice(&(captured as u64).to_le_bytes());
    if runner.slot != expected || runner.stores != 2 || runner.global("sp")? != 1
        || runner.global("operand_peak")? != peak.max(16)
        || runner.global("quote_invocations")? != calls.wrapping_add(1) {
        return Err(denied("installed scalar quote differs from captured stack transition"));
    }
    // The interpreter admits only writes to bytes 0..3 and 8..15 and the
    // three named globals. No heap/sidecar/refcount write can be silently
    // abstracted as a no-op; any such instruction or helper is refused.
    Ok(json!({"schema":"noble-live-slot-installed-direct-scalar-quote/v1",
        "scope":"one captured I64 quote-entry step only; no dispatch or full composite proof",
        "module_sha256":module.sha256,"memory_sha256":memory_sha256,
        "environment":environment,"capture":child,"literal":captured.to_string(),
        "initial_sp":0,"final_sp":1,"before_slot":before,"after_slot":runner.slot,
        "initial_operand_peak":peak,"final_operand_peak":runner.global("operand_peak")?,
        "initial_quote_invocations":(calls as i32).to_string(),
        "final_quote_invocations":(runner.global("quote_invocations")? as i32).to_string(),
        "heap_and_refcount_writes":0,
        "refcount_sidecar_sha256":crate::workflow::intrinsic::sha256(
            &memory[1052672..1077248]),
        "wasm_lean_c_refinement":"not-established"}))
}
