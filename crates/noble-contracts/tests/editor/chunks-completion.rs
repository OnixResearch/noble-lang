//! Completion request, deterministic search and fixture controls
//! (TCHUNK-02, TCHUNK-04).
//!
//! Fixture expectations are written here, independently of any chunk body, and
//! fixture agreement is asserted separately from acceptance: a chunk can be
//! accepted for its interface and still disagree with a host fixture.

use super::chunks::{envelope, library, limits, pinned};
use super::monotonic_now_ns;
use noble_contracts::source::chunks::{
    complete, Accepted, Budget, DefinitionRef, Envelope, Fixture, FixtureSet, Library, Node, Op,
    Recipe, Report, Request, Resource, Response, Scalar, Task,
};
use noble_contracts::source::editor::{Candidate, Node as EditorNode};
use noble_kernel::types::Ty;

fn editor(nodes: Vec<EditorNode>) -> Candidate {
    Candidate { format: 1, nodes }
}

fn hole() -> Candidate {
    editor(vec![EditorNode::Hole])
}

fn request(candidate: Candidate, path: Vec<u32>, stack_in: Vec<Ty>, stack_out: Vec<Ty>) -> Request {
    Request {
        format: 1,
        semantic_revision: 0,
        profile: String::from("pure-i64-bool/1"),
        task: Task {
            stack_in,
            stack_out,
            allowed_effects: noble_kernel::types::EffSet::empty(),
        },
        editor: candidate,
        hole_path: path,
        library_revision: pinned(),
        fixtures: None,
        budget: Budget::PROFILE,
    }
}

fn fixtures(revision_seed: u8, cases: Vec<(Vec<Scalar>, Vec<Scalar>)>) -> FixtureSet {
    let mut id = [0u8; 16];
    for (at, slot) in id.iter_mut().enumerate() {
        *slot = revision_seed.wrapping_add(u8::try_from(at).unwrap_or(0));
    }
    FixtureSet {
        id,
        cases: cases
            .into_iter()
            .map(|(inputs, outputs)| Fixture { inputs, outputs })
            .collect(),
    }
}

fn literal(value: i64, stack_out: &[Ty], source: &'static [u8]) -> Envelope {
    let body = Recipe {
        nodes: vec![Node::I64(value)],
        entry: vec![0],
    };
    envelope(&[], stack_out, body, source)
}

fn arithmetic(values: [i64; 2], op: Op, source: &'static [u8]) -> Envelope {
    let body = Recipe {
        nodes: vec![
            Node::I64(values[0]),
            Node::I64(values[1]),
            Node::Operation {
                op,
                instantiation: 0,
            },
        ],
        entry: vec![0, 1, 2],
    };
    envelope(&[], &[Ty::I64], body, source)
}

fn admitted(namespace: &noble_contracts::source::Session, entries: &[Envelope]) -> Library {
    let mut library = library();
    for candidate in entries {
        library
            .admit(namespace, candidate.clone())
            .unwrap_or_else(|refusal| panic!("admit: {refusal:?}"));
    }
    library
}

