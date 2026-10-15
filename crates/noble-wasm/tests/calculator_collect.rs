//! Calculator-v1 boundary compaction against real compiled calculator modules.
//! Each phase is a separately instantiated module importing one shared memory,
//! table and global set, as the worker engine does.

use noble_kernel::types::Ty;
use wasmtime::{
    Engine, Global, GlobalType, Instance, Linker, Memory, MemoryType, Module, Mutability, Ref,
    RefType, Store, Table, TableType, Val, ValType,
};

const ALLOCATION_BYTES: i32 = 1_376_208;
const TEXT_INGRESS: usize = 983_040;
const CELL_LIMIT: i32 = 72_362;
const GLOBALS: [(&str, i64); 25] = [
    ("failure", 0),
    ("quota_reason", 0),
    ("phase", 0),
    ("heap_cursor", 0),
    ("heap_baseline", 0),
    ("allocation_limit", 196_608),
    ("recipe_limit", 256),
    ("depth_limit", 256),
    ("operand_limit", 2048),
    ("continuation_limit", 4096),
    ("step_limit", 100_000),
    ("allocation_peak", 0),
    ("operand_peak", 0),
    ("continuation_peak", 0),
    ("steps", 0),
    ("quote_invocations", 0),
    ("sp", 0),
    ("cp", 0),
    ("observed_program", 0),
    ("recipe_count", 0),
    ("reflection_steps", 0),
    ("rp", 0),
    ("generation", 0),
    ("allocated_total", 0),
    ("released_total", 0),
];
/// Allocates 4 cells per round (boxed head, cons, boxed counter, pair).
const CHURN: &str = "nil 0 pair [ unpair dup [ swap ] dip [ cons ] dip 1 + \
    dup 2000 i64.cmp -1 = [ pair ] dip ] iterate drop";

fn limits() -> noble_contracts::Limits {
    noble_contracts::Limits {
        bytes: 65_536,
        nodes: 65_536,
        depth: 64,
        work: 200_000_000,
    }
}

struct Arena {
    store: Store<()>,
    linker: Linker<()>,
    memory: Memory,
    globals: Vec<(&'static str, Global)>,
    session: noble_contracts::source::Session,
    compiler: noble_wasm::source::Compiler,
    stack: Vec<Ty>,
    collect: bool,
    /// Follow the runtime's post-submission `compiled_programs_reachable`
    /// answer, as the calculator host does, when choosing the next preparation.
    reclaim: bool,
    quiescent: bool,
    /// The stricter `programs_reachable` answer was 0: no Program, Syntax or
    /// recipe cell is reachable, so the next preparation restarts signatures.
    program_free: bool,
    last: Option<Instance>,
}

struct Phase {
    status: i32,
    quota: i64,
    before: i32,
    after: i32,
    /// `(first slot, slot count)` of this module's compiled-code table init.
    table: Option<(u32, u32)>,
}

impl Arena {
    fn new(collect: bool) -> Result<Self, String> {
        let engine = Engine::default();
        let mut store = Store::new(&engine, ());
        let memory =
            Memory::new(&mut store, MemoryType::new(16, Some(64))).map_err(|e| e.to_string())?;
        let table = Table::new(
            &mut store,
            TableType::new(RefType::FUNCREF, 16_384, Some(16_384)),
            Ref::Func(None),
        )
        .map_err(|e| e.to_string())?;
        let mut linker = Linker::new(&engine);
        linker
            .define(&mut store, "noble", "memory", memory)
            .map_err(|e| e.to_string())?;
        linker
            .define(&mut store, "noble", "table", table)
            .map_err(|e| e.to_string())?;
        let mut globals = Vec::new();
        for (name, value) in GLOBALS {
            let wide = name == "allocated_total" || name == "released_total";
            let (ty, initial) = if wide {
                (ValType::I64, Val::I64(value))
            } else {
                (ValType::I32, Val::I32(value as i32))
            };
            let global = Global::new(&mut store, GlobalType::new(ty, Mutability::Var), initial)
                .map_err(|e| e.to_string())?;
            linker
                .define(&mut store, "noble", name, global)
                .map_err(|e| e.to_string())?;
            globals.push((name, global));
        }
        Ok(Self {
            store,
            linker,
            memory,
            globals,
            session: noble_contracts::source::Session::new_calculator(),
            compiler: noble_wasm::source::Compiler::new_calculator(),
            stack: Vec::new(),
            collect,
            reclaim: false,
            quiescent: false,
            program_free: false,
            last: None,
        })
    }

