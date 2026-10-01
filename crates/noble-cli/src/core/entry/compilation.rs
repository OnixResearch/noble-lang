mod legacy;

pub(super) fn compile(
    options: &super::super::arguments::Options,
) -> Result<u8, super::super::output::Failure> {
    let source_path = attempt!(options.source.as_ref().ok_or_else(|| {
        super::super::output::Failure::new(
            super::super::output::ErrorContext {
                stage: "arguments",
                outcome: "invalid-input",
            },
            super::super::USAGE,
        )
    }));
    let source = attempt!(super::super::framing::read_file(
        source_path,
        options.limits.bytes
    ));
    if options.declared_modules {
        return compile_declared(options, &source);
    }
    legacy::compile(options, &source)
}

/// Preload explicitly supplied source modules, then independently check and
/// compile one expression without executing its body. No ambient import search.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; declared frontend initialization and explicit source-module preloading allocate and mutate runtime namespaces before compilation."
)]
fn compile_declared(
    options: &super::super::arguments::Options,
    source: &[u8],
) -> Result<u8, super::super::output::Failure> {
    let frontend = attempt!(declared_frontend(options));
    let frontend = match attempt!(preload_modules(frontend, options)) {
        Checked::Ready(frontend) => frontend,
        Checked::Reported(exit) => return Ok(exit),
    };
    compile_expression(&frontend, options, source)
}

#[octet::sealed_enum]
enum Checked<T> {
    Ready(T),
    Reported(u8),
}

/// A printed refusal is not an argument or I/O failure, and must not start a worker.
fn reported<T>(
    report: super::super::output::Report,
) -> Result<Checked<T>, super::super::output::Failure> {
    attempt!(super::super::output::print(&report));
    Ok(Checked::Reported(report.exit()))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this phase resolves runtime nominal input types, independently checks a submitted expression, and writes accepted WAT or prepares selected engine artifacts."
)]
fn compile_expression(
    frontend: &noble_contracts::source::ModuleSession,
    options: &super::super::arguments::Options,
    source: &[u8],
) -> Result<u8, super::super::output::Failure> {
    let inputs = match attempt!(declared_input_types(frontend, options)) {
        Checked::Ready(inputs) => inputs,
        Checked::Reported(exit) => return Ok(exit),
    };
    let compiled = match attempt!(accepted_wat(frontend, source, &inputs, options.limits)) {
        Checked::Ready(compiled) => compiled,
        Checked::Reported(exit) => return Ok(exit),
    };
    emit_declared_wat(options, source, compiled.wat())
}

#[expect(
    clippy::while_let_on_iterator,
    reason = "Owner: noble-maintainers; the selected compiler cannot resolve ForLoop expansion. Explicit bounded iteration preserves ordered input resolution and its first-refusal report."
)]
fn declared_input_types(
    frontend: &noble_contracts::source::ModuleSession,
    options: &super::super::arguments::Options,
) -> Result<Checked<std::vec::Vec<noble_kernel::types::Ty>>, super::super::output::Failure> {
    let mut inputs = std::vec::Vec::with_capacity(options.inputs.len());
    let mut requested = options.inputs.iter();
    while let Some(input) = requested.next() {
        match attempt!(declared_input_type(frontend, input)) {
            Checked::Ready(ty) => inputs.push(ty),
            Checked::Reported(exit) => return Ok(Checked::Reported(exit)),
        }
    }
    Ok(Checked::Ready(inputs))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; resolving a nominal CLI input consults the runtime module namespace and prints an allocated source diagnostic when it is unknown."
)]
fn declared_input_type(
    frontend: &noble_contracts::source::ModuleSession,
    input: &super::super::arguments::InputType,
) -> Result<Checked<noble_kernel::types::Ty>, super::super::output::Failure> {
    match input {
        super::super::arguments::InputType::Concrete(ty) => Ok(Checked::Ready(ty.clone())),
        super::super::arguments::InputType::Nominal(name) => match frontend.resolve_type(name) {
            Ok(ty) => Ok(Checked::Ready(ty)),
            Err(error) => {
                let report = super::super::output::Report::declared_source_error(&error, 0);
                reported(report)
            }
        },
    }
}

/// An independently accepted candidate owns its WAT until output or artifact preparation.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; source preparation and expression extraction consult an allocated runtime namespace and may construct a printed diagnostic."
)]
fn accepted_wat(
    frontend: &noble_contracts::source::ModuleSession,
    source: &[u8],
    inputs: &[noble_kernel::types::Ty],
    limits: noble_contracts::Limits,
) -> Result<Checked<noble_wasm::source::Prepared>, super::super::output::Failure> {
    let prepared = match frontend.prepare(source, inputs, limits) {
        Ok(prepared) => prepared,
        Err(error) => {
            let report = super::super::output::Report::declared_source_error(&error, 1);
            return reported(report);
        }
    };
    let submission = attempt!(expression_candidate(&prepared));
    compile_wat(submission)
}

