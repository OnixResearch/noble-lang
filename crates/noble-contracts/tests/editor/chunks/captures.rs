use super::*;

#[test]
fn captures_are_contiguous_and_bounded() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let body = Recipe {
        nodes: vec![Node::Capture { index: 0 }],
        entry: vec![0],
    };
    let mut candidate = envelope(&[], &[Ty::I64], body.clone(), b"7 ");
    candidate.captures = vec![Capture {
        index: 1,
        value: Scalar::I64(7),
    }];
    candidate.chunk_id = chunk::chunk_identity_of(&candidate);
    let refusal = entries.admit(&session, candidate).expect_err("ordinal");
    assert_eq!(refusal.location.field, "envelope.captures");
    let refusal = entries
        .admit(&session, candidate_from(body))
        .expect_err("missing capture");
    assert_eq!(refusal.location.field, "envelope.body.nodes");

    let body = Recipe {
        nodes: vec![Node::Capture { index: 0 }],
        entry: vec![0],
    };
    let mut captured = envelope(&[], &[Ty::I64], body, b"7 ");
    captured.captures = vec![Capture {
        index: 0,
        value: Scalar::I64(7),
    }];
    captured.chunk_id = chunk::chunk_identity_of(&captured);
    let index = entries
        .admit(&session, captured.clone())
        .map_err(|refusal| format!("captured scalar: {refusal:?}"))?;
    assert_eq!(index, 0);
    assert_eq!(entries.entries()[0].source(), b"7 ");
    Ok(())
}

#[test]
fn changed_value_is_rejected_at_the_recomputed_identity() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let body = Recipe {
        nodes: vec![Node::Capture { index: 0 }],
        entry: vec![0],
    };
    let mut candidate = envelope(&[], &[Ty::I64], body, b"7 ");
    candidate.captures = vec![Capture {
        index: 0,
        value: Scalar::I64(7),
    }];
    candidate.chunk_id = chunk::chunk_identity_of(&candidate);
    entries
        .admit(&session, candidate.clone())
        .map_err(|refusal| format!("admit: {refusal:?}"))?;
    let mut changed = candidate;
    if let Some(capture) = changed.captures.get_mut(0) {
        capture.value = Scalar::I64(8);
    }
    let refusal = entries
        .admit(&session, changed)
        .expect_err("changed immutable capture");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.chunk_id");
    assert_eq!(entries.entries().len(), 1);
    Ok(())
}

#[test]
fn effect_widening_is_unsupported() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let body = Recipe {
        nodes: vec![Node::I64(1)],
        entry: vec![0],
    };
    let mut candidate = envelope(&[], &[Ty::I64], body, b"1 ");
    candidate.interface.effects =
        noble_kernel::types::EffSet::empty().with_id(noble_kernel::types::EffId(7));
    candidate.chunk_id = chunk::chunk_identity_of(&candidate);
    let refusal = entries
        .admit(&session, candidate)
        .expect_err("effect widening");
    assert_eq!(refusal.class_name(), "unsupported");
    assert_eq!(refusal.location.field, "envelope.interface.effects");
    assert!(entries.entries().is_empty());
    Ok(())
}

#[test]
fn operation_witness_must_match_the_ordered_stack() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let body = Recipe {
        nodes: vec![Node::Operation {
            op: Op::Dup,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let mut candidate = envelope(&[Ty::I64], &[Ty::I64, Ty::I64], body, b"dup ");
    candidate.instantiations = vec![Instantiation {
        bindings: vec![Binding {
            variable: 1,
            kind: noble_kernel::words::VariableKind::Value,
            ty: Ty::Bool,
        }],
    }];
    candidate.chunk_id = chunk::chunk_identity_of(&candidate);
    let refusal = entries
        .admit(&session, candidate)
        .expect_err("wrong primitive witness");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.interface");
    assert!(entries.entries().is_empty());
    Ok(())
}

#[test]
fn closed_dependency_and_quotation_witnesses_must_be_empty() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let dependency = envelope(
        &[],
        &[Ty::I64],
        Recipe {
            nodes: vec![Node::I64(1)],
            entry: vec![0],
        },
        b"1 ",
    );
    entries
        .admit(&session, dependency.clone())
        .map_err(|refusal| format!("admit dependency: {refusal:?}"))?;

    reject_nonempty_dependency_witness(&mut entries, &session, &dependency)?;
    reject_nonempty_quotation_witness(&mut entries, &session)
}

fn reject_nonempty_dependency_witness(
    entries: &mut Library,
    session: &noble_contracts::source::Session,
    dependency: &Envelope,
) -> Result<(), String> {
    let mut invoking = envelope(
        &[],
        &[Ty::I64],
        Recipe {
            nodes: vec![Node::Invoke {
                dependency: 0,
                instantiation: 0,
            }],
            entry: vec![0],
        },
        b"1 ",
    );
    invoking.dependencies = vec![definition_ref(dependency)];
    invoking.instantiations = vec![Instantiation {
        bindings: vec![Binding {
            variable: 1,
            kind: noble_kernel::words::VariableKind::Value,
            ty: Ty::I64,
        }],
    }];
    invoking.chunk_id = chunk::chunk_identity_of(&invoking);
    let refusal = entries
        .admit(session, invoking)
        .expect_err("closed dependency witness");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.body.nodes");
    assert_eq!(refusal.location.index, Some(0));
    Ok(())
}

fn reject_nonempty_quotation_witness(
    entries: &mut Library,
    session: &noble_contracts::source::Session,
) -> Result<(), String> {
    let mut quoted = envelope(
        &[],
        &[Ty::I64],
        Recipe {
            nodes: vec![
                Node::I64(1),
                Node::Quotation {
                    body: vec![0],
                    instantiation: 0,
                },
                Node::Operation {
                    op: Op::Run,
                    instantiation: 1,
                },
            ],
            entry: vec![1, 2],
        },
        b"[1] run ",
    );
    quoted.instantiations = vec![
        Instantiation {
            bindings: vec![Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::I64,
            }],
        },
        Instantiation {
            bindings: Vec::new(),
        },
    ];
    quoted.chunk_id = chunk::chunk_identity_of(&quoted);
    let refusal = entries
        .admit(session, quoted)
        .expect_err("closed quotation witness");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.body.nodes");
    assert_eq!(refusal.location.index, Some(1));
    assert_eq!(entries.entries().len(), 1);
    Ok(())
}
