//! Direct witness-resolution controls for the fragment's reference bindings.

/// Reference bindings resolve to their terminal witness: acyclic chains
/// substitute exactly as their resolved bindings would, and cycles reject
/// (B-CHECK-05).
// r[verify VT-M3-01]
#[test]
fn witness_references_resolve_and_cycles_reject() -> Result<(), String> {
    let kinds = [
        noble_kernel::words::VariableKind::Stack,
        noble_kernel::words::VariableKind::Value,
        noble_kernel::words::VariableKind::Value,
        noble_kernel::words::VariableKind::Value,
    ];
    // `v1 -> v2 -> v3 -> Bool` resolves with two charged hops.
    let chained = noble_kernel::words::Inst {
        bindings: vec![
            noble_kernel::words::Binding::Stack(vec![noble_kernel::types::Ty::I64]),
            noble_kernel::words::Binding::Ref(noble_kernel::words::Variable(2)),
            noble_kernel::words::Binding::Ref(noble_kernel::words::Variable(3)),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
    };
    let (resolved, spent) = noble_kernel::words::resolve::bindings(&kinds, &chained, 8)
        .map_err(|problem| format!("chain must resolve: {problem:?}"))?;
    // Three charged hops: `v1` walks two references to its terminal, and
    // `v2`'s own reference walks one more.
    assert_eq!(spent, 3);
    assert_eq!(
        resolved.value(noble_kernel::words::Variable(1)),
        Some(&noble_kernel::types::Ty::Bool)
    );
    assert_eq!(
        resolved.stack(noble_kernel::words::Variable(0)),
        Some(&[noble_kernel::types::Ty::I64][..])
    );
    // A mutual cycle rejects.
    let cyclic = noble_kernel::words::Inst {
        bindings: vec![
            noble_kernel::words::Binding::Stack(vec![]),
            noble_kernel::words::Binding::Ref(noble_kernel::words::Variable(2)),
            noble_kernel::words::Binding::Ref(noble_kernel::words::Variable(1)),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
    };
    assert!(matches!(
        noble_kernel::words::resolve::bindings(&kinds, &cyclic, 8),
        Err(noble_kernel::words::InstError::CyclicWitness)
    ));
    // A self-cycle rejects.
    let self_ref = noble_kernel::words::Inst {
        bindings: vec![
            noble_kernel::words::Binding::Stack(vec![]),
            noble_kernel::words::Binding::Ref(noble_kernel::words::Variable(1)),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
    };
    assert!(matches!(
        noble_kernel::words::resolve::bindings(&kinds, &self_ref, 8),
        Err(noble_kernel::words::InstError::CyclicWitness)
    ));
    assert!(matches!(
        noble_kernel::words::resolve::bindings(&kinds, &chained, 1),
        Err(noble_kernel::words::InstError::WalkExhausted)
    ));
    Ok(())
}
