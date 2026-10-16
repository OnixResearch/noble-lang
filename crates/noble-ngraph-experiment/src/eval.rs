//! D8 forward observations, evaluation preflight, scratch accounting (D12)
//! and the normative CPU reference interpreter (leg L3).

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::check::CheckedGraph;
use crate::diag::{BudgetKind, Code, Diagnostic, Site};
use crate::facts;
use crate::ir::{AffineOperand, Fact, Literal, NodeId, Op, ScalarType, SlotKind};
use crate::sem::{self, Value};
use crate::snapshot::ParamSnapshot;

/// `Obs(G, inputs, snapshot)` (D8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Obs {
    /// Fresh immutable outputs, compared by `≡obs`.
    Normal(Vec<Value>),
    /// Refused before evaluation (admission, snapshot, input validation, budget).
    Refused(Diagnostic),
    /// `G-FACT-FALSE(node, fact)`: a guard (or a refusing index read) failed; no outputs.
    GuardFailed {
        /// Failing node.
        node: NodeId,
        /// Failing fact.
        fact: Fact,
    },
    /// Runtime exhaustion of a host budget.
    Exhausted(BudgetKind),
    /// `C-CANCELLED` at a layer boundary.
    Cancelled,
}

impl Obs {
    /// `≡obs` on observations: outputs bitwise except one NaN class; other
    /// classes compare by code/node/fact (refusal detail text is ignored).
    #[must_use]
    pub fn obs_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Normal(a), Self::Normal(b)) => sem::obs_eq_all(a, b),
            (Self::Refused(a), Self::Refused(b)) => a.code == b.code && a.site == b.site,
            (
                Self::GuardFailed { node: n1, fact: f1 },
                Self::GuardFailed { node: n2, fact: f2 },
            ) => n1 == n2 && f1 == f2,
            (Self::Exhausted(a), Self::Exhausted(b)) => a == b,
            (Self::Cancelled, Self::Cancelled) => true,
            _ => false,
        }
    }

    /// Outputs of a normal observation.
    #[must_use]
    pub fn outputs(&self) -> Option<&[Value]> {
        match self {
            Self::Normal(v) => Some(v),
            _ => None,
        }
    }
}

/// Instrumented scratch counters (B6). Every evaluation acquires its scratch
/// through a [`ScratchLease`] whose drop releases it exactly once.
#[derive(Debug, Default)]
pub struct ScratchCounters {
    /// Leases acquired.
    pub acquired: AtomicU64,
    /// Leases released.
    pub released: AtomicU64,
    /// Bytes currently leased.
    pub live_bytes: AtomicU64,
    /// Peak leased bytes.
    pub peak_bytes: AtomicU64,
}

impl ScratchCounters {
    /// `(acquired, released, live_bytes)`.
    #[must_use]
    pub fn snapshot(&self) -> (u64, u64, u64) {
        (
            self.acquired.load(Ordering::SeqCst),
            self.released.load(Ordering::SeqCst),
            self.live_bytes.load(Ordering::SeqCst),
        )
    }
}

/// A scratch lease; its drop is the single release.
#[derive(Debug)]
pub struct ScratchLease<'a> {
    counters: Option<&'a ScratchCounters>,
    bytes: u64,
}

impl<'a> ScratchLease<'a> {
    /// Acquires `bytes` of scratch.
    #[must_use]
    pub fn acquire(counters: Option<&'a ScratchCounters>, bytes: u64) -> Self {
        if let Some(c) = counters {
            c.acquired.fetch_add(1, Ordering::SeqCst);
            let live = c.live_bytes.fetch_add(bytes, Ordering::SeqCst) + bytes;
            c.peak_bytes.fetch_max(live, Ordering::SeqCst);
        }
        Self { counters, bytes }
    }
}

impl Drop for ScratchLease<'_> {
    fn drop(&mut self) {
        if let Some(c) = self.counters {
            c.released.fetch_add(1, Ordering::SeqCst);
            c.live_bytes.fetch_sub(self.bytes, Ordering::SeqCst);
        }
    }
}

