//! Source/session orchestration; execution occurs only in the selected Wasm engine.

mod arguments;
mod framing;
mod output;
mod worker;

pub const USAGE: &str = "usage:
  noble run SOURCE [--opt off|on] [--emit NEW_DIR]
  noble compile SOURCE [--input-type I64|Bool|Text|Unit ...]
  noble session [--framed] [--opt off|on] [--emit NEW_DIR]
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
    compiler: noble_wasm::source::Compiler,
    stack: std::vec::Vec<noble_kernel::types::Ty>,
    worker: Option<worker::Engine>,
    submissions: u64,
}

impl Session {
    fn new() -> Self {
        Self {
            frontend: noble_contracts::source::Session::new(),
            compiler: noble_wasm::source::Compiler::new(),
            stack: std::vec::Vec::new(),
            worker: None,
            submissions: 0,
        }
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
        let prepared = match self.frontend.prepare(source, &self.stack, options.limits) {
            Ok(prepared) => prepared,
            Err(error) => return Ok(output::Report::source_error(&error, self.submissions)),
        };
        if prepared.is_definition() {
            attempt!(self
                .frontend
                .commit(prepared)
                .map_err(output::Failure::source));
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
}

pub fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let result = arguments::parse(arguments).and_then(execute);
    match result {
        Ok(exit) => std::process::ExitCode::from(exit),
        Err(error) => {
            let report = output::Report::failure(&error);
            if let Err(write_error) = output::print(&report) {
                eprintln!("noble: {}", write_error.message);
            }
            std::process::ExitCode::from(error.exit())
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; execute preserves source refusals, terminal engine outcomes and artifact/stdout failures as reports and exit statuses. External input and I/O failures must not become assertion panics."
)]
fn execute(options: arguments::Options) -> Result<u8, output::Failure> {
    if options.compile_only {
        return compile(&options);
    }
    if let Some(directory) = &options.emit {
        attempt!(std::fs::create_dir(directory).map_err(framing::io_error));
    }
    let mut session = Session::new();
    if let Some(path) = &options.source {
        let source = attempt!(framing::read_file(path, options.limits.bytes));
        let report = attempt!(session.submit(&source, &options));
        attempt!(retain(&options, session.submissions, &source, &report));
        attempt!(output::print(&report));
        return Ok(report.exit());
    }
    let mut input = std::io::BufReader::new(std::io::stdin().lock());
    let mut exit = 0;
    while let Some(source) = attempt!(framing::read_submission(
        &mut input,
        options.framed,
        options.limits.bytes,
    )) {
        let report = attempt!(session.submit(&source, &options));
        attempt!(retain(&options, session.submissions, &source, &report));
        attempt!(output::print(&report));
        exit = exit.max(report.exit());
        if report.is_terminal() {
            break;
        }
    }
    Ok(exit)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; compile returns argument, bounded-source, independent-acceptance and stdout failures through Failure. User source and host streams are fallible boundaries, not asserted preconditions."
)]
fn compile(options: &arguments::Options) -> Result<u8, output::Failure> {
    let source_path = attempt!(options.source.as_ref().ok_or_else(|| {
        output::Failure::new(
            output::ErrorContext {
                stage: "arguments",
                outcome: "invalid-input",
            },
            USAGE,
        )
    }));
    let source = attempt!(framing::read_file(source_path, options.limits.bytes));
    let frontend = noble_contracts::source::Session::new();
    let prepared = attempt!(frontend
        .prepare(&source, &options.inputs, options.limits)
        .map_err(output::Failure::source));
    let submission = attempt!(prepared.submission().ok_or_else(|| {
        output::Failure::new(
            output::ErrorContext {
                stage: "check",
                outcome: "unsupported",
            },
            "compile requires an expression submission",
        )
    }));
    let compiled = attempt!(noble_wasm::source::Compiler::new()
        .prepare(submission)
        .map_err(output::Failure::backend));
    attempt!(
        std::io::Write::write_all(&mut std::io::stdout().lock(), compiled.wat())
            .map_err(framing::io_error)
    );
    Ok(0)
}

fn retain(
    options: &arguments::Options,
    number: u64,
    source: &[u8],
    report: &output::Report,
) -> Result<(), output::Failure> {
    if let Some(directory) = &options.emit {
        attempt!(framing::write_new(
            &directory.join(std::format!("{number}.noble")),
            source
        ));
        attempt!(framing::write_new(
            &directory.join(std::format!("{number}.json")),
            report.json.as_bytes(),
        ));
    }
    Ok(())
}
