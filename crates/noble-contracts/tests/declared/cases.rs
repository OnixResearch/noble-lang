#[test]
fn all_exported_public_arms_support_exact_two_branch_match() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session,b"module ledger@1 [ variant Status Ready I64 public Failed Text public export Status export Status.Ready export Status.Failed ]")?;
    let accepted = session
        .prepare(
            b"5 ledger@1.Status.Ready [ drop 1 ] [ drop 0 ] ledger@1.Status.match",
            &[],
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(accepted.output(), &[noble_kernel::types::Ty::I64]);
    assert!(accepted.submission().is_some());
    assert!(session
        .prepare(
            b"5 ledger@1.Status.Ready [ drop 1 ] ledger@1.Status.match",
            &[],
            super::LIMITS
        )
        .is_err());
    assert!(session
        .prepare(
            b"5 ledger@1.Status.Ready [ drop true ] [ drop 0 ] ledger@1.Status.match",
            &[],
            super::LIMITS
        )
        .is_err());
    Ok(())
}

#[test]
fn result_source_definitions_instantiate_at_distinct_named_uses() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    let integers = session
        .resolve_type("result@1.Result<I64,Text>")
        .map_err(|e| format!("{e:?}"))?;
    let texts = session
        .resolve_type("result@1.Result<Text,I64>")
        .map_err(|e| format!("{e:?}"))?;
    assert_ne!(
        integers, texts,
        "ordered type arguments distinguish family instances"
    );
    let incremented = session
        .prepare(
            b"[ 1 + ] result@1.map_ok",
            core::slice::from_ref(&integers),
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(incremented.output(), &[integers]);
    let retained = session
        .prepare(
            b"[ ] result@1.map_ok",
            core::slice::from_ref(&texts),
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(retained.output(), &[texts]);
    Ok(())
}

#[test]
fn result_library_source_is_pinned_and_invalid_callbacks_reject() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    let forged = b"module result@1 [ variant Result<A,E> Ok A public Err E public ]";
    let refused = session.prepare(forged, &[], super::LIMITS);
    assert!(matches!(
        refused,
        Err(error) if error.stage() == noble_contracts::source::Stage::Link
    ));
    let session = super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    let ty = session
        .resolve_type("result@1.Result<I64,Text>")
        .map_err(|e| format!("{e:?}"))?;
    assert!(session
        .prepare(
            b"[ + ] result@1.map_ok",
            core::slice::from_ref(&ty),
            super::LIMITS,
        )
        .is_err());
    assert!(session
        .prepare(b"[ drop drop ] result@1.map_ok", &[ty], super::LIMITS)
        .is_err());
    assert!(session
        .resolve_type("result@1.Result<Resource<test.counter>,Text>")
        .is_err());
    Ok(())
}

#[test]
fn signed_constructor_produces_concrete_result_for_checked_program() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    session = super::commit(
        session,
        b"module result_cases@1 [ signature make_ok forall<S:stack> [ S -- S result@1.Result<I64,Text> ! pure ] export make_ok def make_ok [ 2 result@1.Result.Ok ] ]",
    )?;
    let accepted = session
        .prepare(
            b"result_cases@1.make_ok [ 1 + ] result@1.map_ok",
            &[],
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        accepted.output(),
        &[session
            .resolve_type("result@1.Result<I64,Text>")
            .map_err(|e| format!("{e:?}"))?]
    );
    assert!(session
        .prepare(
            b"module bad_cases@1 [ signature make_ok forall<S:stack> [ S -- S result@1.Result<Bool,Text> ! pure ] def make_ok [ 2 result@1.Result.Ok ] ]",
            &[],
            super::LIMITS,
        )
        .is_err());
    Ok(())
}

#[test]
fn first_class_program_is_a_checked_result_payload() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    session = super::commit(
        session,
        b"module program_cases@1 [ signature make_program forall<S:stack> [ S -- S result@1.Result<Program<I64,I64,pure>,Text> ! pure ] export make_program def make_program [ [ 1 + ] result@1.Result.Ok ] ]",
    )?;
    let payload = session
        .resolve_type("result@1.Result<Program<I64,I64,pure>,Text>")
        .map_err(|e| format!("{e:?}"))?;
    let constructed = session
        .prepare(b"program_cases@1.make_program", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(constructed.output(), &[payload]);
    let mapped = session
        .prepare(
            b"program_cases@1.make_program [ 2 swap run ] result@1.map_ok",
            &[],
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        mapped.output(),
        &[session
            .resolve_type("result@1.Result<I64,Text>")
            .map_err(|e| format!("{e:?}"))?]
    );
    Ok(())
}

