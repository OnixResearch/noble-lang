//! Proof-required build (`VC-TOOL-01/02/04`, CONTRACT-09/13).
//!
//! The shell prepares the contract, independently rechecks any supplied Lean
//! evidence through the shared consumer-side admission seam (which itself uses
//! the sandboxed producer/consumer pipeline), hands a complete offer to the
//! deterministic core, and releases the artifact only when the core admits an
//! applicable accepted `proved` claim whose evidence binds the exact subject,
//! claim, context and current policy revision.

mod arguments;
mod artifacts;
mod compilation;
mod release;
mod report;

pub const USAGE: &str = "usage:
  noble build --require-proof SOURCE --contract FILE [--proof FILE | --refutation FILE]
              [--out NEW_DIR] [--timeout-ms N]
  noble companions

build --require-proof compiles SOURCE, prepares the contract, and releases the
artifact only for an applicable accepted `proved` claim whose evidence binds the
exact subject, claim assumptions, semantic/toolchain context and current policy
revision. SOURCE must export one inert program quotation, for example [ 1 + ];
its complete interface and recipe must match the contract subject. The build
does not invoke that program. SOURCE is the single positional operand; `--contract` names the
contract file; options may follow `build` in any order. Each option occurs at
most once and `--proof`/`--refutation` are mutually exclusive.
disproved, unknown, timeout, unsupported, error and not-run all block release
and remain distinguishable in the report; tests and review records never satisfy
a required proof. `--out` creates a new directory (never overwriting) and writes
report.json, wrapper.noble, contract.noble, artifact.wat, artifact.wasm and
proof.lean when present. The emitted Wasm is build/loading evidence (PO-17/18),
never a verified backend correspondence. Exit status follows VC-TOOL-02:
proved 0, disproved 1, error 2, unknown 3, unsupported 4, timeout 124, not-run 0.";

/// Evidence class names used in the report; the core owns acceptance.
const CLASS_PROOF: &str = "LeanExact";
const CLASS_REFUTATION: &str = "LeanRefutation";

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; run records argument, preparation, evidence and emission failures in the JSON report before choosing the outcome's exit status. External failures must remain reportable rather than become assertion panics."
)]
pub fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let mut report = report::Document::new();
    let options = match arguments::parse(arguments) {
        Ok(options) => options,
        Err(error) => {
            report.fail(error);
            return print_report(&report);
        }
    };
    report.require_proof = options.require_proof;
    report.source_path = options.source.to_string_lossy().into_owned();
    report.contract_path = options.contract.to_string_lossy().into_owned();
    report.timeout_ms = options.timeout_ms;
    let products = match execute(&options, &mut report) {
        Ok(products) => Some(products),
        Err(error) => {
            report.fail(error);
            None
        }
    };
    if let (Some(destination), Some(products)) = (&options.out, products.as_ref()) {
        let emission = artifacts::Emission {
            contract: &products.contract,
            wrapper: products
                .wrapper
                .as_deref()
                .filter(|_| report.release.allowed),
            artifact_wat: products
                .artifact_wat
                .as_deref()
                .filter(|_| report.release.allowed),
            artifact_wasm: products
                .artifact_wasm
                .as_deref()
                .filter(|_| report.release.allowed),
            proof: products.proof.as_deref(),
        };
        if let Err(error) = artifacts::emit(destination, &mut report, &emission) {
            report.fail(error);
        }
    }
    print_report(&report)
}

fn print_report(report: &report::Document) -> std::process::ExitCode {
    let mut output = report.json().encode();
    output.push('\n');
    match std::io::Write::write_all(&mut std::io::stdout().lock(), output.as_bytes()) {
        Ok(()) => std::process::ExitCode::from(report.outcome.exit()),
        Err(_) => std::process::ExitCode::from(2),
    }
}

/// Owned build products retained for `--out`; borrows satisfy `Emission`.
struct Products {
    contract: std::vec::Vec<u8>,
    wrapper: Option<std::string::String>,
    artifact_wat: Option<std::vec::Vec<u8>>,
    artifact_wasm: Option<std::vec::Vec<u8>>,
    proof: Option<std::vec::Vec<u8>>,
}

