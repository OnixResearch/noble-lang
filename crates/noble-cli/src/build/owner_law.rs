//! Independent owner-law proof-required release consumer (VC-OWNER-01,
//! CONTRACT-16).
//!
//! The route is a second, separately controlled release gate over the exact
//! check `noble build --require-proof` performs. It reads a frozen owner law
//! and verifies its bytes against a digest the release owner supplies
//! explicitly; it never adopts a digest from the candidate. It prepares the
//! submitted contract itself, compares the law's subject interface, formal
//! obligation, assumptions and normal-return scope with that regenerated
//! proposition, then runs the shared exact check and binds the checked
//! program/recipe identity, the generated proposition, the checked theorem and
//! proof artifact, and the final emitted artifact before admitting a release.
//!
//! Ordinary execution (`run`, `compile`, `execute`, `verify` and
//! `build --require-proof` without this route) is untouched: no proof policy
//! and no law are read outside this command.

mod arguments;
pub(crate) mod compare;
pub(crate) mod law;
mod terms;

pub const USAGE: &str = "usage:
  noble release --law FILE --law-digest SHA256 --source FILE --contract FILE
                --proof FILE --artifact FILE [--timeout-ms N]

`release` is the separately controlled owner-law gate for an explicitly
proof-required release of an MC2-supported, resource-free Core-Bootstrap
program (VC-OWNER-01, CONTRACT-16). It reads FILE, a frozen
`noble-owner-law/v1` record, and verifies its exact bytes against SHA256, the
64-digit digest the release owner supplies independently of the candidate; a
changed law byte is a different law identity. SOURCE is the submitted program
quotation, CONTRACT the submitted MC1 contract and PROOF the submitted Lean
declaration. ARTIFACT is the already emitted `artifact.wasm` bytes under
review. The command prepares the contract itself, compares the law's subject
interface, input/output bindings, `requires`/`ensures` obligation,
declaration, assumptions and normal-return scope with the regenerated
proposition, then runs the same exact checking path as
`noble build --require-proof` (v3 toolchain binding included) and binds the
checked program/recipe identity, claim statement, theorem declaration, proof
bytes and final emitted artifact. Every mismatch refuses with its own code.

A candidate cannot grant itself a proof: author status is not authority, the
proof library is the pinned MC1/MC2 selection, and no Lean, backend, loader or
host correctness claim follows. Ordinary well-typed execution without the
proof policy is unchanged. Exit status: eligible 0; refused 1 (or the exact
check's own non-proved status); usage or unreadable submission 2; exact check
inconclusive 3; selected toolchain or sandbox unavailable 4; proof timeout
124.";

/// The most artifact bytes one release may bind; equals
/// `sandbox::ARTIFACT_LIMIT`, the compiler's own artifact budget.
const ARTIFACT_LIMIT: usize = 33_554_432;

/// How one release ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Admission {
    Eligible,
    Rejected,
    Error,
    Unsupported,
}

impl Admission {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::Rejected => "rejected",
            Self::Error => "error",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Why the release gate refused, with the exact check that refused.
#[derive(Debug)]
pub(crate) struct Refusal {
    pub(crate) code: &'static str,
    pub(crate) check: &'static str,
    pub(crate) message: std::string::String,
    pub(crate) admission: Admission,
    pub(crate) exit: u8,
}

impl Refusal {
    pub(crate) fn rejected(
        code: &'static str,
        check: &'static str,
        message: std::string::String,
    ) -> Self {
        Self {
            code,
            check,
            message,
            admission: Admission::Rejected,
            exit: 1,
        }
    }

    pub(crate) fn rejected_at(
        code: &'static str,
        check: &'static str,
        message: std::string::String,
        exit: u8,
    ) -> Self {
        Self {
            code,
            check,
            message,
            admission: Admission::Rejected,
            exit,
        }
    }

    pub(crate) fn error(
        code: &'static str,
        check: &'static str,
        message: std::string::String,
    ) -> Self {
        Self {
            code,
            check,
            message,
            admission: Admission::Error,
            exit: 2,
        }
    }

