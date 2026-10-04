//! Independently replay source-derived proof namespace transactions against
//! production ModuleSession. Strict Lean verdicts are independently produced
//! by the frozen CLI and supplied as exact source-bound host observations.
use anyhow::{bail, ensure, Context, Result};
use noble_contracts::{
    intrinsic::{self, CheckedProof, ProofBatch, ProofDependency, ProofKind},
    source::{ModuleKind, ModuleSession, Stage},
    Limits,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn source(document: &Value, key: &str) -> Result<Vec<u8>> {
    let file = document.get("sources").and_then(|item| item.get(key))
        .and_then(Value::as_str).with_context(|| format!("missing source {key}"))?;
    fs::read(file).with_context(|| format!("reading frozen {key}: {file}"))
}

fn verdict(document: &Value, key: &str) -> Result<Value> {
    let file = document.get("verdicts").and_then(|item| item.get(key))
        .and_then(Value::as_str).with_context(|| format!("missing strict verdict {key}"))?;
    serde_json::from_slice(&fs::read(file).with_context(|| format!("reading strict verdict {key}"))?)
        .with_context(|| format!("invalid JSON in strict verdict {key}"))
}

fn source_error(error: noble_contracts::source::Error) -> anyhow::Error {
    anyhow::anyhow!("production source {:?}: {}", error.stage(), error.diagnostic().message)
}

fn checked(batch: &ProofBatch, receipt: &Value) -> Result<Vec<CheckedProof>> {
    ensure!(receipt.get("schema").and_then(Value::as_str)
        == Some("noble-intrinsic-module-report/v1"), "not an intrinsic CLI report");
    ensure!(receipt.get("outcome").and_then(Value::as_str) == Some("proved")
        && receipt.get("independent_recheck").and_then(Value::as_bool) == Some(true),
        "source proof has no independent strict Lean verdict");
    ensure!(receipt.get("module").and_then(|module| module.get("name"))
        .and_then(Value::as_str) == Some(batch.module.as_str()),
        "strict verdict refers to another module");
    ensure!(receipt.get("module").and_then(|module| module.get("version"))
        .and_then(Value::as_u64) == Some(u64::from(batch.version)),
        "strict verdict refers to another module version");
    ensure!(receipt.get("source_sha256").and_then(Value::as_str)
        == Some(digest(&batch.source).as_str()), "strict verdict refers to other source bytes");
    let observed = receipt.get("proofs").and_then(Value::as_array)
        .context("strict verdict contains no checked proofs")?;
    let mut production = intrinsic::check_batch(batch, Limits::default())
        .map_err(|diagnostic| anyhow::anyhow!("source checker rejected: {:?}", diagnostic))?;
    ensure!(observed.len() == production.len(), "strict proof count differs from source checker");
    for (index, (reported, actual)) in observed.iter().zip(&mut production).enumerate() {
        ensure!(reported.get("name").and_then(Value::as_str) == Some(actual.name.as_str()),
            "proof {index}: declaration identity changed");
        ensure!(reported.get("kind").and_then(Value::as_str)
            == Some(match actual.kind {
                ProofKind::Pure => "pure",
                ProofKind::Contract => "contract",
                ProofKind::NamedContract => bail!("legacy v1 peer refuses named v2 proof"),
            }),
            "proof {index}: claim kind changed");
        ensure!(reported.get("claim").and_then(Value::as_str) == Some(actual.claim.as_str())
            && reported.get("lowered_term").and_then(Value::as_str) == Some(actual.lean_term.as_str()),
            "proof {index}: source term/claim differs from checked independent verdict");
        let model = reported.get("model_revision").and_then(Value::as_str)
            .context("strict verdict missing selected reviewed model revision")?;
        let checker = reported.get("checker_revision").and_then(Value::as_str)
            .context("strict verdict missing selected checker/consumer revision")?;
        ensure!(model.len() == 64 && model.bytes().all(|byte| byte.is_ascii_hexdigit())
            && checker.len() == 64 && checker.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "strict verdict missing exact source revision fingerprints");
        actual.model_revision = model.into();
        actual.checker_revision = checker.into();
    }
    Ok(production)
}

fn register(mut session: ModuleSession, bytes: &[u8], strict: Option<&Value>) -> Result<ModuleSession> {
    let pending = session.prepare(bytes, &[], Limits::default()).map_err(source_error)?;
    ensure!(pending.kind() == ModuleKind::Module, "not an actual source module");
    let (next, result) = match strict {
        Some(receipt) => {
            ensure!(pending.proof_obligations().is_some(), "no proof obligations for strict verdict");
            session.commit_verified(pending, |batch| checked(batch, receipt).map_err(|error| error.to_string()))
        }
        None => {
            ensure!(pending.proof_obligations().is_none(), "proof module omitted strict verdict");
            session.commit(pending)
        }
    };
    session = next;
    result.map_err(source_error)?;
    Ok(session)
}

fn import(session: ModuleSession, bytes: &[u8]) -> Result<ModuleSession> {
    let pending = session.prepare(bytes, &[], Limits::default()).map_err(source_error)?;
    ensure!(pending.kind() == ModuleKind::Import, "not an actual source import");
    let (next, result) = session.commit(pending);
    result.map_err(source_error)?;
    Ok(next)
}

fn dependency(batch: &ProofBatch, version: u32, expected: &[u8]) -> Result<Value> {
    ensure!(batch.dependencies.len() == 1, "named proof import omitted or multiplied");
    let item = &batch.dependencies[0];
    ensure!(item.module == "logic" && item.version == version,
        "alias resolved to wrong immutable proof owner/version");
    ensure!(item.name == "alias.poly-refl" && item.declaration.name == "poly-refl"
        && item.exported, "named proof did not retain exported immutable declaration");
    ensure!(item.source.as_slice() == expected,
        "named proof did not retain original owner source bytes");
    ensure!(item.dependencies.is_empty(), "unexpected transitive proof premise");
    Ok(json!({ "owner": item.module, "version": item.version,
        "name": item.name, "declaration": item.declaration.name,
        "source_sha256": digest(&item.source), "dependency_count": item.dependencies.len() }))
}

fn stale(document: &Value) -> Result<Value> {
    let first = source(document, "exported_v1")?;
    let second = source(document, "exported_v2")?;
    let import_first = source(document, "import_v1")?;
    let import_second = source(document, "import_v2")?;
    let consumer = source(document, "consumer")?;
    let first_verdict = verdict(document, "exported_v1")?;
    let second_verdict = verdict(document, "exported_v2")?;
    let consumer_verdict = verdict(document, "consumer_v2")?;
    let mut session = ModuleSession::new(&[]).map_err(source_error)?;
    session = register(session, &first, Some(&first_verdict))?;
    session = import(session, &import_first)?;
    let prepared = session.prepare(&consumer, &[], Limits::default()).map_err(source_error)?;
    let first_dependency = dependency(prepared.proof_obligations().context("no pending imported proof")?, 1, &first)?;
    session = register(session, &second, Some(&second_verdict))?;
    session = import(session, &import_second)?;
    let before = session.generation();
    let (next, result) = session.commit_verified(prepared, |_| {
        panic!("stale proof preparation reached host checker callback")
    });
    let error = match result {
        Err(error) => error,
        Ok(()) => bail!("stale imported proof was published"),
    };
    ensure!(error.stage() == Stage::Acceptance
        && error.diagnostic().message.contains("stale"),
        "old imported proof did not fail at stale namespace acceptance");
    ensure!(next.generation() == before, "stale refusal mutated live namespace");
    let fresh = next.prepare(&consumer, &[], Limits::default()).map_err(source_error)?;
    let second_dependency = dependency(fresh.proof_obligations().context("no refreshed imported proof")?, 2, &second)?;
    let (next, result) = next.commit_verified(fresh, |batch| {
        checked(batch, &consumer_verdict).map_err(|error| error.to_string())
    });
    result.map_err(source_error)?;
    ensure!(next.generation() == before + 1, "fresh exact import proof not published");
    Ok(json!({ "old_dependency": first_dependency,
        "new_dependency": second_dependency, "stale_stage": "acceptance",
        "stale_checker_callback_called": false, "generation_before": before,
        "generation_after_refusal": before, "fresh_generation": next.generation(),
        "guest_requests": 0, "protected_operations": 0 }))
}

fn private(document: &Value) -> Result<Value> {
    let source_private = source(document, "private_v1")?;
    let imported = source(document, "import_v1")?;
    let consumer = source(document, "consumer")?;
    let strict = verdict(document, "private_v1")?;
    let mut session = ModuleSession::new(&[]).map_err(source_error)?;
    session = register(session, &source_private, Some(&strict))?;
    session = import(session, &imported)?;
    let before = session.generation();
    let error = match session.prepare(&consumer, &[], Limits::default()) {
        Err(error) => error,
        Ok(_) => bail!("private imported proof unexpectedly prepared"),
    };
    ensure!(error.stage() == Stage::Resolve
        && error.diagnostic().message.contains("private"),
        "not an exact private imported proof refusal");
    ensure!(session.generation() == before, "private refusal changed namespace");
    Ok(json!({ "stage": "resolve", "outcome": "refused-private-import",
        "generation_before": before, "generation_after": session.generation(),
        "guest_requests": 0, "protected_operations": 0 }))
}

fn transitive(document: &Value) -> Result<Value> {
    let exporter = source(document, "exported_v1")?;
    let imported = source(document, "import_v1")?;
    let consumer = source(document, "consumer")?;
    let forged = source(document, "untrusted_axiom")?;
    let strict = verdict(document, "exported_v1")?;
    let mut session = ModuleSession::new(&[]).map_err(source_error)?;
    session = register(session, &exporter, Some(&strict))?;
    session = import(session, &imported)?;
    let genuine = session.prepare(&consumer, &[], Limits::default()).map_err(source_error)?;
    let valid = genuine.proof_obligations().context("no genuine imported source proof")?;
    let first = dependency(valid, 1, &exporter)?;
    let untrusted = session.prepare(&forged, &[], Limits::default()).map_err(source_error)?;
    let evil = untrusted.proof_obligations().context("forged axiom source has no proof term")?;
    ensure!(evil.obligations.len() == 1, "axiom source has wrong proof count");
    let mut mutated = valid.clone();
    let model_revision = mutated.dependencies[0].model_revision.clone();
    let checker_revision = mutated.dependencies[0].checker_revision.clone();
    mutated.dependencies[0].dependencies.push(ProofDependency {
        name: "foreign.axiom".into(),
        module: "foreign".into(),
        version: 1,
        source: forged,
        declaration: evil.obligations[0].clone(),
        exported: true,
        dependencies: Vec::new(),
        model_revision,
        checker_revision,
    });
    let refused = intrinsic::check_batch(&mutated, Limits::default());
    ensure!(refused.is_err(), "producer-forged transitive axiom entered checked proof context");
    let diagnostic = refused.err().context("missing transitive axiom diagnostic")?;
    ensure!(diagnostic.message.contains("unknown") || diagnostic.message.contains("proof"),
        "transitive control refused for unrelated reason: {}", diagnostic.message);
    Ok(json!({ "accepted_dependency": first, "forged_nested_dependency_rejected": true,
        "diagnostic": diagnostic.message, "guest_requests": 0,
        "protected_operations": 0 }))
}

fn main() -> Result<()> {
    let argument = env::args().nth(1).context("usage: noble-intrinsic-proofs-peer WORKLOAD.json")?;
    ensure!(env::args().count() == 2, "one frozen workload path required");
    let file = Path::new(&argument);
    let bytes = fs::read(file).with_context(|| format!("reading workload {}", file.display()))?;
    let document: Value = serde_json::from_slice(&bytes).context("decoding workload JSON")?;
    ensure!(document.get("schema").and_then(Value::as_str)
        == Some("noble-intrinsic-proofs-peer-workload/v1"), "wrong peer workload schema");
    let snapshot = stale(&document)?;
    let visibility = private(&document)?;
    let dependency = transitive(&document)?;
    let result = json!({ "schema": "noble-intrinsic-proofs-kernel-peer/v1",
        "result": "passed", "workload_sha256": digest(&bytes),
        "stale": snapshot, "visibility": visibility, "transitive": dependency });
    println!("{result}");
    Ok(())
}
