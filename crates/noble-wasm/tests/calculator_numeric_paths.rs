//! Calculator-v1 CALC-04/CALC-05 library harness: the real `num@1` guest words
//! run as separately checked calculator phases, compiled by the selected
//! calculator backend and executed in one shared memory/table/global set.

use noble_contracts::source::{ModuleKind, ModuleSession};
use noble_kernel::types::Ty;
use wasmtime::{
    Engine, Global, GlobalType, Instance, Linker, Memory, MemoryType, Module, Mutability, Ref,
    RefType, Store, Table, TableType, Val, ValType,
};

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 262_144,
    nodes: 524_288,
    depth: 64,
    work: 512_000_000,
};
const BEGIN: (i32, i32, i32, i32, i32, i32) = (1_376_208, 256, 256, 2048, 4096, 100_000);
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
const MODULES: [(&str, &[u8]); 11] = [
    (
        "result@1",
        include_bytes!("../../noble-contracts/src/fixtures/result.noble"),
    ),
    (
        "calc_errors@1",
        include_bytes!("../../noble-contracts/src/fixtures/calc-errors.noble"),
    ),
    (
        "calc_lex@1",
        include_bytes!("../../noble-contracts/src/fixtures/calculator-lexical.noble"),
    ),
    (
        "calc_bits@1",
        include_bytes!("../../noble-contracts/src/fixtures/calculator-bits.noble"),
    ),
    (
        "calc_big@1",
        include_bytes!("../../noble-contracts/src/fixtures/calculator-bigint.noble"),
    ),
    (
        "calc_decimal@1",
        include_bytes!("../../noble-contracts/src/fixtures/calculator-decimal.noble"),
    ),
    (
        "num@1",
        include_bytes!("../../noble-contracts/src/fixtures/calculator-numeric.noble"),
    ),
    (
        "calc_env@1",
        include_bytes!("../../noble-contracts/src/fixtures/calculator-bindings.noble"),
    ),
    (
        "calc_display@1",
        include_bytes!("../../noble-contracts/src/fixtures/calculator-display.noble"),
    ),
    (
        "expr@1",
        include_bytes!("../../noble-contracts/src/fixtures/calculator-expression.noble"),
    ),
    (
        "calc_defs@1",
        include_bytes!("../../noble-contracts/src/fixtures/calculator-definitions.noble"),
    ),
];

/// Test-only constructor gate. `num@1` is at its 256-definition cap, so the
/// harness's own word lives in a separate module checked after the eleven
/// calculator modules. It is the only harness-authored Noble definition.
const PROBE_MODULE: &[u8] = b"module calc_rat_probe@1 [
    signature denominator_check forall<S:stack>
        [ S I64 I64
          -- S result@1.Result<Pair<I64,I64>,calc_errors@1.NumericError> ! pure ]
    export denominator_check
    def denominator_check [
        dup 0 =
        [ drop drop unit calc_errors@1.DomainArithmetic.ZeroDenominator
          calc_errors@1.DomainExtended.Arithmetic
          calc_errors@1.DomainError.Extended
          calc_errors@1.NumericError.Domain result@1.Result.Err ]
        [ pair result@1.Result.Ok ] if
    ]
]";

/// Generous guest budgets: these vectors exercise values, not limits.
const BUDGET: [i64; 7] = [1_024, 256, 4_096, 32, 16, 4_000_000_000, 4_096];

/// `calc_errors@1.numeric_code` values observed by the vectors.
const OUT_OF_RANGE: i64 = 15;
const NON_INTEGRAL: i64 = 14;
const ZERO_DENOMINATOR: i64 = 17;

struct Guest {
    store: Store<()>,
    linker: Linker<()>,
    modules: Option<ModuleSession>,
    compiler: noble_wasm::source::Compiler,
    stack: Vec<Ty>,
    quiescent: bool,
    program_free: bool,
    last: Option<Instance>,
    begin: (i32, i32, i32, i32, i32, i32),
    /// A scalar-consuming prefix for the next phase, as the calculator host
    /// defers `drop` after reading a guest observation.
    pending: Option<&'static str>,
    phases: u64,
}

