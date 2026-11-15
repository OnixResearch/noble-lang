//! Checks the actual graph interface and packed stream, not producer metadata.

use super::{Limits, Partition, Refusal, Selection};
use crate::identity::IdentityScheme;
use crate::ir::{Graph, Op, ScalarType, SlotKind, SubProfile};

pub(super) struct Admitted {
    pub compiled: crate::compiled::Compiled,
    pub workspace: crate::compiled::Workspace,
    pub selection: Selection,
    pub input_words: usize,
    pub retained_bytes: u64,
}

/// Refuse arithmetic overflow before multiplying a word count by 8.
pub(super) fn charge_words(words: usize, cap: u64, label: &'static str) -> Result<(), Refusal> {
    let bytes = u64::try_from(words)
        .ok()
        .and_then(|n| n.checked_mul(8))
        .ok_or(Refusal::Budget(label))?;
    if bytes > cap {
        Err(Refusal::Budget(label))
    } else {
        Ok(())
    }
}

fn check_partition(graph: &Graph, p: Partition, limits: &Limits) -> Result<usize, Refusal> {
    let input_words = p
        .state_words
        .checked_add(p.input_words)
        .and_then(|n| n.checked_add(p.control_words))
        .ok_or(Refusal::Partition)?;
    let output_words = p
        .state_words
        .checked_add(p.output_words)
        .ok_or(Refusal::Partition)?;
    if p.state_words == 0
        || p.input_words == 0
        || p.output_words == 0
        || input_words > 256
        || output_words > 256
        || graph.slots.len() != input_words
        || graph.outputs.len() != output_words
        || graph
            .slots
            .iter()
            .any(|s| s.kind != SlotKind::Input || s.ty != ScalarType::I64)
        || graph.input_shape.is_some()
        || graph.output_shape.is_some()
    {
        return Err(Refusal::Partition);
    }
    charge_words(p.state_words, limits.state_bytes, "state_bytes")?;
    charge_words(p.input_words, limits.input_bytes, "input_bytes")?;
    charge_words(p.control_words, limits.control_bytes, "control_bytes")?;
    charge_words(p.output_words, limits.output_bytes, "output_bytes")?;
    // CheckedGraph::ty is independent of the producer's claim about outputs.
    Ok(input_words)
}

fn profile(checked: &crate::check::CheckedGraph, p: Partition) -> Result<(), Refusal> {
    let graph = checked.graph();
    let interface_words = p.state_words + p.input_words + p.control_words;
    if graph.profile != SubProfile::I64
        || !graph.facts.is_empty()
        || graph.nodes.len() < interface_words
        || graph.nodes[..interface_words]
            .iter()
            .enumerate()
            .any(|(slot, node)| !matches!(node.op, Op::Input(k) if k as usize == slot))
        || graph.nodes[interface_words..]
            .iter()
            .any(|node| matches!(node.op, Op::Input(_)))
        || graph.nodes.iter().any(|n| {
            !matches!(
                n.op,
                Op::Input(_)
                    | Op::Const(crate::ir::Literal::I64(_))
                    | Op::I64Add(_, _)
                    | Op::I64Sub(_, _)
                    | Op::I64Mul(_, _)
            )
        })
        || graph
            .outputs
            .iter()
            .any(|&n| checked.ty(n) != Some(ScalarType::I64))
        || p.state_words + p.output_words != graph.outputs.len()
    {
        return Err(Refusal::Profile);
    }
    Ok(())
}

fn check_limits(limits: &Limits) -> Result<(), Refusal> {
    limits.graph.validate_policy().map_err(Refusal::Graph)?;
    let max = Limits::default();
    let fields = [
        (limits.state_bytes, max.state_bytes),
        (limits.input_bytes, max.input_bytes),
        (limits.control_bytes, max.control_bytes),
        (limits.output_bytes, max.output_bytes),
        (limits.update_bytes, max.update_bytes),
        (limits.retained_bytes, max.retained_bytes),
        (limits.schedule_work, max.schedule_work),
        (limits.tick_work, max.tick_work),
        (limits.ticks, max.ticks),
        (limits.trace_bytes, max.trace_bytes),
    ];
    if fields.iter().any(|&(actual, ceiling)| actual > ceiling)
        || limits.ticks == 0
        || limits.trace_bytes == 0
    {
        return Err(Refusal::Budget("policy"));
    }
    Ok(())
}

/// Digest exact control words, partition and graph under a typed, domain-separated
/// encoding. This is an identity only, never authorization or semantic checking.
pub(super) fn control_id(
    ids: &dyn IdentityScheme,
    graph: crate::ir::Digest,
    p: Partition,
    controls: &[i64],
) -> crate::ir::Digest {
    let mut h = ids.start();
    h.update(b"noble.ngraph.tick-controls.v1\0");
    h.update(super::PROFILE.as_bytes());
    h.update(&graph);
    for count in [
        p.state_words,
        p.input_words,
        p.control_words,
        p.output_words,
    ] {
        h.update(&(count as u64).to_le_bytes());
    }
    h.update(&(controls.len() as u64).to_le_bytes());
    for word in controls {
        h.update(&word.to_le_bytes());
    }
    h.finish()
}

