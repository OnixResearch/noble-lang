//! Saved intrinsic acceptance evidence (`noble-intrinsic-evidence/v1`).
//!
//! `verify-module --emit DIR` writes a compact record of one accepted proof
//! run: the exact submitted module source, the module/version, the selected
//! model and checker revisions, the transitive `use` dependency closure and,
//! per proof, the kind, statement digest, revisions and original subject.
//! `goal-view --evidence DIR` and `verify-module --evidence DIR` read such a
//! record without rewriting or deleting it.
//!
//! An offered record is input, not a fresh or authenticated checker result.
//! The goal view reports its match or mismatch without granting `checked`.
//! `verify-module` refuses a stale record with `intrinsic-stale-proof` before
//! the consumer runs. A matching record never substitutes for that run.

use noble_contracts::intrinsic::{
    CheckedProof, PendingGoal, ProofBatch, ProofKind, SubjectBinding,
};
use noble_contracts::Limits;

use super::{encoding, intrinsic, output};

pub(crate) const SCHEMA: &str = "noble-intrinsic-evidence/v1";
/// Evidence directories one command may read or write.
pub(crate) const DIRECTORY_LIMIT: usize = 16;
const EVIDENCE_BYTES: usize = 1_048_576;

/// The selected model and checker revisions. `named_*` are present only for a
/// module that carries a revision-2 contract proof.
#[derive(Clone)]
pub(crate) struct Revisions {
    pub(crate) model: std::string::String,
    pub(crate) checker: std::string::String,
    pub(crate) named_model: Option<std::string::String>,
    pub(crate) named_checker: Option<std::string::String>,
}

/// One contract subject as it was accepted.
#[derive(Clone)]
pub(crate) struct Subject {
    module: std::string::String,
    version: u32,
    definition: std::string::String,
    module_source_sha256: std::string::String,
    definition_source_sha256: std::string::String,
    generated_statement_sha256: Option<std::string::String>,
}

/// One proof record.
#[derive(Clone)]
pub(crate) struct Proof {
    name: std::string::String,
    kind: &'static str,
    statement_digest: Option<std::string::String>,
    /// `generated-statement` for a contract proof whose exact generated
    /// statement was digested; `source-and-checker` for a pure proof, whose
    /// Lean claim text is only reachable by the consumer.
    statement_basis: &'static str,
    model_revision: std::string::String,
    checker_revision: std::string::String,
    subject: Option<Subject>,
}

/// One `use` dependency record, with its own transitive closure.
#[derive(Clone)]
pub(crate) struct Dependency {
    module: std::string::String,
    version: u32,
    name: std::string::String,
    declaration: std::string::String,
    source_sha256: std::string::String,
    model_revision: std::string::String,
    checker_revision: std::string::String,
    dependencies: std::vec::Vec<Dependency>,
}

/// One accepted module proof run.
#[derive(Clone)]
pub(crate) struct Record {
    pub(crate) source: std::vec::Vec<u8>,
    source_sha256: std::string::String,
    module: std::string::String,
    version: u32,
    revisions: Revisions,
    dependencies: std::vec::Vec<Dependency>,
    proofs: std::vec::Vec<Proof>,
}

/// One loaded evidence directory.
pub(crate) struct Loaded {
    pub(crate) directory: std::string::String,
    pub(crate) evidence_sha256: std::string::String,
    pub(crate) record: Record,
}

/// The current prepared subject's staleness key.
pub(crate) struct Current {
    source_sha256: std::string::String,
    module: std::string::String,
    version: u32,
    dependencies: std::vec::Vec<Dependency>,
    proofs: std::vec::Vec<Proof>,
}

fn failure(code: &'static str, message: std::string::String) -> output::Failure {
    output::Failure::error(code, message)
}

/// The exact generated Lean statement digest for one goal, or `None` for a
/// pure proposition whose Lean claim only the consumer's parser produces.
fn statement_digest(
    goal: &PendingGoal,
    limits: Limits,
) -> Result<Option<std::string::String>, output::Failure> {
    let text = match goal {
        PendingGoal::Pure { .. } => return Ok(None),
        PendingGoal::Contract { contract } if contract.revision == 2 => {
            noble_contracts::intrinsic::prepare_named_contract(contract, limits)
                .map_err(|problem| failure("proof-artifact", problem.message))?
        }
        PendingGoal::Contract { contract } => {
            let prepared = noble_contracts::intrinsic::prepare_contract(contract, limits)
                .map_err(|problem| failure("proof-artifact", problem.message))?;
            noble_contracts::export_lean(&prepared)
        }
    };
    Ok(Some(intrinsic::sha256(text.as_bytes())))
}

