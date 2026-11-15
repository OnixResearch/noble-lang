//! Opt-in, versioned total-I64/Bool **within-layer permutation** profile (issue #129).
//!
//! The canonical compiler, layout, stream checker and evaluator remain unchanged. This profile
//! accepts a producer's actual instruction bodies and node-to-instruction witness only after
//! checking the graph's own layers, table bindings, unique registers, complete bijection of
//! emitting nodes, ordered operands, initialized reads and ordered (including repeated) outputs.
//! The proof in `NobleFoundation.PackedPermutation` is about its handwritten model; there is no
//! general theorem relating Rust bytes, this checker or the executor to that model.

#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; this self-contained artifact/checker module uses the same concrete graph, layout and instruction vocabulary at every stage of validation, hashing, encoding and interpretation."
)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::identity::IdentityScheme;
use crate::ir::{Canon, Digest, NodeId, Op};
use crate::layout::PackedLayout;
use crate::schedule::I64Instruction;

/// Independent profile identity; the canonical `ngraph-layered/0` path never selects it.
pub const PROFILE: &str = "ngraph-packed-permutation/0";
/// Revision of this checker, independently bound into replay identity.
pub const CHECKER_REVISION: &str = "ngraph-permutation-checker/0";
/// Handwritten Lean model revision; a revision bump requires rechecking its proof lane.
pub const MODEL_REVISION: &str = "NobleFoundation.PackedPermutation/0";
/// Binary export vocabulary revision; a change requires fresh decoding and model export.
pub const EXPORT_REVISION: &str = "ngraph-permutation-export/0";
const MAGIC: &[u8; 8] = b"NPRM0000";
/// Maximum encoded bytes for the default opt-in reader/writer (caller may lower the cap).
pub const MAX_EXPORT_BYTES: u64 = 32 * 1024 * 1024;
/// Maximum aggregate graph/declaration/stream/witness/output entries for default checking.
pub const MAX_CHECK_WORK: u64 = 4 * 1024 * 1024;

/// A producer-owned artifact: no self-attested value in it is accepted as a checker verdict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    /// Distinct profile identity.
    pub profile: String,
    /// Semantic revision of the graph operation interpretation.
    pub semantic_revision: String,
    /// Independent checker revision.
    pub checker_revision: String,
    /// Proof-model revision (not a theorem binding these bytes to Lean).
    pub model_revision: String,
    /// Export vocabulary revision.
    pub export_revision: String,
    /// Graph logical identity, rechecked against the checked graph.
    pub graph_id: Digest,
    /// Existing injective layout declaration, checked against the graph, not replayed as a producer.
    pub layout: PackedLayout,
    /// Actual operation bodies in executable order; no table or guard instructions.
    pub stream: Vec<I64Instruction>,
    /// The graph node each instruction purports to compute.
    pub witness: Vec<u32>,
    /// Exact ordered output registers, including repetitions.
    pub output_regs: Vec<u32>,
}

/// Independent refusal classes. `Malformed` and `Unsupported` are decoder classes, not semantic
/// disagreement; `Exhausted` makes no statement about validity of an unexamined artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The binary format was incomplete, noncanonical or ill-shaped.
    Malformed,
    /// An operation/profile outside the first total-I64/Bool profile.
    Unsupported,
    /// Input declared more work/bytes than the selected bound.
    Exhausted { work: u64, cap: u64 },
    /// Semantic, profile, checker, model or export identity was stale.
    StaleIdentity,
    /// The graph logical identity did not match the checked subject.
    GraphIdentity,
    /// The canonical layout's independent validator refused the declaration.
    Layout(crate::layout::Refusal),
    /// Instruction/witness lengths differ.
    Shape,
    /// A node witness names no instruction-emitting graph node.
    WrongWitness,
    /// A node witness is duplicated (even when its output happens to agree).
    DuplicateNode,
    /// At least one required operation is missing.
    MissingNode,
    /// Instruction is outside the graph-assigned layer's range.
    CrossLayer,
    /// An operand register has not been initialized by a table or preceding instruction.
    EarlyRead,
    /// Two instructions/table entries write the same register.
    DuplicateDestination,
    /// Op, ordered source registers or destination differs from the witnessed graph node.
    BodyMismatch,
    /// Ordered/repeated output registers differ from the graph's actual outputs.
    OutputMismatch,
    /// Fresh replay subjects differ from the recorded checked identities.
    StaleReplay,
}