    fn global(&mut self, name: &str) -> i32 {
        let global = self
            .globals
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, g)| *g)
            .unwrap_or_else(|| panic!("missing global {name}"));
        global
            .get(&mut self.store)
            .i32()
            .unwrap_or_else(|| panic!("{name} is not i32"))
    }

    fn set_global(&mut self, name: &str, value: i32) {
        let global = self
            .globals
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, g)| *g)
            .unwrap_or_else(|| panic!("missing global {name}"));
        global
            .set(&mut self.store, Val::I32(value))
            .unwrap_or_else(|_| panic!("set {name}"));
    }

    fn call0(&mut self, name: &str) -> Result<i32, String> {
        let instance = self.last.ok_or("no instance")?;
        instance
            .get_typed_func::<(), i32>(&mut self.store, name)
            .map_err(|e| e.to_string())?
            .call(&mut self.store, ())
            .map_err(|e| e.to_string())
    }

    fn call1(&mut self, name: &str, argument: i32) -> Result<i32, String> {
        let instance = self.last.ok_or("no instance")?;
        instance
            .get_typed_func::<i32, i32>(&mut self.store, name)
            .map_err(|e| e.to_string())?
            .call(&mut self.store, argument)
            .map_err(|e| e.to_string())
    }

    fn call1_wide(&mut self, name: &str, argument: i32) -> Result<i64, String> {
        let instance = self.last.ok_or("no instance")?;
        instance
            .get_typed_func::<i32, i64>(&mut self.store, name)
            .map_err(|e| e.to_string())?
            .call(&mut self.store, argument)
            .map_err(|e| e.to_string())
    }

    /// One calculator phase: compile, instantiate, (collect,) begin, inject, submit.
    fn phase(&mut self, source: &str, text: Option<&[u8]>) -> Result<Phase, String> {
        let mut inputs = self.stack.clone();
        if text.is_some() {
            inputs.push(Ty::Text);
        }
        let checked = self
            .session
            .prepare(source.as_bytes(), &inputs, limits())
            .map_err(|error| format!("{source}: {error:?}"))?;
        let output = checked.output().to_vec();
        let submission = checked.submission().ok_or("no submission")?;
        let prepared = if self.reclaim && self.program_free {
            self.compiler.prepare_program_free(submission)
        } else if self.reclaim && self.quiescent {
            self.compiler.prepare_quiescent(submission)
        } else {
            self.compiler.prepare(submission)
        }
        .map_err(|_| format!("{source}: calculator compiler refused"))?;
        let table = table_span(prepared.wat());
        let wasm = wat::parse_bytes(prepared.wat()).map_err(|e| e.to_string())?;
        let module = Module::new(self.store.engine(), wasm.as_ref()).map_err(|e| e.to_string())?;
        let instance = self
            .linker
            .instantiate(&mut self.store, &module)
            .map_err(|e| e.to_string())?;
        self.last = Some(instance);
        let before = self.global("heap_cursor");
        let mut status = 0;
        if self.collect {
            status = self.call0("collect")?;
        }
        let after = self.global("heap_cursor");
        if status == 0 {
            status = instance
                .get_typed_func::<(i32, i32, i32, i32, i32, i32), i32>(&mut self.store, "begin")
                .map_err(|e| e.to_string())?
                .call(
                    &mut self.store,
                    (ALLOCATION_BYTES, 256, 256, 2048, 4096, 100_000),
                )
                .map_err(|e| e.to_string())?;
        }
        if status == 0 {
            if let Some(bytes) = text {
                self.memory.data_mut(&mut self.store)[TEXT_INGRESS..TEXT_INGRESS + bytes.len()]
                    .copy_from_slice(bytes);
                status = instance
                    .get_typed_func::<(i32, i32), i32>(&mut self.store, "push_text")
                    .map_err(|e| e.to_string())?
                    .call(&mut self.store, (TEXT_INGRESS as i32, bytes.len() as i32))
                    .map_err(|e| e.to_string())?;
            }
        }
        if status == 0 {
            status = self.call0("submit")?;
        }
        let quota = self.call1_wide("metric", 1)?;
        if status == 0 {
            let quiescent = self.reclaim && self.call0("compiled_programs_reachable")? == 0;
            let program_free = quiescent && self.call0("programs_reachable")? == 0;
            if !self.compiler.can_commit(&prepared) {
                return Err(format!("{source}: compiler cannot commit"));
            }
            self.compiler
                .commit(prepared)
                .map_err(|_| format!("{source}: compiler commit refused"))?;
            self.session.commit(checked).map_err(|e| format!("{e:?}"))?;
            self.stack = output;
            self.quiescent = quiescent;
            self.program_free = program_free;
        }
        Ok(Phase {
            status,
            quota,
            before,
            after,
            table,
        })
    }

    fn normal(&mut self, source: &str) -> Result<Phase, String> {
        let phase = self.phase(source, None)?;
        if phase.status != 0 {
            return Err(format!(
                "{source}: status {} quota {}",
                phase.status, phase.quota
            ));
        }
        Ok(phase)
    }

    /// Commit a source definition; it compiles nothing until a phase calls it.
    fn define(&mut self, source: &str) -> Result<(), String> {
        let checked = self
            .session
            .prepare(source.as_bytes(), &self.stack, limits())
            .map_err(|error| format!("{source}: {error:?}"))?;
        self.session.commit(checked).map_err(|e| format!("{e:?}"))
    }

    /// Prepare and commit one stack-preserving phase in the compiler alone,
    /// as a session-wide cursor does, returning its compiled-code table span.
    fn compile_only(&mut self, source: &str) -> Result<Option<(u32, u32)>, noble_wasm::Diagnostic> {
        let checked = self
            .session
            .prepare(source.as_bytes(), &self.stack, limits())
            .unwrap_or_else(|error| panic!("{source}: {error:?}"));
        assert_eq!(checked.output(), self.stack.as_slice());
        let prepared = self.compiler.prepare(
            checked
                .submission()
                .unwrap_or_else(|| panic!("{source}: no submission")),
        )?;
        let table = table_span(prepared.wat());
        self.compiler.commit(prepared)?;
        self.session
            .commit(checked)
            .unwrap_or_else(|error| panic!("{error:?}"));
        Ok(table)
    }

    /// Handle-free structural fingerprint of every operand slot.
    fn snapshot(&mut self) -> Result<Vec<String>, String> {
        let height = self.call0("stack_length")?;
        let mut slots = Vec::new();
        for index in 0..height {
            let kind = self.call1("stack_kind", index)?;
            let value = self.call1_wide("stack_value", index)?;
            slots.push(if kind < 4 {
                format!("{kind}:{value}")
            } else {
                self.cell(value as i32, 0)?
            });
        }
        Ok(slots)
    }

    fn cell(&mut self, handle: i32, depth: u32) -> Result<String, String> {
        if handle == 0 {
            return Ok("_".into());
        }
        if depth > 4096 {
            return Err("cell graph too deep".into());
        }
        let kind = self.call1("cell_kind", handle)?;
        let payload = self.call1_wide("cell_payload", handle)?;
        let fields = ["cell_x", "cell_y", "cell_z", "cell_w", "cell_n"]
            .iter()
            .map(|name| self.call1(name, handle))
            .collect::<Result<Vec<_>, _>>()?;
        let mut text = String::new();
        if kind == 11 {
            let (start, length) = (fields[0] as usize, fields[1] as usize);
            text = String::from_utf8_lossy(&self.memory.data(&self.store)[start..start + length])
                .into_owned();
        }
        let a = self.call1("cell_a", handle)?;
        let b = self.call1("cell_b", handle)?;
        let c = self.call1("cell_c", handle)?;
        // Dynamic Text addresses move with compaction; only their bytes are identity.
        let shown = if kind == 11 { vec![fields[1]] } else { fields };
        Ok(format!(
            "({kind} {payload} {shown:?} {text:?} {} {} {})",
            self.cell(a, depth + 1)?,
            self.cell(b, depth + 1)?,
            self.cell(c, depth + 1)?
        ))
    }

    fn raw_cell(&mut self, handle: i32) -> Vec<u8> {
        let address = cell_address(handle);
        self.memory.data(&self.store)[address..address + 48].to_vec()
    }
}

