//! Distinct strict-kernel Lean consumer for revision-3 `Named-Contract-Calls-v3`
//! obligations (`PREUSE-CALL-01` / `PREUSE-LEMMA-01`).
//!
//! All transitive pure lemmas, revision-1 callees, and revision-3 callees are
//! replayed in topological post-order and individually kernel-checked inside the
//! isolated sandbox session before checking the caller's lowered `LiftedChain`.

struct CompiledDep {
    spec_module: std::string::String,
    proof_module: std::string::String,
    goal: Option<std::boxed::Box<noble_contracts::intrinsic::ContractGoal>>,
}

struct CalleeArg {
    dep_idx: usize,
    occ_slot: u32,
    occ_ui: usize,
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
    if let Some((pos, obl)) = local.iter().enumerate().find(|(_, o)| o.name == reference) {
        return Ok(DepWork {
            key: local_key(obl),
            node: DepNodeKind::Local(obl),
            local_scope: &local[..pos],
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
    preceding_locals: &[noble_contracts::intrinsic::ProofObligation],
    dependencies: &[noble_contracts::intrinsic::ProofDependency],
    limits: noble_contracts::Limits,
) -> Result<noble_contracts::intrinsic::CheckedProof, super::super::output::Failure> {
    let is_rev3 = obligation.revision == 3;
    let mut obligations: std::vec::Vec<_> = preceding_locals
        .iter()
        .filter(|obl| {
            is_rev3
                || matches!(
                    obl.goal,
                    noble_contracts::intrinsic::PendingGoal::Pure { .. }
                )
        })
        .cloned()
        .collect();
    obligations.push(obligation.clone());
    let filtered_dependencies: std::vec::Vec<_> = dependencies
        .iter()
        .filter(|dep| {
            is_rev3
                || matches!(
                    dep.declaration.goal,
                    noble_contracts::intrinsic::PendingGoal::Pure { .. }
                )
        })
        .cloned()
        .collect();
    let synthetic = noble_contracts::intrinsic::ProofBatch {
        module: module.into(),
        version,
        source: source.to_vec(),
        generation: 0,
        dependencies: filtered_dependencies,
        obligations,
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
    checked.into_iter().next_back().ok_or_else(|| {
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

const RUN_SIM_PRELUDE: &str = r#"let simOp (f : Nat → Nat) (opB opA : NobleContracts.NamedV1.Op) : Prop :=
  @NobleContracts.NamedV1.Op.casesOn (fun _ => Prop) opB
    (fun vB => @NobleContracts.NamedV1.Op.casesOn (fun _ => Prop) opA (fun vA => vB = vA) (fun _ => False) (fun _ => False) (fun _ => False))
    (fun wB => @NobleContracts.NamedV1.Op.casesOn (fun _ => Prop) opA (fun _ => False) (fun wA => wB = wA) (fun _ => False) (fun _ => False))
    (fun _ => False)
    (fun uB => @NobleContracts.NamedV1.Op.casesOn (fun _ => Prop) opA (fun _ => False) (fun _ => False) (fun _ => False)
      (fun uA => uA.slot = f uB.slot ∧ uA.caller = uB.caller ∧ uA.origin = uB.origin ∧ uA.input = uB.input ∧ uA.output = uB.output))
let simCode (f : Nat → Nat) (bs : List NobleContracts.NamedV1.Op) : List NobleContracts.NamedV1.Op → Prop :=
  List.recOn (motive := fun _ => List NobleContracts.NamedV1.Op → Prop) bs
    (fun as => List.casesOn (motive := fun _ => Prop) as True (fun _ _ => False))
    (fun b _ ih as => List.casesOn (motive := fun _ => Prop) as False (fun a as' => simOp f b a ∧ ih as'))
let runSim (f : Nat → Nat) (envB envA : NobleContracts.NamedV1.Environment)
    (hbodies : ∀ e ∈ envB.entries, NobleContracts.NamedV3.Sequential e.body)
    (hlookup : ∀ e ∈ envB.entries, ∃ eA, envA.lookup (f e.slot) = some eA ∧
      eA.origin = e.origin ∧ eA.input = e.input ∧ eA.output = e.output ∧
      simCode f e.body eA.body)
    {codeB codeA : List NobleContracts.NamedV1.Op} {s t : NobleContracts.NamedV1.Stack}
    (hseq : NobleContracts.NamedV3.Sequential codeB)
    (hsim : simCode f codeB codeA)
    (hrun : NobleContracts.NamedV1.Run envB codeB s t) : NobleContracts.NamedV1.Run envA codeA s t := by
  suffices H : ∀ codeA, NobleContracts.NamedV3.Sequential codeB → simCode f codeB codeA → NobleContracts.NamedV1.Run envA codeA s t from
    H codeA hseq hsim
  refine NobleContracts.NamedV1.Run.recOn
    (motive_1 := fun codeB s t _ => ∀ codeA, NobleContracts.NamedV3.Sequential codeB → simCode f codeB codeA → NobleContracts.NamedV1.Run envA codeA s t)
    (motive_2 := fun opB s t _ => ∀ opA, NobleContracts.NamedV3.Sequential [opB] → simOp f opB opA → NobleContracts.NamedV1.Step envA opA s t)
    hrun
    (fun codeA _ hsim => by
      cases codeA with
      | nil => exact .nil
      | cons _ _ => exact False.elim hsim)
    (fun {opB opsB s m t} _ _ ihStep ihRest codeA hseq hsim => by
      cases opB with
      | lit v =>
        cases codeA with
        | nil => exact False.elim hsim
        | cons opA opsA => exact .cons (ihStep opA ⟨hseq.1, trivial⟩ hsim.1) (ihRest opsA hseq.2 hsim.2)
      | word w =>
        cases codeA with
        | nil => exact False.elim hsim
        | cons opA opsA => exact .cons (ihStep opA ⟨hseq.1, trivial⟩ hsim.1) (ihRest opsA hseq.2 hsim.2)
      | call u =>
        cases codeA with
        | nil => exact False.elim hsim
        | cons opA opsA => exact .cons (ihStep opA trivial hsim.1) (ihRest opsA hseq hsim.2)
      | block _ => exact False.elim hseq)
    ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_ ?_
  all_goals (intros; rename_i opA hseq hOp)
  all_goals first
    | (rename_i useB entryB _ _ lookupB matchesB _ ihBody
       cases opA with
       | call uA =>
         rcases hOp with ⟨hslot, hcaller, horigin, hinput, houtput⟩
         have hmem := List.mem_of_find?_eq_some lookupB
         have hslotB : entryB.slot = useB.slot :=
           Nat.beq_eq.mp (List.find?_some (p := fun e : NobleContracts.NamedV1.Entry => e.slot == useB.slot) lookupB)
         obtain ⟨eA, hlookupA, horigA, hinA, houtA, hsimBody⟩ := hlookup _ hmem
         rw [hslotB, ← hslot] at hlookupA
         refine .call hlookupA ?_ (ihBody _ (hbodies _ hmem) hsimBody)
         have hslotA : eA.slot = uA.slot :=
           Nat.beq_eq.mp (List.find?_some (p := fun e : NobleContracts.NamedV1.Entry => e.slot == uA.slot) hlookupA)
         exact ⟨hslotA, horigA.trans (matchesB.2.1.trans horigin.symm),
           hinA.trans (matchesB.2.2.1.trans hinput.symm),
           houtA.trans (matchesB.2.2.2.trans houtput.symm)⟩
       | lit _ => exact False.elim hOp
       | word _ => exact False.elim hOp
       | block _ => exact False.elim hOp)
    | (cases opA <;> first | exact False.elim hOp | (subst hOp; exact .lit))
    | (cases opA <;> first | exact False.elim hOp | (subst hOp; exact .dup))
    | (cases opA <;> first | exact False.elim hOp | (subst hOp; exact .drop))
    | (cases opA <;> first | exact False.elim hOp | (subst hOp; exact .swap))
    | (cases opA <;> first | exact False.elim hOp | (subst hOp; exact .add))
    | (cases opA <;> first | exact False.elim hOp | (subst hOp; exact .sub))
    | (cases opA <;> first | exact False.elim hOp | (subst hOp; exact .mul))
    | (cases opA <;> first | exact False.elim hOp | (subst hOp; exact .equals))
    | exact False.elim hseq
    | exact False.elim (by rcases hseq.1 with h | h | h | h | h | h | h <;> cases h)
"#;

fn caller_callee_sub_slots(
    caller_goal: &noble_contracts::intrinsic::ContractGoal,
    occ_slot: u32,
) -> std::vec::Vec<u32> {
    let defs = &caller_goal.subject.accepted_submission.definitions;
    let uses = &caller_goal.subject.named_uses;
    let selected_def = uses[0].definition;
    let mut metas: std::vec::Vec<(u32, u32, std::vec::Vec<u32>)> = std::vec::Vec::new();
    for def in defs {
        if def.definition != selected_def {
            let mut deps: std::vec::Vec<u32> =
                std::vec::Vec::with_capacity(def.body.candidate.body.len());
            for pos in 0..def.body.candidate.body.len() {
                if let Some(u) = uses.iter().skip(1).find(|u| {
                    u.caller_definition == Some(def.definition) && u.source_node == Some(pos as u32)
                }) {
                    let target_slot = u.definition.0;
                    if !deps.contains(&target_slot) {
                        deps.push(target_slot);
                    }
                }
            }
            metas.push((def.definition.0, 0, deps));
        }
    }
    let mut pass = 0usize;
    let mut is_changed = true;
    while is_changed && pass <= metas.len() {
        is_changed = false;
        for i in 0..metas.len() {
            let mut needed = 0u32;
            for di in 0..metas[i].2.len() {
                let target_slot = metas[i].2[di];
                if let Some(t_idx) = metas.iter().position(|m| m.0 == target_slot) {
                    let cand = metas[t_idx].1.saturating_add(1);
                    if cand > needed {
                        needed = cand;
                    }
                }
            }
            if needed > metas[i].1 {
                metas[i].1 = needed;
                is_changed = true;
            }
        }
        pass += 1;
    }
    let mut i = 1usize;
    while i < metas.len() {
        let mut j = i;
        while j > 0 && metas[j.saturating_sub(1)].1 > metas[j].1 {
            metas.swap(j.saturating_sub(1), j);
            j = j.saturating_sub(1);
        }
        i += 1;
    }
    let mut work: std::vec::Vec<u32> = std::vec::Vec::new();
    if let Some(m_idx) = metas.iter().position(|m| m.0 == occ_slot) {
        work.extend_from_slice(&metas[m_idx].2);
    }
    let mut seen: std::vec::Vec<u32> = std::vec::Vec::new();
    while let Some(slot) = work.pop() {
        if !seen.contains(&slot) {
            seen.push(slot);
            if let Some(m_idx) = metas.iter().position(|m| m.0 == slot) {
                work.extend_from_slice(&metas[m_idx].2);
            }
        }
    }
    metas
        .iter()
        .filter(|m| seen.contains(&m.0))
        .map(|m| m.0)
        .collect()
}

fn sim_code_proof(
    goal: &noble_contracts::intrinsic::ContractGoal,
    def_slot: u32,
) -> std::string::String {
    let Some(def) = goal
        .subject
        .accepted_submission
        .definitions
        .iter()
        .find(|d| d.definition.0 == def_slot)
    else {
        return "True.intro".into();
    };
    let body_len = def.body.candidate.body.len();
    let mut out = std::string::String::from("True.intro");
    for pos in (0..body_len).rev() {
        let is_call = goal.subject.named_uses.iter().skip(1).any(|u| {
            u.caller_definition == Some(def.definition) && u.source_node == Some(pos as u32)
        });
        if is_call {
            out = std::format!("⟨⟨rfl, rfl, rfl, rfl, rfl⟩, {out}⟩");
        } else {
            out = std::format!("⟨rfl, {out}⟩");
        }
    }
    out
}

fn is_nonleaf_rev3(goal: Option<&noble_contracts::intrinsic::ContractGoal>) -> bool {
    match goal {
        Some(g) => g.revision == 3 && g.subject.accepted_submission.definitions.len() > 1,
        None => false,
    }
}

fn emit_callee_arg(
    caller_goal: &noble_contracts::intrinsic::ContractGoal,
    caller_spec_mod: &str,
    arg: &CalleeArg,
    arg_idx: usize,
    compiled: &[CompiledDep],
) -> Result<std::string::String, super::super::output::Failure> {
    let dep = &compiled[arg.dep_idx];
    let p_mod = &dep.proof_module;
    let s_mod = &dep.spec_module;
    let Some(callee_goal) = dep.goal.as_deref() else {
        return Ok(std::format!("{p_mod}.proof"));
    };
    if !is_nonleaf_rev3(Some(callee_goal)) {
        return Ok(std::format!("{p_mod}.proof"));
    }
    let callee_root_slot = callee_goal.subject.named_uses[0].definition.0;
    let mut slot_map: std::vec::Vec<(u32, u32)> = std::vec![(arg.occ_slot, callee_root_slot)];
    let mut queue: std::vec::Vec<(u32, u32)> = std::vec![(arg.occ_slot, callee_root_slot)];
    while let Some((b_slot, a_slot)) = queue.pop() {
        let Some(def_b) = caller_goal
            .subject
            .accepted_submission
            .definitions
            .iter()
            .find(|d| d.definition.0 == b_slot)
        else {
            return Err(super::super::output::Failure::error(
                "named-v3-slot-map",
                "caller specialization entry missing during Lean transport".into(),
            ));
        };
        let Some(def_a) = callee_goal
            .subject
            .accepted_submission
            .definitions
            .iter()
            .find(|d| d.definition.0 == a_slot)
        else {
            return Err(super::super::output::Failure::error(
                "named-v3-slot-map",
                "callee specialization entry missing during Lean transport".into(),
            ));
        };
        for pos in 0..def_b.body.candidate.body.len() {
            let u_b = caller_goal.subject.named_uses.iter().skip(1).find(|u| {
                u.caller_definition == Some(def_b.definition) && u.source_node == Some(pos as u32)
            });
            let u_a = callee_goal.subject.named_uses.iter().skip(1).find(|u| {
                u.caller_definition == Some(def_a.definition) && u.source_node == Some(pos as u32)
            });
            if let (Some(ub), Some(ua)) = (u_b, u_a) {
                let bs = ub.definition.0;
                let as_ = ua.definition.0;
                if !slot_map.iter().any(|(x, _)| *x == bs) {
                    slot_map.push((bs, as_));
                    queue.push((bs, as_));
                }
            }
        }
    }
    let b_sub_slots = caller_callee_sub_slots(caller_goal, arg.occ_slot);
    let mut f_body = std::string::String::from("fun s => ");
    for &(bs, as_) in &slot_map {
        f_body.push_str(&std::format!("if s == {bs} then {as_} else "));
    }
    f_body.push('s');

    let hlookup = if b_sub_slots.is_empty() {
        std::string::String::from("(fun _ hmem => by cases hmem)")
    } else {
        let mut s = std::string::String::from("(by\n  intro _ hmem\n");
        let mut cur_hyp = std::string::String::from("hmem");
        for (idx, &b_slot) in b_sub_slots.iter().enumerate() {
            let Some(&(_, a_slot)) = slot_map.iter().find(|(b, _)| *b == b_slot) else {
                return Err(super::super::output::Failure::error(
                    "named-v3-slot-map",
                    "missing slot mapping for callee environment entry".into(),
                ));
            };
            let sim_entry = sim_code_proof(callee_goal, a_slot);
            let next_hyp = std::format!("htl_{idx}");
            s.push_str(&std::format!(
                "  cases {cur_hyp} with\n  | head => exact ⟨{s_mod}.entry_{a_slot}, rfl, rfl, rfl, rfl, {sim_entry}⟩\n  | tail _ {next_hyp} =>\n"
            ));
            cur_hyp = next_hyp;
        }
        s.push_str(&std::format!("  cases {cur_hyp})"));
        s
    };
    let sim_root = sim_code_proof(callee_goal, callee_root_slot);
    let occ_ui = arg.occ_ui;
    Ok(std::format!(
        "(let f_{arg_idx} : Nat → Nat := {f_body}\n fun tail before final hbefore hpre hexec =>\n   {p_mod}.proof tail before final hbefore hpre\n     (runSim f_{arg_idx} {caller_spec_mod}.calleeSubject_{occ_ui}.env {s_mod}.subject.env\n       {caller_spec_mod}.calleeBodies_{occ_ui}\n       {hlookup}\n       {caller_spec_mod}.calleeProfile_{occ_ui}\n       {sim_root}\n       hexec))"
    ))
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
    let (obligation, module_name, version, source, preceding_locals) = match work.node {
        DepNodeKind::Dependency(dep) => (
            &dep.declaration,
            dep.module.as_str(),
            dep.version,
            dep.source.as_slice(),
            &[][..],
        ),
        DepNodeKind::Local(obl) => (
            obl,
            caller.batch.module.as_str(),
            caller.batch.version,
            caller.batch.source.as_slice(),
            work.local_scope,
        ),
    };
    let checked = attempt!(recheck_obligation(
        module_name,
        version,
        source,
        obligation,
        preceding_locals,
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
                goal: None,
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
                goal: None,
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
            let mut lemma_indices: std::vec::Vec<usize> = std::vec::Vec::new();
            for lemma_ref in &env_v3.envelope.lemmas {
                let child = attempt!(resolve_dep_ref(lemma_ref, work.sub_deps, work.local_scope));
                lemma_indices.push(attempt!(compile_dep_recursive(
                    child, caller, session, state
                )));
            }
            let mut callee_args: std::vec::Vec<CalleeArg> = std::vec::Vec::new();
            for occ in &env_v3.envelope.occurrences {
                let child = attempt!(resolve_dep_ref(&occ.proof, work.sub_deps, work.local_scope));
                let dep_idx = attempt!(compile_dep_recursive(child, caller, session, state));
                let occ_ui = contract
                    .subject
                    .named_uses
                    .iter()
                    .enumerate()
                    .skip(1)
                    .find(|(_, u)| u.definition.0 == occ.specialization)
                    .map(|(i, _)| i)
                    .ok_or_else(|| {
                        super::super::output::Failure::error(
                            "named-v3-occurrence-missing",
                            "callee occurrence missing from named_uses".into(),
                        )
                    })?;
                callee_args.push(CalleeArg {
                    dep_idx,
                    occ_slot: occ.specialization,
                    occ_ui,
                });
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
            let mut imported_mods: std::vec::Vec<usize> = std::vec::Vec::new();
            for &c_idx in &lemma_indices {
                if !imported_mods.contains(&c_idx) {
                    imported_mods.push(c_idx);
                }
            }
            for arg in &callee_args {
                if !imported_mods.contains(&arg.dep_idx) {
                    imported_mods.push(arg.dep_idx);
                }
            }
            let mut imports = std::format!("import {spec_mod}\n");
            for &c_idx in &imported_mods {
                let s_mod = &state.compiled[c_idx].spec_module;
                let p_mod = &state.compiled[c_idx].proof_module;
                imports.push_str(&std::format!("import {s_mod}\nimport {p_mod}\n"));
            }
            let has_run_sim = callee_args
                .iter()
                .any(|arg| is_nonleaf_rev3(state.compiled[arg.dep_idx].goal.as_deref()));
            let mut term_args = std::string::String::new();
            let rewritten_term = checked
                .lean_term
                .replace("NamedV3Obligation.", &std::format!("{spec_mod}."));
            if lemma_indices.is_empty() && callee_args.is_empty() {
                term_args.push_str(&rewritten_term);
            } else {
                if has_run_sim {
                    term_args.push_str("(\n");
                    term_args.push_str(RUN_SIM_PRELUDE);
                }
                term_args.push('(');
                term_args.push_str(&rewritten_term);
                term_args.push(')');
                for &c_idx in &lemma_indices {
                    let p_mod = &state.compiled[c_idx].proof_module;
                    term_args.push_str(&std::format!(" {p_mod}.proof"));
                }
                for (arg_idx, arg) in callee_args.iter().enumerate() {
                    let arg_term = attempt!(emit_callee_arg(
                        contract,
                        &spec_mod,
                        arg,
                        arg_idx,
                        &state.compiled,
                    ));
                    term_args.push(' ');
                    term_args.push_str(&arg_term);
                }
                if has_run_sim {
                    term_args.push(')');
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
                goal: Some(contract.clone()),
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

    let caller_pos = caller
        .batch
        .obligations
        .iter()
        .position(|o| o.name == caller.obligation.name);
    let caller_local = caller_pos.map_or(caller.batch.obligations.as_slice(), |pos| {
        &caller.batch.obligations[..pos]
    });

    let env_v3 = noble_contracts::intrinsic::prepare_named_v3(
        caller.contract,
        &caller.batch.dependencies,
        caller_local,
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
    let mut lemma_indices: std::vec::Vec<usize> = std::vec::Vec::new();
    for lemma_ref in &env_v3.envelope.lemmas {
        let dep = attempt!(resolve_dep_ref(
            lemma_ref,
            &caller.batch.dependencies,
            caller_local
        ));
        lemma_indices.push(attempt!(compile_dep_recursive(
            dep,
            caller,
            &session,
            &mut dep_state
        )));
    }
    let mut callee_args: std::vec::Vec<CalleeArg> = std::vec::Vec::new();
    for occ in &env_v3.envelope.occurrences {
        let dep = attempt!(resolve_dep_ref(
            &occ.proof,
            &caller.batch.dependencies,
            caller_local
        ));
        let dep_idx = attempt!(compile_dep_recursive(dep, caller, &session, &mut dep_state));
        let occ_ui = caller
            .contract
            .subject
            .named_uses
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, u)| u.definition.0 == occ.specialization)
            .map(|(i, _)| i)
            .ok_or_else(|| {
                super::super::output::Failure::error(
                    "named-v3-occurrence-missing",
                    "callee occurrence missing from named_uses".into(),
                )
            })?;
        callee_args.push(CalleeArg {
            dep_idx,
            occ_slot: occ.specialization,
            occ_ui,
        });
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
    let has_run_sim = callee_args
        .iter()
        .any(|arg| is_nonleaf_rev3(dep_state.compiled[arg.dep_idx].goal.as_deref()));
    let mut applied_term = std::string::String::new();
    if lemma_indices.is_empty() && callee_args.is_empty() {
        applied_term.push_str(&caller.proof.lean_term);
    } else {
        if has_run_sim {
            applied_term.push_str("(\n");
            applied_term.push_str(RUN_SIM_PRELUDE);
        }
        applied_term.push('(');
        applied_term.push_str(&caller.proof.lean_term);
        applied_term.push(')');
        for &d_idx in &lemma_indices {
            let p_mod = &dep_state.compiled[d_idx].proof_module;
            applied_term.push_str(&std::format!(" {p_mod}.proof"));
        }
        for (arg_idx, arg) in callee_args.iter().enumerate() {
            let arg_term = attempt!(emit_callee_arg(
                caller.contract,
                "NamedV3Obligation",
                arg,
                arg_idx,
                &dep_state.compiled,
            ));
            applied_term.push(' ');
            applied_term.push_str(&arg_term);
        }
        if has_run_sim {
            applied_term.push(')');
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