fn subject_row(binding: &SubjectBinding, statement: Option<std::string::String>) -> Subject {
    Subject {
        module: binding.module.clone(),
        version: binding.version,
        definition: binding.definition.clone(),
        module_source_sha256: intrinsic::sha256(&binding.module_source),
        definition_source_sha256: intrinsic::sha256(&binding.definition_source),
        generated_statement_sha256: statement,
    }
}

/// Build the accepted record of a checked batch. Every statement digest is
/// regenerated from the batch's declared goal, never from the proof text.
pub(crate) fn record(
    batch: &ProofBatch,
    proofs: &[CheckedProof],
    limits: Limits,
) -> Result<Record, output::Failure> {
    if batch.obligations.len() != proofs.len() {
        return Err(failure(
            "proof-artifact",
            "checked proof count differs from the obligation count".into(),
        ));
    }
    let mut rows = std::vec::Vec::with_capacity(proofs.len());
    for (obligation, checked) in batch.obligations.iter().zip(proofs) {
        let kind = match checked.kind {
            ProofKind::Pure => "pure",
            ProofKind::Contract => "contract",
            ProofKind::NamedContract => "named-contract",
        };
        let digest = statement_digest(&obligation.goal, limits)?;
        let basis = if digest.is_some() {
            "generated-statement"
        } else {
            "source-and-checker"
        };
        let subject = match &obligation.goal {
            PendingGoal::Contract { contract } => {
                Some(subject_row(&contract.subject, digest.clone()))
            }
            PendingGoal::Pure { .. } => None,
        };
        rows.push(Proof {
            name: checked.name.clone(),
            kind,
            statement_digest: digest,
            statement_basis: basis,
            model_revision: checked.model_revision.clone(),
            checker_revision: checked.checker_revision.clone(),
            subject,
        });
    }
    let named = proofs
        .iter()
        .any(|proof| proof.kind == ProofKind::NamedContract);
    let checker = intrinsic::checker_revision();
    let revisions = Revisions {
        model: intrinsic::model_revision(),
        named_model: named.then(intrinsic::named_model_revision),
        named_checker: named.then(|| intrinsic::named_checker_revision(&checker)),
        checker,
    };
    Ok(Record {
        source: batch.source.clone(),
        source_sha256: intrinsic::sha256(&batch.source),
        module: batch.module.clone(),
        version: batch.version,
        revisions,
        dependencies: batch.dependencies.iter().map(dependency_record).collect(),
        proofs: rows,
    })
}

fn dependency_record(dependency: &noble_contracts::intrinsic::ProofDependency) -> Dependency {
    Dependency {
        module: dependency.module.clone(),
        version: dependency.version,
        name: dependency.name.clone(),
        declaration: dependency.declaration.name.clone(),
        source_sha256: intrinsic::sha256(&dependency.source),
        model_revision: dependency.model_revision.clone(),
        checker_revision: dependency.checker_revision.clone(),
        dependencies: dependency
            .dependencies
            .iter()
            .map(dependency_record)
            .collect(),
    }
}

fn dependency_json(dependency: &Dependency) -> encoding::Json {
    encoding::object([
        ("module", encoding::string(&dependency.module)),
        (
            "version",
            encoding::Json::Number(u64::from(dependency.version)),
        ),
        ("name", encoding::string(&dependency.name)),
        ("declaration", encoding::string(&dependency.declaration)),
        ("source_sha256", encoding::string(&dependency.source_sha256)),
        (
            "model_revision",
            encoding::string(&dependency.model_revision),
        ),
        (
            "checker_revision",
            encoding::string(&dependency.checker_revision),
        ),
        (
            "dependencies",
            encoding::Json::Array(
                dependency
                    .dependencies
                    .iter()
                    .map(dependency_json)
                    .collect(),
            ),
        ),
    ])
}