fn expression_candidate(
    prepared: &noble_contracts::source::ModulePrepared,
) -> Result<&noble_kernel::execution::Submission, super::super::output::Failure> {
    prepared.submission().ok_or_else(|| {
        super::super::output::Failure::new(
            super::super::output::ErrorContext {
                stage: "check",
                outcome: "reject",
            },
            "compile requires a declared expression, not another module declaration",
        )
    })
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; independent Wasm compiler preparation allocates a checked prospective module, and refusal allocates a structured runtime diagnostic."
)]
fn compile_wat(
    submission: &noble_kernel::execution::Submission,
) -> Result<Checked<noble_wasm::source::Prepared>, super::super::output::Failure> {
    let compiler = noble_wasm::source::Compiler::new();
    let compiled = match compiler.prepare(submission) {
        Ok(compiled) => compiled,
        Err(error) => {
            let report = super::super::output::Report::declared_backend_error(error, 1);
            return reported(report);
        }
    };
    Ok(Checked::Ready(compiled))
}

fn emit_declared_wat(
    options: &super::super::arguments::Options,
    source: &[u8],
    wat: &[u8],
) -> Result<u8, super::super::output::Failure> {
    if let Some(directory) = &options.emit {
        attempt!(std::fs::create_dir(directory).map_err(super::super::framing::io_error));
        let mut engine = attempt!(super::super::worker::Engine::start(options));
        let report = attempt!(engine.prepare(wat, source, 1));
        attempt!(super::super::output::print(&report));
        Ok(report.exit())
    } else {
        attempt!(
            std::io::Write::write_all(&mut std::io::stdout().lock(), wat)
                .map_err(super::super::framing::io_error)
        );
        Ok(0)
    }
}

fn declared_frontend(
    options: &super::super::arguments::Options,
) -> Result<noble_contracts::source::ModuleSession, super::super::output::Failure> {
    let manifest =
        attempt!(options
            .manifest
            .as_ref()
            .ok_or_else(|| super::super::output::Failure::new(
                super::super::output::ErrorContext {
                    stage: "arguments",
                    outcome: "invalid-input"
                },
                "Declared-Modules-v1 requires an explicit host binding manifest",
            )));
    noble_contracts::source::ModuleSession::new(&manifest.operations())
        .map_err(super::super::output::Failure::source)
}

/// Link every explicit source module before the executable expression. A
/// declaration refusal is an ordinary source report, not a worker launch.
#[expect(
    clippy::while_let_on_iterator,
    reason = "Owner: noble-maintainers; the selected compiler cannot resolve ForLoop expansion. Explicit bounded iteration preserves ordered namespace commits and first-refusal propagation."
)]
fn preload_modules(
    mut frontend: noble_contracts::source::ModuleSession,
    options: &super::super::arguments::Options,
) -> Result<Checked<noble_contracts::source::ModuleSession>, super::super::output::Failure> {
    let mut modules = options.modules.iter();
    while let Some(module_path) = modules.next() {
        frontend = match attempt!(preload_module(frontend, module_path, options.limits)) {
            Checked::Ready(next) => next,
            Checked::Reported(exit) => return Ok(Checked::Reported(exit)),
        };
    }
    Ok(Checked::Ready(frontend))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; each explicit module is read within source limits, prepared against the live namespace, and committed as a runtime transaction before its successor."
)]
fn preload_module(
    frontend: noble_contracts::source::ModuleSession,
    module_path: &std::path::Path,
    limits: noble_contracts::Limits,
) -> Result<Checked<noble_contracts::source::ModuleSession>, super::super::output::Failure> {
    let module = attempt!(super::super::framing::read_file(module_path, limits.bytes));
    let prepared = match attempt!(checked_module(&frontend, &module, limits)) {
        Checked::Ready(prepared) => prepared,
        Checked::Reported(exit) => return Ok(Checked::Reported(exit)),
    };
    let (next, outcome) = if prepared.proof_obligations().is_some() {
        frontend.commit_verified(prepared, |batch| {
            crate::workflow::intrinsic::verify_batch(
                batch, &module, limits, crate::workflow::DEFAULT_TIMEOUT
            ).map_err(|error| std::format!("{}: {}",error.code,error.message))
        })
    } else {
        frontend.commit(prepared)
    };
    match outcome {
        Ok(()) => Ok(Checked::Ready(next)),
        Err(error) => {
            let report = super::super::output::Report::declared_source_error(&error, 0);
            reported(report)
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; checking a source module prepares an owned namespace transaction and prints typed source refusals from runtime source bytes."
)]
fn checked_module(
    frontend: &noble_contracts::source::ModuleSession,
    module: &[u8],
    limits: noble_contracts::Limits,
) -> Result<Checked<noble_contracts::source::ModulePrepared>, super::super::output::Failure> {
    match frontend.prepare(module, &[], limits) {
        Ok(prepared) if prepared.submission().is_none() => Ok(Checked::Ready(prepared)),
        Ok(_) => Err(super::super::output::Failure::new(
            super::super::output::ErrorContext {
                stage: "link",
                outcome: "reject",
            },
            "--module requires a source declaration without an executable root",
        )),
        Err(error) => {
            let report = super::super::output::Report::declared_source_error(&error, 0);
            reported(report)
        }
    }
}
