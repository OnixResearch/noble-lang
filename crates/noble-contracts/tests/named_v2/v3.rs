fn rev3_fixture(
    modules: &[(&[u8], &[u8])],
    subject_src: &[u8],
) -> (
    noble_contracts::intrinsic::ContractGoal,
    noble_contracts::intrinsic::ProofBatch,
) {
    assert!(!subject_src.is_empty());
    assert!(modules.len() <= 16);
    let mut session = noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(131_072).unwrap(),
    )
    .unwrap();
    for (mod_src, imp_src) in modules {
        if !mod_src.is_empty() {
            let prep = session.prepare(mod_src, &[], super::LIMITS).unwrap();
            let has_pending = prep.proof_obligations().is_some();
            let (next, outcome) = if has_pending {
                session.commit_verified(prep, |b| {
                    let mut checked = noble_contracts::intrinsic::check_batch(b, super::LIMITS)
                        .map_err(|e| format!("{e:?}"))?;
                    let zero_rev = "0".repeat(64);
                    for item in &mut checked {
                        item.model_revision = zero_rev.clone();
                        item.checker_revision = zero_rev.clone();
                    }
                    Ok(checked)
                })
            } else {
                session.commit(prep)
            };
            outcome.unwrap();
            session = next;
        }
        if !imp_src.is_empty() {
            let imp = session.prepare(imp_src, &[], super::LIMITS).unwrap();
            let (next, outcome) = session.commit(imp);
            outcome.unwrap();
            session = next;
        }
    }
    let prep = session.prepare(subject_src, &[], super::LIMITS).unwrap();
    let batch = prep.proof_obligations().cloned().unwrap();
    let goal = prep.contract_goals().next().unwrap().clone();
    (goal, batch)
}

#[test]
fn rev3_envelope_and_tamper_refusals() {
    let (goal, batch) = rev3_fixture(
        &[(
            b"module d@1 [ export step export step-ok def step [ 1 + ] contract 3 step-law [ subject step input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 3 step-ok for step-law [ (export-named-claim (join (refl (add (in x) 1)))) ] ]",
            b"import d@1 as d",
        )],
        b"module caller@1 [ export twice def twice [ d.step d.step ] contract 3 twice-law [ subject twice input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (add (in x) 1) 1)) ] ] proof 3 twice-ok for twice-law [ (export-named-claim (pc-call 0 d.step-ok (by-true)) (pc-call 1 d.step-ok (by-true)) (join (refl (add (add (in x) 1) 1)))) ] ]",
    );
    let prep = noble_contracts::intrinsic::prepare_named_v3(
        &goal,
        &batch.dependencies,
        &batch.obligations,
        &batch.obligations[0].term,
        super::LIMITS,
    )
    .expect("valid twice envelope");
    assert_eq!(prep.envelope.occurrences.len(), 2);
    assert_eq!(prep.envelope.occurrences[0].ordinal, 0);
    assert_eq!(prep.envelope.occurrences[1].ordinal, 1);

    // Lowered Lean obligation & closed LiftedChain term succeed cleanly.
    let obl = noble_contracts::intrinsic::prepare_named_v3_contract(&goal, super::LIMITS).unwrap();
    assert!(obl.contains("namespace NamedV3Obligation"));
    assert!(obl.contains("exportedLiftedClaim subject"));
    let checked = noble_contracts::intrinsic::check_batch(&batch, super::LIMITS).unwrap();
    assert_eq!(checked.len(), 1);
    assert!(checked[0].lean_term.contains("liftedChainClaim"));
    assert!(!checked[0].lean_term.contains("sorry"));

    // Tamper 1: swap source spans of occurrences 0 and 1 -> refused.
    let mut bad_span = goal.clone();
    let s0 = bad_span.subject.named_uses[1].source_span;
    bad_span.subject.named_uses[1].source_span = bad_span.subject.named_uses[2].source_span;
    bad_span.subject.named_uses[2].source_span = s0;
    assert!(noble_contracts::intrinsic::claim_shape(&bad_span, super::LIMITS).is_err());

    // Tamper 2: duplicate specialization slot -> refused.
    let mut bad_slot = goal.clone();
    bad_slot.subject.named_uses[2].definition = bad_slot.subject.named_uses[1].definition;
    assert!(noble_contracts::intrinsic::claim_shape(&bad_slot, super::LIMITS).is_err());

    // Tamper 3: inject orphan definition -> refused.
    let mut orphan = goal.clone();
    let mut extra = orphan.subject.accepted_submission.definitions[0].clone();
    extra.definition = noble_kernel::contracts::Definition(999);
    orphan.subject.accepted_submission.definitions.push(extra);
    assert!(noble_contracts::intrinsic::claim_shape(&orphan, super::LIMITS).is_err());
}