fn cell_address(handle: i32) -> usize {
    match handle {
        1..=4096 => 65_536 + (handle as usize - 1) * 48,
        4097..=6826 => 393_216 + (handle as usize - 4097) * 48,
        _ => 1_048_576 + (handle as usize - 6827) * 48,
    }
}

/// The `(table.init $code (i32.const first) (i32.const 0) (i32.const count))`
/// operands of one prepared calculator module, if it allocates compiled code.
fn table_span(wat: &[u8]) -> Option<(u32, u32)> {
    let head = b"(table.init $code (i32.const ";
    let start = wat.windows(head.len()).position(|window| window == head)? + head.len();
    let text = std::str::from_utf8(&wat[start..(start + 64).min(wat.len())]).ok()?;
    let (first, rest) = text.split_once(')')?;
    let rest = rest.strip_prefix(" (i32.const 0) (i32.const ")?;
    let (count, _) = rest.split_once(')')?;
    Some((first.parse().ok()?, count.parse().ok()?))
}

/// Defines 16 distinct words of 60 dropped quotations each (240 recipe leaves,
/// under the 256 per-Program bound) and returns a phase calling all of them.
/// Every module re-lowers that closure: about a thousand compiled-code slots,
/// all dead once the phase returns.
fn bursts(arena: &mut Arena) -> Result<String, String> {
    let mut calls = String::new();
    for word in 0..16 {
        arena.define(&format!(
            "def burst{word} [ {}]",
            format!("[ {} + ] drop ", word + 3).repeat(60)
        ))?;
        calls.push_str(&format!("burst{word} "));
    }
    Ok(calls)
}