impl Guest {
    fn new() -> Result<Self, String> {
        let mut modules = ModuleSession::new_calculator().map_err(|e| format!("{e:?}"))?;
        for (name, source) in MODULES
            .into_iter()
            .chain([("calc_rat_probe@1", PROBE_MODULE)])
        {
            let prepared = modules
                .prepare(source, &[], LIMITS)
                .map_err(|e| format!("{name}: {e:?}"))?;
            assert_eq!(prepared.kind(), ModuleKind::Module, "{name}");
            let (next, committed) = modules.commit(prepared);
            committed.map_err(|e| format!("{name}: {e:?}"))?;
            modules = next;
        }
        for source in ["import num@1 as num", "import calc_big@1 as big"] {
            let prepared = modules
                .prepare(source.as_bytes(), &[], LIMITS)
                .map_err(|e| format!("{source}: {e:?}"))?;
            assert_eq!(prepared.kind(), ModuleKind::Import, "{source}");
            let (next, committed) = modules.commit(prepared);
            committed.map_err(|e| format!("{source}: {e:?}"))?;
            modules = next;
        }
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
        }
        Ok(Self {
            store,
            linker,
            modules: Some(modules),
            compiler: noble_wasm::source::Compiler::new_calculator(),
            stack: Vec::new(),
            quiescent: false,
            program_free: false,
            last: None,
            begin: BEGIN,
            pending: None,
            phases: 0,
        })
    }

    fn call0(&mut self, name: &str) -> Result<i32, String> {
        let instance = self.last.ok_or("no instance")?;
        instance
            .get_typed_func::<(), i32>(&mut self.store, name)
            .map_err(|e| e.to_string())?
            .call(&mut self.store, ())
            .map_err(|e| e.to_string())
    }

    /// Source-check a phase over the retained typed stack without running it.
    fn check(&self, source: &str) -> Result<(), String> {
        self.modules
            .as_ref()
            .ok_or("namespace unavailable")?
            .prepare(source.as_bytes(), &self.stack, LIMITS)
            .map(|_| ())
            .map_err(|error| format!("{error:?}"))
    }

    /// One checked guest phase over the retained typed stack, with optional
    /// I64 inputs pushed above it, as the calculator worker runs a module:
    /// check, compile with the selected calculator backend, collect, begin,
    /// inject, submit, then commit compiler and namespace. A refusal before
    /// instantiation leaves every retained state unchanged.
    fn phase(&mut self, source: &str, inputs: &[i64]) -> Result<(), String> {
        let mut types = self.stack.clone();
        types.extend(inputs.iter().map(|_| Ty::I64));
        let modules = self.modules.take().ok_or("namespace unavailable")?;
        let checked = match modules.prepare(source.as_bytes(), &types, LIMITS) {
            Ok(checked) if checked.kind() == ModuleKind::Expression => checked,
            Ok(_) => {
                self.modules = Some(modules);
                return Err(format!("{source}: not a guest expression"));
            }
            Err(error) => {
                self.modules = Some(modules);
                return Err(format!("{source}: {error:?}"));
            }
        };
        let output = checked.output().to_vec();
        let submission = checked.submission().ok_or("no submission")?;
        let prepared = if self.program_free {
            self.compiler.prepare_program_free(submission)
        } else if self.quiescent {
            self.compiler.prepare_quiescent(submission)
        } else {
            self.compiler.prepare(submission)
        };
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.modules = Some(modules);
                return Err(format!("{source}: backend {}", backend(error)));
            }
        };
        let wasm = wat::parse_bytes(prepared.wat()).map_err(|e| e.to_string())?;
        let module = Module::new(self.store.engine(), wasm.as_ref()).map_err(|e| e.to_string())?;
        let instance = self
            .linker
            .instantiate(&mut self.store, &module)
            .map_err(|e| e.to_string())?;
        self.last = Some(instance);
        let mut status = self.call0("collect")?;
        if status == 0 {
            status = instance
                .get_typed_func::<(i32, i32, i32, i32, i32, i32), i32>(&mut self.store, "begin")
                .map_err(|e| e.to_string())?
                .call(&mut self.store, self.begin)
                .map_err(|e| e.to_string())?;
        }
        for value in inputs {
            if status == 0 {
                status = instance
                    .get_typed_func::<i64, i32>(&mut self.store, "push_i64")
                    .map_err(|e| e.to_string())?
                    .call(&mut self.store, *value)
                    .map_err(|e| e.to_string())?;
            }
        }
        if status == 0 {
            status = self.call0("submit")?;
        }
        if status != 0 {
            self.modules = Some(modules);
            return Err(format!("{source}: runtime status {status}"));
        }
        self.quiescent = self.call0("compiled_programs_reachable")? == 0;
        self.program_free = self.quiescent && self.call0("programs_reachable")? == 0;
        self.compiler
            .commit(prepared)
            .map_err(|e| format!("{source}: compiler commit {}", backend(e)))?;
        let (next, committed) = modules.commit(checked);
        self.modules = Some(next);
        committed.map_err(|e| format!("{source}: source commit {e:?}"))?;
        self.stack = output;
        self.phases += 1;
        Ok(())
    }

    /// A phase that pushes host I64 inputs; a deferred consumer runs first on
    /// its own so it cannot consume an input.
    fn input(&mut self, source: &str, inputs: &[i64]) -> Result<(), String> {
        if let Some(consumer) = self.pending.take() {
            self.phase(consumer, &[])?;
        }
        self.phase(source, inputs)
    }

    /// Run carrier-to-carrier stages as one phase. Each stage is complete on
    /// its own, so a backend exhaustion of the joined module falls back to
    /// running the halves as separate phases.
    fn run(&mut self, stages: &[String]) -> Result<(), String> {
        let prefix = self.pending.take();
        let source = prefix
            .into_iter()
            .map(str::to_owned)
            .chain(stages.iter().cloned())
            .collect::<Vec<_>>()
            .join(" ");
        match self.phase(&source, &[]) {
            Err(error) if error.ends_with("backend exhausted") && stages.len() > 1 => {
                let (left, right) = stages.split_at(stages.len() / 2);
                eprintln!("split {} stages after backend exhaustion", stages.len());
                self.pending = prefix;
                self.run(left)?;
                self.run(right)
            }
            outcome => outcome,
        }
    }

    /// Run stages ending in an observer that pushes `count` scalars above the
    /// carrier; read them top first and defer their consumption.
    fn observe(&mut self, stages: &[String], count: usize) -> Result<Vec<i64>, String> {
        self.run(stages)?;
        let values = (0..count)
            .map(|depth| self.scalar(depth as i32))
            .collect::<Result<Vec<_>, _>>()?;
        self.pending = Some(["drop", "drop drop", "drop drop drop"][count - 1]);
        Ok(values)
    }

    /// A runtime metric of the last instance (1 is the quota reason).
    fn metric(&mut self, index: i32) -> Result<i64, String> {
        let instance = self.last.ok_or("no instance")?;
        instance
            .get_typed_func::<i32, i64>(&mut self.store, "metric")
            .map_err(|e| e.to_string())?
            .call(&mut self.store, index)
            .map_err(|e| e.to_string())
    }

    fn cell(&mut self, export: &str, handle: i32) -> Result<i32, String> {
        let instance = self.last.ok_or("no instance")?;
        instance
            .get_typed_func::<i32, i32>(&mut self.store, export)
            .map_err(|e| e.to_string())?
            .call(&mut self.store, handle)
            .map_err(|e| e.to_string())
    }

    /// A boxed scalar cell (kind 1 I64, 2 Bool) and its payload.
    fn boxed(&mut self, handle: i32) -> Result<(i32, i64), String> {
        let kind = self.cell("cell_kind", handle)?;
        let instance = self.last.ok_or("no instance")?;
        let payload = instance
            .get_typed_func::<i32, i64>(&mut self.store, "cell_payload")
            .map_err(|e| e.to_string())?
            .call(&mut self.store, handle)
            .map_err(|e| e.to_string())?;
        Ok((kind, payload))
    }

    /// A nominal variant cell (marker w = 2): Ok(true) for its first
    /// constructor (kind 12), Ok(false) for its second (kind 13), and its
    /// boxed payload handle. Layout: abi.json `cells`/`value_tags`.
    fn variant(&mut self, handle: i32) -> Result<(bool, i32), String> {
        let kind = self.cell("cell_kind", handle)?;
        if self.cell("cell_w", handle)? != 2 || !(kind == 12 || kind == 13) {
            return Err(format!(
                "cell {handle} is not a nominal variant (kind {kind})"
            ));
        }
        Ok((kind == 12, self.cell("cell_a", handle)?))
    }

    /// The top operand slot's live cell handle.
    fn top_handle(&mut self) -> Result<i32, String> {
        let instance = self.last.ok_or("no instance")?;
        let index = self.call0("stack_length")? - 1;
        let kind = instance
            .get_typed_func::<i32, i32>(&mut self.store, "stack_kind")
            .map_err(|e| e.to_string())?
            .call(&mut self.store, index)
            .map_err(|e| e.to_string())?;
        if kind < 4 {
            return Err("top operand is a scalar, not a carrier".into());
        }
        let value = instance
            .get_typed_func::<i32, i64>(&mut self.store, "stack_value")
            .map_err(|e| e.to_string())?
            .call(&mut self.store, index)
            .map_err(|e| e.to_string())?;
        i32::try_from(value).map_err(|e| e.to_string())
    }

    /// Read the guest's Bool from the top carrier C<Pair<X,Bool>> in memory,
    /// as the calculator host reads it from the engine's stack report. No
    /// phase is compiled, and the carrier is left unchanged.
    fn carrier_flag(&mut self) -> Result<bool, String> {
        let top = self.top_handle()?;
        let (ok, payload) = self.variant(top)?;
        if !ok {
            let (_, code) = self.boxed(payload)?;
            return Err(format!("carrier refused with code {code}"));
        }
        if self.cell("cell_kind", payload)? != 5 {
            return Err("carrier payload is not a Pair".into());
        }
        let right = self.cell("cell_b", payload)?;
        match self.boxed(right)? {
            (2, 0) => Ok(false),
            (2, 1) => Ok(true),
            other => Err(format!("carrier flag is not a Bool: {other:?}")),
        }
    }

    /// The constructor of the guest variant inside the top carrier C<V>:
    /// true for V's first constructor.
    fn carrier_variant(&mut self) -> Result<bool, String> {
        let top = self.top_handle()?;
        let (ok, payload) = self.variant(top)?;
        if !ok {
            let (_, code) = self.boxed(payload)?;
            return Err(format!("carrier refused with code {code}"));
        }
        Ok(self.variant(payload)?.0)
    }

    /// The scalar in operand slot `depth` below the top (0 is the top).
    fn scalar(&mut self, depth: i32) -> Result<i64, String> {
        let instance = self.last.ok_or("no instance")?;
        let height = self.call0("stack_length")?;
        let index = height - 1 - depth;
        let kind = instance
            .get_typed_func::<i32, i32>(&mut self.store, "stack_kind")
            .map_err(|e| e.to_string())?
            .call(&mut self.store, index)
            .map_err(|e| e.to_string())?;
        if kind >= 4 {
            return Err(format!(
                "operand slot {index} is not a scalar (kind {kind})"
            ));
        }
        instance
            .get_typed_func::<i32, i64>(&mut self.store, "stack_value")
            .map_err(|e| e.to_string())?
            .call(&mut self.store, index)
            .map_err(|e| e.to_string())
    }
}

