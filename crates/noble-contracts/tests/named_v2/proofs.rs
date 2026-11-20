//! Source-proof and precondition rejection cases for named v2 calls.

#[test]
fn named_v2_rejects_weak_or_mismatched_source_proof() -> Result<(), String> {
    let (_, batch) = super::fixture()?;
    let mut repeated_call = batch.clone();
    if let noble_contracts::intrinsic::FormKind::List(root) =
        &mut repeated_call.obligations[0].term.kind
    {
        if let noble_contracts::intrinsic::FormKind::List(second) = &mut root[2].kind {
            second[1].kind = noble_contracts::intrinsic::FormKind::Atom("0".into());
        }
    }
    assert!(
        noble_contracts::intrinsic::check_batch(&repeated_call, super::LIMITS).is_err(),
        "repeated first call"
    );
    let mut admitted = batch.clone();
    if let noble_contracts::intrinsic::FormKind::List(root) = &mut admitted.obligations[0].term.kind
    {
        if let noble_contracts::intrinsic::FormKind::List(wrap) = &mut root[4].kind {
            wrap[0].kind = noble_contracts::intrinsic::FormKind::Atom("admit".into());
        }
    }
    assert!(
        noble_contracts::intrinsic::check_batch(&admitted, super::LIMITS).is_err(),
        "unchecked final rule"
    );
    let mut missing_slice = batch.clone();
    if let noble_contracts::intrinsic::FormKind::List(root) =
        &mut missing_slice.obligations[0].term.kind
    {
        root.remove(2);
    }
    assert!(
        noble_contracts::intrinsic::check_batch(&missing_slice, super::LIMITS).is_err(),
        "missing call slice"
    );
    Ok(())
}

#[test]
fn named_v2_rejects_wrong_precondition_or_output_claim() -> Result<(), String> {
    let (goal, _) = super::fixture()?;
    let mut pre = goal.clone();
    pre.requires.kind = noble_contracts::intrinsic::FormKind::Atom("false".into());
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&pre, super::LIMITS).is_err(),
        "generated claim cannot discard source precondition"
    );
    let mut output = goal.clone();
    if let noble_contracts::intrinsic::FormKind::List(eq) = &mut output.ensures.kind {
        if let noble_contracts::intrinsic::FormKind::List(add) = &mut eq[2].kind {
            add[2].kind = noble_contracts::intrinsic::FormKind::Atom("3".into());
        }
    }
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&output, super::LIMITS).is_err(),
        "generated claim cannot prove a different postcondition"
    );
    Ok(())
}

#[test]
fn quoted_named_call_cannot_use_two_direct_call_rule() -> Result<(), String> {
    let source = String::from_utf8(super::SOURCE.to_vec()).map_err(|error| error.to_string())?;
    let quoted = source.replace(
        "def twice [ d.step d.step ]",
        "def twice [ [ d.step ] run d.step ]",
    );
    assert!(
        super::fixture_for(quoted.as_bytes()).is_err(),
        "quotation/run changed the executable named-call structure"
    );
    Ok(())
}

#[test]
fn reviewed_v3_rule_identities_are_exact() {
    // The revision-3 reviewed rule set is a fixed list of exact Lean
    // declarations, pinned here and by `proofs/mc1/ContractGate.lean` under the
    // strict axiom list; any change is a reviewed `V-INTRINSIC-01` rule-set edit.
    assert_eq!(
        noble_contracts::intrinsic::REVIEWED_RULES_V3,
        &[
            ("bv-add-assoc", "BitVec.add_assoc"),
            ("bv-add-comm", "BitVec.add_comm"),
            ("bv-mul-add", "BitVec.mul_add"),
            ("list-append-assoc", "List.append_assoc"),
            ("list-append-nil", "List.append_nil"),
            ("list-nil-append", "List.nil_append"),
        ],
        "reviewed rule identities changed"
    );
    for (identifier, declaration) in noble_contracts::intrinsic::REVIEWED_RULES_V3 {
        assert!(
            declaration.contains('.'),
            "rule {identifier} is not qualified"
        );
        assert!(
            !declaration.contains("axiom"),
            "rule {identifier} names an axiom"
        );
        assert!(
            !declaration.contains("sorry"),
            "rule {identifier} contains a hole"
        );
    }
}