/// Two cells per round (boxed counter, pair) after a fixed per-phase overhead.
fn counted(rounds: i32) -> String {
    format!("nil 0 pair [ unpair 1 + dup {rounds} i64.cmp -1 = [ pair ] dip ] iterate drop")
}

/// `nil drop` costs exactly one cell more than `1 drop`; words cost cells too.
fn filler(numbers: i32, nils: i32) -> String {
    format!(
        "{}{}",
        "1 drop ".repeat(numbers as usize),
        "nil drop ".repeat(nils as usize)
    )
}

/// The live calculator state shapes, then enough churn to overrun 72,362 cells.
fn live_values(arena: &mut Arena) -> Result<(), String> {
    // `run` consumes exactly its Program's input stack, so the capture stays at the bottom.
    arena.normal("7 8 pair quote")?;
    arena.normal("nil 3 swap cons 2 swap cons 1 swap cons 40 pair 50 pair")?;
    let injected = arena.phase("swap swap", Some("héllo, arena".as_bytes()))?;
    if injected.status != 0 {
        return Err(format!("text injection status {}", injected.status));
    }
    arena.normal("nil 120 swap cons text.from-ascii drop drop")?;
    arena.normal("nil text.from-ascii drop")?;
    arena.normal("nil 200 swap cons text.from-ascii drop")?;
    Ok(())
}

#[test]
fn calculator_arena_without_collection_exhausts_on_garbage() -> Result<(), String> {
    let mut arena = Arena::new(false)?;
    live_values(&mut arena)?;
    for round in 0..12 {
        let phase = arena.phase(CHURN, None)?;
        if phase.status != 0 {
            assert_eq!(
                (phase.status, phase.quota),
                (2, 1),
                "round {round}: runtime allocation quota"
            );
            assert_eq!(
                arena.global("heap_cursor"),
                72_362,
                "the persistent bump cursor reaches the session cell limit"
            );
            // A poisoned arena never compacts or moves anything.
            assert_eq!(arena.call0("collect").ok(), Some(2));
            assert_eq!(arena.global("heap_cursor"), 72_362);
            return Ok(());
        }
    }
    Err("twelve garbage rounds fit without compaction; the regression is vacuous".into())
}

