use super::super::super::monotonic_now_ns;
use super::{envelope, library, pinned};
use noble_contracts::source::chunks as chunk;
use noble_contracts::source::chunks::{
    complete, Budget, DefinitionRef, Envelope, Fixture, FixtureSet, Library, Node, Op, Recipe,
    Request, Resource, Response, Scalar, Task,
};
use noble_contracts::source::editor::{Candidate, Node as EditorNode};
use noble_kernel::types::Ty;

pub(crate) fn editor(nodes: Vec<EditorNode>) -> Candidate {
    Candidate { format: 1, nodes }
}

pub(crate) fn hole() -> Candidate {
    editor(vec![EditorNode::Hole])
}

pub(crate) fn request(
    candidate: Candidate,
    path: Vec<u32>,
    stack_in: Vec<Ty>,
    stack_out: Vec<Ty>,
) -> Request {
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

pub(crate) fn fixtures(revision_seed: u8, cases: Vec<(Vec<Scalar>, Vec<Scalar>)>) -> FixtureSet {
    let mut id = [0u8; 16];
    for (at, slot) in (0u8..).zip(id.iter_mut()) {
        *slot = revision_seed.wrapping_add(at);
    }
    FixtureSet {
        id,
        cases: cases
            .into_iter()
            .map(|(inputs, outputs)| Fixture { inputs, outputs })
            .collect(),
    }
}

pub(crate) fn literal(value: i64, stack_out: &[Ty], source: &'static [u8]) -> Envelope {
    let body = Recipe {
        nodes: vec![Node::I64(value)],
        entry: vec![0],
    };
    envelope(&[], stack_out, body, source)
}

pub(crate) fn arithmetic(values: [i64; 2], op: Op, source: &'static [u8]) -> Envelope {
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

pub(crate) fn admitted(
    namespace: &noble_contracts::source::Session,
    entries: &[Envelope],
) -> Result<Library, String> {
    let mut library = library();
    for candidate in entries {
        library
            .admit(namespace, candidate.clone())
            .map_err(|refusal| format!("admit: {refusal:?}"))?;
    }
    Ok(library)
}

pub(crate) fn stack_tail_library(
    session: &noble_contracts::source::Session,
) -> Result<(Library, [u8; 16], [u8; 16]), String> {
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
    swap.instantiations = vec![chunk::Instantiation {
        bindings: vec![
            chunk::Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::I64,
            },
            chunk::Binding {
                variable: 2,
                kind: noble_kernel::words::VariableKind::Value,
                ty: Ty::Bool,
            },
        ],
    }];
    swap.chunk_id = chunk::chunk_identity_of(&swap);

    let duplicate_body = Recipe {
        nodes: vec![Node::Operation {
            op: Op::Dup,
            instantiation: 0,
        }],
        entry: vec![0],
    };
    let mut duplicate = envelope(&[Ty::I64], &[Ty::I64, Ty::I64], duplicate_body, b"dup ");
    duplicate.instantiations = vec![chunk::Instantiation {
        bindings: vec![chunk::Binding {
            variable: 1,
            kind: noble_kernel::words::VariableKind::Value,
            ty: Ty::I64,
        }],
    }];
    duplicate.chunk_id = chunk::chunk_identity_of(&duplicate);
    let entries = admitted(session, &[swap.clone(), duplicate.clone()])?;
    assert_eq!(entries.entries().len(), 2);
    assert!(entries
        .entries()
        .iter()
        .all(|entry| entry.interface().effects.is_empty()));
    Ok((entries, swap.chunk_id, duplicate.chunk_id))
}

pub(crate) fn lower_budget(resource: Resource) -> Budget {
    let mut budget = Budget::PROFILE;
    match resource {
        Resource::LibraryBytes => budget.library_bytes = 0,
        Resource::Entries => budget.entries = 0,
        Resource::RecipeNodes => budget.recipe_nodes = 0,
        Resource::StackHeight => budget.stack_height = 0,
        Resource::DependencySteps => budget.dependency_steps = 0,
        Resource::InstantiationBindings => budget.instantiation_bindings = 0,
        Resource::QuotationDepth => budget.quotation_depth = 0,
        Resource::ExpandedNodes => budget.expanded_nodes = 0,
        Resource::ComposedChunks => budget.composed_chunks = 0,
        Resource::SourceBytes => budget.source_bytes = 0,
        Resource::Candidates => budget.candidates = 0,
        Resource::Diagnostics => budget.diagnostics = 0,
        Resource::CheckingWork => budget.checking_work = 0,
        Resource::EvaluationSteps => budget.evaluation_steps = 0,
        Resource::RetainedBytes => budget.retained_bytes = 0,
    }
    debug_assert!(budget.limit(resource) <= Budget::PROFILE.limit(resource));
    debug_assert_eq!(budget.limit(resource), 0);
    budget
}

pub(crate) fn expect_exhausted(
    session: &noble_contracts::source::Session,
    entries: &Library,
    request: &Request,
    resource: Resource,
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
    assert_eq!(refusal.limit, 0);
    assert!(refusal.observed > refusal.limit, "{refusal:?}");
    Ok(())
}

pub(crate) fn with_budget(
    editor: Candidate,
    path: Vec<u32>,
    stack_in: Vec<Ty>,
    stack_out: Vec<Ty>,
    resource: Resource,
) -> Request {
    let mut request = request(editor, path, stack_in, stack_out);
    request.budget = lower_budget(resource);
    request
}

pub(crate) fn definition_ref(envelope: &Envelope) -> DefinitionRef {
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