/// An immutable, checked borrow: callers cannot substitute a different stream while running it.
#[derive(Clone, Copy)]
pub struct Validated<'a> {
    checked: &'a crate::check::CheckedGraph,
    artifact: &'a Artifact,
}

/// Snapshot of every independently bound replay subject. These are fingerprints under the
/// injected identity scheme, not a replacement for fresh semantic checking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replay {
    /// Graph semantic identity.
    pub graph: Digest,
    /// Packed layout including all register and table bindings.
    pub layout: Digest,
    /// Actual instruction bodies.
    pub stream: Digest,
    /// Node-to-instruction witness.
    pub witness: Digest,
    /// Ordered and repeated output map.
    pub output_regs: Digest,
    /// Profile and graph semantic revisions.
    pub profile: Digest,
    /// Checker, proof model and export revisions.
    pub revisions: Digest,
}

/// Export the canonical compiler's actual I64 stream as an opt-in artifact. This constructor does
/// not change compilation or allow the canonical checker to accept permutations. The producer may
/// reorder each `(stream, witness)` pair; the independent validator decides whether it is valid.
///
/// # Errors
/// Refuses unsupported nodes or a compiler built for another checked graph.
pub fn from_compiled(
    checked: &crate::check::CheckedGraph,
    compiled: &crate::compiled::Compiled,
) -> Result<Artifact, Refusal> {
    if checked.id() != compiled.checked().id() {
        return Err(Refusal::GraphIdentity);
    }
    supported(checked)?;
    let stream = compiled
        .i64_instructions()
        .ok_or(Refusal::Unsupported)?
        .to_vec();
    let layout = compiled.packed_layout();
    let mut per_layer = alloc::vec![Vec::new(); layout.layers.len()];
    for (index, node) in checked.graph().nodes.iter().enumerate() {
        if emitting(&node.op) {
            let layer = checked.layer(NodeId(index as u32)) as usize;
            per_layer[layer].push(index as u32);
        }
    }
    let witness = per_layer.into_iter().flatten().collect();
    let output_regs = checked
        .graph()
        .outputs
        .iter()
        .map(|node| layout.reg[node.index()])
        .collect();
    Ok(Artifact {
        profile: PROFILE.to_string(),
        semantic_revision: crate::ir::SEMANTIC_REVISION.to_string(),
        checker_revision: CHECKER_REVISION.to_string(),
        model_revision: MODEL_REVISION.to_string(),
        export_revision: EXPORT_REVISION.to_string(),
        graph_id: checked.id(),
        layout,
        stream,
        witness,
        output_regs,
    })
}

fn emitting(op: &Op) -> bool {
    matches!(
        op,
        Op::I64Add(..) | Op::I64Sub(..) | Op::I64Mul(..) | Op::I64Eq(..)
    )
}

fn supported(checked: &crate::check::CheckedGraph) -> Result<(), Refusal> {
    if checked.graph().profile != crate::ir::SubProfile::I64 {
        return Err(Refusal::Unsupported);
    }
    if checked.graph().nodes.is_empty() {
        return Err(Refusal::Unsupported);
    }
    if checked.graph().outputs.is_empty() {
        return Err(Refusal::Unsupported);
    }
    if checked.graph().nodes.iter().any(|node| {
        !matches!(
            node.op,
            Op::Input(_)
                | Op::Const(crate::ir::Literal::I64(_))
                | Op::I64Add(..)
                | Op::I64Sub(..)
                | Op::I64Mul(..)
                | Op::I64Eq(..)
        )
    }) {
        return Err(Refusal::Unsupported);
    }
    Ok(())
}