#[test]
fn reflected_syntax_is_a_checked_error_payload() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    session = super::commit(
        session,
        b"module syntax_cases@1 [ signature make_syntax forall<S:stack> [ S -- S result@1.Result<I64,Syntax> ! pure ] export make_syntax def make_syntax [ [ 1 + ] reflect result@1.Result.Err ] ]",
    )?;
    let selected = session
        .prepare(
            b"syntax_cases@1.make_syntax [ drop 11 ] [ drop 22 ] result@1.Result.match",
            &[],
            super::LIMITS,
        )
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(selected.output(), &[noble_kernel::types::Ty::I64]);
    Ok(())
}

#[test]
fn universal_source_signatures_reject_narrowed_value_and_effect() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    let narrowed_value = b"module bad@1 [ signature identity forall<S:stack,A:value> [ S A -- S A ! pure ] export identity def identity [ drop 0 ] ]";
    assert!(matches!(
        session.prepare(narrowed_value, &[], super::LIMITS),
        Err(error) if error.stage() == noble_contracts::source::Stage::Check
    ));
    let session = noble_contracts::source::ModuleSession::new(&[super::binding(super::Binding {
        version: 1,
        slot: 3,
        adapter: "version-A",
    })])
    .map_err(|e| format!("{e:?}"))?;
    let narrowed_effect = b"module ledger@1 [ require emit Text -- ! test.emit signature leak forall<S:stack,eps:effect> [ S -- S ! eps ] export leak def leak [ \"event\" emit ] ]";
    assert!(matches!(
        session.prepare(narrowed_effect, &[], super::LIMITS),
        Err(error) if error.stage() == noble_contracts::source::Stage::Check
    ));
    Ok(())
}

