//! Opt-in proof-bound artifact admission (PAA-1, ACORR-ENV-01/CACHE-01/TB-01).
//!
//! The consumer joins the two routes that used to be independent: Route B
//! (`build --require-proof`: owner law, independently rechecked Lean evidence,
//! `bind_artifact` subject binding) and Route A (`admit-artifact`: host-selected
//! source, independent rebuild, final-byte equality, fresh-engine
//! instantiation). It compares the subject bytes *and* the certified-call
//! (guard wrapper) bytes, and passes only the compared in-memory buffers to a
//! fresh worker. The order is fixed and every check precedes instantiation:
//!
//! 1. envelope/profile validation;
//! 2. owner-law statement equality;
//! 3. proof recheck;
//! 4. subject binding;
//! 5. tool/helper pin checks;
//! 6. rebuild;
//! 7. subject and wrapper byte equality;
//! 8. start-section/import reflection (the selected worker's own pre-install
//!    rejection and allowlist run here, inside step 10);
//! 9. host authorization;
//! 10. instantiate the compared buffers.
//!
//! The envelope reports a strengthened trusted-build correspondence, never
//! translation validation or semantic preservation. Ordinary execution
//! (`run`, `session`, `build` without this route) is untouched.

mod arguments;

use crate::workflow::encoding::{object, optional_string, string, strings, Json};

pub const USAGE: &str = "usage:
  noble admit-proof-artifact CANDIDATE --law FILE --law-digest SHA256 --source FILE
        --contract FILE --proof FILE --pins FILE --call FILE --argument I64
        [--manifest FILE] [--opt off|on] [--policy-revision N] [--timeout-ms N]
  noble admit-proof-artifact --emit-pins FILE [--cli-sha256 SHA256] [--opt off|on]

`admit-proof-artifact` is the opt-in proof-bound artifact admission envelope
`noble-proof-artifact-admission/v1` (PAA-1) for the MC2 pure quotation profile:
a resource-free Core-Bootstrap program, one closed quotation root, an MC1
partial-correctness contract, LeanExact evidence and at most one selected v1
guard wrapper. It selects the law, source, contract, proof and optimizer mode
itself, rechecks the proof, binds the exact subject, compares the final subject
and certified-call Wasm bytes with an independent rebuild, and instantiates
exactly those compared buffers in a fresh worker. CANDIDATE and --call are the
candidate's subject and certified-call Wasm; --argument is the host-selected
I64 the certified call runs the subject on. Producer manifest digests, source
and tool claims, trust flags and optimization claims are parsed and discarded.
Every refusal is distinct and reports zero guest requests and protected
operations. `--pins` names a `noble-build-report-pins/v1` document; the
`--emit-pins` route observes this toolchain and writes the document the release
owner then freezes. Ordinary execution is unchanged. Exit status: admitted 0;
refused 1; malformed submission 2; unsupported profile or toolchain 4; a timed
out proof check keeps its own status.";

/// The most artifact bytes one admission binds; equals the owner-law route's
/// artifact bound.
const ARTIFACT_LIMIT: usize = 33_554_432;
const MANIFEST_LIMIT: usize = 1024;
/// The optimizer mode names the envelope's `build` field binds.
const OPT_OFF: &str = "off";
const OPT_ON: &str = "on";

/// One PAA-1 refusal: a distinct code, the check that refused and a retained
/// message. Every refusal path reports zero guest requests and protected
/// operations.
struct AdmissionRefusal {
    code: &'static str,
    check: &'static str,
    message: std::string::String,
    exit: u8,
}

impl AdmissionRefusal {
    fn new(code: &'static str, check: &'static str, message: std::string::String) -> Self {
        let exit = match code {
            "invalid-envelope" => 2,
            "unsupported-profile" | "tool-pin-reject" => 4,
            _ => 1,
        };
        Self {
            code,
            check,
            message,
            exit,
        }
    }

    fn invalid(check: &'static str, message: impl Into<std::string::String>) -> Self {
        Self::new("invalid-envelope", check, message.into())
    }

    fn unsupported(check: &'static str, message: impl Into<std::string::String>) -> Self {
        Self::new("unsupported-profile", check, message.into())
    }

    fn law(check: &'static str, message: impl Into<std::string::String>) -> Self {
        Self::new("law-mismatch", check, message.into())
    }

    fn proof(check: &'static str, message: impl Into<std::string::String>) -> Self {
        Self::new("proof-inconclusive", check, message.into())
    }

    fn subject(check: &'static str, message: impl Into<std::string::String>) -> Self {
        Self::new("subject-mismatch", check, message.into())
    }

    fn pin(check: &'static str, message: impl Into<std::string::String>) -> Self {
        Self::new("tool-pin-reject", check, message.into())
    }

    fn correspondence(check: &'static str, message: impl Into<std::string::String>) -> Self {
        Self::new("correspondence-reject", check, message.into())
    }

    fn guard(check: &'static str, message: impl Into<std::string::String>) -> Self {
        Self::new("guard-mismatch", check, message.into())
    }

    fn stale(check: &'static str, message: impl Into<std::string::String>) -> Self {
        Self::new("stale-context", check, message.into())
    }

    fn authorization(check: &'static str, message: impl Into<std::string::String>) -> Self {
        Self::new("authorization-denied", check, message.into())
    }
}

/// Map one build-path failure code to the envelope's distinct refusal.
fn failure_code(code: &str, message: std::string::String) -> AdmissionRefusal {
    match code {
        "source" | "contract-source" | "proof-source" | "usage" | "invalid-timeout" => {
            AdmissionRefusal::invalid("submission", message)
        }
        "artifact-source" | "artifact-lowering" | "preparation-exhausted" => {
            AdmissionRefusal::unsupported("profile", message)
        }
        "artifact-assembly"
        | "toolchain-binding"
        | "build-pin-selection"
        | "build-pin-mismatch" => AdmissionRefusal::pin("toolchain", message),
        "stale-context" | "mismatched-context" => AdmissionRefusal::stale("context", message),
        "mismatched-subject" | "correspondence-mismatch" | "artifact-correspondence" => {
            AdmissionRefusal::subject("subject-binding", message)
        }
        _ => AdmissionRefusal::proof("exact-check", message),
    }
}

/// The release-owner preparation route's result.
struct PinsWritten {
    path: std::path::PathBuf,
    sha256: std::string::String,
    cli_sha256: std::string::String,
    opt: &'static str,
}

/// The complete adopted envelope.
struct Admitted {
    law: Json,
    source: Json,
    semantic: Json,
    claim: Json,
    proof: Json,
    guard: Json,
    toolchain: Json,
    build: Json,
    artifact: Json,
    load: Json,
    invocation: Json,
    pins: Json,
    checks: std::vec::Vec<super::owner_law::compare::Check>,
    candidate: Json,
}

pub fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let mode = match arguments::parse(arguments) {
        Ok(mode) => mode,
        Err(failure) => {
            return print_refusal(&AdmissionRefusal::invalid("arguments", failure.message), 2);
        }
    };
    match mode {
        arguments::Mode::EmitPins(pins) => match emit_pins(&pins) {
            Ok(written) => print_pins(&written),
            Err(refusal) => print_refusal(&refusal, refusal.exit),
        },
        arguments::Mode::Admit(options) => match decide(&options) {
            Ok(admitted) => print_admission(&admitted),
            Err(refusal) => print_refusal(&refusal, refusal.exit),
        },
    }
}