/// Sum of every length walked by the checker (saturating rather than overflowing). The caller
/// tests this *before* any declaration, witness or instruction content is read.
#[must_use]
pub fn declared_work(checked: &crate::check::CheckedGraph, artifact: &Artifact) -> u64 {
    [
        checked.graph().nodes.len(),
        artifact.layout.reg.len(),
        artifact.layout.consts.len(),
        artifact.layout.slot_regs.len(),
        artifact.layout.layers.len(),
        artifact.layout.fail_suffix_min.len(),
        artifact.stream.len(),
        artifact.witness.len(),
        artifact.output_regs.len(),
    ]
    .into_iter()
    .fold(0u64, |sum, count| sum.saturating_add(count as u64))
}

fn expected(op: &Op, reg: &[u32], destination: u32) -> Option<I64Instruction> {
    let source = |id: NodeId| reg.get(id.index()).copied();
    Some(match op {
        Op::I64Add(a, b) => I64Instruction::Add(destination, source(*a)?, source(*b)?),
        Op::I64Sub(a, b) => I64Instruction::Sub(destination, source(*a)?, source(*b)?),
        Op::I64Mul(a, b) => I64Instruction::Mul(destination, source(*a)?, source(*b)?),
        Op::I64Eq(a, b) => I64Instruction::Eq(destination, source(*a)?, source(*b)?),
        _ => return None,
    })
}

fn operands(ins: I64Instruction) -> Option<(u32, u32, u32)> {
    match ins {
        I64Instruction::Add(d, a, b)
        | I64Instruction::Sub(d, a, b)
        | I64Instruction::Mul(d, a, b)
        | I64Instruction::Eq(d, a, b) => Some((d, a, b)),
        I64Instruction::Guard { .. } => None,
    }
}

/// Recheck the complete graph/declaration/actual stream/witness relation under `cap`.
/// Complexity is `O((n + k) log n)` including the existing layout checker; temporary memory is
/// two Boolean arrays of `layout.regs` and `graph.nodes.len()`. No producer digest is trusted.
///
/// # Errors
/// A classed refusal; exhaustion is not a semantic verdict.
pub fn check_within<'a>(
    checked: &'a crate::check::CheckedGraph,
    artifact: &'a Artifact,
    cap: u64,
) -> Result<Validated<'a>, Refusal> {
    let work = declared_work(checked, artifact);
    if work > cap {
        return Err(Refusal::Exhausted { work, cap });
    }
    supported(checked)?;
    for (declared, selected) in [
        (artifact.profile.as_str(), PROFILE),
        (
            artifact.semantic_revision.as_str(),
            crate::ir::SEMANTIC_REVISION,
        ),
        (artifact.checker_revision.as_str(), CHECKER_REVISION),
        (artifact.model_revision.as_str(), MODEL_REVISION),
        (artifact.export_revision.as_str(), EXPORT_REVISION),
    ] {
        if declared != selected {
            return Err(Refusal::StaleIdentity);
        }
    }
    if artifact.graph_id != checked.id() {
        return Err(Refusal::GraphIdentity);
    }
    crate::layout::check(checked, &artifact.layout).map_err(Refusal::Layout)?;
    if artifact.stream.len() != artifact.witness.len() {
        return Err(Refusal::Shape);
    }
    let graph = checked.graph();
    let mut seen = alloc::vec![false; graph.nodes.len()];
    let mut initialized = alloc::vec![false; artifact.layout.regs as usize];
    for &(reg, _) in &artifact.layout.consts {
        initialized[reg as usize] = true;
    }
    for &(reg, _) in &artifact.layout.slot_regs {
        initialized[reg as usize] = true;
    }
    for (position, (&node_id, &instruction)) in
        artifact.witness.iter().zip(&artifact.stream).enumerate()
    {
        let index = node_id as usize;
        let node = graph.nodes.get(index).ok_or(Refusal::WrongWitness)?;
        if !emitting(&node.op) {
            return Err(Refusal::WrongWitness);
        }
        if seen[index] {
            return Err(Refusal::DuplicateNode);
        }
        seen[index] = true;
        let layer = checked.layer(NodeId(node_id)) as usize;
        let &(start, end) = artifact
            .layout
            .layers
            .get(layer)
            .ok_or(Refusal::CrossLayer)?;
        if position < start as usize || position >= end as usize {
            return Err(Refusal::CrossLayer);
        }
        let (destination, left, right) = operands(instruction).ok_or(Refusal::Unsupported)?;
        if destination >= artifact.layout.regs
            || left >= artifact.layout.regs
            || right >= artifact.layout.regs
        {
            return Err(Refusal::BodyMismatch);
        }
        if initialized[destination as usize] {
            return Err(Refusal::DuplicateDestination);
        }
        if !initialized[left as usize] || !initialized[right as usize] {
            return Err(Refusal::EarlyRead);
        }
        if expected(&node.op, &artifact.layout.reg, artifact.layout.reg[index]) != Some(instruction)
        {
            return Err(Refusal::BodyMismatch);
        }
        initialized[destination as usize] = true;
    }
    if graph
        .nodes
        .iter()
        .enumerate()
        .any(|(index, node)| emitting(&node.op) && !seen[index])
    {
        return Err(Refusal::MissingNode);
    }
    if artifact.output_regs.len() != graph.outputs.len()
        || artifact
            .output_regs
            .iter()
            .zip(&graph.outputs)
            .any(|(&reg, node)| reg != artifact.layout.reg[node.index()])
    {
        return Err(Refusal::OutputMismatch);
    }
    Ok(Validated { checked, artifact })
}

