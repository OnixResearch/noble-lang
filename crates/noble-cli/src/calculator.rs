//! Calculator shell: terminal framing and an opaque, source-checked guest session.
//! Parsing, name resolution, arithmetic and display decisions live in the Noble modules.

use noble_contracts::source::{ModuleKind, ModuleSession};
use noble_kernel::types::Ty;
use std::io::Write;

const ASSEMBLER_VERSION: &str = "noble-calculator-wat 1.243.0";
const ASSEMBLY_LIMIT: u64 = 4 * 1024 * 1024;
const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 262_144,
    nodes: 524_288,
    depth: 64,
    work: 512_000_000,
};
const INPUT_LIMIT: u32 = 65_536;
const DEFAULT_BUDGET: [i64; 7] = [1_024, 256, 4_096, 32, 16, 4_000_000, 4_096];
const BUDGET_NAMES: [&str; 7] = [
    "--input-bytes",
    "--numeric-digits",
    "--intermediate-bits",
    "--syntax-depth",
    "--call-depth",
    "--evaluation-work",
    "--output-size",
];
const RESULT: &[u8] = include_bytes!("../../noble-contracts/src/fixtures/result.noble");
const ERRORS: &[u8] = include_bytes!("../../noble-contracts/src/fixtures/calc-errors.noble");
const LEXICAL: &[u8] =
    include_bytes!("../../noble-contracts/src/fixtures/calculator-lexical.noble");
const BITS: &[u8] = include_bytes!("../../noble-contracts/src/fixtures/calculator-bits.noble");
const BIGINT: &[u8] = include_bytes!("../../noble-contracts/src/fixtures/calculator-bigint.noble");
const DECIMAL: &[u8] =
    include_bytes!("../../noble-contracts/src/fixtures/calculator-decimal.noble");
const NUMERIC: &[u8] =
    include_bytes!("../../noble-contracts/src/fixtures/calculator-numeric.noble");
const BINDINGS: &[u8] =
    include_bytes!("../../noble-contracts/src/fixtures/calculator-bindings.noble");
const DISPLAY: &[u8] =
    include_bytes!("../../noble-contracts/src/fixtures/calculator-display.noble");
const EXPRESSION: &[u8] =
    include_bytes!("../../noble-contracts/src/fixtures/calculator-expression.noble");
const DEFINITIONS: &[u8] =
    include_bytes!("../../noble-contracts/src/fixtures/calculator-definitions.noble");

pub(crate) fn assembler_version(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    if arguments.len() != 1 {
        return std::process::ExitCode::from(2);
    }
    println!("{ASSEMBLER_VERSION}");
    std::process::ExitCode::SUCCESS
}

/// Selected calculator-only, offline WAT assembly. Existing runtime profiles
/// keep their pinned wasm-tools/Binaryen selection and cannot use this route.
pub(crate) fn assemble_wat(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    if arguments.len() != 1 {
        return std::process::ExitCode::from(2);
    }
    let mut source = Vec::new();
    let stdin = std::io::stdin();
    if let Err(error) = std::io::Read::read_to_end(
        &mut std::io::Read::take(stdin.lock(), ASSEMBLY_LIMIT + 1),
        &mut source,
    ) {
        eprintln!("calculator WAT ingress failed: {error}");
        return std::process::ExitCode::from(2);
    }
    if source.len() > ASSEMBLY_LIMIT as usize || source.starts_with(b"\0asm") {
        eprintln!("calculator WAT ingress refused");
        return std::process::ExitCode::from(2);
    }
    let wasm = match wat::parse_bytes(&source) {
        Ok(wasm) => wasm,
        Err(error) => {
            eprintln!("calculator WAT parse refused: {error}");
            return std::process::ExitCode::from(2);
        }
    };
    if wasm.len() > ASSEMBLY_LIMIT as usize {
        eprintln!("calculator Wasm output limit exceeded");
        return std::process::ExitCode::from(2);
    }
    if let Err(error) = std::io::stdout().lock().write_all(&wasm) {
        eprintln!("calculator Wasm output failed: {error}");
        return std::process::ExitCode::from(2);
    }
    std::process::ExitCode::SUCCESS
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let (framed, finite_decimal, budget) = match parse_options(&arguments[1..]) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}\nusage: noble calculator [--framed] [--finite-decimal] [--input-bytes N] [--numeric-digits N] [--intermediate-bits N] [--syntax-depth N] [--call-depth N] [--evaluation-work N] [--output-size N]");
            return std::process::ExitCode::from(2);
        }
    };
    let mut session = match Session::new() {
        Ok(session) => session,
        Err(message) => {
            eprintln!("noble calculator: {message}");
            return std::process::ExitCode::from(2);
        }
    };
    let stdin = std::io::stdin();
    let mut input = std::io::BufReader::new(stdin.lock());
    let mut failed = false;
    loop {
        let line = match crate::core::framing::read_submission(&mut input, framed, INPUT_LIMIT) {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(error) => {
                if error.context.outcome == "exhausted" {
                    eprintln!("TransportLimit InputFrameBytes: {}", error.message);
                } else {
                    eprintln!("noble calculator: {}", error.message);
                }
                return std::process::ExitCode::from(2);
            }
        };
        match session.submit(&line, finite_decimal, budget) {
            Ok(Outcome::Value(value)) => println!("{value}"),
            Ok(Outcome::Defined) => (),
            Ok(Outcome::Diagnostic(diagnostic)) => {
                eprintln!("{diagnostic}");
                failed = true;
            }
            Ok(Outcome::NativeFailure(message)) => {
                eprintln!("NativeFailure: {message}");
                failed = true;
            }
            Ok(Outcome::InfrastructureFailure(message)) => {
                eprintln!("InfrastructureFailure: {message}");
                failed = true;
            }
            Err(message) => {
                eprintln!("noble calculator: {message}");
                return std::process::ExitCode::from(2);
            }
        }
        if let Err(error) = std::io::stdout().flush() {
            eprintln!("noble calculator: terminal output failed: {error}");
            return std::process::ExitCode::from(2);
        }
    }
    if failed {
        std::process::ExitCode::from(1)
    } else {
        std::process::ExitCode::SUCCESS
    }
}

fn parse_options(arguments: &[std::ffi::OsString]) -> Result<(bool, bool, [i64; 7]), String> {
    let mut budget = DEFAULT_BUDGET;
    let mut selected = [false; 7];
    let mut framed = false;
    let mut finite_decimal = false;
    let mut at = 0;
    while at < arguments.len() {
        let word = arguments[at]
            .to_str()
            .ok_or("non-UTF-8 calculator option")?;
        match word {
            "--framed" if !framed => framed = true,
            "--finite-decimal" if !finite_decimal => finite_decimal = true,
            _ => {
                let index = BUDGET_NAMES
                    .iter()
                    .position(|name| *name == word)
                    .ok_or_else(|| format!("unknown or duplicate calculator option: {word}"))?;
                if selected[index] {
                    return Err(format!("duplicate calculator option: {word}"));
                }
                selected[index] = true;
                at += 1;
                let value = arguments
                    .get(at)
                    .and_then(|word| word.to_str())
                    .ok_or_else(|| format!("missing {word} value"))?;
                budget[index] = value
                    .parse::<i64>()
                    .ok()
                    .filter(|value| *value >= 0)
                    .ok_or_else(|| format!("invalid {word} value"))?;
            }
        }
        at += 1;
    }
    Ok((framed, finite_decimal, budget))
}

enum Outcome {
    Value(String),
    Defined,
    Diagnostic(String),
    NativeFailure(String),
    InfrastructureFailure(String),
}

/// One committed calculator session. Each line runs directly on it: success
/// commits, every failure restores the last commit (see `transact`).
struct Session {
    modules: Option<ModuleSession>,
    compiler: noble_wasm::source::Compiler,
    worker: crate::core::worker::Engine,
    stack: Vec<Ty>,
    serial: u64,
    // The last committed module reported that no Program reachable from the
    // operand stack runs compiled code, so the next module may reuse every
    // committed compiled-code table slot.
    quiescent: bool,
    // It also reported that no Program, Syntax or recipe cell is reachable,
    // so the next module may restart the signature pool from ID 0.
    program_free: bool,
    // A quotation-free consumer of the last guest probe (`drop`, `unpair
    // drop`), run as the prefix of the next module instead of as its own.
    deferred: Option<&'static [u8]>,
}

/// The host half of the last commit; the worker holds the arena half.
struct Committed {
    modules: ModuleSession,
    compiler: noble_wasm::source::Compiler,
    stack: Vec<Ty>,
    serial: u64,
    quiescent: bool,
    program_free: bool,
    deferred: Option<&'static [u8]>,
}

impl Session {
    fn parser_fault(
        &mut self,
        report: &crate::core::output::Report,
    ) -> Result<Option<Outcome>, String> {
        // The rehydrated Diagnostic is large, so the converter runs with the
        // parser's [ State Bool ] parked above it, not beneath its nodes:
        // [ State Bool Result<A,ParseFault> ] -> [ State Bool Result<A,Diagnostic> ].
        self.parser_error(
            report,
            b"[ pair ] dip swap [ expr.parse_fault_rehydrate ] dip swap [ unpair ] dip",
        )
    }

    fn parser_state_fault(
        &mut self,
        report: &crate::core::output::Report,
    ) -> Result<Option<Outcome>, String> {
        self.parser_error(report, b"expr.parse_state_rehydrate")
    }

    fn parser_route_fault(
        &mut self,
        report: &crate::core::output::Report,
    ) -> Result<Option<Outcome>, String> {
        if checked_ready(&report.json)? {
            return Ok(None);
        }
        // The existing guest route projection preserves its original
        // ParseFault on Failed, so the state terminal handles both errors.
        let pending = self.execute(b"expr.parse_route_take", &[], None)?;
        if pending.outcome != "normal" {
            return Ok(Some(Outcome::NativeFailure(pending.json)));
        }
        self.parser_state_fault(&pending)
    }

    fn parser_error(
        &mut self,
        report: &crate::core::output::Report,
        converter: &[u8],
    ) -> Result<Option<Outcome>, String> {
        if checked_ready(&report.json)? {
            return Ok(None);
        }
        // The host sees only the checked variant arm. All public diagnostic
        // categorization and original source-span restoration remain guest
        // operations, performed once on the actual terminal fault.
        let public = self.execute(converter, &[], None)?;
        if public.outcome != "normal" {
            return Ok(Some(Outcome::NativeFailure(public.json)));
        }
        let checked = if converter == b"expr.parse_state_rehydrate".as_slice() {
            checked_sum_error(&public.json)?
        } else {
            checked_error(&public.json)?
        };
        let diagnostic = checked.ok_or("guest parser fault lost its original error")?;
        Ok(Some(Outcome::Diagnostic(diagnostic)))
    }

    fn new() -> Result<Self, String> {
        Self::with_worker(
            crate::core::worker::Engine::start_calculator(false).map_err(|error| error.message)?,
        )
    }

    fn with_worker(worker: crate::core::worker::Engine) -> Result<Self, String> {
        let mut modules = ModuleSession::new_calculator().map_err(source_error)?;
        for (name, source) in [
            ("result@1", RESULT),
            ("calc_errors@1", ERRORS),
            ("calc_lex@1", LEXICAL),
            ("calc_bits@1", BITS),
            ("calc_big@1", BIGINT),
            ("calc_decimal@1", DECIMAL),
            ("num@1", NUMERIC),
            ("calc_env@1", BINDINGS),
            ("calc_display@1", DISPLAY),
            ("expr@1", EXPRESSION),
            ("calc_defs@1", DEFINITIONS),
        ] {
            let prepared = modules
                .prepare(source, &[], LIMITS)
                .map_err(|error| format!("{name}: {}", source_error(error)))?;
            if prepared.kind() != ModuleKind::Module {
                return Err(format!(
                    "{name}: calculator dependency is not a checked Noble module"
                ));
            }
            let (next, committed) = modules.commit(prepared);
            committed.map_err(|error| format!("{name}: {}", source_error(error)))?;
            modules = next;
        }
        for (source, name) in [
            (b"import expr@1 as expr".as_slice(), "expression"),
            (b"import num@1 as num".as_slice(), "numeric"),
        ] {
            let imported = modules.prepare(source, &[], LIMITS).map_err(source_error)?;
            if imported.kind() != ModuleKind::Import {
                return Err(format!("calculator {name} module import refused"));
            }
            let (next, committed) = modules.commit(imported);
            committed.map_err(source_error)?;
            modules = next;
        }
        let mut session = Self {
            modules: Some(modules),
            compiler: noble_wasm::source::Compiler::new_calculator(),
            worker,
            stack: Vec::new(),
            serial: 0,
            quiescent: false,
            program_free: false,
            deferred: None,
        };
        let report = session.execute(b"calc_env@1.initial", &[], None)?;
        if report.outcome != "normal" || session.stack.len() != 1 {
            return Err("guest calculator state initialization failed".into());
        }
        session
            .worker
            .calculator_commit()
            .map_err(|error| error.message)?;
        Ok(session)
    }