fn subject_json(subject: &Subject) -> encoding::Json {
    encoding::object([
        ("module", encoding::string(&subject.module)),
        (
            "version",
            encoding::Json::Number(u64::from(subject.version)),
        ),
        ("definition", encoding::string(&subject.definition)),
        (
            "module_source_sha256",
            encoding::string(&subject.module_source_sha256),
        ),
        (
            "definition_source_sha256",
            encoding::string(&subject.definition_source_sha256),
        ),
        (
            "generated_statement_sha256",
            encoding::optional_string(subject.generated_statement_sha256.as_deref()),
        ),
    ])
}

fn proof_json(proof: &Proof) -> encoding::Json {
    encoding::object([
        ("name", encoding::string(&proof.name)),
        ("kind", encoding::string(proof.kind)),
        (
            "statement_digest",
            encoding::optional_string(proof.statement_digest.as_deref()),
        ),
        ("statement_basis", encoding::string(proof.statement_basis)),
        ("model_revision", encoding::string(&proof.model_revision)),
        (
            "checker_revision",
            encoding::string(&proof.checker_revision),
        ),
        (
            "subject",
            proof
                .subject
                .as_ref()
                .map_or(encoding::Json::Null, subject_json),
        ),
    ])
}

/// Encode one record as its stored JSON document.
pub(crate) fn record_json(record: &Record) -> encoding::Json {
    encoding::object([
        ("schema", encoding::string(SCHEMA)),
        ("command", encoding::string("verify-module")),
        ("outcome", encoding::string("proved")),
        ("independent_recheck", encoding::Json::Bool(true)),
        ("source_sha256", encoding::string(&record.source_sha256)),
        (
            "module",
            encoding::object([
                ("name", encoding::string(&record.module)),
                ("version", encoding::Json::Number(u64::from(record.version))),
            ]),
        ),
        (
            "revisions",
            encoding::object([
                ("model", encoding::string(&record.revisions.model)),
                ("checker", encoding::string(&record.revisions.checker)),
                (
                    "named_model",
                    encoding::optional_string(record.revisions.named_model.as_deref()),
                ),
                (
                    "named_checker",
                    encoding::optional_string(record.revisions.named_checker.as_deref()),
                ),
            ]),
        ),
        (
            "dependencies",
            encoding::Json::Array(record.dependencies.iter().map(dependency_json).collect()),
        ),
        (
            "proofs",
            encoding::Json::Array(record.proofs.iter().map(proof_json).collect()),
        ),
    ])
}

/// Write one `--emit` directory: the report always, the source and the
/// evidence record only when a proof run was accepted. An existing directory
/// is refused, so a retained record is never overwritten.
pub(crate) fn write(
    directory: &std::path::Path,
    record: Option<&Record>,
    report: &encoding::Json,
) -> Result<(), output::Failure> {
    std::fs::create_dir(directory).map_err(|error| {
        failure(
            "emit-destination",
            std::format!("new output directory {}: {error}", directory.display()),
        )
    })?;
    let mut text = report.encode();
    text.push('\n');
    write_new(&directory.join("report.json"), text.as_bytes())?;
    let Some(record) = record else {
        return Ok(());
    };
    write_new(&directory.join("source.noble"), &record.source)?;
    let mut evidence = record_json(record).encode();
    evidence.push('\n');
    write_new(&directory.join("evidence.json"), evidence.as_bytes())
}

fn write_new(path: &std::path::Path, bytes: &[u8]) -> Result<(), output::Failure> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            failure(
                "emit-destination",
                std::format!("{}: {error}", path.display()),
            )
        })?;
    std::io::Write::write_all(&mut file, bytes).map_err(|error| {
        failure(
            "emit-destination",
            std::format!("{}: {error}", path.display()),
        )
    })
}

fn text_field(
    value: &serde_json::Value,
    name: &'static str,
) -> Result<std::string::String, String> {
    value
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(std::string::String::from)
        .ok_or_else(|| std::format!("evidence field {name} must be text"))
}

fn number_field(value: &serde_json::Value, name: &'static str) -> Result<u32, String> {
    value
        .get(name)
        .and_then(serde_json::Value::as_u64)
        .and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| std::format!("evidence field {name} must be a u32"))
}