// Carrier lifts. Every stage maps the retained carrier Result<X,I64> (the I64
// is `calc_errors@1.numeric_code`, or -1 for a private-tag Unit refusal) to
// Result<Y,I64>, so an Err passes every later stage unchanged.
fn lift(stage: &str) -> String {
    format!("[ {stage} result@1.Result.Ok ] [ result@1.Result.Err ] result@1.Result.match")
}

fn lift_numeric(stage: &str) -> String {
    format!(
        "[ {stage} [ result@1.Result.Ok ] [ calc_errors@1.numeric_code result@1.Result.Err ] \
         result@1.Result.match ] [ result@1.Result.Err ] result@1.Result.match"
    )
}

fn lift_unit(stage: &str) -> String {
    format!(
        "[ {stage} [ result@1.Result.Ok ] [ drop -1 result@1.Result.Err ] result@1.Result.match ] \
         [ result@1.Result.Err ] result@1.Result.match"
    )
}

/// A stage that already yields Result<Y,I64>.
fn flat(stage: &str) -> String {
    format!("[ {stage} ] [ result@1.Result.Err ] result@1.Result.match")
}

/// Under Pair<K,X>: apply a Result<Y,NumericError> stage to X, keeping K.
fn under_numeric(stage: &str) -> String {
    flat(&format!(
        "unpair {stage} [ pair result@1.Result.Ok ] \
         [ swap drop calc_errors@1.numeric_code result@1.Result.Err ] result@1.Result.match"
    ))
}