fn retained_bytes(
    checked: &crate::check::CheckedGraph,
    compiled: &crate::compiled::Compiled,
    p: Partition,
    input_words: usize,
) -> u64 {
    // Count the graph's owned bytes, packed stream/layout, workspace and
    // provisioned slot/state/output vectors. Checker-owned per-node caches
    // are conservatively charged at 128 bytes per node (actual types/layers/
    // summaries are smaller). Allocator metadata and host trace are separate.
    let words = input_words
        .saturating_add(p.state_words)
        .saturating_add(p.control_words)
        .saturating_add(p.output_words);
    crate::check::retained_bytes(checked.graph())
        .saturating_add(compiled.retained_bytes())
        .saturating_add(compiled.scratch_bytes())
        .saturating_add(
            (input_words as u64).saturating_mul(core::mem::size_of::<crate::sem::Value>() as u64),
        )
        .saturating_add((words.saturating_sub(input_words) as u64).saturating_mul(8))
        .saturating_add((checked.node_count() as u64).saturating_mul(128))
        .saturating_add(core::mem::size_of::<super::Runner>() as u64)
}

/// Prepare once outside the steady loop; the real independent schedule validator
/// must accept both the declaration and emitted instruction bodies.
pub(super) fn admit(
    ids: &dyn IdentityScheme,
    graph: Graph,
    partition: Partition,
    state: &[i64],
    controls: &[i64],
    limits: &Limits,
    reserved_bytes: u64,
) -> Result<Admitted, Refusal> {
    check_limits(limits)?;
    let checked = crate::check::check_with(ids, graph, &limits.graph).map_err(Refusal::Graph)?;
    let input_words = check_partition(checked.graph(), partition, limits)?;
    if state.len() != partition.state_words || controls.len() != partition.control_words {
        return Err(Refusal::Partition);
    }
    profile(&checked, partition)?;
    if checked.node_count() as u64 > limits.tick_work {
        return Err(Refusal::Budget("tick_work"));
    }
    let compiled = crate::compiled::Compiled::new(ids, &checked);
    let layout = compiled.packed_layout();
    let instructions = compiled.i64_instructions().ok_or(Refusal::Profile)?;
    crate::schedule::check_within(&checked, &layout, instructions, limits.schedule_work)
        .map_err(Refusal::Schedule)?;
    if compiled.scratch_bytes() > limits.graph.forward_scratch {
        return Err(Refusal::Budget("forward_scratch"));
    }
    let retained_bytes = retained_bytes(&checked, &compiled, partition, input_words);
    if retained_bytes.saturating_add(reserved_bytes) > limits.retained_bytes {
        return Err(Refusal::Budget(if reserved_bytes == 0 {
            "retained_bytes"
        } else {
            "replacement_peak_bytes"
        }));
    }
    let selection = Selection {
        graph: checked.id(),
        schedule: compiled.layout_key(),
        controls: control_id(ids, checked.id(), partition, controls),
        graph_epoch: 0,
        control_epoch: 0,
        next_tick: 0,
    };
    let workspace = compiled.workspace();
    Ok(Admitted {
        compiled,
        workspace,
        selection,
        input_words,
        retained_bytes,
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::{Limits, Partition, Runner};
    use crate::identity::{IdentityHasher, IdentityScheme};
    use crate::ir::{Digest, Graph, GraphBuilder, Op, ScalarType, SubProfile, Subject};
    use alloc::boxed::Box;

    pub(crate) struct Scheme;
    pub(crate) static IDS: Scheme = Scheme;
    struct Hasher(u64);
    impl IdentityHasher for Hasher {
        fn update(&mut self, bytes: &[u8]) {
            for b in bytes {
                self.0 = self
                    .0
                    .wrapping_mul(1_099_511_628_211)
                    .wrapping_add(u64::from(*b));
            }
        }
        fn finish(self: Box<Self>) -> Digest {
            let mut digest = [0; 32];
            digest[..8].copy_from_slice(&self.0.to_le_bytes());
            digest
        }
    }
    impl IdentityScheme for Scheme {
        fn name(&self) -> &'static str {
            "tick-test-fnv"
        }
        fn start(&self) -> Box<dyn IdentityHasher + '_> {
            Box::new(Hasher(14_695_981_039_346_656_037))
        }
    }
    pub(crate) fn graph() -> Graph {
        let mut builder =
            GraphBuilder::new(SubProfile::I64, Subject::authored(&IDS, "tick/counter"));
        let s = builder.input(ScalarType::I64);
        let x = builder.input(ScalarType::I64);
        let c = builder.input(ScalarType::I64);
        let product = builder.op(Op::I64Mul(c, x));
        let sum = builder.op(Op::I64Add(s, product));
        builder.output(sum);
        builder.output(sum);
        builder.finish()
    }
    pub(crate) fn fixture() -> Runner {
        Runner::prepare(
            &IDS,
            graph(),
            Partition {
                state_words: 1,
                input_words: 1,
                control_words: 1,
                output_words: 1,
            },
            &[0],
            &[7],
            Limits::default(),
        )
        .expect("test graph admitted")
    }
}