#[test]
fn rev3_symbolic_rules_bv_subst_eqterm_and_negatives() {
    let (_, batch_comm_mul) = rev3_fixture(
        &[(
            b"module d@1 [ export mul2 export mul2-ok def mul2 [ 2 * ] contract 3 mul2-law [ subject mul2 input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (mul (in x) 2)) ] ] proof 3 mul2-ok for mul2-law [ (export-named-claim (join (refl (mul (in x) 2)))) ] ]",
            b"import d@1 as d",
        )],
        b"module comm@1 [ export f def f [ + d.mul2 ] contract 3 f-law [ subject f input [ x I64 y I64 ] output [ z I64 ] requires [ true ] ensures [ (eq (out z) (mul (add (in y) (in x)) 2)) ] ] proof 3 f-ok for f-law [ (export-named-claim (pc-call 0 d.mul2-ok (by-true)) (join (subst (motive (v I64) (eq (mul (add (in x) (in y)) 2) (mul v 2))) (bv-add-comm (in x) (in y)) (refl (mul (add (in x) (in y)) 2))))) ] ]",
    );
    let checked = noble_contracts::intrinsic::check_batch(&batch_comm_mul, super::LIMITS).unwrap();
    assert!(checked[0].lean_term.contains("BitVec.add_comm"));
    assert!(checked[0].lean_term.contains("@Eq.subst"));

    // Negative control: bridge unproved -> `preuse-bridge-unproved`
    let (_, batch_bad_bridge) = rev3_fixture(
        &[(
            b"module g@1 [ export dec export dec-ok def dec [ 1 - ] contract 3 dec-law [ subject dec input [ x I64 ] output [ y I64 ] requires [ (lt 0 (in x)) ] ensures [ (eq (out y) (sub (in x) 1)) ] ] proof 3 dec-ok for dec-law [ (export-named-claim (join (refl (sub (in x) 1)))) ] ]",
            b"import g@1 as g",
        )],
        b"module bad@1 [ export f def f [ g.dec ] contract 3 f-law [ subject f input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (sub (in x) 1)) ] ] proof 3 f-ok for f-law [ (export-named-claim (pc-call 0 g.dec-ok (by-true)) (join (refl (sub (in x) 1)))) ] ]",
    );
    let err = noble_contracts::intrinsic::check_batch(&batch_bad_bridge, super::LIMITS)
        .expect_err("unproved bridge must fail");
    assert!(
        err.message.contains("preuse-bridge-unproved"),
        "expected preuse-bridge-unproved, got: {}",
        err.message
    );

    // `bv-mul-add` + `(Eq I64 ...)` Pure lemma + negative swapped `subst` argument order.
    let (_, batch_distrib_ok) = rev3_fixture(
        &[
            (
                b"module arith3@1 [ export distrib proof 1 distrib : [ (Pi (a I64) (Pi (b I64) (Pi (c I64) (Eq I64 (mul a (add b c)) (add (mul a b) (mul a c)))))) ] [ (intro (a I64) (intro (b I64) (intro (c I64) (bv-mul-add a b c)))) ] ]",
                b"import arith3@1 as a3",
            ),
            (
                b"module d@1 [ export id export id-ok def id [ 0 + ] contract 3 id-law [ subject id input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 0)) ] ] proof 3 id-ok for id-law [ (export-named-claim (join (refl (add (in x) 0)))) ] ]",
                b"import d@1 as d",
            ),
        ],
        b"module muladd3@1 [ export f def f [ 1 + * d.id ] contract 3 f-law [ subject f input [ x I64 y I64 ] output [ z I64 ] requires [ true ] ensures [ (eq (out z) (add (add (mul (in x) (in y)) (mul (in x) 1)) 0)) ] ] proof 3 f-ok for f-law [ (export-named-claim (pc-call 0 d.id-ok (by-true)) (join (subst (motive (v I64) (eq (add (mul (in x) (add (in y) 1)) 0) (add v 0))) (apply (apply (apply (use a3.distrib) (in x)) (in y)) 1) (refl (add (mul (in x) (add (in y) 1)) 0))))) ] ]",
    );
    let checked_ma =
        noble_contracts::intrinsic::check_batch(&batch_distrib_ok, super::LIMITS).unwrap();
    assert!(checked_ma[0].lean_term.contains("lemma_0"));
    assert!(!checked_ma[0].lean_term.contains("sorry"));

    let (_, batch_direct_mul_add) = rev3_fixture(
        &[(
            b"module d@1 [ export id export id-ok def id [ 0 + ] contract 3 id-law [ subject id input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 0)) ] ] proof 3 id-ok for id-law [ (export-named-claim (join (refl (add (in x) 0)))) ] ]",
            b"import d@1 as d",
        )],
        b"module muladd4@1 [ export f def f [ 1 + * d.id ] contract 3 f-law [ subject f input [ x I64 y I64 ] output [ z I64 ] requires [ true ] ensures [ (eq (out z) (add (add (mul (in x) (in y)) (mul (in x) 1)) 0)) ] ] proof 3 f-ok for f-law [ (export-named-claim (pc-call 0 d.id-ok (by-true)) (join (subst (motive (v I64) (eq (add (mul (in x) (add (in y) 1)) 0) (add v 0))) (bv-mul-add (in x) (in y) 1) (refl (add (mul (in x) (add (in y) 1)) 0))))) ] ]",
    );
    let checked_direct =
        noble_contracts::intrinsic::check_batch(&batch_direct_mul_add, super::LIMITS).unwrap();
    assert!(checked_direct[0].lean_term.contains("BitVec.mul_add"));

    // Negative control: swapped subst args (premise first, equality second) must fail!
    let (_, batch_swapped) = rev3_fixture(
        &[(
            b"module d@1 [ export id export id-ok def id [ 0 + ] contract 3 id-law [ subject id input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 0)) ] ] proof 3 id-ok for id-law [ (export-named-claim (join (refl (add (in x) 0)))) ] ]",
            b"import d@1 as d",
        )],
        b"module badsubst@1 [ export f def f [ + d.id ] contract 3 f-law [ subject f input [ x I64 y I64 ] output [ z I64 ] requires [ true ] ensures [ (eq (out z) (add (add (in y) (in x)) 0)) ] ] proof 3 f-ok for f-law [ (export-named-claim (pc-call 0 d.id-ok (by-true)) (join (subst (motive (v I64) (eq (add (add (in x) (in y)) 0) (add v 0))) (refl (add (add (in x) (in y)) 0)) (bv-add-comm (in x) (in y))))) ] ]",
    );
    assert!(noble_contracts::intrinsic::check_batch(&batch_swapped, super::LIMITS).is_err());
}