#[test]
fn calculator_collection_preserves_live_values_across_garbage() -> Result<(), String> {
    let mut arena = Arena::new(true)?;
    let first = arena.normal("unit drop")?;
    assert_eq!(
        (first.before, first.after),
        (0, 0),
        "empty stack, empty arena"
    );
    live_values(&mut arena)?;
    let reference = arena.snapshot()?;
    assert_eq!(reference.len(), 5);
    let mut stale = 0;
    let mut live = 0;
    for round in 0..12 {
        // The value built after churn lands above handle 6826 and must slide
        // into the first region on the next boundary.
        let source = if round == 3 {
            format!("{CHURN} 11 22 pair nil 33 swap cons pair nil 105 swap cons 104 swap cons text.from-ascii drop")
        } else {
            CHURN.to_owned()
        };
        let phase = arena.normal(&source)?;
        assert!(phase.after <= phase.before);
        if round == 4 {
            assert!(phase.before > 6827, "churn crossed every region boundary");
            live = phase.after;
        }
        if round == 3 {
            stale = arena.call1_wide("stack_value", 5)? as i32;
            assert!(
                stale > 6827,
                "late value was allocated in the growable region"
            );
        }
        if round > 4 {
            assert_eq!(
                phase.after, live,
                "live set is stable across garbage rounds"
            );
        }
        let snapshot = arena.snapshot()?;
        assert_eq!(
            snapshot[..5],
            reference[..],
            "round {round}: live values decode identically"
        );
    }
    let late = arena.snapshot()?;
    assert!(late[5].contains("(1 11") && late[5].contains("(1 33"));
    assert!(late[6].contains("\"hi\""));

    // Idempotent: a second boundary right after the first changes nothing.
    assert_eq!(arena.call0("collect")?, 0);
    let cursor = arena.global("heap_cursor");
    assert_eq!(cursor, live);
    assert_eq!(arena.call0("collect")?, 0);
    assert_eq!(arena.global("heap_cursor"), cursor);
    assert_eq!(arena.snapshot()?, late);

    // The stale pre-compaction handle is beyond the cursor and its cell was zeroed.
    assert!(stale > cursor);
    assert!(
        arena.call1("cell_kind", stale).is_err(),
        "stale handle must fail liveness"
    );
    assert!(arena.raw_cell(stale).iter().all(|byte| *byte == 0));
    assert!(arena.raw_cell(cursor + 1).iter().all(|byte| *byte == 0));

    // Live values still execute: the capture Program yields its exact pair.
    arena.normal("drop drop drop drop drop drop run unpair +")?;
    assert_eq!(arena.snapshot()?, ["1:15"]);

    // Dropping every root reclaims the whole arena and zeroes it.
    arena.normal("drop")?;
    let cursor = arena.global("heap_cursor");
    assert_eq!(arena.call0("collect")?, 0);
    assert_eq!(arena.global("heap_cursor"), 0);
    assert!(arena.raw_cell(1).iter().all(|byte| *byte == 0));
    assert!(arena.raw_cell(cursor).iter().all(|byte| *byte == 0));
    Ok(())
}

#[test]
fn calculator_collection_keeps_compiled_program_handles_in_place() -> Result<(), String> {
    let mut arena = Arena::new(true)?;
    arena.normal("[ 1 + ]")?;
    arena.normal(CHURN)?;
    let pinned = arena.normal(CHURN)?;
    // A compiled body reads its callee/quotation handles from instance globals,
    // so the boundary refuses to move anything while such a Program is live.
    assert_eq!(pinned.after, pinned.before);
    assert!(pinned.before > 8000);
    arena.normal("5 swap run")?;
    assert_eq!(arena.snapshot()?.last().map(String::as_str), Some("1:6"));
    arena.normal("drop")?;
    assert_eq!(arena.normal("unit drop")?.after, 0);
    Ok(())
}

