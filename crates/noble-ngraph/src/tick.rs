//! Opt-in `noble-tick-i64/v1`: one checked acyclic total-I64 graph per logical tick.
//!
//! The caller owns the sequential stream. A control update becomes active only between
//! ticks; a graph replacement requires an explicit new state and controls even when
//! the declared interface has the same shape. This is not a concurrency, deadline,
//! effect-rollback, authorization, or Rust-to-Lean correspondence guarantee.

use alloc::vec::Vec;

use crate::identity::IdentityScheme;

#[path = "tick/admission.rs"]
mod admission;

/// Semantic revision of the opt-in sequential profile.
pub const PROFILE: &str = "noble-tick-i64/v1";

/// Ordered, contiguous interface partitions. Inputs are state, tick input,
/// controls; outputs are next-state, visible output. Duplicate graph output
/// nodes are allowed, so the same value can be next state and visible output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Partition {
    /// Number of state inputs and next-state outputs (positive).
    pub state_words: usize,
    /// Number of caller-supplied inputs per tick (positive).
    pub input_words: usize,
    /// Number of control inputs (positive).
    pub control_words: usize,
    /// Number of visible outputs (positive).
    pub output_words: usize,
}

/// Host-lowered caps. Graph budgets inherit the existing `/0` policy;
/// all other caps are byte or count limits for this stream, not clock limits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Existing checker limits (may only be lowered).
    pub graph: crate::check::Budgets,
    /// State input and next-state bytes.
    pub state_bytes: u64,
    /// Tick input bytes.
    pub input_bytes: u64,
    /// Control bytes.
    pub control_bytes: u64,
    /// Visible output bytes.
    pub output_bytes: u64,
    /// Bytes supplied to one control-update request.
    pub update_bytes: u64,
    /// Retained active graph, compiler, register file and stream storage; on
    /// replacement the old and candidate footprints must fit together.
    pub retained_bytes: u64,
    /// Maximum checked schedule declaration+instruction work.
    pub schedule_work: u64,
    /// Maximum evaluated graph nodes per tick.
    pub tick_work: u64,
    /// Maximum successful ticks after one prepare/reset (one tick per call).
    pub ticks: u64,
    /// Maximum host-retained trace bytes (charged by the opt-in trace recorder).
    pub trace_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            graph: crate::check::Budgets::default(),
            state_bytes: 2 * 1024,
            input_bytes: 2 * 1024,
            control_bytes: 2 * 1024,
            output_bytes: 2 * 1024,
            update_bytes: 2 * 1024,
            retained_bytes: 64 * 1024 * 1024,
            schedule_work: 32_768,
            tick_work: 4_096,
            ticks: 4_096,
            trace_bytes: 16 * 1024 * 1024,
        }
    }
}

/// One refusal class; no rejected operation changes the active tuple.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The graph uses an operation/type/shape outside the total I64 step.
    Profile,
    /// The graph's actual ordered interface does not match the partition.
    Partition,
    /// The graph checker refused the producer graph.
    Graph(crate::diag::Diagnostic),
    /// The real packed instruction stream did not pass its validator.
    Schedule(crate::schedule::StreamRefusal),
    /// A declared cap was exceeded; the stable name identifies the cap.
    Budget(&'static str),
    /// The request's graph or control epoch does not match the active tuple.
    Stale,
    /// A tick number was previously committed or is otherwise out of order.
    TickOrder,
    /// A tick, epoch, or request counter cannot advance without overflow.
    CounterExhausted,
    /// No tick may run after the configured total successful tick cap.
    Terminal,
    /// The reused evaluator refused; no output or state was committed.
    Evaluation(crate::eval::Obs),
}

/// Stable exact selected identities and sequential counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    /// Checked graph identity, not an authorization credential.
    pub graph: crate::ir::Digest,
    /// Independently validated packed layout build key.
    pub schedule: crate::ir::Digest,
    /// Exact typed control words under the injected identity scheme.
    pub controls: crate::ir::Digest,
    /// Graph epoch (starts at zero, advances on reset only).
    pub graph_epoch: u64,
    /// Control epoch (starts at zero; advances on update and reset).
    pub control_epoch: u64,
    /// Number of successful ticks since prepare or last reset.
    pub next_tick: u64,
}