/// Host evaluation controls: fuel (node evaluations), cancellation probe
/// (called at each layer boundary with the layer about to start) and scratch
/// counters.
#[derive(Default)]
pub struct Control<'a> {
    /// Maximum node evaluations; exceeding it gives `Exhausted(Fuel)`.
    pub fuel: Option<u64>,
    /// Returns `true` to cancel before starting the given layer.
    pub cancel: Option<&'a dyn Fn(u32) -> bool>,
    /// Scratch instrumentation.
    pub scratch: Option<&'a ScratchCounters>,
    /// Forward scratch budget in bytes (defaults to the `/0` budget).
    pub forward_scratch: Option<u64>,
}

/// An evaluation result with the number of graph nodes evaluated.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Observation.
    pub obs: Obs,
    /// Graph nodes evaluated (0 for every refusal before evaluation).
    pub evaluated_nodes: u64,
}

impl Report {
    const fn refused(d: Diagnostic) -> Self {
        Self {
            obs: Obs::Refused(d),
            evaluated_nodes: 0,
        }
    }
}

/// Slot values after preflight, indexed by slot.
pub(crate) type SlotValues = Vec<Value>;

/// Evaluation preflight shared by all evaluators: snapshot identity and
/// layout (D11), input arity/types (`E-INTERFACE`), input-validated facts
/// (`E-FACT-INPUT`, including param facts on the snapshot values).
///
/// # Errors
/// The refusal diagnostic.
pub fn preflight(
    checked: &CheckedGraph,
    inputs: &[Value],
    snapshot: Option<&ParamSnapshot>,
) -> Result<SlotValues, Diagnostic> {
    let g = checked.graph();
    let has_params = g.slots.iter().any(|s| s.kind == SlotKind::Param);
    match (snapshot, has_params) {
        (Some(s), _) => s.check_use(checked)?,
        (None, true) => {
            return Err(Diagnostic::new(
                Code::SnapshotLayout,
                Site::Graph,
                "graph has param slots but no snapshot was supplied",
            ));
        }
        (None, false) => {}
    }
    let mut values = Vec::with_capacity(g.slots.len());
    let mut it = inputs.iter();
    let mut param_pos = 0usize;
    for (k, slot) in g.slots.iter().enumerate() {
        let k32 = k as u32;
        let v = match slot.kind {
            SlotKind::Input => match it.next() {
                Some(v) if v.ty() == slot.ty => *v,
                Some(v) => {
                    return Err(Diagnostic::new(
                        Code::Interface,
                        Site::Slot(k32),
                        format!("{} input for {} slot", v.ty().name(), slot.ty.name()),
                    ));
                }
                None => {
                    return Err(Diagnostic::new(
                        Code::Interface,
                        Site::Slot(k32),
                        "missing input",
                    ));
                }
            },
            SlotKind::Param => {
                // `check_use` established `snapshot.slots() == param_slots()`, so
                // the j-th param slot is snapshot position j.
                let pos = param_pos;
                param_pos += 1;
                Value::F32(snapshot.and_then(|s| s.bits_at(pos)).ok_or_else(|| {
                    Diagnostic::new(Code::SnapshotLayout, Site::Slot(k32), "param missing")
                })?)
            }
        };
        values.push(v);
    }
    if it.next().is_some() {
        return Err(Diagnostic::new(
            Code::Interface,
            Site::Graph,
            "too many inputs",
        ));
    }
    for inf in checked.input_facts() {
        let v = values[inf.slot as usize];
        let group = facts::group_values(&inf.fact, |m| match g.nodes[m.index()].op {
            Op::Input(s) | Op::Param(s) => values[s as usize],
            _ => v,
        });
        if !facts::holds(&inf.fact, v, &group, None) {
            return Err(Diagnostic::new(
                Code::FactInput,
                Site::Slot(inf.slot),
                format!("input fails its {} fact", inf.fact.kind_name()),
            )
            .with_fact(inf.fact.clone()));
        }
    }
    Ok(values)
}