/// Gate on a probe's Pair<X,Bool> (true denies) with a NumericError builder.
fn gate(deny: &str, admit: &str) -> String {
    format!(
        "unpair [ drop {deny} calc_errors@1.numeric_code result@1.Result.Err ] \
         [ {admit} result@1.Result.Ok ] if"
    )
}

/// Observers push scalars above the carrier.
const STATUS: &str = "dup [ drop 0 ] [ ] result@1.Result.match";
const I64_RESULT: &str = "dup [ unpair drop 0 ] [ 1 ] result@1.Result.match";

/// Run stages ending in a guest probe C<Pair<X,Bool>> and read its Bool from
/// memory. The carrier keeps the Bool; the next stage consumes it.
fn probe(guest: &mut Guest, stages: &[String]) -> Result<bool, String> {
    guest.run(stages)?;
    guest.carrier_flag()
}

/// C<Budget> from fresh generous guest budgets.
fn budget(guest: &mut Guest) -> Result<(), String> {
    guest.input(
        "num.budget [ result@1.Result.Ok ] [ calc_errors@1.numeric_code result@1.Result.Err ] \
         result@1.Result.match",
        &BUDGET,
    )
}

/// C<FromI64Input> stages to C<Pair<Rat,Budget>>, under an optional kept value.
fn from_i64_stages(keep: bool) -> Vec<String> {
    if keep {
        vec![
            lift("unpair num.from_i64_measure pair"),
            under_numeric("num.from_i64_reserve"),
            lift("unpair num.from_i64_materialize pair"),
        ]
    } else {
        vec![
            lift("num.from_i64_measure"),
            lift_numeric("num.from_i64_reserve"),
            lift("num.from_i64_materialize"),
        ]
    }
}

/// C<Budget> -> C<Pair<Rat,Budget>> through num@1's staged `from_i64_*`.
fn rat_from_i64(guest: &mut Guest, value: i64) -> Result<(), String> {
    guest.input(
        "swap [ num.from_i64_input result@1.Result.Ok ] [ swap drop result@1.Result.Err ] \
         result@1.Result.match",
        &[value],
    )?;
    guest.run(&from_i64_stages(false))
}

/// C<Pair<Pair<I64,I64>,Budget>> -> C<Pair<Rat,Pair<Rat,Budget>>>: both I64s
/// enter num@1 through `from_i64_*`, threading one Budget.
fn rat_operands(guest: &mut Guest) -> Result<(), String> {
    let mut stages = vec![lift("unpair [ unpair swap ] dip num.from_i64_input pair")];
    stages.extend(from_i64_stages(true));
    stages.push(lift("unpair unpair [ swap ] dip num.from_i64_input pair"));
    stages.extend(from_i64_stages(true));
    guest.run(&stages)
}

/// C<Budget> + (a, b) -> C<Pair<Rat,Pair<Rat,Budget>>>.
fn rat_pair(guest: &mut Guest, a: i64, b: i64) -> Result<(), String> {
    guest.input(
        "pair swap [ pair result@1.Result.Ok ] [ swap drop result@1.Result.Err ] \
         result@1.Result.match",
        &[a, b],
    )?;
    rat_operands(guest)
}

/// The CALC-04 constructor: C<Budget> + (numerator, denominator). The
/// test-only gate refuses a zero denominator before any Rat exists; otherwise
/// num@1 converts both integers and divides them, normalizing through its own
/// gcd/finish pipeline. Returns the gate's status (0 or an error code).
fn construct(guest: &mut Guest, numerator: i64, denominator: i64) -> Result<i64, String> {
    guest.input(
        "calc_rat_probe@1.denominator_check swap \
         [ swap [ swap pair result@1.Result.Ok ] \
           [ swap drop calc_errors@1.numeric_code result@1.Result.Err ] result@1.Result.match ] \
         [ swap drop result@1.Result.Err ] result@1.Result.match",
        &[numerator, denominator],
    )?;
    let status = guest.observe(&[STATUS.to_owned()], 1)?[0];
    if status != 0 {
        return Ok(status);
    }
    rat_operands(guest)?;
    binary(guest, Operator::Divide)?;
    Ok(0)
}

#[derive(Clone, Copy, PartialEq)]
enum Operator {
    Subtract,
    Divide,
}

