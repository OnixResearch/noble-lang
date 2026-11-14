//! Typed-chunk envelope and library controls (TCHUNK-01, TCHUNK-03).
//!
//! Every expected value here is written independently of the producer: the
//! canonical source text is spelled out, and each expected interface is stated
//! by the test rather than read back from the library.

use noble_contracts::source::chunks as chunk;
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
    for (slot, offset) in out.iter_mut().zip(0u8..32) {
        *slot = seed.wrapping_add(offset);
    }
    out
}

/// The library revision every request in these tests pins.
pub(super) fn pinned() -> [u8; 32] {
    revision(7)
}

/// An envelope with its identity and source revision computed from its inputs.
pub(super) fn envelope(stack_in: &[Ty], stack_out: &[Ty], body: Recipe, source: &[u8]) -> Envelope {
    let mut built = Envelope {
        format: chunk::ENVELOPE_FORMAT,
        semantic_revision: chunk::SEMANTIC_REVISION,
        profile: String::from("pure-i64-bool/1"),
        chunk_id: [0u8; 16],
        source_revision: [0u8; 32],
        interface: interface(stack_in, stack_out),
        captures: Vec::new(),
        dependencies: Vec::new(),
        instantiations: instantiations(&body),
        body,
    };
    built.chunk_id = chunk::chunk_identity_of(&built);
    built.source_revision = chunk::canonical_revision(source);
    built
}

fn interface(stack_in: &[Ty], stack_out: &[Ty]) -> Interface {
    Interface {
        stack_in: stack_in.to_vec(),
        stack_out: stack_out.to_vec(),
        effects: noble_kernel::types::EffSet::empty(),
    }
}

fn instantiations(body: &Recipe) -> Vec<Instantiation> {
    if body
        .nodes
        .iter()
        .any(|node| matches!(node, Node::Operation { .. }))
    {
        vec![Instantiation {
            bindings: Vec::new(),
        }]
    } else {
        Vec::new()
    }
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

fn candidate_from(body: Recipe) -> Envelope {
    envelope(&[], &[Ty::I64], body, b"7 ")
}

#[path = "chunks/captures.rs"]
mod capture_tests;
#[path = "chunks/dependencies.rs"]
mod dependency_tests;
#[path = "chunks/envelope.rs"]
mod envelope_tests;
