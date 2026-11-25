//! Distinct strict-kernel Lean consumer for revision-3 `Named-Contract-Calls-v3`
//! obligations (`PREUSE-CALL-01` / `PREUSE-LEMMA-01`).
//!
//! All transitive pure lemmas, revision-1 callees, and revision-3 callees are
//! replayed in topological post-order and individually kernel-checked inside the
//! isolated sandbox session before checking the caller's lowered `LiftedChain`.

struct CompiledDep {
    spec_module: std::string::String,
    proof_module: std::string::String,
}

pub(in crate::workflow) struct CallerVerification<'a> {
    pub(in crate::workflow) obligation: &'a noble_contracts::intrinsic::ProofObligation,
    pub(in crate::workflow) contract: &'a noble_contracts::intrinsic::ContractGoal,
    pub(in crate::workflow) proof: &'a noble_contracts::intrinsic::CheckedProof,
    pub(in crate::workflow) batch: &'a noble_contracts::intrinsic::ProofBatch,
    pub(in crate::workflow) obligation_source: &'a str,
    pub(in crate::workflow) library: &'a super::super::rules::Library,
    pub(in crate::workflow) limits: noble_contracts::Limits,
    pub(in crate::workflow) timeout_ms: u64,
}

#[derive(Clone, Copy)]
enum DepNodeKind<'a> {
    Dependency(&'a noble_contracts::intrinsic::ProofDependency),
    Local(&'a noble_contracts::intrinsic::ProofObligation),
}

struct DepWork<'a> {
    key: std::string::String,
    node: DepNodeKind<'a>,
    local_scope: &'a [noble_contracts::intrinsic::ProofObligation],
    sub_deps: &'a [noble_contracts::intrinsic::ProofDependency],
}

fn dependency_key(dep: &noble_contracts::intrinsic::ProofDependency) -> std::string::String {
    std::format!(
        "dep/{}@{}/{}",
        dep.module,
        dep.version,
        dep.declaration.name
    )
}

fn local_key(obl: &noble_contracts::intrinsic::ProofObligation) -> std::string::String {
    std::format!("local/{}", obl.name)
}

fn find_compiled(compiled: &[(std::string::String, usize)], key: &str) -> Option<usize> {
    compiled
        .iter()
        .find(|(existing, _)| existing == key)
        .map(|(_, idx)| *idx)
}

fn resolve_dep_ref<'a>(
    reference: &str,
    dependencies: &'a [noble_contracts::intrinsic::ProofDependency],
    local: &'a [noble_contracts::intrinsic::ProofObligation],
) -> Result<DepWork<'a>, super::super::output::Failure> {
    if let Some(dep) = dependencies.iter().find(|d| d.name == reference) {
        return Ok(DepWork {
            key: dependency_key(dep),
            node: DepNodeKind::Dependency(dep),
            local_scope: &[],
            sub_deps: &dep.dependencies,
        });
    }
    if let Some(obl) = local.iter().find(|o| o.name == reference) {
        return Ok(DepWork {
            key: local_key(obl),
            node: DepNodeKind::Local(obl),
            local_scope: local,
            sub_deps: dependencies,
        });
    }
    Err(super::super::output::Failure::error(
        "named-v3-dependency-missing",
        std::format!("missing dependency proof `{reference}` during Lean replay"),
    ))
}

fn recheck_obligation(
    module: &str,
    version: u32,
    source: &[u8],
    obligation: &noble_contracts::intrinsic::ProofObligation,
    dependencies: &[noble_contracts::intrinsic::ProofDependency],
    limits: noble_contracts::Limits,
) -> Result<noble_contracts::intrinsic::CheckedProof, super::super::output::Failure> {
    let synthetic = noble_contracts::intrinsic::ProofBatch {
        module: module.into(),
        version,
        source: source.to_vec(),
        generation: 0,
        dependencies: dependencies.to_vec(),
        obligations: std::vec![obligation.clone()],
        elaborations: std::vec::Vec::new(),
        elaboration_work: noble_contracts::intrinsic::ProofWork {
            normalization: 0,
            substitution: 0,
            total: 0,
        },
    };
    let checked =
        noble_contracts::intrinsic::check_batch(&synthetic, limits).map_err(|problem| {
            super::super::output::Failure::error("named-v3-recheck-failed", problem.message)
        })?;
    checked.into_iter().next().ok_or_else(|| {
        super::super::output::Failure::error(
            "named-v3-recheck-failed",
            "rechecked dependency batch yielded no checked proof".into(),
        )
    })
}