    // The guest leaves an authenticated Result<CompactDiagnostic,Unit> on
    // top after the rejected line's State is discarded. Its category
    // selects only a guest decoder; no host code interprets issue payloads.
    fn decode_compact_diagnostic(&mut self) -> Result<Outcome, String> {
        let mut report = self.execute(b"expr.frame_hot_parts", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        report = self.execute(b"expr.frame_hot_category", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        let json: serde_json::Value =
            serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
        let routed = json["stack"]
            .as_array()
            .and_then(|stack| stack.last())
            .ok_or("guest hot failure returned no category")?;
        let (_, category) = pair(routed)?;
        if category["type"] != "I64" {
            return Err("guest hot failure returned a non-I64 category".into());
        }
        let category = category["value"]
            .as_str()
            .ok_or("guest hot failure returned an invalid category")?
            .parse::<i64>()
            .map_err(|error| error.to_string())?;
        let decoder = match category {
            0 => b"expr.frame_hot_simple".as_slice(),
            1 | 2 => b"expr.frame_hot_terminal".as_slice(),
            _ => return Err("guest hot failure had no valid diagnostic category".into()),
        };
        self.defer_consume(b"unpair drop");
        report = self.execute(decoder, &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        Ok(Outcome::Diagnostic(checked_hot_terminal(&report.json)?))
    }

    fn submit(
        &mut self,
        expression: &[u8],
        finite_decimal: bool,
        budget: [i64; 7],
    ) -> Result<Outcome, String> {
        self.transact(|session| {
            let outcome = session.submit_uncommitted(expression, finite_decimal, budget)?;
            if matches!(outcome, Outcome::Value(_) | Outcome::Defined) {
                let state = session
                    .modules
                    .as_ref()
                    .ok_or("calculator namespace unavailable")?
                    .resolve_type("calc_env@1.State")
                    .map_err(source_error)?;
                if session.stack.as_slice() != [state] {
                    return Ok(Outcome::InfrastructureFailure(
                        "guest calculator did not retain its checked State after command completion"
                            .into(),
                    ));
                }
            }
            Ok(outcome)
        })
    }

    /// Run one submission directly on the committed session. A `Value` or
    /// `Defined` outcome commits the worker arena. Every failure class —
    /// guest diagnostic or ResourceLimit, native quota or trap (which poisons
    /// the instance), host bound and infrastructure failure — instead
    /// restores the worker byte for byte and the host state to the last
    /// commit, so a failed line leaves the session exactly as if it had never
    /// been submitted (CALC-STATE-02). Only a failed restore is terminal.
    fn transact(
        &mut self,
        line: impl FnOnce(&mut Self) -> Result<Outcome, String>,
    ) -> Result<Outcome, String> {
        let committed = self.checkpoint()?;
        let mut outcome = line(self).unwrap_or_else(Outcome::InfrastructureFailure);
        if matches!(outcome, Outcome::Value(_) | Outcome::Defined) {
            match self.worker.calculator_commit() {
                Ok(_) => return Ok(outcome),
                Err(error) => {
                    outcome = Outcome::InfrastructureFailure(format!(
                        "calculator commit failed: {}",
                        error.message
                    ));
                }
            }
        }
        self.restore(committed)?;
        Ok(outcome)
    }

    /// Copy the host state of the last commit before a line runs: every
    /// normal guest module of that line replaces `compiler`, `modules`,
    /// `stack` and the boundary flags, and advances `serial`.
    fn checkpoint(&self) -> Result<Committed, String> {
        Ok(Committed {
            modules: self
                .modules
                .as_ref()
                .and_then(ModuleSession::checkpoint)
                .ok_or("calculator namespace cannot be checkpointed")?,
            compiler: self.compiler.clone(),
            stack: self.stack.clone(),
            serial: self.serial,
            quiescent: self.quiescent,
            program_free: self.program_free,
            deferred: self.deferred,
        })
    }

    /// Restore the worker first: if its byte-exact verification fails, the
    /// session ends rather than pairing host state with an unknown arena. The
    /// runtime `$generation` global and the compiler generation return
    /// together, as do signature IDs and their published keys.
    fn restore(&mut self, committed: Committed) -> Result<(), String> {
        self.worker
            .calculator_restore()
            .map_err(|error| format!("calculator restore failed: {}", error.message))?;
        self.modules = Some(committed.modules);
        self.compiler = committed.compiler;
        self.stack = committed.stack;
        self.serial = committed.serial;
        self.quiescent = committed.quiescent;
        self.program_free = committed.program_free;
        self.deferred = committed.deferred;
        Ok(())
    }

    fn submit_uncommitted(
        &mut self,
        expression: &[u8],
        finite_decimal: bool,
        budget: [i64; 7],
    ) -> Result<Outcome, String> {
        let outcome = self.submit_line(expression, finite_decimal, budget)?;
        // An accepted command never leaves a probe consumer pending; any
        // other outcome restores the last commit anyway.
        if !matches!(outcome, Outcome::Value(_) | Outcome::Defined) {
            self.deferred = None;
            return Ok(outcome);
        }
        Ok(match self.flush_deferred()? {
            Some(failed) => Outcome::NativeFailure(failed.json),
            None => outcome,
        })
    }

    fn submit_line(
        &mut self,
        expression: &[u8],
        finite_decimal: bool,
        budget: [i64; 7],
    ) -> Result<Outcome, String> {
        if self.stack.len() != 1 {
            return Err("guest calculator retained state is unavailable".into());
        }
        let inputs = [
            serde_json::json!({ "type": "Bool", "value": finite_decimal }),
            serde_json::json!({ "type": "Text", "bytes": expression }),
        ];
        // The source is retained beside the lexer cursor until its guest
        // tokens are complete. Neither State nor source leaves this worker.
        // Both inputs arrive above State in the module that starts the scan.
        let frame = serde_json::to_string(&inputs).map_err(|error| error.to_string())?;
        let mut report = self.execute(
            b"dup calc_lex@1.scan_start",
            &[Ty::Bool, Ty::Text],
            Some(&frame),
        )?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        let scan_limit = expression
            .len()
            .checked_mul(9)
            .map(|steps| steps / 2)
            .and_then(|steps| steps.checked_add(2))
            .ok_or("guest lexer progress bound overflow")?;
        let mut scanned = false;
        for _ in 0..scan_limit {
            // Only the guest cursor chooses a lexical microphase. The shell
            // sees its certified opcode, not a byte, token, digit, or mode.
            report = self.execute(b"calc_lex@1.scan_phase_code", &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            let phase = checked_guest_opcode(&report.json)?;
            self.defer_consume(b"drop");
            if phase == 4 {
                scanned = true;
                break;
            }
            let stage = match phase {
                0 | 7 => {
                    let selector = if phase == 0 {
                        "scan_idle_opcode"
                    } else {
                        "scan_punctuation_opcode"
                    };
                    let word = format!("calc_lex@1.{selector}");
                    report = self.execute(word.as_bytes(), &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    let opcode = checked_guest_opcode(&report.json)?;
                    self.defer_consume(b"drop");
                    match opcode {
                        20 => "scan_tick_space",
                        21 => "scan_tick_digit",
                        22 => "scan_tick_alpha",
                        23 => "scan_tick_eof",
                        24 => "scan_tick_punctuation_detect",
                        27 => "scan_tick_punctuation_invalid",
                        28 => "scan_tick_punctuation_advance",
                        _ => return Err("guest lexer returned an unknown selector opcode".into()),
                    }
                }
                1 => "scan_tick_integer_byte",
                2 => "scan_tick_fraction_byte",
                3 => "scan_tick_identifier_byte",
                5 => "scan_tick_resume",
                14 => "scan_tick_punctuation_emit",
                8 => "scan_tick_integer_dot",
                13 => "scan_tick_dot_fraction",
                9 => "scan_tick_number_emit",
                10 => "scan_tick_number_end",
                11 => "scan_tick_identifier_emit",
                12 => "scan_tick_identifier_end",
                _ => return Err("guest lexer returned an unknown phase opcode".into()),
            };
            let word = format!("calc_lex@1.{stage}");
            report = self.execute(word.as_bytes(), &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
        }
        if !scanned {
            return Err("guest lexer exceeded its source-byte progress bound".into());
        }
        report = self.execute(b"calc_lex@1.scan_finish_status", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        let invalid = checked_guest_bool(&report.json)?;
        self.defer_consume(b"drop");
        if invalid {
            report = self.execute(b"calc_lex@1.scan_finish_error", &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            report = self.execute(b"expr.lexical_error", &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            let diagnostic = checked_lexical_error(&report.json)?;
            return Ok(Outcome::Diagnostic(diagnostic));
        }
        report = self.execute(b"calc_lex@1.scan_finish_begin", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        let reverse_limit = expression
            .len()
            .checked_add(1)
            .ok_or("guest lexer reverse progress bound overflow")?;
        let mut reversed = false;
        for _ in 0..reverse_limit {
            report = self.execute(b"calc_lex@1.scan_finish_step", &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            let pending = checked_guest_bool(&report.json)?;
            self.defer_consume(b"drop");
            if !pending {
                reversed = true;
                break;
            }
        }
        if !reversed {
            return Err("guest lexer reverse exceeded its source-byte bound".into());
        }
        report = self.execute(b"calc_lex@1.scan_finish_tokens", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        // Guest header recognition consumes only scanner-certified tokens.
        // Retain the actual original list inside the worker so an invalid
        // header can return its source-bound Diagnostic without inventing a
        // replacement token list. The body list is still guest-owned.
        for word in [
            b"dup [ swap ] dip calc_defs@1.header_cursor".as_slice(),
            b"calc_defs@1.header_identifier_route",
        ] {
            report = self.execute(word, &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
        }
        let identifier = checked_guest_bool(&report.json)?;
        self.defer_consume(b"drop");
        let header_kind = if identifier {
            for word in [
                b"calc_defs@1.header_word".as_slice(),
                b"calc_defs@1.header_let_pair",
                b"calc_defs@1.header_pair_size",
                b"0 calc_defs@1.header_pair_byte_if",
                b"1 calc_defs@1.header_pair_byte_if",
                b"2 calc_defs@1.header_pair_byte_if",
                b"calc_defs@1.header_pair_finish",
                b"swap",
                b"calc_defs@1.header_fn_pair",
                b"calc_defs@1.header_pair_size",
                b"0 calc_defs@1.header_pair_byte_if",
                b"1 calc_defs@1.header_pair_byte_if",
                b"calc_defs@1.header_pair_finish",
                b"calc_defs@1.header_opcode_flags",
            ] {
                report = self.execute(word, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            }
            let kind = checked_guest_opcode(&report.json)?;
            self.defer_consume(b"drop");
            kind
        } else {
            0
        };
        // This selector is emitted by the guest after scanning its original
        // tokens; Rust never classifies source bytes as a definition.
        let is_let = header_kind == 1;
        let is_function = header_kind == 2;
        let header_phases: &[&[u8]] = match header_kind {
            0 => &[b"calc_defs@1.header_expression_cursor"],
            1 => &[
                b"calc_defs@1.let_begin",
                b"[ pair pair pair ] dip",
                b"calc_defs@1.header_word",
                b"calc_defs@1.definition_name_finish",
                b"calc_defs@1.let_equals_checked",
                b"calc_defs@1.let_body_valid",
                b"calc_defs@1.let_body_finish",
            ],
            2 => &[
                b"calc_defs@1.fn_keyword",
                b"[ pair pair pair ] dip",
                b"calc_defs@1.header_word",
                b"calc_defs@1.definition_name_finish",
                b"calc_defs@1.fn_lparen",
                b"[ unpair unpair unpair ] dip",
            ],
            _ => return Err("guest calculator returned an unknown header route".into()),
        };
        for word in header_phases {
            report = self.execute(word, &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
        }
        if is_function {
            for word in [
                b"calc_defs@1.fn_params_begin".as_slice(),
                b"[ pair pair pair ] dip",
            ] {
                report = self.execute(word, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            }
            let mut finished = false;
            for _ in 0..=expression.len() {
                for word in [
                    b"swap [ calc_defs@1.fn_param_expected ] dip swap".as_slice(),
                    b"calc_bits@1.rot swap",
                ] {
                    report = self.execute(word, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                }
                let expects_name = checked_guest_bool(&report.json)?;
                self.defer_consume(b"drop");
                let route = if expects_name {
                    b"swap [ calc_defs@1.fn_param_route_name ] dip swap".as_slice()
                } else {
                    b"swap [ calc_defs@1.fn_param_route_delimiter ] dip swap"
                };
                report = self.execute(route, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                report = self.execute(b"calc_bits@1.rot swap", &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                let opcode = checked_guest_opcode(&report.json)?;
                self.defer_consume(b"drop");
                if opcode == 4 {
                    finished = true;
                    break;
                }
                if opcode == 1 {
                    for word in [
                        b"swap [ calc_defs@1.fn_param_checked_name ] dip swap".as_slice(),
                        b"calc_bits@1.rot swap",
                        b"calc_bits@1.rot [ calc_defs@1.fn_param_append_name_finish ] dip swap",
                    ] {
                        report = self.execute(word, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                    }
                } else {
                    let action = match opcode {
                        0 => b"swap [ calc_defs@1.fn_param_complete ] dip swap".as_slice(),
                        2 => b"swap [ calc_defs@1.fn_param_separator ] dip swap",
                        3 => b"swap [ calc_defs@1.fn_param_error ] dip swap",
                        _ => return Err("guest function parameter route was invalid".into()),
                    };
                    report = self.execute(action, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                }
                if opcode == 0 || opcode == 3 {
                    finished = true;
                    break;
                }
            }
            if !finished {
                return Err("guest function parameters exceeded their source-byte bound".into());
            }
            for word in [
                b"[ unpair unpair unpair ] dip".as_slice(),
                b"calc_defs@1.fn_params_end",
                b"calc_defs@1.fn_end_unpack",
                b"[ pair pair pair ] dip",
                b"swap [ calc_defs@1.fn_end_params ] dip swap",
                b"calc_bits@1.rot swap",
                b"[ pair ] dip",
                b"swap [ calc_defs@1.fn_end_validation_begin ] dip swap",
            ] {
                report = self.execute(word, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            }
            let mut validated = false;
            for _ in 0..=expression.len() {
                report = self.execute(
                    b"swap [ calc_defs@1.fn_end_validation_tick ] dip swap",
                    &[],
                    None,
                )?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                let more = checked_guest_bool(&report.json)?;
                report = self.execute(b"drop swap", &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if !more {
                    validated = true;
                    break;
                }
            }
            if !validated {
                return Err("guest parameter validation exceeded its source-byte bound".into());
            }
            for word in [
                b"swap [ calc_defs@1.fn_end_validation_result ] dip swap".as_slice(),
                b"[ unpair ] dip",
                b"calc_bits@1.rot [ calc_defs@1.fn_end_validated_result ] dip swap",
                b"swap [ calc_defs@1.fn_end_equals ] dip swap",
                b"swap [ calc_defs@1.fn_end_command ] dip swap",
                b"[ unpair unpair unpair ] dip",
            ] {
                report = self.execute(word, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            }
            // Capture only the checked Name/parameter witnesses from this
            // authentic guest command. Keep them below parser inputs on the
            // same worker; they are never decoded or parked by the host.
            for word in [
                b"dup".as_slice(),
                b"[ [ pair pair pair ] dip ] dip",
                b"[ pair ] dip",
                b"swap [ calc_defs@1.header_function_metadata ] dip swap",
                b"[ unpair ] dip",
                b"swap",
                b"[ [ unpair unpair unpair ] dip ] dip",
                b"[ swap ] dip",
                b"[ calc_bits@1.rot ] dip",
            ] {
                report = self.execute(word, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            }
        } else if is_let {
            for word in [
                b"dup".as_slice(),
                b"swap [ calc_defs@1.header_let_metadata ] dip swap",
                b"swap",
                b"[ [ unpair unpair unpair ] dip ] dip",
            ] {
                report = self.execute(word, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            }
        }
        if is_let {
            for word in [
                b"[ calc_bits@1.rot ] dip".as_slice(),
                b"[ [ swap ] dip ] dip",
            ] {
                report = self.execute(word, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            }
        }
        let body_route = if is_function || is_let {
            b"calc_defs@1.header_body_route".as_slice()
        } else {
            b"[ swap ] dip calc_defs@1.header_body_route"
        };
        report = self.execute(body_route, &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        let checked: &[&[u8]] = if is_function {
            &[
                b"[ pair pair pair pair ] dip",
                b"swap [ calc_defs@1.header_result_checked ] dip swap",
            ]
        } else if is_let {
            &[
                b"[ calc_bits@1.rot ] dip",
                b"swap",
                b"[ calc_defs@1.header_result_checked ] dip",
                b"swap",
            ]
        } else {
            &[b"calc_defs@1.header_result_checked"]
        };
        for word in checked {
            report = self.execute(word, &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
        }
        if let Some(diagnostic) = checked_error(&report.json)? {
            return Ok(Outcome::Diagnostic(diagnostic));
        }
        self.defer_consume(b"drop");
        if is_function {
            for word in [
                b"unpair unpair unpair unpair".as_slice(),
                b"[ [ swap ] dip ] dip",
                b"[ [ [ pair ] dip ] dip ] dip",
            ] {
                report = self.execute(word, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            }
        } else if is_let {
            // Keep the parser's [ hidden Bool X ] shape: the checked
            // declaration Name rides with State in the hidden cell.
            for word in [
                b"swap".as_slice(),
                b"[ swap ] dip",
                b"[ [ swap [ pair ] dip ] dip ] dip",
            ] {
                report = self.execute(word, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            }
        }
        // Admit the public quota on the same worker after the lexical cursor
        // has certified its token list. The original guest Text remains live
        // below that list. A denied guest charge stays a NumericError until
        // parse_begin binds the real span to a ParseFault; a public Diagnostic
        // is constructed only if that guest error actually terminates parsing.
        let quota_inputs =
            budget.map(|value| serde_json::json!({ "type": "I64", "value": value.to_string() }));
        let quota_frame =
            serde_json::to_string(&quota_inputs).map_err(|error| error.to_string())?;
        report = self.execute(b"num@1.budget", &[const { Ty::I64 }; 7], Some(&quota_frame))?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        report = self.execute(b"swap", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        for word in [
            b"[ expr.preparse_input_measure ] dip".as_slice(),
            b"[ expr.preparse_input_check ] dip",
            b"[ expr.preparse_input_commit ] dip",
            b"[ expr.preparse_work_measure ] dip",
            b"[ expr.preparse_work_check ] dip",
            b"[ expr.preparse_work_commit ] dip",
            b"[ expr.preparse_quota_fault ] dip",
        ] {
            report = self.execute(word, &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
        }
        let parse_begin: &[&[u8]] = if is_function {
            &[
                b"pair",
                b"[ pair ] dip",
                b"swap [ unpair expr.parse_begin_tokens ] dip swap",
                b"[ unpair ] dip",
                b"[ pair ] dip",
                b"swap [ expr.parse_begin_session ] dip swap",
                b"[ unpair ] dip",
            ]
        } else {
            // The hidden cell is State, or Pair<State,Name> for a let; phase()
            // keeps it out of the session constructor's quotations.
            &[
                b"expr.parse_begin_tokens",
                b"[ swap ] dip swap [ expr.parse_begin_session ] dip swap [ swap ] dip",
            ]
        };
        for word in parse_begin {
            report = self.execute(word, &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
        }
        if let Some(outcome) = self.parser_fault(&report)? {
            return Ok(outcome);
        }
        // Every token action is selected only from a guest-produced kind. A
        // token consumes at least one source byte; this host bound prevents
        // accidental nonprogress without inspecting input syntax.
        let mut parsed = false;
        for _ in 0..=expression.len() {
            report = self.phase(b"expr.parse_kind")?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            if let Some(outcome) = self.parser_fault(&report)? {
                return Ok(outcome);
            }
            let kind = match checked_kind(&report.json)? {
                Ok(kind) => kind,
                Err(diagnostic) => return Ok(Outcome::Diagnostic(diagnostic)),
            };
            if kind == 0 {
                report = self.phase(b"expr.parse_finish")?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                parsed = true;
                break;
            }
            report = self.phase(b"expr.parse_take")?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            if let Some(outcome) = self.parser_state_fault(&report)? {
                return Ok(outcome);
            }
            let route = match kind {
                10 => Some(b"expr.parse_identifier_route".as_slice()),
                1..=6 => Some(b"expr.parse_expecting_route".as_slice()),
                7 => Some(b"expr.parse_close_route".as_slice()),
                8 => Some(b"expr.parse_comma_route".as_slice()),
                _ => None,
            };
            let action = if let Some(route) = route {
                report = self.phase(route)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if let Some(outcome) = self.parser_route_fault(&report)? {
                    return Ok(outcome);
                }
                let selected = checked_parser_route_bool(&report.json)?;
                report = self.phase(b"expr.parse_route_take")?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if kind == 6 && selected {
                    for word in [
                        b"expr.parse_open_measure".as_slice(),
                        b"expr.parse_open_admit",
                    ] {
                        report = self.phase(word)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let fault = if word == b"expr.parse_open_admit".as_slice() {
                            self.parser_route_fault(&report)?
                        } else {
                            self.parser_fault(&report)?
                        };
                        if let Some(outcome) = fault {
                            return Ok(outcome);
                        }
                    }
                    let admitted = checked_parser_route_bool(&report.json)?;
                    report = self.phase(b"expr.parse_route_take")?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    report = self.phase(if admitted {
                        b"expr.parse_open_admitted"
                    } else {
                        b"expr.parse_open_denied"
                    })?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if let Some(outcome) = self.parser_state_fault(&report)? {
                        return Ok(outcome);
                    }
                }
                match (kind, selected) {
                    (10, true) => b"expr.parse_apply_identifier_call".as_slice(),
                    (10, false) => b"expr.parse_apply_operand",
                    (1 | 2, true) => b"expr.parse_apply_prefix",
                    (1 | 2, false) => b"expr.parse_binary_route",
                    (3..=5, true) => b"expr.parse_apply_syntax",
                    (3..=5, false) => b"expr.parse_binary_route",
                    (6, true) => b"expr.parse_apply_open_checked",
                    (6, false) => b"expr.parse_apply_operand",
                    (7, true) => b"expr.parse_close_step",
                    (8, true) => b"expr.parse_apply_comma_valid",
                    (7 | 8, false) => b"expr.parse_apply_syntax",
                    _ => return Err("guest parser route did not match its token".into()),
                }
            } else {
                match kind {
                    9 => b"expr.parse_apply_operand".as_slice(),
                    _ => b"expr.parse_apply_syntax",
                }
            };
            let binary_route = action == b"expr.parse_binary_route".as_slice();
            let close_step = action == b"expr.parse_close_step".as_slice();
            if !binary_route && !close_step {
                report = self.phase(action)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if let Some(outcome) = self.parser_fault(&report)? {
                    return Ok(outcome);
                }
            }
            if binary_route {
                let mut reduced = false;
                for _ in 0..=expression.len() {
                    report = self.phase(action)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if let Some(outcome) = self.parser_route_fault(&report)? {
                        return Ok(outcome);
                    }
                    let pop = checked_parser_route_bool(&report.json)?;
                    report = self.phase(b"expr.parse_route_take")?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !pop {
                        reduced = true;
                        break;
                    }
                    report = self.phase(b"expr.parse_binary_pop")?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if let Some(outcome) = self.parser_state_fault(&report)? {
                        return Ok(outcome);
                    }
                }
                if !reduced {
                    return Err(
                        "guest parser precedence reduction exceeded its source-byte bound".into(),
                    );
                }
                report = self.phase(b"expr.parse_apply_open_checked")?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if let Some(outcome) = self.parser_fault(&report)? {
                    return Ok(outcome);
                }
            }
            if close_step {
                let mut reduced = false;
                for _ in 0..=expression.len() {
                    // phase() keeps exactly one parser cell over the hidden
                    // State; pair the step flag so the step stays one-in,
                    // one-out, then expose it with quotation-free words.
                    report = self.phase(b"expr.parse_close_step pair")?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    report = self.execute(b"unpair", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    let more = checked_guest_bool(&report.json)?;
                    report = self.execute(b"drop", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if let Some(outcome) = self.parser_state_fault(&report)? {
                        return Ok(outcome);
                    }
                    if !more {
                        reduced = true;
                        break;
                    }
                }
                if !reduced {
                    return Err(
                        "guest parser close reduction exceeded its source-byte bound".into(),
                    );
                }
                report = self.phase(b"expr.parse_close_finish")?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if let Some(outcome) = self.parser_fault(&report)? {
                    return Ok(outcome);
                }
            }
        }
        if !parsed {
            return Err("guest parser exceeded its source-byte progress bound".into());
        }
        if is_function {
            report = self.execute(b"[ [ unpair ] dip ] dip", &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            return self.install_function(expression.len(), budget[5]);
        }
        if let Some(failed) = self.flush_deferred()? {
            return Ok(Outcome::NativeFailure(failed.json));
        }
        if self.stack.len() != 3 {
            return Err("calculator parser returned an unexpected typed stack".into());
        }
        // A let carries its checked declaration Name inside the hidden
        // evaluator cell, K = Pair<Pair<State,Bool>,Result<Name,Unit>>, so no
        // stage ever types over it; only Ctx-consuming phases unpack K.
        report = self.execute(
            if is_let {
                b"[ swap unpair calc_bits@1.rot swap [ pair ] dip pair ] dip".as_slice()
            } else {
                b"[ pair ] dip"
            },
            &[],
            None,
        )?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        report = self.execute(b"expr.frame_from_parse", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        if let Some(diagnostic) = checked_frame_error(&report.json)? {
            return Ok(Outcome::Diagnostic(diagnostic));
        }
        // Isolate the original State/display context from the diagnostic
        // codec specialization: only the checked FrameRun enters the quote.
        report = self.execute(b"swap [ expr.frame_hot_begin ] dip swap", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        // The shell never decodes a token or number. It only schedules the
        // guest-certified action kind and retains all opaque phase cells in
        // this worker's typed stack.
        // RPN actions plus two source-byte passes (digit admission and exact
        // decimal materialization) each make independently checked progress.
        let mut remaining = (budget[5] as u64).saturating_add(
            (expression.len() as u64)
                .saturating_mul(3)
                .saturating_add(1),
        );
        // A route is a source-bound guest variant; call sites interpret its
        // first arm according to that route's checked Done/More declaration.
        let guest_span_route_left = |report: &str| -> Result<bool, String> {
            let json: serde_json::Value =
                serde_json::from_str(report).map_err(|error| error.to_string())?;
            let outer = json["stack"]
                .as_array()
                .and_then(|stack| stack.last())
                .ok_or("guest numeric route returned no checked outcome")?;
            let (ready, payload) = variant(outer)?;
            if !ready {
                return Err("guest numeric route was not certified".into());
            }
            let (_, route) = pair(payload)?;
            variant(route).map(|(left, _)| left)
        };
        let guest_eval_route_left = |report: &str| -> Result<bool, String> {
            let json: serde_json::Value =
                serde_json::from_str(report).map_err(|error| error.to_string())?;
            let outer = json["stack"]
                .as_array()
                .and_then(|stack| stack.last())
                .ok_or("guest arithmetic route returned no checked outcome")?;
            let (ready, payload) = variant(outer)?;
            if !ready {
                return Err("guest arithmetic route was not certified".into());
            }
            let (_, progress) = pair(payload)?;
            let (_, route) = pair(progress)?;
            variant(route).map(|(left, _)| left)
        };
        let guest_numeric_bool = |report: &str| -> Result<bool, String> {
            let json: serde_json::Value =
                serde_json::from_str(report).map_err(|error| error.to_string())?;
            let outer = json["stack"]
                .as_array()
                .and_then(|stack| stack.last())
                .ok_or("guest numeric probe returned no checked outcome")?;
            let (ready, payload) = carrier_variant(outer)?;
            if !ready {
                return Err("guest numeric probe was not certified".into());
            }
            let (_, progress) = pair(payload)?;
            let (_, flag) = pair(progress)?;
            if flag["type"] != "Bool" {
                return Err("guest numeric probe returned a non-Bool progress flag".into());
            }
            flag["value"]
                .as_bool()
                .ok_or("guest numeric probe returned an invalid progress flag".into())
        };
        let guest_numeric_i64 = |report: &str| -> Result<i64, String> {
            let json: serde_json::Value =
                serde_json::from_str(report).map_err(|error| error.to_string())?;
            let outer = json["stack"]
                .as_array()
                .and_then(|stack| stack.last())
                .ok_or("guest numeric comparison returned no checked outcome")?;
            let (ready, payload) = carrier_variant(outer)?;
            if !ready {
                return Err("guest numeric comparison was not certified".into());
            }
            let (_, progress) = pair(payload)?;
            let (_, value) = pair(progress)?;
            if value["type"] != "I64" {
                return Err("guest numeric comparison returned a non-I64 selector".into());
            }
            value["value"]
                .as_str()
                .ok_or("guest numeric comparison returned an invalid selector")?
                .parse::<i64>()
                .map_err(|error| error.to_string())
        };
        let guest_power_square_done = |report: &str| -> Result<bool, String> {
            let json: serde_json::Value =
                serde_json::from_str(report).map_err(|error| error.to_string())?;
            let last = json["stack"]
                .as_array()
                .and_then(|stack| stack.last())
                .ok_or("guest square route returned no checked continuation")?;
            let (ready, payload) = carrier_variant(last)?;
            if !ready {
                return Err("guest square route lost its original pending frame".into());
            }
            let (_, progress) = pair(payload)?;
            let (decision, _) = pair(progress)?;
            variant(decision).map(|(done, _)| done)
        };
        macro_rules! numeric_stage {
            ($name:literal) => {
                concat!(
                    "[ pair ] dip swap [ [ unpair num.",
                    $name,
                    " pair result@1.Result.Ok ] [ result@1.Result.Err ] ",
                    "result@1.Result.match ] dip swap [ unpair ] dip",
                )
                .as_bytes()
            };
        }
        // Unlike a literal's Span, Pending consumes the sole FrameHot when
        // made. The retained context (and, for let, the original declaration
        // Name) precedes its checked outcome; never invent a second hot frame.
        // Every quotation's concrete stack counts toward the compiler's
        // per-binding type-size bound, so Numeric runs with Pending dipped
        // away instead of beneath its own quotations.
        macro_rules! eval_stage {
            ($name:literal) => {
                concat!(
                    "swap [ [ unpair swap [ num.",
                    $name,
                    " ] dip swap pair result@1.Result.Ok ] [ result@1.Result.Err ] ",
                    "result@1.Result.match ] dip swap",
                )
                .as_bytes()
            };
        }
        // Binary operators keep their operator Span directly under the
        // carrier (see the swap after make_pending); hide it as well.
        macro_rules! span_stage {
            ($name:literal) => {
                concat!(
                    "swap [ swap [ [ unpair swap [ num.",
                    $name,
                    " ] dip swap pair result@1.Result.Ok ] [ result@1.Result.Err ] ",
                    "result@1.Result.match ] dip swap ] dip swap",
                )
                .as_bytes()
            };
        }
        const EVAL_DROP_BOOL: &[u8] = b"swap [ [ unpair unpair drop pair result@1.Result.Ok ] \
              [ result@1.Result.Err ] result@1.Result.match ] dip swap";
        // Schoolbook products and the exponent loop carry large private
        // cursors. A generic Result counts its payload twice in the
        // compiler's type-size bound, so these stages carry the same
        // Pending/cursor pair in a builtin Sum (left = ready, right = the
        // original Unit failure) and convert back once the payload is small.
        const TO_SUM: &[u8] = b"swap [ [ inl ] [ inr ] result@1.Result.match ] dip swap";
        const FROM_SUM: &[u8] =
            b"swap [ [ result@1.Result.Ok ] [ result@1.Result.Err ] case ] dip swap";
        macro_rules! sum_stage {
            ($name:literal) => {
                concat!(
                    "swap [ [ unpair swap [ num.",
                    $name,
                    " ] dip swap pair inl ] [ inr ] case ] dip swap",
                )
                .as_bytes()
            };
        }
        // Numeric's sealed schoolbook stages can reject a mismatched cursor.
        macro_rules! sum_checked_stage {
            ($name:literal) => {
                concat!(
                    "swap [ [ unpair swap [ num.",
                    $name,
                    " ] dip swap [ pair inl ] [ drop drop unit inr ] ",
                    "result@1.Result.match ] [ inr ] case ] dip swap",
                )
                .as_bytes()
            };
        }
        const SUM_DROP_BOOL: &[u8] =
            b"swap [ [ unpair unpair drop pair inl ] [ inr ] case ] dip swap";
        // The second pair member is an authentic Budget or a retained
        // accumulator/Budget pair; only Numeric's private cursor advances.
        macro_rules! sum_power_stage {
            ($name:literal) => {
                concat!(
                    "swap [ [ unpair unpair [ swap [ num.",
                    $name,
                    " ] dip swap ] dip pair pair inl ] [ inr ] case ] dip swap",
                )
                .as_bytes()
            };
        }
        macro_rules! sum_power_checked_stage {
            ($name:literal) => {
                concat!(
                    "swap [ [ unpair unpair [ swap [ num.",
                    $name,
                    " ] dip swap ] dip swap [ swap pair pair inl ] ",
                    "[ drop drop drop unit inr ] result@1.Result.match ] ",
                    "[ inr ] case ] dip swap",
                )
                .as_bytes()
            };
        }
        // A cursor probe leaves its guest Bool beside, not inside, the
        // carrier: [ carrier Bool Ctx ]. The failure arm's false is never
        // read because the carrier is checked first (checked_probe_flag).
        // SUM_PROBE_DONE then drops the flag and restores [ Ctx carrier ].
        macro_rules! sum_power_probe {
            ($name:literal) => {
                concat!(
                    "swap [ [ unpair unpair [ swap [ num.",
                    $name,
                    " ] dip swap ] dip [ unpair ] dip swap [ pair pair inl ] dip ] ",
                    "[ inr false ] case ] dip",
                )
                .as_bytes()
            };
        }
        const SUM_PROBE_DONE: &[u8] = b"[ drop ] dip swap";
        const EVAL_BOOL_ACCEPT: &[u8] = b"swap [ [ unpair unpair \
              [ drop drop unit result@1.Result.Err ] \
              [ pair result@1.Result.Ok ] if ] \
              [ result@1.Result.Err ] result@1.Result.match ] dip swap";
        macro_rules! eval_bool_deny {
            ($name:literal) => {
                concat!(
                    "swap [ [ unpair unpair ",
                    "[ drop unit num.",
                    $name,
                    " pair result@1.Result.Ok ] ",
                    "[ drop drop unit result@1.Result.Err ] if ] ",
                    "[ result@1.Result.Err ] result@1.Result.match ] dip swap",
                )
                .as_bytes()
            };
        }
        const GCD_REMAINDER_DONE: &[u8] = b"[ pair ] dip swap [ [ unpair \
              [ drop drop unit result@1.Result.Err ] \
              [ pair result@1.Result.Ok ] \
              num.GcdRemainderProgress.match ] \
              [ result@1.Result.Err ] result@1.Result.match ] \
              dip swap [ unpair ] dip";
        macro_rules! eval_binary_begin {
            ($name:literal) => {
                concat!(
                    "swap [ [ unpair unpair [ unpair ] dip num.",
                    $name,
                    " pair result@1.Result.Ok ] ",
                    "[ result@1.Result.Err ] result@1.Result.match ] dip swap",
                )
                .as_bytes()
            };
        }
        const EVAL_ROUTE_ACCEPT: &[u8] =
            b"swap [ [ unpair unpair [ drop pair result@1.Result.Ok ] \
              [ drop drop drop unit result@1.Result.Err ] result@1.Result.match ] \
              [ result@1.Result.Err ] result@1.Result.match ] dip swap";
        macro_rules! eval_numeric_deny {
            ($name:literal) => {
                concat!(
                    "swap [ [ unpair unpair ",
                    "[ drop drop drop unit result@1.Result.Err ] ",
                    "[ swap drop num.",
                    $name,
                    " pair result@1.Result.Ok ] ",
                    "result@1.Result.match ] [ result@1.Result.Err ] ",
                    "result@1.Result.match ] dip swap",
                )
                .as_bytes()
            };
        }
        macro_rules! power_route_gate {
            ($deny:literal, $label:literal) => {{
                let admitted = guest_eval_route_left(&report.json)?;
                report = self.execute(
                    if admitted {
                        EVAL_ROUTE_ACCEPT
                    } else {
                        eval_numeric_deny!($deny)
                    },
                    &[],
                    None,
                )?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if !admitted {
                    report = self.execute(
                        b"swap [ expr.arithmetic_numeric_denial_guest ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    let diagnostic = checked_error(&report.json)?.ok_or($label)?;
                    return Ok(Outcome::Diagnostic(diagnostic));
                }
                if !checked_ready(&report.json)? {
                    return Err($label.into());
                }
            }};
        }
        macro_rules! power_bool_gate {
            ($deny:literal, $label:literal) => {{
                let denied = guest_numeric_bool(&report.json)?;
                report = self.execute(
                    if denied {
                        eval_bool_deny!($deny)
                    } else {
                        EVAL_BOOL_ACCEPT
                    },
                    &[],
                    None,
                )?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if denied {
                    report = self.execute(
                        b"swap [ expr.arithmetic_numeric_denial_guest ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    let diagnostic = checked_error(&report.json)?.ok_or($label)?;
                    return Ok(Outcome::Diagnostic(diagnostic));
                }
                if !checked_ready(&report.json)? {
                    return Err($label.into());
                }
            }};
        }
        const PENDING_BOOL_ACCEPT: &[u8] = b"[ pair ] dip swap [ [ unpair unpair \
              [ drop drop unit result@1.Result.Err ] \
              [ pair result@1.Result.Ok ] if ] \
              [ result@1.Result.Err ] result@1.Result.match ] dip swap [ unpair ] dip";
        macro_rules! pending_bool_deny {
            ($name:literal) => {
                concat!(
                    "[ pair ] dip swap [ [ unpair unpair ",
                    "[ drop unit num.",
                    $name,
                    " pair result@1.Result.Ok ] ",
                    "[ drop drop unit result@1.Result.Err ] if ] ",
                    "[ result@1.Result.Err ] result@1.Result.match ] ",
                    "dip swap [ unpair ] dip",
                )
                .as_bytes()
            };
        }
        const DECIMAL_STATUS_TRUE: &[u8] =
            b"swap [ [ unpair unpair [ drop pair result@1.Result.Ok ] \
              [ drop drop drop unit result@1.Result.Err ] result@1.Result.match ] \
              [ result@1.Result.Err ] result@1.Result.match ] dip swap";
        const DECIMAL_STATUS_FALSE: &[u8] =
            b"swap [ [ unpair unpair [ drop drop drop unit result@1.Result.Err ] \
              [ drop pair result@1.Result.Ok ] result@1.Result.match ] \
              [ result@1.Result.Err ] result@1.Result.match ] dip swap";
        const SPAN_INNER_ERR: &[u8] =
            b"swap [ swap [ [ unpair [ drop drop unit result@1.Result.Err ] \
              [ pair result@1.Result.Ok ] result@1.Result.match ] \
              [ result@1.Result.Err ] result@1.Result.match ] dip swap ] dip swap";
        // Preserve the guest's original span and NumericError while the
        // source-bound diagnostic conversion runs; Rust never decodes either.
        let literal_numeric_denial =
            |session: &mut Self| -> Result<crate::core::output::Report, String> {
                let report = session.execute(SPAN_INNER_ERR, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(report);
                }
                session.execute(
                    b"swap [ expr.literal_numeric_denial_finish_guest ] dip swap",
                    &[],
                    None,
                )
            };
        let literal_direct_denial =
            |session: &mut Self| -> Result<crate::core::output::Report, String> {
                session.execute(
                    b"swap [ expr.literal_numeric_denial_finish_guest ] dip swap",
                    &[],
                    None,
                )
            };
        // Carrier: Sum<Pair<Pending,OpFirst*>,Unit> (see TO_SUM).
        let signed_product = |session: &mut Self,
                              _entry: crate::core::output::Report,
                              remaining: &mut u64|
         -> Result<crate::core::output::Report, String> {
            let mut current =
                session.execute(sum_checked_stage!("op_first_stage_begin"), &[], None)?;
            if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                return Ok(current);
            }
            loop {
                for source in [
                    sum_checked_stage!("op_first_stage_route"),
                    sum_checked_stage!("op_first_stage_route_probe"),
                ] {
                    current = session.execute(source, &[], None)?;
                    if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                        return Ok(current);
                    }
                }
                let done = guest_numeric_bool(&current.json)?;
                current = session.execute(SUM_DROP_BOOL, &[], None)?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                if done {
                    return session.execute(sum_checked_stage!("op_first_stage_finish"), &[], None);
                }
                current =
                    session.execute(sum_checked_stage!("op_first_stage_add_probe"), &[], None)?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                let needs_add = guest_numeric_bool(&current.json)?;
                current = session.execute(SUM_DROP_BOOL, &[], None)?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                if needs_add {
                    current = session.execute(
                        sum_checked_stage!("op_first_stage_add_begin"),
                        &[],
                        None,
                    )?;
                    if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                        return Ok(current);
                    }
                    loop {
                        for source in [
                            sum_checked_stage!("op_first_stage_add_digit"),
                            sum_checked_stage!("op_first_stage_add_more"),
                        ] {
                            current = session.execute(source, &[], None)?;
                            if current.outcome != "normal" || !checked_carrier_ready(&current.json)?
                            {
                                return Ok(current);
                            }
                        }
                        let more = guest_numeric_bool(&current.json)?;
                        current = session.execute(SUM_DROP_BOOL, &[], None)?;
                        if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                            return Ok(current);
                        }
                        if !more {
                            break;
                        }
                        if *remaining == 0 {
                            return Err(
                                "guest signed schoolbook addition exceeded its progress bound"
                                    .into(),
                            );
                        }
                        *remaining -= 1;
                    }
                    current = session.execute(
                        sum_checked_stage!("op_first_stage_add_finish"),
                        &[],
                        None,
                    )?;
                } else {
                    current = session.execute(
                        sum_checked_stage!("op_first_stage_add_keep"),
                        &[],
                        None,
                    )?;
                }
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                current = session.execute(sum_checked_stage!("op_first_stage_shift"), &[], None)?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                if *remaining == 0 {
                    return Err(
                        "guest signed schoolbook multiplication exceeded its progress bound".into(),
                    );
                }
                *remaining -= 1;
            }
        };
        // Carrier: Sum<Pair<Pending,Pair<PowMul*,retained>>,Unit>. The
        // addition step is staged like the signed product's; one monolithic
        // accumulate exceeds the compiler's per-prepare Work ceiling.
        let power_product = |session: &mut Self,
                             _entry: crate::core::output::Report,
                             remaining: &mut u64|
         -> Result<crate::core::output::Report, String> {
            let mut current;
            loop {
                current = session.execute(sum_power_stage!("pow_mul_classify"), &[], None)?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                current = session.execute(sum_power_probe!("pow_mul_probe"), &[], None)?;
                if current.outcome != "normal" {
                    return Ok(current);
                }
                let more = checked_probe_flag(&current.json)?;
                current = session.execute(SUM_PROBE_DONE, &[], None)?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                if !more {
                    return session.execute(
                        sum_power_checked_stage!("pow_mul_select_done"),
                        &[],
                        None,
                    );
                }
                current = session.execute(
                    sum_power_checked_stage!("pow_mul_select_active"),
                    &[],
                    None,
                )?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                current = session.execute(sum_power_probe!("pow_mul_add_probe"), &[], None)?;
                if current.outcome != "normal" {
                    return Ok(current);
                }
                let needs_add = checked_probe_flag(&current.json)?;
                current = session.execute(SUM_PROBE_DONE, &[], None)?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                if needs_add {
                    current = session.execute(sum_power_stage!("pow_mul_add_begin"), &[], None)?;
                    if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                        return Ok(current);
                    }
                    loop {
                        current =
                            session.execute(sum_power_stage!("pow_mul_add_digit"), &[], None)?;
                        if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                            return Ok(current);
                        }
                        current =
                            session.execute(sum_power_probe!("pow_mul_add_more"), &[], None)?;
                        if current.outcome != "normal" {
                            return Ok(current);
                        }
                        let more = checked_probe_flag(&current.json)?;
                        current = session.execute(SUM_PROBE_DONE, &[], None)?;
                        if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                            return Ok(current);
                        }
                        if !more {
                            break;
                        }
                        if *remaining == 0 {
                            return Err(
                                "guest exponent schoolbook addition exceeded its progress bound"
                                    .into(),
                            );
                        }
                        *remaining -= 1;
                    }
                    current = session.execute(sum_power_stage!("pow_mul_add_finish"), &[], None)?;
                } else {
                    current = session.execute(sum_power_stage!("pow_mul_add_keep"), &[], None)?;
                }
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                current = session.execute(sum_power_stage!("pow_mul_shift"), &[], None)?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
                if *remaining == 0 {
                    return Err(
                        "guest exponent schoolbook multiplication exceeded its progress bound"
                            .into(),
                    );
                }
                *remaining -= 1;
            }
        };
        let power_acc_product = |session: &mut Self,
                                 mut current: crate::core::output::Report,
                                 remaining: &mut u64|
         -> Result<crate::core::output::Report, String> {
            current = power_product(session, current, remaining)?;
            if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                return Ok(current);
            }
            for source in [
                sum_power_checked_stage!("pow_mul_finish_numerator"),
                sum_power_stage!("pow_acc_den_begin"),
            ] {
                current = session.execute(source, &[], None)?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
            }
            current = power_product(session, current, remaining)?;
            if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                return Ok(current);
            }
            for source in [
                sum_power_checked_stage!("pow_mul_finish_denominator"),
                sum_power_stage!("pow_acc_finish"),
            ] {
                current = session.execute(source, &[], None)?;
                if current.outcome != "normal" || !checked_carrier_ready(&current.json)? {
                    return Ok(current);
                }
            }
            Ok(current)
        };
        const LITERAL_NUMERIC_ACCEPT: &[u8] =
            b"swap [ swap [ [ unpair [ pair result@1.Result.Ok ] \
              [ drop drop unit result@1.Result.Err ] result@1.Result.match ] \
              [ result@1.Result.Err ] result@1.Result.match ] dip swap ] dip swap";
        // A call never parks its caller frame on the typed stack: during a
        // callee the worker holds exactly [ Ctx FrameHot ]. The guest stashes
        // the caller Text and remaining tokens in a State-held Function and
        // restores the exact caller State from it when the body completes.
        // A let's hidden cell K = Pair<Pair<State,Bool>,Result<Name,Unit>> is
        // reshaped to Pair<State,Pair<Bool,Sum<Name,Unit>>> around each call
        // phase, so every call stage types over Pair<State,X> either way.
        const CALL_LET_ENTER: &[u8] = b"[ [ unpair [ inl ] [ inr ] result@1.Result.match \
              swap unpair calc_bits@1.rot pair pair ] dip ] dip";
        const CALL_LET_EXIT: &[u8] = b"[ unpair unpair [ pair ] dip \
              [ result@1.Result.Ok ] [ result@1.Result.Err ] case pair ] dip";
        const CALL_STATUS: &[u8] =
            b"swap [ expr.call_status_guest ] dip calc_bits@1.rot calc_bits@1.rot";
        let mut calls = 0usize;
        loop {
            report = self.execute(b"expr.frame_hot_failed", &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            let json: serde_json::Value =
                serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
            let probe = json["stack"]
                .as_array()
                .and_then(|stack| stack.last())
                .ok_or("guest hot frame returned no checked failure flag")?;
            let (_, failed) = pair(probe)?;
            if failed["type"] != "Bool" {
                return Err("guest hot frame returned a non-Bool failure flag".into());
            }
            let failed = failed["value"]
                .as_bool()
                .ok_or("guest hot frame returned an invalid failure flag")?;
            self.defer_consume(b"unpair drop");
            if failed {
                report = self.execute(b"expr.frame_hot_error", &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                // Only this rejected line loses its guest State; submit then
                // restores the commit. The compact failure stays in the
                // worker for typed rehydration.
                report = self.execute(b"swap drop", &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                return self.decode_compact_diagnostic();
            }
            if remaining == 0 {
                return Err("guest evaluator exceeded its declared progress bound".into());
            }
            remaining -= 1;
            report = self.execute(b"expr.frame_peek_kind", &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            let kind = checked_guest_opcode(&report.json)?;
            if kind == 0 {
                if calls != 0 {
                    // The completed body returns through its stash: the exact
                    // caller State, Text and tokens resume over exactly one
                    // result pushed above the caller's own values.
                    if is_let {
                        report = self.execute(CALL_LET_ENTER, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                    }
                    for source in [
                        b"drop [ unpair [ expr.call_body_finish_guest ] dip swap [ pair ] dip ] dip swap"
                            .as_slice(),
                        b"calc_bits@1.rot [ expr.call_return_resume_guest ] dip \
                          calc_bits@1.rot calc_bits@1.rot",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                    }
                    if !checked_guest_bool(&report.json)? {
                        return Err("guest call return lost its stashed caller frame".into());
                    }
                    self.defer_consume(b"drop");
                    if is_let {
                        report = self.execute(CALL_LET_EXIT, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                    }
                    calls -= 1;
                    continue;
                }
                report = self.execute(b"drop expr.frame_finish_begin", &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                report = self.execute(b"expr.finish_extract", &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if !checked_ready(&report.json)? {
                    return Err("guest calculator completed evaluator projection rejected".into());
                }
                for word in [
                    b"expr.finish_first".as_slice(),
                    b"expr.finish_rest",
                    b"swap [ expr.finish_values ] dip swap",
                ] {
                    report = self.execute(word, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                }
                break;
            }
            report = self.execute(b"drop expr.frame_token_take", &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            if !checked_ready(&report.json)? {
                return Err("guest calculator checked evaluator token projection rejected".into());
            }
            if kind == 10 {
                // State-backed variable lookup. The variable list is lent
                // beside the token Name, and the State/display context and
                // FrameHot travel as one register on top, so each guest search
                // stage types over [ Vars Name ] or [ Search ] only
                // (HOST-PROTOCOL I1/I2/I5: probes are read under the register).
                for source in [
                    b"pair expr.frame_name_begin".as_slice(),
                    if is_let {
                        b"[ dup unpair drop unpair drop calc_env@1.state_variables ] dip unpair \
                          [ swap ] dip [ [ pair ] dip ] dip calc_bits@1.rot"
                            .as_slice()
                    } else {
                        b"[ dup unpair drop calc_env@1.state_variables ] dip unpair \
                          [ swap ] dip [ [ pair ] dip ] dip calc_bits@1.rot"
                    },
                    b"[ calc_env@1.variable_lookup_begin ] dip",
                ] {
                    report = self.execute(source, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                }
                loop {
                    report = self.execute(b"[ calc_env@1.variable_lookup_step ] dip", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    let more = checked_register_probe_bool(&report.json)?;
                    report = self.execute(b"[ drop ] dip", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !more {
                        break;
                    }
                    if remaining == 0 {
                        return Err(
                            "guest variable lookup exceeded its declared progress bound".into()
                        );
                    }
                    remaining -= 1;
                }
                report = self.execute(b"[ calc_env@1.variable_lookup_status ] dip", &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                let status = checked_register_probe_opcode(&report.json)?;
                report = self.execute(b"[ drop ] dip", &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                // VariableSearch.match: Found -> 0, exhausted Missing -> 1.
                let resolve: [&[u8]; 2] = match status {
                    0 => [
                        b"[ calc_env@1.variable_lookup_value ] dip unpair calc_bits@1.rot",
                        b"expr.eval_name unpair",
                    ],
                    1 => [
                        b"[ calc_env@1.variable_lookup_missing \
                          calc_env@1.variable_lookup_missing_failure ] dip unpair calc_bits@1.rot",
                        // A failed lookup ends evaluation; a let's Name is no
                        // longer needed and is dropped with this line's State.
                        if is_let {
                            b"[ [ unpair drop unpair swap ] dip ] dip expr.frame_name_missing_guest"
                        } else {
                            b"[ [ unpair swap ] dip ] dip expr.frame_name_missing_guest"
                        },
                    ],
                    _ => return Err("guest variable lookup lost its source-bound cursor".into()),
                };
                for source in resolve {
                    report = self.execute(source, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                }
                if !checked_guest_bool(&report.json)? {
                    return Err("guest name resolution rejected its checked result".into());
                }
                report = self.execute(
                    if status == 0 {
                        b"drop".as_slice()
                    } else {
                        b"drop calc_bits@1.rot swap [ pair ] dip"
                    },
                    &[],
                    None,
                )?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            } else if kind >= 100 {
                let arity = u64::try_from(kind - 100)
                    .map_err(|_| "guest call arity exceeded the host progress bound")?;
                if arity > remaining {
                    return Err("guest call arity exceeded the evaluator progress bound".into());
                }
                remaining -= arity;
                if is_let {
                    report = self.execute(CALL_LET_ENTER, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                }
                // Stages type over [ Reg Carrier ] with Reg = Pair<Ctx,FrameHot>:
                // each hides the register and checks its small carrier alone.
                // Every refusal ends in one guest exit to [ Ctx FrameHot ]; a
                // source failure is then reported by the failed-frame check.
                let exit: &[u8] = 'call: {
                    report = self.execute(
                        b"pair swap [ expr.frame_call_name_begin ] dip swap unpair \
                          calc_bits@1.rot calc_bits@1.rot pair swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest call token projection was not source-certified".into());
                    }
                    report = self.execute(
                        b"[ unpair ] dip calc_bits@1.rot [ expr.call_args_pop_guest ] dip \
                          swap [ swap pair ] dip",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    // A missing RPN argument has already failed the caller frame.
                    if !checked_carrier_ready(&report.json)? {
                        break 'call b"drop unpair".as_slice();
                    }
                    report = self.execute(
                        b"swap dup unpair drop unpair drop calc_env@1.state_functions swap \
                          [ swap expr.call_lookup_open_guest ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    loop {
                        report = self.execute(
                            b"swap [ expr.call_lookup_tick_guest unpair ] dip calc_bits@1.rot calc_bits@1.rot",
                            &[], None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let more = checked_guest_bool(&report.json)?;
                        self.defer_consume(b"drop");
                        if !more {
                            break;
                        }
                        if remaining == 0 {
                            return Err("guest function lookup exceeded its progress bound".into());
                        }
                        remaining -= 1;
                    }
                    for source in [
                        b"swap [ expr.call_lookup_resolve_guest ] dip swap".as_slice(),
                        CALL_STATUS,
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                    }
                    let status = checked_guest_opcode(&report.json)?;
                    self.defer_consume(b"drop");
                    match status {
                        0 => {}
                        // UnknownName or WrongArity at the call Name's span.
                        1 => break 'call b"[ unpair ] dip calc_bits@1.rot [ expr.call_fail_guest ] dip swap"
                            .as_slice(),
                        _ => return Err("guest function lookup lost its original call witness".into()),
                    }
                    report = self.execute(
                        b"[ dup unpair swap drop expr.call_frame_budget ] dip calc_bits@1.rot \
                          [ expr.call_depth_guest ] dip calc_bits@1.rot calc_bits@1.rot",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    let status = checked_guest_opcode(&report.json)?;
                    self.defer_consume(b"drop");
                    match status {
                        0 => {}
                        1 => break 'call b"[ unpair ] dip calc_bits@1.rot [ expr.call_depth_fail_guest ] dip swap"
                            .as_slice(),
                        _ => return Err("guest call depth admission lost its caller Budget".into()),
                    }
                    for source in [
                        b"[ unpair ] dip calc_bits@1.rot [ expr.call_enter_frame_guest ] dip \
                          swap [ swap pair ] dip".as_slice(),
                        b"[ unpair ] dip swap [ [ unpair ] dip swap [ expr.call_enter_stash_guest ] dip \
                          swap [ pair ] dip ] dip swap [ pair ] dip",
                        b"swap dup unpair drop unpair drop swap [ swap expr.call_enter_scope_guest ] dip swap",
                        b"swap [ expr.call_enter_bind_guest ] dip swap",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                    }
                    // Bind every body identifier with its value in the
                    // function's lexical scope, one guest token at a time.
                    loop {
                        report = self.execute(
                            b"swap [ expr.call_scan_peek ] dip calc_bits@1.rot calc_bits@1.rot",
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let status = checked_guest_opcode(&report.json)?;
                        self.defer_consume(b"drop");
                        match status {
                            0 => {
                                report = self.execute(
                                    b"swap [ expr.call_scan_resolve_guest ] dip calc_bits@1.rot calc_bits@1.rot",
                                    &[], None,
                                )?;
                                if report.outcome != "normal" {
                                    return Ok(Outcome::NativeFailure(report.json));
                                }
                                let resolved = checked_guest_opcode(&report.json)?;
                                self.defer_consume(b"drop");
                                if resolved != 0 {
                                    return Err("guest function body named a variable outside its lexical scope".into());
                                }
                                report = self.execute(
                                    b"unpair swap [ expr.call_scan_bind_guest ] dip",
                                    &[],
                                    None,
                                )?;
                            }
                            1 => break,
                            2 => report = self.execute(b"unpair drop", &[], None)?,
                            _ => {
                                return Err("guest function body scan lost its lexical scope".into())
                            }
                        }
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if remaining == 0 {
                            return Err(
                                "guest function body scan exceeded its progress bound".into()
                            );
                        }
                        remaining -= 1;
                    }
                    calls += 1;
                    b"drop unpair".as_slice()
                };
                report = self.execute(exit, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if is_let {
                    report = self.execute(CALL_LET_EXIT, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                }
                continue;
            } else {
                if !matches!(kind, 1..=5 | 9 | 12 | 13) {
                    return Err("guest evaluator returned an unsupported action kind".into());
                }
                if kind != 9 {
                    if (1..=4).contains(&kind) {
                        report = self.execute(b"expr.frame_operator_span_guest", &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                    }
                    // Arithmetic owns the sole evaluator frame in Pending.
                    // The preceding FrameHot/TokenTake pair is consumed by
                    // the guest, not retained as a duplicate hot frame.
                    report = self.execute(b"expr.frame_token_pending_guest", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest arithmetic token lost its active frame".into());
                    }
                    report = self.execute(b"expr.make_pending", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err(
                            "guest calculator checked arithmetic pending projection rejected"
                                .into(),
                        );
                    }
                    if (1..=4).contains(&kind) {
                        // Park the small operator Span beneath the State/display
                        // context: each `swap [ stage ] dip swap` then hides the
                        // 54-node context instead of typing every quotation over
                        // it. The same swap restores the order before
                        // arithmetic_raw_from_span_guest consumes the Span.
                        report = self.execute(b"[ swap ] dip", &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                    }
                }
                if (1..=5).contains(&kind) {
                    report = self.execute(b"expr.binary_operand_first", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest calculator first operand projection rejected".into());
                    }
                    report = self.execute(b"expr.binary_operand_right", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest calculator first value projection rejected".into());
                    }
                    let right_ready = checked_numeric_phase(&report.json)?;
                    report = self.execute(
                        if right_ready {
                            b"expr.binary_operand_right_ready"
                        } else {
                            b"expr.binary_operand_right_error"
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !right_ready {
                        let diagnostic = checked_error(&report.json)?.ok_or(
                            "guest first operand failure lacked a typed syntax diagnostic",
                        )?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest binary right operand projection rejected".into());
                    }
                    // A let's hidden cell carries its Name; these two words type
                    // the whole stack, so the span and cell are lifted away.
                    report = self.execute(
                        if is_let {
                            b"swap [ swap [ expr.binary_second_begin_guest ] dip swap ] dip swap"
                                .as_slice()
                        } else {
                            b"expr.binary_second_begin_guest"
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest second operand lost its original pending frame".into());
                    }
                    report = self.execute(
                        if is_let {
                            b"swap [ swap [ expr.binary_second_route_guest ] dip swap ] dip swap"
                                .as_slice()
                        } else {
                            b"expr.binary_second_route_guest"
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    let json: serde_json::Value =
                        serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
                    let routed = json["stack"]
                        .as_array()
                        .and_then(|stack| stack.last())
                        .ok_or("guest second operand returned no checked route")?;
                    let (_, ready) = pair(routed)?;
                    if ready["type"] != "Bool" {
                        return Err("guest second operand returned a non-Bool route".into());
                    }
                    let second_ready = ready["value"]
                        .as_bool()
                        .ok_or("guest second operand returned an invalid route")?;
                    self.defer_consume(b"unpair drop");
                    if !second_ready {
                        for source in [
                            b"expr.binary_second_error_phase_guest".as_slice(),
                            b"swap [ [ expr.binary_second_failure_guest result@1.Result.Err ] \
                              [ drop unit result@1.Result.Ok ] result@1.Result.match ] dip swap",
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                        }
                        let diagnostic = checked_error(&report.json)?
                            .ok_or("guest second operand failure lacked its syntax diagnostic")?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    for source in [
                        b"swap [ expr.binary_second_assemble_guest ] dip swap".as_slice(),
                        b"swap [ [ expr.arithmetic_operand_input_guest \
                          result@1.Result.Ok ] [ result@1.Result.Err ] \
                          result@1.Result.match ] dip swap",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err(
                                "guest arithmetic operands lost their original candidate".into()
                            );
                        }
                    }
                    if kind != 5 {
                        if kind == 4 {
                            for source in [
                                span_stage!("op_divisor_probe"),
                                span_stage!("op_divisor_route"),
                            ] {
                                report = self.execute(source, &[], None)?;
                                if report.outcome != "normal" {
                                    return Ok(Outcome::NativeFailure(report.json));
                                }
                                if !checked_ready(&report.json)? {
                                    return Err("guest division operand was not certified".into());
                                }
                            }
                            let admitted = guest_eval_route_left(&report.json)?;
                            report = self.execute(
                                if admitted {
                                    EVAL_ROUTE_ACCEPT
                                } else {
                                    eval_numeric_deny!("op_divisor_deny")
                                },
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !admitted {
                                report = self.execute(
                                    b"swap [ expr.arithmetic_numeric_denial_guest ] dip swap",
                                    &[],
                                    None,
                                )?;
                                if report.outcome != "normal" {
                                    return Ok(Outcome::NativeFailure(report.json));
                                }
                                let diagnostic = checked_error(&report.json)?
                                .ok_or("guest division by zero lacked its original operator diagnostic")?;
                                return Ok(Outcome::Diagnostic(diagnostic));
                            }
                            report = self.execute(span_stage!("op_divisor_admit"), &[], None)?;
                        } else {
                            report = self.execute(
                                match kind {
                                    1 => eval_binary_begin!("op_begin_add"),
                                    2 => eval_binary_begin!("op_begin_sub"),
                                    3 => eval_binary_begin!("op_begin_mul"),
                                    _ => unreachable!("binary operator is 1 through 4"),
                                },
                                &[],
                                None,
                            )?;
                        }
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest binary operation input was not certified".into());
                        }
                        for source in [
                            span_stage!("op_bits_measure_left"),
                            span_stage!("op_bits_measure_right"),
                            span_stage!("op_bits_limit"),
                            span_stage!("op_bits_probe"),
                            span_stage!("op_bits_route"),
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err("guest binary width quota was not certified".into());
                            }
                        }
                        let admitted = guest_eval_route_left(&report.json)?;
                        report = self.execute(
                            if admitted {
                                EVAL_ROUTE_ACCEPT
                            } else {
                                eval_numeric_deny!("op_bits_deny")
                            },
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !admitted {
                            report = self.execute(
                                b"swap [ expr.arithmetic_numeric_denial_guest ] dip swap",
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            let diagnostic = checked_error(&report.json)?
                                .ok_or("guest binary width denial lacked an operator diagnostic")?;
                            return Ok(Outcome::Diagnostic(diagnostic));
                        }
                        for source in [
                            span_stage!("op_bits_admit"),
                            span_stage!("op_work_limit"),
                            span_stage!("op_work_probe"),
                            span_stage!("op_work_route"),
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err("guest binary work quota was not certified".into());
                            }
                        }
                        let admitted = guest_eval_route_left(&report.json)?;
                        report = self.execute(
                            if admitted {
                                EVAL_ROUTE_ACCEPT
                            } else {
                                eval_numeric_deny!("op_work_deny")
                            },
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !admitted {
                            report = self.execute(
                                b"swap [ expr.arithmetic_numeric_denial_guest ] dip swap",
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            let diagnostic = checked_error(&report.json)?
                                .ok_or("guest binary work denial lacked an operator diagnostic")?;
                            return Ok(Outcome::Diagnostic(diagnostic));
                        }
                        for source in [span_stage!("op_work_admit"), span_stage!("op_work_commit")]
                        {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err(
                                    "guest binary admission did not retain its original candidate"
                                        .into(),
                                );
                            }
                        }
                    }
                }
                if (1..=4).contains(&kind) {
                    report = self.execute(
                        b"swap [ [ unpair expr.arithmetic_binary_opcode_guest ] \
                          [ result@1.Result.Err ] result@1.Result.match ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err(
                            "guest signed opcode did not match its original operator".into()
                        );
                    }
                    report = self.execute(TO_SUM, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_carrier_ready(&report.json)? {
                        return Err("guest signed opcode lost its original operator".into());
                    }
                    report = self.execute(
                        if kind == 3 {
                            sum_checked_stage!("op_first_stage_seed_mul")
                        } else {
                            sum_checked_stage!("op_first_stage_seed_cross")
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_carrier_ready(&report.json)? {
                        return Err(
                            "guest signed product seed rejected its admitted operand".into()
                        );
                    }
                    report = signed_product(self, report, &mut remaining)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_carrier_ready(&report.json)? {
                        return Err("guest first signed product lost its schoolbook cursor".into());
                    }
                    report = self.execute(
                        if kind == 3 {
                            sum_checked_stage!("op_first_stage_second_mul")
                        } else {
                            sum_checked_stage!("op_first_stage_second_cross")
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_carrier_ready(&report.json)? {
                        return Err("guest second signed product rejected original factors".into());
                    }
                    report = signed_product(self, report, &mut remaining)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_carrier_ready(&report.json)? {
                        return Err("guest second signed product lost its schoolbook cursor".into());
                    }
                    if kind == 1 || kind == 2 {
                        for source in [
                            if kind == 1 {
                                sum_checked_stage!("op_first_stage_sum_prepare_add")
                            } else {
                                sum_checked_stage!("op_first_stage_sum_prepare_sub")
                            },
                            sum_checked_stage!("op_first_stage_sum_begin"),
                            sum_checked_stage!("op_first_stage_sum_route_probe"),
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_carrier_ready(&report.json)? {
                                return Err(
                                    "guest signed sum rejected its original products".into()
                                );
                            }
                        }
                        // The guest's own BigAddRoute selects the finish; a
                        // mismatched finish rejects the cursor.
                        let adds = guest_numeric_bool(&report.json)?;
                        for source in [
                            SUM_DROP_BOOL,
                            if adds {
                                sum_checked_stage!("op_first_stage_sum_finish_add")
                            } else {
                                sum_checked_stage!("op_first_stage_sum_finish_sub")
                            },
                            sum_checked_stage!("op_first_stage_sum_den_mul"),
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_carrier_ready(&report.json)? {
                                return Err(
                                    "guest signed sum rejected its original products".into()
                                );
                            }
                        }
                        report = signed_product(self, report, &mut remaining)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_carrier_ready(&report.json)? {
                            return Err(
                                "guest signed denominator product lost its schoolbook cursor"
                                    .into(),
                            );
                        }
                    }
                    report = self.execute(
                        match kind {
                            1 | 2 => sum_checked_stage!("op_first_stage_sum_raw"),
                            3 => sum_checked_stage!("op_first_stage_mul_raw"),
                            4 => sum_checked_stage!("op_first_stage_div_raw"),
                            _ => unreachable!("signed operator kind is 1 through 4"),
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_carrier_ready(&report.json)? {
                        return Err("guest signed RawRat rejected its original factors".into());
                    }
                    report = self.execute(FROM_SUM, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest signed RawRat rejected its original factors".into());
                    }
                    // Consume the parked Span with the context (for a let, the
                    // Name-bearing context cell) dipped above the guest: its
                    // diagnostic arm is large, so it compiles with nothing
                    // beneath it.
                    // [ Span Ctx carrier ] -> [ FH Ctx Result ] is the literal
                    // layout that the shared RawRat normalization tail
                    // (raw_rat_push_guest) consumes.
                    report = self.execute(
                        b"swap [ expr.arithmetic_raw_from_span_guest ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest signed RawRat lost its original operator span".into());
                    }
                }
                if kind == 5 {
                    for source in [
                        b"swap [ [ expr.arithmetic_power_opcode_guest ] \
                          [ result@1.Result.Err ] result@1.Result.match ] dip swap"
                            .as_slice(),
                        b"swap [ [ unpair unpair [ unpair ] dip num.pow_input \
                          pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                          result@1.Result.match ] dip swap",
                        eval_stage!("pow_integral_bool"),
                        eval_stage!("pow_integral_route"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest power operand lost its original operator".into());
                        }
                    }
                    power_route_gate!(
                        "pow_integral_reject",
                        "guest nonintegral exponent lacked its operator diagnostic"
                    );
                    for source in [
                        eval_stage!("pow_integral_admit"),
                        eval_stage!("pow_zero_sign"),
                        eval_stage!("pow_zero_base"),
                        eval_stage!("pow_zero_route"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err(
                                "guest zero-negative power domain lost its original operands"
                                    .into(),
                            );
                        }
                    }
                    power_route_gate!(
                        "pow_zero_reject",
                        "guest zero-negative exponent lacked its operator diagnostic"
                    );
                    for source in [
                        eval_stage!("pow_zero_admit"),
                        eval_stage!("pow_base_num"),
                        eval_stage!("pow_base_den"),
                        eval_stage!("pow_exponent_bound"),
                        eval_stage!("pow_exponent_bits"),
                        eval_stage!("pow_exponent_length_exp"),
                        eval_stage!("pow_exponent_length_bound"),
                        eval_stage!("pow_exponent_length_compare"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err(
                                "guest exponent width bound lost its sealed operands".into()
                            );
                        }
                    }
                    match guest_numeric_i64(&report.json)? {
                        0 => {
                            report = self.execute(EVAL_DROP_BOOL, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            for source in [
                                eval_stage!("pow_exponent_compare_parts"),
                                eval_stage!("pow_exponent_compare_exp"),
                                eval_stage!("pow_exponent_compare_bound"),
                                eval_stage!("pow_exponent_compare_probe"),
                            ] {
                                report = self.execute(source, &[], None)?;
                                if report.outcome != "normal" {
                                    return Ok(Outcome::NativeFailure(report.json));
                                }
                                if !checked_ready(&report.json)? {
                                    return Err(
                                        "guest equal-length exponent comparison was not certified"
                                            .into(),
                                    );
                                }
                            }
                        }
                        -1 | 1 => {
                            // Convert the *guest's* ordering code to a guest
                            // denial Bool; the host supplies no exponent cap.
                            report = self.execute(
                                b"swap [ [ unpair unpair 1 = pair pair \
                                  result@1.Result.Ok ] [ result@1.Result.Err ] \
                                  result@1.Result.match ] dip swap",
                                &[],
                                None,
                            )?;
                        }
                        _ => return Err("guest exponent length comparison was not ordered".into()),
                    }
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    report = self.execute(eval_stage!("pow_exponent_route"), &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest exponent limit route was not certified".into());
                    }
                    power_route_gate!(
                        "pow_exponent_deny",
                        "guest exponent limit denial lacked its operator diagnostic"
                    );
                    for source in [
                        eval_stage!("pow_exponent_admit"),
                        eval_stage!("pow_width_exponent"),
                        eval_stage!("pow_width_base"),
                        eval_stage!("pow_width_measure"),
                        eval_stage!("pow_bits_probe"),
                        eval_stage!("pow_bits_route"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err(
                                "guest power Bits allowance was not source-certified".into()
                            );
                        }
                    }
                    power_route_gate!(
                        "pow_bits_deny",
                        "guest power Bits denial lacked its operator diagnostic"
                    );
                    for source in [
                        eval_stage!("pow_bits_measure_debit"),
                        eval_stage!("pow_bits_commit_debit"),
                        eval_stage!("pow_work_bounded"),
                        eval_stage!("pow_work_limit_probe"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err(
                                "guest power Work quotient allowance was not certified".into()
                            );
                        }
                    }
                    power_bool_gate!(
                        "pow_work_deny",
                        "guest power Work limit lacked its operator diagnostic"
                    );
                    report = self.execute(eval_stage!("pow_work_square_probe"), &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest power quadratic Work allowance was not certified".into());
                    }
                    power_bool_gate!(
                        "pow_work_deny",
                        "guest power quadratic Work denial lacked its operator diagnostic"
                    );
                    for source in [
                        eval_stage!("pow_work_admit"),
                        eval_stage!("pow_work_commit"),
                        eval_stage!("pow_materialize"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err(
                                "guest power Budget reservation lost its original operands".into(),
                            );
                        }
                    }
                    // The exponent loop carries Sum<Pair<Pending,Pair<PowState,Budget>>,Unit>.
                    report = self.execute(TO_SUM, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_carrier_ready(&report.json)? {
                        return Err(
                            "guest power Budget reservation lost its original operands".into()
                        );
                    }
                    loop {
                        for source in [
                            b"swap [ [ unpair swap [ unpair num.pow_bit_route ] dip swap \
                              pair inl ] [ inr ] case ] dip swap"
                                .as_slice(),
                            sum_stage!("pow_route_kind"),
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_carrier_ready(&report.json)? {
                                return Err(
                                    "guest exponent bit route lost its sealed PowState".into()
                                );
                            }
                        }
                        let route = guest_numeric_i64(&report.json)?;
                        report = self.execute(SUM_DROP_BOOL, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_carrier_ready(&report.json)? {
                            return Err("guest exponent branch lost its source-bound Budget".into());
                        }
                        match route {
                            0 => {
                                for source in
                                    [sum_checked_stage!("pow_route_select_done"), FROM_SUM]
                                {
                                    report = self.execute(source, &[], None)?;
                                    if report.outcome != "normal" {
                                        return Ok(Outcome::NativeFailure(report.json));
                                    }
                                    if !checked_carrier_ready(&report.json)? {
                                        return Err(
                                            "guest exponent terminal Rat was not certified".into(),
                                        );
                                    }
                                }
                                for source in [
                                    b"expr.arithmetic_finished_rat_guest".as_slice(),
                                    b"swap [ expr.resume_finished_rat ] dip swap",
                                ] {
                                    report = self.execute(source, &[], None)?;
                                    if report.outcome != "normal" {
                                        return Ok(Outcome::NativeFailure(report.json));
                                    }
                                }
                                break;
                            }
                            1 | 2 => {
                                report = self.execute(
                                    if route == 1 {
                                        sum_checked_stage!("pow_route_select_acc")
                                    } else {
                                        sum_checked_stage!("pow_route_select_square")
                                    },
                                    &[],
                                    None,
                                )?;
                                if report.outcome != "normal" {
                                    return Ok(Outcome::NativeFailure(report.json));
                                }
                                if !checked_carrier_ready(&report.json)? {
                                    return Err(
                                        "guest exponent branch selector rejected its actual bit"
                                            .into(),
                                    );
                                }
                            }
                            _ => {
                                return Err(
                                    "guest exponent returned an unknown bit-route arm".into()
                                )
                            }
                        }
                        if route == 1 {
                            report =
                                self.execute(sum_power_stage!("pow_acc_num_begin"), &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_carrier_ready(&report.json)? {
                                return Err(
                                    "guest accumulator numerator cursor was not certified".into()
                                );
                            }
                            report = power_acc_product(self, report, &mut remaining)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_carrier_ready(&report.json)? {
                                return Err(
                                    "guest accumulator denominator cursor was not certified".into(),
                                );
                            }
                        }
                        report = self.execute(sum_power_stage!("pow_square_route"), &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_carrier_ready(&report.json)? {
                            return Err("guest terminal square route lost its PowState".into());
                        }
                        // Select the guest's own square-route arm; the other
                        // arm never invents a PowState or PowSquareInput.
                        if guest_power_square_done(&report.json)? {
                            report = self.execute(
                                b"swap [ [ unpair unpair swap \
                                  [ swap pair pair inl ] [ drop drop drop unit inr ] \
                                  result@1.Result.match ] [ inr ] case ] dip swap",
                                &[],
                                None,
                            )?;
                        } else {
                            report = self.execute(
                                b"swap [ [ unpair unpair swap \
                                  [ drop drop drop unit inr ] [ swap pair pair inl ] \
                                  result@1.Result.match ] [ inr ] case ] dip swap",
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_carrier_ready(&report.json)? {
                                return Err(
                                    "guest pending square lost its original PowSquareInput".into(),
                                );
                            }
                            report = self.execute(
                                sum_power_checked_stage!("pow_square_begin"),
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_carrier_ready(&report.json)? {
                                return Err(
                                    "guest square was not admitted from remaining exponent bits"
                                        .into(),
                                );
                            }
                            report = self.execute(
                                b"swap [ [ unpair swap [ unpair [ unpair ] dip [ swap ] dip \
                                  [ [ num.pow_acc_num_begin ] dip ] dip pair pair ] dip swap \
                                  pair inl ] [ inr ] case ] dip swap",
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_carrier_ready(&report.json)? {
                                return Err(
                                    "guest square numerator cursor lost its accumulator".into()
                                );
                            }
                            report = power_acc_product(self, report, &mut remaining)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_carrier_ready(&report.json)? {
                                return Err(
                                    "guest squared base lost its original accumulator".into()
                                );
                            }
                            report = self.execute(
                                b"swap [ [ unpair swap [ unpair unpair \
                                  [ swap pair num.pow_square_finish ] dip pair ] dip swap \
                                  pair inl ] [ inr ] case ] dip swap",
                                &[],
                                None,
                            )?;
                        }
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_carrier_ready(&report.json)? {
                            return Err(
                                "guest exponent square did not preserve its original Budget".into(),
                            );
                        }
                        if remaining == 0 {
                            return Err(
                                "guest exponentiation exceeded its declared progress bound".into(),
                            );
                        }
                        remaining -= 1;
                    }
                }
                if kind == 12 || kind == 13 {
                    for source in [
                        b"expr.binary_operand_first".as_slice(),
                        b"expr.binary_operand_right",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest unary operand projection was not certified".into());
                        }
                    }
                    let ready = checked_numeric_phase(&report.json)?;
                    report = self.execute(
                        if ready {
                            b"expr.binary_operand_right_ready"
                        } else {
                            b"expr.binary_operand_right_error"
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !ready {
                        let diagnostic = checked_error(&report.json)?.ok_or(
                            "guest missing unary operand lacked an original syntax diagnostic",
                        )?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    for source in [
                        b"swap [ expr.arithmetic_unary_ready_guest ] dip swap".as_slice(),
                        b"swap [ [ expr.arithmetic_unary_input_guest \
                          result@1.Result.Ok ] [ result@1.Result.Err ] \
                          result@1.Result.match ] dip swap",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest unary operand lost its original candidate".into());
                        }
                    }
                    if kind == 12 {
                        for source in [
                            eval_stage!("plus_work_limit"),
                            eval_stage!("plus_work_probe"),
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err("guest unary plus Work gate was not certified".into());
                            }
                        }
                        let denied = guest_numeric_bool(&report.json)?;
                        report = self.execute(
                            if denied {
                                eval_bool_deny!("gcd_work_deny")
                            } else {
                                EVAL_BOOL_ACCEPT
                            },
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if denied {
                            report = self.execute(
                                b"swap [ expr.arithmetic_numeric_denial_guest ] dip swap",
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            let diagnostic = checked_error(&report.json)?.ok_or(
                                "guest unary plus Work denial lacked an operator diagnostic",
                            )?;
                            return Ok(Outcome::Diagnostic(diagnostic));
                        }
                        for source in [
                            eval_stage!("plus_work_admit"),
                            b"swap [ expr.arithmetic_finished_rat_guest ] dip swap".as_slice(),
                            b"swap [ expr.resume_finished_rat ] dip swap",
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err("guest unary plus result was not certified".into());
                            }
                        }
                    } else {
                        for source in [
                            b"swap [ [ unpair unpair num.neg_measure_num \
                              pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                              result@1.Result.match ] dip swap"
                                .as_slice(),
                            eval_stage!("neg_measure_den"),
                            eval_stage!("neg_bits_probe"),
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err("guest unary negation width was not certified".into());
                            }
                        }
                        let denied = guest_numeric_bool(&report.json)?;
                        report = self.execute(
                            if denied {
                                eval_bool_deny!("gcd_bits_deny")
                            } else {
                                EVAL_BOOL_ACCEPT
                            },
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if denied {
                            report = self.execute(
                                b"swap [ expr.arithmetic_numeric_denial_guest ] dip swap",
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            let diagnostic = checked_error(&report.json)?.ok_or(
                                "guest unary negation width denial lacked an operator diagnostic",
                            )?;
                            return Ok(Outcome::Diagnostic(diagnostic));
                        }
                        for source in [eval_stage!("neg_work_limit"), eval_stage!("neg_work_probe")]
                        {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err(
                                    "guest unary negation Work quota was not certified".into()
                                );
                            }
                        }
                        let denied = guest_numeric_bool(&report.json)?;
                        report = self.execute(
                            if denied {
                                eval_bool_deny!("gcd_work_deny")
                            } else {
                                EVAL_BOOL_ACCEPT
                            },
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if denied {
                            report = self.execute(
                                b"swap [ expr.arithmetic_numeric_denial_guest ] dip swap",
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            let diagnostic = checked_error(&report.json)?.ok_or(
                                "guest unary negation Work denial lacked an operator diagnostic",
                            )?;
                            return Ok(Outcome::Diagnostic(diagnostic));
                        }
                        for source in [
                            eval_stage!("neg_work_admit"),
                            eval_stage!("neg_work_commit"),
                            eval_stage!("neg_materialize"),
                            b"swap [ expr.arithmetic_finished_rat_guest ] dip swap".as_slice(),
                            b"swap [ expr.resume_finished_rat ] dip swap",
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err("guest unary negation result was not certified".into());
                            }
                        }
                    }
                }
                if kind == 9 {
                    report = self.execute(
                        b"pair swap [ unpair expr.frame_literal_seed ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    report =
                        self.execute(b"swap [ expr.literal_span_pair_guest ] dip swap", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    for source in [
                        b"swap [ expr.literal_input_guest ] dip swap".as_slice(),
                        b"swap [ expr.literal_count_parts_guest ] dip swap",
                        b"swap [ expr.literal_count_begin_guest ] dip swap",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest calculator literal input projection rejected".into());
                        }
                    }
                    loop {
                        report = self.execute(
                            b"swap [ expr.literal_count_probe_guest ] dip swap",
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let json: serde_json::Value = serde_json::from_str(&report.json)
                            .map_err(|error| error.to_string())?;
                        let probe = json["stack"]
                            .as_array()
                            .and_then(|stack| stack.last())
                            .ok_or("guest literal count returned no checked cursor")?;
                        let (ready, payload) = variant(probe)?;
                        if !ready {
                            return Err("guest literal count cursor was not certified".into());
                        }
                        let (_, done) = pair(payload)?;
                        if done["type"] != "Bool" {
                            return Err(
                                "guest literal count returned a non-Bool progress flag".into()
                            );
                        }
                        let done = done["value"]
                            .as_bool()
                            .ok_or("guest literal count returned an invalid progress flag")?;
                        report = self.execute(
                            b"swap [ [ num.parse_count_route result@1.Result.Ok ] \
                              [ result@1.Result.Err ] result@1.Result.match ] dip swap",
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest literal count route was not certified".into());
                        }
                        if done {
                            report = self.execute(
                                b"swap [ [ [ result@1.Result.Ok ] \
                                  [ drop unit result@1.Result.Err ] result@1.Result.match ] \
                                  [ result@1.Result.Err ] result@1.Result.match ] dip swap",
                                &[],
                                None,
                            )?;
                            break;
                        }
                        if remaining == 0 {
                            return Err(
                                "guest literal count exceeded its source-byte progress bound"
                                    .into(),
                            );
                        }
                        remaining -= 1;
                        for source in [
                            b"swap [ [ [ drop unit result@1.Result.Err ] \
                              [ result@1.Result.Ok ] result@1.Result.match ] \
                              [ result@1.Result.Err ] result@1.Result.match ] dip swap"
                                .as_slice(),
                            b"swap [ expr.literal_count_byte_guest ] dip swap",
                            b"swap [ expr.literal_count_classify_guest ] dip swap",
                            b"swap [ expr.literal_count_advance_guest ] dip swap",
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err("guest literal count advance was not certified".into());
                            }
                        }
                    }
                    for source in [
                        b"swap [ expr.literal_count_span_guest ] dip swap".as_slice(),
                        b"swap [ expr.literal_digit_measure_guest ] dip swap",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err(
                                "guest numeric digit admission input was not certified".into()
                            );
                        }
                    }
                    // Keep the source span in the guest while the measured
                    // digit quota is checked. The wide combined check exceeds
                    // the compiler's physical specialization limit.
                    for source in [
                        b"swap [ expr.literal_digit_check_guest ] dip swap".as_slice(),
                        b"swap [ swap [ unpair [ num.parse_digit_check result@1.Result.Ok ] \
                          [ result@1.Result.Err ] result@1.Result.match pair ] dip swap ] dip swap",
                        b"swap [ swap [ unpair [ swap [ swap pair result@1.Result.Ok ] \
                          [ drop drop unit result@1.Result.Err ] result@1.Result.match ] \
                          [ drop drop unit result@1.Result.Err ] result@1.Result.match ] \
                          dip swap ] dip swap",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest numeric digit admission input was not certified".into());
                    }
                    let accepted = checked_numeric_phase(&report.json)?;
                    report = if accepted {
                        self.execute(LITERAL_NUMERIC_ACCEPT, &[], None)?
                    } else {
                        literal_numeric_denial(self)?
                    };
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !accepted {
                        let diagnostic = checked_error(&report.json)?
                            .ok_or("guest numeric digit denial lacked a typed diagnostic")?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    report = self.execute(
                        b"swap [ expr.literal_digit_commit_guest ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest numeric digit charge was not certified".into());
                    }
                    // Admit the estimated width before allocating digits,
                    // then charge the genuine work budget before the decimal
                    // cursor can append a digit.
                    for source in [
                        b"swap [ swap [ [ unpair num.parse_reserve_bounds pair result@1.Result.Ok ] \
                          [ result@1.Result.Err ] result@1.Result.match ] dip swap ] dip swap".as_slice(),
                        b"swap [ swap [ [ unpair num.parse_reserve_bits pair result@1.Result.Ok ] \
                          [ result@1.Result.Err ] result@1.Result.match ] dip swap ] dip swap",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let accepted = checked_numeric_phase(&report.json)?;
                        report = if accepted {
                            self.execute(LITERAL_NUMERIC_ACCEPT, &[], None)?
                        } else {
                            literal_numeric_denial(self)?
                        };
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !accepted {
                            let diagnostic = checked_error(&report.json)?
                                .ok_or("guest numeric failure lacked a typed diagnostic")?;
                            return Ok(Outcome::Diagnostic(diagnostic));
                        }
                    }
                    for source in [
                        b"swap [ [ unpair num.parse_work_limit pair result@1.Result.Ok ] \
                          [ result@1.Result.Err ] result@1.Result.match ] dip swap".as_slice(),
                        b"[ pair ] dip swap [ [ unpair num.parse_work_check pair result@1.Result.Ok ] \
                          [ result@1.Result.Err ] result@1.Result.match ] dip swap [ unpair ] dip",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest numeric Work input was not certified".into());
                        }
                    }
                    let work_admitted = guest_span_route_left(&report.json)?;
                    if work_admitted {
                        for source in [
                            b"swap [ [ unpair [ pair result@1.Result.Ok ] \
                              [ drop drop unit result@1.Result.Err ] result@1.Result.match ] \
                              [ result@1.Result.Err ] result@1.Result.match ] dip swap"
                                .as_slice(),
                            b"swap [ [ unpair num.parse_work_commit pair result@1.Result.Ok ] \
                              [ result@1.Result.Err ] result@1.Result.match ] dip swap",
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err(
                                    "guest numeric Work commitment was not certified".into()
                                );
                            }
                        }
                    } else {
                        report = self.execute(
                            b"swap [ [ unpair [ drop drop unit result@1.Result.Err ] \
                              [ pair result@1.Result.Ok ] result@1.Result.match ] \
                              [ result@1.Result.Err ] result@1.Result.match ] dip swap",
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        report = self.execute(
                            b"swap [ expr.literal_numeric_denial_finish_guest ] dip swap",
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let diagnostic = checked_error(&report.json)?
                            .ok_or("guest numeric Work denial lacked a typed diagnostic")?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    report = self.execute(
                        b"swap [ expr.literal_decimal_begin_guest ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest decimal cursor was not certified".into());
                    }
                    loop {
                        report = self.execute(numeric_stage!("parse_decimal_route"), &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if guest_span_route_left(&report.json)? {
                            for source in [
                                LITERAL_NUMERIC_ACCEPT,
                                b"swap [ expr.literal_decimal_finish_guest ] dip swap",
                            ] {
                                report = self.execute(source, &[], None)?;
                                if report.outcome != "normal" {
                                    return Ok(Outcome::NativeFailure(report.json));
                                }
                                if !checked_ready(&report.json)? {
                                    return Err(
                                        "guest exact decimal materialization was not certified"
                                            .into(),
                                    );
                                }
                            }
                            break;
                        }
                        if remaining == 0 {
                            return Err(
                                "guest decimal cursor exceeded its source-byte progress bound"
                                    .into(),
                            );
                        }
                        remaining -= 1;
                        for source in [SPAN_INNER_ERR, numeric_stage!("parse_decimal_classify")] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err(
                                    "guest decimal cursor classification was not certified".into(),
                                );
                            }
                        }
                        let dot = guest_span_route_left(&report.json)?;
                        report = self.execute(
                            if dot {
                                LITERAL_NUMERIC_ACCEPT
                            } else {
                                SPAN_INNER_ERR
                            },
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest decimal source byte was not certified".into());
                        }
                        if dot {
                            continue;
                        }
                        report =
                            self.execute(numeric_stage!("parse_decimal_digit_begin"), &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest decimal digit bits were not certified".into());
                        }
                        loop {
                            for source in [
                                numeric_stage!("parse_decimal_digit_run"),
                                numeric_stage!("parse_decimal_digit_seal"),
                                b"swap [ [ unpair [ pair result@1.Result.Ok ] \
                                  [ drop drop unit result@1.Result.Err ] result@1.Result.match ] \
                                  [ result@1.Result.Err ] result@1.Result.match ] dip swap"
                                    .as_slice(),
                            ] {
                                report = self.execute(source, &[], None)?;
                                if report.outcome != "normal" {
                                    return Ok(Outcome::NativeFailure(report.json));
                                }
                                if !checked_ready(&report.json)? {
                                    return Err(
                                        "guest decimal digit did not seal its finite bits".into()
                                    );
                                }
                            }
                            report = self.execute(
                                numeric_stage!("parse_decimal_digit_finish_probe"),
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            let denominator_pass = guest_numeric_bool(&report.json)?;
                            report = self.execute(
                                numeric_stage!("parse_decimal_digit_finish_route"),
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            for source in if denominator_pass {
                                [
                                    DECIMAL_STATUS_TRUE,
                                    numeric_stage!("parse_decimal_digit_finish_true"),
                                ]
                            } else {
                                [
                                    DECIMAL_STATUS_FALSE,
                                    numeric_stage!("parse_decimal_digit_finish_false_begin"),
                                ]
                            } {
                                report = self.execute(source, &[], None)?;
                                if report.outcome != "normal" {
                                    return Ok(Outcome::NativeFailure(report.json));
                                }
                                if !checked_ready(&report.json)? {
                                    return Err(
                                        "guest decimal digit completion was not certified".into()
                                    );
                                }
                            }
                            if denominator_pass {
                                break;
                            }
                            report = self.execute(
                                numeric_stage!("parse_decimal_digit_finish_false_probe"),
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            let needs_denominator = guest_numeric_bool(&report.json)?;
                            report = self.execute(
                                numeric_stage!("parse_decimal_digit_finish_false_route"),
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            for source in if needs_denominator {
                                [
                                    DECIMAL_STATUS_TRUE,
                                    numeric_stage!("parse_decimal_digit_finish_false_den"),
                                ]
                            } else {
                                [
                                    DECIMAL_STATUS_FALSE,
                                    numeric_stage!("parse_decimal_digit_finish_false_ready"),
                                ]
                            } {
                                report = self.execute(source, &[], None)?;
                                if report.outcome != "normal" {
                                    return Ok(Outcome::NativeFailure(report.json));
                                }
                                if !checked_ready(&report.json)? {
                                    return Err(
                                        "guest fractional denominator was not certified".into()
                                    );
                                }
                            }
                            if !needs_denominator {
                                break;
                            }
                        }
                    }
                }
                if kind == 9 || (1..=4).contains(&kind) {
                    for source in [
                        b"swap [ expr.raw_gcd_measure_guest ] dip swap".as_slice(),
                        b"swap [ expr.raw_gcd_bits_limit_guest ] dip swap",
                        numeric_stage!("gcd_bits_probe"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest exact rational GCD input was not certified".into());
                        }
                    }
                    let denied = guest_numeric_bool(&report.json)?;
                    report = self.execute(
                        if denied {
                            pending_bool_deny!("gcd_bits_deny")
                        } else {
                            PENDING_BOOL_ACCEPT
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if denied {
                        report = literal_direct_denial(self)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let diagnostic = checked_error(&report.json)?
                            .ok_or("guest GCD width denial lacked a typed diagnostic")?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    report = self.execute(numeric_stage!("gcd_bits_admit"), &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest GCD width admission lost its original source".into());
                    }
                    for source in [
                        b"swap [ expr.raw_gcd_work_limit_guest ] dip swap".as_slice(),
                        numeric_stage!("gcd_work_probe"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest GCD work admission input was not certified".into());
                        }
                    }
                    let denied = guest_numeric_bool(&report.json)?;
                    report = self.execute(
                        if denied {
                            pending_bool_deny!("gcd_work_deny")
                        } else {
                            PENDING_BOOL_ACCEPT
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if denied {
                        report = literal_direct_denial(self)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let diagnostic = checked_error(&report.json)?
                            .ok_or("guest GCD work denial lacked a typed diagnostic")?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    report = self.execute(numeric_stage!("gcd_work_admit"), &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest GCD Work admission lost its original source".into());
                    }
                    report = self.execute(
                        b"swap [ expr.raw_gcd_commit_work_guest ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest GCD work commitment was not certified".into());
                    }
                    // The unit-GCD certificate is the heaviest raw stage: run it
                    // with FrameHot and Ctx parked above, not beneath its nodes.
                    report = self.execute(
                        b"[ pair ] dip swap [ expr.raw_gcd_one_probe_guest ] dip swap [ unpair ] dip",
                        &[], None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    let json: serde_json::Value =
                        serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
                    let probe = json["stack"]
                        .as_array()
                        .and_then(|stack| stack.last())
                        .ok_or("guest GCD one probe returned no checked outcome")?;
                    let (ready, payload) = variant(probe)?;
                    if !ready {
                        return Err("guest GCD one probe was not certified".into());
                    }
                    let (_, progress) = pair(payload)?;
                    let (_, one) = pair(progress)?;
                    if one["type"] != "Bool" {
                        return Err("guest GCD one probe returned a non-Bool progress flag".into());
                    }
                    let one = one["value"]
                        .as_bool()
                        .ok_or("guest GCD one probe returned an invalid progress flag")?;
                    // The route re-certifies the unit GCD inside the guest; hide
                    // the hot frame too so its quotations type without it.
                    report = self.execute(
                        b"swap [ swap [ expr.raw_gcd_one_route_guest ] dip swap ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest GCD one route was not certified".into());
                    }
                    if !one {
                        report = self.execute(
                            b"swap [ expr.raw_gcd_iterate_guest ] dip swap",
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest GCD cursor was not certified".into());
                        }
                        loop {
                            if remaining == 0 {
                                return Err(
                                    "guest GCD remainder exceeded its charged progress bound"
                                        .into(),
                                );
                            }
                            remaining -= 1;
                            report = self.execute(
                                b"swap [ expr.raw_gcd_remainder_start_guest ] dip swap",
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err("guest GCD remainder was not certified".into());
                            }
                            // This source-bound guest batch consumes the
                            // entire finite dividend while preserving its
                            // original charged Budget and lexical Span.
                            for source in [
                                numeric_stage!("gcd_remainder_run"),
                                GCD_REMAINDER_DONE,
                                numeric_stage!("gcd_remainder_end"),
                            ] {
                                report = self.execute(source, &[], None)?;
                                if report.outcome != "normal" {
                                    return Ok(Outcome::NativeFailure(report.json));
                                }
                                if !checked_ready(&report.json)? {
                                    return Err(
                                        "guest GCD remainder batch was not certified".into()
                                    );
                                }
                            }
                            report = self.execute(
                                b"swap [ expr.raw_gcd_cursor_check_guest ] dip swap",
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if guest_span_route_left(&report.json)? {
                                break;
                            }
                            report = self.execute(
                                b"swap [ expr.raw_gcd_iterate_guest ] dip swap",
                                &[],
                                None,
                            )?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err("guest next GCD cursor was not certified".into());
                            }
                        }
                    }
                    report =
                        self.execute(b"swap [ expr.raw_gcd_done_guest ] dip swap", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest canonical GCD result was not certified".into());
                    }
                    for source in [
                        b"swap [ expr.raw_num_measure_guest ] dip swap".as_slice(),
                        b"swap [ expr.raw_num_bits_limit_guest ] dip swap",
                        numeric_stage!("finish_num_bits_probe"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest numerator width input was not certified".into());
                        }
                    }
                    let denied = guest_numeric_bool(&report.json)?;
                    report = self.execute(
                        if denied {
                            pending_bool_deny!("finish_num_bits_deny")
                        } else {
                            PENDING_BOOL_ACCEPT
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if denied {
                        report = literal_direct_denial(self)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let diagnostic = checked_error(&report.json)?
                            .ok_or("guest numerator width denial lacked a typed diagnostic")?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    report = self.execute(numeric_stage!("finish_num_bits_admit"), &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err(
                            "guest numerator width admission lost its original source".into()
                        );
                    }
                    for source in [
                        b"swap [ expr.raw_num_work_limit_guest ] dip swap".as_slice(),
                        numeric_stage!("finish_num_work_probe"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest numerator work input was not certified".into());
                        }
                    }
                    let denied = guest_numeric_bool(&report.json)?;
                    report = self.execute(
                        if denied {
                            pending_bool_deny!("finish_num_work_deny")
                        } else {
                            PENDING_BOOL_ACCEPT
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if denied {
                        report = literal_direct_denial(self)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let diagnostic = checked_error(&report.json)?
                            .ok_or("guest numerator work denial lacked a typed diagnostic")?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    report = self.execute(numeric_stage!("finish_num_work_admit"), &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err(
                            "guest numerator Work admission lost its original source".into()
                        );
                    }
                    for source in [
                            b"swap [ expr.raw_num_commit_work_guest ] dip swap".as_slice(),
                            b"swap [ swap [ [ unpair num.finish_num_one_probe pair result@1.Result.Ok ] \
                              [ result@1.Result.Err ] result@1.Result.match ] dip swap ] dip swap",
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err("guest numerator quotient route was not certified".into());
                            }
                        }
                    let json: serde_json::Value =
                        serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
                    let probe = json["stack"]
                        .as_array()
                        .and_then(|stack| stack.last())
                        .ok_or("guest numerator one probe returned no checked outcome")?;
                    let (ready, payload) = variant(probe)?;
                    if !ready {
                        return Err("guest numerator one probe was not certified".into());
                    }
                    let (_, progress) = pair(payload)?;
                    let (_, num_one) = pair(progress)?;
                    if num_one["type"] != "Bool" {
                        return Err(
                            "guest numerator one probe returned a non-Bool progress flag".into(),
                        );
                    }
                    let num_one = num_one["value"]
                        .as_bool()
                        .ok_or("guest numerator one probe returned an invalid progress flag")?;
                    // A second operand's literal runs as [ Ctx FH cursor ];
                    // hide both cells so the route types over its cursor only.
                    report = self.execute(
                        b"swap [ swap [ expr.raw_num_one_route_guest ] dip swap ] dip swap",
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest numerator quotient route was not certified".into());
                    }
                    if num_one {
                        report =
                            self.execute(b"swap [ expr.raw_num_ready_guest ] dip swap", &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err(
                                "guest unit-GCD numerator quotient was not certified".into()
                            );
                        }
                    } else {
                        report = self.execute(
                            b"swap [ expr.raw_num_divide_guest ] dip swap",
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest numerator divide cursor was not certified".into());
                        }
                        if remaining == 0 {
                            return Err(
                                "guest numerator divide exceeded its charged progress bound".into(),
                            );
                        }
                        remaining -= 1;
                        for source in [
                            b"[ pair ] dip swap [ [ unpair num.finish_num_bit_batch_begin \
                                  pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                                  result@1.Result.match ] dip swap [ unpair ] dip"
                                .as_slice(),
                            b"[ pair ] dip swap [ [ unpair num.finish_num_bit_batch_step \
                                  pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                                  result@1.Result.match ] dip swap [ unpair ] dip",
                            b"[ pair ] dip swap [ [ unpair num.finish_num_bit_batch_done_direct \
                                  pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                                  result@1.Result.match ] dip swap [ unpair ] dip",
                            b"[ pair ] dip swap [ [ unpair num.finish_num_divide_bits \
                                  pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                                  result@1.Result.match ] dip swap [ unpair ] dip",
                            b"[ pair ] dip swap [ [ unpair num.finish_num_divide_materialize \
                                  pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                                  result@1.Result.match ] dip swap [ unpair ] dip",
                            b"swap [ expr.raw_num_divide_finish_guest ] dip swap",
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err(
                                    "guest numerator quotient completion was not certified".into(),
                                );
                            }
                        }
                    }
                    for source in [
                        b"swap [ expr.raw_den_measure_guest ] dip swap".as_slice(),
                        b"swap [ expr.raw_den_bits_limit_guest ] dip swap",
                        numeric_stage!("finish_den_bits_probe"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest denominator width input was not certified".into());
                        }
                    }
                    let denied = guest_numeric_bool(&report.json)?;
                    report = self.execute(
                        if denied {
                            pending_bool_deny!("finish_den_bits_deny")
                        } else {
                            PENDING_BOOL_ACCEPT
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if denied {
                        report = literal_direct_denial(self)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let diagnostic = checked_error(&report.json)?
                            .ok_or("guest denominator width denial lacked a typed diagnostic")?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    report = self.execute(numeric_stage!("finish_den_bits_admit"), &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err(
                            "guest denominator width admission lost its original source".into()
                        );
                    }
                    for source in [
                        b"swap [ expr.raw_den_work_limit_guest ] dip swap".as_slice(),
                        numeric_stage!("finish_den_work_probe"),
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest denominator work input was not certified".into());
                        }
                    }
                    let denied = guest_numeric_bool(&report.json)?;
                    report = self.execute(
                        if denied {
                            pending_bool_deny!("finish_den_work_deny")
                        } else {
                            PENDING_BOOL_ACCEPT
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if denied {
                        report = literal_direct_denial(self)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        let diagnostic = checked_error(&report.json)?
                            .ok_or("guest denominator work denial lacked a typed diagnostic")?;
                        return Ok(Outcome::Diagnostic(diagnostic));
                    }
                    report = self.execute(numeric_stage!("finish_den_work_admit"), &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err(
                            "guest denominator Work admission lost its original source".into()
                        );
                    }
                    for source in [
                        b"swap [ expr.raw_den_commit_work_guest ] dip swap".as_slice(),
                        b"swap [ expr.raw_den_one_probe_guest ] dip swap",
                    ] {
                        report = self.execute(source, &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest denominator quotient route was not certified".into());
                        }
                    }
                    let json: serde_json::Value =
                        serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
                    let probe = json["stack"]
                        .as_array()
                        .and_then(|stack| stack.last())
                        .ok_or("guest denominator one probe returned no checked outcome")?;
                    let (ready, payload) = variant(probe)?;
                    if !ready {
                        return Err("guest denominator one probe was not certified".into());
                    }
                    let (_, progress) = pair(payload)?;
                    let (_, den_one) = pair(progress)?;
                    if den_one["type"] != "Bool" {
                        return Err(
                            "guest denominator one probe returned a non-Bool progress flag".into(),
                        );
                    }
                    let den_one = den_one["value"]
                        .as_bool()
                        .ok_or("guest denominator one probe returned an invalid progress flag")?;
                    report =
                        self.execute(b"swap [ expr.raw_den_one_route_guest ] dip swap", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest denominator quotient route was not certified".into());
                    }
                    if den_one {
                        report =
                            self.execute(b"swap [ expr.raw_den_ready_guest ] dip swap", &[], None)?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err(
                                "guest unit-GCD denominator quotient was not certified".into()
                            );
                        }
                    } else {
                        report = self.execute(
                            b"swap [ expr.raw_den_divide_guest ] dip swap",
                            &[],
                            None,
                        )?;
                        if report.outcome != "normal" {
                            return Ok(Outcome::NativeFailure(report.json));
                        }
                        if !checked_ready(&report.json)? {
                            return Err("guest denominator divide cursor was not certified".into());
                        }
                        if remaining == 0 {
                            return Err(
                                "guest denominator divide exceeded its charged progress bound"
                                    .into(),
                            );
                        }
                        remaining -= 1;
                        for source in [
                            b"[ pair ] dip swap [ [ unpair num.finish_den_bit_batch_begin \
                                      pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                                      result@1.Result.match ] dip swap [ unpair ] dip"
                                .as_slice(),
                            b"[ pair ] dip swap [ [ unpair num.finish_den_bit_batch_step \
                                      pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                                      result@1.Result.match ] dip swap [ unpair ] dip",
                            b"[ pair ] dip swap [ [ unpair num.finish_den_bit_batch_done_direct \
                                      pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                                      result@1.Result.match ] dip swap [ unpair ] dip",
                            b"[ pair ] dip swap [ [ unpair num.finish_den_divide_bits \
                                      pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                                      result@1.Result.match ] dip swap [ unpair ] dip",
                            b"[ pair ] dip swap [ [ unpair num.finish_den_divide_materialize \
                                      pair result@1.Result.Ok ] [ result@1.Result.Err ] \
                                      result@1.Result.match ] dip swap [ unpair ] dip",
                            b"swap [ expr.raw_den_divide_finish_guest ] dip swap",
                        ] {
                            report = self.execute(source, &[], None)?;
                            if report.outcome != "normal" {
                                return Ok(Outcome::NativeFailure(report.json));
                            }
                            if !checked_ready(&report.json)? {
                                return Err(
                                    "guest denominator quotient completion was not certified"
                                        .into(),
                                );
                            }
                        }
                    }
                    report =
                        self.execute(b"swap [ expr.raw_rat_finish_guest ] dip swap", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    if !checked_ready(&report.json)? {
                        return Err("guest canonical rational was not certified".into());
                    }
                    report = self.execute(if is_let {
                                b"[ unpair calc_bits@1.rot calc_bits@1.rot ] dip expr.raw_rat_push_guest".as_slice()
                            } else {
                                b"expr.raw_rat_push_guest"
                            }, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    let json: serde_json::Value =
                        serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
                    let pushed = json["stack"]
                        .as_array()
                        .and_then(|stack| stack.last())
                        .ok_or("guest final rational push returned no checked outcome")?;
                    let (_, ready) = pair(pushed)?;
                    if ready["type"] != "Bool" || ready["value"].as_bool() != Some(true) {
                        return Err("guest final rational push was not certified".into());
                    }
                    report = self.execute(
                        if is_let {
                            b"unpair drop [ swap pair ] dip".as_slice()
                        } else {
                            b"unpair drop"
                        },
                        &[],
                        None,
                    )?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                }
            }
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
        }
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        let json: serde_json::Value =
            serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
        let finished = json["stack"]
            .as_array()
            .and_then(|stack| stack.last())
            .ok_or("guest finish returned no checked final value")?;
        let (valid, final_run) = variant(finished)?;
        if !valid {
            // Submit restores the committed session afterwards; discard this
            // line's State before expanding the source-bound compact diagnosis.
            report = self.execute(b"swap drop", &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            report = self.execute(
                b"[ drop unit result@1.Result.Err ] [ result@1.Result.Ok ] result@1.Result.match",
                &[],
                None,
            )?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            return self.decode_compact_diagnostic();
        }
        let (ready, payload) = variant(final_run)?;
        if !ready {
            return Ok(Outcome::Diagnostic(diagnostic(payload)?));
        }
        report = self.execute(b"expr.final_ready", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        if !checked_ready(&report.json)? {
            return Err("guest calculator final value projection rejected".into());
        }
        if is_let {
            // The guest alone turns the original checked declaration Name
            // and actual FinalReady into a proposed State. It stays
            // uncommitted: formatting must succeed before submit commits it.
            // calc_defs@1.bind_let_checked types its whole [ Name Ctx
            // FinalReady ] match beyond the backend's per-binding bound, so the
            // same guest steps run staged: copy the guest Rat out of a Ready
            // FinalReady, join it with the checked Name, and let
            // calc_env@1.bind_variable extend State. A Failed value or an
            // unchecked Name leaves State unchanged (CALC-STATE-02).
            for source in [
                b"dup [ unpair swap drop unpair drop result@1.Result.Ok ] \
                  [ drop unit result@1.Result.Err ] expr.FinalReady.match".as_slice(),
                b"[ [ unpair ] dip ] dip [ swap ] dip",
                b"swap [ swap [ pair result@1.Result.Ok ] [ drop drop unit result@1.Result.Err ] \
                  result@1.Result.match ] [ drop drop unit result@1.Result.Err ] result@1.Result.match",
            ] {
                report = self.execute(source, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
            }
            report = self.execute(
                b"swap [ swap ] dip [ unpair [ swap ] dip \
                  [ [ unpair calc_env@1.bind_variable ] [ drop ] result@1.Result.match ] dip \
                  pair ] dip",
                &[],
                None,
            )?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
        }
        report = self.execute(b"expr.format_start", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        if !checked_ready(&report.json)? {
            return Err("guest calculator final value was not available to format".into());
        }
        report = self.execute(b"expr.format_measure", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        if !checked_ready(&report.json)? {
            return Err("guest calculator formatter measurement was not available".into());
        }
        report = self.execute(b"expr.format_admit_checked", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        if let Some(diagnostic) = checked_error(&report.json)? {
            return Ok(Outcome::Diagnostic(diagnostic));
        }
        report = self.execute(b"expr.format_charge_checked", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        if let Some(diagnostic) = checked_error(&report.json)? {
            return Ok(Outcome::Diagnostic(diagnostic));
        }
        for word in [
            b"expr.format_factor_two".as_slice(),
            b"expr.format_five_seed",
            b"expr.format_five_begin",
        ] {
            report = self.execute(word, &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            if !checked_ready(&report.json)? {
                return Err("guest calculator formatting phase was not certified".into());
            }
        }
        let factor_more = |report: &str| -> Result<bool, String> {
            let json: serde_json::Value =
                serde_json::from_str(report).map_err(|error| error.to_string())?;
            let current = json["stack"]
                .as_array()
                .and_then(|stack| stack.last())
                .ok_or("guest factor cursor returned no checked progress")?;
            let (ready, span) = variant(current)?;
            if !ready {
                return Err("guest factor cursor readiness was not certified".into());
            }
            let (_, route_budget) = pair(span)?;
            let (route_progress, _) = pair(route_budget)?;
            let (_, progress) = pair(route_progress)?;
            if progress["type"] != "Bool" {
                return Err("guest factor cursor returned a non-Bool progress flag".into());
            }
            progress["value"]
                .as_bool()
                .ok_or_else(|| "guest factor cursor returned invalid progress".into())
        };
        let mut factor_steps = budget[5] as u64;
        loop {
            // factor_five_begin and factor_five_round_advance report false once
            // the odd part of the denominator is one. No five can then be
            // divided out, so no round may run: an untouched cursor's zero
            // remainder would be read as an exact division forever.
            if !factor_more(&report.json)? {
                break;
            }
            while factor_more(&report.json)? {
                if factor_steps == 0 {
                    return Err("guest factor cursor exceeded its declared progress bound".into());
                }
                factor_steps -= 1;
                report = self.execute(b"expr.format_five_tick", &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if !checked_ready(&report.json)? {
                    return Err("guest factor cursor tick was not certified".into());
                }
            }
            for word in [
                b"expr.format_five_round_check".as_slice(),
                b"expr.format_five_round_trim",
                b"expr.format_five_round_compare",
                b"expr.format_five_round_advance",
            ] {
                report = self.execute(word, &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if !checked_ready(&report.json)? {
                    return Err("guest factor cursor round was not certified".into());
                }
            }
            if factor_steps == 0 {
                return Err("guest factor cursor exceeded its declared progress bound".into());
            }
            factor_steps -= 1;
        }
        for word in [
            b"expr.format_five_finish".as_slice(),
            b"expr.format_select",
            b"expr.format_count_numerator",
            b"expr.format_count_denominator",
            b"expr.format_count_charge",
        ] {
            report = self.execute(word, &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            if !checked_ready(&report.json)? {
                return Err("guest calculator formatting phase was not certified".into());
            }
        }
        report = self.execute(b"expr.format_count_begin", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        report = self.execute(b"expr.format_count_signal", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        let mut count_steps = budget[5] as u64;
        if checked_guest_bool(&report.json)? {
            loop {
                while checked_guest_bool(&report.json)? {
                    if count_steps == 0 {
                        return Err(
                            "guest decimal-count cursor exceeded its declared progress bound"
                                .into(),
                        );
                    }
                    count_steps -= 1;
                    report = self.execute(b"expr.format_count_tick", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                    report = self.execute(b"expr.format_count_signal", &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                }
                // expr.format_count_round exceeds the 2e9 compiler Work cap:
                // count_finite_round's 10-definition closure is already near
                // it, and every State-carrying node in the same compile adds
                // more. Park Pair<State,Bool> on top in its own compile, run
                // the round under one dip, then restore the layout separately.
                for source in [
                    b"drop [ pair ] dip swap".as_slice(),
                    b"[ calc_display@1.count_finite_round ] dip",
                    b"swap [ unpair ] dip",
                ] {
                    report = self.execute(source, &[], None)?;
                    if report.outcome != "normal" {
                        return Ok(Outcome::NativeFailure(report.json));
                    }
                }
                report = self.execute(b"expr.format_count_signal", &[], None)?;
                if report.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(report.json));
                }
                if !checked_guest_bool(&report.json)? {
                    break;
                }
                if count_steps == 0 {
                    return Err(
                        "guest decimal-count cursor exceeded its declared progress bound".into(),
                    );
                }
                count_steps -= 1;
            }
        }
        report = self.execute(b"expr.format_count_finite_size", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        if !checked_ready(&report.json)? {
            return Err("guest calculator finite display size was not certified".into());
        }
        report = self.execute(b"expr.format_count_finite_charge", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        report = self.execute(b"expr.format_count_finite_resolve", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        if let Some(diagnostic) = checked_error(&report.json)? {
            return Ok(Outcome::Diagnostic(diagnostic));
        }
        for word in [
            b"expr.format_ascii_fraction".as_slice(),
            b"expr.format_ascii_quotient",
            b"expr.format_ascii_measure",
        ] {
            report = self.execute(word, &[], None)?;
            if report.outcome != "normal" {
                return Ok(Outcome::NativeFailure(report.json));
            }
            if !checked_ready(&report.json)? {
                return Err("guest calculator display construction was not certified".into());
            }
        }
        report = self.execute(b"expr.format_ascii_begin", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        report = self.execute(b"unpair", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        // ASCII fraction digit driver (CALC-09 finite decimal display). Each
        // digit runs calc_display@1 `*_with_state`/`scan_*` stages over
        // Bool Pair<State,...>, so State never sits beneath their quotations;
        // only guest-issued Bools select the next stage. The host never builds
        // a cursor, Bool or State, and an Err digit Result is refused, never
        // formatted.
        macro_rules! guest {
            ($source:expr) => {{
                let next = self.execute($source, &[], None)?;
                if next.outcome != "normal" {
                    return Ok(Outcome::NativeFailure(next.json));
                }
                next
            }};
        }
        // Bool Pair<State,X> -> Bool State X Bool: read the guest Bool, then
        // restore Bool Pair<State,X>.
        macro_rules! signal {
            () => {{
                let next = guest!(b"unpair unpair");
                let flag = checked_guest_bool(&next.json)?;
                guest!(b"drop pair");
                flag
            }};
        }
        // Bool Pair<State,Result<Route,Unit>> -> Bool Pair<State,Route>.
        macro_rules! route {
            () => {{
                guest!(b"calc_display@1.ascii_fraction_result_route_with_state");
                if !signal!() {
                    return Err("guest finite display digit returned an internal Err".into());
                }
            }};
        }
        let mut ascii_steps = budget[5] as u64;
        if checked_guest_bool(&report.json)? {
            // State Bool Route Bool -> Bool Pair<State,Route>
            guest!(b"drop calc_bits@1.rot swap pair");
            loop {
                guest!(b"calc_display@1.ascii_fraction_width_with_state");
                if !signal!() {
                    break;
                }
                guest!(b"calc_display@1.ascii_fraction_admit_with_state");
                if !signal!() {
                    break;
                }
                if ascii_steps == 0 {
                    return Err(
                        "guest ASCII fraction cursor exceeded its declared progress bound".into(),
                    );
                }
                ascii_steps -= 1;
                guest!(b"calc_display@1.ascii_fraction_multiply_with_state");
                guest!(b"calc_display@1.ascii_fraction_product_signal_with_state");
                if !signal!() {
                    guest!(b"calc_display@1.ascii_fraction_product_unchanged_with_state");
                    route!();
                    break;
                }
                // Bool Pair<State,Product> -> Bool State Result<ActiveDigit,Unit>
                guest!(b"calc_display@1.ascii_fraction_scan_begin_with_state");
                report = guest!(b"unpair");
                let mut active = checked_guest_bool(&report.json)?;
                // Bool Result<ActiveDigit,Unit> State: keep State on top so the
                // scan stages run under dip with only Bool beneath them; each
                // probe leaves its Bool on top for one read, then drops it.
                guest!(b"drop swap");
                let mut quotient_steps = (budget[2] as u64).saturating_add(1).saturating_mul(20);
                while active {
                    if quotient_steps == 0 {
                        return Err(
                            "guest fractional quotient exceeded its certified bit-scan bound"
                                .into(),
                        );
                    }
                    quotient_steps -= 1;
                    guest!(b"[ calc_display@1.ascii_fraction_scan_bit_stage ] dip");
                    report = guest!(
                        b"[ calc_display@1.ascii_fraction_scan_terminal_stage unpair ] dip swap"
                    );
                    let scanning = checked_guest_bool(&report.json)?;
                    self.defer_consume(b"drop");
                    if !scanning {
                        break;
                    }
                    report = guest!(
                        b"[ calc_display@1.ascii_fraction_scan_subtract_stage unpair ] dip swap"
                    );
                    active = checked_guest_bool(&report.json)?;
                    self.defer_consume(b"drop");
                    if !active {
                        break;
                    }
                    report = guest!(b"[ calc_display@1.ascii_fraction_scan_subtract_check_stage unpair ] dip swap");
                    let more_bits = checked_guest_bool(&report.json)?;
                    self.defer_consume(b"drop");
                    if !more_bits {
                        report = guest!(b"[ calc_display@1.ascii_fraction_scan_subtract_finish_stage unpair ] dip swap");
                        active = checked_guest_bool(&report.json)?;
                        self.defer_consume(b"drop");
                    }
                }
                // Bool Result State -> Bool State Result<ActiveDigit,Unit>
                //   -> Bool Pair<State,Pair<Result<Route,Unit>,Bool>>
                guest!(b"swap");
                guest!(b"calc_display@1.ascii_fraction_scan_finish_with_state");
                guest!(b"calc_display@1.ascii_fraction_scan_done_with_state");
                let more = signal!();
                route!();
                if !more {
                    break;
                }
            }
            // Bool Pair<State,Pair<Route,Bool>> after a false width/admit is
            // already Bool Pair<State,Route> after signal!(); park State and
            // Bool above the Route as Pair<State,Bool>.
            guest!(b"unpair calc_bits@1.rot swap [ pair ] dip swap");
        } else {
            guest!(b"drop [ pair ] dip swap");
        }
        // expr.format_ascii_finite's finish stage and result match together
        // leave too little compiler Work headroom; run them as two compiles
        // over the parked Pair<State,Bool>, then restore State Bool Result.
        for source in [
            b"[ calc_display@1.ascii_fraction_finish_stage ] dip".as_slice(),
            b"[ expr.format_ascii_final ] dip",
        ] {
            guest!(source);
        }
        report = guest!(b"swap [ unpair ] dip");
        if let Some(diagnostic) = checked_ascii_error(&report.json)? {
            return Ok(Outcome::Diagnostic(diagnostic));
        }
        report = self.execute(b"expr.format_text", &[], None)?;
        if report.outcome != "normal" {
            return Ok(Outcome::NativeFailure(report.json));
        }
        let outcome = decode_result(&report.json)?;
        if let Outcome::Value(ref text) = outcome {
            if text.len() > INPUT_LIMIT as usize || text.len() as i64 > budget[6] {
                return Err("calculator guest output exceeded the selected display budget".into());
            }
            let retained = self.execute(b"drop drop", &[], None)?;
            if retained.outcome != "normal" {
                return Ok(Outcome::NativeFailure(retained.json));
            }
            if self.stack.len() != 1 {
                return Err("guest calculator retained state has an invalid typed stack".into());
            }
        }
        Ok(outcome)
    }

    /// Admit a parsed function body without evaluating it. The guest owns a
    /// sealed BeginSession over the original State, header metadata and parsed
    /// Plan/Budget; the host only schedules guest checks from guest-reported
    /// token kinds and cursors, and installs exactly the guest's sealed result.
    fn install_function(&mut self, source_len: usize, work: i64) -> Result<Outcome, String> {
        match self.admit_function_body(source_len, work) {
            Ok(()) => Ok(Outcome::Defined),
            Err(Halt::Outcome(outcome)) => Ok(outcome),
            Err(Halt::Error(message)) => Err(message),
        }
    }

    fn admit_function_body(&mut self, source_len: usize, work: i64) -> Result<(), Halt> {
        // [ State Metadata Bool ParseFinish ] -> [ State Metadata Result ]; a
        // failed parse keeps its own diagnostic.
        self.guest_step(b"[ drop ] dip")?;
        let report = self.guest_step(
            b"[ pair ] dip swap [ expr.parse_function_plan ] dip swap [ unpair ] dip",
        )?;
        if let Some(diagnostic) = checked_error(&report)? {
            return Err(Halt::Outcome(Outcome::Diagnostic(diagnostic)));
        }
        self.guest_step(b"calc_defs@1.begin_session_start")?;
        // Every scheduled step is one guest module; a body token costs a few
        // steps per candidate byte, bounded by the public work quota.
        let mut remaining = u64::try_from(work)
            .unwrap_or(0)
            .saturating_add((source_len as u64).saturating_mul(16).saturating_add(64));
        loop {
            let report = self.guest_step(b"calc_defs@1.begin_session_peek")?;
            let present = checked_guest_bool(&report)?;
            let report = self.guest_step(b"drop")?;
            let token = if present {
                Some(checked_body_token(&report)?)
            } else {
                None
            };
            self.defer_consume(b"drop");
            let Some((kind, width)) = token else { break };
            // Each arm leaves [ BeginSession Bool ], Bool = token checked.
            let report = match kind {
                10 => self.check_body_name(width, &mut remaining)?,
                kind if kind >= 100 => self.check_body_call(width, &mut remaining)?,
                _ => self.guest_step(b"calc_defs@1.begin_session_check_ordinary")?,
            };
            let checked = checked_guest_bool(&report)?;
            self.defer_consume(b"drop");
            if !checked {
                let report = self.guest_step(b"calc_defs@1.begin_session_diagnostic")?;
                return Err(body_diagnostic(&report)?);
            }
            let report = self.guest_step(b"calc_defs@1.begin_session_advance_checked")?;
            if !checked_guest_bool(&report)? {
                return Err(Halt::Error(
                    "guest function body token did not advance after its check".into(),
                ));
            }
            self.defer_consume(b"drop");
            remaining = remaining
                .checked_sub(1)
                .ok_or("guest function body exceeded its scheduling bound")?;
        }
        self.guest_step(b"calc_defs@1.begin_session_finish_outcome")?;
        let report = self.guest_step(b"calc_defs@1.finished_session_commit_checked")?;
        if !checked_ready(&report)? {
            return Err(Halt::Error(
                "guest function admission did not issue its install capability".into(),
            ));
        }
        // Only the Ready arm runs: the guest's installed State replaces the
        // original, and the parser Budget is released.
        self.guest_step(b"[ unpair drop ] [ drop calc_env@1.initial ] result@1.Result.match")?;
        Ok(())
    }

    /// Check one body identifier: a byte-matched parameter, else an existing
    /// global variable. Returns the report of [ BeginSession Bool ]. The
    /// guest-reported token width only schedules the completion check; the
    /// guest itself certifies every byte and the final size.
    fn check_body_name(&mut self, width: i64, remaining: &mut u64) -> Result<String, Halt> {
        for word in [
            b"calc_defs@1.begin_session_name_begin".as_slice(),
            b"calc_defs@1.begin_name_extract",
            b"calc_defs@1.begin_name_params",
            b"calc_defs@1.begin_name_guard",
        ] {
            self.guest_step(word)?;
        }
        // A positive cursor is the next byte offset. Completion is tested once
        // the last byte is committed; every other positive cursor compares.
        let mut committed = false;
        let global = loop {
            *remaining = remaining
                .checked_sub(1)
                .ok_or("guest body name search exceeded its scheduling bound")?;
            let cursor = self.body_cursor(b"calc_defs@1.begin_name_cursor")?;
            let step = match cursor {
                -4 => break false,
                -5 => break true,
                0 => b"calc_defs@1.begin_name_param_length".as_slice(),
                // A byte mismatch moves to the next parameter candidate.
                -7 => b"calc_defs@1.begin_name_param_next",
                cursor if cursor <= -1_000_000 => {
                    committed = true;
                    self.guest_step(b"calc_defs@1.begin_name_param_commit_byte")?;
                    continue;
                }
                cursor if cursor > 0 && committed && cursor - 1 == width => {
                    committed = false;
                    b"calc_defs@1.begin_name_param_complete"
                }
                cursor if cursor > 0 => {
                    committed = false;
                    b"calc_defs@1.begin_name_param_compare"
                }
                // An invalid route leaves the token unchecked.
                _ => break false,
            };
            self.guest_step(step)?;
        };
        if !global {
            return self.guest_step(b"calc_defs@1.begin_name_finish_parameter");
        }
        // Not a parameter: the identifier must name an existing variable.
        self.guest_step(b"calc_defs@1.begin_name_global")?;
        committed = false;
        loop {
            *remaining = remaining
                .checked_sub(1)
                .ok_or("guest body global search exceeded its scheduling bound")?;
            let cursor = self.body_cursor(b"calc_defs@1.begin_global_cursor")?;
            let step = match cursor {
                -2 => break,
                0 => b"calc_defs@1.begin_global_length".as_slice(),
                cursor if cursor <= -1_000_000 => {
                    committed = true;
                    self.guest_step(b"calc_defs@1.begin_global_commit_byte")?;
                    continue;
                }
                cursor if cursor > 0 && committed && cursor - 1 == width => {
                    committed = false;
                    b"calc_defs@1.begin_global_complete"
                }
                cursor if cursor > 0 => {
                    committed = false;
                    b"calc_defs@1.begin_global_compare"
                }
                // -5: no variable has this name; anything else is a failed
                // sealed route. The guest reports either fault.
                _ => {
                    let report = self.guest_step(b"calc_defs@1.begin_global_diagnostic")?;
                    return Err(body_diagnostic(&report)?);
                }
            };
            self.guest_step(step)?;
        }
        self.guest_step(b"calc_defs@1.begin_global_finish")
    }

    /// Check one body call: a call naming the function being defined is
    /// self-recursion, otherwise the newest existing function of that name
    /// must take exactly the call's argument count. Returns the report of
    /// [ BeginSession Bool ]. As for names, the guest-reported width only
    /// schedules steps; the guest certifies every byte, size and arity.
    fn check_body_call(&mut self, width: i64, remaining: &mut u64) -> Result<String, Halt> {
        let steps = u64::try_from(width)
            .ok()
            .and_then(|width| width.checked_mul(2))
            .and_then(|steps| steps.checked_add(2))
            .ok_or("guest body call reported an invalid name width")?;
        *remaining = remaining
            .checked_sub(steps)
            .ok_or("guest body call self check exceeded its scheduling bound")?;
        // Self check against the defined name: the size, then each byte
        // compared and committed, then completion. Every guest transition
        // checks its own stage, so after a size or byte mismatch the rest
        // leave "another function" unchanged; a full match is -4.
        self.guest_step(b"calc_defs@1.begin_session_call_begin")?;
        self.guest_step(b"calc_defs@1.begin_call_self_length")?;
        for _ in 0..width {
            self.guest_step(b"calc_defs@1.begin_call_self_compare")?;
            self.guest_step(b"calc_defs@1.begin_call_self_commit_byte")?;
        }
        for word in [
            b"calc_defs@1.begin_call_self_complete".as_slice(),
            b"calc_defs@1.begin_call_history",
            b"calc_defs@1.begin_call_scan",
        ] {
            self.guest_step(word)?;
        }
        // The function history search follows the captured-global cursor
        // protocol; -2 found schedules the arity check, -3 is admitted.
        let mut committed = false;
        loop {
            *remaining = remaining
                .checked_sub(1)
                .ok_or("guest body call search exceeded its scheduling bound")?;
            let cursor = self.body_cursor(b"calc_defs@1.begin_call_scan_cursor")?;
            let step = match cursor {
                -3 => break,
                -2 => b"calc_defs@1.begin_call_scan_arity".as_slice(),
                0 => b"calc_defs@1.begin_call_scan_length",
                cursor if cursor <= -1_000_000 => {
                    committed = true;
                    self.guest_step(b"calc_defs@1.begin_call_scan_commit_byte")?;
                    continue;
                }
                cursor if cursor > 0 && committed && cursor - 1 == width => {
                    committed = false;
                    b"calc_defs@1.begin_call_scan_complete"
                }
                cursor if cursor > 0 => {
                    committed = false;
                    b"calc_defs@1.begin_call_scan_compare"
                }
                -7 => {
                    let report = self.guest_step(b"calc_defs@1.begin_call_arity_diagnostic")?;
                    return Err(body_diagnostic(&report)?);
                }
                // -4 self-recursion, -5 no such function; anything else is a
                // failed sealed route. The guest reports each.
                _ => {
                    let report = self.guest_step(b"calc_defs@1.begin_call_scan_diagnostic")?;
                    return Err(body_diagnostic(&report)?);
                }
            };
            self.guest_step(step)?;
        }
        self.guest_step(b"calc_defs@1.begin_call_scan_finish")
    }

    /// Read a guest cursor beside its owner and pop it.
    fn body_cursor(&mut self, word: &[u8]) -> Result<i64, Halt> {
        let report = self.guest_step(word)?;
        let cursor = checked_guest_opcode(&report)?;
        self.defer_consume(b"drop");
        Ok(cursor)
    }

    /// One guest phase; a native failure ends the command with its outcome.
    fn guest_step(&mut self, word: &[u8]) -> Result<String, Halt> {
        let report = self.execute(word, &[], None)?;
        if report.outcome != "normal" {
            return Err(Halt::Outcome(Outcome::NativeFailure(report.json)));
        }
        Ok(report.json)
    }

    fn phase(&mut self, word: &[u8]) -> Result<crate::core::output::Report, String> {
        let word = std::str::from_utf8(word).map_err(|error| error.to_string())?;
        let source = format!("[ swap ] dip swap [ {word} ] dip swap [ swap ] dip");
        self.execute(source.as_bytes(), &[], None)
    }

    /// Consume the last probe as the prefix of the next module. The consumer
    /// takes only the probe value, before any quotation of that module.
    fn defer_consume(&mut self, consumer: &'static [u8]) {
        debug_assert!(self.deferred.is_none());
        self.deferred = Some(consumer);
    }

    /// Run a pending probe consumer as its own module, for boundaries that
    /// inspect the typed stack or inject host values.
    fn flush_deferred(&mut self) -> Result<Option<crate::core::output::Report>, String> {
        match self.deferred.take() {
            Some(consumer) => {
                let report = self.execute(consumer, &[], None)?;
                Ok((report.outcome != "normal").then_some(report))
            }
            None => Ok(None),
        }
    }

    fn execute(
        &mut self,
        source: &[u8],
        injected: &[Ty],
        frame: Option<&str>,
    ) -> Result<crate::core::output::Report, String> {
        let merged;
        let source = match self.deferred {
            Some(consumer) if injected.is_empty() => {
                self.deferred = None;
                merged = [consumer, b" ".as_slice(), source].concat();
                merged.as_slice()
            }
            Some(_) => {
                if let Some(failed) = self.flush_deferred()? {
                    return Ok(failed);
                }
                source
            }
            None => source,
        };
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or("calculator module counter exhausted")?;
        let augmented;
        let stack = if injected.is_empty() {
            self.stack.as_slice()
        } else {
            augmented = [self.stack.as_slice(), injected].concat();
            augmented.as_slice()
        };
        let prepared = self
            .modules
            .as_ref()
            .ok_or("calculator namespace unavailable")?
            .prepare(source, stack, LIMITS)
            .map_err(|error| {
                format!(
                    "{}: {}",
                    String::from_utf8_lossy(source),
                    source_error(error)
                )
            })?;
        if prepared.kind() != ModuleKind::Expression {
            return Err("calculator command did not check as guest expression".into());
        }
        let output = prepared.output().to_vec();
        let submission = prepared
            .submission()
            .ok_or("checked expression has no guest program")?;
        let compiled = if self.program_free {
            self.compiler.prepare_program_free(submission)
        } else if self.quiescent {
            self.compiler.prepare_quiescent(submission)
        } else {
            self.compiler.prepare(submission)
        }
        .map_err(|error| {
            format!(
                "{}: {}",
                String::from_utf8_lossy(source),
                backend_error(error)
            )
        })?;
        let ready = self
            .worker
            .prepare(compiled.wat(), source, self.serial)
            .map_err(|error| error.message)?;
        if ready.outcome != "ready" {
            return Err(format!("calculator Wasm assembly refused: {}", ready.json));
        }
        let report = match frame {
            Some(frame) => self.worker.execute_inputs(frame),
            None => self.worker.execute(),
        }
        .map_err(|error| error.message)?;
        if report.outcome == "normal" {
            let (compiled_reachable, programs_reachable) = boundary_reachability(&report.json)?;
            self.compiler.commit(compiled).map_err(backend_error)?;
            let (next, committed) = self
                .modules
                .take()
                .ok_or("calculator namespace unavailable")?
                .commit(prepared);
            self.modules = Some(next);
            committed.map_err(source_error)?;
            self.stack = output;
            self.quiescent = !compiled_reachable;
            self.program_free = !programs_reachable;
        }
        Ok(report)
    }
}

/// The boundary answers of a normal submission: whether a reachable Program
/// runs compiled code, and whether any Program, Syntax or recipe cell is
/// reachable at all. The second implies the first.
fn boundary_reachability(report: &str) -> Result<(bool, bool), String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let compiled = json["compiled_programs_reachable"]
        .as_bool()
        .ok_or("calculator report lacks its compiled Program reachability")?;
    let programs = json["programs_reachable"]
        .as_bool()
        .ok_or("calculator report lacks its Program reachability")?;
    if compiled && !programs {
        return Err("calculator report has inconsistent Program reachability".into());
    }
    Ok((compiled, programs))
}

fn backend_error(error: noble_wasm::Diagnostic) -> String {
    let failure = crate::core::output::Failure::backend(error);
    format!("{} ({})", failure.message, failure.context.outcome)
}

fn source_error(error: noble_contracts::source::Error) -> String {
    let span = error.diagnostic().span;
    format!(
        "{:?} [{},{}): {}",
        error.stage(),
        span.start,
        span.end,
        error.diagnostic().message
    )
}

fn decode_result(report: &str) -> Result<Outcome, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("calculator guest returned no result")?;
    let (complete, result) = variant(result)?;
    if !complete {
        return Err("guest calculator formatter retained an unfinished ASCII fraction".into());
    }
    let (is_ok, payload) = variant(result)?;
    if is_ok {
        if payload["type"] != "Text" {
            return Err("calculator guest returned a non-Text exact result".into());
        }
        Ok(Outcome::Value(
            payload["value"]
                .as_str()
                .ok_or("calculator guest returned invalid exact Text")?
                .to_owned(),
        ))
    } else {
        Ok(Outcome::Diagnostic(diagnostic(payload)?))
    }
}

fn checked_ascii_error(report: &str) -> Result<Option<String>, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("guest ASCII fraction returned no checked result")?;
    let (complete, result) = variant(result)?;
    if !complete {
        return Err("guest ASCII fraction cursor was not completed".into());
    }
    let (ready, payload) = variant(result)?;
    if ready {
        Ok(None)
    } else {
        Ok(Some(diagnostic(payload)?))
    }
}

/// Early end of a scheduled guest route: a public outcome or a host error.
enum Halt {
    Outcome(Outcome),
    Error(String),
}

impl From<String> for Halt {
    fn from(message: String) -> Self {
        Halt::Error(message)
    }
}

impl From<&str> for Halt {
    fn from(message: &str) -> Self {
        Halt::Error(message.into())
    }
}

/// The guest's Result<Unit, Diagnostic> for a rejected body token. Ok marks an
/// original admission failure that the header/parser checks already report.
fn body_diagnostic(report: &str) -> Result<Halt, String> {
    match checked_error(report)? {
        Some(diagnostic) => Ok(Halt::Outcome(Outcome::Diagnostic(diagnostic))),
        None => Err("guest function body lost its original admission".into()),
    }
}

/// Kind and byte width of a peeked body token: Result<Pair<kind, span>, Unit>.
fn checked_body_token(report: &str) -> Result<(i64, i64), String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("guest body peek returned no token")?;
    let (present, token) = variant(result)?;
    if !present {
        return Err("guest body peek reported a token without one".into());
    }
    let (kind, span) = pair(token)?;
    let (start, end) = pair(span)?;
    let [kind, start, end] = [kind, start, end].map(|cell| -> Result<i64, String> {
        if cell["type"] != "I64" {
            return Err("guest body token returned a non-I64 field".into());
        }
        cell["value"]
            .as_str()
            .ok_or("guest body token returned an invalid field")?
            .parse::<i64>()
            .map_err(|error| error.to_string())
    });
    let (start, end) = (start?, end?);
    if end < start {
        return Err("guest body token returned an inverted span".into());
    }
    Ok((kind?, end - start))
}

fn checked_error(report: &str) -> Result<Option<String>, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("calculator guest returned no checked result")?;
    let (ok, payload) = variant(result)?;
    if ok {
        Ok(None)
    } else {
        Ok(Some(diagnostic(payload)?))
    }
}

fn checked_sum_error(report: &str) -> Result<Option<String>, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("guest parser returned no terminal error")?;
    if result["type"] != "Sum" {
        return Err("guest parser returned a non-sum terminal error".into());
    }
    match result["variant"].as_str() {
        Some("left") => Ok(None),
        Some("right") => {
            Ok(Some(diagnostic(result.get("value").ok_or(
                "guest parser terminal error omitted its diagnostic",
            )?)?))
        }
        _ => Err("guest parser returned an invalid terminal error arm".into()),
    }
}

fn checked_lexical_error(report: &str) -> Result<String, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("guest lexical error returned no source-bound result")?;
    let (ready, payload) = variant(result)?;
    if !ready {
        return Err("guest lexical failure lacked its original source span".into());
    }
    diagnostic(payload)
}

fn checked_hot_terminal(report: &str) -> Result<String, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("guest hot frame returned no terminal diagnostic result")?;
    let (valid, payload) = variant(result)?;
    if !valid {
        return Err("guest hot frame contained no valid public diagnostic".into());
    }
    diagnostic(payload)
}

fn checked_frame_error(report: &str) -> Result<Option<String>, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let stack = json["stack"]
        .as_array()
        .ok_or("calculator guest returned no typed stack")?;
    // The authentic FrameRun is the final typed stack cell. A checked let
    // Name may itself be a Result variant below it and must never be decoded
    // as the evaluator frame.
    let frame = stack
        .last()
        .ok_or("calculator guest returned no evaluator frame")?;
    let (active, payload) = variant(frame)?;
    if active {
        Ok(None)
    } else {
        Ok(Some(diagnostic(payload)?))
    }
}

fn checked_ready(report: &str) -> Result<bool, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("calculator guest returned no checked state")?;
    variant(result).map(|(ready, _)| ready)
}

/// Readiness of an evaluator carrier: a guest Result or the builtin Sum
/// that the schoolbook and exponent stages use (left = ready).
fn checked_carrier_ready(report: &str) -> Result<bool, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("calculator guest returned no checked carrier")?;
    carrier_variant(result).map(|(ready, _)| ready)
}

/// A Sum cursor probe leaves `[ carrier Bool Ctx ]`. The flag is only
/// meaningful when the carrier itself is ready.
fn checked_probe_flag(report: &str) -> Result<bool, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let stack = json["stack"]
        .as_array()
        .ok_or("guest cursor probe returned no typed stack")?;
    let [.., carrier, flag, _] = stack.as_slice() else {
        return Err("guest cursor probe returned a short stack".into());
    };
    if !carrier_variant(carrier)?.0 {
        return Err("guest cursor probe was not certified".into());
    }
    if flag["type"] != "Bool" {
        return Err("guest cursor probe returned a non-Bool flag".into());
    }
    flag["value"]
        .as_bool()
        .ok_or_else(|| "guest cursor probe returned an invalid flag".into())
}

/// Register-on-top probe: the guest scalar sits directly under the register.
fn register_probe(report: &str) -> Result<serde_json::Value, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let stack = json["stack"]
        .as_array()
        .ok_or("guest probe returned no typed stack")?;
    let [.., probe, _] = stack.as_slice() else {
        return Err("guest probe returned a short stack".into());
    };
    Ok(probe.clone())
}

fn checked_register_probe_bool(report: &str) -> Result<bool, String> {
    let probe = register_probe(report)?;
    if probe["type"] != "Bool" {
        return Err("guest probe returned a non-Bool flag".into());
    }
    probe["value"]
        .as_bool()
        .ok_or_else(|| "guest probe returned an invalid flag".into())
}

fn checked_register_probe_opcode(report: &str) -> Result<i64, String> {
    let probe = register_probe(report)?;
    if probe["type"] != "I64" {
        return Err("guest probe returned a non-I64 selector".into());
    }
    probe["value"]
        .as_str()
        .ok_or("guest probe returned an invalid selector")?
        .parse::<i64>()
        .map_err(|error| error.to_string())
}

fn carrier_variant(value: &serde_json::Value) -> Result<(bool, &serde_json::Value), String> {
    if value["type"] != "Sum" {
        return variant(value);
    }
    let payload = value
        .get("value")
        .ok_or("guest carrier omitted its payload")?;
    match value["variant"].as_str() {
        Some("left") => Ok((true, payload)),
        Some("right") => Ok((false, payload)),
        _ => Err("guest carrier returned an invalid Sum arm".into()),
    }
}

fn checked_guest_bool(report: &str) -> Result<bool, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("calculator guest returned no checked progress flag")?;
    if result["type"] != "Bool" {
        return Err("calculator guest returned a non-Bool progress flag".into());
    }
    result["value"]
        .as_bool()
        .ok_or("calculator guest returned an invalid progress flag".into())
}

fn checked_parser_route_bool(report: &str) -> Result<bool, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("guest parser returned no checked route")?;
    let (ready, payload) = variant(result)?;
    if !ready {
        return Err("guest parser route was not certified".into());
    }
    let (_, selector) = pair(payload)?;
    if selector["type"] != "Bool" {
        return Err("guest parser route returned a non-Bool selector".into());
    }
    selector["value"]
        .as_bool()
        .ok_or_else(|| "guest parser route returned an invalid selector".into())
}

fn checked_guest_opcode(report: &str) -> Result<i64, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("guest lexer returned no checked opcode")?;
    if result["type"] != "I64" {
        return Err("guest lexer returned a non-I64 opcode".into());
    }
    result["value"]
        .as_str()
        .ok_or("guest lexer returned an invalid opcode")?
        .parse::<i64>()
        .map_err(|error| error.to_string())
}

fn checked_numeric_phase(report: &str) -> Result<bool, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("guest numeric phase returned no checked result")?;
    let (ready, payload) = variant(result)?;
    if !ready {
        return Err("guest numeric phase readiness was not certified".into());
    }
    let (_, outcome) = pair(payload)?;
    variant(outcome).map(|(success, _)| success)
}

fn checked_kind(report: &str) -> Result<Result<i64, String>, String> {
    let json: serde_json::Value =
        serde_json::from_str(report).map_err(|error| error.to_string())?;
    let result = json["stack"]
        .as_array()
        .and_then(|stack| stack.last())
        .ok_or("calculator guest returned no checked action result")?;
    let (ok, payload) = variant(result)?;
    if !ok {
        if payload["type"] == "Unit" {
            return Err("guest evaluator action readiness was not certified".into());
        }
        return Ok(Err(diagnostic(payload)?));
    }
    let (_, kind) = pair(payload)?;
    if kind["type"] != "I64" {
        return Err("guest parser returned a non-I64 action kind".into());
    }
    Ok(Ok(kind["value"]
        .as_str()
        .ok_or("guest parser returned invalid action kind")?
        .parse::<i64>()
        .map_err(|error| error.to_string())?))
}

fn variant(value: &serde_json::Value) -> Result<(bool, &serde_json::Value), String> {
    if value["type"] != "Nominal" || value["shape"] != "variant" {
        return Err("calculator guest returned an invalid diagnostic schema".into());
    }
    let arm = match value["variant"].as_str() {
        Some("left") => true,
        Some("right") => false,
        _ => return Err("calculator guest returned an invalid diagnostic arm".into()),
    };
    let payload = value
        .get("value")
        .ok_or("calculator guest omitted diagnostic payload")?;
    Ok((arm, payload))
}

fn pair(value: &serde_json::Value) -> Result<(&serde_json::Value, &serde_json::Value), String> {
    if value["type"] != "Pair" {
        return Err("calculator guest returned invalid diagnostic span".into());
    }
    let values = value["value"]
        .as_array()
        .filter(|values| values.len() == 2)
        .ok_or("calculator guest returned incomplete diagnostic span")?;
    Ok((&values[0], &values[1]))
}

fn diagnostic(value: &serde_json::Value) -> Result<String, String> {
    let (parse, inner) = variant(value)?;
    let (category, span) = if parse {
        let (syntax, span) = variant(inner)?;
        (
            if syntax {
                "SyntaxError"
            } else {
                "TrailingInput"
            }
            .to_owned(),
            span,
        )
    } else {
        let (scope, inner) = variant(inner)?;
        if scope {
            let (name, inner) = variant(inner)?;
            if name {
                let (unknown, span) = variant(inner)?;
                (
                    if unknown { "UnknownName" } else { "WrongArity" }.to_owned(),
                    span,
                )
            } else {
                let (invalid, span) = variant(inner)?;
                (
                    if invalid {
                        "DefinitionError"
                    } else {
                        "InvalidCall"
                    }
                    .to_owned(),
                    span,
                )
            }
        } else {
            let (numeric, inner) = variant(inner)?;
            if numeric {
                let (error, span) = pair(inner)?;
                (numeric_category(error)?, span)
            } else {
                ("Unsupported".into(), inner)
            }
        }
    };
    let (start, end) = pair(span)?;
    let start = start["value"]
        .as_str()
        .ok_or("invalid diagnostic byte start")?
        .parse::<usize>()
        .map_err(|error| error.to_string())?;
    let end = end["value"]
        .as_str()
        .ok_or("invalid diagnostic byte end")?
        .parse::<usize>()
        .map_err(|error| error.to_string())?;
    if start > end || end > usize::try_from(INPUT_LIMIT).map_err(|error| error.to_string())? {
        return Err("guest diagnostic span exceeds calculator input bound".into());
    }
    Ok(format!("{category} [{start},{end}) provenance=unavailable"))
}

fn numeric_category(value: &serde_json::Value) -> Result<String, String> {
    let (limited, inner) = variant(value)?;
    if limited {
        let (primary, inner) = variant(inner)?;
        let limit = if primary {
            let (io, inner) = variant(inner)?;
            let (first, _) = variant(inner)?;
            if io {
                if first {
                    "InputBytes"
                } else {
                    "OutputSize"
                }
            } else if first {
                "NumericDigits"
            } else {
                "IntermediateIntegerBits"
            }
        } else {
            let (work, inner) = variant(inner)?;
            if work {
                "EvaluationWork"
            } else {
                let (syntax, _) = variant(inner)?;
                if syntax {
                    "SyntaxDepth"
                } else {
                    "CallDepth"
                }
            }
        };
        Ok(format!("ResourceLimit {limit}"))
    } else {
        let (conversion, inner) = variant(inner)?;
        if conversion {
            let (non_integral, _) = variant(inner)?;
            Ok(if non_integral {
                "NonIntegral"
            } else {
                "OutOfRange"
            }
            .into())
        } else {
            let (non_integer, inner) = variant(inner)?;
            if non_integer {
                Ok("NonIntegerExponent".into())
            } else {
                let (zero_denominator, _) = variant(inner)?;
                Ok(if zero_denominator {
                    "ZeroDenominator"
                } else {
                    "DivisionByZero"
                }
                .into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Outcome, Session, DEFAULT_BUDGET};
    use noble_kernel::types::Ty;

    /// One calculator line run through `Session::transact`.
    type Line = fn(&mut Session) -> Result<Outcome, String>;

    // These tests drive real worker modules, so they need a `noble` binary as
    // the calculator WAT assembler. `cargo test` builds it beside this unit
    // test executable (target/<profile>/deps/..) for the integration tests.
    fn assembler() -> std::path::PathBuf {
        let executable = std::env::current_exe().unwrap();
        let profile = executable.parent().unwrap().parent().unwrap();
        let assembler = profile.join("noble");
        assert!(
            assembler.is_file(),
            "build the noble binary first: {}",
            assembler.display()
        );
        assembler
    }

    fn session() -> Session {
        let worker = crate::core::worker::Engine::start_calculator_with(false, &assembler())
            .map_err(|error| error.message)
            .unwrap();
        Session::with_worker(worker).unwrap()
    }

    /// Everything a later line can observe: the host's namespace, compiler,
    /// typed stack and boundary flags, plus the worker's digest of all memory,
    /// every table entry, every ABI global, the published signatures and the
    /// engine counters.
    fn fingerprint(session: &mut Session) -> String {
        let arena = session
            .worker
            .calculator_arena()
            .map_err(|error| error.message)
            .unwrap();
        let arena: serde_json::Value = serde_json::from_str(&arena.json).unwrap();
        format!(
            "{:?}\n{:?}\n{:?}\nserial={} quiescent={} program_free={} deferred={:?}\n{}",
            session.modules,
            session.compiler,
            session.stack,
            session.serial,
            session.quiescent,
            session.program_free,
            session.deferred,
            arena["arena"]
        )
    }

    /// One accepted guest module as a committed transaction; returns its
    /// checked output stack.
    fn accept(session: &mut Session, source: &[u8]) -> serde_json::Value {
        let mut stack = serde_json::Value::Null;
        let outcome = session
            .transact(|session| {
                let report = session.execute(source, &[], None)?;
                assert_eq!(report.outcome, "normal", "{}", report.json);
                let report: serde_json::Value = serde_json::from_str(&report.json).unwrap();
                stack = report["stack"].clone();
                Ok(Outcome::Defined)
            })
            .unwrap();
        match outcome {
            Outcome::Defined => (),
            Outcome::InfrastructureFailure(message) | Outcome::NativeFailure(message) => {
                panic!("{}: {message}", String::from_utf8_lossy(source))
            }
            _ => panic!("{} was not accepted", String::from_utf8_lossy(source)),
        }
        stack
    }

    /// The debug/test cross-check: a session reconstructed by replaying only
    /// the accepted history from scratch matches the restored session.
    fn assert_matches_replay(restored: &mut Session, history: &[&[u8]]) {
        let mut replayed = session();
        for line in history {
            accept(&mut replayed, line);
        }
        assert_eq!(fingerprint(restored), fingerprint(&mut replayed));
    }

    // A failed line of each class runs real guest modules that consume the
    // committed State, rewrite cells beyond the first 16 pages, install
    // table entries and publish signatures before it fails.
    fn guest_diagnostic(session: &mut Session) -> Result<Outcome, String> {
        session.execute(b"drop drop calc_env@1.initial [ 9 * ]", &[], None)?;
        Ok(Outcome::Diagnostic("DivisionByZero [9,10)".into()))
    }

    fn guest_resource_limit(session: &mut Session) -> Result<Outcome, String> {
        session.execute(b"nil [ 1 swap cons true ] 0 [ 1 + ] pair", &[], None)?;
        Ok(Outcome::Diagnostic(
            "ResourceLimit IntermediateBits [0,8)".into(),
        ))
    }

    fn native_quota(session: &mut Session) -> Result<Outcome, String> {
        let report = session.execute(b"nil [ 1 swap cons true ] iterate", &[], None)?;
        assert_eq!(report.outcome, "runtime-exhausted", "{}", report.json);
        Ok(Outcome::NativeFailure(report.json))
    }

    fn host_limit(session: &mut Session) -> Result<Outcome, String> {
        session.execute(b"unit pair [ 3 + ]", &[], None)?;
        Err("guest lexer exceeded its source-byte progress bound".into())
    }

    fn rust_infrastructure(session: &mut Session) -> Result<Outcome, String> {
        session.execute(b"drop 5", &[], None)?;
        session.execute(b"calc_missing@1.word", &[], None)?;
        unreachable!("an unknown word must not check")
    }

    fn engine_infrastructure(session: &mut Session) -> Result<Outcome, String> {
        session.execute(b"[ 4 + ] pair", &[], None)?;
        // The worker fails a malformed input frame after the module installed,
        // and survives only to be restored.
        session.execute(b"drop", &[Ty::Text], Some("[ malformed"))?;
        unreachable!("a malformed input frame must not execute")
    }

    #[test]
    fn failed_lines_of_every_class_restore_the_committed_session() {
        // The history drops the calculator State so the committed compiled
        // Program can later run on a closed stack row.
        let history: [&[u8]; 3] = [b"drop 7 35 +", b"[ 1 + ]", b"unit pair"];
        let mut restored = session();
        let mut reference = session();
        // A real (cheap) calculator line whose guest lexer phases run on the
        // committed session before it fails with a guest diagnostic.
        match restored.submit(b"$", false, DEFAULT_BUDGET).unwrap() {
            Outcome::Diagnostic(diagnostic) => {
                assert_eq!(diagnostic, "SyntaxError [0,1) provenance=unavailable");
            }
            _ => panic!("an invalid byte must fail lexing"),
        }
        assert_eq!(fingerprint(&mut restored), fingerprint(&mut reference));
        for line in history {
            assert_eq!(accept(&mut restored, line), accept(&mut reference, line));
        }
        assert!(
            !restored.quiescent,
            "a reachable compiled Program keeps its table slots"
        );
        let committed = fingerprint(&mut reference);
        assert_eq!(fingerprint(&mut restored), committed);
        let failures: [(&str, Line); 6] = [
            ("guest diagnostic", guest_diagnostic),
            ("guest ResourceLimit", guest_resource_limit),
            ("native quota", native_quota),
            ("host limit", host_limit),
            ("Rust infrastructure failure", rust_infrastructure),
            ("engine infrastructure failure", engine_infrastructure),
        ];
        for (class, line) in failures {
            let outcome = restored.transact(line).unwrap();
            assert!(
                !matches!(outcome, Outcome::Value(_) | Outcome::Defined),
                "{class}"
            );
            assert_eq!(fingerprint(&mut restored), committed, "{class}");
        }
        // Later lines see only the committed session: identical outputs and
        // identical arenas, including the reused table slots and signatures.
        for line in [b"unpair drop swap 2 *".as_slice(), b"swap run"] {
            assert_eq!(accept(&mut restored, line), accept(&mut reference, line));
            assert_eq!(fingerprint(&mut restored), fingerprint(&mut reference));
        }
        assert_matches_replay(
            &mut restored,
            &[
                b"drop 7 35 +",
                b"[ 1 + ]",
                b"unit pair",
                b"unpair drop swap 2 *",
                b"swap run",
            ],
        );
    }

    /// CALC-STATE-02 on real calculator lines. Each definition costs minutes
    /// of guest modules, so this stays out of the default suite; run it with
    /// `cargo test --release -p noble-cli --bin noble calc_state_02 -- --ignored`.
    #[test]
    #[ignore = "runs real calculator lines for about an hour"]
    fn calc_state_02_failed_lines_leave_definitions_byte_identical() {
        let definitions: [&[u8]; 3] = [b"let x = 10", b"fn f(y)=x + y", b"let x = 20"];
        let reference = std::thread::spawn(move || {
            let mut reference = session();
            for line in definitions {
                assert!(matches!(
                    reference.submit(line, false, DEFAULT_BUDGET).unwrap(),
                    Outcome::Value(_) | Outcome::Defined
                ));
            }
            let committed = fingerprint(&mut reference);
            let mut outputs = Vec::new();
            for line in [b"f(1)".as_slice(), b"x"] {
                outputs.push(value(
                    reference.submit(line, false, DEFAULT_BUDGET).unwrap(),
                ));
                outputs.push(fingerprint(&mut reference));
            }
            (committed, outputs)
        });
        let mut restored = session();
        for line in definitions {
            assert!(matches!(
                restored.submit(line, false, DEFAULT_BUDGET).unwrap(),
                Outcome::Value(_) | Outcome::Defined
            ));
        }
        let committed = fingerprint(&mut restored);
        let mut diagnostics = Vec::new();
        for line in [b"let x = 1 / 0".as_slice(), b"2 ^ 5000"] {
            match restored.submit(line, false, DEFAULT_BUDGET).unwrap() {
                Outcome::Diagnostic(diagnostic) => diagnostics.push(diagnostic),
                _ => panic!("{} must fail", String::from_utf8_lossy(line)),
            }
            assert_eq!(fingerprint(&mut restored), committed);
        }
        let injected: [Line; 4] = [
            native_quota,
            host_limit,
            rust_infrastructure,
            engine_infrastructure,
        ];
        for line in injected {
            assert!(!matches!(
                restored.transact(line).unwrap(),
                Outcome::Value(_) | Outcome::Defined
            ));
            assert_eq!(fingerprint(&mut restored), committed);
        }
        let mut outputs = Vec::new();
        for line in [b"f(1)".as_slice(), b"x"] {
            outputs.push(value(restored.submit(line, false, DEFAULT_BUDGET).unwrap()));
            outputs.push(fingerprint(&mut restored));
        }
        let (reference_committed, reference_outputs) = reference.join().unwrap();
        assert_eq!(committed, reference_committed);
        assert_eq!(outputs, reference_outputs);
        assert_eq!(outputs[0], "11");
        assert_eq!(outputs[2], "20");
        eprintln!(
            "CALC-STATE-02 diagnostics: {diagnostics:?}; outputs: {:?}",
            [&outputs[0], &outputs[2]]
        );
    }

    fn value(outcome: Outcome) -> String {
        match outcome {
            Outcome::Value(value) => value,
            _ => panic!("expected a value"),
        }
    }
}