/// Default opt-in checker. The canonical schedule::check is not changed.
///
/// # Errors
/// A classed refusal.
pub fn check<'a>(
    checked: &'a crate::check::CheckedGraph,
    artifact: &'a Artifact,
) -> Result<Validated<'a>, Refusal> {
    check_within(checked, artifact, MAX_CHECK_WORK)
}

fn hash_stream(ids: &dyn IdentityScheme, stream: &[I64Instruction]) -> Digest {
    let mut canon = Canon::new(ids, "noble.ngraph.permutation.stream.v0");
    canon.u64(stream.len() as u64);
    for &instruction in stream {
        match instruction {
            I64Instruction::Add(d, a, b) => {
                canon.u8(0);
                canon.u32(d);
                canon.u32(a);
                canon.u32(b);
            }
            I64Instruction::Sub(d, a, b) => {
                canon.u8(1);
                canon.u32(d);
                canon.u32(a);
                canon.u32(b);
            }
            I64Instruction::Mul(d, a, b) => {
                canon.u8(2);
                canon.u32(d);
                canon.u32(a);
                canon.u32(b);
            }
            I64Instruction::Eq(d, a, b) => {
                canon.u8(3);
                canon.u32(d);
                canon.u32(a);
                canon.u32(b);
            }
            I64Instruction::Guard { node } => {
                canon.u8(4);
                canon.u32(node);
            }
        }
    }
    canon.finish()
}

fn hash_u32s(ids: &dyn IdentityScheme, domain: &str, values: &[u32]) -> Digest {
    let mut canon = Canon::new(ids, domain);
    canon.u64(values.len() as u64);
    for &value in values {
        canon.u32(value);
    }
    canon.finish()
}