fn stack_tail_library(session: &noble_contracts::source::Session) -> (Library, [u8; 16], [u8; 16]) {
    let swap_body = Recipe {
        nodes: vec![Node::Operation {
            op: Op::Swap,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let mut swap = envelope(
        &[Ty::I64, Ty::Bool],
        &[Ty::Bool, Ty::I64],
        swap_body,
        b"swap ",
    );
    swap.instantiations = vec![noble_contracts::source::chunks::Instantiation {
        bindings: vec![
            noble_contracts::source::chunks::Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::I64,
            },
            noble_contracts::source::chunks::Binding {
                variable: 2,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::Bool,
            },
        ],
    }];
    swap.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&swap);

    let duplicate_body = Recipe {
        nodes: vec![Node::Operation {
            op: Op::Dup,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let mut duplicate = envelope(&[Ty::I64], &[Ty::I64, Ty::I64], duplicate_body, b"dup ");
    duplicate.instantiations = vec![noble_contracts::source::chunks::Instantiation {
        bindings: vec![noble_contracts::source::chunks::Binding {
            variable: 1,
            kind: noble_kernel::words::VariableKind::Value,
            ty: Ty::I64,
        }],
    }];
    duplicate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&duplicate);
    (
        admitted(session, &[swap.clone(), duplicate.clone()]),
        swap.chunk_id,
        duplicate.chunk_id,
    )
}

fn maybe_write_measurement_receipt(accepted: &Accepted, report: &Report) -> Result<(), String> {
    let Some(path) = std::env::var_os("NOBLE_TYPED_CHUNK_RECEIPT") else {
        return Ok(());
    };
    let source_commit = std::env::var("NOBLE_TYPED_CHUNK_SOURCE_COMMIT")
        .map_err(|error| format!("source commit environment: {error}"))?;
    let worktree = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or_else(|| String::from("worktree root unavailable"))?;
    let head = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|error| format!("read source commit: {error}"))?;
    if !head.status.success() {
        return Err(String::from("git rev-parse HEAD failed"));
    }
    let head =
        String::from_utf8(head.stdout).map_err(|error| format!("decode source commit: {error}"))?;
    if source_commit.trim() != head.trim() {
        return Err(String::from("receipt source commit differs from HEAD"));
    }
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["status", "--porcelain"])
        .output()
        .map_err(|error| format!("check source tree: {error}"))?;
    if !status.status.success() || !status.stdout.is_empty() {
        return Err(String::from("receipt source tree is not clean"));
    }
    let worktree = std::fs::canonicalize(worktree)
        .map_err(|error| format!("canonicalize worktree: {error}"))?;
    let output = std::path::PathBuf::from(path);
    if !output.is_absolute() {
        return Err(String::from("receipt output path must be absolute"));
    }
    let output_parent = output
        .parent()
        .ok_or_else(|| String::from("receipt output parent unavailable"))?;
    let output_parent = std::fs::canonicalize(output_parent)
        .map_err(|error| format!("canonicalize receipt parent: {error}"))?;
    if output_parent.starts_with(worktree) {
        return Err(String::from("receipt output must be outside the worktree"));
    }
    let receipt = format!(
        concat!(
            "{{\n",
            "  \"schema\": \"typed-chunk-measurement/1\",\n",
            "  \"source_commit\": \"{}\",\n",
            "  \"scenario\": \"two-chunk-stack-tail\",\n",
            "  \"outcome\": \"accepted\",\n",
            "  \"enumerated_candidates\": {},\n",
            "  \"checked_candidates\": {},\n",
            "  \"rejected_candidates\": {},\n",
            "  \"expanded_nodes\": {},\n",
            "  \"dependency_steps\": {},\n",
            "  \"instantiation_bindings\": {},\n",
            "  \"checking_work\": {},\n",
            "  \"evaluation_steps\": {},\n",
            "  \"source_bytes\": {},\n",
            "  \"retained_bytes\": {},\n",
            "  \"completed_source_bytes\": {},\n",
            "  \"selected_chunks\": {},\n",
            "  \"total_latency_ns\": {}\n",
            "}}\n"
        ),
        source_commit,
        report.candidates,
        report.checked_candidates,
        report.rejected,
        report.expanded_nodes,
        report.dependency_steps,
        report.instantiation_bindings,
        report.checking_work,
        report.evaluation_steps,
        report.source_bytes,
        report.retained_bytes,
        accepted.source.len(),
        accepted.selected.len(),
        report.total_latency_ns,
    );
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| format!("create new measurement receipt: {error}"))?;
    std::io::Write::write_all(&mut output, receipt.as_bytes())
        .map_err(|error| format!("write measurement receipt: {error}"))
}

fn lower_budget(resource: Resource, limit: u32) -> Budget {
    let mut budget = Budget::PROFILE;
    match resource {
        Resource::LibraryBytes => budget.library_bytes = limit,
        Resource::Entries => budget.entries = limit,
        Resource::RecipeNodes => budget.recipe_nodes = limit,
        Resource::StackHeight => budget.stack_height = limit,
        Resource::DependencySteps => budget.dependency_steps = limit,
        Resource::InstantiationBindings => budget.instantiation_bindings = limit,
        Resource::QuotationDepth => budget.quotation_depth = limit,
        Resource::ExpandedNodes => budget.expanded_nodes = limit,
        Resource::ComposedChunks => budget.composed_chunks = limit,
        Resource::SourceBytes => budget.source_bytes = limit,
        Resource::Candidates => budget.candidates = limit,
        Resource::Diagnostics => budget.diagnostics = limit,
        Resource::CheckingWork => budget.checking_work = limit,
        Resource::EvaluationSteps => budget.evaluation_steps = limit,
        Resource::RetainedBytes => budget.retained_bytes = limit,
    }
    budget
}

fn expect_exhausted(
    session: &noble_contracts::source::Session,
    entries: &Library,
    request: &Request,
    resource: Resource,
) -> Result<(), String> {
    expect_exhausted_at(session, entries, request, resource, 0)
}

fn expect_exhausted_at(
    session: &noble_contracts::source::Session,
    entries: &Library,
    request: &Request,
    resource: Resource,
    expected_limit: u32,
) -> Result<(), String> {
    let response = complete(session, entries, request, monotonic_now_ns);
    let Response::Exhausted { refusal, .. } = response else {
        return Err(format!(
            "{} limit should exhaust, got {}",
            resource.name(),
            response.outcome()
        ));
    };
    assert_eq!(refusal.resource, Some(resource));
    assert_eq!(refusal.limit, expected_limit);
    assert!(refusal.observed > refusal.limit, "{refusal:?}");
    Ok(())
}