fn print_json(json: &Json, exit: u8) -> std::process::ExitCode {
    let mut output = json.encode();
    output.push('\n');
    match std::io::Write::write_all(&mut std::io::stdout().lock(), output.as_bytes()) {
        Ok(()) => std::process::ExitCode::from(exit),
        Err(_) => std::process::ExitCode::from(2),
    }
}

fn refusal_json(refusal: &AdmissionRefusal) -> Json {
    object([
        ("schema", string("noble-proof-artifact-admission/v1")),
        ("profile", string("MC2-pure-quotation@1")),
        ("command", string("admit-proof-artifact")),
        ("stage", string("proof-bound-artifact-admission")),
        ("outcome", string(refusal.code)),
        ("admission", string("refused")),
        (
            "refusal",
            object([
                ("code", string(refusal.code)),
                ("check", string(refusal.check)),
                ("message", string(&refusal.message)),
            ]),
        ),
        ("guest_requests", Json::Number(0)),
        ("protected_operations", Json::Number(0)),
        (
            "non_claims",
            strings(&[
                "No translation validation, semantic preservation or verified backend is claimed.",
                "Artifact identity and finite control executions do not discharge PO-14/16/17/18/20/21.",
            ]),
        ),
    ])
}

fn print_refusal(refusal: &AdmissionRefusal, exit: u8) -> std::process::ExitCode {
    print_json(&refusal_json(refusal), exit)
}

fn print_pins(written: &PinsWritten) -> std::process::ExitCode {
    let json = object([
        ("schema", string("noble-artifact-admission-pins/v1")),
        ("command", string("admit-proof-artifact")),
        ("stage", string("pin-preparation")),
        ("outcome", string("pins-written")),
        ("path", string(written.path.to_string_lossy())),
        ("sha256", string(&written.sha256)),
        ("cli_sha256", string(&written.cli_sha256)),
        ("opt", string(written.opt)),
        (
            "meaning",
            string(
                "The release owner freezes this observed toolchain document; the admission route refuses unless every identity it observes equals it.",
            ),
        ),
        ("guest_requests", Json::Number(0)),
        ("protected_operations", Json::Number(0)),
    ]);
    print_json(&json, 0)
}

fn print_admission(admitted: &Admitted) -> std::process::ExitCode {
    let checks = Json::Array(
        admitted
            .checks
            .iter()
            .map(|check| {
                object([
                    ("name", string(check.name)),
                    ("status", string(check.status)),
                    ("observed", string(&check.observed)),
                    ("expected", string(&check.expected)),
                ])
            })
            .collect(),
    );
    let json = object([
        ("schema", string("noble-proof-artifact-admission/v1")),
        ("profile", string("MC2-pure-quotation@1")),
        ("command", string("admit-proof-artifact")),
        ("stage", string("proof-bound-artifact-admission")),
        ("admission", string("admitted")),
        ("outcome", string("admitted")),
        ("refusal", Json::Null),
        ("law_comparison", checks),
        ("envelope", envelope(admitted)),
        ("candidate", admitted.candidate.clone()),
        (
            "correspondence_basis",
            string(
                "strengthened trusted build (TB+): exact final-byte equality with the consumer's own rebuild by policy-pinned tools; not translation validation and not semantic preservation",
            ),
        ),
        ("translation_validation", Json::Bool(false)),
        ("semantic_preservation_claimed", Json::Bool(false)),
        ("verified_backend_claimed", Json::Bool(false)),
        (
            "open_obligations",
            strings(&[
                "PO-17 lowering preservation",
                "optimizer preservation",
                "PO-18 loading binding",
                "PO-21 guard/wrapper correspondence",
                "PO-20 erasure theorem",
                "PO-16 consumer decision refinement",
                "their composition into a semantic claim",
            ]),
        ),
        (
            "residual_assumptions",
            strings(&[
                "the noble-wasm compiler and its embedded runtime helpers",
                "wasm-tools assembly and, when selected, wasm-opt optimization",
                "Node/V8 and the OS loader",
                "integrity of the pinned CLI binary (hash-pinned, not proved)",
                "Lean kernel and isolated-checker soundness",
                "CheckObservation authenticity",
                "a non-malicious worker process",
                "the pin document itself",
                "absence of cross-host provenance",
            ]),
        ),
        (
            "unsupported",
            strings(&[
                "definitions, imports and declared modules (#7 profile)",
                "resources and components",
                "live slots and reload",
                "the calculator profile",
                "async",
                "alternate backends",
                "allocation-strategy changes",
            ]),
        ),
        ("ordinary_execution", string("unchanged; PAA-1 is opt-in and labeled")),
        (
            "non_claims",
            strings(&[
                "No verified compiler, Wasm engine, host or malicious-worker resistance claim.",
                "Hashes and finite controls do not prove semantics.",
            ]),
        ),
    ]);
    print_json(&json, 0)
}

fn envelope(admitted: &Admitted) -> Json {
    object([
        ("profile", profile_json()),
        ("owner_law", admitted.law.clone()),
        ("source", admitted.source.clone()),
        ("semantic", admitted.semantic.clone()),
        ("claim", admitted.claim.clone()),
        ("proof", admitted.proof.clone()),
        ("guard", admitted.guard.clone()),
        ("toolchain", admitted.toolchain.clone()),
        ("build", admitted.build.clone()),
        ("artifact", admitted.artifact.clone()),
        ("load", admitted.load.clone()),
        ("invocation", admitted.invocation.clone()),
        ("pins", admitted.pins.clone()),
        (
            "authorization",
            object([
                ("required_effects", strings(&[])),
                ("allowed_effects", strings(&[])),
                ("policy", string("resource-free; no host effect is granted")),
                (
                    "identity",
                    string("host authorization is a separate invocation check outside the envelope identity"),
                ),
            ]),
        ),
        (
            "cache",
            object([
                (
                    "key",
                    string("every envelope field above except authorization"),
                ),
                ("revalidation", string("full; no acceptance is cached")),
                ("fresh_worker_per_admission", Json::Bool(true)),
            ]),
        ),
    ])
}

fn profile_json() -> Json {
    object([
        ("name", string("MC2-pure-quotation@1")),
        (
            "shape",
            string("resource-free Core-Bootstrap, one closed inert quotation root, empty dependency closure"),
        ),
        ("definitions", string("refused")),
        ("imports", string("refused")),
        ("resources", string("refused")),
        ("live_slots", string("refused")),
        ("calculator", string("refused")),
    ])
}

fn read(
    path: &std::path::Path,
    limit: usize,
    kind: &'static str,
) -> Result<std::vec::Vec<u8>, AdmissionRefusal> {
    crate::workflow::read_bounded(path, limit, kind).map_err(|failure| {
        AdmissionRefusal::invalid(
            "submission",
            std::format!(
                "cannot read {kind} at {}: {}",
                path.display(),
                failure.message
            ),
        )
    })
}

struct Submitted {
    bytes: std::vec::Vec<u8>,
    sha256: std::string::String,
}