/// Freshly compute fingerprints of every replay subject, without trusting the artifact's claimed
/// graph identity as a stand-in for the graph supplied by the checker.
#[must_use]
pub fn identities(
    ids: &dyn IdentityScheme,
    checked: &crate::check::CheckedGraph,
    artifact: &Artifact,
) -> Replay {
    let layout = &artifact.layout;
    let mut layout_hash = Canon::new(ids, "noble.ngraph.permutation.layout.v0");
    layout_hash.str(&layout.revision);
    layout_hash.str(&layout.compiler_revision);
    layout_hash.u32(layout.regs);
    layout_hash.bytes(&layout.layout_key);
    layout_hash.u64(layout.reg.len() as u64);
    for &reg in &layout.reg {
        layout_hash.u32(reg);
    }
    layout_hash.u64(layout.consts.len() as u64);
    for &(reg, bits) in &layout.consts {
        layout_hash.u32(reg);
        layout_hash.u64(bits);
    }
    layout_hash.u64(layout.slot_regs.len() as u64);
    for &(reg, slot) in &layout.slot_regs {
        layout_hash.u32(reg);
        layout_hash.u32(slot);
    }
    layout_hash.u64(layout.layers.len() as u64);
    for &(start, end) in &layout.layers {
        layout_hash.u32(start);
        layout_hash.u32(end);
    }
    layout_hash.u64(layout.fail_suffix_min.len() as u64);
    for &minimum in &layout.fail_suffix_min {
        layout_hash.u32(minimum);
    }
    let mut profile_hash = Canon::new(ids, "noble.ngraph.permutation.profile.v0");
    profile_hash.str(&artifact.profile);
    profile_hash.str(&artifact.semantic_revision);
    let mut revision_hash = Canon::new(ids, "noble.ngraph.permutation.revisions.v0");
    revision_hash.str(&artifact.checker_revision);
    revision_hash.str(&artifact.model_revision);
    revision_hash.str(&artifact.export_revision);
    Replay {
        graph: checked.id(),
        layout: layout_hash.finish(),
        stream: hash_stream(ids, &artifact.stream),
        witness: hash_u32s(
            ids,
            "noble.ngraph.permutation.witness.v0",
            &artifact.witness,
        ),
        output_regs: hash_u32s(
            ids,
            "noble.ngraph.permutation.outputs.v0",
            &artifact.output_regs,
        ),
        profile: profile_hash.finish(),
        revisions: revision_hash.finish(),
    }
}

/// Recheck a previously bound artifact against fresh graph, stream, witness and every revision.
/// An old verdict is never reused: even an unchanged fingerprint still runs `check_within`.
///
/// # Errors
/// A stale subject or a fresh classed check refusal.
pub fn replay<'a>(
    ids: &dyn IdentityScheme,
    checked: &'a crate::check::CheckedGraph,
    artifact: &'a Artifact,
    prior: &Replay,
    cap: u64,
) -> Result<Validated<'a>, Refusal> {
    if &identities(ids, checked, artifact) != prior {
        return Err(Refusal::StaleReplay);
    }
    check_within(checked, artifact, cap)
}

/// Input or evaluation refusal for this opt-in interpreter; not a theorem about the compiled
/// evaluator. Fuel is the entire graph-node count, including input/constant preloads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunRefusal {
    /// An input does not satisfy the checked graph's interface/facts.
    Input(crate::diag::Diagnostic),
    /// Fuel or register scratch not sufficient for the selected graph.
    Exhausted,
    /// A supposedly validated artifact no longer has an executable instruction or output type.
    InvalidatedArtifact,
}