fn compile_lean_in_deps(
    session: &super::Session,
    module_name: &str,
    source_text: &str,
    extra_search: &str,
) -> Result<(), super::super::output::Failure> {
    let deps_dir = &session.workspace.submission;
    let src_name = std::format!("{module_name}.lean");
    let obj_name = std::format!("{module_name}.olean");
    attempt!(super::super::artifacts::write_source(
        &deps_dir.join(&src_name),
        source_text.as_bytes()
    ));
    let object = std::path::Path::new(&obj_name);
    let writable = attempt!(super::super::artifacts::output_slots(deps_dir, object));
    let mounts = attempt!(super::super::artifacts::output_mounts(
        std::vec![
            super::super::artifacts::read_mount(&session.workspace.library, "/library"),
            super::super::artifacts::read_mount(deps_dir, "/deps"),
        ],
        deps_dir,
        &writable,
        "/deps"
    ));
    let transcript = attempt!(session.compile_module(
        mounts,
        extra_search,
        super::compilation::Job {
            source: &std::format!("/deps/{src_name}"),
            root: "/deps",
            output: &std::format!("/deps/{obj_name}"),
        }
    ));
    attempt!(super::require_success(
        transcript,
        "named-v3-dependency-rejected",
        "dependency Lean obligation or proof failed strict kernel check"
    ));
    super::super::artifacts::finish_outputs(deps_dir, object, &writable)
}

fn compile_intrinsic_witness_in_lib(
    session: &super::Session,
) -> Result<(), super::super::output::Failure> {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proofs/mc1/IntrinsicTypeWitness.lean"
    ));
    attempt!(super::super::artifacts::write_source(
        &session.workspace.sources.join("IntrinsicTypeWitness.lean"),
        source.as_bytes()
    ));
    let object = std::path::Path::new("IntrinsicTypeWitness.olean");
    let writable = attempt!(super::super::artifacts::output_slots(
        &session.workspace.library,
        object
    ));
    let mounts = attempt!(super::super::artifacts::output_mounts(
        std::vec![
            super::super::artifacts::read_mount(&session.workspace.sources, "/inputs"),
            super::super::artifacts::read_mount(&session.workspace.library, "/out"),
        ],
        &session.workspace.library,
        &writable,
        "/out"
    ));
    let transcript = attempt!(session.compile_module(
        mounts,
        "/out",
        super::compilation::Job {
            source: "/inputs/IntrinsicTypeWitness.lean",
            root: "/inputs",
            output: "/out/IntrinsicTypeWitness.olean",
        }
    ));
    attempt!(super::require_success(
        transcript,
        "intrinsic-library-rejected",
        "trusted PureTyCode witness failed Lean"
    ));
    super::super::artifacts::finish_outputs(&session.workspace.library, object, &writable)
}

struct DepCompileState {
    seen: std::vec::Vec<(std::string::String, usize)>,
    compiled: std::vec::Vec<CompiledDep>,
}

