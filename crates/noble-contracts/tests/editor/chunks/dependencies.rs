use super::*;

#[test]
fn instantiation_bindings_must_increase() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let body = Recipe {
        nodes: vec![Node::Bool(true)],
        entry: vec![0],
    };
    let mut candidate = envelope(&[], &[Ty::Bool], body, b"true ");
    candidate.instantiations = vec![Instantiation {
        bindings: vec![
            Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::I64,
            },
            Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::Bool,
            },
        ],
    }];
    candidate.chunk_id = chunk::chunk_identity_of(&candidate);
    let refusal = entries.admit(&session, candidate).expect_err("order");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.instantiations");
    Ok(())
}

#[test]
fn one_instantiation_cannot_exceed_its_binding_bound() -> Result<(), String> {
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
        bindings: (1u16..=17u16)
            .map(|variable| Binding {
                variable,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::I64,
            })
            .collect(),
    }];
    candidate.chunk_id = chunk::chunk_identity_of(&candidate);
    let refusal = entries
        .admit(&session, candidate)
        .expect_err("one witness cannot exceed 16 bindings");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.instantiations");
    Ok(())
}

#[test]
fn records_must_resolve_to_an_earlier_entry() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let base = Recipe {
        nodes: vec![Node::I64(1)],
        entry: vec![0],
    };
    let base_envelope = envelope(&[], &[Ty::I64], base, b"1 ");
    let index = entries
        .admit(&session, base_envelope.clone())
        .map_err(|refusal| format!("admit base: {refusal:?}"))?;
    assert_eq!(index, 0);
    let record = definition_ref(&base_envelope);
    // An invocation of the retained entry resolves and re-emits its body.
    let body = Recipe {
        nodes: vec![
            Node::Invoke {
                dependency: 0,
                instantiation: 0,
            },
            Node::Invoke {
                dependency: 0,
                instantiation: 0,
            },
            Node::Operation {
                op: Op::Add,
                instantiation: 0,
            },
        ],
        entry: vec![0, 1, 2],
    };
    let mut invoking = envelope(&[], &[Ty::I64], body, b"1 1 + ");
    invoking.dependencies = vec![record];
    invoking.instantiations = vec![Instantiation {
        bindings: Vec::new(),
    }];
    invoking.chunk_id = chunk::chunk_identity_of(&invoking);
    let index = entries
        .admit(&session, invoking)
        .map_err(|refusal| format!("admit invoke: {refusal:?}"))?;
    assert_eq!(index, 1);
    // A stale source revision in the record never resolves.
    let body = Recipe {
        nodes: vec![Node::Invoke {
            dependency: 0,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let mut stale = envelope(&[], &[Ty::I64], body, b"1 ");
    stale.dependencies = vec![DefinitionRef {
        source_revision: revision(9),
        ..record
    }];
    stale.instantiations = vec![Instantiation {
        bindings: Vec::new(),
    }];
    stale.chunk_id = chunk::chunk_identity_of(&stale);
    let refusal = entries.admit(&session, stale).expect_err("stale");
    assert_eq!(refusal.location.field, "envelope.dependencies");
    Ok(())
}

#[test]
fn changed_record_is_rejected_by_the_envelope_identity() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let one = envelope(
        &[],
        &[Ty::I64],
        Recipe {
            nodes: vec![Node::I64(1)],
            entry: vec![0],
        },
        b"1 ",
    );
    let two = envelope(
        &[],
        &[Ty::I64],
        Recipe {
            nodes: vec![Node::I64(2)],
            entry: vec![0],
        },
        b"2 ",
    );
    entries
        .admit(&session, one.clone())
        .map_err(|refusal| format!("admit one: {refusal:?}"))?;
    entries
        .admit(&session, two.clone())
        .map_err(|refusal| format!("admit two: {refusal:?}"))?;
    let body = Recipe {
        nodes: vec![Node::Invoke {
            dependency: 0,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let mut candidate = envelope(&[], &[Ty::I64], body, b"1 ");
    candidate.dependencies = vec![definition_ref(&one)];
    candidate.instantiations = vec![Instantiation {
        bindings: Vec::new(),
    }];
    candidate.chunk_id = chunk::chunk_identity_of(&candidate);
    entries
        .admit(&session, candidate.clone())
        .map_err(|refusal| format!("admit dependent entry: {refusal:?}"))?;

    let mut changed = candidate;
    changed.dependencies = vec![definition_ref(&two)];
    let refusal = entries
        .admit(&session, changed)
        .expect_err("changed dependency record");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.chunk_id");
    assert_eq!(entries.entries().len(), 3);
    Ok(())
}

#[test]
fn equal_interfaces_keep_distinct_behavior() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let one = Recipe {
        nodes: vec![Node::I64(1)],
        entry: vec![0],
    };
    let two = Recipe {
        nodes: vec![Node::I64(2)],
        entry: vec![0],
    };
    entries
        .admit(&session, envelope(&[], &[Ty::I64], one, b"1 "))
        .map_err(|refusal| format!("admit one: {refusal:?}"))?;
    entries
        .admit(&session, envelope(&[], &[Ty::I64], two, b"2 "))
        .map_err(|refusal| format!("admit two: {refusal:?}"))?;
    assert_eq!(entries.entries().len(), 2);
    let identities: Vec<[u8; 16]> = entries
        .entries()
        .iter()
        .map(|entry| entry.envelope().chunk_id)
        .collect();
    assert_ne!(identities.first(), identities.get(1));
    Ok(())
}

#[test]
fn the_library_pin_must_match_the_snapshot() -> Result<(), String> {
    let mut entries = library();
    let session = noble_contracts::source::Session::without_test_hosts();
    let body = Recipe {
        nodes: vec![Node::I64(1)],
        entry: vec![0],
    };
    entries
        .admit(&session, envelope(&[], &[Ty::I64], body, b"1 "))
        .map_err(|refusal| format!("admit: {refusal:?}"))?;
    let request = chunk::Request {
        format: 1,
        semantic_revision: 0,
        profile: String::from("pure-i64-bool/1"),
        task: Task {
            stack_in: Vec::new(),
            stack_out: vec![Ty::I64],
            allowed_effects: noble_kernel::types::EffSet::empty(),
        },
        editor: noble_contracts::source::editor::Candidate {
            format: 1,
            nodes: vec![noble_contracts::source::editor::Node::Hole],
        },
        hole_path: vec![0],
        library_revision: revision(1),
        fixtures: None,
        budget: Budget::PROFILE,
    };
    let response = chunk::complete(&session, &entries, &request, super::super::monotonic_now_ns);
    let Response::Invalid { refusal, .. } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.library_revision");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(entries.entries().len(), 1);
    Ok(())
}
