use noble_contracts::source::chunks::{self, Envelope, Instantiation, Interface, Node, Op, Recipe};
use noble_contracts::source::procedures::{ProcedureRecord, Producer, Provenance};
use noble_kernel::types::{EffSet, Ty};
use std::string::String;
use std::vec;
use std::vec::Vec;

pub mod verifier;

pub const fn revision(seed: u8) -> [u8; 32] {
    let mut output = [0; 32];
    let mut index = 0;
    while index < 32 {
        output[index] = seed.wrapping_add(index as u8);
        index += 1;
    }
    output
}

pub fn limits() -> noble_contracts::Limits {
    noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    }
}

pub fn provenance(producer: Producer) -> Provenance {
    Provenance {
        producer,
        producer_tool_revision: 1,
        task_id: (producer == Producer::Synthesis).then_some([0x31; 32]),
        grammar_revision: (producer == Producer::Synthesis).then_some(1),
    }
}

pub fn envelope(
    stack_in: &[Ty],
    stack_out: &[Ty],
    source: &[u8],
    nodes: Vec<Node>,
    entry: Vec<u32>,
) -> Envelope {
    let body = Recipe { nodes, entry };
    let instantiations = if body
        .nodes
        .iter()
        .any(|node| matches!(node, Node::Operation { .. } | Node::Invoke { .. }))
    {
        vec![Instantiation {
            bindings: Vec::new(),
        }]
    } else {
        Vec::new()
    };
    let mut output = Envelope {
        format: chunks::ENVELOPE_FORMAT,
        semantic_revision: chunks::SEMANTIC_REVISION,
        profile: String::from("pure-i64-bool/1"),
        chunk_id: [0; chunks::IDENTITY_BYTES],
        source_revision: [0; chunks::REVISION_BYTES],
        interface: Interface {
            stack_in: stack_in.to_vec(),
            stack_out: stack_out.to_vec(),
            effects: EffSet::empty(),
        },
        captures: Vec::new(),
        dependencies: Vec::new(),
        instantiations,
        body,
    };
    output.chunk_id = chunks::chunk_identity_of(&output);
    output.source_revision = chunks::canonical_revision(source);
    debug_assert_eq!(output.chunk_id, chunks::chunk_identity_of(&output));
    debug_assert_eq!(output.source_revision, chunks::canonical_revision(source));
    output
}

pub fn literal_record(
    value: i64,
    producer: Producer,
) -> Result<ProcedureRecord, noble_contracts::source::procedures::Refusal> {
    let mut source = value.to_string();
    source.push(' ');
    let body = envelope(
        &[],
        &[Ty::I64],
        source.as_bytes(),
        vec![Node::I64(value)],
        vec![0],
    );
    ProcedureRecord::candidate(body, provenance(producer))
}

pub fn add_seven_record(
    producer: Producer,
) -> Result<ProcedureRecord, noble_contracts::source::procedures::Refusal> {
    let body = envelope(
        &[Ty::I64],
        &[Ty::I64],
        b"7 + ",
        vec![
            Node::I64(7),
            Node::Operation {
                op: Op::Add,
                instantiation: 0,
            },
        ],
        vec![0, 1],
    );
    ProcedureRecord::candidate(body, provenance(producer))
}

pub fn times_five_record(
    producer: Producer,
) -> Result<ProcedureRecord, noble_contracts::source::procedures::Refusal> {
    let body = envelope(
        &[Ty::I64],
        &[Ty::I64],
        b"5 * ",
        vec![
            Node::I64(5),
            Node::Operation {
                op: Op::Mul,
                instantiation: 0,
            },
        ],
        vec![0, 1],
    );
    ProcedureRecord::candidate(body, provenance(producer))
}

pub fn add_record(
    producer: Producer,
) -> Result<ProcedureRecord, noble_contracts::source::procedures::Refusal> {
    let body = envelope(
        &[Ty::I64, Ty::I64],
        &[Ty::I64],
        b"+ ",
        vec![Node::Operation {
            op: Op::Add,
            instantiation: 0,
        }],
        vec![0],
    );
    ProcedureRecord::candidate(body, provenance(producer))
}

pub fn increment_record(
    producer: Producer,
) -> Result<ProcedureRecord, noble_contracts::source::procedures::Refusal> {
    let body = envelope(
        &[Ty::I64],
        &[Ty::I64],
        b"1 + ",
        vec![
            Node::I64(1),
            Node::Operation {
                op: Op::Add,
                instantiation: 0,
            },
        ],
        vec![0, 1],
    );
    ProcedureRecord::candidate(body, provenance(producer))
}