/// C<Pair<Rat,Pair<Rat,Budget>>> -> C<Pair<Rat,Budget>>: the num@1 operator
/// pipeline in the calculator host's stage order, then normalization.
fn binary(guest: &mut Guest, operator: Operator) -> Result<(), String> {
    let begin = match operator {
        Operator::Subtract => lift("unpair unpair num.op_begin_sub"),
        Operator::Divide => lift_numeric("unpair unpair num.op_begin_div"),
    };
    guest.run(&[
        begin,
        lift("num.op_bits_measure_left"),
        lift("num.op_bits_measure_right"),
        lift("num.op_bits_limit"),
        lift("num.op_bits_probe"),
        flat(&gate("unit num.op_bits_deny", "num.op_bits_admit")),
        lift("num.op_work_limit"),
        lift("num.op_work_probe"),
        lift("num.op_work_route"),
        flat(
            "unpair [ drop num.op_work_admit result@1.Result.Ok ] \
             [ swap drop num.op_work_deny calc_errors@1.numeric_code result@1.Result.Err ] \
             result@1.Result.match",
        ),
        lift("num.op_work_commit"),
        lift_unit("num.op_first_stage_seed_cross"),
    ])?;
    signed_product(guest)?;
    guest.run(&[lift_unit("num.op_first_stage_second_cross")])?;
    signed_product(guest)?;
    if operator == Operator::Subtract {
        let adds = probe(
            guest,
            &[
                lift_unit("num.op_first_stage_sum_prepare_sub"),
                lift_unit("num.op_first_stage_sum_begin"),
                lift_unit("num.op_first_stage_sum_route_probe"),
            ],
        )?;
        let finish = if adds {
            "num.op_first_stage_sum_finish_add"
        } else {
            "num.op_first_stage_sum_finish_sub"
        };
        guest.run(&[
            lift("unpair drop"),
            lift_unit(finish),
            lift_unit("num.op_first_stage_sum_den_mul"),
        ])?;
        signed_product(guest)?;
    }
    guest.run(&[lift_unit(match operator {
        Operator::Subtract => "num.op_first_stage_sum_raw",
        Operator::Divide => "num.op_first_stage_div_raw",
    })])?;
    normalize(guest)
}

/// C<OpFirstA> -> C<OpFirstC>: the staged shift-and-add product loop.
fn signed_product(guest: &mut Guest) -> Result<(), String> {
    guest.run(&[lift_unit("num.op_first_stage_begin")])?;
    for _ in 0..256 {
        let done = probe(
            guest,
            &[
                lift_unit("num.op_first_stage_route"),
                lift_unit("num.op_first_stage_route_probe"),
            ],
        )?;
        if done {
            return guest.run(&[lift("unpair drop"), lift_unit("num.op_first_stage_finish")]);
        }
        let add = probe(
            guest,
            &[
                lift("unpair drop"),
                lift_unit("num.op_first_stage_add_probe"),
            ],
        )?;
        if add {
            guest.run(&[
                lift("unpair drop"),
                lift_unit("num.op_first_stage_add_begin"),
            ])?;
            let mut more = true;
            let mut first = true;
            while more {
                let mut stages = Vec::new();
                if !first {
                    stages.push(lift("unpair drop"));
                }
                first = false;
                stages.extend([
                    lift_unit("num.op_first_stage_add_digit"),
                    lift_unit("num.op_first_stage_add_more"),
                ]);
                more = probe(guest, &stages)?;
            }
            guest.run(&[
                lift("unpair drop"),
                lift_unit("num.op_first_stage_add_finish"),
                lift_unit("num.op_first_stage_shift"),
            ])?;
        } else {
            guest.run(&[
                lift("unpair drop"),
                lift_unit("num.op_first_stage_add_keep"),
                lift_unit("num.op_first_stage_shift"),
            ])?;
        }
    }
    Err("signed product exceeded 256 multiplier bits".into())
}

/// C<Pair<RawRat,Budget>> -> C<Pair<Rat,Budget>> through num@1's gcd,
/// numerator-finish and denominator-finish gates and materialization.
fn normalize(guest: &mut Guest) -> Result<(), String> {
    guest.run(&[
        lift("unpair num.gcd_measure"),
        lift("num.gcd_bits_limit"),
        lift("num.gcd_bits_probe"),
        flat(&gate("unit num.gcd_bits_deny", "num.gcd_bits_admit")),
        lift("num.gcd_work_limit"),
        lift("num.gcd_work_probe"),
        flat(&gate("unit num.gcd_work_deny", "num.gcd_work_admit")),
        lift("num.gcd_commit_work"),
        lift("num.gcd_one_probe"),
        lift("num.gcd_one_route"),
    ])?;
    for _ in 0..4096 {
        // GcdRoute's first constructor is Done; read the guest's own tag.
        if guest.carrier_variant()? {
            guest.run(&[flat(
                "[ result@1.Result.Ok ] [ drop -1 result@1.Result.Err ] num.GcdRoute.match",
            )])?;
            finish(guest, "num")?;
            finish(guest, "den")?;
            return guest.run(&[lift("unpair num.finish_materialize")]);
        }
        guest.run(&[
            flat("[ drop -1 result@1.Result.Err ] [ result@1.Result.Ok ] num.GcdRoute.match"),
            lift("num.gcd_remainder_start"),
            lift("num.gcd_remainder_run"),
            flat(
                "[ drop -1 result@1.Result.Err ] [ result@1.Result.Ok ] \
                 num.GcdRemainderProgress.match",
            ),
            lift("num.gcd_remainder_end"),
            lift("num.gcd_cursor_check"),
        ])?;
    }
    Err("gcd exceeded 4096 remainder steps".into())
}