fn compile_dep_recursive(
    work: DepWork<'_>,
    caller: &CallerVerification<'_>,
    session: &super::Session,
    state: &mut DepCompileState,
) -> Result<usize, super::super::output::Failure> {
    if let Some(idx) = find_compiled(&state.seen, &work.key) {
        return Ok(idx);
    }
    let (obligation, module_name, version, source) = match work.node {
        DepNodeKind::Dependency(dep) => (
            &dep.declaration,
            dep.module.as_str(),
            dep.version,
            dep.source.as_slice(),
        ),
        DepNodeKind::Local(obl) => (
            obl,
            caller.batch.module.as_str(),
            caller.batch.version,
            caller.batch.source.as_slice(),
        ),
    };
    let checked = attempt!(recheck_obligation(
        module_name,
        version,
        source,
        obligation,
        work.sub_deps,
        caller.limits
    ));

    match &obligation.goal {
        noble_contracts::intrinsic::PendingGoal::Pure { .. } => {
            let idx = state.compiled.len();
            let spec_mod = std::format!("DepSpec_{idx}");
            let proof_mod = std::format!("Dep_{idx}");
            let spec_src = std::format!(
                "import IntrinsicTypeWitness\nopen NobleContracts.Intrinsic\nnamespace {spec_mod}\ndef claim : Prop := {}\nend {spec_mod}\n",
                checked.claim
            );
            attempt!(compile_lean_in_deps(
                session,
                &spec_mod,
                &spec_src,
                "/library:/deps"
            ));
            let proof_src = std::format!(
                "import {spec_mod}\nopen NobleContracts.Intrinsic\nnamespace {proof_mod}\ntheorem proof : {spec_mod}.claim := {}\nend {proof_mod}\n",
                checked.lean_term
            );
            if proof_src.len() > super::super::PROOF_LIMIT {
                return Err(super::super::output::Failure::error(
                    "named-proof-limit",
                    "lowered dependency proof exceeded bounded source size".into(),
                ));
            }
            attempt!(compile_lean_in_deps(
                session,
                &proof_mod,
                &proof_src,
                "/library:/deps"
            ));
            state.compiled.push(CompiledDep {
                spec_module: spec_mod,
                proof_module: proof_mod,
            });
            state.seen.push((work.key, idx));
            Ok(idx)
        }
        noble_contracts::intrinsic::PendingGoal::Contract { contract }
            if contract.revision == 1 =>
        {
            let prepared = noble_contracts::intrinsic::prepare_contract(contract, caller.limits)
                .map_err(|problem| {
                    super::super::output::Failure::error(
                        "intrinsic-contract-invalid",
                        problem.message,
                    )
                })?;
            let mc1_src = noble_contracts::export_lean(&prepared);
            let idx = state.compiled.len();
            let spec_mod = std::format!("DepSpec_{idx}");
            let proof_mod = std::format!("Dep_{idx}");
            let spec_src = mc1_src
                .replace(
                    "namespace MC1Obligation",
                    &std::format!("namespace {spec_mod}"),
                )
                .replace("end MC1Obligation", &std::format!("end {spec_mod}"));
            attempt!(compile_lean_in_deps(
                session,
                &spec_mod,
                &spec_src,
                "/library:/deps"
            ));
            let proof_src = std::format!(
                "import {spec_mod}\nopen NobleContracts\nnamespace {proof_mod}\ntheorem proof : {spec_mod}.claim := {}\nend {proof_mod}\n",
                checked.lean_term
            );
            if proof_src.len() > super::super::PROOF_LIMIT {
                return Err(super::super::output::Failure::error(
                    "named-proof-limit",
                    "lowered mc1 callee proof exceeded bounded source size".into(),
                ));
            }
            attempt!(compile_lean_in_deps(
                session,
                &proof_mod,
                &proof_src,
                "/library:/deps"
            ));
            state.compiled.push(CompiledDep {
                spec_module: spec_mod,
                proof_module: proof_mod,
            });
            state.seen.push((work.key, idx));
            Ok(idx)
        }
        noble_contracts::intrinsic::PendingGoal::Contract { contract }
            if contract.revision == 3 =>
        {
            let env_v3 = noble_contracts::intrinsic::prepare_named_v3(
                contract,
                work.sub_deps,
                work.local_scope,
                &obligation.term,
                caller.limits,
            )
            .map_err(|problem| {
                super::super::output::Failure::error("intrinsic-contract-invalid", problem.message)
            })?;
            let mut child_indices: std::vec::Vec<usize> = std::vec::Vec::new();
            for lemma_ref in &env_v3.envelope.lemmas {
                let child = attempt!(resolve_dep_ref(lemma_ref, work.sub_deps, work.local_scope));
                child_indices.push(attempt!(compile_dep_recursive(
                    child, caller, session, state
                )));
            }
            let mut seen_callees: std::vec::Vec<&str> = std::vec::Vec::new();
            for occ in &env_v3.envelope.occurrences {
                if !seen_callees.contains(&occ.proof.as_str()) {
                    seen_callees.push(occ.proof.as_str());
                    let child =
                        attempt!(resolve_dep_ref(&occ.proof, work.sub_deps, work.local_scope));
                    child_indices.push(attempt!(compile_dep_recursive(
                        child, caller, session, state
                    )));
                }
            }
            let raw_spec =
                noble_contracts::intrinsic::prepare_named_v3_contract(contract, caller.limits)
                    .map_err(|problem| {
                        super::super::output::Failure::error(
                            "intrinsic-contract-invalid",
                            problem.message,
                        )
                    })?;
            let idx = state.compiled.len();
            let spec_mod = std::format!("DepSpec_{idx}");
            let proof_mod = std::format!("Dep_{idx}");
            let spec_src = raw_spec
                .replace(
                    "namespace NamedV3Obligation",
                    &std::format!("namespace {spec_mod}"),
                )
                .replace("end NamedV3Obligation", &std::format!("end {spec_mod}"));
            attempt!(compile_lean_in_deps(
                session,
                &spec_mod,
                &spec_src,
                "/library:/deps"
            ));
            let mut imports = std::format!("import {spec_mod}\n");
            let mut term_args = std::string::String::new();
            let rewritten_term = checked
                .lean_term
                .replace("NamedV3Obligation.", &std::format!("{spec_mod}."));
            if child_indices.is_empty() {
                term_args.push_str(&rewritten_term);
            } else {
                term_args.push('(');
                term_args.push_str(&rewritten_term);
                term_args.push(')');
                for &c_idx in &child_indices {
                    let c_mod = &state.compiled[c_idx].proof_module;
                    imports.push_str(&std::format!("import {c_mod}\n"));
                    term_args.push_str(&std::format!(" {c_mod}.proof"));
                }
            }
            let proof_src = std::format!(
                "{imports}open NobleContracts\nnamespace {proof_mod}\ntheorem proof : {spec_mod}.claim := {term_args}\nend {proof_mod}\n"
            );
            if proof_src.len() > super::super::PROOF_LIMIT {
                return Err(super::super::output::Failure::error(
                    "named-proof-limit",
                    "lowered rev3 callee proof exceeded bounded source size".into(),
                ));
            }
            attempt!(compile_lean_in_deps(
                session,
                &proof_mod,
                &proof_src,
                "/library:/deps"
            ));
            state.compiled.push(CompiledDep {
                spec_module: spec_mod,
                proof_module: proof_mod,
            });
            state.seen.push((work.key, idx));
            Ok(idx)
        }
        _ => Err(super::super::output::Failure::unsupported(
            "named-v3-unsupported-dependency",
            "unsupported dependency revision in revision-3 closure replay".into(),
        )),
    }
}

