//! CLI orchestration. The no_std frontend is the only contract preparation path.

pub const USAGE: &str = "usage:
  noble verify CONTRACT [--emit DIR] [--proof FILE | --refutation FILE] [--timeout-ms N]
  noble verify CONTRACT --counterexample boundary|input:N [--emit DIR] [--timeout-ms N]
  noble explain-proof CONTRACT
  noble verify-module SOURCE [--module PREVIOUS_SOURCE ...] [--timeout-ms N]
      [--normalization-work N] [--substitution-work N] [--report-schema v1|v2]
  noble obligations SOURCE [--module PREVIOUS_SOURCE ...]
  noble goal-view SOURCE [--module PREVIOUS_SOURCE ...]
  noble --internal-budget-smoke <u32>

Consumer configuration (absolute paths): NOBLE_LEAN, NOBLE_BWRAP,
NOBLE_PRLIMIT, NOBLE_SYSTEMD_RUN, NOBLE_CONTRACT_LIBRARY
(and NOBLE_NIX_STORE for a Nix toolchain).
Lean must be release 4.35.0.
Default proof budget: 120000 ms; maximum: 600000 ms. Missing evidence is unknown.
--report-schema v2 (the default) reports the VC-TOOL-02 outcome unknown with exit 3
when no proof obligation was supplied; v1 keeps the historical field set and exit.
--counterexample searches a unary I64 contract with a straight-line program: the
boundary producer tries 0, 1, -1, the signed extremes and i64::MAX - 1; input:N
supplies one candidate. A candidate is disproved only when the pinned Lean
consumer accepts the instantiated refutation template; a compiled run of the
candidate is recorded as observed-execution evidence and never decides the outcome.
Obligations and the goal view are read-only projections of prepared source data:
they never run the proof consumer and are never acceptance evidence.
Explanation never executes Lean. Proof source runs only in a Linux bubblewrap
sandbox with no network, credentials, or writable host directories.
Proof checking requires an active user systemd manager and XDG_RUNTIME_DIR.
--emit creates a new directory; existing destinations are never overwritten.";

pub(crate) const SOURCE_LIMIT: usize = 1_048_576;
pub(crate) const PROOF_LIMIT: usize = 524_288;
const PROOF_WIRE_LIMIT: usize = 8_388_608;
const LIBRARY_LIMIT: usize = 4_194_304;
const MODULE_LIMIT: usize = 128;
pub(crate) const DEFAULT_TIMEOUT: u64 = 120_000;
pub(crate) const MAX_TIMEOUT: u64 = 600_000;

pub(crate) mod admission;
mod arguments;
pub(crate) mod artifacts;
mod counterexample;
pub(crate) mod diagnostics;
pub(crate) mod encoding;
pub(crate) mod intrinsic;
pub(crate) mod obligations;
pub(crate) mod output;
mod rules;
mod subject;
mod verification;

// Shared host-only surface for the proof-required build lane. The build command
// reuses the exact same preparation, library snapshot and sandboxed
// producer/consumer pipeline rather than a second Lean route.
pub(crate) fn read_bounded(
    path: &std::path::Path,
    limit_bytes: usize,
    kind: &'static str,
) -> Result<std::vec::Vec<u8>, output::Failure> {
    artifacts::read_bounded(path, limit_bytes, kind)
}

pub(crate) fn subject_json(prepared: &noble_contracts::Prepared, source: &[u8]) -> encoding::Json {
    subject::describe(prepared, source)
}