/// Interpret **only** this already validated, immutable artifact under sufficient fuel and
/// scratch. Existing canonical execution is not called and no Rust executor theorem follows.
///
/// # Errors
/// Input rejection or insufficient budget before any arithmetic executes.
pub fn execute(
    accepted: Validated<'_>,
    inputs: &[crate::sem::Value],
    fuel: u64,
    scratch_bytes: u64,
) -> Result<Vec<crate::sem::Value>, RunRefusal> {
    let graph = accepted.checked.graph();
    let artifact = accepted.artifact;
    if (graph.nodes.len() as u64) > fuel
        || u64::from(artifact.layout.regs).saturating_mul(8) > scratch_bytes
    {
        return Err(RunRefusal::Exhausted);
    }
    let slots =
        crate::eval::preflight(accepted.checked, inputs, None).map_err(RunRefusal::Input)?;
    let mut regs = alloc::vec![0i64; artifact.layout.regs as usize];
    for &(register, bits) in &artifact.layout.consts {
        regs[register as usize] = bits as i64;
    }
    for &(register, slot) in &artifact.layout.slot_regs {
        match slots[slot as usize] {
            crate::sem::Value::I64(bits) => regs[register as usize] = bits,
            _ => {
                return Err(RunRefusal::Input(crate::diag::Diagnostic::new(
                    crate::diag::Code::Interface,
                    crate::diag::Site::Slot(slot),
                    "the permutation profile requires I64 inputs",
                )))
            }
        }
    }
    for instruction in &artifact.stream {
        let (dst, value) = match *instruction {
            I64Instruction::Add(dst, a, b) => {
                (dst, regs[a as usize].wrapping_add(regs[b as usize]))
            }
            I64Instruction::Sub(dst, a, b) => {
                (dst, regs[a as usize].wrapping_sub(regs[b as usize]))
            }
            I64Instruction::Mul(dst, a, b) => {
                (dst, regs[a as usize].wrapping_mul(regs[b as usize]))
            }
            I64Instruction::Eq(dst, a, b) => (dst, i64::from(regs[a as usize] == regs[b as usize])),
            I64Instruction::Guard { .. } => return Err(RunRefusal::InvalidatedArtifact),
        };
        regs[dst as usize] = value;
    }
    graph
        .outputs
        .iter()
        .map(|node| {
            let bits = regs[artifact.layout.reg[node.index()] as usize];
            match accepted.checked.ty(*node) {
                Some(crate::ir::ScalarType::Bool) => Ok(crate::sem::Value::Bool(bits != 0)),
                Some(crate::ir::ScalarType::I64) => Ok(crate::sem::Value::I64(bits)),
                Some(crate::ir::ScalarType::F32) | None => Err(RunRefusal::InvalidatedArtifact),
            }
        })
        .collect()
}

fn encoded_bytes(artifact: &Artifact) -> u64 {
    let mut total = 8u64 + 32 + 32 + 4;
    for text in [
        &artifact.profile,
        &artifact.semantic_revision,
        &artifact.checker_revision,
        &artifact.model_revision,
        &artifact.export_revision,
        &artifact.layout.revision,
        &artifact.layout.compiler_revision,
    ] {
        total = total.saturating_add(4).saturating_add(text.len() as u64);
    }
    for (len, width) in [
        (artifact.layout.reg.len(), 4u64),
        (artifact.layout.consts.len(), 12),
        (artifact.layout.slot_regs.len(), 8),
        (artifact.layout.layers.len(), 8),
        (artifact.layout.fail_suffix_min.len(), 4),
        (artifact.stream.len(), 13),
        (artifact.witness.len(), 4),
        (artifact.output_regs.len(), 4),
    ] {
        total = total
            .saturating_add(4)
            .saturating_add((len as u64).saturating_mul(width));
    }
    total
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the encoder appends to the caller's preallocated Vec and needs its growable capacity, not a fixed slice."
)]
fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the encoder appends to the caller's preallocated Vec and needs its growable capacity, not a fixed slice."
)]
fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; length-prefixed text must append to the same growable encoded Vec."
)]
fn push_text(bytes: &mut Vec<u8>, text: &str) {
    push_u32(bytes, text.len() as u32);
    bytes.extend_from_slice(text.as_bytes());
}
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the vector length and every value must append to the same growable encoded Vec."
)]
fn push_u32s(bytes: &mut Vec<u8>, values: &[u32]) {
    push_u32(bytes, values.len() as u32);
    for &value in values {
        push_u32(bytes, value);
    }
}