#[test]
fn public_constructor_cannot_expose_unexported_nominal_payload() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    let hidden=b"module ledger@1 [ opaque Secret I64 private opaque Wrapper Secret public export Wrapper export Wrapper.new ]";
    assert!(
        matches!(&session.prepare(hidden,&[],super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Link)
    );
    let hidden_arm=b"module ledger@1 [ opaque Secret I64 private variant Status Ready Secret public Failed Text public export Status export Status.Ready export Status.Failed ]";
    assert!(
        matches!(&session.prepare(hidden_arm,&[],super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Link)
    );
    Ok(())
}

#[test]
fn alias_rebinding_retains_each_checked_adapter_slot() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(&[
        super::binding(super::Binding {
            version: 1,
            slot: 3,
            adapter: "version-A",
        }),
        super::binding(super::Binding {
            version: 2,
            slot: 7,
            adapter: "version-B",
        }),
    ])
    .map_err(|e| format!("{e:?}"))?;
    session = super::commit(session,b"module ledger@1 [ require emit Text -- ! test.emit export handle def handle [ \"notice\" emit ] ]")?;
    session = super::commit(session,b"module ledger@2 [ require emit Text -- ! test.emit export handle def handle [ \"notice\" emit ] ]")?;
    session = super::commit(session, b"import ledger@1 as account")?;
    let retained = session
        .prepare(b"account.handle", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    session = super::commit(session, b"import ledger@2 as account")?;
    let rebound = session
        .prepare(b"account.handle", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let selected =
        |prepared: &noble_contracts::source::ModulePrepared| -> Result<Vec<u32>, String> {
            let submission = prepared.submission().ok_or("checked submission missing")?;
            Ok(submission
                .definitions
                .iter()
                .flat_map(|definition| &definition.body.candidate.nodes)
                .filter_map(|node| {
                    if let noble_kernel::untrusted::Node::Invocation { def, .. } = node {
                        if let Some(noble_kernel::contracts::Behavior::BoundEmit(slot)) =
                            submission.environment.kind(*def)
                        {
                            return Some(slot);
                        }
                    }
                    None
                })
                .collect::<Vec<u32>>())
        };
    assert_eq!(selected(&retained)?, vec![3]);
    assert_eq!(selected(&rebound)?, vec![7]);
    Ok(())
}

#[test]
fn largest_module_version_survives_direct_and_imported_resolution() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(
        session,
        b"module ledger@4294967295 [ export get def get [ 17 ] ]",
    )?;
    let direct = session
        .prepare(b"ledger@4294967295.get", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(direct.output(), &[noble_kernel::types::Ty::I64]);
    session = super::commit(session, b"import ledger@4294967295 as account")?;
    let imported = session
        .prepare(b"account.get", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(imported.output(), direct.output());
    Ok(())
}

#[test]
fn scoped_generic_types_resolve_forward_ordinals_without_cycles() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session,b"module ledger@1 [ opaque Composite Pair<Base,Sum<Bool,List<I64>>> private opaque Base I64 private export Composite ]")?;
    let composite = session
        .resolve_type("ledger@1.Composite")
        .map_err(|e| format!("{e:?}"))?;
    let prepared = session
        .prepare(b"", &[composite], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(
        prepared.kind(),
        noble_contracts::source::ModuleKind::Expression
    );
    assert!(prepared.submission().is_some());
    assert!(
        matches!(&session.prepare(b"module ledger@2 [ opaque Self Self private ]",&[],super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Resolve)
    );
    assert!(session
        .prepare(
            b"module ledger@2 [ opaque Bad Pair<I64, Bool> private ]",
            &[],
            super::LIMITS
        )
        .is_err());
    Ok(())
}

#[test]
fn adapter_selection_changes_linked_definition_not_module_schema() -> Result<(), String> {
    let source=b"module ledger@1 [ opaque Id I64 private export Id require emit Text -- ! test.emit export handle def handle [ \"event\" emit ] ]";
    let mut first =
        noble_contracts::source::ModuleSession::new(&[super::binding(super::Binding {
            version: 1,
            slot: 3,
            adapter: "version-A",
        })])
        .map_err(|e| format!("{e:?}"))?;
    let mut second =
        noble_contracts::source::ModuleSession::new(&[super::binding(super::Binding {
            version: 1,
            slot: 3,
            adapter: "version-B",
        })])
        .map_err(|e| format!("{e:?}"))?;
    first = super::commit(first, source)?;
    second = super::commit(second, source)?;
    let first_type = first
        .resolve_type("ledger@1.Id")
        .map_err(|e| format!("{e:?}"))?;
    let second_type = second
        .resolve_type("ledger@1.Id")
        .map_err(|e| format!("{e:?}"))?;
    match (first_type, second_type) {
        (noble_kernel::types::Ty::Nominal(_, left), noble_kernel::types::Ty::Nominal(_, right)) => {
            assert_eq!(left, right)
        }
        _ => return Err("missing checked nominal schemas".into()),
    }
    let first = first
        .prepare(b"ledger@1.handle", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let second = second
        .prepare(b"ledger@1.handle", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    let first = first.submission().ok_or("missing first linked program")?;
    let second = second.submission().ok_or("missing second linked program")?;
    assert_ne!(
        first
            .definitions
            .first()
            .ok_or("missing first checked body")?
            .identity,
        second
            .definitions
            .first()
            .ok_or("missing second checked body")?
            .identity
    );
    Ok(())
}

#[test]
fn local_definitions_resolve_forward_without_accepting_cycles() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(
        session,
        b"module ledger@1 [ export first def first [ second ] def second [ 2 ] ]",
    )?;
    let prepared = session
        .prepare(b"ledger@1.first", &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(prepared.output(), &[noble_kernel::types::Ty::I64]);
    let cycle = b"module ledger@2 [ export first def first [ second ] def second [ first ] ]";
    assert!(
        matches!(&session.prepare(cycle,&[],super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Resolve)
    );
    Ok(())
}

#[test]
fn aliases_do_not_silently_shadow_qualified_definitions() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::commit(session, b"def account.get [ 1 ]")?;
    session = super::commit(session, b"module ledger@1 [ export get def get [ 2 ] ]")?;
    assert!(
        matches!(&session.prepare(b"import ledger@1 as account",&[],super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Resolve)
    );
    let mut other =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    other = super::commit(other, b"module ledger@1 [ export get def get [ 2 ] ]")?;
    other = super::commit(other, b"import ledger@1 as account")?;
    assert!(
        matches!(&other.prepare(b"def account.get [ 1 ]",&[],super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Resolve)
    );
    Ok(())
}

#[test]
fn full_bounded_module_source_is_counted_once() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    let mut source = String::from("module ledger@1 [ export get def get [ 1 # ");
    let closing = "\n ] ]";
    let padding = usize::try_from(super::LIMITS.bytes)
        .map_err(|_| "source byte ceiling does not fit usize".to_string())?
        .checked_sub(source.len())
        .and_then(|remaining| remaining.checked_sub(closing.len()))
        .ok_or_else(|| {
            "module fixture prefix and closing exceed source byte ceiling".to_string()
        })?;
    source.extend(std::iter::repeat_n('x', padding));
    source.push_str(closing);
    assert_eq!(source.len(), 65_536);
    let prepared = session
        .prepare(source.as_bytes(), &[], super::LIMITS)
        .map_err(|e| format!("{e:?}"))?;
    assert_eq!(prepared.kind(), noble_contracts::source::ModuleKind::Module);
    source.push(' ');
    assert!(
        matches!(&session.prepare(source.as_bytes(),&[],super::LIMITS),Err(e) if e.stage()==noble_contracts::source::Stage::Parse)
    );
    Ok(())
}

#[test]
fn exact_intrinsic_source_proofs_stage_without_ordinary_publication() -> Result<(), String> {
    let logic = b"module logic@1 [
  proof 1 polymorphic-reflexivity :
    [ (Pi (A Type0) (Pi (x A) (Eq A x x))) ]
    [ (intro (A Type0) (intro (x A) (refl x))) ]
]
";
    let session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    let prepared = session.prepare(logic, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared.proof_obligations().ok_or("missing pending source proof")?;
    assert_eq!(batch.module, "logic");
    assert_eq!(batch.version, 1);
    assert_eq!(batch.source.as_slice(), logic.as_slice());
    assert_eq!(batch.obligations[0].name, "polymorphic-reflexivity");
    assert!(matches!(&batch.obligations[0].goal, noble_contracts::intrinsic::PendingGoal::Pure { .. }));
    let (session, result) = session.commit(prepared);
    assert!(matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance));
    assert_eq!(session.generation(), 0);
    let source_checked = session.prepare(logic, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let (session, result) = session.commit_verified(source_checked, |batch| {
        noble_contracts::intrinsic::check_batch(batch, super::LIMITS)
            .map_err(|error| format!("{error:?}"))
    });
    assert!(matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance),
        "source-only checking without a host model/checker revision must not publish a proof");
    assert_eq!(session.generation(), 0);
    let ordinary = session.prepare(b"module ordinary@1 [ export proof def proof [ 1 ] ]", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(ordinary.proof_obligations().is_none());
    let (session, result) = session.commit(ordinary);
    result.map_err(|error| format!("{error:?}"))?;
    let callable = session.prepare(b"ordinary@1.proof", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(callable.output(), &[noble_kernel::types::Ty::I64]);
    let mixed = session.prepare(
        b"module mixed@1 [ export proof def proof [ 1 ] proof 1 theorem : [ (Eq I64 0 0) ] [ (refl 0) ] ]",
        &[], super::LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert_eq!(mixed.proof_obligations().ok_or("mixed module lost logical body")?
        .obligations[0].name, "theorem");
    Ok(())
}

#[test]
fn callback_cannot_publish_invalid_source_or_substituted_claim() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    session = super::commit(session, b"module anchor@1 [ export alive def alive [ 1 ] ]")?;
    let prior = session.generation();
    let invalid = b"module bad@1 [ proof 1 unsound : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl missing)) ] ]";
    let prepared = session.prepare(invalid, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let mut invoked = false;
    let (next, result) = session.commit_verified(prepared, |batch| {
        invoked = true;
        Ok(vec![noble_contracts::intrinsic::CheckedProof {
            name: batch.obligations[0].name.clone(),
            kind: noble_contracts::intrinsic::ProofKind::Pure,
            claim: "True".into(),
            lean_term: "True.intro".into(),
            model_revision: "a".repeat(64),
            checker_revision: "b".repeat(64),
        }])
    });
    assert!(!invoked, "source-invalid proof must reject before invoking host");
    assert!(matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance));
    session = next;
    assert_eq!(session.generation(), prior);

    let unsupported = b"module identity@1 [ def id [ ] contract 1 id-law [ subject id input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (in x)) ] ] proof 1 forged for id-law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]";
    let prepared = session.prepare(unsupported, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let mut invoked = false;
    let (next, result) = session.commit_verified(prepared, |batch| {
        invoked = true;
        Ok(vec![noble_contracts::intrinsic::CheckedProof {
            name: batch.obligations[0].name.clone(),
            kind: noble_contracts::intrinsic::ProofKind::Contract,
            claim: "True".into(),
            lean_term: "True.intro".into(),
            model_revision: "a".repeat(64),
            checker_revision: "b".repeat(64),
        }])
    });
    assert!(!invoked, "unsupported program proof must reject before invoking host");
    assert!(matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance));
    session = next;
    assert_eq!(session.generation(), prior);

    let valid = b"module logic@1 [ proof 1 refl : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] ]";
    for change_claim in [true, false] {
        let prepared = session.prepare(valid, &[], super::LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        let (next, result) = session.commit_verified(prepared, |batch| {
            let mut checked = noble_contracts::intrinsic::check_batch(batch, super::LIMITS)
                .map_err(|error| format!("{error:?}"))?;
            let accepted = checked.first_mut().ok_or("missing source-checked proof")?;
            // Deliberately forged syntactically valid host revisions are not
            // sufficient to replace either the actual claim or Lean term.
            accepted.model_revision = "a".repeat(64);
            accepted.checker_revision = "b".repeat(64);
            if change_claim {
                accepted.claim = "True".into();
            } else {
                // One more pair of parentheses is still a well-typed Lean
                // expression, but not the exact lowering of the source term.
                accepted.lean_term = format!("({})", accepted.lean_term);
            }
            Ok(checked)
        });
        assert!(matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance));
        session = next;
        assert_eq!(session.generation(), prior);
    }
    assert!(session.prepare(b"import bad@1 as absent", &[], super::LIMITS).is_err());
    assert!(session.prepare(b"import identity@1 as absent", &[], super::LIMITS).is_err());
    assert!(session.prepare(b"import logic@1 as absent", &[], super::LIMITS).is_err());
    let preserved = session.prepare(b"anchor@1.alive", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(preserved.output(), &[noble_kernel::types::Ty::I64]);
    Ok(())
}

#[test]
fn retained_proof_dependency_budget_rejects_atomically() -> Result<(), String> {
    let source = b"module logic@1 [ proof 1 base : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] proof 1 derived : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (use base) x)) ] export derived ]";
    let bytes = u32::try_from(source.len() * 2)
        .map_err(|_| "proof dependency fixture exceeds u32 byte limit")?;
    let limits = noble_contracts::Limits { bytes, ..super::LIMITS };
    let mut session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    session = super::commit(session, b"module anchor@1 [ export alive def alive [ 1 ] ]")?;
    let prior = session.generation();
    let prepared = session.prepare(source, &[], limits)
        .map_err(|error| format!("{error:?}"))?;
    let (session, outcome) = session.commit_verified(prepared, |batch| {
        let mut checked = noble_contracts::intrinsic::check_batch(batch, limits)
            .map_err(|error| format!("{error:?}"))?;
        for proof in &mut checked {
            // An adversarial host can syntactically forge revision fields,
            // but cannot exceed the immutable dependency publication budget.
            proof.model_revision = "a".repeat(64);
            proof.checker_revision = "b".repeat(64);
        }
        Ok(checked)
    });
    assert!(matches!(outcome, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance));
    assert_eq!(session.generation(), prior);
    assert!(session.prepare(b"import logic@1 as absent", &[], super::LIMITS).is_err());
    let retained = session.prepare(b"anchor@1.alive", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(retained.output(), &[noble_kernel::types::Ty::I64]);
    Ok(())
}

#[test]
fn exact_increment_contract_binds_checked_definition_not_text_only() -> Result<(), String> {
    let source = b"module arithmetic@1 [
  def increment [ 1 + ]
  contract 1 increment-law [ subject increment
    input [ x I64 ] output [ y I64 ]
    requires [ true ]
    ensures [ (eq (out y) (add (in x) 1)) ] ]
  proof 1 increment-correct for increment-law
    [ (export-unary-I64
        (intro (tail Stack) (intro (x I64)
          (pc-sequence (pc-exact (exec-literal 1))
            (pc-exact (exec-add))
            (bridge (by-exact-append-assoc))
            (join (by-exact-result)))))
        (mc1-true-eq-wrap)) ]
]
";
    let session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    let prepared = session.prepare(source, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared.proof_obligations().ok_or("missing pending program proof")?;
    let noble_contracts::intrinsic::PendingGoal::Contract { contract } = &batch.obligations[0].goal else {
        return Err("missing resolved MC1 contract claim".into());
    };
    assert_eq!(contract.contract_name, "increment-law");
    assert_eq!(contract.subject.definition_source.as_slice(), b"def increment [ 1 + ]");
    assert_eq!(contract.subject.module_source.as_slice(), source.as_slice());
    assert_eq!(contract.subject.input_types, [noble_kernel::types::Ty::I64]);
    assert_eq!(contract.subject.output_types, [noble_kernel::types::Ty::I64]);
    assert_eq!(contract.subject.accepted_submission.definitions.len(), 1);
    assert_eq!(contract.subject.source_dependencies.len(), 1);
    assert_eq!(contract.subject.source_dependencies[0].full_source.as_slice(), source.as_slice());
    assert_eq!(contract.subject.effects, noble_kernel::types::EffSet::empty());
    let (session, result) = session.commit(prepared);
    assert!(matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance));
    assert_eq!(session.generation(), 0);
    assert!(session.prepare(b"arithmetic@1.increment", &[], super::LIMITS).is_err());
    Ok(())
}

#[test]
fn direct_mc1_contract_rejects_inconsistent_public_binding_and_recipe() -> Result<(), String> {
    use noble_contracts::intrinsic::{prepare_contract, PendingGoal};
    use noble_kernel::untrusted::{Node, NodeId};
    let source = b"module arithmetic@1 [
  def other [ 2 + ]
  def increment [ 1 + ]
  contract 1 law [ subject increment input [ x I64 ] output [ y I64 ]
    requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ]
  proof 1 p for law [ (export-unary-I64
    (intro (tail Stack) (intro (x I64)
      (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add))
        (bridge (by-exact-append-assoc)) (join (by-exact-result)))))
    (mc1-true-eq-wrap)) ]
]";
    let session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    let prepared = session.prepare(source, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared.proof_obligations().ok_or("missing contract proof")?;
    let PendingGoal::Contract {contract} = &batch.obligations[0].goal else {
        return Err("missing resolved contract".into());
    };
    prepare_contract(contract,super::LIMITS).map_err(|error| format!("{error:?}"))?;
    noble_contracts::intrinsic::check_batch(batch,super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(contract.subject.definition_ordinal,1);
    let mut changed = contract.clone();
    changed.subject.definition_source = b"def increment [ 2 + ]".to_vec();
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"forged definition bytes");
    let mut changed = contract.clone();
    let at = changed.subject.module_source.windows(b"def increment [ 1 + ]".len())
        .position(|window|window == b"def increment [ 1 + ]").ok_or("missing source occurrence")?;
    changed.subject.module_source[at+b"def increment [ ".len()] = b'2';
    changed.subject.definition_source = b"def increment [ 2 + ]".to_vec();
    changed.subject.source_dependencies[0].full_source = changed.subject.module_source.clone();
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"source changed but accepted recipe stayed old");
    let mut changed = contract.clone();
    changed.subject.source_span.start += 1;
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"forged lexical span");
    let mut changed = contract.clone();
    changed.subject.definition_ordinal = 0;
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"forged source ordinal");
    let mut changed = contract.clone();
    changed.subject.definition_owner += 1;
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"forged owner");
    let mut changed = contract.clone();
    changed.subject.source_dependencies[0].full_source[0] = b'x';
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"forged source dependency");
    let mut changed = contract.clone();
    changed.subject.accepted_submission.environment.kinds[4] = noble_kernel::contracts::Behavior::Equals;
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"forged builtin behavior");
    let mut changed = contract.clone();
    let definition = changed.subject.accepted_submission.definitions.iter_mut()
        .find(|definition|definition.identity == changed.subject.definition_identity)
        .ok_or("missing accepted specialization")?;
    let first = definition.body.candidate.body[0].0 as usize;
    let Node::Literal {lit,..} = &mut definition.body.candidate.nodes[first] else {
        return Err("missing literal".into());
    };
    *lit = noble_kernel::untrusted::Lit::I64(2);
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"same interface but different accepted code");
    let mut changed = contract.clone();
    let definition = changed.subject.accepted_submission.definitions.iter_mut()
        .find(|definition|definition.identity == changed.subject.definition_identity)
        .ok_or("missing accepted specialization")?;
    let first = definition.body.candidate.body[0].0 as usize;
    let Node::Literal {inst,..} = &mut definition.body.candidate.nodes[first] else {
        return Err("missing literal".into());
    };
    inst.bindings.clear();
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"forged literal instantiation");
    let mut changed = contract.clone();
    let definition = changed.subject.accepted_submission.definitions.iter_mut()
        .find(|definition|definition.identity == changed.subject.definition_identity)
        .ok_or("missing accepted specialization")?;
    let second = definition.body.candidate.body[1].0 as usize;
    let Node::Invocation {inst,..} = &mut definition.body.candidate.nodes[second] else {
        return Err("missing add".into());
    };
    inst.bindings.clear();
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"forged add instantiation");
    let mut changed = contract.clone();
    let definition = changed.subject.accepted_submission.definitions.iter_mut()
        .find(|definition|definition.identity == changed.subject.definition_identity)
        .ok_or("missing accepted specialization")?;
    definition.body.candidate.body = vec![NodeId(0),NodeId(0)];
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"wrong complete recipe");
    let mut changed = contract.clone();
    let definition = changed.subject.accepted_submission.definitions.iter_mut()
        .find(|definition|definition.identity == changed.subject.definition_identity)
        .ok_or("missing accepted specialization")?;
    definition.definition.0 += 1;
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"wrong specialization slot");
    let mut changed = contract.clone();
    let root = changed.subject.accepted_submission.body.candidate.body[0].0 as usize;
    let Node::Invocation {inst,..} = &mut changed.subject.accepted_submission.body.candidate.nodes[root] else {
        return Err("missing root invocation".into());
    };
    inst.bindings.push(noble_kernel::words::Binding::Stack(vec![]));
    assert!(prepare_contract(&changed,super::LIMITS).is_err(),"forged root instantiation");
    Ok(())
}