/// The canonical contract library root this process compiles and checks
/// against (`NOBLE_CONTRACT_LIBRARY` or the pinned default). The proof-required
/// build binds only the bytes this selection actually uses.
pub(crate) fn contract_library_root() -> Result<std::path::PathBuf, output::Failure> {
    rules::root()
}

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
    let prepared = attempt!(prepare_contract(&options.contract, report));
    report.is_vacuity_suspected = precondition_is_false(&prepared);
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
    if let Some(producer) = options.counterexample {
        return search(producer, &prepared, library, options.timeout_ms, report);
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

/// VC-LOGIC-02: only the literal `false` precondition is detected here; any
/// other unsatisfiable precondition stays unassessed.
fn precondition_is_false(prepared: &noble_contracts::Prepared) -> bool {
    usize::try_from(prepared.requires())
        .ok()
        .and_then(|index| prepared.expressions().get(index))
        .is_some_and(|expression| matches!(expression.kind, noble_contracts::ExprKind::Bool(false)))
}

/// Search for a counterexample with an untrusted producer, replay the first
/// candidate semantically through the existing refutation route, and record
/// one compiled observation. Returns the instantiated refutation source.
fn search(
    producer: arguments::Producer,
    prepared: &noble_contracts::Prepared,
    library: Result<rules::Library, output::Failure>,
    timeout_ms: u64,
    report: &mut output::Report,
) -> Result<Option<std::string::String>, output::Failure> {
    let plan = attempt!(counterexample::plan(prepared));
    let (name, candidates) = match producer {
        arguments::Producer::Boundary => (
            counterexample::PRODUCER,
            counterexample::boundary_candidates(&plan, prepared),
        ),
        arguments::Producer::Supplied(input) => (
            "supplied-input",
            counterexample::supplied_candidate(&plan, input)
                .into_iter()
                .collect(),
        ),
    };
    let producer_json = encoding::object([
        ("id", encoding::string(name)),
        (
            "revision",
            encoding::Json::Number(counterexample::PRODUCER_REVISION),
        ),
        ("trust", encoding::string("untrusted; candidates are data")),
        ("budget", counterexample::budget_json()),
        (
            "candidates",
            encoding::Json::Array(
                candidates
                    .iter()
                    .map(counterexample::candidate_json)
                    .collect(),
            ),
        ),
    ]);
    let Some(candidate) = candidates.first() else {
        report.outcome = output::Outcome::Unknown;
        report.diagnostics.push(
            output::Failure::error(
                "producer-exhausted",
                "the producer returned no candidate that returns normally and falsifies the postcondition; nothing was replayed and nothing is claimed".into(),
            )
            .json(),
        );
        report.counterexample = Some(encoding::object([
            ("producer", producer_json),
            ("category", encoding::string("producer-exhausted")),
            ("advertised_as", encoding::string("nothing")),
        ]));
        return Ok(None);
    };
    let module = counterexample::refutation_module(&plan, prepared, candidate);
    report.proof_request = encoding::object([
        ("kind", encoding::string("refutation")),
        ("theorem", encoding::string("MC1Proof.refutation")),
        (
            "source",
            encoding::string(
                "instantiated reviewed template NobleContracts.exportedClaim_unaryI64_refutation",
            ),
        ),
    ]);
    let observation =
        counterexample::observe(&counterexample::observation_source(&plan, candidate));
    let library = attempt!(library);
    let generated = attempt!(report.generated.clone().ok_or_else(|| {
        output::Failure::error(
            "export-internal",
            "prepared contract has no generated statement".into(),
        )
    }));
    // A disagreement is reported for artifact correspondence (#10) and resolves
    // nothing: it neither confirms nor withdraws the semantic outcome.
    let agrees = observation.completed && observation.output == Some(candidate.output);
    let discrepancy = if observation.completed && !agrees {
        encoding::object([
            (
                "semantic_output",
                encoding::string(candidate.output.to_string()),
            ),
            (
                "compiled_output",
                match observation.output {
                    Some(observed) => encoding::string(observed.to_string()),
                    None => encoding::Json::Null,
                },
            ),
            (
                "status",
                encoding::string("reported for artifact correspondence (#10); resolves nothing"),
            ),
        ])
    } else {
        encoding::Json::Null
    };
    let (category, advertised) =
        match verification::verify(&generated, &module, &library, true, timeout_ms) {
            Ok(acceptance) => {
                record_acceptance(report, acceptance, true);
                let advertised = if agrees {
                    "executable counterexample"
                } else {
                    "semantic refutation"
                };
                ("semantic-refutation", advertised)
            }
            // The consumer refused the instantiated refutation itself: the
            // candidate stays a model and nothing is disproved.
            Err(error) if REFUTATION_REJECTIONS.contains(&error.code) => {
                report.outcome = output::Outcome::Unknown;
                report.diagnostics.push(error.json());
                ("candidate-model-only", "candidate")
            }
            Err(error) => {
                let category = if matches!(error.outcome, output::Outcome::Timeout) {
                    "checker-timeout"
                } else {
                    "not-checked"
                };
                report.fail(error);
                (category, "candidate")
            }
        };
    let ensures = prepared.ensures();
    let semantic_evaluation =
        counterexample::evaluation(prepared, ensures, candidate.input, Some(candidate.output));
    let observed_evaluation = match (observation.completed, observation.output) {
        (true, Some(observed)) => {
            counterexample::evaluation(prepared, ensures, candidate.input, Some(observed))
        }
        (true, None) => "no-normal-return",
        (false, _) => "not-observed",
    };
    report.counterexample = Some(encoding::object([
        ("producer", producer_json),
        ("category", encoding::string(category)),
        ("advertised_as", encoding::string(advertised)),
        (
            "arithmetic",
            encoding::string("two's-complement 64-bit wrapping; values are signed"),
        ),
        ("candidate", counterexample::candidate_json(candidate)),
        // Named only when the host evaluator reads the postcondition as false
        // at the semantic output; a supplied model may not violate it at all.
        (
            "violated",
            if semantic_evaluation == "false" {
                encoding::string("postcondition")
            } else {
                encoding::Json::Null
            },
        ),
        (
            "postcondition",
            encoding::object([
                (
                    "declared",
                    counterexample::declared(prepared, ensures, report.source.as_deref()),
                ),
                ("semantic_evaluation", encoding::string(semantic_evaluation)),
                ("observed_evaluation", encoding::string(observed_evaluation)),
                (
                    "evaluator",
                    encoding::string("host producer evaluator; descriptive, never evidence"),
                ),
            ]),
        ),
        ("observation", observation.json),
        ("discrepancy", discrepancy),
    ]));
    Ok(Some(module))
}

/// Consumer failures that refuse the submitted refutation itself, as opposed
/// to an unavailable sandbox, library or obligation.
const REFUTATION_REJECTIONS: [&str; 7] = [
    "proof-rejected",
    "proof-export-rejected",
    "theorem-missing",
    "theorem-type-mismatch",
    "forbidden-axiom",
    "dependency-limit",
    "independent-recheck-rejected",
];

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; prepare_contract feeds byte-bounded hostile input to the checked frontend and preserves invalid UTF-8 and preparation diagnostics in the report. Rejection must remain a Failure rather than a panic."
)]
fn prepare_contract(
    path: &std::path::Path,
    report: &mut output::Report,
) -> Result<noble_contracts::Prepared, output::Failure> {
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
    Ok(prepared)
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
