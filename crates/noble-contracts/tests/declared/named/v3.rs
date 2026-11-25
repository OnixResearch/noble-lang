use super::LIMITS;

fn new_session() -> noble_contracts::source::ModuleSession {
    noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(131_072).unwrap(),
    )
    .unwrap()
}

fn register_and_check(
    session: noble_contracts::source::ModuleSession,
    source: &[u8],
) -> (
    noble_contracts::source::ModuleSession,
    Vec<noble_contracts::intrinsic::CheckedProof>,
) {
    let prepared = session
        .prepare(source, &[], LIMITS)
        .expect("prepare rev-3 module");
    let batch = prepared.proof_obligations().cloned();
    let mut checked_out = Vec::new();
    let (next, outcome) = if batch.is_some() {
        let mut captured = Vec::new();
        let res = session.commit_verified(prepared, |b| {
            let mut checked =
                noble_contracts::intrinsic::check_batch(b, LIMITS).map_err(|e| format!("{e:?}"))?;
            let zero_rev = "0".repeat(64);
            for item in &mut checked {
                item.model_revision = zero_rev.clone();
                item.checker_revision = zero_rev.clone();
            }
            captured = checked.clone();
            Ok(checked)
        });
        checked_out = captured;
        res
    } else {
        session.commit(prepared)
    };
    outcome.expect("commit rev-3 module");
    (next, checked_out)
}

fn commit_ok(
    session: noble_contracts::source::ModuleSession,
    source: &[u8],
) -> noble_contracts::source::ModuleSession {
    let prepared = session.prepare(source, &[], LIMITS).expect("prepare");
    let (next, outcome) = session.commit(prepared);
    outcome.expect("commit");
    next
}