    pub(crate) fn unsupported(
        code: &'static str,
        check: &'static str,
        message: std::string::String,
    ) -> Self {
        Self {
            code,
            check,
            message,
            admission: Admission::Unsupported,
            exit: 4,
        }
    }
}

/// The exact-check observations the admission record retains.
struct Exact {
    outcome: std::string::String,
    release_allowed: bool,
    release_reason: std::string::String,
    release_refusals: std::vec::Vec<&'static str>,
    diagnostics: crate::workflow::encoding::Json,
    recipe_identity: Option<std::string::String>,
    proposition_sha256: std::string::String,
    claim_statement_digest: Option<std::string::String>,
    subject_sha256: std::string::String,
    independent_recheck: bool,
    evidence_class: Option<&'static str>,
    accepted_axioms: std::vec::Vec<std::string::String>,
    lean_sha256: Option<std::string::String>,
    lean_version: Option<std::string::String>,
    library_blake3: Option<std::string::String>,
    library_files: Option<u64>,
    proof_artifact_sha256: std::string::String,
    proof_artifact_bytes: u64,
    emitted_artifact_sha256: std::string::String,
    emitted_artifact_bytes: u64,
    rebuilt_artifact_sha256: Option<std::string::String>,
    emitted_artifact_matches_rebuild: bool,
    contract_sha256: std::string::String,
}

/// Everything the release report retains.
struct Report {
    admission: Admission,
    refusal: Option<Refusal>,
    checks: std::vec::Vec<compare::Check>,
    law: Option<law::Law>,
    law_digest: Option<(std::string::String, std::string::String)>,
    exact: Option<Exact>,
    source_bytes: u64,
    submitted_proof_sha256: std::string::String,
    submitted_contract_sha256: std::string::String,
    submitted_artifact_sha256: std::string::String,
}

impl Report {
    const fn new() -> Self {
        Self {
            admission: Admission::Error,
            refusal: None,
            checks: std::vec::Vec::new(),
            law: None,
            law_digest: None,
            exact: None,
            source_bytes: 0,
            submitted_proof_sha256: std::string::String::new(),
            submitted_contract_sha256: std::string::String::new(),
            submitted_artifact_sha256: std::string::String::new(),
        }
    }

    fn fail(&mut self, refusal: Refusal) {
        self.admission = refusal.admission;
        self.refusal = Some(refusal);
    }
}

/// The release route.
pub fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let mut report = Report::new();
    let options = match arguments::parse(arguments) {
        Ok(options) => options,
        Err(failure) => {
            let refusal = Refusal::error("owner-law-argument", "arguments", failure.message);
            report.fail(refusal);
            return print(&report, 2);
        }
    };
    match decide(&options, &mut report) {
        Ok(()) => {
            report.admission = Admission::Eligible;
            print(&report, 0)
        }
        Err(refusal) => {
            let exit = refusal.exit;
            report.fail(refusal);
            print(&report, exit)
        }
    }
}

/// Emit the one JSON admission record even when the release is refused: a
/// refusal is a decision the caller must be able to read, never a silent exit.
fn print(report: &Report, exit: u8) -> std::process::ExitCode {
    let mut output = json(report).encode();
    output.push('\n');
    match std::io::Write::write_all(&mut std::io::stdout().lock(), output.as_bytes()) {
        Ok(()) => std::process::ExitCode::from(exit),
        Err(_) => std::process::ExitCode::from(2),
    }
}

fn read_bounded(
    path: &std::path::Path,
    limit: usize,
    kind: &'static str,
) -> Result<std::vec::Vec<u8>, crate::workflow::output::Failure> {
    crate::workflow::read_bounded(path, limit, kind)
}

/// One bounded submission file.
struct Submitted {
    bytes: std::vec::Vec<u8>,
    sha256: std::string::String,
}

fn read_submitted(
    path: &std::path::Path,
    limit: usize,
    kind: &'static str,
) -> Result<Submitted, Refusal> {
    let bytes = read_bounded(path, limit, kind).map_err(|failure| {
        Refusal::error(
            "owner-law-unreadable",
            "submission",
            std::format!(
                "cannot read the submitted {kind} at {}: {}",
                path.display(),
                failure.message
            ),
        )
    })?;
    let sha256 = crate::workflow::intrinsic::sha256(&bytes);
    Ok(Submitted { bytes, sha256 })
}