fn read_submitted(
    path: &std::path::Path,
    limit: usize,
    kind: &'static str,
) -> Result<Submitted, AdmissionRefusal> {
    let bytes = read(path, limit, kind)?;
    let sha256 = crate::workflow::intrinsic::sha256(&bytes);
    Ok(Submitted { bytes, sha256 })
}

/// One consumer-selected law: a frozen `noble-owner-law/v1` record or an MC1
/// contract whose digest the release owner supplied independently.
struct SelectedLaw {
    mode: &'static str,
    id: std::string::String,
    revision: std::string::String,
    record: Option<super::owner_law::law::Law>,
    prepared: noble_contracts::Prepared,
    statement: std::string::String,
    statement_sha256: std::string::String,
    computed_sha256: std::string::String,
    expected_sha256: std::string::String,
}

fn first_significant(bytes: &[u8]) -> Option<u8> {
    bytes
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

fn map_law_refusal(refusal: super::owner_law::Refusal) -> AdmissionRefusal {
    let code = match refusal.code {
        "owner-law-record-invalid" | "owner-law-unreadable" => "invalid-envelope",
        "owner-law-policy-mismatch" | "owner-law-fragment-unsupported" => "unsupported-profile",
        "owner-law-exact-check-unsupported" => "tool-pin-reject",
        "owner-law-proof-not-exact" | "owner-law-exact-check-not-proved" => "proof-inconclusive",
        "owner-law-program-not-acceptable" => "subject-mismatch",
        _ => "law-mismatch",
    };
    AdmissionRefusal::new(code, refusal.check, refusal.message)
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the owner-law comparison appends to the accumulating check record, which the consumer later reports whole, so it is borrowed mutably as the growable record owner_law::compare::compare takes"
)]
fn select_law(
    law_bytes: &[u8],
    law_digest: &str,
    contract_bytes: &[u8],
    checks: &mut std::vec::Vec<super::owner_law::compare::Check>,
) -> Result<SelectedLaw, AdmissionRefusal> {
    let computed_sha256 = crate::workflow::intrinsic::sha256(law_bytes);
    if computed_sha256 != law_digest {
        return Err(AdmissionRefusal::law(
            "law-digest",
            std::format!(
                "the law bytes hash to {computed_sha256}, not the owner-supplied {law_digest}; a changed law byte is a different law identity"
            ),
        ));
    }
    if first_significant(law_bytes) != Some(b'{') {
        // Contract mode: the law is an MC1 contract the consumer prepares
        // itself. The submitted contract must be those exact bytes, so the
        // candidate can never substitute a weaker law.
        if law_bytes != contract_bytes {
            return Err(AdmissionRefusal::law(
                "law-contract",
                "the submitted contract bytes differ from the consumer-selected law contract",
            ));
        }
        let prepared = noble_contracts::prepare(contract_bytes, crate::core::SOURCE_LIMITS)
            .map_err(|error| {
                AdmissionRefusal::law(
                    "law-preparation",
                    std::format!(
                        "the law contract is not accepted ordinary typing: {:?}: {}",
                        error.kind,
                        error.message
                    ),
                )
            })?;
        if !prepared.definitions().is_empty() || !prepared.params().is_empty() {
            return Err(AdmissionRefusal::unsupported(
                "profile",
                "the law contract carries definitions or parameters outside the MC2 pure quotation profile",
            ));
        }
        let statement = noble_contracts::export_lean(&prepared);
        let statement_sha256 = crate::workflow::intrinsic::sha256(statement.as_bytes());
        return Ok(SelectedLaw {
            mode: "contract",
            id: prepared.name().into(),
            revision: std::string::String::from("contract-v1"),
            prepared,
            statement,
            statement_sha256,
            computed_sha256,
            expected_sha256: law_digest.into(),
            record: None,
        });
    }
    let law = super::owner_law::law::decode(law_bytes, computed_sha256.clone(), law_digest.into())
        .map_err(map_law_refusal)?;
    let candidate = super::owner_law::compare::compare(&law, contract_bytes, checks)
        .map_err(map_law_refusal)?;
    Ok(SelectedLaw {
        mode: "frozen-record",
        id: law.revision.clone(),
        revision: law.revision.clone(),
        prepared: candidate.prepared,
        statement: candidate.statement,
        statement_sha256: candidate.statement_sha256,
        computed_sha256,
        expected_sha256: law_digest.into(),
        record: Some(law),
    })
}

/// The exact check, through the same `build --require-proof` path and with the
/// v3 toolchain binding.
fn exact(
    options: &arguments::Options,
) -> Result<(super::report::Document, super::Products), AdmissionRefusal> {
    let lean_binary = crate::sandbox::release_binary().map_err(|failure| {
        AdmissionRefusal::pin(
            "lean-release",
            std::format!(
                "the configured Lean release is unavailable: {}",
                failure.message
            ),
        )
    })?;
    let library_tree = crate::workflow::contract_library_root().map_err(|failure| {
        AdmissionRefusal::pin(
            "contract-library",
            std::format!(
                "the configured contract library root is unavailable: {}",
                failure.message
            ),
        )
    })?;
    let exact = super::arguments::Options {
        source: options.source.clone(),
        contract: options.contract.clone(),
        evidence: Some(options.proof.clone()),
        is_refutation: false,
        out: None,
        timeout_ms: options.timeout_ms,
        schema: super::report::Schema::V3,
        lean_binary: Some(lean_binary),
        library_tree: Some(library_tree),
        pins: None,
        policy_revision: options.policy_revision,
    };
    let mut build = super::report::Document::new();
    build.require_proof = true;
    build.source_path = options.source.to_string_lossy().into_owned();
    build.contract_path = options.contract.to_string_lossy().into_owned();
    build.timeout_ms = options.timeout_ms;
    build.schema = super::report::Schema::V3;
    let products = match super::checked(&exact, &mut build) {
        Ok(products) => products,
        Err(failure) => {
            let message = std::format!("{}: {}", failure.code, failure.message);
            return Err(failure_code(failure.code, message));
        }
    };
    if !matches!(build.outcome, crate::workflow::output::Outcome::Proved) || !build.release.allowed
    {
        let nested = first_refusal(&build.release.refusals);
        let code = match nested.as_deref() {
            Some("mismatched-subject") | Some("correspondence-mismatch") => "subject-mismatch",
            Some("stale-context") | Some("mismatched-context") => "stale-context",
            _ => "proof-inconclusive",
        };
        let exit = match build.outcome.exit() {
            0 => 1,
            exit => exit,
        };
        return Err(AdmissionRefusal {
            code,
            check: "exact-check",
            message: std::format!(
                "the exact check reported `{}` with release {}: {} ({})",
                build.outcome.name(),
                if build.release.allowed {
                    "allowed"
                } else {
                    "blocked"
                },
                build.release.reason,
                build.release.refusals.join(",")
            ),
            exit,
        });
    }
    Ok((build, products))
}

