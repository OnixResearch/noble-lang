//! CLI orchestration. The no_std frontend is the only contract preparation path.

pub const USAGE: &str = "usage:
  noble verify CONTRACT [--emit DIR] [--proof FILE | --refutation FILE] [--timeout-ms N]
  noble explain-proof CONTRACT
  noble --internal-budget-smoke <u32>

Consumer configuration (absolute paths): NOBLE_LEAN, NOBLE_BWRAP,
NOBLE_PRLIMIT, NOBLE_SYSTEMD_RUN, NOBLE_CONTRACT_LIBRARY
(and NOBLE_NIX_STORE for a Nix toolchain).
Lean must be release 4.31.0.
Default proof budget: 120000 ms; maximum: 600000 ms. Missing evidence is unknown.
Explanation never executes Lean. Proof source runs only in a Linux bubblewrap
sandbox with no network, credentials, or writable host directories.
Proof checking requires an active user systemd manager and XDG_RUNTIME_DIR.
--emit creates a new directory; existing destinations are never overwritten.";

const SOURCE_LIMIT: usize = 1_048_576;
const PROOF_LIMIT: usize = 524_288;
const PROOF_WIRE_LIMIT: usize = 8_388_608;
const LIBRARY_LIMIT: usize = 4_194_304;
const MODULE_LIMIT: usize = 128;
const DEFAULT_TIMEOUT: u64 = 120_000;
const MAX_TIMEOUT: u64 = 600_000;

mod arguments;
mod artifacts;
mod encoding;
pub(super) mod output;
mod rules;
mod subject;
mod verification;

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; run records argument, verification and emission failures in the JSON report before choosing the outcome's exit status. External failures must remain reportable rather than become assertion panics."
)]
pub fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let mut report = output::Report::new();
    let options = match arguments::parse(arguments) {
        Ok(options) => options,
        Err(error) => {
            report.fail(error);
            return print_report(&report);
        }
    };
    report.command = if options.is_explanation {
        "explain-proof"
    } else {
        "verify"
    };
    report.source_path = options.contract.to_string_lossy().into_owned();
    report.timeout_ms = options.timeout_ms;
    let proof_source = match execute(&options, &mut report) {
        Ok(proof) => proof,
        Err(error) => {
            report.fail(error);
            None
        }
    };
    if let Some(destination) = &options.emit {
        if let Err(error) = artifacts::emit(destination, &mut report, proof_source.as_deref()) {
            report.fail(error);
        }
    }
    print_report(&report)
}