fn decide(options: &arguments::Options, report: &mut Report) -> Result<(), Refusal> {
    // 1. The law's bytes are verified against the owner-supplied digest before
    // any field is believed. The candidate never supplies that digest.
    let law_bytes = read_bounded(&options.law, law::LAW_LIMIT, "owner-law").map_err(|failure| {
        Refusal::error(
            "owner-law-unreadable",
            "law",
            std::format!(
                "cannot read the owner law at {}: {}",
                options.law.display(),
                failure.message
            ),
        )
    })?;
    let computed = crate::workflow::intrinsic::sha256(&law_bytes);
    report.law_digest = Some((computed.clone(), options.law_digest.clone()));
    if computed != options.law_digest {
        return Err(Refusal::rejected(
            "owner-law-digest-mismatch",
            "law-digest",
            std::format!(
                "the law bytes hash to {computed}, not the owner-supplied {}; a changed law byte is a different law identity",
                options.law_digest
            ),
        ));
    }
    let law = law::decode(&law_bytes, computed, options.law_digest.clone())?;

    // 2. The submission is read once; the exact check re-reads the same paths
    // and every identity below is compared against these exact bytes.
    let contract = read_submitted(&options.contract, crate::workflow::SOURCE_LIMIT, "contract")?;
    let source = read_submitted(&options.source, crate::workflow::SOURCE_LIMIT, "source")?;
    let proof = read_submitted(&options.proof, crate::workflow::PROOF_LIMIT, "proof")?;
    let artifact = read_submitted(&options.artifact, ARTIFACT_LIMIT, "artifact")?;
    report.source_bytes = source.bytes.len() as u64;
    report.submitted_contract_sha256 = contract.sha256.clone();
    report.submitted_proof_sha256 = proof.sha256.clone();
    report.submitted_artifact_sha256 = artifact.sha256.clone();

    // 3. The consumer prepares the contract itself and compares every law
    // field with its own regenerated proposition.
    let candidate = compare::compare(&law, &contract.bytes, &mut report.checks)?;
    report.law = Some(law);

    // 4. The exact check is the build's own path, with v3 toolchain binding.
    let (build, products) = exact(options)?;

    // 5. Bind the exact resolved program/recipe identity and the generated
    // proposition the build actually checked.
    let expected_recipe = noble_contracts::companion::statement_digest(&candidate.prepared);
    if build.candidate_digest != Some(expected_recipe) {
        return Err(Refusal::rejected(
            "owner-law-recipe-identity-mismatch",
            "recipe-identity",
            std::format!(
                "the exact check bound recipe identity {:?}, not the compared contract's {expected_recipe:016x}",
                build.candidate_digest
            ),
        ));
    }
    let expected_statement = super::statement_fold(candidate.statement.as_bytes());
    if build.statement_digest != Some(expected_statement) {
        return Err(Refusal::rejected(
            "owner-law-proposition-binding-mismatch",
            "proposition-binding",
            std::format!(
                "the exact check bound claim statement {:?}, not the compared proposition's {expected_statement:016x}",
                build.statement_digest
            ),
        ));
    }

    // 6. The checked theorem and the proof artifact must be the submitted bytes
    // the build independently rechecked.
    if !build.independent_recheck {
        return Err(Refusal::rejected(
            "owner-law-proof-not-exact",
            "checked-theorem",
            "the exact check did not report an independent recheck observation".into(),
        ));
    }
    let checked_proof = products
        .proof
        .as_deref()
        .map(crate::workflow::intrinsic::sha256);
    if checked_proof.as_deref() != Some(proof.sha256.as_str()) {
        return Err(Refusal::rejected(
            "owner-law-proof-not-exact",
            "checked-theorem",
            "the proof artifact the exact check rechecked differs from the submitted proof bytes"
                .into(),
        ));
    }

    // 7. The final emitted artifact is the one exact byte string this build
    // produced; a stale, unrelated or edited artifact is not that byte string.
    let rebuilt = products
        .artifact_wasm
        .as_deref()
        .map(crate::workflow::intrinsic::sha256);
    if rebuilt.as_deref() != Some(artifact.sha256.as_str()) {
        return Err(Refusal::rejected(
            "owner-law-artifact-mismatch",
            "emitted-artifact",
            std::format!(
                "the submitted artifact {} is not the artifact this exact check emits ({})",
                artifact.sha256,
                rebuilt.unwrap_or_else(|| std::string::String::from("no artifact was produced"))
            ),
        ));
    }

    // 8. Everything the build released must still be the compared contract.
    let checked_contract = crate::workflow::intrinsic::sha256(&products.contract);
    if checked_contract != contract.sha256 {
        return Err(Refusal::rejected(
            "owner-law-proposition-binding-mismatch",
            "proposition-binding",
            "the contract the exact check prepared differs from the compared contract bytes".into(),
        ));
    }

    report.exact = Some(Exact {
        outcome: build.outcome.name().into(),
        release_allowed: build.release.allowed,
        release_reason: build.release.reason.clone(),
        release_refusals: build.release.refusals.clone(),
        diagnostics: crate::workflow::encoding::Json::Array(build.diagnostics.clone()),
        recipe_identity: build
            .candidate_digest
            .map(|digest| std::format!("{digest:016x}")),
        proposition_sha256: candidate.statement_sha256.clone(),
        claim_statement_digest: build
            .statement_digest
            .map(|digest| std::format!("{digest:016x}")),
        subject_sha256: crate::workflow::intrinsic::sha256(build.subject.encode().as_bytes()),
        independent_recheck: build.independent_recheck,
        evidence_class: build.evidence_class,
        accepted_axioms: build.accepted_axioms.clone().unwrap_or_default(),
        lean_sha256: selected_text(&build, &["selected_toolchain", "lean", "binary_sha256"]),
        lean_version: selected_text(&build, &["selected_toolchain", "lean", "version"]),
        library_blake3: selected_text(&build, &["selected_toolchain", "library_tree", "blake3"]),
        library_files: selected_number(&build, &["selected_toolchain", "library_tree", "files"]),
        proof_artifact_sha256: proof.sha256.clone(),
        proof_artifact_bytes: proof.bytes.len() as u64,
        emitted_artifact_sha256: artifact.sha256.clone(),
        emitted_artifact_bytes: artifact.bytes.len() as u64,
        rebuilt_artifact_sha256: rebuilt,
        emitted_artifact_matches_rebuild: true,
        contract_sha256: contract.sha256.clone(),
    });
    Ok(())
}

