//! Source/session orchestration; execution occurs only in the selected Wasm engine.

mod arguments;
pub(crate) mod artifact;
pub(crate) mod companions;
mod declared;
pub(crate) mod editor;
pub(crate) mod entry;
mod framing;
pub(crate) mod live;
pub(crate) mod live_slot;
mod output;
mod report;
pub(crate) mod worker;
pub(crate) mod worker_scalar;
pub(crate) mod worker_package;
pub(crate) mod worker_typed;
pub(crate) mod worker_value;
pub(crate) mod worker_shell;

pub(crate) const SOURCE_LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

/// Evidence ingress has its own 512 KiB bound; source preparation stays 64 KiB.
pub(crate) const EVIDENCE_LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 524_288,
    ..SOURCE_LIMITS
};

pub const USAGE: &str = "usage:
  noble live repl [--source ABS_PATH] [--engine v8|interpreter] [--self-edit NAME --expect-generation N] [--verify-source-before-execute] [--watch] [--declared-modules --bindings MANIFEST] [--max-retained-generations N]
  live repl: newline-delimited source, :reload ABS_PATH, :grant-self-edit NAME N,
    or :release-observed-roots (relinquishes all previously reported raw cell handles);
    interpreter is unavailable
  noble live watch --source ABS_PATH [--engine v8] [--declared-modules --bindings MANIFEST] [--max-retained-generations N]
  live watch: Linux inotify complete-save rename + checked file ACK; in-place writes have no watch ACK
  noble live slot --engine v8 --authority ABS_PATH
  live slot: bounded JSONL define/install/candidate/publish/delete/rollback/invoke/reflect/trace/
    replay/policy/discard/release-program/release-replay/hold-checkpoint/resume-checkpoint;
    stdin must be an anonymous OS pipe whose launcher retains the write end exclusively and
    never delegates it to guest code; pipe metadata alone does not authenticate its writer;
    a checked saved Program owner may be published instead of its expression wrapper,
    but a dynamically composed owner without checked installed target metadata cannot;
    authority JSON separately selects owner, exact source SHA-256 allowlist, nominal resources,
    ordered slot contracts, effect ceiling and scoped grants. Proof-required publication refuses
    without independent target evidence; source text is never a control command.
  noble editor analyze|admit JSON_FILE [--emit NEW_DIR (admit only)]
  JSON_FILE: editor AST format 1, nodes integer/boolean/word/quotation/hole
  noble worker scalar SOURCE --state I64 --delta I64
  Scalar worker precursor: independently check I64 I64 -- I64 ! {}, run one
  bounded source-backed guest invocation, report its exact local outcome.
  noble worker typed PACKAGE --schema SCHEMA --state STATE_JSON --event EVENT_JSON
  Typed worker: host-select a source-checked WorkerData@1 module and two canonical
  nominal values; check one exact local no-capture package and exported transition
  before compiling and running one bounded State Event -- State List<Action> guest.
  noble worker shell PACKAGE --schema SCHEMA --state STATE --submit EVENT --cancel EVENT --authority HOST_GRANT
  Local synthetic-send shell: authorize checked inert Send against a separately
  selected exact grant, then cancel a pinned native task before late completion.
  No network send or portable package/export authority is provided.
  noble admit-artifact WASM --effects CLAIMS_JSON [--source HOST_SOURCE] [--allow-effects test.emit,test.abort,test.clock] [--opt off|on]
  CLAIMS_JSON: {\"claimed_effects\":[]}; optional source/digest/allowed/trusted_correspondence are ignored
  --source and --allow-effects are selected by the host invoker, never by candidate claims.
  Admission rebuilds selected source and compares final Wasm bytes before fresh isolated execution.
  noble run SOURCE [--opt off|on] [--emit NEW_DIR]
  noble compile SOURCE [--text-byte-cursor] [--input-type I64|Bool|Text|Unit ...]
  noble compile SOURCE --declared-modules --bindings HOST_FILE [--module MODULE_FILE ...] [--input-type MODULE@VERSION.TYPE ...]
  noble compile SOURCE --declared-modules --signed-self-recursion --bindings HOST_FILE [--module MODULE_FILE ...]
  noble compile SOURCE --declared-modules --text-byte-cursor [--signed-self-recursion] [--module MODULE_FILE ...]
  noble session [--text-byte-cursor] [--framed] [--opt off|on] [--emit NEW_DIR]
  noble session --framed --declared-modules --bindings HOST_FILE [--opt off|on] [--emit NEW_DIR]
  noble session --framed --declared-modules --signed-self-recursion --bindings HOST_FILE [--opt off|on] [--emit NEW_DIR]
  noble session --framed --declared-modules --text-byte-cursor [--signed-self-recursion] [--opt off|on] [--emit NEW_DIR]
  HOST_FILE: bind MODULE@VERSION test.emit ADAPTER Text -- ! test.emit allow|deny
             bind MODULE@VERSION test.clock ADAPTER -- I64 ! test.clock allow|deny script I64[,I64...]
  Declared-Modules-v1 module/import/definition units link without running guest code.
  Preparation limits: --source-bytes N (0..=65536) --source-nodes N --source-depth N --source-work N
  --retained-source-bytes N (1..=262144) requires declared compile or session;
  sets the cumulative retained original-source budget independently of the per-input
  --source-bytes limit; without it the cumulative budget equals --source-bytes.

Core-Bootstrap uses the selected managed-linear-memory backend.
--text-byte-cursor alone selects Text-Byte-Cursor-v1 for ordinary compile/session;
it adds pure text.byte, retaining Text and returning unsigned byte 0..255,
EOF -1 at the byte length, negative-offset -2 or past-end -3. Increment
the offset only after a nonnegative byte. With --declared-modules it selects a
new pure declared-text profile with text.byte, signed I64 <, and no host
bindings or ambient test effects. --signed-self-recursion also requires
--declared-modules, an exported self word and an explicit fixed signature.
Existing ordinary declared modules retain their explicit --bindings file.
run checks one complete file before executing any of it.
session reads one source submission per line; --framed reads a decimal byte count,
newline, then exactly that many source bytes (permits multiline and invalid UTF-8).
Declared framed sessions drain a complete oversized frame up to 262144 bytes in
bounded chunks, refuse it without checking or executing it, then read the next
header; larger, malformed or incomplete frames terminate the input stream.
Reports are JSON lines, ordered bottom-to-top. A static refusal preserves the
session; a runtime trap or exhausted execution limit ends it with its effect prefix.
--emit retains source, WAT, Wasm, tool bindings and runtime observations in a new
directory. No file is overwritten. Missing or incompatible tools fail closed.
compile emits independently accepted WAT, not an execution report; input types
are ordered bottom-to-top and values are supplied only after module compilation.";

struct Session {
    frontend: noble_contracts::source::Session,
    declared: Option<noble_contracts::source::ModuleSession>,
    compiler: noble_wasm::source::Compiler,
    stack: std::vec::Vec<noble_kernel::types::Ty>,
    worker: Option<worker::Engine>,
    submissions: u64,
}

impl Session {
    fn new(options: &arguments::Options) -> Result<Self, output::Failure> {
        let declared = if options.declared_modules {
            let bindings = options
                .manifest
                .as_ref()
                .map(|manifest| manifest.operations())
                .unwrap_or_default();
            let retained = options.retained_source_limit();
            Some(attempt!(if options.declared_text() {
                noble_contracts::source::ModuleSession::new_declared_text(
                    &bindings,
                    options.signed_self_recursion,
                    retained,
                )
            } else if options.signed_self_recursion {
                noble_contracts::source::ModuleSession::new_signed_self(&bindings, retained)
            } else {
                noble_contracts::source::ModuleSession::new(&bindings, retained)
            }
            .map_err(output::Failure::source)))
        } else {
            None
        };
        Ok(Self {
            frontend: if options.text_byte_cursor {
                noble_contracts::source::Session::new_text_cursor()
            } else {
                noble_contracts::source::Session::new()
            },
            declared,
            compiler: if options.declared_text() {
                noble_wasm::source::Compiler::new_declared_text()
            } else if options.text_byte_cursor {
                noble_wasm::source::Compiler::new_text_cursor()
            } else {
                noble_wasm::source::Compiler::new()
            },
            stack: std::vec::Vec::new(),
            worker: None,
            submissions: 0,
        })
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; submit prepares and commits allocated source/compiler state and exchanges commands with the selected engine process. These runtime library and I/O operations cannot be const."
    )]
    fn submit(
        &mut self,
        source: &[u8],
        options: &arguments::Options,
    ) -> Result<output::Report, output::Failure> {
        attempt!(self.advance_submission());
        if self.declared.is_some() {
            return self.submit_declared(source, options);
        }
        let prepared = match self.frontend.prepare(source, &self.stack, options.limits) {
            Ok(prepared) => prepared,
            Err(error) => {
                return Ok(output::Report::source_error_for_profile(
                    &error,
                    self.submissions,
                    options.text_byte_cursor,
                ))
            }
        };
        if prepared.is_definition() {
            attempt!(self
                .frontend
                .commit(prepared)
                .map_err(output::Failure::source));
            return Ok(output::Report::defined_for_profile(
                self.submissions,
                options.text_byte_cursor,
            ));
        }
        let submission = attempt!(prepared.submission().ok_or_else(|| {
            output::Failure::new(
                output::ErrorContext {
                    stage: "acceptance",
                    outcome: "internal-failure",
                },
                "missing expression candidate",
            )
        }));
        let compiled = match self.compiler.prepare(submission) {
            Ok(compiled) => compiled,
            Err(error) => {
                return Ok(output::Report::backend_error_for_profile(
                    error,
                    self.submissions,
                    options.text_byte_cursor,
                ))
            }
        };
        if self.worker.is_none() {
            self.worker = Some(attempt!(worker::Engine::start(options)));
        }
        let engine = attempt!(self.worker.as_mut().ok_or_else(|| {
            output::Failure::new(
                output::ErrorContext {
                    stage: "wasm",
                    outcome: "internal-failure",
                },
                "missing engine worker",
            )
        }));
        let prepared_report = attempt!(engine.prepare(compiled.wat(), source, self.submissions));
        if prepared_report.outcome != "ready" {
            return Ok(prepared_report);
        }
        let output_types = prepared.output().to_vec();
        attempt!(self
            .compiler
            .commit(compiled)
            .map_err(output::Failure::backend));
        attempt!(self
            .frontend
            .commit(prepared)
            .map_err(output::Failure::source));
        let report = attempt!(engine.execute());
        if report.outcome == "normal" {
            self.stack = output_types;
        }
        Ok(report)
    }

    fn advance_submission(&mut self) -> Result<u64, output::Failure> {
        self.submissions = attempt!(self.submissions.checked_add(1).ok_or_else(|| {
            output::Failure::new(
                output::ErrorContext {
                    stage: "session",
                    outcome: "exhausted",
                },
                "submission counter exhausted",
            )
        }));
        Ok(self.submissions)
    }
}