#[test]
fn rev3_bool_eq_binder_shadowing_folding_and_fact_bounds() {
    // 1. `(Eq Bool a true)` accepted in Pure lemma; `(Eq I64 a 1)` sort mismatch rejected.
    let (_, batch_bool_eq) = rev3_fixture(
        &[(
            b"module be@1 [ export beq proof 1 beq : [ (Pi (a Bool) (Eq Bool a a)) ] [ (intro (a Bool) (refl a)) ] ]",
            b"import be@1 as be",
        )],
        b"module usebe@1 [ export f def f [ dup drop ] contract 3 f-law [ subject f input [ x Bool ] output [ y Bool ] requires [ true ] ensures [ (eq (out y) (in x)) ] ] proof 3 f-ok for f-law [ (export-named-claim (join (subst (motive (v Bool) (eq (in x) v)) (apply (use be.beq) (in x)) (refl (in x))))) ] ]",
    );
    let checked = noble_contracts::intrinsic::check_batch(&batch_bool_eq, super::LIMITS).unwrap();
    assert!(!checked[0].lean_term.contains("sorry"));

    let bad_sess = noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(131_072).unwrap(),
    )
    .unwrap();
    let bad_prep = bad_sess
        .prepare(
            b"module badbe@1 [ export bad proof 1 bad : [ (Pi (a Bool) (Eq I64 a 1)) ] [ (intro (a Bool) (refl a)) ] ]",
            &[],
            super::LIMITS,
        )
        .unwrap();
    let bad_batch = bad_prep.proof_obligations().cloned().unwrap();
    assert!(noble_contracts::intrinsic::check_batch(&bad_batch, super::LIMITS).is_err());

    // 2. Motive binder shadowing `(in x)` vs bound `x`, plus constant folding `(= (add 2 3) 5)` & `(use f0)`.
    let (_, batch_shadow_fold) = rev3_fixture(
        &[(
            b"module d@1 [ export id export id-ok def id [ 0 + ] contract 3 id-law [ subject id input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 0)) ] ] proof 3 id-ok for id-law [ (export-named-claim (join (refl (add (in x) 0)))) ] ]",
            b"import d@1 as d",
        )],
        b"module sh@1 [ export f def f [ d.id ] contract 3 f-law [ subject f input [ x I64 ] output [ y I64 ] requires [ (eq (add 2 3) 5) ] ensures [ (eq (out y) (add (in x) 0)) ] ] proof 3 f-ok for f-law [ (export-named-claim (pc-call 0 d.id-ok (use f0)) (join (subst (motive (x I64) (eq (add (in x) 0) (add x 0))) (refl (in x)) (refl (add (in x) 0))))) ] ]",
    );
    let checked_sh =
        noble_contracts::intrinsic::check_batch(&batch_shadow_fold, super::LIMITS).unwrap();
    assert!(checked_sh[0].lean_term.contains("@Eq.subst"));

    // 3. Out-of-bounds fact references (`f1000`, `g0`) rejected.
    for bad_use in [
        &b"module bf1@1 [ export f def f [ 1 ] contract 3 f-law [ subject f input [ ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) 1) ] ] proof 3 f-ok for f-law [ (export-named-claim (join (use f1000))) ] ]"[..],
        &b"module bf2@1 [ export f def f [ 1 ] contract 3 f-law [ subject f input [ ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) 1) ] ] proof 3 f-ok for f-law [ (export-named-claim (join (use g0))) ] ]"[..],
    ] {
        let sess = noble_contracts::source::ModuleSession::new(
            &[],
            noble_contracts::source::RetainedSourceLimit::new(131_072).unwrap(),
        )
        .unwrap();
        let res = sess.prepare(bad_use, &[], super::LIMITS);
        assert!(res.is_err());
    }
}