fn proof_from_json(value: &serde_json::Value) -> Result<Proof, String> {
    let kind = match text_field(value, "kind")?.as_str() {
        "pure" => "pure",
        "contract" => "contract",
        "named-contract" => "named-contract",
        other => return Err(std::format!("unknown evidence proof kind {other}")),
    };
    let statement_digest = match value.get("statement_digest") {
        Some(serde_json::Value::String(digest)) => Some(digest.clone()),
        None | Some(serde_json::Value::Null) => None,
        Some(_) => return Err("evidence statement_digest must be text or null".into()),
    };
    let subject = match value.get("subject") {
        Some(serde_json::Value::Object(_)) => {
            let subject = value.get("subject").ok_or("evidence subject missing")?;
            Some(Subject {
                module: text_field(subject, "module")?,
                version: number_field(subject, "version")?,
                definition: text_field(subject, "definition")?,
                module_source_sha256: text_field(subject, "module_source_sha256")?,
                definition_source_sha256: text_field(subject, "definition_source_sha256")?,
                generated_statement_sha256: match subject.get("generated_statement_sha256") {
                    Some(serde_json::Value::String(digest)) => Some(digest.clone()),
                    None | Some(serde_json::Value::Null) => None,
                    Some(_) => {
                        return Err(
                            "evidence generated_statement_sha256 must be text or null".into()
                        )
                    }
                },
            })
        }
        None | Some(serde_json::Value::Null) => None,
        Some(_) => return Err("evidence subject must be an object or null".into()),
    };
    Ok(Proof {
        name: text_field(value, "name")?,
        kind,
        statement_digest,
        statement_basis: "recorded",
        model_revision: text_field(value, "model_revision")?,
        checker_revision: text_field(value, "checker_revision")?,
        subject,
    })
}

fn dependency_from_json(value: &serde_json::Value, depth: usize) -> Result<Dependency, String> {
    if depth > 64 {
        return Err("evidence dependency closure exceeds depth 64".into());
    }
    let children = match value.get("dependencies") {
        Some(serde_json::Value::Array(items)) => items,
        None => return Err("evidence dependency lacks its dependencies array".into()),
        Some(_) => return Err("evidence dependency dependencies must be an array".into()),
    };
    let mut dependencies = std::vec::Vec::with_capacity(children.len());
    for child in children {
        dependencies.push(dependency_from_json(child, depth.saturating_add(1))?);
    }
    Ok(Dependency {
        module: text_field(value, "module")?,
        version: number_field(value, "version")?,
        name: text_field(value, "name")?,
        declaration: text_field(value, "declaration")?,
        source_sha256: text_field(value, "source_sha256")?,
        model_revision: text_field(value, "model_revision")?,
        checker_revision: text_field(value, "checker_revision")?,
        dependencies,
    })
}

fn record_from_json(
    value: &serde_json::Value,
    source: std::vec::Vec<u8>,
) -> Result<Record, String> {
    let schema = text_field(value, "schema")?;
    if schema != SCHEMA {
        return Err(std::format!("unsupported evidence schema {schema}"));
    }
    if text_field(value, "outcome")?.as_str() != "proved" {
        return Err("evidence record is not an accepted proof".into());
    }
    if value
        .get("independent_recheck")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err("evidence record lacks the independent recheck".into());
    }
    let source_sha256 = text_field(value, "source_sha256")?;
    if intrinsic::sha256(&source) != source_sha256 {
        return Err("evidence source.noble does not match its recorded digest".into());
    }
    let module = value.get("module").ok_or("evidence module missing")?;
    let revisions = value.get("revisions").ok_or("evidence revisions missing")?;
    let named = |name: &'static str| match revisions.get(name) {
        Some(serde_json::Value::String(revision)) => Ok(Some(revision.clone())),
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(_) => Err(std::format!(
            "evidence revisions.{name} must be text or null"
        )),
    };
    let dependencies = match value.get("dependencies") {
        Some(serde_json::Value::Array(items)) => {
            let mut rows = std::vec::Vec::with_capacity(items.len());
            for item in items {
                rows.push(dependency_from_json(item, 0)?);
            }
            rows
        }
        None | Some(serde_json::Value::Null) => std::vec::Vec::new(),
        Some(_) => return Err("evidence dependencies must be an array".into()),
    };
    let proofs = match value.get("proofs") {
        Some(serde_json::Value::Array(items)) if !items.is_empty() => {
            let mut rows = std::vec::Vec::with_capacity(items.len());
            for item in items {
                rows.push(proof_from_json(item)?);
            }
            rows
        }
        _ => return Err("evidence proofs must be a nonempty array".into()),
    };
    Ok(Record {
        source,
        source_sha256,
        module: text_field(module, "name")?,
        version: number_field(module, "version")?,
        revisions: Revisions {
            model: text_field(revisions, "model")?,
            checker: text_field(revisions, "checker")?,
            named_model: named("named_model")?,
            named_checker: named("named_checker")?,
        },
        dependencies,
        proofs,
    })
}

