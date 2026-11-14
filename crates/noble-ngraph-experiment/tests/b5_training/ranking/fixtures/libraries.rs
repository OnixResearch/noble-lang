use super::*;

pub(crate) fn empty_instantiation() -> Vec<Instantiation> {
    vec![Instantiation {
        bindings: Vec::new(),
    }]
}

pub(crate) fn operation(op: Op) -> Node {
    Node::Operation {
        op,
        instantiation: 0,
    }
}

fn pure_interface(stack_in: &[Ty], stack_out: &[Ty]) -> Interface {
    Interface {
        stack_in: stack_in.to_vec(),
        stack_out: stack_out.to_vec(),
        effects: EffSet::empty(),
    }
}

pub(crate) fn envelope(
    stack_in: &[Ty],
    stack_out: &[Ty],
    body: Recipe,
    source: &[u8],
    instantiations: Vec<Instantiation>,
) -> Envelope {
    let mut envelope = Envelope {
        format: 1,
        semantic_revision: 0,
        profile: String::from("pure-i64-bool/1"),
        chunk_id: [0; chunks::IDENTITY_BYTES],
        source_revision: chunks::canonical_revision(source),
        interface: pure_interface(stack_in, stack_out),
        captures: Vec::new(),
        dependencies: Vec::new(),
        instantiations,
        body,
    };
    envelope.chunk_id = chunks::chunk_identity_of(&envelope);
    envelope
}

pub(crate) fn literal_envelope(value: i64, source: &[u8]) -> Envelope {
    envelope(
        &[],
        &[Ty::I64],
        Recipe {
            nodes: vec![Node::I64(value)],
            entry: vec![0],
        },
        source,
        Vec::new(),
    )
}

pub(crate) fn operation_envelope(op: Op, source: &[u8]) -> Envelope {
    let (stack_in, stack_out) = match op {
        Op::Add | Op::Sub | Op::Mul => (vec![Ty::I64, Ty::I64], vec![Ty::I64]),
        Op::Eq => (vec![Ty::I64, Ty::I64], vec![Ty::Bool]),
        Op::Dup | Op::Drop | Op::Swap | Op::Run => unreachable!(),
    };
    envelope(
        &stack_in,
        &stack_out,
        Recipe {
            nodes: vec![operation(op)],
            entry: vec![0],
        },
        source,
        empty_instantiation(),
    )
}

pub(crate) fn generic_operation_envelope(
    op: Op,
    stack_in: &[Ty],
    stack_out: &[Ty],
    bindings: Vec<Binding>,
    source: &[u8],
) -> Envelope {
    envelope(
        stack_in,
        stack_out,
        Recipe {
            nodes: vec![operation(op)],
            entry: vec![0],
        },
        source,
        vec![Instantiation { bindings }],
    )
}

pub(crate) fn boolean_comparison_envelope(op: Op, source: &[u8]) -> Envelope {
    let nodes = if op == Op::Eq {
        vec![operation(Op::Eq)]
    } else {
        vec![operation(op), Node::I64(0), operation(Op::Eq)]
    };
    let entry = (0..nodes.len())
        .map(|index| u32::try_from(index).expect("boolean recipe has at most three nodes"))
        .collect();
    envelope(
        &[Ty::I64, Ty::I64],
        &[Ty::Bool],
        Recipe { nodes, entry },
        source,
        empty_instantiation(),
    )
}

fn quotation_recipe(value: i64, depth: u8) -> Recipe {
    let mut nodes = vec![Node::I64(value)];
    let mut previous = 0u32;
    for _ in 0..depth {
        let at = u32::try_from(nodes.len()).expect("quotation depth fits u32");
        nodes.push(Node::Quotation {
            body: vec![previous],
            instantiation: 0,
        });
        previous = at;
    }
    let mut entry = vec![previous];
    for _ in 0..depth {
        let at = u32::try_from(nodes.len()).expect("quotation depth fits u32");
        nodes.push(operation(Op::Run));
        entry.push(at);
    }
    Recipe { nodes, entry }
}

pub(crate) fn quotation_envelope(value: i64, depth: u8, source: &[u8]) -> Envelope {
    envelope(
        &[],
        &[Ty::I64],
        quotation_recipe(value, depth),
        source,
        empty_instantiation(),
    )
}

fn arithmetic_envelopes() -> Vec<Envelope> {
    vec![
        operation_envelope(Op::Add, b"+ "),
        operation_envelope(Op::Sub, b"- "),
        operation_envelope(Op::Mul, b"* "),
    ]
}

fn value_binding(variable: u16, ty: Ty) -> Binding {
    Binding {
        variable,
        kind: noble_kernel::words::VariableKind::Value,
        ty,
    }
}

fn routing_swap_envelope(first: Ty, second: Ty) -> Envelope {
    let stack_in = [first.clone(), second.clone()];
    let stack_out = [second.clone(), first.clone()];
    let bindings = vec![value_binding(1, first), value_binding(2, second)];
    generic_operation_envelope(
        Op::Swap,
        &stack_in,
        &stack_out,
        bindings,
        b"swap ",
    )
}

fn routing_dup_envelope() -> Envelope {
    generic_operation_envelope(
        Op::Dup,
        &[Ty::I64],
        &[Ty::I64, Ty::I64],
        vec![value_binding(1, Ty::I64)],
        b"dup ",
    )
}

fn routing_envelopes() -> Vec<Envelope> {
    vec![
        routing_swap_envelope(Ty::I64, Ty::Bool),
        routing_swap_envelope(Ty::Bool, Ty::I64),
        routing_dup_envelope(),
    ]
}

fn boolean_envelopes() -> Vec<Envelope> {
    vec![
        boolean_comparison_envelope(Op::Eq, b"= "),
        boolean_comparison_envelope(Op::Add, b"+ 0 = "),
        boolean_comparison_envelope(Op::Sub, b"- 0 = "),
    ]
}

fn quotation_envelopes() -> Vec<Envelope> {
    vec![
        literal_envelope(0, b"0 "),
        quotation_envelope(1, 1, b"[ 1 ] run "),
        quotation_envelope(2, 2, b"[ [ 2 ] ] run run "),
    ]
}

fn integer_envelopes() -> Vec<Envelope> {
    vec![
        literal_envelope(0, b"0 "),
        literal_envelope(1, b"1 "),
        literal_envelope(2, b"2 "),
    ]
}

pub(crate) fn family_envelopes(family: usize) -> Vec<Envelope> {
    match family {
        0 => arithmetic_envelopes(),
        1 => routing_envelopes(),
        2 => boolean_envelopes(),
        3 => quotation_envelopes(),
        4 => integer_envelopes(),
        _ => unreachable!(),
    }
}

pub(crate) fn family_revision(family: usize) -> [u8; chunks::REVISION_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"noble.tchunk-ranking-library/1\0");
    hasher.update(FAMILY_NAMES[family].as_bytes());
    *hasher.finalize().as_bytes()
}

pub(crate) fn family_library(
    session: &noble_contracts::source::Session,
    family: usize,
) -> Result<Library, String> {
    let mut library = Library::new(family_revision(family));
    for envelope in family_envelopes(family) {
        library.admit(session, envelope).map_err(|refusal| {
            format!("admit family {} chunk: {refusal:?}", FAMILY_NAMES[family])
        })?;
    }
    Ok(library)
}