/// One of num@1's numerator/denominator finish gates and quotient routes,
/// one exported word per phase as the calculator host schedules them.
fn finish(guest: &mut Guest, part: &str) -> Result<(), String> {
    let route = if part == "num" {
        "FinishNumRoute"
    } else {
        "FinishDenRoute"
    };
    guest.run(&[
        lift(&format!("unpair num.finish_{part}_measure")),
        lift(&format!("num.finish_{part}_bits_limit")),
        lift(&format!("num.finish_{part}_bits_probe")),
        flat(&gate(
            &format!("unit num.finish_{part}_bits_deny"),
            &format!("num.finish_{part}_bits_admit"),
        )),
        lift(&format!("num.finish_{part}_work_limit")),
        lift(&format!("num.finish_{part}_work_probe")),
        flat(&gate(
            &format!("unit num.finish_{part}_work_deny"),
            &format!("num.finish_{part}_work_admit"),
        )),
        lift(&format!("num.finish_{part}_commit_work")),
    ])?;
    let one = probe(guest, &[lift(&format!("num.finish_{part}_one_probe"))])?;
    guest.run(&[lift(&format!("num.finish_{part}_route"))])?;
    if one {
        return guest.run(&[flat(&format!(
            "[ result@1.Result.Ok ] [ drop -1 result@1.Result.Err ] num.{route}.match"
        ))]);
    }
    let mut stages = vec![flat(&format!(
        "[ drop -1 result@1.Result.Err ] [ result@1.Result.Ok ] num.{route}.match"
    ))];
    stages.extend(
        [
            "bit_batch_begin",
            "bit_batch_step",
            "bit_batch_done_direct",
            "divide_bits",
            "divide_materialize",
            "divide_finish",
        ]
        .map(|word| lift(&format!("num.finish_{part}_{word}"))),
    );
    guest.run(&stages)
}

/// C<Pair<Rat,Budget>> -> C<Pair<Rat,Budget>> through num@1's staged negation.
fn negate(guest: &mut Guest) -> Result<(), String> {
    guest.run(&[
        lift("unpair num.neg_measure_num num.neg_measure_den"),
        flat(&format!(
            "num.neg_bits_probe {}",
            gate(
                "unit calc_errors@1.LimitNumeric.IntermediateIntegerBits \
                 calc_errors@1.LimitPrimary.Numeric calc_errors@1.LimitCategory.Primary \
                 calc_errors@1.NumericError.ResourceLimit",
                ""
            )
        )),
        flat(&format!(
            "num.neg_work_limit num.neg_work_probe {}",
            gate(
                "unit calc_errors@1.LimitRemaining.EvaluationWork \
                 calc_errors@1.LimitCategory.Remaining calc_errors@1.NumericError.ResourceLimit",
                "num.neg_work_admit num.neg_work_commit num.neg_materialize"
            )
        )),
    ])
}

/// C<Pair<Rat,Budget>> -> explicit `to_i64_*` conversion; Ok(value) or
/// Err(numeric code). The carrier is consumed into C<Pair<I64,Budget>>.
fn to_i64(guest: &mut Guest) -> Result<Result<i64, i64>, String> {
    let observed = guest.observe(
        &[
            flat(
                "unpair num.to_i64_input num.to_i64_integral_probe num.to_i64_integral_admit \
                 [ result@1.Result.Ok ] [ calc_errors@1.numeric_code result@1.Result.Err ] \
                 num.ToI64DomainOutcome.match",
            ),
            lift("num.to_i64_measure"),
            lift_numeric("num.to_i64_reserve"),
            lift_numeric("num.to_i64_materialize"),
            I64_RESULT.to_owned(),
        ],
        2,
    )?;
    Ok(if observed[0] == 0 {
        Ok(observed[1])
    } else {
        Err(observed[1])
    })
}

/// One decoded BigInt, read only through exported guest projections.
#[derive(Debug, PartialEq)]
struct Big {
    negative: bool,
    width: i64,
    ones: i64,
    low_bit: bool,
    high_bit: bool,
    /// `calc_bits@1.bits_to_i64_positive` of the magnitude bits: exact
    /// modulo 2^64, so exact for every width up to 64.
    magnitude: u64,
}

impl Big {
    /// Canonical BigInt: no redundant high zero bit and no negative zero.
    fn canonical(&self) -> bool {
        if self.width == 0 {
            !self.negative && self.ones == 0
        } else {
            self.high_bit
        }
    }

    fn exact(&self) -> Result<i128, String> {
        if self.width > 64 || self.magnitude.count_ones() as i64 != self.ones {
            return Err(format!(
                "BigInt fields disagree or exceed 64 bits: {self:?}"
            ));
        }
        let magnitude = i128::from(self.magnitude);
        Ok(if self.negative { -magnitude } else { magnitude })
    }
}

