#[test]
fn unproved_contract_metadata_erases_without_shifting_definition_ids() -> Result<(), String> {
    let plain = b"module arithmetic@1 [ export increment def increment [ 1 + ] ]";
    let decorated = b"module arithmetic@1 [ export increment def increment [ 1 + ] contract 1 increment-law [ subject increment input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] ]";
    let mut bare =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let mut logical =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    bare = super::super::commit(bare, plain)?;
    logical = super::super::commit(logical, decorated)?;
    let input = [noble_kernel::types::Ty::I64];
    let original = bare
        .prepare(b"arithmetic@1.increment", &input, super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let erased = logical
        .prepare(b"arithmetic@1.increment", &input, super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(original.output(), erased.output());
    let original = original.submission().ok_or("missing plain typed program")?;
    let erased = erased
        .submission()
        .ok_or("missing decorated typed program")?;
    assert_eq!(original.definitions.len(), erased.definitions.len());
    for (before, after) in original.definitions.iter().zip(&erased.definitions) {
        assert_eq!(before.definition, after.definition);
    }
    Ok(())
}

#[test]
fn proof_lexer_modes_and_stale_host_gate_leave_prior_namespace_intact() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    session = super::super::commit(session, b"module logic@1 [ export value def value [ 1 ] ]")?;
    let staged = session.prepare(
        b"module theorem@1 [ proof 1 poly-refl : [ (Eq I64 0 0) # ordinary comment\n ] [ (refl 0) ] ]",
        &[], super::super::LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        staged
            .proof_obligations()
            .ok_or("proof was not staged")?
            .obligations
            .len(),
        1
    );
    assert!(
        session
            .prepare(
                b"module bad@1 [ def value [ (refl 0) ] ]",
                &[],
                super::super::LIMITS,
            )
            .is_err(),
        "logical proof syntax must not become an ordinary guest word"
    );
    for bad in [
        b"module bad@1 [ proof 01 p : [ (Eq I64 0 0) ] [ (refl 0) ] ]".as_slice(),
        b"module bad@1 [ proof 1 p : [ (Eq I64 0 0) ] [ (refl 0\\) ] ]".as_slice(),
        b"module bad@1 [ proof 1 p : [ (Eq I64 0 0) ] [ (refl 0) (refl 0) ] ]".as_slice(),
        b"module bad@1 [ proof 1 p : [ (Eq I64 0 0 ] [ (refl 0) ] ]".as_slice(),
    ] {
        assert!(
            matches!(session.prepare(bad, &[], super::super::LIMITS), Err(error)
            if error.stage() == noble_contracts::source::Stage::Parse)
        );
    }
    session = super::super::commit(session, b"import logic@1 as alias")?;
    let prior = session.generation();
    let mut was_checker_invoked = false;
    let (session, result) = session.commit_verified(staged, |_| -> Result<_, String> {
        was_checker_invoked = true;
        Err(String::from(
            "stale staged source must not invoke the independent checker",
        ))
    });
    assert!(
        !was_checker_invoked,
        "stale staged source invoked the independent checker"
    );
    assert!(
        matches!(result, Err(error) if error.stage() == noble_contracts::source::Stage::Acceptance)
    );
    assert_eq!(session.generation(), prior);
    let callable = session
        .prepare(b"alias.value", &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(callable.output(), &[noble_kernel::types::Ty::I64]);
    Ok(())
}

#[test]
fn general_mc1_contract_is_unproved_metadata_and_alias_resolves_exact_subject() -> Result<(), String>
{
    let source = b"module generic@1 [ def identity [ ] contract 1 identity-law [ subject identity input [ x I64 ] output [ y I64 ] requires [ (le (in x) 4) ] ensures [ (eq (out y) (in x)) ] ] export identity-law ]";
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let prepared = session
        .prepare(source, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(prepared.proof_obligations().is_none());
    let (next, outcome) = session.commit(prepared);
    outcome.map_err(|error| format!("{error:?}"))?;
    session = next;
    assert!(
        session
            .prepare(b"generic@1.identity-law", &[], super::super::LIMITS)
            .is_err(),
        "logical export must not create a guest word"
    );
    session = super::super::commit(session, b"import generic@1 as g")?;
    let next = session
        .prepare(
            b"module uses@1 [ proof 1 identity-check for g.identity-law [ (refl 0) ] ]",
            &[],
            super::super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    let batch = next
        .proof_obligations()
        .ok_or("missing imported-contract proof goal")?;
    let noble_contracts::intrinsic::PendingGoal::Contract { contract } = &batch.obligations[0].goal
    else {
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
    let session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let prepared = session
        .prepare(source, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared
        .proof_obligations()
        .ok_or("missing ordered proof obligations")?;
    assert_eq!(batch.obligations.len(), 2);
    assert_eq!(batch.obligations[0].name, "base");
    assert_eq!(batch.obligations[1].name, "derived");
    assert!(
        batch.dependencies.is_empty(),
        "local proof may not impersonate imported metadata"
    );
    let exact = noble_contracts::Limits {
        bytes: u32::try_from(source.len()).map_err(|_| "proof source exceeds u32 byte limit")?,
        ..super::super::LIMITS
    };
    let checked = noble_contracts::intrinsic::check_batch(batch, exact)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(checked[0].claim, checked[1].claim);
    let short_bytes = exact
        .bytes
        .checked_sub(1)
        .ok_or("proof source has no byte below its exact budget")?;
    let short = noble_contracts::Limits {
        bytes: short_bytes,
        ..exact
    };
    let refused = noble_contracts::intrinsic::check_batch(batch, short)
        .expect_err("one byte below retained source length must exhaust proof budget");
    assert_eq!(refused.kind, noble_contracts::DiagnosticKind::Exhausted);
    assert_eq!(refused.span, noble_contracts::Span { start: 0, end: 0 });
    assert_eq!(refused.message, "proof size, depth or work limit exhausted");
    let forward = b"module bad@1 [ proof 1 derived : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (apply (use base) x)) ] proof 1 base : [ (Pi (x I64) (Eq I64 x x)) ] [ (intro (x I64) (refl x)) ] ]";
    assert!(
        matches!(session.prepare(forward, &[], super::super::LIMITS), Err(error)
        if error.stage() == noble_contracts::source::Stage::Resolve)
    );
    assert_eq!(session.generation(), 0);
    Ok(())
}

#[test]
fn applying_value_binder_preserves_later_dependent_type_codes() -> Result<(), String> {
    let source = b"module logic@1 [
        proof 1 base :
          [ (Pi (x I64) (Pi (h (Eq I64 x x)) (Pi (A Type0) (Pi (y (Pair (List A) I64)) (Eq (Pair (List A) I64) y y))))) ]
          [ (intro (x I64) (intro (h (Eq I64 x x)) (intro (A Type0) (intro (y (Pair (List A) I64)) (refl y))))) ]
        proof 1 derived :
          [ (Pi (x I64) (Pi (z (Pair (List I64) I64)) (Eq (Pair (List I64) I64) z z))) ]
          [ (intro (x I64) (intro (z (Pair (List I64) I64)) (apply (apply (apply (apply (use base) x) (refl x)) I64) z))) ]
    ]";
    let session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let prepared = session
        .prepare(source, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let checked = noble_contracts::intrinsic::check_batch(
        prepared
            .proof_obligations()
            .ok_or("missing dependent proof obligations")?,
        super::super::LIMITS,
    )
    .map_err(|error| format!("{error:?}"))?;
    assert_eq!(checked[0].claim, "(∀ (v0 : El .i64), (∀ (v1 : v0 = v0), (∀ (v2 : PureTyCode), (∀ (v3 : El (.pair (.list v2) .i64)), v3 = v3))))");
    assert_eq!(
        checked[1].claim,
        "(∀ (v0 : El .i64), (∀ (v1 : El (.pair (.list .i64) .i64)), v1 = v1))"
    );

    let invalid = b"module bad@1 [
        proof 1 base :
          [ (Pi (x I64) (Pi (h (Eq I64 x x)) (Pi (A Type0) (Pi (y (Pair (List A) I64)) (Eq (Pair (List A) I64) y y))))) ]
          [ (intro (x I64) (intro (h (Eq I64 x x)) (intro (A Type0) (intro (y (Pair (List A) I64)) (refl y))))) ]
        proof 1 derived :
          [ (Pi (x I64) (Pi (z (Pair (List I64) I64)) (Pi (w Bool) (Eq (Pair (List I64) I64) z z)))) ]
          [ (intro (x I64) (intro (z (Pair (List I64) I64)) (intro (w Bool) (apply (apply (apply (apply (use base) x) (refl x)) I64) w)))) ]
    ]";
    let prepared = session
        .prepare(invalid, &[], super::super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(
        noble_contracts::intrinsic::check_batch(
            prepared
                .proof_obligations()
                .ok_or("missing hostile proof obligations")?,
            super::super::LIMITS,
        )
        .is_err(),
        "a Bool sibling cannot stand in for the composite value argument"
    );
    Ok(())
}

/// `n` inferred `Result.Ok` layers whose error types are fixed to `I64` by
/// nested matches, with no explicit nested `Result<...>` annotation.
fn nested_inferred_results(depth: usize) -> String {
    let mut matcher = String::new();
    for _ in 0..depth {
        matcher = format!("[ {matcher} ] [ 1 + ] result@1.Result.match");
    }
    format!("1 {}{matcher}", "result@1.Result.Ok ".repeat(depth))
}

#[test]
fn nested_inferred_results_refuse_descriptor_expansion_before_admission() -> Result<(), String> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    session = super::super::commit(session, noble_contracts::source::RESULT_LIBRARY_SOURCE)?;
    for depth in [1, 2, 4, 8] {
        let accepted = session
            .prepare(
                nested_inferred_results(depth).as_bytes(),
                &[],
                super::super::LIMITS,
            )
            .map_err(|e| format!("depth {depth}: {e:?}"))?;
        assert_eq!(accepted.output(), &[noble_kernel::types::Ty::I64]);
    }
    // Owned descriptor nodes double per layer while the logical size stays
    // far below 256 constructors. Refusal must come from inference before
    // the copies are made, not from kernel admission of an expanded witness.
    for depth in [11, 12, 16] {
        let refused = session
            .prepare(
                nested_inferred_results(depth).as_bytes(),
                &[],
                super::super::LIMITS,
            )
            .err()
            .ok_or_else(|| format!("depth {depth} was accepted"))?;
        assert_eq!(refused.stage(), noble_contracts::source::Stage::Check);
        assert_eq!(
            refused.diagnostic().kind,
            noble_contracts::DiagnosticKind::Exhausted
        );
        assert_eq!(
            refused.diagnostic().message,
            "inferred type representation exceeds the 4096-node limit"
        );
    }
    Ok(())
}