fn read_bounded(
    path: &std::path::Path,
    limit: usize,
) -> Result<std::vec::Vec<u8>, output::Failure> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| failure("evidence-io", std::format!("{}: {error}", path.display())))?;
    if !metadata.is_file() {
        return Err(failure(
            "evidence-io",
            std::format!("{} is not a regular file", path.display()),
        ));
    }
    let limit_u64 = u64::try_from(limit)
        .map_err(|error| failure("evidence-io", std::format!("{}: {error}", path.display())))?;
    if metadata.len() > limit_u64 {
        return Err(failure(
            "evidence-limit",
            std::format!("{} exceeds its byte limit", path.display()),
        ));
    }
    let file = std::fs::File::open(path)
        .map_err(|error| failure("evidence-io", std::format!("{}: {error}", path.display())))?;
    let mut reader = std::io::Read::take(file, limit_u64.saturating_add(1));
    let mut bytes = std::vec::Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut bytes)
        .map_err(|error| failure("evidence-io", std::format!("{}: {error}", path.display())))?;
    if bytes.len() > limit {
        return Err(failure(
            "evidence-limit",
            std::format!("{} exceeds its byte limit", path.display()),
        ));
    }
    Ok(bytes)
}

/// Load one retained evidence directory without modifying it.
pub(crate) fn load(directory: &std::path::Path) -> Result<Loaded, output::Failure> {
    let evidence_path = directory.join("evidence.json");
    let evidence = read_bounded(&evidence_path, EVIDENCE_BYTES)?;
    let source = read_bounded(&directory.join("source.noble"), super::SOURCE_LIMIT)?;
    let value: serde_json::Value = serde_json::from_slice(&evidence).map_err(|error| {
        failure(
            "evidence-invalid",
            std::format!("{}: {error}", evidence_path.display()),
        )
    })?;
    let record = record_from_json(&value, source).map_err(|message| {
        failure(
            "evidence-invalid",
            std::format!("{}: {message}", evidence_path.display()),
        )
    })?;
    Ok(Loaded {
        directory: directory.to_string_lossy().into_owned(),
        evidence_sha256: intrinsic::sha256(&evidence),
        record,
    })
}

/// Build the current prepared subject's staleness key.
pub(crate) fn current(
    batch: &ProofBatch,
    source_sha256: &str,
    revisions: &Revisions,
    limits: Limits,
) -> Result<Current, output::Failure> {
    let named = batch
        .obligations
        .iter()
        .any(|obligation| match &obligation.goal {
            PendingGoal::Contract { contract } => contract.revision == 2,
            PendingGoal::Pure { .. } => false,
        });
    if named && (revisions.named_model.is_none() || revisions.named_checker.is_none()) {
        return Err(failure(
            "proof-artifact",
            "named contract revisions are missing".into(),
        ));
    }
    let mut proofs = std::vec::Vec::with_capacity(batch.obligations.len());
    for obligation in &batch.obligations {
        let (kind, model, checker) = match &obligation.goal {
            PendingGoal::Pure { .. } => {
                ("pure", revisions.model.clone(), revisions.checker.clone())
            }
            PendingGoal::Contract { contract } if contract.revision == 2 => (
                "named-contract",
                revisions.named_model.clone().unwrap_or_default(),
                revisions.named_checker.clone().unwrap_or_default(),
            ),
            PendingGoal::Contract { .. } => (
                "contract",
                revisions.model.clone(),
                revisions.checker.clone(),
            ),
        };
        let digest = statement_digest(&obligation.goal, limits)?;
        let basis = if digest.is_some() {
            "generated-statement"
        } else {
            "source-and-checker"
        };
        let subject = match &obligation.goal {
            PendingGoal::Contract { contract } => {
                Some(subject_row(&contract.subject, digest.clone()))
            }
            PendingGoal::Pure { .. } => None,
        };
        proofs.push(Proof {
            name: obligation.name.clone(),
            kind,
            statement_digest: digest,
            statement_basis: basis,
            model_revision: model,
            checker_revision: checker,
            subject,
        });
    }
    Ok(Current {
        source_sha256: std::string::String::from(source_sha256),
        module: batch.module.clone(),
        version: batch.version,
        dependencies: batch.dependencies.iter().map(dependency_record).collect(),
        proofs,
    })
}