fn with_budget(
    editor: Candidate,
    path: Vec<u32>,
    stack_in: Vec<Ty>,
    stack_out: Vec<Ty>,
    resource: Resource,
) -> Request {
    let mut request = request(editor, path, stack_in, stack_out);
    request.budget = lower_budget(resource, 0);
    request
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
fn two_chunks_compose_over_a_retained_tail() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let (entries, swap_id, duplicate_id) = stack_tail_library(&session);
    let candidate = request(
        hole(),
        vec![0],
        vec![Ty::Bool, Ty::I64, Ty::Bool],
        vec![Ty::Bool, Ty::Bool, Ty::I64, Ty::I64],
    );
    let generation = session.generation();
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(accepted.source, b"swap dup ");
    assert_eq!(accepted.selected.len(), 2);
    assert_eq!(accepted.selected, vec![swap_id, duplicate_id]);
    assert_eq!(
        accepted.equation.stack_in,
        vec![Ty::Bool, Ty::I64, Ty::Bool]
    );
    assert_eq!(
        accepted.equation.stack_out,
        vec![Ty::Bool, Ty::Bool, Ty::I64, Ty::I64]
    );
    assert_eq!(accepted.equation.tail, 1);
    assert_eq!(report.candidates, 6);
    assert!(report.rejected >= 5);
    assert_eq!(session.generation(), generation);
    // The completion is fresh source: preparing it again against the same task
    // is what accepted it, and no program was committed.
    let prepared = session
        .prepare(&accepted.source, &candidate.task.stack_in, limits())
        .map_err(|error| format!("prepare: {error:?}"))?;
    assert_eq!(prepared.output(), [Ty::Bool, Ty::Bool, Ty::I64, Ty::I64]);
    Ok(())
}

#[test]
fn a_later_chunk_can_consume_the_initial_stack_tail() -> Result<(), String> {
    use noble_contracts::source::chunks::{Binding, Instantiation};
    let session = noble_contracts::source::Session::without_test_hosts();
    let duplicate_body = Recipe {
        nodes: vec![Node::Operation {
            op: Op::Dup,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let mut duplicate = envelope(&[Ty::Bool], &[Ty::Bool, Ty::Bool], duplicate_body, b"dup ");
    duplicate.instantiations = vec![Instantiation {
        bindings: vec![Binding {
            variable: 1,
            kind: noble_kernel::words::VariableKind::Value,
            ty: Ty::Bool,
        }],
    }];
    duplicate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&duplicate);

    let drop_body = Recipe {
        nodes: vec![
            Node::Operation {
                op: Op::Drop,
                instantiation: 0,
            },
            Node::Operation {
                op: Op::Drop,
                instantiation: 0,
            },
            Node::Operation {
                op: Op::Drop,
                instantiation: 1,
            },
        ],
        entry: vec![0, 1, 2],
    };
    let mut drop_all = envelope(
        &[Ty::I64, Ty::Bool, Ty::Bool],
        &[],
        drop_body,
        b"drop drop drop ",
    );
    drop_all.instantiations = vec![
        Instantiation {
            bindings: vec![Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::Bool,
            }],
        },
        Instantiation {
            bindings: vec![Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::I64,
            }],
        },
    ];
    drop_all.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&drop_all);

    let entries = admitted(&session, &[duplicate.clone(), drop_all.clone()]);
    let candidate = request(hole(), vec![0], vec![Ty::I64, Ty::Bool], Vec::new());
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, .. } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(accepted.source, b"dup drop drop drop ");
    assert_eq!(
        accepted.selected,
        vec![duplicate.chunk_id, drop_all.chunk_id]
    );
    assert!(accepted.equation.stack_out.is_empty());
    assert_eq!(accepted.equation.tail, 0);
    Ok(())
}

#[test]
fn fixed_baseline_measurement_reports_bounded_work() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let (entries, _, _) = stack_tail_library(&session);
    let candidate = request(
        hole(),
        vec![0],
        vec![Ty::Bool, Ty::I64, Ty::Bool],
        vec![Ty::Bool, Ty::Bool, Ty::I64, Ty::I64],
    );
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(accepted.source, b"swap dup ");
    assert_eq!(report.candidates, 6);
    assert!(report.checked_candidates > 0);
    assert!(report.expanded_nodes > 0);
    assert!(report.checking_work > 0);
    assert!(report.checking_work <= Budget::PROFILE.checking_work);
    assert!(report.source_bytes > 0);
    assert!(report.retained_bytes > 0);
    assert!(report.total_latency_ns > 0);
    maybe_write_measurement_receipt(&accepted, &report)?;
    Ok(())
}

#[test]
fn shared_budget_refuses_snapshot_retention_and_stack_boundaries() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")]);
    for resource in [
        Resource::LibraryBytes,
        Resource::Entries,
        Resource::RetainedBytes,
        Resource::StackHeight,
    ] {
        let candidate = with_budget(hole(), vec![0], vec![], vec![Ty::I64], resource);
        expect_exhausted(&session, &entries, &candidate, resource)?;
    }
    Ok(())
}