/// Emit a byte-for-byte deterministic export after preflighting the whole output size. Decoder
/// and checker have independent bounds; writing this alone never accepts its semantic content.
///
/// # Errors
/// Exhaustion if the size exceeds `cap` or an encoding length cannot fit u32.
pub fn encode_within(artifact: &Artifact, cap: u64) -> Result<Vec<u8>, Refusal> {
    let size_bytes = encoded_bytes(artifact);
    if size_bytes > cap || size_bytes > u32::MAX as u64 {
        return Err(Refusal::Exhausted {
            work: size_bytes,
            cap: cap.min(u32::MAX as u64),
        });
    }
    let mut bytes = Vec::with_capacity(size_bytes as usize);
    bytes.extend_from_slice(MAGIC);
    for text in [
        &artifact.profile,
        &artifact.semantic_revision,
        &artifact.checker_revision,
        &artifact.model_revision,
        &artifact.export_revision,
        &artifact.layout.revision,
        &artifact.layout.compiler_revision,
    ] {
        push_text(&mut bytes, text);
    }
    bytes.extend_from_slice(&artifact.graph_id);
    bytes.extend_from_slice(&artifact.layout.layout_key);
    push_u32(&mut bytes, artifact.layout.regs);
    push_u32s(&mut bytes, &artifact.layout.reg);
    push_u32(&mut bytes, artifact.layout.consts.len() as u32);
    for &(reg, bits) in &artifact.layout.consts {
        push_u32(&mut bytes, reg);
        push_u64(&mut bytes, bits);
    }
    push_u32(&mut bytes, artifact.layout.slot_regs.len() as u32);
    for &(reg, slot) in &artifact.layout.slot_regs {
        push_u32(&mut bytes, reg);
        push_u32(&mut bytes, slot);
    }
    push_u32(&mut bytes, artifact.layout.layers.len() as u32);
    for &(start, end) in &artifact.layout.layers {
        push_u32(&mut bytes, start);
        push_u32(&mut bytes, end);
    }
    push_u32s(&mut bytes, &artifact.layout.fail_suffix_min);
    push_u32(&mut bytes, artifact.stream.len() as u32);
    for instruction in &artifact.stream {
        match instruction {
            I64Instruction::Add(d, a, b) => {
                bytes.push(0);
                push_u32(&mut bytes, *d);
                push_u32(&mut bytes, *a);
                push_u32(&mut bytes, *b);
            }
            I64Instruction::Sub(d, a, b) => {
                bytes.push(1);
                push_u32(&mut bytes, *d);
                push_u32(&mut bytes, *a);
                push_u32(&mut bytes, *b);
            }
            I64Instruction::Mul(d, a, b) => {
                bytes.push(2);
                push_u32(&mut bytes, *d);
                push_u32(&mut bytes, *a);
                push_u32(&mut bytes, *b);
            }
            I64Instruction::Eq(d, a, b) => {
                bytes.push(3);
                push_u32(&mut bytes, *d);
                push_u32(&mut bytes, *a);
                push_u32(&mut bytes, *b);
            }
            I64Instruction::Guard { .. } => return Err(Refusal::Unsupported),
        }
    }
    push_u32s(&mut bytes, &artifact.witness);
    push_u32s(&mut bytes, &artifact.output_regs);
    Ok(bytes)
}