fn compare_dependencies(
    recorded: &[Dependency],
    current: &[Dependency],
    path: &str,
    mut mismatches: std::vec::Vec<std::string::String>,
) -> std::vec::Vec<std::string::String> {
    if recorded.len() != current.len() {
        mismatches.push(std::format!("{path}.length"));
        return mismatches;
    }
    for (index, (left, right)) in recorded.iter().zip(current).enumerate() {
        let at = std::format!("{path}[{index}]");
        if left.module != right.module {
            mismatches.push(std::format!("{at}.module"));
        }
        if left.version != right.version {
            mismatches.push(std::format!("{at}.version"));
        }
        if left.name != right.name {
            mismatches.push(std::format!("{at}.name"));
        }
        if left.declaration != right.declaration {
            mismatches.push(std::format!("{at}.declaration"));
        }
        if left.source_sha256 != right.source_sha256 {
            mismatches.push(std::format!("{at}.source_sha256"));
        }
        if left.model_revision != right.model_revision {
            mismatches.push(std::format!("{at}.model_revision"));
        }
        if left.checker_revision != right.checker_revision {
            mismatches.push(std::format!("{at}.checker_revision"));
        }
        mismatches = compare_dependencies(
            &left.dependencies,
            &right.dependencies,
            &std::format!("{at}.dependencies"),
            mismatches,
        );
    }
    mismatches
}

fn compare_subject(
    recorded: &Subject,
    current: &Subject,
    path: &str,
    mut mismatches: std::vec::Vec<std::string::String>,
) -> std::vec::Vec<std::string::String> {
    if recorded.module != current.module || recorded.version != current.version {
        mismatches.push(std::format!("{path}.subject"));
    }
    if recorded.definition != current.definition {
        mismatches.push(std::format!("{path}.definition"));
    }
    if recorded.module_source_sha256 != current.module_source_sha256 {
        mismatches.push(std::format!("{path}.subject_module_source_sha256"));
    }
    if recorded.definition_source_sha256 != current.definition_source_sha256 {
        mismatches.push(std::format!("{path}.subject_definition_source_sha256"));
    }
    if recorded.generated_statement_sha256 != current.generated_statement_sha256 {
        mismatches.push(std::format!("{path}.subject_generated_statement_sha256"));
    }
    mismatches
}

/// Every key whose recorded value differs from the current prepared source.
/// The list is ordered: module keys, then dependencies, then proofs.
pub(crate) fn compare(recorded: &Record, current: &Current) -> std::vec::Vec<std::string::String> {
    let mut mismatches = std::vec::Vec::new();
    if recorded.source_sha256 != current.source_sha256 {
        mismatches.push(std::string::String::from("source_sha256"));
    }
    if recorded.module != current.module {
        mismatches.push(std::string::String::from("module.name"));
    }
    if recorded.version != current.version {
        mismatches.push(std::string::String::from("module.version"));
    }
    mismatches = compare_dependencies(
        &recorded.dependencies,
        &current.dependencies,
        "dependencies",
        mismatches,
    );
    for proof in &current.proofs {
        let Some(recorded) = recorded.proofs.iter().find(|row| row.name == proof.name) else {
            mismatches.push(std::format!("proofs.{}.missing", proof.name));
            continue;
        };
        let path = std::format!("proofs.{}", proof.name);
        if recorded.kind != proof.kind {
            mismatches.push(std::format!("{path}.kind"));
        }
        if recorded.model_revision != proof.model_revision {
            mismatches.push(std::format!("{path}.model_revision"));
        }
        if recorded.checker_revision != proof.checker_revision {
            mismatches.push(std::format!("{path}.checker_revision"));
        }
        if recorded.statement_digest != proof.statement_digest {
            mismatches.push(std::format!("{path}.statement_digest"));
        }
        match (&recorded.subject, &proof.subject) {
            (Some(left), Some(right)) => {
                mismatches = compare_subject(left, right, &path, mismatches);
            }
            (None, None) => {}
            _ => mismatches.push(std::format!("{path}.subject")),
        }
    }
    for proof in &recorded.proofs {
        if !current.proofs.iter().any(|row| row.name == proof.name) {
            mismatches.push(std::format!("proofs.{}.absent", proof.name));
        }
    }
    mismatches
}