#[test]
fn editor_context_source_lower_bound_exhausts_before_analysis() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let mut nodes = Vec::with_capacity(2_050);
    nodes.push(EditorNode::Hole);
    for _ in 0..2_049 {
        nodes.push(EditorNode::Integer(1));
    }
    let candidate = request(editor(nodes), vec![0], Vec::new(), Vec::new());
    let response = complete(&session, &library(), &candidate, monotonic_now_ns);
    let Response::Exhausted { refusal, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(refusal.resource, Some(Resource::SourceBytes));
    assert_eq!(refusal.location.field, "request.editor");
    assert_eq!(refusal.observed, 4_098);
    assert_eq!(refusal.limit, Budget::PROFILE.source_bytes);
    assert_eq!(report.candidates, 0);
    Ok(())
}

#[test]
fn shared_budget_refuses_enumeration_and_diagnostic_boundaries() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")]);
    let candidate = with_budget(hole(), vec![0], vec![], vec![Ty::I64], Resource::Candidates);
    expect_exhausted(&session, &entries, &candidate, Resource::Candidates)?;

    let candidate = with_budget(
        hole(),
        vec![0],
        vec![],
        vec![Ty::I64],
        Resource::ComposedChunks,
    );
    expect_exhausted(&session, &entries, &candidate, Resource::ComposedChunks)?;

    let candidate = with_budget(
        hole(),
        vec![0],
        vec![],
        vec![Ty::Bool],
        Resource::Diagnostics,
    );
    expect_exhausted(&session, &entries, &candidate, Resource::Diagnostics)?;
    Ok(())
}

#[test]
fn shared_budget_refuses_recipe_expansion_dependency_and_emission() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let literal_entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")]);
    for resource in [
        Resource::RecipeNodes,
        Resource::ExpandedNodes,
        Resource::SourceBytes,
    ] {
        let candidate = with_budget(hole(), vec![0], vec![], vec![Ty::I64], resource);
        expect_exhausted(&session, &literal_entries, &candidate, resource)?;
    }

    let duplicate_body = Recipe {
        nodes: vec![Node::Operation {
            op: Op::Dup,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let mut duplicate = envelope(&[Ty::I64], &[Ty::I64, Ty::I64], duplicate_body, b"dup ");
    duplicate.instantiations = vec![noble_contracts::source::chunks::Instantiation {
        bindings: vec![noble_contracts::source::chunks::Binding {
            variable: 1,
            kind: noble_kernel::words::VariableKind::Value,
            ty: Ty::I64,
        }],
    }];
    duplicate.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&duplicate);
    let duplicate_entries = admitted(&session, &[duplicate.clone()]);
    let candidate = with_budget(
        hole(),
        vec![0],
        vec![Ty::I64],
        vec![Ty::I64, Ty::I64],
        Resource::InstantiationBindings,
    );
    expect_exhausted(
        &session,
        &duplicate_entries,
        &candidate,
        Resource::InstantiationBindings,
    )?;

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
        noble_contracts::source::chunks::Instantiation {
            bindings: Vec::new(),
        },
        noble_contracts::source::chunks::Instantiation {
            bindings: vec![noble_contracts::source::chunks::Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::I64,
            }],
        },
    ];
    wrapper.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&wrapper);
    let dependency_entries = admitted(&session, &[duplicate, wrapper]);
    let candidate = with_budget(
        hole(),
        vec![0],
        vec![Ty::I64],
        vec![Ty::I64, Ty::I64],
        Resource::DependencySteps,
    );
    expect_exhausted(
        &session,
        &dependency_entries,
        &candidate,
        Resource::DependencySteps,
    )?;
    Ok(())
}

#[test]
fn shared_budget_refuses_tree_checking_and_execution_boundaries() -> Result<(), String> {
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
    expect_exhausted(&session, &entries, &nested, Resource::QuotationDepth)?;

    let checking = with_budget(
        editor(vec![EditorNode::Integer(1), EditorNode::Hole]),
        vec![1],
        vec![],
        vec![Ty::I64],
        Resource::CheckingWork,
    );
    expect_exhausted(&session, &entries, &checking, Resource::CheckingWork)?;

    let mut checking_admission = request(hole(), vec![0], vec![Ty::I64], vec![Ty::I64]);
    checking_admission.budget = lower_budget(Resource::CheckingWork, 1);
    expect_exhausted_at(
        &session,
        &entries,
        &checking_admission,
        Resource::CheckingWork,
        1,
    )?;

    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")]);
    let mut evaluation = request(hole(), vec![0], vec![], vec![Ty::I64]);
    evaluation.fixtures = Some(fixtures(17, vec![(Vec::new(), vec![Scalar::I64(1)])]));
    evaluation.budget = lower_budget(Resource::EvaluationSteps, 0);
    expect_exhausted(&session, &entries, &evaluation, Resource::EvaluationSteps)?;
    Ok(())
}