/// Bytes of reference-interpreter scratch for `checked`.
#[must_use]
pub fn reference_scratch_bytes(checked: &CheckedGraph) -> u64 {
    (checked.node_count() * std::mem::size_of::<Value>()) as u64
}

pub(crate) fn scratch_preflight(bytes: u64, control: &Control<'_>) -> Result<(), Diagnostic> {
    let limit = control
        .forward_scratch
        .unwrap_or_else(|| crate::check::Budgets::default().forward_scratch);
    if bytes > limit {
        Err(Diagnostic::budget(BudgetKind::ForwardScratch, limit, bytes))
    } else {
        Ok(())
    }
}

/// The fact an `index` read fails under `refuse`, for `G-FACT-FALSE`.
#[must_use]
pub fn index_refusal_fact(len: usize) -> Fact {
    #[allow(clippy::cast_precision_loss)]
    let hi = (len.saturating_sub(1)) as f32;
    Fact::Range {
        lo: Literal::F32(0),
        hi: Literal::f32(hi),
    }
}

/// Evaluates one value node given operand access. Returns `Err(fact)` for a
/// refusing index read. Guards are handled by the caller.
pub(crate) fn eval_node(
    op: &Op,
    slot: &[Value],
    get: impl Fn(NodeId) -> Value,
) -> Result<Value, Fact> {
    let f = |x: NodeId| match get(x) {
        Value::F32(b) => f32::from_bits(b),
        _ => f32::NAN,
    };
    let i = |x: NodeId| match get(x) {
        Value::I64(v) => v,
        _ => 0,
    };
    Ok(match op {
        Op::Input(k) | Op::Param(k) => slot[*k as usize],
        Op::Const(l) => Value::of_literal(*l),
        Op::I64Add(a, b) => Value::I64(sem::i64_add(i(*a), i(*b))),
        Op::I64Sub(a, b) => Value::I64(sem::i64_sub(i(*a), i(*b))),
        Op::I64Mul(a, b) => Value::I64(sem::i64_mul(i(*a), i(*b))),
        Op::I64Eq(a, b) => Value::Bool(i(*a) == i(*b)),
        Op::F32Add(a, b) => Value::f32(sem::add(f(*a), f(*b))),
        Op::F32Sub(a, b) => Value::f32(sem::sub(f(*a), f(*b))),
        Op::F32Mul(a, b) => Value::f32(sem::mul(f(*a), f(*b))),
        Op::F32Affine { bias, terms } => Value::f32(sem::affine(
            f32::from_bits(*bias),
            terms.iter().map(|t| {
                (
                    f32::from_bits(t.coefficient),
                    match t.operand {
                        AffineOperand::One => 1.0,
                        AffineOperand::Node(x) => f(x),
                    },
                )
            }),
        )),
        Op::F32Relu(x) => Value::f32(sem::relu(f(*x))),
        Op::F32Reglu { left, gate } => Value::f32(sem::reglu(f(*left), f(*gate))),
        Op::F32Step(s) => Value::f32(sem::step(f(*s))),
        Op::F32Select { bit, off, on } => {
            let (Value::F32(b), Value::F32(o0), Value::F32(o1)) = (get(*bit), get(*off), get(*on))
            else {
                return Ok(Value::F32(sem::POS_ZERO));
            };
            Value::F32(sem::select(b, o0, o1))
        }
        Op::F32Index {
            index,
            table,
            below,
            above,
        } => match sem::index_position(f(*index), table.len(), *below, *above) {
            Ok(Some(k)) => get(table[k]),
            Ok(None) => Value::F32(sem::POS_ZERO),
            Err(()) => return Err(index_refusal_fact(table.len())),
        },
        Op::Guard { .. } => Value::Bool(true),
    })
}