/// Default bounded exporter.
///
/// # Errors
/// Exhausted or unsupported stream opcode.
pub fn encode(artifact: &Artifact) -> Result<Vec<u8>, Refusal> {
    encode_within(artifact, MAX_EXPORT_BYTES)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Refusal> {
        let end = self.at.checked_add(count).ok_or(Refusal::Malformed)?;
        let slice = self.bytes.get(self.at..end).ok_or(Refusal::Malformed)?;
        self.at = end;
        Ok(slice)
    }
    fn u32(&mut self) -> Result<u32, Refusal> {
        let bytes: [u8; 4] = self.take(4)?.try_into().map_err(|_| Refusal::Malformed)?;
        Ok(u32::from_le_bytes(bytes))
    }
    fn u64(&mut self) -> Result<u64, Refusal> {
        let bytes: [u8; 8] = self.take(8)?.try_into().map_err(|_| Refusal::Malformed)?;
        Ok(u64::from_le_bytes(bytes))
    }
    fn digest(&mut self) -> Result<Digest, Refusal> {
        self.take(32)?.try_into().map_err(|_| Refusal::Malformed)
    }
    fn text(&mut self) -> Result<String, Refusal> {
        let len = self.u32()? as usize;
        let raw = self.take(len)?;
        let text = core::str::from_utf8(raw).map_err(|_| Refusal::Malformed)?;
        Ok(text.to_string())
    }
    fn rows<T>(
        &mut self,
        width: usize,
        mut row: impl FnMut(&mut Self) -> Result<T, Refusal>,
    ) -> Result<Vec<T>, Refusal> {
        let len = self.u32()? as usize;
        let required_bytes = len.checked_mul(width).ok_or(Refusal::Malformed)?;
        if required_bytes > self.bytes.len().saturating_sub(self.at) {
            return Err(Refusal::Malformed);
        }
        let mut values = Vec::with_capacity(len);
        for _ in 0..len {
            values.push(row(self)?);
        }
        Ok(values)
    }
    fn u32s(&mut self) -> Result<Vec<u32>, Refusal> {
        self.rows(4, Self::u32)
    }
}

/// Decode a producer artifact with a byte cap before allocating any per-field storage. Lengths
/// are checked against remaining bytes before each allocation, trailing bytes are malformed,
/// and unknown instruction tags are unsupported (rather than silently interpreted).
///
/// # Errors
/// Malformed, unsupported or exhausted are distinct from the subsequent semantic checker.
pub fn decode_within(bytes: &[u8], cap: u64) -> Result<Artifact, Refusal> {
    if bytes.len() as u64 > cap {
        return Err(Refusal::Exhausted {
            work: bytes.len() as u64,
            cap,
        });
    }
    let mut reader = Reader { bytes, at: 0 };
    if reader.take(8)? != MAGIC {
        return Err(Refusal::Unsupported);
    }
    let profile = reader.text()?;
    let semantic_revision = reader.text()?;
    let checker_revision = reader.text()?;
    let model_revision = reader.text()?;
    let export_revision = reader.text()?;
    let revision = reader.text()?;
    let compiler_revision = reader.text()?;
    let graph_id = reader.digest()?;
    let layout_key = reader.digest()?;
    let regs = reader.u32()?;
    let reg = reader.u32s()?;
    let consts = reader.rows(12, |r| Ok((r.u32()?, r.u64()?)))?;
    let slot_regs = reader.rows(8, |r| Ok((r.u32()?, r.u32()?)))?;
    let layers = reader.rows(8, |r| Ok((r.u32()?, r.u32()?)))?;
    let fail_suffix_min = reader.u32s()?;
    let stream = reader.rows(13, |r| {
        let tag = r.take(1)?[0];
        let (d, a, b) = (r.u32()?, r.u32()?, r.u32()?);
        match tag {
            0 => Ok(I64Instruction::Add(d, a, b)),
            1 => Ok(I64Instruction::Sub(d, a, b)),
            2 => Ok(I64Instruction::Mul(d, a, b)),
            3 => Ok(I64Instruction::Eq(d, a, b)),
            _ => Err(Refusal::Unsupported),
        }
    })?;
    let witness = reader.u32s()?;
    let output_regs = reader.u32s()?;
    if reader.at != bytes.len() {
        return Err(Refusal::Malformed);
    }
    Ok(Artifact {
        profile,
        semantic_revision,
        checker_revision,
        model_revision,
        export_revision,
        graph_id,
        layout: PackedLayout {
            revision,
            compiler_revision,
            regs,
            reg,
            consts,
            slot_regs,
            layers,
            fail_suffix_min,
            layout_key,
        },
        stream,
        witness,
        output_regs,
    })
}

/// Default bounded decoder. The caller must still run `check`/`replay` before execution.
///
/// # Errors
/// Malformed, unsupported or exhausted.
pub fn decode(bytes: &[u8]) -> Result<Artifact, Refusal> {
    decode_within(bytes, MAX_EXPORT_BYTES)
}