#[test]
fn rev3_nested_and_folded_conjunctions_and_twice_rev3() {
    // 1. Repeated rev-3 callee `twice [ g.dec g.dec ]` emits per-occurrence `callee_0` and `callee_1`.
    let (_, batch_twice) = rev3_fixture(
        &[(
            b"module g@1 [ export dec export dec-ok def dec [ 1 - ] contract 3 dec-law [ subject dec input [ x I64 ] output [ y I64 ] requires [ (lt 0 (in x)) ] ensures [ (eq (out y) (sub (in x) 1)) ] ] proof 3 dec-ok for dec-law [ (export-named-claim (join (refl (sub (in x) 1)))) ] ]",
            b"import g@1 as g",
        )],
        b"module tw@1 [ export twice def twice [ g.dec g.dec ] contract 3 twice-law [ subject twice input [ x I64 ] output [ y I64 ] requires [ (and (lt 0 (in x)) (lt 0 (sub (in x) 1))) ] ensures [ (eq (out y) (sub (sub (in x) 1) 1)) ] ] proof 3 twice-ok for twice-law [ (export-named-claim (pc-call 0 g.dec-ok (use f0)) (pc-call 1 g.dec-ok (use f1)) (join (refl (sub (sub (in x) 1) 1)))) ] ]",
    );
    let checked_twice =
        noble_contracts::intrinsic::check_batch(&batch_twice, super::LIMITS).unwrap();
    assert!(checked_twice[0].lean_term.contains("callee_0"));
    assert!(checked_twice[0].lean_term.contains("callee_1"));

    // 2. Left-nested `(and (and ...) ...)` + constant-folded `requires` / `ensures` / `CombineAnd` under `Not`.
    let (_, batch_left_and) = rev3_fixture(
        &[(
            b"module d@1 [ export id export id-ok def id [ 0 + ] contract 3 id-law [ subject id input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 0)) ] ] proof 3 id-ok for id-law [ (export-named-claim (join (refl (add (in x) 0)))) ] ]",
            b"import d@1 as d",
        )],
        b"module land@1 [ export f def f [ d.id ] contract 3 f-law [ subject f input [ x I64 ] output [ y I64 ] requires [ (and (and (eq (in x) (in x)) (eq (add 2 3) 5)) (not (and (eq 1 2) (eq (in x) (in x))))) ] ensures [ (eq 1 1) ] ] proof 3 f-ok for f-law [ (export-named-claim (pc-call 0 d.id-ok (use f1)) (join (by-true))) ] ]",
    );
    let checked_land =
        noble_contracts::intrinsic::check_batch(&batch_left_and, super::LIMITS).unwrap();
    assert!(checked_land[0].lean_term.contains("rfact_0"));
    assert!(checked_land[0].lean_term.contains("True.intro"));

    // 3. Left-nested + constant-folded conjuncts in callee `ensures`.
    let (_, batch_callee_and) = rev3_fixture(
        &[(
            b"module ce@1 [ export inc export inc-ok def inc [ 1 + ] contract 3 inc-law [ subject inc input [ x I64 ] output [ y I64 ] requires [ (lt 0 (in x)) ] ensures [ (and (and (eq (out y) (add (in x) 1)) (eq 1 1)) (eq 2 2)) ] ] proof 3 inc-ok for inc-law [ (export-named-claim (join (refl (add (in x) 1)))) ] ]",
            b"import ce@1 as ce",
        )],
        b"module usece@1 [ export f def f [ ce.inc ] contract 3 f-law [ subject f input [ x I64 ] output [ y I64 ] requires [ (lt 0 (in x)) ] ensures [ (eq 1 1) ] ] proof 3 f-ok for f-law [ (export-named-claim (pc-call 0 ce.inc-ok (use f0)) (join (use f3))) ] ]",
    );
    let checked_ce =
        noble_contracts::intrinsic::check_batch(&batch_callee_and, super::LIMITS).unwrap();
    assert!(checked_ce[0].lean_term.contains("cfact_rest_0"));
    assert!(checked_ce[0].lean_term.contains("cfact_2"));
}