/// The decoder path: `num@1.rat_parts` on the carrier's Rat, then exported
/// `calc_big@1`/`calc_bits@1` projections of each part. The carrier stays.
fn decode(guest: &mut Guest) -> Result<(Big, Big), String> {
    let mut parts = Vec::new();
    for select in ["unpair drop", "unpair swap drop"] {
        let part = format!("unpair drop num.rat_parts {select}");
        let shape = guest.observe(
            &[format!(
                "dup [ {part} dup big.big_sign swap big.big_bits calc_bits@1.bit_count ] \
                 [ drop false -1 ] result@1.Result.match"
            )],
            2,
        )?;
        let bits = guest.observe(
            &[format!(
                "dup [ {part} big.big_bits dup calc_bits@1.count_one_bits swap \
                 dup [ false ] [ drop ] list.case swap \
                 calc_bits@1.reverse_bits [ false ] [ drop ] list.case ] \
                 [ drop -1 false false ] result@1.Result.match"
            )],
            3,
        )?;
        let magnitude = guest.observe(
            &[format!(
                "dup [ {part} big.big_bits calc_bits@1.bits_to_i64_positive ] [ drop 0 ] \
                 result@1.Result.match"
            )],
            1,
        )?;
        if shape[0] < 0 || bits[2] < 0 {
            return Err("decoder ran on a refused carrier".into());
        }
        parts.push(Big {
            negative: shape[1] != 0,
            width: shape[0],
            ones: bits[2],
            low_bit: bits[1] != 0,
            high_bit: bits[0] != 0,
            magnitude: magnitude[0] as u64,
        });
    }
    let den = parts.pop().ok_or("missing denominator")?;
    let num = parts.pop().ok_or("missing numerator")?;
    Ok((num, den))
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.abs()
}

/// CALC-NUM-02's invariant on a decoded Rat; returns the exact (n, d).
fn accepted_rat(num: &Big, den: &Big) -> Result<(i128, i128), String> {
    if !num.canonical() || !den.canonical() {
        return Err(format!("non-canonical BigInt exposed: {num:?} / {den:?}"));
    }
    let (n, d) = (num.exact()?, den.exact()?);
    if d <= 0 || den.negative || gcd(n, d) != 1 || (n == 0 && d != 1) {
        return Err(format!("invalid Rat exposed: {n}/{d}"));
    }
    let width = 128 - i128::from(n.unsigned_abs().leading_zeros());
    if (n < 0) != num.negative || i128::from(num.width) != width {
        return Err(format!("decoded numerator fields disagree: {num:?}"));
    }
    Ok((n, d))
}

fn backend(error: noble_wasm::Diagnostic) -> &'static str {
    match error {
        noble_wasm::Diagnostic::Invalid => "invalid",
        noble_wasm::Diagnostic::Exhausted => "exhausted",
        noble_wasm::Diagnostic::Unsupported => "unsupported",
        noble_wasm::Diagnostic::Defective => "defective",
    }
}

/// The source checker refuses `implicit` over the retained stack while the
/// explicit `control` with the same shape checks.
fn implicit_refused(guest: &Guest, implicit: &str, control: &str) -> Result<(), String> {
    guest.check(control)?;
    let refusal = guest
        .check(implicit)
        .err()
        .ok_or(format!("implicit conversion accepted: {implicit}"))?;
    eprintln!("refused {implicit}: {refusal}");
    Ok(())
}

fn report(label: &str, guest: &Guest, started: std::time::Instant) {
    eprintln!("{label}: {} phases, {:?}", guest.phases, started.elapsed());
}

// ---------------------------------------------------------------- CALC-05

/// Core-Bootstrap I64 wraps on the same selected backend. This is core
/// semantics, recorded beside CALC-05; it is not a calculator conversion.
#[test]
fn calc_05_core_wrap_is_core_semantics() -> Result<(), String> {
    let mut core = Guest::new()?;
    core.phase("9223372036854775807 1 +", &[])?;
    assert_eq!(core.stack, [Ty::I64]);
    assert_eq!(core.scalar(0)?, i64::MIN);
    Ok(())
}

/// No implicit conversion: the source checker refuses an I64 where a Rat is
/// required and a Rat where an I64 is required, while the explicit staged
/// words check over the same retained stacks.
#[test]
fn calc_05_no_implicit_conversion() -> Result<(), String> {
    let mut guest = Guest::new()?;
    budget(&mut guest)?;
    implicit_refused(
        &guest,
        "[ 5 swap num.to_i64_input result@1.Result.Ok ] [ result@1.Result.Err ] result@1.Result.match",
        "[ 5 swap num.from_i64_input result@1.Result.Ok ] [ result@1.Result.Err ] result@1.Result.match",
    )?;
    rat_from_i64(&mut guest, i64::MAX)?;
    implicit_refused(
        &guest,
        "[ unpair drop 1 + result@1.Result.Ok ] [ result@1.Result.Err ] result@1.Result.match",
        "[ unpair num.to_i64_input result@1.Result.Ok ] [ result@1.Result.Err ] result@1.Result.match",
    )
}