#[test]
fn calculator_collection_fails_closed_on_malformed_boundaries() -> Result<(), String> {
    let mut arena = Arena::new(true)?;
    arena.normal("1 2 pair")?;
    arena.normal(CHURN)?;
    let cursor = arena.global("heap_cursor");
    arena.set_global("cp", 1);
    assert_eq!(
        arena.call0("collect")?,
        4,
        "live continuation refuses compaction"
    );
    assert_eq!(arena.global("heap_cursor"), cursor);
    assert_eq!(arena.global("failure"), 4);

    let mut forged = Arena::new(true)?;
    forged.normal("1 2 pair")?;
    let cursor = forged.global("heap_cursor");
    let slot = 8usize;
    forged.memory.data_mut(&mut forged.store)[slot..slot + 8]
        .copy_from_slice(&i64::from(cursor + 1).to_le_bytes());
    assert_eq!(
        forged.call0("collect")?,
        4,
        "a root beyond the cursor is not a live cell"
    );
    assert_eq!(forged.global("heap_cursor"), cursor);
    Ok(())
}

#[test]
fn calculator_collection_reclaims_an_arena_filled_exactly_to_the_cell_limit() -> Result<(), String>
{
    let mut compact = Arena::new(true)?;
    live_values(&mut compact)?;
    let live = compact.normal("unit drop")?.after;

    let mut arena = Arena::new(false)?;
    live_values(&mut arena)?;
    let reference = arena.snapshot()?;
    assert!(
        arena.global("heap_cursor") > live,
        "the live values left garbage behind"
    );
    // Calibrate whole-phase allocation counts on this arena.
    fn delta(arena: &mut Arena, cursor: &mut i32, source: &str) -> Result<i32, String> {
        arena.normal(source)?;
        let next = arena.global("heap_cursor");
        let step = next - *cursor;
        *cursor = next;
        Ok(step)
    }
    let mut cursor = arena.global("heap_cursor");
    let number = delta(&mut arena, &mut cursor, &filler(1, 0))?;
    let nil = delta(&mut arena, &mut cursor, &filler(0, 1))?;
    assert!(
        number > 0 && nil == number + 1,
        "`nil` allocates exactly one more cell than `1`"
    );
    assert_eq!(
        delta(&mut arena, &mut cursor, &filler(2, 3))?,
        2 * number + 3 * nil,
        "additive"
    );
    // Every count >= number * nil is a non-negative mix of both phrases.
    let reserve = number * nil;
    let ten = delta(&mut arena, &mut cursor, &counted(10))?;
    let twenty = delta(&mut arena, &mut cursor, &counted(20))?;
    let slope = (twenty - ten) / 10;
    let fixed = ten - 10 * slope;
    assert!(
        slope > 0 && fixed >= 0 && twenty == fixed + 20 * slope,
        "linear calibration"
    );
    loop {
        let rounds = ((CELL_LIMIT - cursor - fixed - reserve) / slope).min(3000);
        if rounds < 20 {
            break;
        }
        assert_eq!(
            delta(&mut arena, &mut cursor, &counted(rounds))?,
            fixed + rounds * slope
        );
    }
    let tail = CELL_LIMIT - cursor;
    let nils = tail % number;
    let numbers = tail / number - nils;
    assert!(tail >= reserve && numbers >= 0);
    assert_eq!(
        delta(&mut arena, &mut cursor, &filler(numbers, nils))?,
        tail
    );
    assert_eq!(
        arena.global("heap_cursor"),
        CELL_LIMIT,
        "every cell is in use"
    );
    assert_eq!(
        arena.global("failure"),
        0,
        "filling exactly to the limit is not a failure"
    );

    arena.collect = true;
    let boundary = arena.normal("unit drop")?;
    assert_eq!(boundary.before, CELL_LIMIT);
    assert_eq!(
        boundary.after, live,
        "exactly the live values survive a full arena"
    );
    assert_eq!(arena.snapshot()?, reference);
    assert!(arena.raw_cell(CELL_LIMIT).iter().all(|byte| *byte == 0));
    // The boundary phase's own word cells follow the live set; the rest is zero.
    let cursor = arena.global("heap_cursor");
    assert!(cursor < CELL_LIMIT / 2);
    assert!(arena.raw_cell(cursor + 1).iter().all(|byte| *byte == 0));
    // The reclaimed arena allocates again.
    arena.normal(CHURN)?;
    assert_eq!(arena.snapshot()?, reference);

    // One cell past the limit is not a valid arena.
    arena.set_global("heap_cursor", CELL_LIMIT + 1);
    assert_eq!(arena.call0("collect")?, 4);
    assert_eq!(arena.global("heap_cursor"), CELL_LIMIT + 1);
    Ok(())
}