/// Tests a guard root.
pub(crate) fn guard_holds(
    checked: &CheckedGraph,
    fact: &Fact,
    value: NodeId,
    get: impl Fn(NodeId) -> Value,
) -> bool {
    let group = facts::group_values(fact, &get);
    let shape = match &checked.graph().nodes[value.index()].op {
        Op::F32Index { table, .. } => Some(table.len() as u32),
        _ => None,
    };
    facts::holds(fact, get(value), &group, shape)
}

/// Graph outputs as fresh values.
pub(crate) fn collect_outputs(checked: &CheckedGraph, get: impl Fn(NodeId) -> Value) -> Vec<Value> {
    checked.graph().outputs.iter().map(|o| get(*o)).collect()
}

/// L3: the normative CPU reference interpreter. Visits nodes in canonical
/// order; the first failing guard (or refusing index read) in canonical
/// order is the observation. Cancellation is probed when evaluation first
/// enters a new layer; fuel is charged per node.
#[must_use]
pub fn reference(
    checked: &CheckedGraph,
    inputs: &[Value],
    snapshot: Option<&ParamSnapshot>,
    control: &Control<'_>,
) -> Report {
    let slots = match preflight(checked, inputs, snapshot) {
        Ok(v) => v,
        Err(d) => return Report::refused(d),
    };
    let bytes = reference_scratch_bytes(checked);
    if let Err(d) = scratch_preflight(bytes, control) {
        return Report::refused(d);
    }
    let _lease = ScratchLease::acquire(control.scratch, bytes);
    let g = checked.graph();
    let mut vals: Vec<Value> = Vec::with_capacity(g.nodes.len());
    let evaluated = Cell::new(0u64);
    let mut entered: Option<u32> = None;
    for (i, node) in g.nodes.iter().enumerate() {
        let id = NodeId(i as u32);
        let layer = checked.layer(id);
        if entered.is_none_or(|e| layer > e) {
            entered = Some(layer);
            if control.cancel.is_some_and(|c| c(layer)) {
                return Report {
                    obs: Obs::Cancelled,
                    evaluated_nodes: evaluated.get(),
                };
            }
        }
        if control.fuel.is_some_and(|f| evaluated.get() >= f) {
            return Report {
                obs: Obs::Exhausted(BudgetKind::Fuel),
                evaluated_nodes: evaluated.get(),
            };
        }
        evaluated.set(evaluated.get() + 1);
        let get = |x: NodeId| vals[x.index()];
        if let Op::Guard { fact, value } = &node.op {
            if !guard_holds(checked, fact, *value, get) {
                return Report {
                    obs: Obs::GuardFailed {
                        node: id,
                        fact: fact.clone(),
                    },
                    evaluated_nodes: evaluated.get(),
                };
            }
            vals.push(Value::Bool(true));
            continue;
        }
        match eval_node(&node.op, &slots, get) {
            Ok(v) => vals.push(v),
            Err(fact) => {
                return Report {
                    obs: Obs::GuardFailed { node: id, fact },
                    evaluated_nodes: evaluated.get(),
                };
            }
        }
    }
    Report {
        obs: Obs::Normal(collect_outputs(checked, |x| vals[x.index()])),
        evaluated_nodes: evaluated.get(),
    }
}

/// Helper: `F32` inputs from `f32` values.
#[must_use]
pub fn f32_inputs(values: &[f32]) -> Vec<Value> {
    values.iter().map(|v| Value::f32(*v)).collect()
}

/// Helper: `I64` inputs.
#[must_use]
pub fn i64_inputs(values: &[i64]) -> Vec<Value> {
    values.iter().map(|v| Value::I64(*v)).collect()
}

/// The scalar type of a value node's output (for evaluators' buffers).
#[must_use]
pub fn value_type(checked: &CheckedGraph, n: NodeId) -> Option<ScalarType> {
    checked.ty(n)
}
