//! Source/session orchestration; execution occurs only in the selected Wasm engine.

mod arguments;
pub(crate) mod artifact;
pub(crate) mod companions;
mod declared;
pub(crate) mod editor;
pub(crate) mod entry;
mod framing;
pub(crate) mod live;
mod output;
mod report;
pub(crate) mod worker;

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
  noble live repl [--source ABS_PATH] [--engine v8|interpreter] [--self-edit NAME --expect-generation N]
  live repl: newline-delimited source, :reload ABS_PATH, or :grant-self-edit NAME N; interpreter is unavailable
  noble editor analyze|admit JSON_FILE [--emit NEW_DIR (admit only)]
  JSON_FILE: editor AST format 1, nodes integer/boolean/word/quotation/hole
  noble admit-artifact WASM --effects CLAIMS_JSON [--source HOST_SOURCE] [--allow-effects test.emit,test.abort,test.clock] [--opt off|on]
  CLAIMS_JSON: {\"claimed_effects\":[]}; optional source/digest/allowed/trusted_correspondence are ignored
  --source and --allow-effects are selected by the host invoker, never by candidate claims.
  Admission rebuilds selected source and compares final Wasm bytes before fresh isolated execution.
  noble run SOURCE [--opt off|on] [--emit NEW_DIR]
  noble compile SOURCE [--input-type I64|Bool|Text|Unit ...]
  noble compile SOURCE --declared-modules --bindings HOST_FILE [--module MODULE_FILE ...] [--input-type MODULE@VERSION.TYPE ...]
  noble session [--framed] [--opt off|on] [--emit NEW_DIR]
  noble session --framed --declared-modules --bindings HOST_FILE [--opt off|on] [--emit NEW_DIR]
  HOST_FILE: bind MODULE@VERSION test.emit ADAPTER Text -- ! test.emit allow|deny
             bind MODULE@VERSION test.clock ADAPTER -- I64 ! test.clock allow|deny script I64[,I64...]
  Declared-Modules-v1 module/import/definition units link without running guest code.
  Preparation limits: --source-bytes N --source-nodes N --source-depth N --source-work N

Core-Bootstrap uses the selected managed-linear-memory backend.
run checks one complete file before executing any of it.
session reads one source submission per line; --framed reads a decimal byte count,
newline, then exactly that many source bytes (permits multiline and invalid UTF-8).
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
        let declared = match &options.manifest {
            Some(manifest) => Some(attempt!(
                noble_contracts::source::ModuleSession::new(&manifest.operations())
                    .map_err(output::Failure::source)
            )),
            None => None,
        };
        Ok(Self {
            frontend: noble_contracts::source::Session::new(),
            declared,
            compiler: noble_wasm::source::Compiler::new(),
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
        self.submissions = attempt!(self.submissions.checked_add(1).ok_or_else(|| {
            output::Failure::new(
                output::ErrorContext {
                    stage: "session",
                    outcome: "exhausted",
                },
                "submission counter exhausted",
            )
        }));
        if self.declared.is_some() {
            return self.submit_declared(source, options);
        }
        let prepared = match self.frontend.prepare(source, &self.stack, options.limits) {
            Ok(prepared) => prepared,
            Err(error) => return Ok(output::Report::source_error(&error, self.submissions)),
        };
        if prepared.is_definition() {
            attempt!(
                self.frontend
                    .commit(prepared)
                    .map_err(output::Failure::source)
            );
            return Ok(output::Report::defined(self.submissions));
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
            Err(error) => return Ok(output::Report::backend_error(error, self.submissions)),
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
        attempt!(
            self.compiler
                .commit(compiled)
                .map_err(output::Failure::backend)
        );
        attempt!(
            self.frontend
                .commit(prepared)
                .map_err(output::Failure::source)
        );
        let report = attempt!(engine.execute());
        if report.outcome == "normal" {
            self.stack = output_types;
        }
        Ok(report)
    }
}