#[test]
fn oversized_fixture_request_is_refused_before_candidate_enumeration() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = library();
    let mut candidate = request(hole(), vec![0], vec![], vec![]);
    candidate.fixtures = Some(fixtures(
        19,
        (0..33_000).map(|_| (Vec::new(), Vec::new())).collect(),
    ));
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Exhausted { refusal, report } = response else {
        return Err(format!("retained-byte bound: {}", response.outcome()));
    };
    assert_eq!(refusal.resource, Some(Resource::RetainedBytes));
    assert_eq!(refusal.location.field, "request.retained");
    assert_eq!(refusal.limit, Budget::PROFILE.retained_bytes);
    assert!(refusal.observed > refusal.limit, "{refusal:?}");
    assert_eq!(report.candidates, 0);
    Ok(())
}

#[test]
fn retained_fixture_report_cannot_bypass_the_shared_byte_budget() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = library();
    let stack = vec![Ty::I64; 32];
    let values: Vec<Scalar> = (0..32).map(|value| Scalar::I64(i64::from(value))).collect();
    let mut candidate = request(hole(), vec![0], stack.clone(), stack);
    candidate.fixtures = Some(fixtures(
        91,
        (0..100).map(|_| (values.clone(), values.clone())).collect(),
    ));
    candidate.budget.retained_bytes = 100_000;

    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Exhausted { refusal, report } = response else {
        return Err(format!(
            "retained evidence should exhaust, got {}",
            response.outcome()
        ));
    };
    assert_eq!(refusal.resource, Some(Resource::RetainedBytes));
    assert_eq!(refusal.limit, 100_000);
    assert!(refusal.observed > refusal.limit);
    assert_eq!(report.candidates, 1);
    assert_eq!(report.evaluation_steps, 0);
    assert!(report.fixtures.is_none());
    Ok(())
}

#[test]
fn raised_request_budgets_are_clamped_to_the_profile() {
    let mut raised = Budget::PROFILE;
    raised.candidates = u32::MAX;
    raised.evaluation_steps = u32::MAX;
    let clamped = raised.lowered(&Budget::PROFILE);
    assert_eq!(clamped.candidates, Budget::PROFILE.candidates);
    assert_eq!(clamped.evaluation_steps, Budget::PROFILE.evaluation_steps);
}

#[test]
fn the_257th_fixture_step_exhausts_at_the_256_step_limit() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let mut candidate = request(
        editor(vec![EditorNode::Integer(1), EditorNode::Hole]),
        vec![1],
        vec![],
        vec![Ty::I64],
    );
    candidate.fixtures = Some(fixtures(
        18,
        (0..129)
            .map(|_| (Vec::new(), vec![Scalar::I64(1)]))
            .collect(),
    ));
    let response = complete(&session, &library(), &candidate, monotonic_now_ns);
    let Response::Exhausted { refusal, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(refusal.resource, Some(Resource::EvaluationSteps));
    assert_eq!(refusal.observed, 257);
    assert_eq!(refusal.limit, 256);
    assert_eq!(report.evaluation_steps, 256);
    Ok(())
}

#[test]
fn nested_quotation_completion_is_reparsed_as_source() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")]);
    let candidate = request(
        editor(vec![
            EditorNode::Quotation(vec![EditorNode::Hole]),
            EditorNode::Word(String::from("run")),
        ]),
        vec![0, 0],
        vec![],
        vec![Ty::I64],
    );
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(accepted.source, b"[ 1 ] run ");
    assert_eq!(accepted.selected.len(), 1);
    assert_eq!(report.quotation_depth, 1);
    Ok(())
}

#[test]
fn wrapping_i64_boundaries_agree_with_independent_fixtures() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    type ArithmeticFixture = (Op, &'static [u8], Vec<(i64, i64, i64)>);
    let cases: [ArithmeticFixture; 3] = [
        (
            Op::Add,
            b"+ ",
            vec![
                (i64::MAX, 1, i64::MIN),
                (i64::MIN, -1, i64::MAX),
                (-1, 1, 0),
                (0, 1, 1),
                (1, 0, 1),
            ],
        ),
        (
            Op::Sub,
            b"- ",
            vec![
                (i64::MIN, 1, i64::MAX),
                (i64::MAX, -1, i64::MIN),
                (1, 0, 1),
                (0, 1, -1),
                (1, 3, -2),
            ],
        ),
        (
            Op::Mul,
            b"* ",
            vec![
                (i64::MIN, -1, i64::MIN),
                (i64::MAX, 1, i64::MAX),
                (-1, -1, 1),
                (0, i64::MAX, 0),
                (1, i64::MIN, i64::MIN),
            ],
        ),
    ];
    for (op, source, values) in cases {
        let recipe = Recipe {
            nodes: vec![Node::Operation {
                op,
                instantiation: 0,
            }],
            entry: vec![0],
        };
        let chunk = envelope(&[Ty::I64, Ty::I64], &[Ty::I64], recipe, source);
        let entries = admitted(&session, &[chunk]);
        let mut candidate = request(hole(), vec![0], vec![Ty::I64, Ty::I64], vec![Ty::I64]);
        candidate.fixtures = Some(fixtures(
            4,
            values
                .iter()
                .map(|(left, right, expected)| {
                    (
                        vec![Scalar::I64(*left), Scalar::I64(*right)],
                        vec![Scalar::I64(*expected)],
                    )
                })
                .collect(),
        ));
        let response = complete(&session, &entries, &candidate, monotonic_now_ns);
        let Response::Accepted { accepted, report } = response else {
            return Err(format!("{source:?}: {}", response.outcome()));
        };
        assert_eq!(accepted.source, source);
        let reported = report
            .fixtures
            .as_ref()
            .ok_or_else(|| String::from("fixture report"))?;
        assert!(reported.agreement, "{reported:?}");
        for (case, (left, right, expected)) in reported.cases.iter().zip(values) {
            assert_eq!(case.inputs, [Scalar::I64(left), Scalar::I64(right)]);
            assert_eq!(case.expected, [Scalar::I64(expected)]);
            assert_eq!(case.actual, Some(vec![Scalar::I64(expected)]));
            assert!(case.agrees);
        }
    }
    Ok(())
}