#[test]
fn calculator_collection_fails_closed_on_non_monotone_dynamic_text() -> Result<(), String> {
    let mut arena = Arena::new(true)?;
    arena.normal("nil 97 swap cons text.from-ascii drop")?;
    arena.normal("nil 98 swap cons text.from-ascii drop")?;
    let low = arena.call1_wide("stack_value", 0)? as i32;
    let high = arena.call1_wide("stack_value", 1)? as i32;
    assert!(low < high);
    let x = |handle: i32| cell_address(handle) + 28;
    let low_x = u32::from_le_bytes(
        arena.memory.data(&arena.store)[x(low)..x(low) + 4]
            .try_into()
            .map_err(|_| "x")?,
    );
    let high_x = u32::from_le_bytes(
        arena.memory.data(&arena.store)[x(high)..x(high) + 4]
            .try_into()
            .map_err(|_| "x")?,
    );
    assert!(
        low_x < high_x && (262_144..393_216).contains(&low_x),
        "dynamic Text follows handle order"
    );
    // Invert the extents: the higher handle now names the lower Text.
    let data = arena.memory.data_mut(&mut arena.store);
    data[x(low)..x(low) + 4].copy_from_slice(&high_x.to_le_bytes());
    data[x(high)..x(high) + 4].copy_from_slice(&low_x.to_le_bytes());
    let cursor = arena.global("heap_cursor");
    let text_end = arena.memory.data(&arena.store)[65_528..65_532].to_vec();
    assert_eq!(
        arena.call0("collect")?,
        4,
        "non-monotone dynamic Text cannot slide"
    );
    assert_eq!(arena.global("failure"), 4);
    assert_eq!(arena.global("heap_cursor"), cursor);
    assert_eq!(
        arena.memory.data(&arena.store)[65_528..65_532],
        text_end[..]
    );
    assert_eq!(arena.call0("collect")?, 4, "the session stays terminated");
    Ok(())
}

#[test]
fn calculator_quiescent_boundaries_reuse_compiled_table_slots() -> Result<(), String> {
    // Control: a session-wide cursor never reuses a slot, so the same dead
    // bursts exhaust the 16,384-entry shared table. Exhaustion is a compiler
    // refusal, so this cursor needs no execution.
    let mut control = Arena::new(true)?;
    let source = bursts(&mut control)?;
    let mut span = 0;
    let mut refused = None;
    for round in 0..64u32 {
        match control.compile_only(&source) {
            Ok(table) => {
                let (first, count) = table.ok_or("burst allocated no compiled code")?;
                assert!(count > 16 * 60, "every quotation owns a slot");
                assert_eq!(first, 5 + round * count, "round {round}: cumulative cursor");
                span = count;
            }
            Err(error) => {
                assert!(
                    error == noble_wasm::Diagnostic::Exhausted,
                    "round {round}: not Exhausted"
                );
                assert!(
                    5 + (round + 1) * span > 16_384,
                    "refused only past the table limit"
                );
                refused = Some(round);
                break;
            }
        }
    }
    let refused = refused.ok_or("64 bursts fit without reuse; the regression is vacuous")?;

    // Each burst leaves no Program on the stack. Following the runtime's
    // boundary answer, every module restarts at the base and overwrites only
    // dead slots, past the point where the control was refused.
    let mut arena = Arena::new(true)?;
    arena.reclaim = true;
    assert_eq!(bursts(&mut arena)?, source);
    for round in 0..refused + 2 {
        let phase = arena.normal(&source)?;
        assert_eq!(
            phase.table,
            Some((5, span)),
            "round {round}: reused from the base"
        );
        assert!(
            arena.quiescent,
            "round {round}: no compiled Program is reachable"
        );
    }
    // Code installed into reused slots runs as its own module's code.
    arena.normal("5 [ 3 + ] run")?;
    assert_eq!(arena.snapshot()?, ["1:8"]);
    let phase = arena.normal("drop")?;
    assert_eq!(phase.table.map(|(first, _)| first), Some(5));
    Ok(())
}