/// A distinct strict-kernel consumer for one host-authenticated revision-3
/// obligation and its closed `LiftedChain` lowering.
pub(in crate::workflow) fn verify(
    caller: &CallerVerification<'_>,
) -> Result<super::Acceptance, super::super::output::Failure> {
    let deadline = attempt!(crate::sandbox::now()
        .checked_add(std::time::Duration::from_millis(caller.timeout_ms))
        .ok_or_else(|| super::super::output::Failure::error(
            "timeout-overflow",
            "invalid named v3 proof deadline".into()
        )));
    let sandbox = attempt!(crate::sandbox::Environment::discover(deadline));
    let probe = attempt!(super::probe_toolchain(&sandbox, deadline));
    let session = super::Session {
        sandbox,
        workspace: attempt!(super::Workspace::create(caller.library, false)),
        deadline,
    };
    attempt!(session.compile_library_for(
        caller.library,
        &["NobleContracts.NamedV3Lifted", "NobleContracts"]
    ));
    attempt!(compile_intrinsic_witness_in_lib(&session));

    let env_v3 = noble_contracts::intrinsic::prepare_named_v3(
        caller.contract,
        &caller.batch.dependencies,
        &caller.batch.obligations,
        &caller.obligation.term,
        caller.limits,
    )
    .map_err(|problem| {
        super::super::output::Failure::error("intrinsic-contract-invalid", problem.message)
    })?;

    let mut dep_state = DepCompileState {
        seen: std::vec::Vec::new(),
        compiled: std::vec::Vec::new(),
    };
    let mut direct_indices: std::vec::Vec<usize> = std::vec::Vec::new();
    for lemma_ref in &env_v3.envelope.lemmas {
        let dep = attempt!(resolve_dep_ref(
            lemma_ref,
            &caller.batch.dependencies,
            &caller.batch.obligations
        ));
        direct_indices.push(attempt!(compile_dep_recursive(
            dep,
            caller,
            &session,
            &mut dep_state
        )));
    }
    let mut seen_callees: std::vec::Vec<&str> = std::vec::Vec::new();
    for occ in &env_v3.envelope.occurrences {
        if !seen_callees.contains(&occ.proof.as_str()) {
            seen_callees.push(occ.proof.as_str());
            let dep = attempt!(resolve_dep_ref(
                &occ.proof,
                &caller.batch.dependencies,
                &caller.batch.obligations
            ));
            direct_indices.push(attempt!(compile_dep_recursive(
                dep,
                caller,
                &session,
                &mut dep_state
            )));
        }
    }

    let generated = &session.workspace.obligation;
    attempt!(super::super::artifacts::write_source(
        &generated.join("NamedV3Obligation.lean"),
        caller.obligation_source.as_bytes()
    ));
    let object = std::path::Path::new("NamedV3Obligation.olean");
    let writable = attempt!(super::super::artifacts::output_slots(generated, object));
    let mounts = attempt!(super::super::artifacts::output_mounts(
        std::vec![
            super::super::artifacts::read_mount(&session.workspace.library, "/library"),
            super::super::artifacts::read_mount(generated, "/obligation"),
        ],
        generated,
        &writable,
        "/obligation"
    ));
    let transcript = attempt!(session.compile_module(
        mounts,
        "/library:/obligation",
        super::compilation::Job {
            source: "/obligation/NamedV3Obligation.lean",
            root: "/obligation",
            output: "/obligation/NamedV3Obligation.olean",
        }
    ));
    attempt!(super::require_success(
        transcript,
        "named-obligation-rejected",
        "generated revision-3 source obligation failed strict Lean compilation"
    ));
    attempt!(super::super::artifacts::finish_outputs(
        generated, object, &writable
    ));

    let mut dep_imports = std::string::String::new();
    let mut import_list = std::string::String::from("{ module := `NamedV3Obligation }");
    let mut dep_checks = std::string::String::new();
    for (dep_idx, compiled) in dep_state.compiled.iter().enumerate() {
        let p_mod = &compiled.proof_module;
        let s_mod = &compiled.spec_module;
        dep_imports.push_str(&std::format!("import {s_mod}\nimport {p_mod}\n"));
        import_list.push_str(&std::format!(
            ", {{ module := `{s_mod} }}, {{ module := `{p_mod} }}"
        ));
        dep_checks.push_str(&std::format!(
            r#"  let some (.thmInfo depProof_{dep_idx}) := trusted.find? ``{p_mod}.proof
    | throwError "dependency proof missing from replayed kernel"
  match Kernel.isDefEq (.ofKernelEnv trusted) {{}} depProof_{dep_idx}.type (mkConst `{s_mod}.claim) with
  | .ok true => pure ()
  | _ => throwError "dependency claim mismatch"
  let depAxioms_{dep_idx} ← liftCoreM (collectAxioms ``{p_mod}.proof)
  for ax in depAxioms_{dep_idx} do
    unless [``propext, ``Classical.choice, ``Quot.sound].contains ax do
      throwError "disallowed dependency axiom {{ax}}"
"#
        ));
    }
    let mut applied_term = std::string::String::new();
    if direct_indices.is_empty() {
        applied_term.push_str(&caller.proof.lean_term);
    } else {
        applied_term.push('(');
        applied_term.push_str(&caller.proof.lean_term);
        applied_term.push(')');
        for &d_idx in &direct_indices {
            let p_mod = &dep_state.compiled[d_idx].proof_module;
            applied_term.push_str(&std::format!(" {p_mod}.proof"));
        }
    }

    let consumer = std::format!(
        r#"import Lean
import NamedV3Obligation
{dep_imports}open NobleContracts
open Lean Elab Command
theorem named_v3_proof : NamedV3Obligation.claim := {applied_term}
elab "check_named_v3_axioms" : command => do
  let env ← getEnv
  let some (.thmInfo proof) := env.find? ``named_v3_proof
    | throwError "named v3 proof was not a theorem"
  let imported ← liftIO <| importModules
    #[{import_list}] {{}} (trustLevel := 0) (loadExts := false)
  let empty ← liftIO mkEmptyEnvironment
  let trusted ← liftIO <| Kernel.Environment.replay imported.toKernelEnv.constants.map₁ empty.toKernelEnv
{dep_checks}  let checked ← match trusted.addDeclCore 0 maxRecDepth.defValue.toUSize (.thmDecl proof) (cancelTk? := none) with
    | .ok next => pure next
    | .error _ => throwError "named v3 fresh kernel rejected proof"
  match Kernel.isDefEq (.ofKernelEnv checked) {{}} proof.type
      (mkConst `NamedV3Obligation.claim) with
  | .ok true => pure ()
  | _ => throwError "named v3 theorem has wrong exact claim type"
  let axioms ← liftCoreM (collectAxioms ``named_v3_proof)
  for ax in axioms do
    unless [``propext, ``Classical.choice, ``Quot.sound].contains ax do
      throwError "disallowed named v3 axiom {{ax}}"
  logInfo "NOBLE-NAMED-V3-ACCEPT"
  for ax in axioms do
    logInfo m!"NOBLE-NAMED-V3-AXIOM:{{ax}}"
check_named_v3_axioms
"#
    );
    if consumer.len() > super::super::PROOF_LIMIT {
        return Err(super::super::output::Failure::error(
            "named-proof-limit",
            "lowered revision-3 proof exceeded bounded source size".into(),
        ));
    }
    attempt!(super::super::artifacts::write_source(
        &session.workspace.consumer.join("NamedV3Consumer.lean"),
        consumer.as_bytes()
    ));
    let mounts = std::vec![
        super::super::artifacts::read_mount(&session.workspace.library, "/library"),
        super::super::artifacts::read_mount(&session.workspace.submission, "/deps"),
        super::super::artifacts::read_mount(generated, "/obligation"),
        super::super::artifacts::read_mount(&session.workspace.consumer, "/consumer"),
    ];
    let transcript = attempt!(session.sandbox.run(
        &mounts,
        "/library:/deps:/obligation",
        &std::vec![
            "-R".into(),
            "/consumer".into(),
            "-j".into(),
            "2".into(),
            "/consumer/NamedV3Consumer.lean".into()
        ],
        deadline
    ));
    let transcript = attempt!(super::require_intrinsic_consumer_success(transcript));
    let axioms = attempt!(super::intrinsic_axioms(
        &transcript.stdout,
        "NOBLE-NAMED-V3"
    ));
    Ok(super::Acceptance {
        axioms,
        wire: consumer,
        tools: super::tools(&session.sandbox, &probe.stdout),
    })
}