/// Run the shared exact check with the selected toolchain bound (v3).
fn exact(
    options: &arguments::Options,
) -> Result<(super::report::Document, super::Products), Refusal> {
    let lean_binary = crate::sandbox::release_binary().map_err(|failure| {
        Refusal::unsupported(
            "owner-law-exact-check-unsupported",
            "exact-check",
            std::format!(
                "the configured Lean release is unavailable: {}",
                failure.message
            ),
        )
    })?;
    let library_tree = crate::workflow::contract_library_root().map_err(|failure| {
        Refusal::unsupported(
            "owner-law-exact-check-unsupported",
            "exact-check",
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
        policy_revision: super::report::POLICY_REVISION,
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
            let code = exact_code(failure.code);
            let admission = failure.outcome;
            let message = std::format!("{}: {}", failure.code, failure.message);
            let exit = match admission.exit() {
                0 => 1,
                exit => exit,
            };
            let refusal = if matches!(admission, crate::workflow::output::Outcome::Unsupported) {
                Refusal::unsupported("owner-law-exact-check-unsupported", "exact-check", message)
            } else {
                Refusal::rejected_at(code, "exact-check", message, exit)
            };
            build.fail(failure);
            return Err(refusal);
        }
    };
    if !matches!(build.outcome, crate::workflow::output::Outcome::Proved) || !build.release.allowed
    {
        let nested = diagnostic_code(&build.diagnostics);
        let code = exact_code(nested.as_deref().unwrap_or(""));
        let message = std::format!(
            "the exact check reported `{}` with release {}: {}",
            build.outcome.name(),
            if build.release.allowed {
                "allowed"
            } else {
                "blocked"
            },
            build.release.reason
        );
        let exit = match build.outcome.exit() {
            0 => 1,
            exit => exit,
        };
        return Err(Refusal::rejected_at(code, "exact-check", message, exit));
    }
    Ok((build, products))
}

/// The consumer refusal code for the build's own nested refusal.
fn exact_code(code: &str) -> &'static str {
    match code {
        "proof-rejected"
        | "theorem-type-mismatch"
        | "theorem-missing"
        | "forbidden-axiom"
        | "independent-recheck-rejected"
        | "proof-export-rejected"
        | "proof-wire-encoding"
        | "proof-encoding"
        | "dependency-limit" => "owner-law-proof-not-exact",
        "mismatched-subject"
        | "correspondence-mismatch"
        | "artifact-source"
        | "artifact-lowering" => "owner-law-program-not-acceptable",
        _ => "owner-law-exact-check-not-proved",
    }
}

/// The first `code` field of any diagnostic entry.
fn diagnostic_code(entries: &[crate::workflow::encoding::Json]) -> Option<std::string::String> {
    for entry in entries {
        if let crate::workflow::encoding::Json::Object(fields) = entry {
            for (key, value) in fields {
                if *key == "code" {
                    if let crate::workflow::encoding::Json::String(text) = value {
                        return Some(text.clone());
                    }
                }
            }
        }
    }
    None
}