/// CALC-05 vectors 1, 2, 3 and 6: from-I64 -2^63, to-I64 at both bounds, and
/// 2^63 (num@1 negation of -2^63) refused as OutOfRange instead of wrapping.
#[test]
#[ignore = "slow (~40 phases); run: cargo test --release -p noble-wasm --test calculator_numeric_paths -- --ignored"]
fn calc_05_i64_bound_vectors() -> Result<(), String> {
    let started = std::time::Instant::now();
    let mut guest = Guest::new()?;
    budget(&mut guest)?;
    rat_from_i64(&mut guest, i64::MIN)?;
    let (num, den) = decode(&mut guest)?;
    assert_eq!(accepted_rat(&num, &den)?, (i128::from(i64::MIN), 1));
    assert_eq!((num.width, num.ones), (64, 1));
    assert_eq!(to_i64(&mut guest)?, Ok(i64::MIN));
    guest.run(&["drop".to_owned()])?;
    budget(&mut guest)?;
    rat_from_i64(&mut guest, i64::MAX)?;
    let (num, den) = decode(&mut guest)?;
    assert_eq!(accepted_rat(&num, &den)?, (i128::from(i64::MAX), 1));
    assert_eq!(to_i64(&mut guest)?, Ok(i64::MAX));
    guest.run(&["drop".to_owned()])?;
    budget(&mut guest)?;
    rat_from_i64(&mut guest, i64::MIN)?;
    negate(&mut guest)?;
    let (num, den) = decode(&mut guest)?;
    assert_eq!(accepted_rat(&num, &den)?, (1_i128 << 63, 1));
    assert_eq!(to_i64(&mut guest)?, Err(OUT_OF_RANGE));
    report("calc_05_i64_bound_vectors", &guest, started);
    Ok(())
}

/// CALC-05 vectors 4 and 5 need exact subtraction/division, which runs the
/// whole staged operator and normalization pipeline (hundreds of phases).
#[test]
#[ignore = "slow (staged num@1 operator pipeline); run: cargo test --release -p noble-wasm --test calculator_numeric_paths -- --ignored"]
fn calc_05_out_of_range_below_and_non_integral() -> Result<(), String> {
    let started = std::time::Instant::now();
    let mut guest = Guest::new()?;
    // -2^63 - 1, exact.
    budget(&mut guest)?;
    rat_pair(&mut guest, i64::MIN, 1)?;
    binary(&mut guest, Operator::Subtract)?;
    let (num, den) = decode(&mut guest)?;
    assert_eq!(accepted_rat(&num, &den)?, (-(1_i128 << 63) - 1, 1));
    assert!(num.low_bit);
    assert_eq!(to_i64(&mut guest)?, Err(OUT_OF_RANGE));
    // 1/2 is NonIntegral.
    guest.run(&["drop".to_owned()])?;
    budget(&mut guest)?;
    assert_eq!(construct(&mut guest, 1, 2)?, 0);
    let (num, den) = decode(&mut guest)?;
    assert_eq!(accepted_rat(&num, &den)?, (1, 2));
    assert_eq!(to_i64(&mut guest)?, Err(NON_INTEGRAL));
    report(
        "calc_05_out_of_range_below_and_non_integral",
        &guest,
        started,
    );
    Ok(())
}

// ---------------------------------------------------------------- CALC-04

/// CALC-04 vector 5: the constructor refuses 1/0 before any Rat exists.
#[test]
fn calc_04_zero_denominator() -> Result<(), String> {
    let mut guest = Guest::new()?;
    budget(&mut guest)?;
    assert_eq!(construct(&mut guest, 1, 0)?, ZERO_DENOMINATOR);
    // The retained carrier is the refusal itself: no Rat was exposed, and the
    // decoder has nothing to decode.
    assert!(decode(&mut guest).is_err());
    Ok(())
}

/// CALC-04 vectors 1-4 through both paths, and equality by normalized value.
#[test]
#[ignore = "slow (staged num@1 division and normalization); run: cargo test --release -p noble-wasm --test calculator_numeric_paths -- --ignored"]
fn calc_04_constructor_and_decoder_vectors() -> Result<(), String> {
    let started = std::time::Instant::now();
    let mut guest = Guest::new()?;
    let mut decoded = Vec::new();
    for (numerator, denominator, expected) in [
        (2, 4, (1, 2)),
        (-4, -8, (1, 2)),
        (1, -2, (-1, 2)),
        (0, -7, (0, 1)),
    ] {
        budget(&mut guest)?;
        assert_eq!(construct(&mut guest, numerator, denominator)?, 0);
        let (num, den) = decode(&mut guest)?;
        let rat = accepted_rat(&num, &den)?;
        assert_eq!(rat, expected, "{numerator}/{denominator}");
        decoded.push((num, den));
        guest.run(&["drop".to_owned()])?;
    }
    // 2/4 and -4/-8 decode to the identical canonical parts of 1/2.
    assert_eq!(decoded[0], decoded[1]);
    assert_ne!(decoded[0], decoded[2]);
    report("calc_04_constructor_and_decoder_vectors", &guest, started);
    Ok(())
}

// ---------------------------------------------------------------- CALC-08

/// A native runtime quota is a runtime status of the selected backend, never a
/// guest `ResourceLimit`: the same budget phase that succeeds under the
/// calculator's selected limits fails with a nonzero status, and no guest
/// Result exists, when only the runtime step quota is lowered.
#[test]
fn native_step_quota_is_not_a_guest_resource_limit() -> Result<(), String> {
    let mut guest = Guest::new()?;
    budget(&mut guest)?;
    assert_eq!(guest.observe(&[STATUS.to_owned()], 1)?, [0]);
    let mut starved = Guest::new()?;
    starved.begin.5 = 8;
    let refused = budget(&mut starved)
        .err()
        .ok_or("step quota was not enforced")?;
    // Runtime status 1/2 is initialization/runtime exhaustion (abi.json); the
    // quota reason is a runtime metric, and no guest value was produced.
    assert!(
        refused.ends_with("runtime status 1") || refused.ends_with("runtime status 2"),
        "{refused}"
    );
    eprintln!("starved: {refused}; quota reason {}", starved.metric(1)?);
    assert!(starved.stack.is_empty());
    Ok(())
}
