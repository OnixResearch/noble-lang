use super::*;

#[test]
fn primitive_chunk_retains_its_recipe_and_derived_interface() -> Result<(), String> {
    let mut entries = library();
    // A literal recipe emits its literal; the interface is derived from source
    // admission, so the declared one must match exactly.
    let body = Recipe {
        nodes: vec![
            Node::I64(1),
            Node::I64(2),
            Node::Operation {
                op: Op::Add,
                instantiation: 0,
            },
        ],
        entry: vec![0, 1, 2],
    };
    let candidate = envelope(&[], &[Ty::I64], body, b"1 2 + ");
    let session = noble_contracts::source::Session::without_test_hosts();
    let index = entries
        .admit(&session, candidate)
        .map_err(|refusal| format!("admit: {refusal:?}"))?;
    assert_eq!(index, 0);
    let entry = entries
        .entries()
        .first()
        .ok_or_else(|| String::from("entry retained"))?;
    assert_eq!(entry.source(), b"1 2 + ");
    assert_eq!(entry.interface().stack_in, Vec::new());
    assert_eq!(entry.interface().stack_out, vec![Ty::I64]);
    assert_eq!(entry.expanded_nodes(), 3);
    Ok(())
}

#[test]
fn forged_identity_is_refused() -> Result<(), String> {
    let mut entries = library();
    let body = Recipe {
        nodes: vec![Node::Bool(true)],
        entry: vec![0],
    };
    let mut candidate = envelope(&[], &[Ty::Bool], body, b"true ");
    candidate.chunk_id = [9u8; 16];
    let session = noble_contracts::source::Session::without_test_hosts();
    let refusal = entries
        .admit(&session, candidate)
        .expect_err("forged identity");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.chunk_id");
    assert!(entries.entries().is_empty());
    Ok(())
}

#[test]
fn forged_source_revision_is_refused() -> Result<(), String> {
    let mut entries = library();
    let body = Recipe {
        nodes: vec![Node::Bool(true)],
        entry: vec![0],
    };
    let mut candidate = envelope(&[], &[Ty::Bool], body, b"true ");
    candidate.source_revision = revision(3);
    let session = noble_contracts::source::Session::without_test_hosts();
    let refusal = entries
        .admit(&session, candidate)
        .expect_err("forged revision");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.source_revision");
    assert!(entries.entries().is_empty());
    Ok(())
}

#[test]
fn wrong_declared_interface_is_refused() -> Result<(), String> {
    let mut entries = library();
    let body = Recipe {
        nodes: vec![
            Node::Bool(false),
            Node::Bool(true),
            Node::Operation {
                op: Op::Swap,
                instantiation: 0,
            },
        ],
        entry: vec![0, 1, 2],
    };
    let mut candidate = envelope(&[], &[Ty::Bool, Ty::Bool], body, b"false true swap ");
    candidate.instantiations = vec![Instantiation {
        bindings: vec![
            Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::Bool,
            },
            Binding {
                variable: 2,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::Bool,
            },
        ],
    }];
    candidate.chunk_id = chunk::chunk_identity_of(&candidate);
    let session = noble_contracts::source::Session::without_test_hosts();
    entries
        .admit(&session, candidate.clone())
        .map_err(|refusal| format!("admit: {refusal:?}"))?;
    // The same recipe declared with its two results in the other order is wrong,
    // and the interface source admission derives is the one that is retained.
    candidate.interface.stack_in = vec![Ty::I64];
    candidate.chunk_id = chunk::chunk_identity_of(&candidate);
    let refusal = entries
        .admit(&session, candidate)
        .expect_err("wrong ordered input");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.interface");
    assert_eq!(entries.entries().len(), 1);
    Ok(())
}

#[test]
fn changed_body_is_refused_at_the_identity() -> Result<(), String> {
    let mut entries = library();
    let body = Recipe {
        nodes: vec![
            Node::I64(1),
            Node::I64(2),
            Node::Operation {
                op: Op::Add,
                instantiation: 0,
            },
        ],
        entry: vec![0, 1, 2],
    };
    let candidate = envelope(&[], &[Ty::I64], body, b"1 2 + ");
    let session = noble_contracts::source::Session::without_test_hosts();
    entries
        .admit(&session, candidate.clone())
        .map_err(|refusal| format!("admit: {refusal:?}"))?;
    // Change only the body: the retained identity no longer describes it.
    let mut changed = candidate;
    changed.body.nodes = vec![
        Node::I64(1),
        Node::I64(2),
        Node::Operation {
            op: Op::Sub,
            instantiation: 0,
        },
    ];
    let refusal = entries
        .admit(&session, changed.clone())
        .expect_err("changed body");
    assert_eq!(refusal.location.field, "envelope.chunk_id");
    // Recompute the identity but keep the old source revision: the canonical
    // source no longer matches either.
    changed.chunk_id = chunk::chunk_identity_of(&changed);
    let refusal = entries
        .admit(&session, changed)
        .expect_err("stale source revision");
    assert_eq!(refusal.location.field, "envelope.source_revision");
    assert_eq!(entries.entries().len(), 1);
    Ok(())
}

#[test]
fn unsupported_format_and_foreign_types_are_unsupported() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let body = Recipe {
        nodes: vec![Node::Bool(true)],
        entry: vec![0],
    };
    let mut newer = envelope(&[], &[Ty::Bool], body.clone(), b"true ");
    newer.format = 2;
    let refusal = entries.admit(&session, newer).expect_err("format");
    assert_eq!(refusal.class_name(), "unsupported");
    assert_eq!(refusal.location.field, "envelope.format");

    let mut stale_semantics = envelope(&[], &[Ty::Bool], body.clone(), b"true ");
    stale_semantics.semantic_revision = 1;
    let refusal = entries
        .admit(&session, stale_semantics)
        .expect_err("stale semantic revision");
    assert_eq!(refusal.class_name(), "unsupported");
    assert_eq!(refusal.location.field, "envelope.semantic_revision");

    let body = Recipe {
        nodes: vec![Node::Bool(true)],
        entry: vec![0],
    };
    let mut foreign = envelope(&[], &[Ty::Text], body, b"true ");
    foreign.chunk_id = chunk::chunk_identity_of(&foreign);
    let refusal = entries.admit(&session, foreign).expect_err("text");
    assert_eq!(refusal.class_name(), "unsupported");
    assert_eq!(refusal.location.field, "envelope.interface.stack_out");
    Ok(())
}
