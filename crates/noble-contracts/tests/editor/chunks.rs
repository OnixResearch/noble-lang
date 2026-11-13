//! Typed-chunk envelope and library controls (TCHUNK-01, TCHUNK-03).
//!
//! Every expected value here is written independently of the producer: the
//! canonical source text is spelled out, and each expected interface is stated
//! by the test rather than read back from the library.

use noble_contracts::source::chunks::{
    Binding, Budget, Capture, DefinitionRef, Envelope, Instantiation, Interface, Library, Node, Op,
    Recipe, Response, Scalar, Task,
};
use noble_kernel::types::Ty;

pub(super) const fn limits() -> noble_contracts::Limits {
    noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    }
}

pub(super) fn revision(seed: u8) -> [u8; 32] {
    let mut out = [0u8; 32];
    for (at, slot) in out.iter_mut().enumerate() {
        *slot = seed.wrapping_add(u8::try_from(at).unwrap_or(0));
    }
    out
}

/// The library revision every request in these tests pins.
pub(super) fn pinned() -> [u8; 32] {
    revision(7)
}

/// An envelope whose identity and source revision are left for `seal` to fill.
pub(super) fn envelope(stack_in: &[Ty], stack_out: &[Ty], body: Recipe, source: &[u8]) -> Envelope {
    let mut built = Envelope {
        format: noble_contracts::source::chunks::ENVELOPE_FORMAT,
        semantic_revision: noble_contracts::source::chunks::SEMANTIC_REVISION,
        profile: String::from("pure-i64-bool/1"),
        chunk_id: [0u8; 16],
        source_revision: [0u8; 32],
        interface: Interface {
            stack_in: stack_in.to_vec(),
            stack_out: stack_out.to_vec(),
            effects: noble_kernel::types::EffSet::empty(),
        },
        captures: Vec::new(),
        dependencies: Vec::new(),
        instantiations: if body
            .nodes
            .iter()
            .any(|node| matches!(node, Node::Operation { .. }))
        {
            vec![Instantiation {
                bindings: Vec::new(),
            }]
        } else {
            Vec::new()
        },
        body,
    };
    built.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&built);
    built.source_revision = noble_contracts::source::chunks::canonical_revision(source);
    built
}

pub(super) fn library() -> Library {
    Library::new(pinned())
}

fn definition_ref(envelope: &Envelope) -> DefinitionRef {
    let mut identity = [0u8; 8];
    for (slot, byte) in identity.iter_mut().zip(envelope.chunk_id.iter()) {
        *slot = *byte;
    }
    DefinitionRef {
        identity: u64::from_be_bytes(identity),
        format: envelope.format,
        semantic_revision: envelope.semantic_revision,
        source_revision: envelope.source_revision,
    }
}

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
    let candidate = envelope(&[], &[Ty::Bool, Ty::Bool], body, b"false true swap ");
    let mut candidate = candidate;
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
    candidate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&candidate);
    let session = noble_contracts::source::Session::without_test_hosts();
    entries
        .admit(&session, candidate)
        .map_err(|refusal| format!("admit: {refusal:?}"))?;
    // The same recipe declared with its two results in the other order is wrong,
    // and the interface source admission derives is the one that is retained.
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
    let mut liar = envelope(&[], &[Ty::Bool, Ty::Bool], body, b"false true swap ");
    liar.instantiations = vec![Instantiation {
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
    liar.interface.stack_in = vec![Ty::I64];
    liar.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&liar);
    let refusal = entries
        .admit(&session, liar)
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
    changed.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&changed);
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
    foreign.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&foreign);
    let refusal = entries.admit(&session, foreign).expect_err("text");
    assert_eq!(refusal.class_name(), "unsupported");
    assert_eq!(refusal.location.field, "envelope.interface.stack_out");
    Ok(())
}

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
    candidate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&candidate);
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
    captured.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&captured);
    let index = entries
        .admit(&session, captured.clone())
        .map_err(|refusal| format!("captured scalar: {refusal:?}"))?;
    assert_eq!(index, 0);
    assert_eq!(entries.entries()[0].source(), b"7 ");
    Ok(())
}

#[test]
fn changed_capture_is_rejected_at_the_recomputed_identity() -> Result<(), String> {
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
    candidate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&candidate);
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
    candidate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&candidate);
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
    candidate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&candidate);
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
    invoking.dependencies = vec![definition_ref(&dependency)];
    invoking.instantiations = vec![Instantiation {
        bindings: vec![Binding {
            variable: 1,
            kind: noble_kernel::words::VariableKind::Value,
            ty: Ty::I64,
        }],
    }];
    invoking.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&invoking);
    let refusal = entries
        .admit(&session, invoking)
        .expect_err("closed dependency witness");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.body.nodes");
    assert_eq!(refusal.location.index, Some(0));

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
    quoted.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&quoted);
    let refusal = entries
        .admit(&session, quoted)
        .expect_err("closed quotation witness");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.body.nodes");
    assert_eq!(refusal.location.index, Some(1));
    assert_eq!(entries.entries().len(), 1);
    Ok(())
}

fn candidate_from(body: Recipe) -> Envelope {
    envelope(&[], &[Ty::I64], body, b"7 ")
}

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
    candidate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&candidate);
    let refusal = entries.admit(&session, candidate).expect_err("order");
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
    candidate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&candidate);
    let refusal = entries
        .admit(&session, candidate)
        .expect_err("one witness cannot exceed 16 bindings");
    assert_eq!(refusal.class_name(), "invalid");
    assert_eq!(refusal.location.field, "envelope.instantiations");
    Ok(())
}

#[test]
fn dependency_records_must_resolve_to_an_earlier_entry() -> Result<(), String> {
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
    let record = DefinitionRef {
        identity: u64::from_be_bytes(
            base_envelope.chunk_id[..8]
                .try_into()
                .map_err(|_| String::from("identity width"))?,
        ),
        format: base_envelope.format,
        semantic_revision: base_envelope.semantic_revision,
        source_revision: base_envelope.source_revision,
    };
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
    invoking.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&invoking);
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
    stale.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&stale);
    let refusal = entries.admit(&session, stale).expect_err("stale");
    assert_eq!(refusal.location.field, "envelope.dependencies");
    Ok(())
}

#[test]
fn changed_dependency_is_rejected_by_the_envelope_identity() -> Result<(), String> {
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
    candidate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&candidate);
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
fn equal_signature_chunks_stay_distinct() -> Result<(), String> {
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
    let request = noble_contracts::source::chunks::Request {
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
    let response = noble_contracts::source::chunks::complete(
        &session,
        &entries,
        &request,
        super::monotonic_now_ns,
    );
    let Response::Invalid { refusal, .. } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.library_revision");
    Ok(())
}