#[test]
fn boolean_results_have_independent_true_and_false_expectations() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    for (left, right, expected, source) in [
        (1, 1, true, b"1 1 = ".as_slice()),
        (1, 2, false, b"1 2 = ".as_slice()),
    ] {
        let recipe = Recipe {
            nodes: vec![
                Node::I64(left),
                Node::I64(right),
                Node::Operation {
                    op: Op::Eq,
                    instantiation: 0,
                },
            ],
            entry: vec![0, 1, 2],
        };
        let entries = admitted(&session, &[envelope(&[], &[Ty::Bool], recipe, source)]);
        let mut candidate = request(hole(), vec![0], vec![], vec![Ty::Bool]);
        candidate.fixtures = Some(fixtures(
            6,
            vec![(Vec::new(), vec![Scalar::Bool(expected)])],
        ));
        let response = complete(&session, &entries, &candidate, monotonic_now_ns);
        let Response::Accepted { accepted, report } = response else {
            return Err(format!("{source:?}: {}", response.outcome()));
        };
        assert_eq!(accepted.source, source);
        let fixtures = report
            .fixtures
            .as_ref()
            .ok_or_else(|| String::from("fixture report"))?;
        assert!(fixtures.agreement, "{fixtures:?}");
        assert_eq!(fixtures.cases[0].expected, [Scalar::Bool(expected)]);
        assert_eq!(fixtures.cases[0].actual, Some(vec![Scalar::Bool(expected)]));
    }
    Ok(())
}

#[test]
fn subtraction_operand_order_is_not_guessed() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[arithmetic([10, 3], Op::Sub, b"10 3 - ")]);
    let mut candidate = request(hole(), vec![0], vec![], vec![Ty::I64]);
    candidate.fixtures = Some(fixtures(5, vec![(Vec::new(), vec![Scalar::I64(7)])]));
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { report, .. } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    let reported = report
        .fixtures
        .as_ref()
        .ok_or_else(|| String::from("fixture report"))?;
    assert!(reported.agreement, "{reported:?}");
    assert_eq!(
        reported.cases.first().and_then(|case| case.actual.clone()),
        Some(vec![Scalar::I64(7)])
    );
    Ok(())
}

#[test]
fn equal_signatures_do_not_transfer_behavior() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let agree = {
        let body = Recipe {
            nodes: vec![
                Node::I64(1),
                Node::I64(1),
                Node::Operation {
                    op: Op::Eq,
                    instantiation: 0,
                },
            ],
            entry: vec![0, 1, 2],
        };
        envelope(&[], &[Ty::Bool], body, b"1 1 = ")
    };
    let differ = {
        let body = Recipe {
            nodes: vec![
                Node::I64(1),
                Node::I64(2),
                Node::Operation {
                    op: Op::Eq,
                    instantiation: 0,
                },
            ],
            entry: vec![0, 1, 2],
        };
        envelope(&[], &[Ty::Bool], body, b"1 2 = ")
    };
    for (chunk, expected, agreement) in [
        (agree.clone(), Scalar::Bool(true), true),
        (differ.clone(), Scalar::Bool(true), false),
    ] {
        let entries = admitted(&session, &[chunk]);
        let mut candidate = request(hole(), vec![0], vec![], vec![Ty::Bool]);
        candidate.fixtures = Some(fixtures(6, vec![(Vec::new(), vec![expected])]));
        let response = complete(&session, &entries, &candidate, monotonic_now_ns);
        let Response::Accepted { accepted, report } = response else {
            return Err(format!("outcome: {}", response.outcome()));
        };
        // The same ordered interface is accepted in both libraries; the host
        // fixture separately reports that one of them behaves differently.
        assert_eq!(accepted.interface.stack_out, vec![Ty::Bool]);
        let reported = report
            .fixtures
            .as_ref()
            .ok_or_else(|| String::from("fixture report"))?;
        assert_eq!(reported.agreement, agreement, "{reported:?}");
    }
    // Both bodies remain distinct entries when a library holds both.
    let entries = admitted(&session, &[agree, differ]);
    assert_eq!(entries.entries().len(), 2);
    Ok(())
}