/// Read one nested string field of the build's encoded report.
fn selected_text(build: &super::report::Document, path: &[&str]) -> Option<std::string::String> {
    let report = build.json();
    let mut value = &report;
    for key in path {
        value = field(value, key)?;
    }
    match value {
        crate::workflow::encoding::Json::String(text) => Some(text.clone()),
        _ => None,
    }
}

/// Read one nested unsigned field of the build's encoded report.
fn selected_number(build: &super::report::Document, path: &[&str]) -> Option<u64> {
    let report = build.json();
    let mut value = &report;
    for key in path {
        value = field(value, key)?;
    }
    match value {
        crate::workflow::encoding::Json::Number(number) => Some(*number),
        _ => None,
    }
}

fn field<'a>(
    value: &'a crate::workflow::encoding::Json,
    key: &str,
) -> Option<&'a crate::workflow::encoding::Json> {
    match value {
        crate::workflow::encoding::Json::Object(fields) => fields
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| value),
        _ => None,
    }
}

fn json(report: &Report) -> crate::workflow::encoding::Json {
    use crate::workflow::encoding::{object, optional_string, string, strings, Json};
    let admission = string(report.admission.name());
    let refusal = match &report.refusal {
        Some(refusal) => object([
            ("code", string(refusal.code)),
            ("check", string(refusal.check)),
            ("message", string(&refusal.message)),
        ]),
        None => Json::Null,
    };
    let law_digest = match &report.law_digest {
        Some((computed, expected)) => object([
            ("computed", string(computed)),
            ("expected", string(expected)),
            ("matched", Json::Bool(computed == expected)),
            (
                "basis",
                string(
                    "SHA-256 over the exact law bytes; `expected` is the explicitly supplied --law-digest operand, never a field read from the candidate or the law file",
                ),
            ),
        ]),
        None => Json::Null,
    };
    let law = match &report.law {
        Some(law) => object([
            ("revision", string(&law.revision)),
            ("supersedes", optional_string(law.supersedes.as_deref())),
            ("variant", optional_string(law.variant.as_deref())),
            ("frozen", string(&law.frozen)),
            ("fragment", string(&law.fragment)),
            ("sha256", string(&law.computed_sha256)),
            ("expected_sha256", string(&law.expected_sha256)),
            ("subject_interface", string(&law.subject_interface)),
            ("declaration", string(&law.declaration)),
            ("expected_statement", string(&law.expected_statement)),
            (
                "contract_obligation",
                object([
                    ("language", string(&law.obligation.language)),
                    ("requires", string(&law.obligation.requires)),
                    ("ensures", string(&law.obligation.ensures)),
                    ("arithmetic", string(&law.obligation.arithmetic)),
                ]),
            ),
            (
                "observations",
                object([
                    ("initial", string(&law.observations.initial)),
                    ("final", string(&law.observations.final_observation)),
                ]),
            ),
            ("scope", string(&law.scope)),
            (
                "preconditions",
                Json::Array(law.preconditions.iter().map(string).collect()),
            ),
            (
                "assumptions",
                Json::Array(law.assumptions.iter().map(string).collect()),
            ),
            (
                "proof_library",
                object([
                    ("requirement", string(&law.proof_library.requirement)),
                    ("accepted", string(&law.proof_library.accepted)),
                ]),
            ),
            ("consumer_policy", string(&law.consumer_policy)),
            (
                "candidate_unknown_at_authoring",
                Json::Array(
                    law.candidate_unknown_at_authoring
                        .iter()
                        .map(string)
                        .collect(),
                ),
            ),
            (
                "non_claims",
                Json::Array(law.non_claims.iter().map(string).collect()),
            ),
        ]),
        None => Json::Null,
    };
    let checks = Json::Array(
        report
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
    let exact = match &report.exact {
        Some(exact) => object([
            ("outcome", string(&exact.outcome)),
            ("release_allowed", Json::Bool(exact.release_allowed)),
            ("release_reason", string(&exact.release_reason)),
            (
                "release_refusals",
                Json::Array(exact.release_refusals.iter().map(string).collect()),
            ),
            ("diagnostics", exact.diagnostics.clone()),
            (
                "recipe_identity",
                object([
                    ("digest", optional_string(exact.recipe_identity.as_deref())),
                    (
                        "basis",
                        string(
                            "the deterministic core recomputed this exact statement/subject digest from the retained preparation the exact check used",
                        ),
                    ),
                ]),
            ),
            (
                "generated_proposition",
                object([
                    ("sha256", string(&exact.proposition_sha256)),
                    (
                        "claim_statement_digest",
                        optional_string(exact.claim_statement_digest.as_deref()),
                    ),
                    ("subject_record_sha256", string(&exact.subject_sha256)),
                    (
                        "basis",
                        string(
                            "the consumer regenerated the Lean proposition from the submitted contract and compared it with the build's bound claim-statement fold; `sha256` hashes those exact proposition bytes and `subject_record_sha256` hashes the build's encoded subject record",
                        ),
                    ),
                ]),
            ),
            (
                "checked_theorem",
                object([
                    ("module", string("MC1Proof")),
                    ("declaration", string("proof")),
                    ("type", string("MC1Obligation.claim")),
                    (
                        "proof_artifact_sha256",
                        string(&exact.proof_artifact_sha256),
                    ),
                    (
                        "proof_artifact_bytes",
                        Json::Number(exact.proof_artifact_bytes),
                    ),
                    (
                        "independent_recheck",
                        Json::Bool(exact.independent_recheck),
                    ),
                    (
                        "evidence_class",
                        optional_string(exact.evidence_class),
                    ),
                    (
                        "accepted_transitive_axioms",
                        Json::Array(exact.accepted_axioms.iter().map(string).collect()),
                    ),
                ]),
            ),
            (
                "selected_toolchain",
                object([
                    ("lean_binary_sha256", optional_string(exact.lean_sha256.as_deref())),
                    ("lean_version", optional_string(exact.lean_version.as_deref())),
                    ("library_blake3", optional_string(exact.library_blake3.as_deref())),
                    (
                        "library_files",
                        match exact.library_files {
                            Some(files) => Json::Number(files),
                            None => Json::Null,
                        },
                    ),
                ]),
            ),
            (
                "emitted_artifact",
                object([
                    ("sha256", string(&exact.emitted_artifact_sha256)),
                    ("bytes", Json::Number(exact.emitted_artifact_bytes)),
                    (
                        "rebuilt_sha256",
                        optional_string(exact.rebuilt_artifact_sha256.as_deref()),
                    ),
                    (
                        "matches_rebuild",
                        Json::Bool(exact.emitted_artifact_matches_rebuild),
                    ),
                    (
                        "basis",
                        string(
                            "the submitted artifact was compared byte-for-byte with the artifact the same exact check rebuilt from the submitted source and admitted construction",
                        ),
                    ),
                ]),
            ),
            ("contract_sha256", string(&exact.contract_sha256)),
        ]),
        None => Json::Null,
    };
    object([
        (
            "schema",
            string("noble-owner-law-release/v1"),
        ),
        ("command", string("release")),
        ("stage", string("proof-required-release-admission")),
        ("case", string("CONTRACT-16")),
        ("requirement", string("VC-OWNER-01")),
        ("admission", admission),
        ("refusal", refusal),
        ("law_digest", law_digest),
        ("law", law),
        ("checks", checks),
        ("exact_check", exact),
        (
            "submission",
            object([
                ("source_bytes", Json::Number(report.source_bytes)),
                ("contract_sha256", string(&report.submitted_contract_sha256)),
                ("proof_sha256", string(&report.submitted_proof_sha256)),
                ("artifact_sha256", string(&report.submitted_artifact_sha256)),
            ]),
        ),
        ("owner_law_independent", Json::Bool(true)),
        ("theorem_statement_and_assumptions_compared", Json::Bool(true)),
        (
            "exact_checked_proof_and_artifact_binding_required",
            Json::Bool(true),
        ),
        ("proof_author_status_is_authority", Json::Bool(false)),
        ("pure_fragment_only", Json::Bool(true)),
        ("compiler_backend_host_proof_claimed", Json::Bool(false)),
        ("ordinary_admission", string("unchanged")),
        ("candidate_body_started", Json::Bool(false)),
        ("guest_requests", Json::Number(0)),
        ("protected_operations", Json::Number(0)),
        ("verified_backend_claimed", Json::Bool(false)),
        (
            "non_claims",
            strings(&[
                "No compiler, backend, loader or host correctness claim.",
                "No resource, host-protocol, termination or general Noble source-verification claim.",
                "Ordinary well-typed execution without the proof policy is unchanged (VC-SCOPE-01).",
                "This gate does not re-execute CONTRACT-09/11/13 and does not establish MC2 beyond its retained receipt.",
            ]),
        ),
    ])
}