/// One success borrowed from the active runner, with no partial-success variant.
#[derive(Debug, PartialEq, Eq)]
pub struct Tick<'a> {
    /// The tuple pinned at entry, before advancing the tick counter.
    pub selected: Selection,
    /// Committed state after this tick.
    pub state: &'a [i64],
    /// Committed visible output after this tick.
    pub output: &'a [i64],
    /// Graph nodes evaluated by the selected packed backend.
    pub evaluated_nodes: u64,
}

/// Fully admitted but not active control words, bound to the exact graph,
/// current graph/control epochs and tick boundary. Holding this token never
/// changes the runner; activation rechecks its expected boundary.
#[derive(Debug)]
pub struct ControlUpdate {
    expected: Selection,
    words: Vec<i64>,
    digest: crate::ir::Digest,
}

/// A sequential, explicitly resettable stream; one active graph/control
/// tuple and one reusable packed workspace, with no shared mutable state.
#[derive(Debug)]
pub struct Runner {
    compiled: crate::compiled::Compiled,
    workspace: crate::compiled::Workspace,
    partition: Partition,
    limits: Limits,
    scheme: &'static str,
    selection: Selection,
    state: Vec<i64>,
    controls: Vec<i64>,
    inputs: Vec<crate::sem::Value>,
    output: Vec<i64>,
    retained_bytes: u64,
}

impl Runner {
    /// Admits and binds the complete initial tuple before a stream is runnable.
    /// No state is inherited from an earlier runner.
    ///
    /// # Errors
    /// A checker/profile/partition/schedule or byte/work-cap refusal.
    pub fn prepare(
        ids: &dyn IdentityScheme,
        graph: crate::ir::Graph,
        partition: Partition,
        state: &[i64],
        controls: &[i64],
        limits: Limits,
    ) -> Result<Self, Refusal> {
        Self::prepare_reserved(ids, graph, partition, state, controls, limits, 0)
    }

    fn prepare_reserved(
        ids: &dyn IdentityScheme,
        graph: crate::ir::Graph,
        partition: Partition,
        state: &[i64],
        controls: &[i64],
        limits: Limits,
        reserved_bytes: u64,
    ) -> Result<Self, Refusal> {
        let admitted = admission::admit(
            ids,
            graph,
            partition,
            state,
            controls,
            &limits,
            reserved_bytes,
        )?;
        Ok(Self {
            compiled: admitted.compiled,
            workspace: admitted.workspace,
            partition,
            limits,
            scheme: ids.name(),
            selection: admitted.selection,
            state: state.to_vec(),
            controls: controls.to_vec(),
            inputs: alloc::vec![crate::sem::Value::I64(0); admitted.input_words],
            output: alloc::vec![0; partition.output_words],
            retained_bytes: admitted.retained_bytes,
        })
    }

    /// The active selected tuple and the number of successful ticks.
    #[must_use]
    pub const fn selection(&self) -> Selection {
        self.selection
    }

    /// State after the last successful tick, or the initial/reset state.
    #[must_use]
    pub fn state(&self) -> &[i64] {
        &self.state
    }

    /// Last successfully committed visible output; `None` before any tick in
    /// this graph epoch. An update does not erase the last successful output.
    #[must_use]
    pub fn output(&self) -> Option<&[i64]> {
        (self.selection.next_tick != 0).then_some(self.output.as_slice())
    }

    /// Conservative charged active footprint, excluding allocator metadata.
    #[must_use]
    pub const fn retained_bytes(&self) -> u64 {
        self.retained_bytes
    }

    /// Admitted reusable register-workspace bytes (not including the
    /// evaluator's separately allocated preflight and output vectors).
    #[must_use]
    pub fn scratch_bytes(&self) -> u64 {
        self.compiled.scratch_bytes()
    }

