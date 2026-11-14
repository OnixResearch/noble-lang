use super::support::*;
use noble_contracts::source::chunks as chunk;

#[test]
fn recipe_dependency_and_emission_caps_are_shared() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let literal_entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")])?;
    assert_eq!(literal_entries.entries().len(), 1);
    for resource in [
        Resource::RecipeNodes,
        Resource::ExpandedNodes,
        Resource::SourceBytes,
    ] {
        let candidate = with_budget(hole(), vec![0], vec![], vec![Ty::I64], resource);
        assert_eq!(candidate.budget.limit(resource), 0);
        expect_exhausted(&session, &literal_entries, &candidate, resource)?;
    }

    let duplicate = duplicate_chunk();
    let wrapper_body = Recipe {
        nodes: vec![
            Node::Invoke {
                dependency: 0,
                instantiation: 0,
            },
            Node::Operation {
                op: Op::Drop,
                instantiation: 1,
            },
        ],
        entry: vec![0, 1],
    };
    let mut wrapper = envelope(&[Ty::I64], &[Ty::I64], wrapper_body, b"dup drop ");
    wrapper.dependencies = vec![definition_ref(&duplicate)];
    wrapper.instantiations = vec![
        chunk::Instantiation {
            bindings: Vec::new(),
        },
        chunk::Instantiation {
            bindings: vec![chunk::Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::I64,
            }],
        },
    ];
    wrapper.chunk_id = chunk::chunk_identity_of(&wrapper);
    let dependency_entries = admitted(&session, &[duplicate, wrapper])?;
    let candidate = with_budget(
        hole(),
        vec![0],
        vec![Ty::I64],
        vec![Ty::I64, Ty::I64],
        Resource::DependencySteps,
    );
    assert_eq!(dependency_entries.entries().len(), 2);
    assert_eq!(candidate.budget.dependency_steps, 0);
    expect_exhausted(
        &session,
        &dependency_entries,
        &candidate,
        Resource::DependencySteps,
    )?;
    Ok(())
}

#[test]
fn instantiation_binding_cap_is_shared() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let duplicate = duplicate_chunk();
    let entries = admitted(&session, &[duplicate])?;
    let candidate = with_budget(
        hole(),
        vec![0],
        vec![Ty::I64],
        vec![Ty::I64, Ty::I64],
        Resource::InstantiationBindings,
    );
    assert_eq!(entries.entries().len(), 1);
    assert_eq!(candidate.budget.instantiation_bindings, 0);
    expect_exhausted(
        &session,
        &entries,
        &candidate,
        Resource::InstantiationBindings,
    )
}

fn duplicate_chunk() -> Envelope {
    let body = Recipe {
        nodes: vec![Node::Operation {
            op: Op::Dup,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let mut duplicate = envelope(&[Ty::I64], &[Ty::I64, Ty::I64], body, b"dup ");
    duplicate.instantiations = vec![chunk::Instantiation {
        bindings: vec![chunk::Binding {
            variable: 1,
            kind: noble_kernel::words::VariableKind::Value,
            ty: Ty::I64,
        }],
    }];
    duplicate.chunk_id = chunk::chunk_identity_of(&duplicate);
    assert_eq!(duplicate.body.entry, [0]);
    assert_eq!(duplicate.interface.stack_out, [Ty::I64, Ty::I64]);
    duplicate
}

#[test]
fn tree_checking_and_execution_caps_are_shared() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = library();
    let nested = with_budget(
        editor(vec![
            EditorNode::Quotation(vec![EditorNode::Hole]),
            EditorNode::Word(String::from("run")),
        ]),
        vec![0, 0],
        vec![],
        vec![Ty::I64],
        Resource::QuotationDepth,
    );
    assert_eq!(nested.hole_path, [0, 0]);
    expect_exhausted(&session, &entries, &nested, Resource::QuotationDepth)?;

    let checking = with_budget(
        editor(vec![EditorNode::Integer(1), EditorNode::Hole]),
        vec![1],
        vec![],
        vec![Ty::I64],
        Resource::CheckingWork,
    );
    assert_eq!(checking.budget.checking_work, 0);
    expect_exhausted(&session, &entries, &checking, Resource::CheckingWork)?;

    let mut checking_admission = request(hole(), vec![0], vec![Ty::I64], vec![Ty::I64]);
    checking_admission.budget.checking_work = 1;
    let response = complete(
        &session,
        &entries,
        &checking_admission,
        crate::monotonic_now_ns,
    );
    let Response::Exhausted { refusal, .. } = response else {
        return Err(format!(
            "checking work limit should exhaust, got {}",
            response.outcome()
        ));
    };
    assert_eq!(refusal.resource, Some(Resource::CheckingWork));
    assert_eq!(refusal.limit, 1);
    assert!(refusal.observed > refusal.limit, "{refusal:?}");

    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")])?;
    let mut evaluation = request(hole(), vec![0], vec![], vec![Ty::I64]);
    evaluation.fixtures = Some(fixtures(17, vec![(Vec::new(), vec![Scalar::I64(1)])]));
    evaluation.budget = lower_budget(Resource::EvaluationSteps);
    assert_eq!(evaluation.budget.evaluation_steps, 0);
    expect_exhausted(&session, &entries, &evaluation, Resource::EvaluationSteps)?;
    Ok(())
}
