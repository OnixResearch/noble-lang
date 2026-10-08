//! Executes only the original installed pure storage-read Wasm helpers over an
//! exact imported-memory snapshot. This is a concrete one-state observation,
//! not a proof of future writes, host imports, or full target refinement.
use super::{Module, denied};
use std::collections::BTreeMap;
use wasmparser::{Operator, ValType};

struct Function<'a> {
    ops: Vec<Operator<'a>>,
    locals: usize,
}

struct Machine<'a, 'm> {
    module: &'m Module<'a>,
    memory: &'m [u8],
    cursor: u32,
    functions: BTreeMap<u32, Function<'a>>,
}

fn refused() -> String { denied("unsupported or trapped installed storage-read instruction") }

fn pop(stack: &[u64; 16], top: &mut usize) -> Result<u64, String> {
    *top = top.checked_sub(1).ok_or_else(refused)?;
    Ok(stack[*top])
}

impl<'a, 'm> Machine<'a, 'm> {
    fn new(module: &'m Module<'a>, memory: &'m [u8], cursor: u32) -> Result<Self, String> {
        let mut functions = BTreeMap::new();
        for name in ["linear_address", "storage_is_live", "linear_live_address", "kind", "payload"] {
            let index = module.index(name)?;
            module.signature(index, &[ValType::I32], &[if name == "payload" {
                ValType::I64
            } else { ValType::I32 }])?;
            let body = module.body(index)?;
            let mut locals = 0usize;
            for row in body.get_locals_reader().map_err(|_| refused())? {
                let (count, ty) = row.map_err(|_| refused())?;
                if ty != ValType::I32 { return Err(refused()); }
                locals = locals.checked_add(count as usize).filter(|n| *n <= 1)
                    .ok_or_else(refused)?;
            }
            if locals != usize::from(name == "storage_is_live") { return Err(refused()); }
            let ops = body.get_operators_reader().map_err(|_| refused())?
                .collect::<Result<Vec<_>, _>>().map_err(|_| refused())?;
            if ops.is_empty() || ops.len() > 96 { return Err(refused()); }
            functions.insert(index, Function { ops, locals });
        }
        Ok(Self { module, memory, cursor, functions })
    }