#[test]
fn rev3_grammar_and_cross_revision_controls() {
    let mut session = new_session();
    session = register_and_check(
        session,
        b"module g@1 [ export inc def inc [ 1 + ] export dec def dec [ 1 - ] export dec-ok contract 3 dec-law [ subject dec input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (sub (in x) 1)) ] ] proof 3 dec-ok for dec-law [ (export-named-claim (join (refl (sub (in x) 1)))) ] ]",
    )
    .0;
    session = commit_ok(session, b"import g@1 as g");

    // Cross-revision contract 2 + proof 3 refused.
    let err = session
        .prepare(
            b"module m1@1 [ def f [ g.inc g.inc ] contract 2 c [ subject f input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ] proof 3 p for c [ (export-named-claim (pc-call 0 g.dec-ok (by-true)) (join (use f1))) ] ]",
            &[],
            LIMITS,
        )
        .expect_err("contract 2 + proof 3 must refuse");
    assert!(
        format!("{err:?}").contains("proof and contract revisions differ"),
        "unexpected error: {err:?}"
    );

    // Colon syntax on proof 3 refused.
    let err = session
        .prepare(
            b"module m2@1 [ def f [ g.dec ] contract 3 c [ subject f input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (sub (in x) 1)) ] ] proof 3 p : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] ]",
            &[],
            LIMITS,
        )
        .expect_err("colon syntax on proof 3 must refuse");
    assert!(
        format!("{err:?}").contains("proof revision 3 requires a contract claim"),
        "unexpected error: {err:?}"
    );

    // Cross-revision contract 3 + proof 2 refused.
    let err = session
        .prepare(
            b"module m3@1 [ def f [ g.dec ] contract 3 c [ subject f input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (sub (in x) 1)) ] ] proof 2 p for c [ (export-named-claim (join (by-true))) ] ]",
            &[],
            LIMITS,
        )
        .expect_err("contract 3 + proof 2 must refuse");
    assert!(
        format!("{err:?}").contains("proof and contract revisions differ"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn rev3_preuse_s01_s02_s03_s04_and_erasure() {
    let mut session = new_session();
    // PREUSE-S01: rev-1 callee inc + rev-3 callee dec (`guard@1.dec` uses contract 3/proof 3).
    session = register_and_check(
        session,
        b"module incmod@1 [ export inc export inc-ok def inc [ 1 + ] contract 1 inc-law [ subject inc input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 1 inc-ok for inc-law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]",
    )
    .0;
    session = commit_ok(session, b"import incmod@1 as m");

    session = register_and_check(
        session,
        b"module guard@1 [ export dec export dec-ok def dec [ 1 - ] contract 3 dec-law [ subject dec input [ x I64 ] output [ y I64 ] requires [ (lt 0 (in x)) ] ensures [ (eq (out y) (sub (in x) 1)) ] ] proof 3 dec-ok for dec-law [ (export-named-claim (join (refl (sub (in x) 1)))) ] ]",
    )
    .0;
    session = commit_ok(session, b"import guard@1 as g");

    // S01: `[ m.inc g.dec ]`
    let (session_s01, checked_s01) = register_and_check(
        session,
        b"module app_s01@1 [ export round export round-ok def round [ m.inc g.dec ] contract 3 round-law [ subject round input [ x I64 ] output [ y I64 ] requires [ (lt 0 (add (in x) 1)) ] ensures [ (eq (out y) (sub (add (in x) 1) 1)) ] ] proof 3 round-ok for round-law [ (export-named-claim (pc-call 0 m.inc-ok (by-true)) (pc-call 1 g.dec-ok (use f0)) (join (refl (sub (add (in x) 1) 1)))) ] ]",
    );
    assert_eq!(checked_s01.len(), 1);
    assert!(!checked_s01[0].lean_term.contains("sorry"));

    // PREUSE-S02: pure lemma module `arith@1` absent from `source_dependencies` + `subst` + `bv-add-assoc`.
    let session = register_and_check(
        session_s01,
        b"module arith@1 [ export add-assoc-wrap proof 1 add-assoc-wrap : [ (Pi (a I64) (Pi (b I64) (Pi (c I64) (Eq I64 (add (add a b) c) (add a (add b c)))))) ] [ (intro (a I64) (intro (b I64) (intro (c I64) (bv-add-assoc a b c)))) ] ]",
    )
    .0;
    let session = commit_ok(session, b"import arith@1 as ar");

    let (session, checked_s02) = register_and_check(
        session,
        b"module app_s02@1 [ export plus2 def plus2 [ m.inc m.inc ] contract 3 plus2-law [ subject plus2 input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) (add 1 1))) ] ] proof 3 plus2-ok for plus2-law [ (export-named-claim (pc-call 0 m.inc-ok (by-true)) (pc-call 1 m.inc-ok (by-true)) (join (subst (motive (v I64) (eq (add (add (in x) 1) 1) v)) (apply (apply (apply (use ar.add-assoc-wrap) (in x)) 1) 1) (refl (add (add (in x) 1) 1))))) ] ]",
    );
    assert_eq!(checked_s02.len(), 1);
    assert!(checked_s02[0].lean_term.contains("BitVec"));

    // PREUSE-S06: rebinding `ar` after `app_s02@1` committed does NOT break `app_s02@1`!
    let session = register_and_check(
        session,
        b"module arith@2 [ export other proof 1 other : [ (Pi (a I64) (Eq I64 a a)) ] [ (intro (a I64) (refl a)) ] ]",
    )
    .0;
    let session = commit_ok(session, b"import arith@2 as ar");

    // PREUSE-S04: `[ swap d.sub2 ]` (non-zero `source_node` for first call!)
    let session = register_and_check(
        session,
        b"module diff@1 [ export sub2 export sub2-ok def sub2 [ - ] contract 3 sub2-law [ subject sub2 input [ a I64 b I64 ] output [ d I64 ] requires [ true ] ensures [ (eq (out d) (sub (in a) (in b))) ] ] proof 3 sub2-ok for sub2-law [ (export-named-claim (join (refl (sub (in a) (in b))))) ] ]",
    )
    .0;
    let session = commit_ok(session, b"import diff@1 as d");

    let (session, checked_s04) = register_and_check(
        session,
        b"module app_s04@1 [ export revsub def revsub [ swap d.sub2 ] contract 3 revsub-law [ subject revsub input [ x I64 y I64 ] output [ z I64 ] requires [ true ] ensures [ (eq (out z) (sub (in y) (in x))) ] ] proof 3 revsub-ok for revsub-law [ (export-named-claim (pc-call 0 d.sub2-ok (by-true)) (join (refl (sub (in y) (in x))))) ] ]",
    );
    assert_eq!(checked_s04.len(), 1);
    assert!(checked_s04[0].lean_term.contains("liftedChainClaim"));

    // PREUSE-S03: nested rev-3 -> rev-3 -> leaf + PREUSE-S18 proof erasure check!
    let session = commit_ok(session, b"import app_s01@1 as s01");
    let (_session_nested, checked_nested) = register_and_check(
        session,
        b"module app_nested@1 [ export top def top [ s01.round m.inc ] contract 3 top-law [ subject top input [ x I64 ] output [ y I64 ] requires [ (lt 0 (add (in x) 1)) ] ensures [ (eq (out y) (add (sub (add (in x) 1) 1) 1)) ] ] proof 3 top-ok for top-law [ (export-named-claim (pc-call 0 s01.round-ok (use f0)) (pc-call 1 m.inc-ok (by-true)) (join (refl (add (sub (add (in x) 1) 1) 1)))) ] ]",
    );
    assert_eq!(checked_nested.len(), 1);

    // Proof erasure parity (with vs without contract 3 / proof 3):
    let s_plain = commit_ok(
        new_session(),
        b"module diff@1 [ export sub2 def sub2 [ - ] ]",
    );
    let plain_typed = s_plain
        .prepare(
            b"diff@1.sub2",
            &[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64],
            LIMITS,
        )
        .unwrap();
    let s_erased = register_and_check(
        new_session(),
        b"module diff@1 [ export sub2 def sub2 [ - ] contract 3 sub2-law [ subject sub2 input [ a I64 b I64 ] output [ d I64 ] requires [ true ] ensures [ (eq (out d) (sub (in a) (in b))) ] ] proof 3 sub2-ok for sub2-law [ (export-named-claim (join (refl (sub (in a) (in b))))) ] ]",
    )
    .0;
    let verified_typed = s_erased
        .prepare(
            b"diff@1.sub2",
            &[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64],
            LIMITS,
        )
        .unwrap();
    assert_eq!(plain_typed.output(), verified_typed.output());
    assert_eq!(
        format!("{:?}", plain_typed.submission().map(|s| &s.body)),
        format!("{:?}", verified_typed.submission().map(|s| &s.body))
    );
    assert_eq!(
        format!("{:?}", plain_typed.submission().map(|s| &s.definitions)),
        format!("{:?}", verified_typed.submission().map(|s| &s.definitions))
    );
}