fn first_refusal(refusals: &[&'static str]) -> Option<std::string::String> {
    refusals
        .first()
        .map(|code| std::string::String::from(*code))
}

/// One consumer-owned rebuild: the exact WAT and Wasm plus the selected
/// worker's verified tool record.
struct Recompiled {
    source_sha256: std::string::String,
    wat: std::vec::Vec<u8>,
    wat_sha256: std::string::String,
    wasm: std::vec::Vec<u8>,
    wasm_sha256: std::string::String,
    tools: std::vec::Vec<u8>,
}

/// Compile one consumer-selected source under the pure quotation profile. The
/// profile is validated in step 1, before the law or any proof check.
fn compile_profile(source: &[u8]) -> Result<std::vec::Vec<u8>, AdmissionRefusal> {
    let frontend = noble_contracts::source::Session::new();
    let prepared = frontend
        .prepare(source, &[], crate::core::SOURCE_LIMITS)
        .map_err(|error| {
            AdmissionRefusal::unsupported(
                "profile",
                std::format!(
                    "the consumer-selected source is not accepted ordinary typing: {:?}",
                    error.diagnostic().kind
                ),
            )
        })?;
    let submission = prepared.submission().ok_or_else(|| {
        AdmissionRefusal::unsupported(
            "profile",
            "the consumer-selected source is a definition, not the one closed quotation root PAA-1 admits",
        )
    })?;
    let module = noble_wasm::source::Compiler::new()
        .prepare(submission)
        .map_err(|diagnostic| {
            AdmissionRefusal::unsupported(
                "profile",
                match diagnostic {
                    noble_wasm::Diagnostic::Invalid => {
                        "the independent compiler refused the source as invalid"
                    }
                    noble_wasm::Diagnostic::Exhausted => {
                        "the independent compiler exhausted its bounded budget"
                    }
                    noble_wasm::Diagnostic::Unsupported => {
                        "the independent compiler does not support the source"
                    }
                    noble_wasm::Diagnostic::Defective => {
                        "the independent compiler reported a defective request"
                    }
                },
            )
        })?;
    Ok(module.wat().to_vec())
}

fn rebuild(
    wat: std::vec::Vec<u8>,
    source_sha256: &str,
    optimized: bool,
) -> Result<Recompiled, AdmissionRefusal> {
    let directory = crate::workflow::artifacts::Temporary::create()
        .map_err(|failure| AdmissionRefusal::invalid("rebuild", failure.message))?;
    let assembly =
        crate::core::worker::assembly::assemble_wat_optimized(&directory.path, &wat, optimized)
            .map_err(|message| AdmissionRefusal::pin("rebuild", message))?;
    Ok(Recompiled {
        source_sha256: source_sha256.into(),
        wat_sha256: crate::workflow::intrinsic::sha256(&wat),
        wasm_sha256: crate::workflow::intrinsic::sha256(&assembly.bytes),
        wat,
        wasm: assembly.bytes,
        tools: assembly.tools,
    })
}

/// Verify every tool identity the selection names against the record the
/// selected worker itself wrote.
fn verify_tools(
    selection: &super::report::pins::Selection,
    tools: &[u8],
    check: &'static str,
) -> Result<(), AdmissionRefusal> {
    let observed = super::report::pins::observe_tools(tools)
        .map_err(|failure| AdmissionRefusal::pin(check, failure.message))?;
    super::report::pins::verify_tools(selection, &observed)
        .map_err(|failure| AdmissionRefusal::pin(check, failure.message))
}

fn mode_name(optimized: bool) -> &'static str {
    if optimized {
        OPT_ON
    } else {
        OPT_OFF
    }
}

/// The arguments the envelope's `semantic` field reports: constants of the
/// pinned CLI plus the candidate's own semantic revision.
fn semantic(prepared: &noble_contracts::Prepared) -> Json {
    object([
        (
            "candidate_revision",
            Json::Number(u64::from(prepared.candidate().revision)),
        ),
        ("ir_revision", Json::Number(1)),
        ("candidate_format", string("noble-kernel untrusted Candidate")),
        ("kernel_acceptance", string("noble-kernel acceptance environment")),
        (
            "basis",
            string("constant of the pinned CLI and the retained preparation; no producer field selects one"),
        ),
    ])
}

fn law_json(law: &SelectedLaw) -> Json {
    let mut entries: std::vec::Vec<(&'static str, Json)> = std::vec![
        ("mode", string(law.mode)),
        ("id", string(&law.id)),
        ("revision", string(&law.revision)),
    ];
    entries.push((
        "digest",
        object([
            ("computed", string(&law.computed_sha256)),
            ("expected", string(&law.expected_sha256)),
            ("matched", Json::Bool(law.computed_sha256 == law.expected_sha256)),
            (
                "basis",
                string("SHA-256 over the exact law bytes; `expected` is the invoker-supplied operand, never a candidate field"),
            ),
        ]),
    ));
    entries.push(("statement_sha256", string(&law.statement_sha256)));
    if let Some(record) = &law.record {
        entries.push(("record_revision", string(&record.revision)));
        entries.push(("supersedes", optional_string(record.supersedes.as_deref())));
        entries.push(("variant", optional_string(record.variant.as_deref())));
        entries.push(("fragment", string(&record.fragment)));
        entries.push(("declaration", string(&record.declaration)));
        entries.push(("expected_statement", string(&record.expected_statement)));
        entries.push((
            "obligation",
            object([
                ("language", string(&record.obligation.language)),
                ("requires", string(&record.obligation.requires)),
                ("ensures", string(&record.obligation.ensures)),
                ("arithmetic", string(&record.obligation.arithmetic)),
            ]),
        ));
        entries.push((
            "assumptions",
            Json::Array(record.assumptions.iter().map(string).collect()),
        ));
        entries.push(("scope", string(&record.scope)));
        entries.push((
            "non_claims",
            Json::Array(record.non_claims.iter().map(string).collect()),
        ));
    } else {
        entries.push(("declaration", string(law.prepared.name())));
        entries.push((
            "obligation",
            object([
                ("language", string("noble-contract/1 (MC1)")),
                ("requires", string("regenerated contract precondition")),
                ("ensures", string("regenerated contract postcondition")),
                (
                    "basis",
                    string("the consumer selected this exact contract as its law; the candidate cannot substitute another"),
                ),
            ]),
        ));
    }
    Json::Object(entries)
}

fn claim_json(law: &SelectedLaw) -> Json {
    object([
        ("kind", string("normal-return partial correctness")),
        ("statement_sha256", string(&law.statement_sha256)),
        (
            "statement_basis",
            string("full regenerated Lean proposition bytes, not the 64-bit report fold"),
        ),
        (
            "subject_interface",
            string(subject_interface(&law.prepared)),
        ),
        (
            "definitions",
            Json::Number(law.prepared.definitions().len() as u64),
        ),
        (
            "parameters",
            Json::Number(law.prepared.params().len() as u64),
        ),
    ])
}

fn subject_interface(prepared: &noble_contracts::Prepared) -> std::string::String {
    let mut text = std::string::String::new();
    for (index, input) in prepared.inputs().iter().enumerate() {
        if index > 0 {
            text.push(' ');
        }
        text.push_str(&input.name);
        text.push(':');
        text.push_str(&std::format!("{:?}", input.ty));
    }
    text.push_str(" -> ");
    for (index, output) in prepared.outputs().iter().enumerate() {
        if index > 0 {
            text.push(' ');
        }
        text.push_str(&output.name);
        text.push(':');
        text.push_str(&std::format!("{:?}", output.ty));
    }
    text
}

/// The producer manifest fields PAA-1 parses and then discards.
struct Manifest {
    claimed_effects: std::vec::Vec<std::string::String>,
    discarded: std::vec::Vec<&'static str>,
}