struct Artifact {
    prepared: noble_contracts::source::Prepared,
    wat: std::vec::Vec<u8>,
    wasm: std::vec::Vec<u8>,
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; execute performs bounded filesystem reads, runtime compilation, isolated consumer admission and allocated reporting, which cannot be const. Hostile source, missing tools and release refusals remain typed Failure or reported refusals rather than asserted success assumptions."
)]
fn execute(
    options: &arguments::Options,
    report: &mut report::Document,
) -> Result<Products, crate::workflow::output::Failure> {
    let Inputs {
        contract,
        source,
        evidence,
    } = attempt!(load(options, report));

    // The artifact must compile and load before release is considered.
    // Instantiation does not execute its subject.
    let artifact = attempt!(compilation::prepare(&source));
    report.artifacts = report::artifact_correspondence(
        Some(artifact.wat.len() as u64),
        Some(artifact.wasm.len() as u64),
    );

    // The consumer prepares the contract and rechecks any complete offer. An
    // empty offer is left to the core, which reports it as inconclusive.
    let offer = match &evidence {
        Some(bytes) if !bytes.is_empty() => Some((bytes.as_slice(), options.is_refutation)),
        Some(_) | None => None,
    };
    let mut core = noble_contracts::companion::Core::new(crate::core::EVIDENCE_LIMITS);
    core.set_policy(report::POLICY_REVISION);
    let admission = match crate::workflow::admission::admit(
        &mut core,
        &contract,
        crate::core::SOURCE_LIMITS,
        offer,
        options.timeout_ms,
    ) {
        Ok(admission) => admission,
        Err(refusal) => {
            report.outcome = refusal.outcome;
            report
                .diagnostics
                .push(crate::workflow::admission::refusal_json(&refusal));
            report.release = report::ReleaseDecision::blocked(
                "the consumer did not accept a claim about the exact subject",
            );
            return Ok(Products {
                contract,
                wrapper: None,
                artifact_wat: Some(artifact.wat),
                artifact_wasm: Some(artifact.wasm),
                proof: evidence,
            });
        }
    };
    report.record_application(&admission.contract, &contract, admission.evidence);
    let wrapper = wrapper_source(&core, admission.decision.contract);
    if evidence.is_none() {
        report.outcome = crate::workflow::output::Outcome::NotRun;
        report.release = if options.require_proof {
            report::ReleaseDecision::blocked("proof required but no evidence offer was supplied")
        } else {
            release::allowed("permissive build; no proof was required")
        };
    } else {
        let submission = attempt!(artifact.prepared.submission().ok_or_else(|| {
            crate::workflow::output::Failure::error(
                "artifact-source",
                "missing accepted expression".into(),
            )
        }));
        release::record(report, &core, admission.decision, submission);
    }
    Ok(Products {
        contract,
        wrapper,
        artifact_wat: Some(artifact.wat),
        artifact_wasm: Some(artifact.wasm),
        proof: evidence,
    })
}

/// Emit the prechecked live-handle wrapper. It consumes an argument and the
/// artifact's exported Program; it never reparses or rebuilds that subject.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; wrapper_source retrieves an owned guard-template vector and allocates invocation-wrapper text through runtime core APIs. Vector iteration and wrapper generation are not const on the pinned compiler."
)]
fn wrapper_source(
    core: &noble_contracts::companion::Core,
    contract: Option<noble_contracts::companion::ContractId>,
) -> Option<std::string::String> {
    match contract {
        Some(contract) => match core.guard_templates(contract) {
            Ok(templates) => templates
                .into_iter()
                .next()
                .map(|template| core.invocation_wrapper(template)),
            Err(_) => None,
        },
        None => None,
    }
}

fn artifact_diagnostic(
    code: &'static str,
    message: std::string::String,
) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::object([
        ("code", crate::workflow::encoding::string(code)),
        ("message", crate::workflow::encoding::string(message)),
    ])
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

const fn fold(mut hash: u64, bytes: &[u8]) -> u64 {
    let mut byte_index = 0usize;
    while byte_index < bytes.len() {
        hash ^= bytes[byte_index] as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
        byte_index = byte_index.saturating_add(1);
    }
    hash
}

const fn statement_fold(statement: &[u8]) -> u64 {
    fold(FNV_OFFSET, statement)
}

struct Inputs {
    contract: std::vec::Vec<u8>,
    source: std::vec::Vec<u8>,
    evidence: Option<std::vec::Vec<u8>>,
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; load uses non-const filesystem reads and owned byte buffers. read_bounded enforces the separate source and proof byte budgets and returns I/O or budget failures through Failure; external files must not become assertion preconditions."
)]
fn load(
    options: &arguments::Options,
    report: &mut report::Document,
) -> Result<Inputs, crate::workflow::output::Failure> {
    let contract = attempt!(crate::workflow::read_bounded(
        &options.contract,
        crate::workflow::SOURCE_LIMIT,
        "contract-source"
    ));
    let source = attempt!(crate::workflow::read_bounded(
        &options.source,
        crate::workflow::SOURCE_LIMIT,
        "source"
    ));
    let evidence = match &options.evidence {
        Some(path) => Some(attempt!(crate::workflow::read_bounded(
            path,
            crate::workflow::PROOF_LIMIT,
            "proof-source"
        ))),
        None => None,
    };
    report.evidence_supplied = evidence.is_some();
    report.declaration_bytes = evidence.as_ref().map_or(0, |bytes| bytes.len() as u64);
    report.evidence_class = match &evidence {
        None => None,
        Some(_) if options.is_refutation => Some(CLASS_REFUTATION),
        Some(_) => Some(CLASS_PROOF),
    };

    Ok(Inputs {
        contract,
        source,
        evidence,
    })
}
