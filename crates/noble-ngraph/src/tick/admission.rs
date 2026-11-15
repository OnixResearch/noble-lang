//! Checks the actual graph interface and packed stream, not producer metadata.

use crate::identity::IdentityScheme;

pub(super) struct Admitted {
    pub compiled: crate::compiled::Compiled,
    pub workspace: crate::compiled::Workspace,
    pub selection: super::Selection,
    pub input_words: usize,
    pub retained_bytes: u64,
}

/// Refuse arithmetic overflow before multiplying a word count by 8.
pub(super) fn charge_words(
    words: usize,
    cap: u64,
    label: &'static str,
) -> Result<(), super::Refusal> {
    let bytes = u64::try_from(words)
        .ok()
        .and_then(|n| n.checked_mul(8))
        .ok_or(super::Refusal::Budget(label))?;
    if bytes > cap {
        Err(super::Refusal::Budget(label))
    } else {
        Ok(())
    }
}

fn check_partition(
    graph: &crate::ir::Graph,
    p: super::Partition,
    limits: &super::Limits,
) -> Result<usize, super::Refusal> {
    let input_words = p
        .state_words
        .checked_add(p.input_words)
        .and_then(|n| n.checked_add(p.control_words))
        .ok_or(super::Refusal::Partition)?;
    let output_words = p
        .state_words
        .checked_add(p.output_words)
        .ok_or(super::Refusal::Partition)?;
    if p.state_words == 0 || p.input_words == 0 || p.output_words == 0 {
        return Err(super::Refusal::Partition);
    }
    if input_words > 256 || output_words > 256 {
        return Err(super::Refusal::Partition);
    }
    if graph.slots.len() != input_words
        || graph.outputs.len() != output_words
        || graph
            .slots
            .iter()
            .any(|s| s.kind != crate::ir::SlotKind::Input || s.ty != crate::ir::ScalarType::I64)
    {
        return Err(super::Refusal::Partition);
    }
    if graph.input_shape.is_some() || graph.output_shape.is_some() {
        return Err(super::Refusal::Partition);
    }
    charge_words(p.state_words, limits.state_bytes, "state_bytes")?;
    charge_words(p.input_words, limits.input_bytes, "input_bytes")?;
    charge_words(p.control_words, limits.control_bytes, "control_bytes")?;
    charge_words(p.output_words, limits.output_bytes, "output_bytes")?;
    // CheckedGraph::ty is independent of the producer's claim about outputs.
    Ok(input_words)
}

fn profile(
    checked: &crate::check::CheckedGraph,
    p: super::Partition,
) -> Result<(), super::Refusal> {
    let graph = checked.graph();
    let interface_words = p.state_words + p.input_words + p.control_words;
    if graph.profile != crate::ir::SubProfile::I64
        || !graph.facts.is_empty()
        || graph.nodes.len() < interface_words
    {
        return Err(super::Refusal::Profile);
    }
    if graph.nodes[..interface_words]
        .iter()
        .enumerate()
        .any(|(slot, node)| !matches!(node.op, crate::ir::Op::Input(k) if k as usize == slot))
        || graph.nodes[interface_words..]
            .iter()
            .any(|node| matches!(node.op, crate::ir::Op::Input(_)))
    {
        return Err(super::Refusal::Profile);
    }
    if graph.nodes.iter().any(|n| {
        !matches!(
            n.op,
            crate::ir::Op::Input(_)
                | crate::ir::Op::Const(crate::ir::Literal::I64(_))
                | crate::ir::Op::I64Add(_, _)
                | crate::ir::Op::I64Sub(_, _)
                | crate::ir::Op::I64Mul(_, _)
        )
    }) || graph
        .outputs
        .iter()
        .any(|&n| checked.ty(n) != Some(crate::ir::ScalarType::I64))
        || p.state_words + p.output_words != graph.outputs.len()
    {
        return Err(super::Refusal::Profile);
    }
    Ok(())
}

fn check_limits(limits: &super::Limits) -> Result<(), super::Refusal> {
    limits
        .graph
        .validate_policy()
        .map_err(super::Refusal::Graph)?;
    let max = super::Limits::default();
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
        return Err(super::Refusal::Budget("policy"));
    }
    Ok(())
}

/// Digest exact control words, partition and graph under a typed, domain-separated
/// encoding. This is an identity only, never authorization or semantic checking.
pub(super) fn control_id(
    ids: &dyn IdentityScheme,
    graph: crate::ir::Digest,
    p: super::Partition,
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
    p: super::Partition,
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
    start: super::Start<'_>,
    limits: &super::Limits,
    reserved_bytes: u64,
) -> Result<Admitted, super::Refusal> {
    let super::Start {
        graph,
        partition,
        state,
        controls,
    } = start;
    check_limits(limits)?;
    let checked =
        crate::check::check_with(ids, graph, &limits.graph).map_err(super::Refusal::Graph)?;
    let input_words = check_partition(checked.graph(), partition, limits)?;
    if state.len() != partition.state_words || controls.len() != partition.control_words {
        return Err(super::Refusal::Partition);
    }
    profile(&checked, partition)?;
    if checked.node_count() as u64 > limits.tick_work {
        return Err(super::Refusal::Budget("tick_work"));
    }
    let compiled = crate::compiled::Compiled::new(ids, &checked);
    let layout = compiled.packed_layout();
    let instructions = compiled.i64_instructions().ok_or(super::Refusal::Profile)?;
    crate::schedule::check_within(&checked, &layout, instructions, limits.schedule_work)
        .map_err(super::Refusal::Schedule)?;
    if compiled.scratch_bytes() > limits.graph.forward_scratch {
        return Err(super::Refusal::Budget("forward_scratch"));
    }
    let retained_bytes = retained_bytes(&checked, &compiled, partition, input_words);
    if retained_bytes.saturating_add(reserved_bytes) > limits.retained_bytes {
        return Err(super::Refusal::Budget(if reserved_bytes == 0 {
            "retained_bytes"
        } else {
            "replacement_peak_bytes"
        }));
    }
    let selection = super::Selection {
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
    use crate::identity::{IdentityHasher, IdentityScheme};

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
        fn finish(self: alloc::boxed::Box<Self>) -> crate::ir::Digest {
            let mut digest = [0; 32];
            digest[..8].copy_from_slice(&self.0.to_le_bytes());
            digest
        }
    }
    impl IdentityScheme for Scheme {
        fn name(&self) -> &'static str {
            "tick-test-fnv"
        }
        fn start(&self) -> alloc::boxed::Box<dyn IdentityHasher + '_> {
            alloc::boxed::Box::new(Hasher(14_695_981_039_346_656_037))
        }
    }
    pub(crate) fn graph() -> crate::ir::Graph {
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            crate::ir::Subject::authored(&IDS, "tick/counter"),
        );
        let s = builder.input(crate::ir::ScalarType::I64);
        let x = builder.input(crate::ir::ScalarType::I64);
        let c = builder.input(crate::ir::ScalarType::I64);
        let product = builder.op(crate::ir::Op::I64Mul(c, x));
        let sum = builder.op(crate::ir::Op::I64Add(s, product));
        builder.output(sum);
        builder.output(sum);
        builder.finish()
    }
    pub(crate) fn fixture() -> super::super::Runner {
        super::super::Runner::prepare(
            &IDS,
            super::super::Start {
                graph: graph(),
                partition: super::super::Partition {
                    state_words: 1,
                    input_words: 1,
                    control_words: 1,
                    output_words: 1,
                },
                state: &[0],
                controls: &[7],
            },
            super::super::Limits::default(),
        )
        .expect("test graph admitted")
    }
}