#[test]
fn source_bound_contract_preserves_formatting_and_refuses_named_v1() -> Result<(), String> {
    let formatted = b"module formatted@1 [
  export increment
  def increment # comment between name and bracket
    [ 1 + ]
  contract 1 law [ subject increment input [ x I64 ] output [ y I64 ]
    requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ]
]";
    let session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    let prepared = session.prepare(formatted,&[],super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(prepared.proof_obligations().is_none(),"proof-free contract remains inert metadata");
    let (session, committed) = session.commit(prepared);
    committed.map_err(|error| format!("{error:?}"))?;
    let typed = session.prepare(b"formatted@1.increment",&[noble_kernel::types::Ty::I64],super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(typed.output(),&[noble_kernel::types::Ty::I64]);

    let two = b"module two@1 [
      def increment [ 2 + ]
      contract 1 law [ subject increment input [ x I64 ] output [ y I64 ]
        requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ]
      proof 1 p for law [ (export-unary-I64
        (intro (tail Stack) (intro (x I64)
          (pc-sequence (pc-exact (exec-literal 2)) (pc-exact (exec-add))
            (bridge (by-exact-append-assoc)) (join (by-exact-result)))))
        (mc1-true-eq-wrap)) ]
    ]";
    let session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    let prepared = session.prepare(two,&[],super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    noble_contracts::intrinsic::check_batch(
        prepared.proof_obligations().ok_or("missing nonunit literal proof")?,super::LIMITS,
    ).map_err(|error| format!("{error:?}"))?;

    let named = b"module named@1 [ def one [ 1 ] export increment def increment [ one + ] ]";
    let session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    let session = super::commit(session,named)?;
    let ordinary = session.prepare(b"named@1.increment",&[noble_kernel::types::Ty::I64],super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(ordinary.output(),&[noble_kernel::types::Ty::I64]);
    let named_contract = b"module named_law@1 [
      def one [ 1 ] def increment [ one + ]
      contract 1 law [ subject increment input [ x I64 ] output [ y I64 ]
        requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ]
    ]";
    let fresh = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    assert!(fresh.prepare(named_contract,&[],super::LIMITS).is_err(),
        "named ordinary runtime is not an MC1-v1 contract theorem");
    let named_proof = b"module named_proof@1 [
      def one [ 1 ] def increment [ one + ]
      contract 1 law [ subject increment input [ x I64 ] output [ y I64 ]
        requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ]
      proof 1 p for law [ (export-unary-I64
        (intro (tail Stack) (intro (x I64)
          (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add))
            (bridge (by-exact-append-assoc)) (join (by-exact-result)))))
        (mc1-true-eq-wrap)) ]
    ]";
    assert!(fresh.prepare(named_proof,&[],super::LIMITS).is_err(),
        "a named runtime call cannot be silently exported as a v1 proof");
    Ok(())
}