fn parse_manifest(bytes: &[u8]) -> Result<Manifest, AdmissionRefusal> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| AdmissionRefusal::invalid("manifest", "candidate manifest is not UTF-8"))?;
    let value: serde_json::Value = serde_json::from_str(text).map_err(|error| {
        AdmissionRefusal::invalid(
            "manifest",
            std::format!("candidate manifest is not JSON: {error}"),
        )
    })?;
    let object = value.as_object().ok_or_else(|| {
        AdmissionRefusal::invalid("manifest", "candidate manifest must be an object")
    })?;
    let mut claimed_effects: std::vec::Vec<std::string::String> = std::vec::Vec::new();
    let mut discarded: std::vec::Vec<&'static str> = std::vec::Vec::new();
    for (key, value) in object {
        let known: Option<&'static str> = match key.as_str() {
            "claimed_effects" => Some("claimed_effects"),
            "trusted_correspondence" => Some("trusted_correspondence"),
            "allowed" => Some("allowed"),
            "source" => Some("source"),
            "digest" => Some("digest"),
            "tool_sha256" => Some("tool_sha256"),
            "opt" => Some("opt"),
            _ => None,
        };
        let Some(name) = known else {
            return Err(AdmissionRefusal::invalid(
                "manifest",
                std::format!("unknown candidate manifest field `{key}`"),
            ));
        };
        discarded.push(name);
        match name {
            "claimed_effects" => {
                let values = value.as_array().ok_or_else(|| {
                    AdmissionRefusal::invalid(
                        "manifest",
                        "candidate claimed_effects must be an array",
                    )
                })?;
                if values.len() > 3 {
                    return Err(AdmissionRefusal::invalid(
                        "manifest",
                        "too many claimed effects",
                    ));
                }
                for value in values {
                    let effect = value.as_str().ok_or_else(|| {
                        AdmissionRefusal::invalid("manifest", "effect identity must be text")
                    })?;
                    claimed_effects.push(effect.into());
                }
            }
            "trusted_correspondence" | "allowed" => {
                if !value.is_boolean() {
                    return Err(AdmissionRefusal::invalid(
                        "manifest",
                        "candidate manifest boolean has wrong type",
                    ));
                }
            }
            _ => {
                if !value.is_string() {
                    return Err(AdmissionRefusal::invalid(
                        "manifest",
                        "candidate manifest text has wrong type",
                    ));
                }
            }
        }
    }
    Ok(Manifest {
        claimed_effects,
        discarded,
    })
}

fn manifest_json(manifest: &Manifest) -> Json {
    object([
        ("present", Json::Bool(true)),
        (
            "claimed_effects",
            Json::Array(manifest.claimed_effects.iter().map(string).collect()),
        ),
        (
            "discarded_fields",
            Json::Array(manifest.discarded.iter().map(string).collect()),
        ),
        (
            "basis",
            string("every producer field is parsed for shape and then discarded; none selects source, trust, toolchain or optimization"),
        ),
    ])
}

fn no_manifest() -> Json {
    object([("present", Json::Bool(false))])
}