    /// A tick pins the selected graph, controls and state at entry. The
    /// existing packed evaluator may allocate its fresh output vector; that
    /// allocation and its preflight are NOT an allocation-free claim. The
    /// runner preallocates its input, output and register buffers once.
    ///
    /// # Errors
    /// Refusal, cancellation, exhaustion, duplicate/stale request or terminal
    /// cap preserves the current selection/state/output.
    pub fn tick(
        &mut self,
        graph_epoch: u64,
        control_epoch: u64,
        tick: u64,
        input: &[i64],
        evaluation: &mut crate::eval::Control<'_>,
    ) -> Result<Tick<'_>, Refusal> {
        self.check_epochs(graph_epoch, control_epoch)?;
        if tick != self.selection.next_tick {
            return Err(Refusal::TickOrder);
        }
        if tick >= self.limits.ticks {
            return Err(Refusal::Terminal);
        }
        let next_tick = tick.checked_add(1).ok_or(Refusal::CounterExhausted)?;
        if input.len() != self.partition.input_words {
            return Err(Refusal::Partition);
        }
        admission::charge_words(input.len(), self.limits.input_bytes, "input_bytes")?;
        for (slot, &value) in self.inputs.iter_mut().zip(&self.state) {
            *slot = crate::sem::Value::I64(value);
        }
        for (slot, &value) in self.inputs[self.partition.state_words..]
            .iter_mut()
            .zip(input)
        {
            *slot = crate::sem::Value::I64(value);
        }
        for (slot, &value) in self.inputs[self.partition.state_words + input.len()..]
            .iter_mut()
            .zip(&self.controls)
        {
            *slot = crate::sem::Value::I64(value);
        }
        // Caller may lower fuel; the admitted graph always fits the stream's
        // per-tick node cap, including input and constant nodes.
        let fuel = evaluation.fuel;
        let scratch = evaluation.forward_scratch;
        evaluation.fuel = Some(
            fuel.unwrap_or(self.limits.tick_work)
                .min(self.limits.tick_work),
        );
        evaluation.forward_scratch = Some(
            scratch
                .unwrap_or(self.limits.graph.forward_scratch)
                .min(self.limits.graph.forward_scratch),
        );
        let report = self
            .compiled
            .evaluate_in(&mut self.workspace, &self.inputs, None, evaluation);
        evaluation.fuel = fuel;
        evaluation.forward_scratch = scratch;
        let values = match report.obs {
            crate::eval::Obs::Normal(values) => values,
            other => return Err(Refusal::Evaluation(other)),
        };
        if values.len() != self.partition.state_words + self.partition.output_words
            || values
                .iter()
                .any(|v| !matches!(v, crate::sem::Value::I64(_)))
        {
            return Err(Refusal::Profile);
        }
        // Profile admission fixed these exact output arities/types. No state
        // or output is mutated until the evaluator has produced all values.
        let (new_state, new_output) = values.split_at(self.partition.state_words);
        for (dest, value) in self.state.iter_mut().zip(new_state) {
            if let crate::sem::Value::I64(bits) = value {
                *dest = *bits;
            }
        }
        for (dest, value) in self.output.iter_mut().zip(new_output) {
            if let crate::sem::Value::I64(bits) = value {
                *dest = *bits;
            }
        }
        let selected = self.selection;
        self.selection.next_tick = next_tick;
        Ok(Tick {
            selected,
            state: &self.state,
            output: &self.output,
            evaluated_nodes: report.evaluated_nodes,
        })
    }

    /// Validate and bind a control request without activating it. An update
    /// admitted before a tick cannot activate after that tick.
    ///
    /// # Errors
    /// Stale epochs, partition mismatch, cap or epoch exhaustion.
    pub fn prepare_control_update(
        &self,
        graph_epoch: u64,
        control_epoch: u64,
        controls: &[i64],
        ids: &dyn IdentityScheme,
    ) -> Result<ControlUpdate, Refusal> {
        self.check_epochs(graph_epoch, control_epoch)?;
        if ids.name() != self.scheme || controls.len() != self.partition.control_words {
            return Err(Refusal::Partition);
        }
        admission::charge_words(controls.len(), self.limits.update_bytes, "update_bytes")?;
        self.selection
            .control_epoch
            .checked_add(1)
            .ok_or(Refusal::CounterExhausted)?;
        let digest = admission::control_id(ids, self.selection.graph, self.partition, controls);
        Ok(ControlUpdate {
            expected: self.selection,
            words: controls.to_vec(),
            digest,
        })
    }

    /// Activate an admitted update exactly at its selected boundary. The
    /// token is consumed regardless of outcome; a refusal preserves the
    /// complete active tuple.
    ///
    /// # Errors
    /// Stale graph, controls, or tick boundary; epoch exhaustion.
    pub fn activate_control_update(
        &mut self,
        admitted: ControlUpdate,
    ) -> Result<Selection, Refusal> {
        if admitted.expected != self.selection {
            return Err(Refusal::Stale);
        }
        let next_epoch = self
            .selection
            .control_epoch
            .checked_add(1)
            .ok_or(Refusal::CounterExhausted)?;
        self.controls = admitted.words;
        self.selection.controls = admitted.digest;
        self.selection.control_epoch = next_epoch;
        Ok(self.selection)
    }

    /// Admit and activate one complete control update between ticks.
    ///
    /// # Errors
    /// Stale, malformed, oversized or exhausted updates preserve the tuple.
    pub fn update_controls(
        &mut self,
        graph_epoch: u64,
        control_epoch: u64,
        controls: &[i64],
        ids: &dyn IdentityScheme,
    ) -> Result<Selection, Refusal> {
        let admitted = self.prepare_control_update(graph_epoch, control_epoch, controls, ids)?;
        self.activate_control_update(admitted)
    }

    /// Replace graph/partition only by explicitly supplying fresh initial
    /// state and controls. The candidate is fully admitted beside the old
    /// tuple before activation, then old storage is dropped. Tick numbering
    /// restarts at zero; graph AND control epochs advance monotonically.
    ///
    /// # Errors
    /// Any refusal preserves the old complete tuple, including last output.
    pub fn reset(
        &mut self,
        ids: &dyn IdentityScheme,
        graph_epoch: u64,
        control_epoch: u64,
        graph: crate::ir::Graph,
        partition: Partition,
        state: &[i64],
        controls: &[i64],
    ) -> Result<Selection, Refusal> {
        self.check_epochs(graph_epoch, control_epoch)?;
        if ids.name() != self.scheme {
            return Err(Refusal::Profile);
        }
        let next_graph = graph_epoch
            .checked_add(1)
            .ok_or(Refusal::CounterExhausted)?;
        let next_controls = control_epoch
            .checked_add(1)
            .ok_or(Refusal::CounterExhausted)?;
        let mut candidate = Self::prepare_reserved(
            ids,
            graph,
            partition,
            state,
            controls,
            self.limits.clone(),
            self.retained_bytes,
        )?;
        candidate.selection.graph_epoch = next_graph;
        candidate.selection.control_epoch = next_controls;
        *self = candidate;
        Ok(self.selection)
    }

    fn check_epochs(&self, graph: u64, controls: u64) -> Result<(), Refusal> {
        if graph != self.selection.graph_epoch || controls != self.selection.control_epoch {
            Err(Refusal::Stale)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Refusal, Runner};

    #[test]
    fn counters_refuse_overflow_without_mutation() {
        // Internal boundary injection avoids waiting 2^64 requests to exercise
        // arithmetic exhaustion; public construction always starts at zero.
        let mut runner: Runner = super::admission::tests::fixture();
        runner.selection.control_epoch = u64::MAX;
        let selected = runner.selection;
        assert_eq!(
            runner.update_controls(0, u64::MAX, &[7], &super::admission::tests::IDS),
            Err(Refusal::CounterExhausted)
        );
        assert_eq!(runner.selection, selected);
        runner.selection.graph_epoch = u64::MAX;
        let selected = runner.selection;
        let graph = super::admission::tests::graph();
        let partition = runner.partition;
        assert_eq!(
            runner.reset(
                &super::admission::tests::IDS,
                u64::MAX,
                u64::MAX,
                graph,
                partition,
                &[0],
                &[7]
            ),
            Err(Refusal::CounterExhausted)
        );
        assert_eq!(runner.selection, selected);
    }
}