#[test]
fn canonical_order_selects_the_first_equal_interface_chunk() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let one = literal(1, &[Ty::I64], b"1 ");
    let two = literal(2, &[Ty::I64], b"2 ");
    let entries = admitted(&session, &[one, two]);
    let first = entries
        .entries()
        .iter()
        .min_by(|left, right| left.envelope().chunk_id.cmp(&right.envelope().chunk_id))
        .ok_or_else(|| String::from("entries"))?
        .envelope()
        .chunk_id;
    let value = if entries
        .entries()
        .first()
        .is_some_and(|entry| entry.envelope().chunk_id == first)
    {
        Scalar::I64(1)
    } else {
        Scalar::I64(2)
    };
    let mut candidate = request(hole(), vec![0], vec![], vec![Ty::I64]);
    candidate.fixtures = Some(fixtures(8, vec![(Vec::new(), vec![value])]));
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Accepted { accepted, report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(accepted.selected, vec![first]);
    let reported = report
        .fixtures
        .as_ref()
        .ok_or_else(|| String::from("fixture report"))?;
    assert!(reported.agreement, "{reported:?}");
    Ok(())
}

#[test]
fn no_result_is_not_a_completeness_claim() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")]);
    let candidate = request(
        editor(vec![
            EditorNode::Hole,
            EditorNode::Word(String::from("drop")),
            EditorNode::Word(String::from("drop")),
            EditorNode::Word(String::from("=")),
            EditorNode::Word(String::from("swap")),
        ]),
        vec![0],
        vec![],
        vec![Ty::Bool, Ty::I64],
    );
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::NoResult { report } = response else {
        return Err(format!("outcome: {}", response.outcome()));
    };
    assert_eq!(report.candidates, 5);
    assert_eq!(report.rejected, 5);
    assert!(!report.diagnostics.is_empty());
    let longer_source = b"1 1 1 1 1 drop drop = swap ";
    let prepared = session
        .prepare(longer_source, &[], limits())
        .map_err(|error| format!("five-chunk completion: {error:?}"))?;
    assert_eq!(prepared.output(), [Ty::Bool, Ty::I64]);
    Ok(())
}

#[test]
fn lowered_candidate_limit_exhausts_before_the_next_proposal() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")]);
    let mut candidate = request(
        editor(vec![
            EditorNode::Hole,
            EditorNode::Word(String::from("swap")),
        ]),
        vec![0],
        vec![],
        vec![Ty::Bool, Ty::I64],
    );
    candidate.budget = lower_budget(Resource::Candidates, 2);
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    let Response::Exhausted { refusal, report } = response else {
        return Err(format!("candidate budget: {}", response.outcome()));
    };
    assert_eq!(
        refusal.resource,
        Some(Resource::Candidates),
        "refusal={refusal:?}, candidates={}, checking_work={}, rejected={}",
        report.candidates,
        report.checking_work,
        report.rejected,
    );
    assert_eq!(Budget::PROFILE.candidates, 128);
    assert_eq!(refusal.observed, 3);
    assert_eq!(refusal.limit, 2);
    assert_eq!(report.candidates, 2);
    assert_eq!(report.rejected, 2);
    assert!(!report.diagnostics.is_empty());
    Ok(())
}