/// The VC-TOOL-02 refusal for evidence that no longer matches the subject.
pub(crate) fn stale_failure(
    loaded: &Loaded,
    mismatches: &[std::string::String],
) -> output::Failure {
    let mut failure = output::Failure::unsupported(
        "intrinsic-stale-proof",
        std::format!(
            "saved evidence in {} no longer matches the current subject: {}",
            loaded.directory,
            mismatches.join(", ")
        ),
    );
    failure.details = encoding::object([
        ("evidence", encoding::string(&loaded.directory)),
        ("evidence_sha256", encoding::string(&loaded.evidence_sha256)),
        (
            "mismatches",
            encoding::strings(
                &mismatches
                    .iter()
                    .map(std::string::String::as_str)
                    .collect::<std::vec::Vec<_>>(),
            ),
        ),
    ]);
    failure
}

/// One retained record as the goal view lists it: the directory, its digest,
/// the recorded module and the key comparison, never a rewritten record.
pub(crate) fn summary(loaded: &Loaded, mismatches: &[std::string::String]) -> encoding::Json {
    encoding::object([
        ("path", encoding::string(&loaded.directory)),
        ("sha256", encoding::string(&loaded.evidence_sha256)),
        ("schema", encoding::string(SCHEMA)),
        (
            "source_sha256",
            encoding::string(&loaded.record.source_sha256),
        ),
        ("module", encoding::string(&loaded.record.module)),
        (
            "version",
            encoding::Json::Number(u64::from(loaded.record.version)),
        ),
        (
            "proofs",
            encoding::Json::Number(loaded.record.proofs.len() as u64),
        ),
        ("matches", encoding::Json::Bool(mismatches.is_empty())),
        (
            "mismatches",
            encoding::strings(
                &mismatches
                    .iter()
                    .map(std::string::String::as_str)
                    .collect::<std::vec::Vec<_>>(),
            ),
        ),
    ])
}

/// One retained-evidence row for the goal view: the original subject and the
/// keys that differ, never a rewritten record.
pub(crate) fn retained_json(
    loaded: &Loaded,
    proof: &Proof,
    mismatches: &[std::string::String],
) -> encoding::Json {
    encoding::object([
        ("path", encoding::string(&loaded.directory)),
        ("sha256", encoding::string(&loaded.evidence_sha256)),
        (
            "recorded",
            encoding::object([
                ("module", encoding::string(&loaded.record.module)),
                (
                    "version",
                    encoding::Json::Number(u64::from(loaded.record.version)),
                ),
                (
                    "source_sha256",
                    encoding::string(&loaded.record.source_sha256),
                ),
                (
                    "subject",
                    proof
                        .subject
                        .as_ref()
                        .map_or(encoding::Json::Null, subject_json),
                ),
                (
                    "statement_digest",
                    encoding::optional_string(proof.statement_digest.as_deref()),
                ),
                ("model_revision", encoding::string(&proof.model_revision)),
                (
                    "checker_revision",
                    encoding::string(&proof.checker_revision),
                ),
            ]),
        ),
        (
            "mismatches",
            encoding::strings(
                &mismatches
                    .iter()
                    .map(std::string::String::as_str)
                    .collect::<std::vec::Vec<_>>(),
            ),
        ),
    ])
}

/// The recorded proof row for `name`, when the record holds one.
pub(crate) fn recorded_proof<'a>(loaded: &'a Loaded, name: &str) -> Option<&'a Proof> {
    loaded.record.proofs.iter().find(|proof| proof.name == name)
}