#[test]
fn calculator_live_compiled_program_keeps_table_slots_reserved() -> Result<(), String> {
    let mut arena = Arena::new(true)?;
    arena.reclaim = true;
    let source = bursts(&mut arena)?;
    let live = arena.normal("[ 1 + ]")?;
    let (first, count) = live.table.ok_or("quotation allocated no compiled code")?;
    assert_eq!(first, 5);
    assert!(
        !arena.quiescent,
        "a compiled Program on the stack is reachable"
    );
    let mut next = first + count;
    for round in 0..4 {
        let phase = arena.normal(&source)?;
        let (first, count) = phase.table.ok_or("burst allocated no compiled code")?;
        assert_eq!(
            first, next,
            "round {round}: no reuse below a live compiled Program"
        );
        assert!(!arena.quiescent);
        next += count;
    }
    // The reserved slots still hold `[ 1 + ]`; a premature rewind would have
    // overwritten them with a burst's `[ 3 + ]` or a definition body.
    let run = arena.normal("5 swap run")?;
    assert_eq!(run.table.map(|(first, _)| first), Some(next));
    assert_eq!(arena.snapshot()?, ["1:6"]);
    assert!(
        arena.quiescent,
        "running the Program consumed the last compiled handle"
    );
    let reused = arena.normal(&source)?;
    assert_eq!(reused.table.map(|(first, _)| first), Some(5));
    // A compiled Program nested inside an aggregate is reachable too.
    arena.normal("drop [ 2 + ] 7 pair")?;
    assert!(!arena.quiescent, "a Program below a pair is reachable");
    let held = arena.normal(&source)?;
    assert!(held.table.is_some_and(|(first, _)| first > 5));
    arena.normal("unpair swap run")?;
    assert_eq!(arena.snapshot()?, ["1:9"]);
    assert!(arena.quiescent);
    Ok(())
}

#[test]
fn calculator_any_reachable_program_blocks_the_signature_restart() -> Result<(), String> {
    let mut arena = Arena::new(true)?;
    arena.reclaim = true;
    arena.normal("1 2 pair")?;
    assert!(
        arena.quiescent && arena.program_free,
        "plain data holds no signature ID"
    );
    arena.normal("drop [ 1 + ]")?;
    assert_eq!(arena.call0("compiled_programs_reachable")?, 1);
    assert_eq!(arena.call0("programs_reachable")?, 1);
    assert!(!arena.quiescent && !arena.program_free);
    // A Program that runs no compiled code still names its interface
    // signatures: table reuse may proceed, the signature restart may not.
    arena.normal("drop 7 quote")?;
    assert_eq!(
        arena.call0("programs_reachable")?,
        1,
        "a quoted Program holds signature IDs"
    );
    assert!(!arena.program_free);
    arena.normal("drop 9")?;
    assert!(arena.quiescent && arena.program_free);
    // After restarting at signature ID 0 the next module republishes its
    // own keys, and Programs it builds run under the fresh IDs.
    arena.normal("drop 5 [ 3 + ] run")?;
    assert_eq!(arena.snapshot()?, ["1:8"]);
    arena.normal("drop [ 2 + ] 7 pair")?;
    assert!(
        !arena.program_free,
        "a Program nested in an aggregate is reachable"
    );
    Ok(())
}

#[test]
fn calculator_static_literals_stay_content_addressed_across_a_long_session() -> Result<(), String>
{
    let mut arena = Arena::new(true)?;
    arena.normal("\"let\"")?;
    let live = arena.snapshot()?;
    for _ in 0..200 {
        arena.normal("\"let\" drop \"fn\" drop")?;
    }
    // Every module rewrote the same bytes at the same address, so the
    // first module's live static Text still reads "let".
    assert_eq!(arena.snapshot()?, live);
    // Only distinct literal contents ever advance the static cursor.
    let checked = arena
        .session
        .prepare(b"\"fn\" drop \"x\" drop", &arena.stack, limits())
        .map_err(|error| format!("{error:?}"))?;
    let metrics = arena
        .compiler
        .prepare(checked.submission().ok_or("no submission")?)
        .map_err(|_| "calculator compiler refused")?
        .metrics();
    assert_eq!(
        metrics.text_end,
        noble_wasm::source::CALCULATOR_CAPS.text_start + 3 + 2 + 1
    );
    Ok(())
}