/// One admission's decision. Every check precedes instantiation.
fn decide(options: &arguments::Options) -> Result<Admitted, AdmissionRefusal> {
    // 1. Envelope and submission.
    let candidate = read_submitted(
        &options.artifact,
        ARTIFACT_LIMIT,
        "candidate subject artifact",
    )?;
    let call = read_submitted(
        &options.call,
        ARTIFACT_LIMIT,
        "candidate certified-call artifact",
    )?;
    let manifest = match &options.manifest {
        Some(path) => Some(parse_manifest(&read(
            path,
            MANIFEST_LIMIT,
            "candidate manifest",
        )?)?),
        None => None,
    };
    let law_bytes = read(
        &options.law,
        super::owner_law::law::LAW_LIMIT.max(crate::workflow::SOURCE_LIMIT),
        "owner law",
    )?;
    let contract = read_submitted(&options.contract, crate::workflow::SOURCE_LIMIT, "contract")?;
    let source = read_submitted(&options.source, crate::workflow::SOURCE_LIMIT, "source")?;
    let proof = read_submitted(&options.proof, crate::workflow::PROOF_LIMIT, "proof")?;
    let mut checks: std::vec::Vec<super::owner_law::compare::Check> = std::vec::Vec::new();

    // 1. Envelope/profile validation: the consumer-selected source must be one
    // closed executable quotation root under the pure profile, decided before
    // the law, the proof or any tool is consulted.
    let subject_wat = compile_profile(&source.bytes)?;

    // 2. Owner-law statement equality.
    let law = select_law(
        &law_bytes,
        &options.law_digest,
        &contract.bytes,
        &mut checks,
    )?;

    // 3-4. Proof recheck and subject binding, through the shared exact path.
    let (build, products) = exact(options)?;
    if !build.independent_recheck {
        return Err(AdmissionRefusal::proof(
            "proof-recheck",
            "the exact check reported no independent recheck observation",
        ));
    }
    let checked_proof = products
        .proof
        .as_deref()
        .map(crate::workflow::intrinsic::sha256);
    if checked_proof.as_deref() != Some(proof.sha256.as_str()) {
        return Err(AdmissionRefusal::proof(
            "proof-artifact",
            "the proof bytes the exact check rechecked differ from the submitted proof",
        ));
    }
    let evaluated = noble_contracts::export_lean(&law.prepared);
    if evaluated != law.statement {
        return Err(AdmissionRefusal::law(
            "law-statement",
            "the law statement the consumer regenerated is not stable across preparations",
        ));
    }
    if build.statement_digest != Some(super::statement_fold(law.statement.as_bytes())) {
        return Err(AdmissionRefusal::law(
            "law-statement-binding",
            "the exact check bound a claim statement that differs from the consumer-selected law statement",
        ));
    }
    if build.candidate_digest != Some(noble_contracts::companion::statement_digest(&law.prepared)) {
        return Err(AdmissionRefusal::subject(
            "subject-recipe",
            "the exact check bound a recipe identity that differs from the compared contract",
        ));
    }
    let checked_contract = crate::workflow::intrinsic::sha256(&products.contract);
    if checked_contract != contract.sha256 {
        return Err(AdmissionRefusal::law(
            "law-contract-binding",
            "the contract the exact check prepared differs from the compared contract bytes",
        ));
    }

    // 5. Tool/helper pin checks before any rebuild or instantiation.
    let lean_binary = crate::sandbox::release_binary()
        .map_err(|failure| AdmissionRefusal::pin("lean-release", failure.message))?;
    let library_tree = crate::workflow::contract_library_root()
        .map_err(|failure| AdmissionRefusal::pin("contract-library", failure.message))?;
    let selected = super::report::toolchain::select(&lean_binary, &library_tree)
        .map_err(|failure| AdmissionRefusal::pin("toolchain-binding", failure.message))?;
    let selection = super::report::pins::select(&options.pins)
        .map_err(|failure| AdmissionRefusal::pin("pin-selection", failure.message))?;
    let observed = super::report::pins::observe(&selected)
        .map_err(|failure| AdmissionRefusal::pin("pin-observe", failure.message))?;
    super::report::pins::verify_static(&selection, &observed)
        .map_err(|failure| AdmissionRefusal::pin("pin-mismatch", failure.message))?;
    if selection.opt_mode != mode_name(options.optimized) {
        return Err(AdmissionRefusal::pin(
            "pin-opt-mode",
            std::format!(
                "the pin document selects optimizer mode `{}` but the consumer selected `{}`",
                selection.opt_mode,
                mode_name(options.optimized)
            ),
        ));
    }
    let mut subject_worker = crate::core::worker::Engine::start_admission(options.optimized)
        .map_err(|failure| {
            AdmissionRefusal::unsupported(
                "engine",
                std::format!(
                    "the fresh admission worker did not start: {}",
                    failure.message
                ),
            )
        })?;
    let worker_tools = subject_worker
        .tools_record()
        .ok_or_else(|| {
            AdmissionRefusal::pin(
                "worker-tools",
                "the admission worker reported no tool identity",
            )
        })?
        .to_vec();
    verify_tools(&selection, &worker_tools, "worker-tools")?;

    // 6. The consumer's own rebuild in the selected mode.
    let subject = rebuild(subject_wat, &source.sha256, options.optimized)?;
    verify_tools(&selection, &subject.tools, "rebuild-tools")?;
    if options.optimized && !matches!(selection.opt_mode.as_str(), OPT_ON) {
        return Err(AdmissionRefusal::pin(
            "pin-opt-mode",
            "the optimizer was not selected",
        ));
    }
    if let Some(products_wasm) = products.artifact_wasm.as_deref() {
        if !options.optimized && products_wasm != subject.wasm.as_slice() {
            return Err(AdmissionRefusal::correspondence(
                "rebuild-consistency",
                "two consumer rebuilds of the same source disagree, so no exact artifact exists",
            ));
        }
    }

    // 6b. The certified call: the subject program run on the host-selected
    // argument, through the bound guard wrapper when the precondition is not
    // `true`.
    let precondition_true = precondition_is_true(&law.prepared);
    let guard = guard_json(
        products.wrapper_template,
        products.wrapper.as_deref(),
        precondition_true,
    )?;
    let call_source = certified_call(options.argument, &source.bytes, products.wrapper.as_deref());
    let call_wat = compile_profile(call_source.as_bytes())?;
    let certified = rebuild(
        call_wat,
        &crate::workflow::intrinsic::sha256(call_source.as_bytes()),
        options.optimized,
    )?;
    verify_tools(&selection, &certified.tools, "call-tools")?;

    // 7. Final-byte equality for the compared subject and call artifacts.
    if subject.wasm != candidate.bytes {
        return Err(AdmissionRefusal::correspondence(
            "subject-bytes",
            std::format!(
                "the candidate subject bytes {} differ from the consumer rebuild {}",
                candidate.sha256,
                subject.wasm_sha256
            ),
        ));
    }
    if certified.wasm != call.bytes {
        // The certified call is the bound wrapper's compiled invocation when a
        // guard is present, so a substituted or omitted wrapper is a guard
        // mismatch rather than the subject's byte correspondence.
        let message = std::format!(
            "the candidate certified-call bytes {} differ from the consumer rebuild {}",
            call.sha256,
            certified.wasm_sha256
        );
        return Err(if matches!(guard, Json::Null) {
            AdmissionRefusal::correspondence("call-bytes", message)
        } else {
            AdmissionRefusal::guard("call-bytes", message)
        });
    }

    // 8-9. Opaque container checks and host authorization: the pure profile
    // imports no effect, so an effect import is an authorization denial.
    reject_start_section(&subject.wasm)?;
    reject_start_section(&certified.wasm)?;
    if !required_effects_empty(&subject.wasm) || !required_effects_empty(&certified.wasm) {
        return Err(AdmissionRefusal::authorization(
            "host-authorization",
            "the compared module imports an effect the resource-free profile never grants",
        ));
    }

    // 10. Instantiate exactly the compared buffers, each in its own fresh
    // worker. The worker re-assembles the offered WAT and refuses unless the
    // candidate equals it, so the comparison is independent of this process.
    let subject_report = admit(&mut subject_worker, &candidate, &subject, &source.bytes)?;
    let mut call_worker =
        crate::core::worker::Engine::start_admission(options.optimized).map_err(|failure| {
            AdmissionRefusal::unsupported(
                "engine",
                std::format!("the fresh call worker did not start: {}", failure.message),
            )
        })?;
    let call_tools = call_worker
        .tools_record()
        .ok_or_else(|| {
            AdmissionRefusal::pin("worker-tools", "the call worker reported no tool identity")
        })?
        .to_vec();
    verify_tools(&selection, &call_tools, "call-worker-tools")?;
    let call_report = admit(&mut call_worker, &call, &certified, call_source.as_bytes())?;

    let selection_sha256 = selection.document_sha256.clone();
    let toolchain = object([
        ("cli_sha256", string(&observed.cli_sha256)),
        (
            "helpers",
            object([
                ("root", string(super::report::pins::HELPER_ROOT)),
                ("count", Json::Number(observed.helpers.len() as u64)),
                ("set_sha256", string(&observed.helper_set_sha256)),
                (
                    "listing",
                    Json::Array(
                        observed
                            .helpers
                            .iter()
                            .map(|(path, sha256)| {
                                object([("path", string(path)), ("sha256", string(sha256))])
                            })
                            .collect(),
                    ),
                ),
            ]),
        ),
        ("lean_binary_sha256", string(&observed.lean_binary_sha256)),
        ("lean_toolchain", string(observed.lean_toolchain)),
        ("lean_commit", string(observed.lean_commit)),
        ("library_blake3", string(&observed.library_blake3)),
        (
            "tools",
            tools_json(
                &selection.tools.node,
                &selection.tools.wasm_tools,
                &selection.tools.wasm_opt,
            ),
        ),
        (
            "basis",
            string("every hashed identity was compared to the frozen pin document before any rebuild or instantiation"),
        ),
    ]);
    let pins = object([
        ("selection_sha256", string(&selection_sha256)),
        ("opt_mode", string(&selection.opt_mode)),
        (
            "status",
            string("selected; every observed static identity, the selected worker's tool record and the optimizer mode matched"),
        ),
    ]);
    let optimizer_flags = if options.optimized {
        string(optimizer_flags())
    } else {
        string("")
    };
    let build_json = object([
        ("opt", string(mode_name(options.optimized))),
        ("optimizer_flags", optimizer_flags),
        (
            "basis",
            string("consumer selection; the candidate's optimization claim was discarded"),
        ),
    ]);
    let proof_json = object([
        ("class", string("LeanExact")),
        ("artifact_sha256", string(&proof.sha256)),
        ("artifact_bytes", Json::Number(proof.bytes.len() as u64)),
        ("independent_recheck", Json::Bool(build.independent_recheck)),
        (
            "accepted_transitive_axioms",
            Json::Array(
                build
                    .accepted_axioms
                    .clone()
                    .unwrap_or_default()
                    .iter()
                    .map(string)
                    .collect(),
            ),
        ),
        ("lean_binary_sha256", json_or_null(&build, &["selected_toolchain", "lean", "binary_sha256"])),
        ("lean_version", json_or_null(&build, &["selected_toolchain", "lean", "version"])),
        ("library_blake3", json_or_null(&build, &["selected_toolchain", "library_tree", "blake3"])),
        ("policy_revision", Json::Number(u64::from(options.policy_revision))),
        (
            "basis",
            string("the isolated consumer rechecked these exact evidence bytes against the selected Lean release and contract library"),
        ),
    ]);
    let artifact = object([
        (
            "subject",
            object([
                ("source_sha256", string(&source.sha256)),
                ("wat_sha256", string(&subject.wat_sha256)),
                ("wasm_sha256", string(&subject.wasm_sha256)),
                ("wasm_bytes", Json::Number(subject.wasm.len() as u64)),
                (
                    "op_events",
                    string(
                        "bound by the companion core's canonical event comparison in bind_artifact",
                    ),
                ),
            ]),
        ),
        (
            "certified_call",
            object([
                ("source_sha256", string(&certified.source_sha256)),
                ("wat_sha256", string(&certified.wat_sha256)),
                ("wasm_sha256", string(&certified.wasm_sha256)),
                ("wasm_bytes", Json::Number(certified.wasm.len() as u64)),
                ("argument", Json::Signed(options.argument)),
            ]),
        ),
    ]);
    let load = object([
        ("subject_buffer_sha256", string(&subject_report.sha256)),
        ("call_buffer_sha256", string(&call_report.sha256)),
        (
            "buffers_sha256",
            string(crate::workflow::intrinsic::sha256(
                std::format!("{}:{}", subject_report.sha256, call_report.sha256).as_bytes(),
            )),
        ),
        ("subject_worker", Json::Number(u64::from(subject_report.worker))),
        ("call_worker", Json::Number(u64::from(call_report.worker))),
        ("subject_test_barrier", subject_report.barrier),
        ("call_test_barrier", call_report.barrier),
        (
            "basis",
            string("SHA-256 of the exact in-memory buffers passed to new WebAssembly.Module; never re-read from disk"),
        ),
    ]);
    let invocation = object([
        ("argument", Json::Signed(options.argument)),
        ("outcome", string(&call_report.outcome)),
        ("stack", call_report.stack),
        ("guest_requests", Json::Number(call_report.guest_requests)),
        (
            "protected_operations",
            Json::Number(call_report.protected_operations),
        ),
        (
            "identity",
            string("host-selected invocation observations outside the envelope identity"),
        ),
    ]);
    Ok(Admitted {
        law: law_json(&law),
        source: object([
            ("sha256", string(&source.sha256)),
            ("bytes", Json::Number(source.bytes.len() as u64)),
            (
                "canonical_op_events",
                string("compared by bind_artifact against the contract subject before any rebuild"),
            ),
            ("dependency_closure", strings(&[])),
        ]),
        semantic: semantic(&law.prepared),
        claim: claim_json(&law),
        proof: proof_json,
        guard,
        toolchain,
        build: build_json,
        artifact,
        load,
        invocation,
        pins,
        checks,
        candidate: object([
            ("subject_sha256", string(&candidate.sha256)),
            ("call_sha256", string(&call.sha256)),
            (
                "manifest",
                match &manifest {
                    Some(manifest) => manifest_json(manifest),
                    None => no_manifest(),
                },
            ),
        ]),
    })
}