fn print_report(report: &output::Report) -> std::process::ExitCode {
    let mut output = report.json().encode();
    output.push('\n');
    match std::io::Write::write_all(&mut std::io::stdout().lock(), output.as_bytes()) {
        Ok(()) => std::process::ExitCode::from(report.outcome.exit()),
        Err(_) => std::process::ExitCode::from(2),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; execute distinguishes explanation, missing evidence, preparation failure and verification outcomes. Proof-source, library and subprocess failures are typed diagnostics, not asserted success assumptions."
)]
fn execute(
    options: &arguments::Options,
    report: &mut output::Report,
) -> Result<Option<std::string::String>, output::Failure> {
    attempt!(prepare_contract(&options.contract, report));
    let library = rules::read_library();
    report.library = match &library {
        Ok(library) => library.json(),
        Err(error) => encoding::object([
            ("status", encoding::string("unavailable")),
            ("diagnostic", error.json()),
        ]),
    };
    if options.is_explanation {
        report.outcome = output::Outcome::NotRun;
        return Ok(None);
    }
    let Some(path) = &options.proof else {
        report.outcome = output::Outcome::Unknown;
        report.diagnostics.push(
            output::Failure::error(
                "evidence-missing",
                "no application proof or refutation was supplied; no proof process ran".into(),
            )
            .json(),
        );
        return Ok(None);
    };
    report.proof_request = proof_request(path, options.is_refutation);
    let proof = attempt!(
        std::string::String::from_utf8(attempt!(artifacts::read_bounded(
            path,
            PROOF_LIMIT,
            "proof-source"
        )))
        .map_err(|_| output::Failure::error("proof-encoding", "proof source must be UTF-8".into()))
    );
    let library = attempt!(library);
    let generated = attempt!(report.generated.as_deref().ok_or_else(|| {
        output::Failure::error(
            "export-internal",
            "prepared contract has no generated statement".into(),
        )
    }));
    match verification::verify(
        generated,
        &proof,
        &library,
        options.is_refutation,
        options.timeout_ms,
    ) {
        Ok(acceptance) => record_acceptance(report, acceptance, options.is_refutation),
        Err(error) => report.fail(error),
    }
    Ok(Some(proof))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; prepare_contract feeds byte-bounded hostile input to the checked frontend and preserves invalid UTF-8 and preparation diagnostics in the report. Rejection must remain a Failure rather than a panic."
)]
fn prepare_contract(
    path: &std::path::Path,
    report: &mut output::Report,
) -> Result<(), output::Failure> {
    let limits = noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    };
    let preparation_limit_bytes = attempt!(usize::try_from(limits.bytes)
        .map_err(|error| output::Failure::error("preparation-limit", error.to_string())));
    let bytes = attempt!(artifacts::read_bounded(
        path,
        SOURCE_LIMIT.min(preparation_limit_bytes),
        "contract-source"
    ));
    match std::str::from_utf8(&bytes) {
        Ok(source) => report.source = Some(source.into()),
        Err(_) => report.invalid_source_bytes = Some(encoding::hex(&bytes)),
    }
    let prepared = match noble_contracts::prepare(&bytes, limits) {
        Ok(prepared) => prepared,
        Err(error) => return Err(preparation_failure(error, report)),
    };
    report.ordinary_typing = encoding::object([
        ("outcome", encoding::string("accepted")),
        (
            "checked",
            encoding::string(std::format!("{:?}", prepared.checked())),
        ),
    ]);
    report.subject = subject::describe(&prepared, &bytes);
    report.generated = Some(noble_contracts::export_lean(&prepared));
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; preparation_failure exhaustively translates the frontend's four diagnostic kinds and retains ordinary typing and source spans. Reporting a rejected contract has no success invariant to assert."
)]
fn preparation_failure(
    error: noble_contracts::Diagnostic,
    report: &mut output::Report,
) -> output::Failure {
    report.ordinary_typing = match error.ordinary_typing() {
        Some(checked) => encoding::object([
            ("outcome", encoding::string("accepted")),
            ("checked", encoding::string(std::format!("{checked:?}"))),
        ]),
        None => encoding::string("not-accepted"),
    };
    let mut failure = match error.kind {
        noble_contracts::DiagnosticKind::Unsupported => {
            output::Failure::unsupported("contract-unsupported", error.message)
        }
        noble_contracts::DiagnosticKind::Invalid => {
            output::Failure::error("contract-invalid", error.message)
        }
        noble_contracts::DiagnosticKind::Exhausted => {
            output::Failure::error("preparation-limit", error.message)
        }
        noble_contracts::DiagnosticKind::Internal => {
            output::Failure::error("preparation-internal", error.message)
        }
    };
    failure.details = encoding::object([
        ("start", encoding::Json::Number(u64::from(error.span.start))),
        ("end", encoding::Json::Number(u64::from(error.span.end))),
    ]);
    failure
}

fn proof_request(path: &std::path::Path, is_refutation: bool) -> encoding::Json {
    encoding::object([
        ("source_path", encoding::string(path.to_string_lossy())),
        (
            "kind",
            encoding::string(if is_refutation { "refutation" } else { "proof" }),
        ),
        (
            "theorem",
            encoding::string(if is_refutation {
                "MC1Proof.refutation"
            } else {
                "MC1Proof.proof"
            }),
        ),
        (
            "expected_type",
            encoding::string(if is_refutation {
                "Not MC1Obligation.claim"
            } else {
                "MC1Obligation.claim"
            }),
        ),
    ])
}

fn record_acceptance(
    report: &mut output::Report,
    acceptance: verification::Acceptance,
    is_refutation: bool,
) {
    report.outcome = if is_refutation {
        output::Outcome::Disproved
    } else {
        output::Outcome::Proved
    };
    report.theorem = Some(if is_refutation {
        "MC1Proof.refutation"
    } else {
        "MC1Proof.proof"
    });
    report.axioms = Some(acceptance.axioms);
    report.proof_wire = Some(acceptance.wire);
    report.rechecked = true;
    report.tools = acceptance.tools;
}