#[test]
fn unproved_contract_metadata_erases_without_shifting_definition_ids() -> Result<(), String> {
    let plain = b"module arithmetic@1 [ export increment def increment [ 1 + ] ]";
    let decorated = b"module arithmetic@1 [ export increment def increment [ 1 + ] contract 1 increment-law [ subject increment input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] ]";
    let mut bare = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    let mut logical = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    bare = super::commit(bare, plain)?;
    logical = super::commit(logical, decorated)?;
    let input = [noble_kernel::types::Ty::I64];
    let original = bare.prepare(b"arithmetic@1.increment", &input, super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let erased = logical.prepare(b"arithmetic@1.increment", &input, super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(original.output(), erased.output());
    let original = original.submission().ok_or("missing plain typed program")?;
    let erased = erased.submission().ok_or("missing decorated typed program")?;
    assert_eq!(original.definitions.len(), erased.definitions.len());
    for (before, after) in original.definitions.iter().zip(&erased.definitions) {
        assert_eq!(before.definition, after.definition);
    }
    Ok(())
}

#[test]
fn proof_lexer_modes_and_stale_host_gate_leave_prior_namespace_intact() -> Result<(), String> {
    let mut session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    session = super::commit(session, b"module logic@1 [ export value def value [ 1 ] ]")?;
    let staged = session.prepare(
        b"module theorem@1 [ proof 1 poly-refl : [ (Eq I64 0 0) # ordinary comment\n ] [ (refl 0) ] ]",
        &[], super::LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert_eq!(staged.proof_obligations().ok_or("proof was not staged")?.obligations.len(), 1);
    assert!(session.prepare(
        b"module bad@1 [ def value [ (refl 0) ] ]", &[], super::LIMITS,
    ).is_err(), "logical proof syntax must not become an ordinary guest word");
    for bad in [
        b"module bad@1 [ proof 01 p : [ (Eq I64 0 0) ] [ (refl 0) ] ]".as_slice(),
        b"module bad@1 [ proof 1 p : [ (Eq I64 0 0) ] [ (refl 0\\) ] ]".as_slice(),
        b"module bad@1 [ proof 1 p : [ (Eq I64 0 0) ] [ (refl 0) (refl 0) ] ]".as_slice(),
        b"module bad@1 [ proof 1 p : [ (Eq I64 0 0 ] [ (refl 0) ] ]".as_slice(),
    ] {
        assert!(matches!(session.prepare(bad, &[], super::LIMITS), Err(error)
            if error.stage() == noble_contracts::source::Stage::Parse));
    }
    session = super::commit(session, b"import logic@1 as alias")?;
    let prior = session.generation();
    let (session, result) = session.commit_verified(staged, |_| -> Result<_, String> {
        panic!("stale staged source must not invoke the independent checker")
    });
    assert!(matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance));
    assert_eq!(session.generation(), prior);
    let callable = session.prepare(b"alias.value", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(callable.output(), &[noble_kernel::types::Ty::I64]);
    Ok(())
}

#[test]
fn general_mc1_contract_is_unproved_metadata_and_alias_resolves_exact_subject() -> Result<(), String> {
    let source = b"module generic@1 [ def identity [ ] contract 1 identity-law [ subject identity input [ x I64 ] output [ y I64 ] requires [ (le (in x) 4) ] ensures [ (eq (out y) (in x)) ] ] export identity-law ]";
    let mut session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    let prepared = session.prepare(source, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(prepared.proof_obligations().is_none());
    let (next, outcome) = session.commit(prepared);
    outcome.map_err(|error| format!("{error:?}"))?;
    session = next;
    assert!(session.prepare(b"generic@1.identity-law", &[], super::LIMITS).is_err(),
        "logical export must not create a guest word");
    session = super::commit(session, b"import generic@1 as g")?;
    let next = session.prepare(
        b"module uses@1 [ proof 1 identity-check for g.identity-law [ (refl 0) ] ]",
        &[], super::LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    let batch = next.proof_obligations().ok_or("missing imported-contract proof goal")?;
    let noble_contracts::intrinsic::PendingGoal::Contract { contract } = &batch.obligations[0].goal else {
        return Err("imported contract did not resolve to a typed goal".into());
    };
    assert_eq!(contract.subject.module, "generic");
    assert_eq!(contract.subject.version, 1);
    assert_eq!(contract.subject.module_source.as_slice(), source.as_slice());
    assert_eq!(contract.contract_name, "identity-law");
    Ok(())
}

#[test]
fn local_named_proof_use_is_ordered_and_not_an_executable_word() -> Result<(), String> {
    let source = b"module logic@1 [ proof 1 base : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] proof 1 derived : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (use base) x)) ] export derived ]";
    let session = noble_contracts::source::ModuleSession::new(&[])
        .map_err(|error| format!("{error:?}"))?;
    let prepared = session.prepare(source, &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared.proof_obligations().ok_or("missing ordered proof obligations")?;
    assert_eq!(batch.obligations.len(), 2);
    assert_eq!(batch.obligations[0].name, "base");
    assert_eq!(batch.obligations[1].name, "derived");
    assert!(batch.dependencies.is_empty(), "local proof may not impersonate imported metadata");
    let checked = noble_contracts::intrinsic::check_batch(batch, super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(checked[0].claim, checked[1].claim);
    let forward = b"module bad@1 [ proof 1 derived : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (use base) x)) ] proof 1 base : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] ]";
    assert!(matches!(session.prepare(forward, &[], super::LIMITS), Err(error)
        if error.stage() == noble_contracts::source::Stage::Resolve));
    assert_eq!(session.generation(), 0);
    Ok(())
}