#[test]
fn malformed_requests_are_classified_before_any_proposal() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")]);

    let two_holes = request(
        editor(vec![EditorNode::Hole, EditorNode::Hole]),
        vec![0],
        vec![],
        vec![Ty::I64],
    );
    let response = complete(&session, &entries, &two_holes, monotonic_now_ns);
    let Response::Invalid { refusal, report } = response else {
        return Err(format!("two holes: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.editor");
    assert_eq!(report.candidates, 0);

    let no_hole = request(
        editor(vec![EditorNode::Integer(1)]),
        vec![0],
        vec![],
        vec![Ty::I64],
    );
    let response = complete(&session, &entries, &no_hole, monotonic_now_ns);
    let Response::Invalid { refusal, .. } = response else {
        return Err(format!("no hole: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.editor");

    let wrong_path = request(hole(), vec![3], vec![], vec![Ty::I64]);
    let response = complete(&session, &entries, &wrong_path, monotonic_now_ns);
    let Response::Invalid { refusal, .. } = response else {
        return Err(format!("wrong path: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.hole_path");

    let foreign = request(
        editor(vec![
            EditorNode::Quotation(vec![EditorNode::Hole]),
            EditorNode::Word(String::from("test.emit")),
        ]),
        vec![0, 0],
        vec![],
        vec![Ty::I64],
    );
    let response = complete(&session, &entries, &foreign, monotonic_now_ns);
    let Response::Unsupported { refusal, .. } = response else {
        return Err(format!("foreign word: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.editor");

    let mut newer = request(hole(), vec![0], vec![], vec![Ty::I64]);
    newer.editor.format = 2;
    let response = complete(&session, &entries, &newer, monotonic_now_ns);
    assert_eq!(response.outcome(), "unsupported");

    let mut profile = request(hole(), vec![0], vec![], vec![Ty::I64]);
    profile.profile = String::from("pure-i64-bool/2");
    let response = complete(&session, &entries, &profile, monotonic_now_ns);
    let Response::Unsupported { refusal, .. } = response else {
        return Err(format!("profile: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.profile");

    let text = request(
        editor(vec![EditorNode::Hole]),
        vec![0],
        vec![],
        vec![Ty::Text],
    );
    let response = complete(&session, &entries, &text, monotonic_now_ns);
    let Response::Unsupported { refusal, .. } = response else {
        return Err(format!("text task: {}", response.outcome()));
    };
    assert_eq!(refusal.location.field, "request.task.stack_out");
    Ok(())
}

#[test]
fn an_empty_sequence_is_the_identity_completion() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")]);
    let candidate = request(
        editor(vec![EditorNode::Integer(2), EditorNode::Hole]),
        vec![1],
        vec![],
        vec![Ty::I64, Ty::I64],
    );
    // With two literals and a hole, the empty replacement leaves `2` alone and
    // does not reach `I64`; one literal chunk completes it.
    let response = complete(&session, &entries, &candidate, monotonic_now_ns);
    assert_eq!(response.outcome(), "accepted");
    let Response::Accepted { accepted, .. } = response else {
        return Ok(());
    };
    assert_eq!(accepted.source, b"2 1 ");
    let empty = request(
        editor(vec![EditorNode::Hole]),
        vec![0],
        vec![Ty::Bool],
        vec![Ty::Bool],
    );
    let response = complete(&session, &entries, &empty, monotonic_now_ns);
    let Response::Accepted { accepted, .. } = response else {
        return Err(format!("identity: {}", response.outcome()));
    };
    assert!(accepted.selected.is_empty());
    assert_eq!(accepted.equation.tail, 1);
    Ok(())
}

#[test]
fn operations_outside_the_whitelist_are_not_search_surface() -> Result<(), String> {
    let session = noble_contracts::source::Session::without_test_hosts();
    let entries = admitted(&session, &[literal(1, &[Ty::I64], b"1 ")]);
    // `dip`, `quote`, `compose` and `if` exist in the namespace but are outside
    // the profile, so a request cannot route a completion through them.
    for word in ["dip", "quote", "compose", "if", "over", "not", "reflect"] {
        let candidate = request(
            editor(vec![EditorNode::Hole, EditorNode::Word(String::from(word))]),
            vec![0],
            vec![],
            vec![Ty::I64],
        );
        let response = complete(&session, &entries, &candidate, monotonic_now_ns);
        assert_eq!(
            response.outcome(),
            "unsupported",
            "{word}: {}",
            response.outcome()
        );
    }
    Ok(())
}

#[test]
fn polymorphic_recipe_is_freshly_instantiated_for_each_value_type() -> Result<(), String> {
    use noble_contracts::source::chunks::{Binding, Instantiation};
    let session = noble_contracts::source::Session::without_test_hosts();
    let duplicate = |ty: Ty| {
        let body = Recipe {
            nodes: vec![Node::Operation {
                op: Op::Dup,
                instantiation: 0,
            }],
            entry: vec![0],
        };
        let output = vec![ty.clone(), ty.clone()];
        let mut chunk = envelope(core::slice::from_ref(&ty), &output, body, b"dup ");
        chunk.instantiations = vec![Instantiation {
            bindings: vec![Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty,
            }],
        }];
        chunk.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&chunk);
        chunk
    };
    let mut entries = library();
    let boolean = duplicate(Ty::Bool);
    let integer = duplicate(Ty::I64);
    entries
        .admit(&session, boolean.clone())
        .map_err(|refusal| format!("bool instantiation: {refusal:?}"))?;
    entries
        .admit(&session, integer.clone())
        .map_err(|refusal| format!("I64 instantiation: {refusal:?}"))?;
    assert_eq!(boolean.source_revision, integer.source_revision);
    assert_ne!(boolean.chunk_id, integer.chunk_id);
    for (input, output, ty, selected_id, expected_bindings) in [
        (
            vec![Ty::Bool],
            vec![Ty::Bool, Ty::Bool],
            Ty::Bool,
            boolean.chunk_id,
            2,
        ),
        (
            vec![Ty::I64],
            vec![Ty::I64, Ty::I64],
            Ty::I64,
            integer.chunk_id,
            1,
        ),
    ] {
        let candidate = request(hole(), vec![0], input.clone(), output.clone());
        let response = complete(&session, &entries, &candidate, monotonic_now_ns);
        let Response::Accepted { accepted, report } = response else {
            return Err(format!("{ty:?}: {}", response.outcome()));
        };
        assert_eq!(accepted.interface.stack_in, input);
        assert_eq!(accepted.interface.stack_out, output);
        assert_eq!(accepted.source, b"dup ");
        assert_eq!(accepted.selected, vec![selected_id]);
        assert_eq!(report.instantiation_bindings, expected_bindings);
    }
    Ok(())
}
