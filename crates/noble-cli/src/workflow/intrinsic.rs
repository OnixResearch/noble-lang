//! Host-selected source proof check; source preparation alone is never proof authority.
use noble_contracts::intrinsic::{CheckedProof, PendingGoal, ProofBatch, ProofKind};
use super::{encoding, output, verification};

// Exact transitive source closure of the generated MC1Obligation import of
// NobleContracts, and of the PureTyCode witness's Expression import. Extra
// library modules are compiled in isolation but are not imported by either
// checked theorem; they must not invalidate unrelated accepted proofs.
const MODEL: &[(&str,&str)] = &[
    ("NobleContracts",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/NobleContracts.lean"))),
    ("NobleContracts.Model",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/NobleContracts/Model.lean"))),
    ("NobleContracts.Rules",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/NobleContracts/Rules.lean"))),
    ("NobleContracts.Expression",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/NobleContracts/Expression.lean"))),
    ("NobleContracts.Obligation",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/NobleContracts/Obligation.lean"))),
    ("NobleContracts.Examples",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/NobleContracts/Examples.lean"))),
    ("NobleContracts.Composition",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/NobleContracts/Composition.lean"))),
];
// Separate named import closure; never extend MODEL or its historical digest.
// NamedV2 imports NamedSubject and Expression; MODEL pins their transitive
// Model/Rules/Std ancestry, and the generated obligation imports only NamedV2.
const NAMED_MODEL: &[(&str,&str)] = &[
    ("NobleContracts.NamedSubject",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/NobleContracts/NamedSubject.lean"))),
    ("NobleContracts.NamedV2",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/NobleContracts/NamedV2.lean"))),
];
const TYPE_WITNESS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/IntrinsicTypeWitness.lean"));
fn pinned_import(owner: &str, imported: &str) -> Result<(),output::Failure> {
    // Std comes from the selected, pinned Lean release. No other external
    // dependency or repository module can enter the intrinsic proof closure.
    if imported == "Std" || MODEL.iter().any(|(name,_)|*name == imported) {
        Ok(())
    } else {
        Err(output::Failure::unsupported("intrinsic-unpinned-import",
            std::format!("{owner} imports unreviewed module {imported}")))
    }
}
fn pinned_library(library: &super::rules::Library) -> Result<(),output::Failure> {
    for (name,source) in MODEL {
        let module = library.modules.iter().find(|module| module.name == *name && module.source == *source)
            .ok_or_else(||output::Failure::unsupported("intrinsic-model-mismatch",
                std::format!("reviewed MC1 model source {name} differs from selected library")))?;
        for imported in &module.imports { pinned_import(name,imported)?; }
    }
    for line in TYPE_WITNESS.lines() {
        let trimmed = line.trim();
        if let Some(imports) = trimmed.strip_prefix("import ")
            .or_else(||trimmed.strip_prefix("public import ")) {
            for imported in imports.split_ascii_whitespace().take_while(|name|!name.starts_with("--")) {
                pinned_import("IntrinsicTypeWitness",imported)?;
            }
        }
    }
    Ok(())
}
fn pinned_named_library(library: &super::rules::Library) -> Result<(),output::Failure> {
    for (name,source) in NAMED_MODEL {
        let module = library.modules.iter().find(|module|module.name == *name && module.source == *source)
            .ok_or_else(||output::Failure::unsupported("named-model-mismatch",
                std::format!("reviewed named model source {name} differs from selected library")))?;
        for imported in &module.imports {
            if !MODEL.iter().chain(NAMED_MODEL).any(|(reviewed,_)|reviewed == imported) {
                return Err(output::Failure::unsupported("named-unpinned-import",
                    std::format!("{name} imports unreviewed module {imported}")));
            }
        }
    }
    Ok(())
}
fn model_revision() -> std::string::String {
    let mut digest=Sha256::new();
    digest.update(b"noble-intrinsic-import-closure/v2");
    for (name,source) in MODEL {
        digest.update(&(name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update(&(source.len() as u64).to_le_bytes());
        digest.update(source.as_bytes());
    }
    digest.update(&(TYPE_WITNESS.len() as u64).to_le_bytes());
    digest.update(TYPE_WITNESS.as_bytes());
    digest.finish()
}
fn named_model_revision() -> std::string::String {
    let mut digest=Sha256::new();
    digest.update(b"noble-named-v2-import-closure/v1");
    for (name,source) in MODEL.iter().chain(NAMED_MODEL) {
        digest.update(&(name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        digest.update(&(source.len() as u64).to_le_bytes());
        digest.update(source.as_bytes());
    }
    digest.finish()
}
fn checker_revision() -> std::string::String {
    // The revision binds preparation and atomic publication, not just the
    // proof AST checker: a changed lexer, resolver, or source validity gate
    // must invalidate every previously accepted imported proof.
    macro_rules! contract_source {
        ($path:literal) => {
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../noble-contracts/src/",$path))
        };
    }
    let parts = [
        contract_source!("intrinsic.rs"),
        contract_source!("source.rs"),
        contract_source!("source/lexer.rs"),
        contract_source!("source/lexer/tokens.rs"),
        contract_source!("source/lexer/text.rs"),
        contract_source!("source/parsing.rs"),
        contract_source!("source/preparation.rs"),
        contract_source!("source/preflight.rs"),
        contract_source!("source/preflight/paths.rs"),
        contract_source!("source/resolution.rs"),
        contract_source!("source/resolution/comparison.rs"),
        contract_source!("source/declared.rs"),
        contract_source!("source/declared/types.rs"),
        contract_source!("source/declared/signatures.rs"),
        contract_source!("source/declared/links.rs"),
        contract_source!("source/declared/parsing.rs"),
        contract_source!("source/declared/parsing/body.rs"),
        contract_source!("source/declared/parsing/imports.rs"),
        contract_source!("source/declared/parsing/logic.rs"),
        contract_source!("source/declared/parsing/members.rs"),
        contract_source!("source/declared/state.rs"),
        contract_source!("source/declared/state/prepare.rs"),
        contract_source!("source/declared/state/register.rs"),
        contract_source!("source/declared/state/register/adapter.rs"),
        contract_source!("source/declared/state/register/collection.rs"),
        contract_source!("source/declared/state/register/definitions.rs"),
        contract_source!("source/declared/state/register/definitions/graph.rs"),
        contract_source!("source/declared/state/register/definitions/scheduling.rs"),
        contract_source!("source/declared/state/register/exports.rs"),
        contract_source!("source/declared/state/register/logical.rs"),
        contract_source!("source/declared/state/register/publication.rs"),
        contract_source!("source/declared/state/register/schema.rs"),
        contract_source!("source/declared/state/register/schema/names.rs"),
        contract_source!("source/declared/state/register/schema/nominals.rs"),
        contract_source!("source/declared/state/register/schema/types.rs"),
        contract_source!("source/declared/state/namespace.rs"),
        contract_source!("source/declared/state/namespace/words.rs"),
        include_str!("../core/declared/mod.rs"),
        include_str!("../core/entry/compilation.rs"),
        TYPE_WITNESS,
        include_str!("intrinsic.rs"),
        include_str!("verification.rs"),
        include_str!("../consumer/decoding.lean"),
        include_str!("../consumer/replay.lean"),
    ];
    let mut digest=Sha256::new();
    for part in parts { digest.update(part.as_bytes()); }
    digest.finish()
}
fn named_checker_revision(base: &str) -> std::string::String {
    // A v2 accepted dependency must become stale if either the independent
    // original-source binder, the named graph/lowering checker, or the fresh
    // kernel consumer changes. Keep these additive files out of v1's source
    // inventory so v1 cannot depend on a different consumer wire mode.
    let mut digest=Sha256::new();
    digest.update(b"noble-named-v2-checker/v1");
    digest.update(base.as_bytes());
    digest.update(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../noble-contracts/src/intrinsic/named_v2.rs")).as_bytes());
    digest.update(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/../noble-contracts/src/source/declared/state/register/logical/origin.rs")).as_bytes());
    digest.update(include_str!("verification/named_v2.rs").as_bytes());
    digest.update(include_str!("verification/compilation.rs").as_bytes());
    digest.update(include_str!("rules.rs").as_bytes());
    digest.update(include_str!("artifacts.rs").as_bytes());
    digest.update(include_str!("../compiler.lean").as_bytes());
    digest.update(include_str!("../sandbox.rs").as_bytes());
    digest.update(include_str!("../sandbox/launch.rs").as_bytes());
    digest.update(include_str!("../sandbox/process.rs").as_bytes());
    digest.update(include_str!("../sandbox/runtime.rs").as_bytes());
    digest.finish()
}
fn has_named_dependency(dependency:&noble_contracts::intrinsic::ProofDependency)->bool {
    dependency.declaration.revision == 2 ||
        dependency.dependencies.iter().any(has_named_dependency)
}
fn validate_dependency_revision(
    dependency: &noble_contracts::intrinsic::ProofDependency,
    model: &str,
    named_model: &str,
    checker: &str,
    named_checker: &str,
) -> Result<(),output::Failure> {
    let (selected_model,selected_checker) = match (&dependency.declaration.goal,dependency.declaration.revision) {
        (PendingGoal::Contract {contract},2) if contract.revision == 2 => (named_model,named_checker),
        (PendingGoal::Contract {contract},1) if contract.revision == 1 => (model,checker),
        (PendingGoal::Pure {..},1) => (model,checker),
        _ => return Err(output::Failure::unsupported("intrinsic-stale-proof",
            std::format!("proof {} has unsupported revision",dependency.name))),
    };
    if dependency.model_revision!=selected_model || dependency.checker_revision!=selected_checker {
        return Err(output::Failure::unsupported("intrinsic-stale-proof",
            std::format!("proof {} was accepted under different model/checker sources",dependency.name)));
    }
    for prerequisite in &dependency.dependencies {
        validate_dependency_revision(prerequisite,model,named_model,checker,named_checker)?;
    }
    Ok(())
}
const ROUND: [u32;64] = [
    0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
    0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
    0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
    0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
    0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
    0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
    0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
    0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2,
];
fn digest_block(state: &mut [u32;8],block: &[u8]) {
    let mut words = [0u32;64];
    for (i,word) in words.iter_mut().take(16).enumerate() {
        let offset = i*4;
        *word = u32::from_be_bytes([block[offset],block[offset+1],block[offset+2],block[offset+3]]);
    }
    for i in 16..64 {
        let x=words[i-15]; let y=words[i-2];
        let s0=x.rotate_right(7)^x.rotate_right(18)^(x>>3);
        let s1=y.rotate_right(17)^y.rotate_right(19)^(y>>10);
        words[i]=words[i-16].wrapping_add(s0).wrapping_add(words[i-7]).wrapping_add(s1);
    }
    let mut a=state[0];let mut b=state[1];let mut c=state[2];let mut d=state[3];
    let mut e=state[4];let mut f=state[5];let mut g=state[6];let mut h=state[7];
    for i in 0..64 {
        let s1=e.rotate_right(6)^e.rotate_right(11)^e.rotate_right(25);
        let ch=(e&f)^(!e&g);
        let t1=h.wrapping_add(s1).wrapping_add(ch).wrapping_add(ROUND[i]).wrapping_add(words[i]);
        let s0=a.rotate_right(2)^a.rotate_right(13)^a.rotate_right(22);
        let maj=(a&b)^(a&c)^(b&c);
        let t2=s0.wrapping_add(maj);
        h=g;g=f;f=e;e=d.wrapping_add(t1);d=c;c=b;b=a;a=t1.wrapping_add(t2);
    }
    for (value,word) in state.iter_mut().zip([a,b,c,d,e,f,g,h]) { *value=value.wrapping_add(word); }
}
struct Sha256 { state:[u32;8], buffer:[u8;64], buffered:usize, length:u64 }
impl Sha256 {
    const fn new() -> Self {
        Self {state:[0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19],
            buffer:[0;64],buffered:0,length:0}
    }
    fn update(&mut self, mut bytes:&[u8]) {
        self.length=self.length.wrapping_add(bytes.len() as u64);
        if self.buffered>0 {
            let count=(64-self.buffered).min(bytes.len());
            self.buffer[self.buffered..self.buffered+count].copy_from_slice(&bytes[..count]);
            self.buffered+=count;
            bytes=&bytes[count..];
            if self.buffered==64 {digest_block(&mut self.state,&self.buffer);self.buffered=0;}
            else {return;}
        }
        let mut chunks=bytes.chunks_exact(64);
        for chunk in &mut chunks {digest_block(&mut self.state,chunk);}
        let remaining=chunks.remainder();
        self.buffer[..remaining.len()].copy_from_slice(remaining);
        self.buffered=remaining.len();
    }
    fn finish(mut self) -> std::string::String {
        let mut suffix=[0u8;128];
        suffix[..self.buffered].copy_from_slice(&self.buffer[..self.buffered]);
        suffix[self.buffered]=0x80;
        let blocks=if self.buffered<56 {1} else {2};
        suffix[blocks*64-8..blocks*64].copy_from_slice(&self.length.wrapping_mul(8).to_be_bytes());
        for block in suffix[..blocks*64].chunks_exact(64) {digest_block(&mut self.state,block);}
        let mut encoded=std::string::String::with_capacity(64);
        for word in self.state {encoded.push_str(&std::format!("{word:08x}"));}
        encoded
    }
}
pub(crate) fn sha256(bytes: &[u8]) -> std::string::String {
    let mut digest=Sha256::new();
    digest.update(bytes);
    digest.finish()
}
pub(crate) struct IntrinsicEvidence {
    acceptance: verification::Acceptance,
    subject: encoding::Json,
}
fn dependency_evidence(dependency: &noble_contracts::intrinsic::ProofDependency) -> encoding::Json {
    encoding::object([
        ("name",encoding::string(&dependency.name)),
        ("module",encoding::string(&dependency.module)),
        ("version",encoding::Json::Number(u64::from(dependency.version))),
        ("source_sha256",encoding::string(sha256(&dependency.source))),
        ("declaration",encoding::string(&dependency.declaration.name)),
        ("model_revision",encoding::string(&dependency.model_revision)),
        ("checker_revision",encoding::string(&dependency.checker_revision)),
    ])
}
pub(crate) fn session_proof_report(batch:&ProofBatch,checked:&[CheckedProof],evidence:&[IntrinsicEvidence]) -> encoding::Json {
    encoding::object([
        ("outcome",encoding::string("proved")),
        ("source_sha256",encoding::string(sha256(&batch.source))),
        ("model_revision",checked.first().map_or(encoding::Json::Null,|proof|encoding::string(&proof.model_revision))),
        ("checker_revision",checked.first().map_or(encoding::Json::Null,|proof|encoding::string(&proof.checker_revision))),
        ("dependencies",encoding::Json::Array(batch.dependencies.iter().map(dependency_evidence).collect())),
        ("proofs",encoding::Json::Array(batch.obligations.iter().zip(checked).zip(evidence).map(|((goal,proof),proof_evidence)| {
            let mut fields=std::vec::Vec::from([
                ("name",encoding::string(&goal.name)),
                ("kind",encoding::string(match proof.kind {ProofKind::Pure=>"pure",ProofKind::Contract=>"contract",ProofKind::NamedContract=>"named-contract"})),
                ("claim",encoding::string(&proof.claim)),
                ("strict_lean_axioms",encoding::Json::Array(proof_evidence.acceptance.axioms.iter().map(encoding::string).collect())),
                ("strict_lean_tools",proof_evidence.acceptance.tools.clone()),
                ("subject",proof_evidence.subject.clone()),
            ]);
            if proof.kind == ProofKind::NamedContract {
                fields.push(("wire_mode",encoding::string("named-v2-strict-kernel")));
                fields.push(("model_revision",encoding::string(&proof.model_revision)));
                fields.push(("checker_revision",encoding::string(&proof.checker_revision)));
            }
            encoding::Json::Object(fields)
        }).collect())),
    ])
}

/// Returns the exact checked AST lowering only after each proof has survived
/// the separate, pinned, resource-bounded Lean consumer. A failed item never
/// returns a partially approved batch to the caller's atomic commit.
pub(crate) fn verify_batch(batch: &ProofBatch, source: &[u8], limits: noble_contracts::Limits, timeout_ms: u64)
    -> Result<std::vec::Vec<CheckedProof>,output::Failure> {
    verify_with_evidence(batch,source,limits,timeout_ms,
        noble_contracts::intrinsic::ProofBudgets::from_limits(limits)).map(|(checked,_,_)| checked)
}
pub(crate) fn verify_with_evidence(
    batch: &ProofBatch,
    source: &[u8],
    limits: noble_contracts::Limits,
    timeout_ms: u64,
    budgets: noble_contracts::intrinsic::ProofBudgets,
) -> Result<(std::vec::Vec<CheckedProof>,std::vec::Vec<IntrinsicEvidence>,noble_contracts::intrinsic::ProofWork),output::Failure> {
    if batch.source != source {
        return Err(output::Failure::error("intrinsic-source-binding","proof source changed between preparation and verification".into()));
    }
    if timeout_ms == 0 || timeout_ms > super::MAX_TIMEOUT {
        return Err(output::Failure::error("intrinsic-timeout-limit","invalid proof deadline".into()));
    }
    let (mut checked,work) = noble_contracts::intrinsic::check_batch_with_budgets(batch,limits,budgets).map_err(|diagnostic| {
        let code = match diagnostic.message.as_str() {
            "normalization work exhausted" => "normalization-work-exhausted",
            "substitution work exhausted" => "substitution-work-exhausted",
            _ => "intrinsic-source-invalid",
        };
        output::Failure::error(code,std::format!("{:?} at {}..{}: {}",diagnostic.kind,diagnostic.span.start,diagnostic.span.end,diagnostic.message))
    })?;
    let library = super::rules::read_library()?;
    pinned_library(&library)?;
    let model = model_revision();
    let checker = checker_revision();
    let named = checked.iter().any(|proof|proof.kind == ProofKind::NamedContract)
        || batch.dependencies.iter().any(has_named_dependency);
    let named_model = if named {named_model_revision()} else {std::string::String::new()};
    let named_checker = if named {named_checker_revision(&checker)} else {std::string::String::new()};
    for dependency in &batch.dependencies {
        validate_dependency_revision(dependency,&model,&named_model,&checker,&named_checker)?;
    }
    let mut evidence = std::vec::Vec::with_capacity(checked.len());
    for (obligation,proof) in batch.obligations.iter().zip(&checked) {
        let (acceptance,subject) = match (&obligation.goal,proof.kind) {
            (PendingGoal::Pure {..},ProofKind::Pure) => {
                (verification::verify_intrinsic_pure(&proof.claim,&proof.lean_term,&library,timeout_ms)?,encoding::Json::Null)
            }
            (PendingGoal::Contract {contract},ProofKind::Contract) => {
                if contract.revision != 1 || obligation.revision != 1 {
                    return Err(output::Failure::error("intrinsic-kind-mismatch","v1 contract revision mismatch".into()));
                }
                let typed = noble_contracts::intrinsic::prepare_contract(contract,limits).map_err(|problem|
                    output::Failure::error("intrinsic-contract-invalid",problem.message))?;
                let claim = noble_contracts::export_lean(&typed);
                if proof.claim != "MC1Obligation.claim" {
                    return Err(output::Failure::error("intrinsic-lowering-mismatch","unsupported contract rule lowering".into()));
                }
                let lean_proof = std::format!("import MC1Obligation\nimport NobleContracts.Obligation\nimport NobleContracts.Expression\nopen NobleContracts\nnamespace MC1Proof\ntheorem proof : MC1Obligation.claim := {}\nend MC1Proof\n",proof.lean_term);
                let acceptance=verification::verify(&claim,&lean_proof,&library,false,timeout_ms)?;
                (acceptance,contract_subject_evidence(contract,&typed,&claim))
            }
            (PendingGoal::Contract {contract},ProofKind::NamedContract) => {
                if contract.revision != 2 || obligation.revision != 2
                    || proof.claim != "NamedV2Obligation.claim" {
                    return Err(output::Failure::error("intrinsic-kind-mismatch","named v2 contract rule or claim mismatch".into()));
                }
                pinned_named_library(&library)?;
                let obligation_source = noble_contracts::intrinsic::prepare_named_contract(contract,limits)
                    .map_err(|problem|output::Failure::error("intrinsic-contract-invalid",problem.message))?;
                let acceptance=verification::named_v2::verify(
                    &obligation_source,&proof.lean_term,&library,timeout_ms)?;
                (acceptance,named_subject_evidence(contract,&obligation_source)?)
            }
            _ => return Err(output::Failure::error("intrinsic-kind-mismatch","typed proof and staged goal disagree".into())),
        };
        evidence.push(IntrinsicEvidence {acceptance,subject});
    }
    for proof in &mut checked {
        proof.model_revision.clone_from(if proof.kind == ProofKind::NamedContract {&named_model} else {&model});
        proof.checker_revision.clone_from(if proof.kind == ProofKind::NamedContract {&named_checker} else {&checker});
    }
    Ok((checked,evidence,work))
}

fn source_error(error: &noble_contracts::source::Error) -> output::Failure {
    let code = match error.diagnostic().kind {
        noble_contracts::DiagnosticKind::Exhausted => "intrinsic-source-exhausted",
        noble_contracts::DiagnosticKind::Unsupported => "intrinsic-source-unsupported",
        noble_contracts::DiagnosticKind::Internal => "intrinsic-source-internal",
        noble_contracts::DiagnosticKind::Invalid => match error.stage() {
            noble_contracts::source::Stage::Parse => "intrinsic-parse-invalid",
            noble_contracts::source::Stage::Resolve => "intrinsic-resolve-invalid",
            noble_contracts::source::Stage::Link => "intrinsic-link-invalid",
            noble_contracts::source::Stage::Check => "intrinsic-contract-invalid",
            noble_contracts::source::Stage::Acceptance => "intrinsic-proof-refused",
        },
    };
    let message = std::format!("{:?}: {}",error.stage(),error.diagnostic().message);
    match error.diagnostic().kind {
        noble_contracts::DiagnosticKind::Unsupported => output::Failure::unsupported(code,message),
        _ => output::Failure::error(code,message),
    }
}

fn accepted_report(
    batch: &ProofBatch,
    proofs: &[CheckedProof],
    evidence: &[IntrinsicEvidence],
    work: noble_contracts::intrinsic::ProofWork,
    source_path: &str,
    submitted: &str,
) -> encoding::Json {
    let named = proofs.iter().any(|proof|proof.kind == ProofKind::NamedContract);
    encoding::object([
            ("schema",encoding::string(if named {"noble-intrinsic-module-report/v2"} else {"noble-intrinsic-module-report/v1"})),
            ("profile",encoding::string(if named {"Named-Contracts-v2"} else {"Intrinsic-Proofs-Draft"})),
            ("command",encoding::string("verify-module")),
            ("outcome",encoding::string("proved")),
            ("stage",encoding::string("independent-recheck")),
            ("source_path",encoding::string(source_path)),
            ("source_sha256",encoding::string(submitted)),
            ("guest_requests",encoding::Json::Number(0)),
            ("host_requests",encoding::Json::Number(0)),
            ("protected_operations",encoding::Json::Number(0)),
            ("candidate_executions",encoding::Json::Number(0)),
            ("runtime_prover_calls",encoding::Json::Number(0)),
            ("proof_grants_host_authority",encoding::Json::Bool(false)),
            ("independent_recheck",encoding::Json::Bool(true)),
            ("work",encoding::object([
                ("normalization",encoding::Json::Number(u64::from(work.normalization))),
                ("substitution",encoding::Json::Number(u64::from(work.substitution))),
                ("total",encoding::Json::Number(u64::from(work.total))),
            ])),
            ("module",encoding::object([
                ("name",encoding::string(&batch.module)),
                ("version",encoding::Json::Number(u64::from(batch.version))),
                ("generation",encoding::Json::Number(batch.generation)),
                ("source_sha256",encoding::string(submitted)),
            ])),
            ("dependencies",encoding::Json::Array(batch.dependencies.iter().map(dependency_evidence).collect())),
            ("proofs",encoding::Json::Array(proofs.iter().zip(evidence).map(|(proof,acceptance)|
                encoding::object([
                    ("name",encoding::string(&proof.name)),
                    ("kind",encoding::string(match proof.kind { ProofKind::Pure => "pure",ProofKind::Contract => "contract",ProofKind::NamedContract => "named-contract" })),
                    ("claim",encoding::string(&proof.claim)),
                    ("lowered_term",encoding::string(&proof.lean_term)),
                    ("model_revision",encoding::string(&proof.model_revision)),
                    ("checker_revision",encoding::string(&proof.checker_revision)),
                    ("strict_lean_axioms",encoding::Json::Array(acceptance.acceptance.axioms.iter().map(encoding::string).collect())),
                    ("strict_lean_tools",acceptance.acceptance.tools.clone()),
                    ("consumer_wire_sha256",encoding::string(sha256(acceptance.acceptance.wire.as_bytes()))),
                    ("subject",acceptance.subject.clone()),
                ])).collect())),
    ])
}

/// Standalone, opt-in module source proof check, never a release decision.
pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let source_path = arguments.get(1).map_or_else(std::string::String::new,
        |path|path.to_string_lossy().into_owned());
    let mut submitted_sha256 = None;
    let result = execute(arguments,&mut submitted_sha256);
    let exit = match &result {
        Ok(_) => std::process::ExitCode::SUCCESS,
        Err(error) => std::process::ExitCode::from(error.outcome.exit()),
    };
    let report = match result {
        Ok(Some(report)) => report,
        Ok(None) => encoding::object([
            ("schema",encoding::string("noble-intrinsic-module-report/v1")),
            ("profile",encoding::string("Intrinsic-Proofs-Draft")),
            ("command",encoding::string("verify-module")),
            ("outcome",encoding::string("unproved")),
            ("stage",encoding::string("source-contract")),
            ("source_path",encoding::string(&source_path)),
            ("source_sha256",submitted_sha256.as_deref().map_or(encoding::Json::Null,encoding::string)),
            ("guest_requests",encoding::Json::Number(0)),
            ("host_requests",encoding::Json::Number(0)),
            ("protected_operations",encoding::Json::Number(0)),
            ("candidate_executions",encoding::Json::Number(0)),
            ("runtime_prover_calls",encoding::Json::Number(0)),
            ("proof_grants_host_authority",encoding::Json::Bool(false)),
            ("independent_recheck",encoding::Json::Bool(false)),
        ]),
        Err(error) => encoding::object([
            ("schema",encoding::string("noble-intrinsic-module-report/v1")),
            ("profile",encoding::string("Intrinsic-Proofs-Draft")),
            ("command",encoding::string("verify-module")),
            ("outcome",encoding::string(error.outcome.name())),
            ("stage",encoding::string(match error.code {
                "intrinsic-parse-invalid" => "parse",
                "intrinsic-resolve-invalid" => "resolve",
                "intrinsic-link-invalid" => "link",
                "intrinsic-contract-invalid" => "check",
                "intrinsic-source-exhausted" => "source-exhausted",
                "intrinsic-proof-refused" => "acceptance",
                "intrinsic-source-invalid" | "normalization-work-exhausted" | "substitution-work-exhausted" => "proof-check",
                _ => "independent-recheck",
            })),
            ("source_path",encoding::string(&source_path)),
            ("source_sha256",submitted_sha256.as_deref().map_or(encoding::Json::Null,encoding::string)),
            ("guest_requests",encoding::Json::Number(0)),
            ("host_requests",encoding::Json::Number(0)),
            ("protected_operations",encoding::Json::Number(0)),
            ("candidate_executions",encoding::Json::Number(0)),
            ("runtime_prover_calls",encoding::Json::Number(0)),
            ("proof_grants_host_authority",encoding::Json::Bool(false)),
            ("independent_recheck",encoding::Json::Bool(false)),
            ("diagnostic",error.json()),
        ]),
    };
    std::println!("{}",report.encode());
    exit
}
fn contract_subject_evidence(
    contract: &noble_contracts::intrinsic::ContractGoal,
    typed: &noble_contracts::Prepared,
    generated_statement: &str,
) -> encoding::Json {
    let subject=&contract.subject;
    encoding::object([
        ("owner",encoding::string(&subject.module)),
        ("version",encoding::Json::Number(u64::from(subject.version))),
        ("definition",encoding::string(&subject.definition)),
        ("definition_identity_session_local",encoding::string(subject.definition_identity.to_string())),
        ("module_source_sha256",encoding::string(sha256(&subject.module_source))),
        ("definition_source_sha256",encoding::string(sha256(&subject.definition_source))),
        ("resolved_candidate",encoding::string(std::format!("{:?}",subject.accepted_submission.body.candidate))),
        ("resolved_definitions",encoding::string(std::format!("{:?}",subject.accepted_submission.definitions))),
        ("accepted_candidate",encoding::string(std::format!("{:?}",typed.candidate()))),
        ("generated_statement",encoding::string(generated_statement)),
    ])
}
fn named_subject_evidence(
    contract: &noble_contracts::intrinsic::ContractGoal,
    obligation_source: &str,
) -> Result<encoding::Json,output::Failure> {
    use noble_kernel::untrusted::{Node,Outcome};
    let subject=&contract.subject;
    let module_digests:std::vec::Vec<_>=subject.source_dependencies.iter()
        .map(|module|sha256(&module.full_source)).collect();
    let subject_digest=sha256(&subject.module_source);
    let definition_digest=sha256(&subject.definition_source);
    let accepted=&subject.accepted_submission;
    let root=&subject.named_uses[0];
    let selected=accepted.definitions.iter().find(|row|row.definition==root.definition)
        .ok_or_else(||output::Failure::error("named-report-graph","accepted subject row missing".into()))?;
    let derived=noble_kernel::acceptance::check(
        &accepted.environment,
        &noble_kernel::untrusted::Request {
            input_bytes:accepted.request.input_bytes,
            expected:selected.expected.clone(),
            limits:accepted.request.limits,
        },
        &selected.body.candidate,
    );
    let Outcome::Accepted(derived)=derived else {
        return Err(output::Failure::error("named-report-derivation","accepted body derivation unavailable".into()));
    };
    let root_node=accepted.body.candidate.nodes.get(root.candidate_node.0 as usize);
    let Some(Node::Invocation {inst:root_inst,..})=root_node else {
        return Err(output::Failure::error("named-report-graph","subject submission root is not an invocation".into()));
    };
    let Outcome::Accepted(root_checked)=noble_kernel::acceptance::check(
        &accepted.environment,&accepted.request,&accepted.body.candidate
    ) else {
        return Err(output::Failure::error("named-report-derivation","submission root derivation unavailable".into()));
    };
    let root_derivation=root_checked.derivations.iter().find(|item|item.node==root.candidate_node)
        .ok_or_else(||output::Failure::error("named-report-derivation","submission root node derivation missing".into()))?;
    let mut use_reports=std::vec::Vec::with_capacity(2);
    for (order,use_record) in subject.named_uses.iter().enumerate().skip(1) {
        let module=subject.source_dependencies.get(use_record.module_index)
            .ok_or_else(||output::Failure::error("named-report-graph","original module missing".into()))?;
        let node=selected.body.candidate.nodes.get(use_record.candidate_node.0 as usize);
        let Some(Node::Invocation {def,inst})=node else {
            return Err(output::Failure::error("named-report-graph","named use node is not an invocation".into()));
        };
        if *def != use_record.definition {
            return Err(output::Failure::error("named-report-graph","named use slot differs".into()));
        }
        let actual=derived.derivations.iter().find(|item|item.node==use_record.candidate_node)
            .ok_or_else(||output::Failure::error("named-report-derivation","named use derivation missing".into()))?;
        let specialization=accepted.definitions.iter().find(|row|row.definition==use_record.definition)
            .ok_or_else(||output::Failure::error("named-report-graph","named specialization row missing".into()))?;
        let original=source_slice(&module.full_source,use_record.definition_span)?;
        let body=source_slice(&module.full_source,use_record.body_span)?;
        use_reports.push(encoding::object([
            ("order",encoding::Json::Number(order.saturating_sub(1) as u64)),
            ("original",encoding::object([
                ("module",encoding::string(&module.module)),
                ("version",encoding::Json::Number(u64::from(module.version))),
                ("module_source_sha256",encoding::string(&module_digests[use_record.module_index])),
                ("owner_session_local",encoding::Json::Number(module.owner)),
                ("definition",encoding::string(&use_record.definition_name)),
                ("ordinal",encoding::Json::Number(u64::from(use_record.definition_ordinal))),
                ("definition_span",span_evidence(use_record.definition_span)),
                ("definition_sha256",encoding::string(sha256(original))),
                ("body_span",span_evidence(use_record.body_span)),
                ("body_sha256",encoding::string(sha256(body))),
            ])),
            ("caller",encoding::object([
                ("module",encoding::string(&subject.module)),
                ("owner_session_local",encoding::Json::Number(subject.definition_owner)),
                ("definition",encoding::string(&subject.definition)),
                ("ordinal",encoding::Json::Number(u64::from(subject.definition_ordinal))),
                ("definition_span",span_evidence(subject.source_span)),
                ("lexical_span",use_record.source_span.map_or(encoding::Json::Null,span_evidence)),
                ("source_node",use_record.source_node.map_or(encoding::Json::Null,|id|encoding::Json::Number(u64::from(id)))),
                ("candidate_node",encoding::Json::Number(u64::from(use_record.candidate_node.0))),
            ])),
            ("accepted",encoding::object([
                ("specialization_slot",encoding::Json::Number(u64::from(use_record.definition.0))),
                ("definition_identity_session_local",encoding::Json::Number(use_record.definition_identity)),
                ("inst",inst_evidence(inst)),
                ("derivation",interface_evidence(&actual.interface)),
                ("specialization_body",specialization_evidence(accepted,specialization)?),
            ])),
        ]));
    }
    let original=subject.source_dependencies.get(root.module_index)
        .ok_or_else(||output::Failure::error("named-report-graph","subject original module missing".into()))?;
    Ok(encoding::object([
        ("owner",encoding::string(&subject.module)),
        ("version",encoding::Json::Number(u64::from(subject.version))),
        ("definition",encoding::string(&subject.definition)),
        ("definition_identity_session_local",encoding::string(subject.definition_identity.to_string())),
        ("module_source_sha256",encoding::string(&subject_digest)),
        ("definition_source_sha256",encoding::string(&definition_digest)),
        ("resolved_candidate",encoding::string(std::format!("{:?}",subject.accepted_submission.body.candidate))),
        ("resolved_definitions",encoding::string(std::format!("{:?}",subject.accepted_submission.definitions))),
        ("generated_statement_sha256",encoding::string(sha256(obligation_source.as_bytes()))),
        ("generated_claim",encoding::string("NamedV2Obligation.claim")),
        ("named_use_count",encoding::Json::Number(subject.named_uses.len() as u64)),
        ("resolved_imports",encoding::Json::Array(subject.named_imports.iter().map(|binding|
            encoding::object([
                ("alias",encoding::string(&binding.alias)),
                ("owner_session_local",encoding::Json::Number(binding.owner)),
                ("original_module_index",encoding::Json::Number(binding.module_index as u64)),
            ])).collect())),
        ("original_subject",encoding::object([
            ("module",encoding::string(&original.module)),
            ("version",encoding::Json::Number(u64::from(original.version))),
            ("module_source_sha256",encoding::string(&module_digests[root.module_index])),
            ("owner_session_local",encoding::Json::Number(original.owner)),
            ("definition",encoding::string(&root.definition_name)),
            ("ordinal",encoding::Json::Number(u64::from(root.definition_ordinal))),
            ("definition_span",span_evidence(root.definition_span)),
            ("body_span",span_evidence(root.body_span)),
            ("definition_sha256",encoding::string(&definition_digest)),
        ])),
        ("accepted_submission_root",encoding::object([
            ("specialization_slot",encoding::Json::Number(u64::from(root.definition.0))),
            ("candidate_node",encoding::Json::Number(u64::from(root.candidate_node.0))),
            ("inst",inst_evidence(root_inst)),
            ("derivation",interface_evidence(&root_derivation.interface)),
        ])),
        ("definition_body_uses",encoding::Json::Array(use_reports)),
    ]))
}
fn source_slice(source: &[u8],span:noble_contracts::Span)->Result<&[u8],output::Failure> {
    source.get(span.start as usize..span.end as usize).ok_or_else(||
        output::Failure::error("named-report-graph","original source span is outside immutable source".into()))
}
fn span_evidence(span:noble_contracts::Span)->encoding::Json {
    encoding::object([
        ("start",encoding::Json::Number(u64::from(span.start))),
        ("end",encoding::Json::Number(u64::from(span.end))),
    ])
}
fn type_evidence(types:&[noble_kernel::types::Ty])->encoding::Json {
    encoding::Json::Array(types.iter().map(|ty|
        encoding::string(std::format!("{ty:?}"))).collect())
}
fn interface_evidence(interface:&noble_kernel::untrusted::Interface)->encoding::Json {
    encoding::object([
        ("stack_in",type_evidence(&interface.stack_in)),
        ("stack_out",type_evidence(&interface.stack_out)),
        ("effects",encoding::string(std::format!("{:?}",interface.effects))),
    ])
}
fn inst_evidence(inst:&noble_kernel::words::Inst)->encoding::Json {
    use noble_kernel::words::Binding;
    encoding::Json::Array(inst.bindings.iter().map(|binding|match binding {
        Binding::Stack(types)=>encoding::object([
            ("kind",encoding::string("stack")),("types",type_evidence(types)),
        ]),
        Binding::Value(ty)=>encoding::object([
            ("kind",encoding::string("value")),
            ("type",encoding::string(std::format!("{ty:?}"))),
        ]),
        Binding::Effect(effects)=>encoding::object([
            ("kind",encoding::string("effect")),
            ("effects",encoding::string(std::format!("{effects:?}"))),
        ]),
        Binding::Ref(variable)=>encoding::object([
            ("kind",encoding::string("reference")),
            ("variable",encoding::Json::Number(u64::from(variable.0))),
        ]),
    }).collect())
}
fn specialization_evidence(
    submission:&noble_kernel::execution::Submission,
    definition:&noble_kernel::execution::Definition,
)->Result<encoding::Json,output::Failure> {
    use noble_kernel::untrusted::{Node,Outcome};
    let body=&definition.body.candidate;
    let request=noble_kernel::untrusted::Request {
        input_bytes:submission.request.input_bytes,
        expected:definition.expected.clone(),
        limits:submission.request.limits,
    };
    let Outcome::Accepted(checked)=noble_kernel::acceptance::check(
        &submission.environment,&request,body
    ) else {
        return Err(output::Failure::error("named-report-derivation","named step derivation unavailable".into()));
    };
    let mut nodes=std::vec::Vec::with_capacity(body.body.len());
    for id in &body.body {
        let node=body.nodes.get(id.0 as usize).ok_or_else(||
            output::Failure::error("named-report-graph","named step body node missing".into()))?;
        let derived=checked.derivations.iter().find(|item|item.node==*id)
            .ok_or_else(||output::Failure::error("named-report-derivation","named step node derivation missing".into()))?;
        let detail=match node {
            Node::Literal {lit,inst}=>encoding::object([
                ("kind",encoding::string("literal")),
                ("value",encoding::string(std::format!("{lit:?}"))),
                ("inst",inst_evidence(inst)),
            ]),
            Node::Invocation {def,inst}=>encoding::object([
                ("kind",encoding::string("invocation")),
                ("slot",encoding::Json::Number(u64::from(def.0))),
                ("inst",inst_evidence(inst)),
            ]),
            Node::Quotation {..}=>return Err(output::Failure::error(
                "named-report-graph","quoted step body is outside named rule".into())),
        };
        nodes.push(encoding::object([
            ("candidate_node",encoding::Json::Number(u64::from(id.0))),
            ("node",detail),
            ("derivation",interface_evidence(&derived.interface)),
        ]));
    }
    Ok(encoding::object([
        ("candidate_format",encoding::Json::Number(u64::from(body.format))),
        ("candidate_revision",encoding::Json::Number(u64::from(body.revision))),
        ("interface",interface_evidence(&checked.interface)),
        ("nodes",encoding::Json::Array(nodes)),
    ]))
}
fn execute(arguments: &[std::ffi::OsString], submitted_sha256: &mut Option<std::string::String>) -> Result<Option<encoding::Json>,output::Failure> {
    let path = arguments.get(1).ok_or_else(|| output::Failure::error("usage","verify-module SOURCE [--module FILE ...] [--timeout-ms N]".into()))?;
    let mut modules = std::vec::Vec::new();
    let mut timeout_ms = super::DEFAULT_TIMEOUT;
    let mut budgets = noble_contracts::intrinsic::ProofBudgets::from_limits(noble_contracts::Limits::default());
    let mut at = 2;
    while at < arguments.len() {
        let option = arguments[at].to_str().ok_or_else(||output::Failure::error("usage","argument must be UTF-8".into()))?;
        at += 1;
        let value = arguments.get(at).ok_or_else(|| output::Failure::error("usage","missing option value".into()))?;
        match option {
            "--module" => modules.push(std::path::PathBuf::from(value)),
            "--timeout-ms" => timeout_ms = value.to_str().and_then(|value|value.parse::<u64>().ok())
                .filter(|value| (1..=super::MAX_TIMEOUT).contains(value))
                .ok_or_else(|| output::Failure::error("usage","invalid proof deadline".into()))?,
            "--normalization-work" | "--substitution-work" => {
                let limit = value.to_str().and_then(|text|text.parse::<u32>().ok())
                    .filter(|value|*value <= noble_contracts::Limits::default().work)
                    .ok_or_else(|| output::Failure::error("usage","invalid proof work ceiling".into()))?;
                if option == "--normalization-work" { budgets.normalization_work=limit; }
                else { budgets.substitution_work=limit; }
            }
            _ => return Err(output::Failure::error("usage","unknown verify-module option".into())),
        }
        at += 1;
    }
    if modules.len() > super::MODULE_LIMIT { return Err(output::Failure::error("module-limit","too many prior module units".into())); }
    let limits = noble_contracts::Limits::default();
    let mut session = noble_contracts::source::ModuleSession::new(&[]).map_err(|error|source_error(&error))?;
    let target = std::path::PathBuf::from(path);
    for module in modules.iter().chain(core::iter::once(&target)) {
        let source = super::artifacts::read_bounded(module,super::SOURCE_LIMIT,"module-source")?;
        if module == &target { *submitted_sha256 = Some(sha256(&source)); }
        let prepared = session.prepare(&source,&[],limits).map_err(|error|source_error(&error))?;
        let has_proofs = prepared.proof_obligations().is_some();
        let mut accepted = None;
        let mut verification_failure = None;
        let (next,committed) = if has_proofs {
            session.commit_verified(prepared,|batch| {
                let (verified,evidence,work) = match verify_with_evidence(batch,&source,limits,timeout_ms,budgets) {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        let message=std::format!("{}: {}: {}",error.code,error.message,error.details.encode());
                        verification_failure=Some(error);
                        return Err(message);
                    }
                };
                if module == &target {
                    let digest = submitted_sha256.as_deref()
                        .expect("a verified target module must have been read");
                    accepted = Some(accepted_report(batch,&verified,&evidence,work,
                        &module.to_string_lossy(),digest));
                }
                Ok(verified)
            })
        } else { session.commit(prepared) };
        session = next;
        if let Some(failure)=verification_failure { return Err(failure); }
        committed.map_err(|error|source_error(&error))?;
        if module == &target {
            return Ok(accepted);
        }
    }
    Err(output::Failure::error("intrinsic-internal","no target module was selected".into()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_identity_sha256_matches_standard_vectors_across_block_boundary() {
        assert_eq!(super::sha256(b""),"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(super::sha256(b"abc"),"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(super::sha256(&[b'a';64]),"ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb");
        let mut streamed=super::Sha256::new();
        streamed.update(b"a");
        streamed.update(b"bc");
        assert_eq!(streamed.finish(),super::sha256(b"abc"));
        let mut million=super::Sha256::new();
        for _ in 0..1_000 {million.update(&[b'a';1_000]);}
        assert_eq!(million.finish(),"cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    }
}