fn tools_json(
    node: &super::report::pins::ToolIdentity,
    wasm_tools: &super::report::pins::ToolIdentity,
    wasm_opt: &super::report::pins::ToolIdentity,
) -> Json {
    let tool = |identity: &super::report::pins::ToolIdentity, v8: bool| {
        let mut entries: std::vec::Vec<(&'static str, Json)> = std::vec::Vec::new();
        entries.push(("path", string(&identity.path)));
        entries.push(("version", string(&identity.version)));
        entries.push(("sha256", string(&identity.sha256)));
        if v8 {
            entries.push(("v8", optional_string(identity.v8.as_deref())));
        }
        Json::Object(entries)
    };
    object([
        ("node", tool(node, true)),
        ("wasm_tools", tool(wasm_tools, false)),
        ("wasm_opt", tool(wasm_opt, false)),
        (
            "v8_flags",
            strings(&[
                "--no-liftoff",
                "--no-wasm-lazy-compilation",
                "--no-wasm-tier-up",
                "--stack-size=1024",
                "--max-old-space-size=256",
            ]),
        ),
    ])
}

fn optimizer_flags() -> &'static str {
    // The selected configuration's flags, as the worker applies them.
    "-O2"
}

fn json_or_null(build: &super::report::Document, path: &[&str]) -> Json {
    let mut value = &build.json();
    for key in path {
        let next = match value {
            Json::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        };
        match next {
            Some(next) => value = next,
            None => return Json::Null,
        }
    }
    value.clone()
}

fn precondition_is_true(prepared: &noble_contracts::Prepared) -> bool {
    let index = prepared.requires() as usize;
    prepared
        .expressions()
        .get(index)
        .is_some_and(|expression| matches!(expression.kind, noble_contracts::ExprKind::Bool(true)))
}

fn template_name(template: noble_contracts::companion::GuardTemplate) -> &'static str {
    match template {
        noble_contracts::companion::GuardTemplate::LtI64Max => "LtI64Max",
        noble_contracts::companion::GuardTemplate::NeI64Min => "NeI64Min",
        noble_contracts::companion::GuardTemplate::EqI64Literal(_) => "EqI64Literal",
    }
}

fn template_literal(template: noble_contracts::companion::GuardTemplate) -> Option<i64> {
    match template {
        noble_contracts::companion::GuardTemplate::LtI64Max => Some(i64::MAX),
        noble_contracts::companion::GuardTemplate::NeI64Min => Some(i64::MIN),
        noble_contracts::companion::GuardTemplate::EqI64Literal(value) => Some(value),
    }
}

fn guard_json(
    template: Option<noble_contracts::companion::GuardTemplate>,
    wrapper: Option<&str>,
    precondition_true: bool,
) -> Result<Json, AdmissionRefusal> {
    if precondition_true {
        if template.is_some() || wrapper.is_some() {
            return Err(AdmissionRefusal::guard(
                "guard-presence",
                "the contract precondition is `true`, so no guard wrapper may be bound",
            ));
        }
        return Ok(Json::Null);
    }
    let Some(template) = template else {
        return Err(AdmissionRefusal::guard(
            "guard-template",
            "the contract precondition is not `true` and the selected registry entry has no recognized v1 guard template",
        ));
    };
    let Some(wrapper) = wrapper else {
        return Err(AdmissionRefusal::guard(
            "guard-wrapper",
            "the selected guard template produced no invocation wrapper",
        ));
    };
    Ok(object([
        ("template_set", string("v1")),
        ("template", string(template_name(template))),
        (
            "literal",
            match template_literal(template) {
                Some(value) => Json::Signed(value),
                None => Json::Null,
            },
        ),
        (
            "wrapper_source_sha256",
            string(crate::workflow::intrinsic::sha256(wrapper.as_bytes())),
        ),
        (
            "erasable",
            Json::Bool(false),
        ),
        (
            "basis",
            string("recomputed from the registry entry's recognized template; the candidate cannot supply it, and a non-`true` precondition requires one"),
        ),
    ]))
}