    // The installed functions in this closed subset have only empty-result
    // `if/else/end` blocks; no loops, tables, writes, or imported functions.
    fn skip(ops: &[Operator<'_>], at: usize, stop_at_else: bool) -> Result<usize, String> {
        let mut nesting = 0usize;
        for (at, op) in ops.iter().enumerate().skip(at + 1) {
            match op {
                Operator::If { blockty: wasmparser::BlockType::Empty } => nesting += 1,
                Operator::End if nesting == 0 => return Ok(at + 1),
                Operator::End => nesting -= 1,
                Operator::Else if nesting == 0 && stop_at_else => return Ok(at + 1),
                Operator::Else => {},
                _ => {},
            }
        }
        Err(refused())
    }

    fn run(&self, index: u32, handle: u32, depth: u8) -> Result<u64, String> {
        if depth > 5 { return Err(refused()); }
        let function = self.functions.get(&index).ok_or_else(refused)?;
        let mut locals = [u64::from(handle), 0];
        let mut stack = [0u64; 16];
        let mut top = 0usize;
        let mut at = 0usize;
        while let Some(op) = function.ops.get(at) {
            let value = match op {
                Operator::LocalGet { local_index } => Some(*locals.get(*local_index as usize)
                    .filter(|_| *local_index as usize <= function.locals).ok_or_else(refused)?),
                Operator::LocalSet { local_index } => {
                    let value = pop(&stack, &mut top)?;
                    *locals.get_mut(*local_index as usize)
                        .filter(|_| *local_index as usize <= function.locals).ok_or_else(refused)? = value;
                    None
                }
                Operator::GlobalGet { global_index }
                    if self.module.globals.get(global_index).map(String::as_str) == Some("heap_cursor") => {
                    Some(u64::from(self.cursor))
                }
                Operator::I32Const { value } => Some(u64::from(*value as u32)),
                Operator::Call { function_index } => {
                    let arg = pop(&stack, &mut top)? as u32;
                    Some(self.run(*function_index, arg, depth + 1)?)
                }
                Operator::I32Load { memarg } if memarg.memory == 0 && memarg.align == 2 => {
                    let base = pop(&stack, &mut top)? as u32 as usize;
                    let address = usize::try_from(memarg.offset).ok()
                        .and_then(|offset| base.checked_add(offset))
                        .ok_or_else(refused)?;
                    let bytes: [u8; 4] = self.memory.get(address..address.checked_add(4).ok_or_else(refused)?)
                        .ok_or_else(refused)?.try_into().map_err(|_| refused())?;
                    Some(u64::from(u32::from_le_bytes(bytes)))
                }
                Operator::I64Load { memarg } if memarg.memory == 0 && memarg.align == 3 => {
                    let base = pop(&stack, &mut top)? as u32 as usize;
                    let address = usize::try_from(memarg.offset).ok()
                        .and_then(|offset| base.checked_add(offset))
                        .ok_or_else(refused)?;
                    let bytes: [u8; 8] = self.memory.get(address..address.checked_add(8).ok_or_else(refused)?)
                        .ok_or_else(refused)?.try_into().map_err(|_| refused())?;
                    Some(u64::from_le_bytes(bytes))
                }
                Operator::I32Add | Operator::I32Sub | Operator::I32Mul | Operator::I32And
                | Operator::I32Or | Operator::I32Eq | Operator::I32LtU | Operator::I32GeU
                | Operator::I32LeU => {
                    let right = pop(&stack, &mut top)? as u32;
                    let left = pop(&stack, &mut top)? as u32;
                    let result = match op {
                        Operator::I32Add => left.wrapping_add(right),
                        Operator::I32Sub => left.wrapping_sub(right),
                        Operator::I32Mul => left.wrapping_mul(right),
                        Operator::I32And => left & right,
                        Operator::I32Or => left | right,
                        Operator::I32Eq => u32::from(left == right),
                        Operator::I32LtU => u32::from(left < right),
                        Operator::I32GeU => u32::from(left >= right),
                        Operator::I32LeU => u32::from(left <= right),
                        _ => return Err(refused()),
                    };
                    Some(u64::from(result))
                }
                Operator::I32Eqz =>
                    Some(u64::from(u32::from(pop(&stack, &mut top)? as u32 == 0))),
                Operator::If { blockty: wasmparser::BlockType::Empty } => {
                    if pop(&stack, &mut top)? as u32 == 0 {
                        at = Self::skip(&function.ops, at, true)?;
                    }
                    else { at += 1; }
                    continue;
                }
                Operator::Else => { at = Self::skip(&function.ops, at, false)?; continue; }
                Operator::End if at == function.ops.len() - 1 => {
                    if top != 1 { return Err(refused()); }
                    return Ok(stack[0]);
                }
                Operator::End => None,
                Operator::Return => {
                    if top != 1 { return Err(refused()); }
                    return Ok(stack[0]);
                }
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

    fn named(&self, name: &str, handle: u32) -> Result<u64, String> {
        self.run(self.module.index(name)?, handle, 0)
    }
}

pub(super) fn probe(module: &Module<'_>, memory: &[u8], cursor: u32,
    handles: &[u32]) -> Result<(), String> {
    if handles.is_empty() || handles.len() > 32 || memory.len() != 20 * 65536 {
        return Err(refused());
    }
    let machine = Machine::new(module, memory, cursor)?;
    for handle in handles {
        let slot = handle & 4095;
        let address = 65536 + slot as usize * 48;
        let kind = memory.get(address).copied().ok_or_else(refused)?;
        let payload: [u8; 8] = memory.get(address + 8..address + 16)
            .ok_or_else(refused)?.try_into().map_err(|_| refused())?;
        if machine.named("storage_is_live", *handle)? != 1
            || machine.named("kind", *handle)? != u64::from(kind)
            || machine.named("payload", *handle)? != u64::from_le_bytes(payload) {
            return Err(denied("installed storage helper differs from selected live cell observation"));
        }
        // A nonwrapping future epoch using this same physical slot must not
        // alias the currently retained cell. This is a one-state negative,
        // not an allocator/retirement theorem.
        if let Some(stale) = handle.checked_add(4096) {
            if machine.named("storage_is_live", stale)? != 0 {
                return Err(denied("installed storage helper accepts a stale epoch handle"));
            }
        }
    }
    Ok(())
}