fn certified_call(argument: i64, source: &[u8], wrapper: Option<&str>) -> std::string::String {
    let mut text = std::format!("{argument} ");
    text.push_str(&std::string::String::from_utf8_lossy(source));
    match wrapper {
        Some(wrapper) => {
            text.push(' ');
            text.push_str(wrapper);
            text.push_str(" run\n");
        }
        None => text.push_str(" run\n"),
    }
    text
}

struct Instantiated {
    sha256: std::string::String,
    worker: u32,
    outcome: std::string::String,
    stack: Json,
    guest_requests: u64,
    protected_operations: u64,
    barrier: Json,
}

fn admit(
    worker: &mut crate::core::worker::Engine,
    candidate: &Submitted,
    rebuilt: &Recompiled,
    source: &[u8],
) -> Result<Instantiated, AdmissionRefusal> {
    let identity = worker.worker_id();
    let report = worker
        .admit(&candidate.bytes, &rebuilt.wat, source, &[], &[])
        .map_err(|failure| AdmissionRefusal::unsupported("instantiation", failure.message))?;
    let value: serde_json::Value = serde_json::from_str(&report.json).map_err(|error| {
        AdmissionRefusal::invalid(
            "instantiation",
            std::format!("worker report is not JSON: {error}"),
        )
    })?;
    let code = value
        .get("outcome")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("internal-failure");
    if code != "normal" {
        return Err(match code {
            "correspondence-reject" => AdmissionRefusal::correspondence(
                "worker-comparison",
                serde_json::Value::to_string(&value),
            ),
            "effect-manifest-reject" | "effect-policy-reject" => AdmissionRefusal::authorization(
                "worker-authorization",
                serde_json::Value::to_string(&value),
            ),
            "stale-session-reject" => AdmissionRefusal::stale("worker", "the worker was not fresh"),
            "invalid-artifact" => {
                AdmissionRefusal::invalid("worker-artifact", serde_json::Value::to_string(&value))
            }
            _ => AdmissionRefusal::unsupported("worker", serde_json::Value::to_string(&value)),
        });
    }
    let module = value
        .get("module")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let wasm = module
        .get("wasm_sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            AdmissionRefusal::invalid("load", "the admission report carries no module identity")
        })?;
    if wasm != rebuilt.wasm_sha256 {
        return Err(AdmissionRefusal::correspondence(
            "load-buffer",
            std::format!(
                "the instantiated buffer {wasm} is not the compared buffer {}",
                rebuilt.wasm_sha256
            ),
        ));
    }
    let admission = value
        .get("admission")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Ok(Instantiated {
        sha256: wasm.into(),
        worker: identity,
        outcome: code.into(),
        stack: stack_json(&value),
        guest_requests: value
            .get("guest_requests")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        protected_operations: value
            .get("protected_operations")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        barrier: match admission.get("test_barrier") {
            Some(serde_json::Value::Null) | None => Json::Null,
            Some(value) => Json::String(value.to_string()),
        },
    })
}

fn stack_json(report: &serde_json::Value) -> Json {
    let stack = report
        .get("stack")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    Json::String(stack.to_string())
}

fn reject_start_section(bytes: &[u8]) -> Result<(), AdmissionRefusal> {
    let parser = wasmparser::Parser::new(0);
    for payload in parser.parse_all(bytes) {
        match payload {
            Ok(wasmparser::Payload::StartSection { .. }) => {
                return Err(AdmissionRefusal::invalid(
                    "start-section",
                    "a compared module has a start section",
                ))
            }
            Ok(_) => {}
            Err(error) => {
                return Err(AdmissionRefusal::invalid(
                    "wasm-format",
                    std::format!("a compared module is not valid Wasm: {error}"),
                ))
            }
        }
    }
    Ok(())
}

fn required_effects_empty(bytes: &[u8]) -> bool {
    let parser = wasmparser::Parser::new(0);
    for payload in parser.parse_all(bytes) {
        if let Ok(wasmparser::Payload::ImportSection(section)) = payload {
            for import in section.into_iter().flatten() {
                if matches!(import.ty, wasmparser::TypeRef::Func(_)) {
                    return false;
                }
            }
        }
    }
    true
}

/// Observe the selected toolchain and write the pin document the release owner
/// freezes. The document records what this process observed; the admission
/// route still refuses unless its own observation equals it.
fn emit_pins(options: &arguments::Pins) -> Result<PinsWritten, AdmissionRefusal> {
    let lean_binary = crate::sandbox::release_binary()
        .map_err(|failure| AdmissionRefusal::pin("lean-release", failure.message))?;
    let library_tree = crate::workflow::contract_library_root()
        .map_err(|failure| AdmissionRefusal::pin("contract-library", failure.message))?;
    let selected = super::report::toolchain::select(&lean_binary, &library_tree)
        .map_err(|failure| AdmissionRefusal::pin("toolchain-binding", failure.message))?;
    let observed = super::report::pins::observe(&selected)
        .map_err(|failure| AdmissionRefusal::pin("pin-observe", failure.message))?;
    let worker = crate::core::worker::Engine::start_admission(options.optimized)
        .map_err(|failure| AdmissionRefusal::unsupported("engine", failure.message))?;
    let record = worker.tools_record().ok_or_else(|| {
        AdmissionRefusal::pin(
            "worker-tools",
            "the admission worker reported no tool identity",
        )
    })?;
    let tools = super::report::pins::observe_tools(record)
        .map_err(|failure| AdmissionRefusal::pin("worker-tools", failure.message))?;
    let cli_sha256 = options
        .cli_sha256
        .clone()
        .unwrap_or_else(|| observed.cli_sha256.clone());
    let document = serde_json::json!({
        "schema": super::report::pins::SCHEMA,
        "opt_mode": mode_name(options.optimized),
        "cli": { "sha256": cli_sha256 },
        "helpers": {
            "root": super::report::pins::HELPER_ROOT,
            "count": observed.helpers.len(),
            "set_sha256": observed.helper_set_sha256,
        },
        "lean": {
            "binary_sha256": observed.lean_binary_sha256,
            "toolchain": observed.lean_toolchain,
            "commit": observed.lean_commit,
            "library_blake3": observed.library_blake3,
        },
        "tools": {
            "node": tool_pin(&tools.node),
            "wasm_tools": tool_pin(&tools.wasm_tools),
            "wasm_opt": tool_pin(&tools.wasm_opt),
        },
    });
    let bytes = serde_json::to_vec_pretty(&document).map_err(|error| {
        AdmissionRefusal::invalid("pins", std::format!("cannot encode pins: {error}"))
    })?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&options.output)
        .map_err(|error| {
            AdmissionRefusal::invalid(
                "pins",
                std::format!("cannot create {}: {error}", options.output.display()),
            )
        })?;
    std::io::Write::write_all(&mut file, &bytes).map_err(|error| {
        AdmissionRefusal::invalid(
            "pins",
            std::format!("cannot write {}: {error}", options.output.display()),
        )
    })?;
    Ok(PinsWritten {
        path: options.output.clone(),
        sha256: crate::workflow::intrinsic::sha256(&bytes),
        cli_sha256,
        opt: mode_name(options.optimized),
    })
}

fn tool_pin(identity: &super::report::pins::ToolIdentity) -> serde_json::Value {
    serde_json::json!({
        "path": identity.path,
        "version": identity.version,
        "sha256": identity.sha256,
    })
}
