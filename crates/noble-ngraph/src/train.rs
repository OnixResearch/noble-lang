//! D11 training contract, `adam/0`, immutable snapshot publication and
//! replayable provenance. The training loop, PRNG and optimizer arithmetic
//! are host-side (D4); the graph is never modified.
//!
//! `Numeric-Graph-v1` (design D10) closes three gaps of the Phase B trainer:
//! every data point is evaluated with the reference failure semantics (input
//! facts, guards and refusing index reads stop the step), the loss graph's
//! interface is admitted, and replay starts from the recorded parent.

use alloc::rc::Rc;
use alloc::{boxed::Box, format, string::String, vec, vec::Vec};

use crate::check::{Budgets, CheckedGraph};
use crate::compiled::COMPILER_REVISION;
use crate::diag::{BudgetKind, Code, Diagnostic, Site};
use crate::eval::{self, ScratchCounters, ScratchLease};
use crate::grad::{self, DerivOptions, DERIV_POLICY};
use crate::identity::IdentityScheme;
use crate::ir::{
    Canon, Digest, SubProfile, Subject, FACT_RULE_REVISION, IR_VERSION, SEMANTIC_REVISION,
};
use crate::prng::SplitMix64;
use crate::sem::{self, Value};
use crate::snapshot::ParamSnapshot;

/// Optimizer revision.
pub const ADAM: &str = "adam/0";

/// Library tool revision recorded in training provenance.
pub const TOOL_REVISION: &str = "noble-ngraph/0";

/// `adam/0` hyperparameters (exact binary64 bits are recorded).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Adam {
    /// Learning rate.
    pub lr: f64,
    /// β₁.
    pub beta1: f64,
    /// β₂.
    pub beta2: f64,
    /// ε.
    pub eps: f64,
}

impl Default for Adam {
    fn default() -> Self {
        Self {
            lr: 0.02,
            beta1: 0.9,
            beta2: 0.999,
            eps: 1e-8,
        }
    }
}

impl Adam {
    /// Canonical description with exact bits.
    #[must_use]
    pub fn describe(&self) -> String {
        format!(
            "{ADAM}(lr=0x{:016x},beta1=0x{:016x},beta2=0x{:016x},eps=0x{:016x};binary64 RNE, no FMA, running beta products, no pow; no weight decay/clipping/clamp)",
            self.lr.to_bits(),
            self.beta1.to_bits(),
            self.beta2.to_bits(),
            self.eps.to_bits()
        )
    }

    /// Whether every hyperparameter has the same binary64 encoding.
    #[must_use]
    pub fn same_bits(&self, other: &Self) -> bool {
        self.lr.to_bits() == other.lr.to_bits()
            && self.beta1.to_bits() == other.beta1.to_bits()
            && self.beta2.to_bits() == other.beta2.to_bits()
            && self.eps.to_bits() == other.eps.to_bits()
    }

    /// Admits the hyperparameters `adam/0` defines (design D7 of
    /// `numeric-graph-v1`): every value finite, `lr > 0`, `eps > 0` and
    /// `0 ≤ β₁, β₂ < 1`, compared as numbers (so `-0` is a zero β). Outside
    /// this domain the bias correction `1 − βᵗ` or the denominator
    /// `√v̂ + eps` can be zero, the moments never decay, or the step does not
    /// descend.
    ///
    /// # Errors
    /// `E-PROFILE` naming the first value outside the domain.
    pub fn admit(&self) -> Result<(), Diagnostic> {
        let refuse = |name: &str, value: f64, rule: &str| {
            Err(Diagnostic::new(
                Code::Profile,
                Site::Graph,
                format!(
                    "{ADAM} {name} 0x{:016x} is outside its domain ({rule})",
                    value.to_bits()
                ),
            ))
        };
        if !(self.lr.is_finite() && self.lr > 0.0) {
            return refuse("lr", self.lr, "finite, > 0");
        }
        for (name, beta) in [("beta1", self.beta1), ("beta2", self.beta2)] {
            if !(0.0..1.0).contains(&beta) {
                return refuse(name, beta, "0 <= beta < 1");
            }
        }
        if !(self.eps.is_finite() && self.eps > 0.0) {
            return refuse("eps", self.eps, "finite, > 0");
        }
        Ok(())
    }
}

/// `adam/0` optimizer state (binary64), immutable once published.
#[derive(Clone, Debug, PartialEq)]
pub struct AdamState {
    /// First moments.
    pub m: Vec<f64>,
    /// Second moments.
    pub v: Vec<f64>,
    /// Running `β₁ᵗ`.
    pub b1t: f64,
    /// Running `β₂ᵗ`.
    pub b2t: f64,
    /// Steps taken.
    pub t: u64,
}

impl AdamState {
    /// Zero state for `n` params.
    #[must_use]
    pub fn new(n: usize) -> Self {
        Self {
            m: vec![0.0; n],
            v: vec![0.0; n],
            b1t: 1.0,
            b2t: 1.0,
            t: 0,
        }
    }

    /// Digest of the exact state bytes.
    #[must_use]
    pub fn digest(&self, ids: &dyn IdentityScheme) -> Digest {
        let mut c = Canon::new(ids, "noble.ngraph.adam-state.v0");
        c.u64(self.t);
        c.u64(self.b1t.to_bits());
        c.u64(self.b2t.to_bits());
        c.u64(self.m.len() as u64);
        for (m, v) in self.m.iter().zip(&self.v) {
            c.u64(m.to_bits());
            c.u64(v.to_bits());
        }
        c.finish()
    }
}

/// One training point: model inputs and loss targets.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    /// Model `input` slot values (binary32).
    pub inputs: Vec<f32>,
    /// Loss targets (binary32).
    pub targets: Vec<f32>,
}

/// Ordered training-data digest.
#[must_use]
pub fn data_digest(ids: &dyn IdentityScheme, points: &[Point]) -> Digest {
    let mut c = Canon::new(ids, "noble.ngraph.training-data.v0");
    c.u64(points.len() as u64);
    for p in points {
        c.u64(p.inputs.len() as u64);
        for x in &p.inputs {
            c.u32(x.to_bits());
        }
        c.u64(p.targets.len() as u64);
        for t in &p.targets {
            c.u32(t.to_bits());
        }
    }
    c.finish()
}

/// The training contract (D11).
#[derive(Clone, Debug)]
pub struct Contract {
    /// Model graph.
    pub model: CheckedGraph,
    /// Separately admitted loss graph `(outputs, targets) → F32`.
    pub loss: CheckedGraph,
    /// Trainable param slots (all must be `param`).
    pub params: Vec<u32>,
    /// Derivative options.
    pub deriv: DerivOptions,
    /// Optimizer hyperparameters.
    pub adam: Adam,
    /// Seed (`splitmix64/0`) used for initialization.
    pub seed: u64,
    /// Ordered training data.
    pub data: Vec<Point>,
}

/// Training provenance (D11): everything needed to replay a snapshot, as
/// structured fields (design D6 of `numeric-graph-v1`).
#[derive(Clone, Debug, PartialEq)]
pub struct Provenance {
    /// Parent snapshot id.
    pub parent: Digest,
    /// Model subject (kind, identity and closure).
    pub subject: Subject,
    /// Model `GraphLogicalId`.
    pub graph: Digest,
    /// Model sub-profile (with [`SEMANTIC_REVISION`]).
    pub profile: SubProfile,
    /// Fact-binding digest of the model.
    pub facts: Digest,
    /// Loss graph id.
    pub loss: Digest,
    /// Training-data digest.
    pub data: Digest,
    /// Derivative opt-ins (policy [`DERIV_POLICY`]).
    pub deriv: DerivOptions,
    /// Seed (PRNG [`crate::prng::PRNG`]).
    pub seed: u64,
    /// Optimizer hyperparameters (optimizer [`ADAM`]).
    pub optimizer: Adam,
    /// Steps.
    pub steps: u64,
    /// Tool revisions.
    pub tools: String,
    /// Resulting snapshot id.
    pub result: Digest,
    /// Resulting optimizer-state digest.
    pub optimizer_state: Digest,
}

/// The result of a training call: the last published snapshot pair and its
/// provenance, plus the per-step losses.
#[derive(Clone, Debug)]
pub struct Trained {
    /// Initial snapshot `S₀` (the parent).
    pub initial: ParamSnapshot,
    /// Last published snapshot.
    pub snapshot: ParamSnapshot,
    /// Last published optimizer state.
    pub state: Rc<AdamState>,
    /// Loss before each step, then the final loss (binary64 mean).
    pub losses: Vec<f64>,
    /// Provenance of `snapshot`.
    pub provenance: Provenance,
}

/// Why a training call stopped without completing all steps.
#[derive(Clone, Debug)]
pub struct Stopped {
    /// The refusal (`E-NONFINITE-UPDATE`, `C-CANCELLED`, `G-FACT-FALSE`,
    /// `E-FACT-INPUT`, admission codes).
    pub diagnostic: Diagnostic,
    /// The last published snapshot (unchanged by the failed step), if any.
    pub last: Option<ParamSnapshot>,
    /// Steps published before stopping.
    pub published_steps: u64,
}

/// Deterministic initialization: `splitmix64/0(seed)`, uniform in
/// `[-0.5, 0.5)` in binary64, named RNE conversion to binary32.
#[must_use]
pub fn init_params(seed: u64, n: usize) -> Vec<f32> {
    let mut r = SplitMix64::new(seed);
    (0..n).map(|_| (r.next_f64() - 0.5) as f32).collect()
}

/// Tool revisions recorded in provenance.
#[must_use]
pub fn tools() -> String {
    format!(
        "{TOOL_REVISION}; {IR_VERSION}; {SEMANTIC_REVISION}; {FACT_RULE_REVISION}; {COMPILER_REVISION}"
    )
}

/// Static training admission of a model, its trainable params, its
/// derivative options and its loss graph (D10, D11).
///
/// # Errors
/// `E-PARAM-UNDECLARED`, `E-DERIV-I64`, `E-DERIV-BOOLEAN`, `E-DERIV-INDEX`
/// or `E-INTERFACE` (loss graph: not `ngraph-f32/0`, a `param` slot, fewer
/// input slots than model outputs, or not exactly one output).
pub fn admit(
    model: &CheckedGraph,
    loss: &CheckedGraph,
    params: &[u32],
    deriv: &DerivOptions,
) -> Result<(), Diagnostic> {
    if model.param_slots() != params {
        let k = params.first().copied().unwrap_or(0);
        return Err(Diagnostic::new(
            Code::ParamUndeclared,
            Site::Slot(k),
            "params must be every param slot in order",
        ));
    }
    grad::admit(model, params, deriv)?;
    let lg = loss.graph();
    let interface = |why: &str| Err(Diagnostic::new(Code::Interface, Site::Graph, why));
    if lg.profile != SubProfile::F32 {
        return interface("the loss graph must be ngraph-f32/0");
    }
    if !loss.param_slots().is_empty() {
        return interface("the loss graph must have no param slot");
    }
    if lg.slots.len() < model.graph().outputs.len() {
        return interface("the loss graph takes fewer inputs than the model has outputs");
    }
    if lg.outputs.len() != 1 {
        return interface("the loss graph must have exactly one output");
    }
    Ok(())
}

/// Number of target values each training point must carry for `loss`.
#[must_use]
pub fn target_count(model: &CheckedGraph, loss: &CheckedGraph) -> usize {
    loss.graph()
        .slots
        .len()
        .saturating_sub(model.graph().outputs.len())
}

fn check_data(c: &Contract) -> Result<(), Diagnostic> {
    let inputs = c.model.graph().input_slots().len();
    let targets = target_count(&c.model, &c.loss);
    for (i, p) in c.data.iter().enumerate() {
        if p.inputs.len() != inputs || p.targets.len() != targets {
            return Err(Diagnostic::new(
                Code::Interface,
                Site::Graph,
                format!(
                    "data point {i} has {} inputs and {} targets; the model takes {inputs} and the loss {targets}",
                    p.inputs.len(),
                    p.targets.len()
                ),
            ));
        }
    }
    Ok(())
}

fn values_f32(slots: &[Value]) -> Vec<f32> {
    slots
        .iter()
        .map(|v| match v {
            Value::F32(b) => f32::from_bits(*b),
            Value::I64(_) | Value::Bool(_) => f32::NAN,
        })
        .collect()
}

/// Mean loss and binary64 mean gradient over the data, point order fixed.
/// Per-point gradients come from the FP32 reference backward (G2). Each
/// point is first admitted with the reference failure semantics (D10): the
/// model's input facts (including param facts on `params`), then the first
/// failing guard or refusing index read of the model and of the loss graph,
/// in canonical order.
fn loss_and_grad(
    c: &Contract,
    params: &ParamSnapshot,
    evaluated_nodes: &mut Option<&mut u64>,
) -> Result<(f64, Vec<f64>, bool), Diagnostic> {
    let mut grad_sum = vec![0f64; c.params.len()];
    let mut loss_sum = 0f64;
    let mut finite = true;
    for p in &c.data {
        let model_slots = eval::preflight(&c.model, &eval::f32_inputs(&p.inputs), Some(params))?;
        let (l, gs) = grad::objective_observed(
            &c.model,
            &c.loss,
            &values_f32(&model_slots),
            &p.targets,
            &c.deriv,
            evaluated_nodes,
        )?;
        loss_sum += f64::from(l);
        for (acc, k) in grad_sum.iter_mut().zip(&c.params) {
            let gk = gs[*k as usize];
            finite &= gk.is_finite();
            *acc += f64::from(gk);
        }
    }
    let n = c.data.len().max(1) as f64;
    let inv = 1.0 / n;
    Ok((
        loss_sum * inv,
        grad_sum.iter().map(|g| g * inv).collect(),
        finite && loss_sum.is_finite(),
    ))
}

/// One `adam/0` step into fresh vectors (the published state is untouched).
fn adam_step(a: &Adam, s: &AdamState, p: &[f32], g: &[f64]) -> (Vec<f32>, AdamState) {
    let mut n = s.clone();
    n.t += 1;
    n.b1t *= a.beta1;
    n.b2t *= a.beta2;
    let mut out = Vec::with_capacity(p.len());
    for k in 0..p.len() {
        n.m[k] = a.beta1 * s.m[k] + (1.0 - a.beta1) * g[k];
        n.v[k] = a.beta2 * s.v[k] + (1.0 - a.beta2) * (g[k] * g[k]);
        let mhat = n.m[k] / (1.0 - n.b1t);
        let vhat = n.v[k] / (1.0 - n.b2t);
        let upd = a.lr * mhat / (sem::soft::sqrt_f64(vhat) + a.eps);
        out.push((f64::from(p[k]) - upd) as f32);
    }
    (out, n)
}

/// Host controls for a training call.
#[derive(Default)]
pub struct TrainControl<'a> {
    /// Cancel before step `t` (1-based) when this returns `true`.
    pub cancel_before_step: Option<&'a dyn Fn(u64) -> bool>,
    /// Scratch instrumentation (one lease per step), lent for the call.
    pub scratch: Option<&'a mut ScratchCounters>,
    /// Actual forward value operations and guard/index validation events.
    /// This is not the evaluator's historical fuel/Report node accounting;
    /// backward and optimizer work are not graph evaluation events.
    pub evaluated_nodes: Option<&'a mut u64>,
    /// Budgets (D5): `optimizer_steps`, `backward_scratch` and
    /// `snapshot_bytes` are charged; a raised budget is `E-PROFILE`.
    pub budgets: Budgets,
}

/// Runs `steps` `adam/0` steps from `init` (or from the seed). Each step
/// publishes a new immutable snapshot; a failing step publishes nothing.
///
/// # Errors
/// [`Stopped`] with the code and the last published snapshot.
pub fn train(
    ids: &dyn IdentityScheme,
    c: &Contract,
    init: Option<&ParamSnapshot>,
    steps: u64,
    control: &mut TrainControl<'_>,
) -> Result<Trained, Box<Stopped>> {
    let stop = |d: Diagnostic, last: Option<ParamSnapshot>, k: u64| {
        Box::new(Stopped {
            diagnostic: d,
            last,
            published_steps: k,
        })
    };
    let budgets = &control.budgets;
    budgets.validate_policy().map_err(|d| stop(d, None, 0))?;
    c.adam.admit().map_err(|d| stop(d, None, 0))?;
    admit(&c.model, &c.loss, &c.params, &c.deriv).map_err(|d| stop(d, None, 0))?;
    check_data(c).map_err(|d| stop(d, None, 0))?;
    if steps > budgets.optimizer_steps {
        return Err(stop(
            Diagnostic::budget(BudgetKind::Steps, budgets.optimizer_steps, steps),
            None,
            0,
        ));
    }
    let step_bytes = (c.model.node_count() + c.loss.node_count()) as u64 * 16;
    if step_bytes > budgets.backward_scratch {
        return Err(stop(
            Diagnostic::budget(
                BudgetKind::BackwardScratch,
                budgets.backward_scratch,
                step_bytes,
            ),
            None,
            0,
        ));
    }
    let snapshot =
        |values: &[f32]| ParamSnapshot::with_budget(ids, &c.model, values, budgets.snapshot_bytes);
    let s0 = match init {
        Some(s) => {
            s.check_use(&c.model).map_err(|d| stop(d, None, 0))?;
            s.clone()
        }
        None => snapshot(&init_params(c.seed, c.params.len())).map_err(|d| stop(d, None, 0))?,
    };
    let mut current = s0.clone();
    let mut state = Rc::new(AdamState::new(c.params.len()));
    let mut losses = Vec::with_capacity(usize::try_from(steps).unwrap_or(0).saturating_add(1));
    for t in 1..=steps {
        if control.cancel_before_step.is_some_and(|f| f(t)) {
            return Err(stop(
                Diagnostic::new(
                    Code::Cancelled,
                    Site::Graph,
                    format!("cancelled before step {t}"),
                ),
                Some(current),
                t - 1,
            ));
        }
        let lease = ScratchLease::acquire(control.scratch.as_deref_mut(), step_bytes);
        let (l, g, finite) = match loss_and_grad(c, &current, &mut control.evaluated_nodes) {
            Ok(r) => r,
            Err(d) => {
                drop(lease);
                return Err(stop(d, Some(current), t - 1));
            }
        };
        losses.push(l);
        if !finite || g.iter().any(|x| !x.is_finite()) {
            drop(lease);
            return Err(stop(
                Diagnostic::new(
                    Code::NonfiniteUpdate,
                    Site::Graph,
                    format!("non-finite gradient at step {t}"),
                ),
                Some(current),
                t - 1,
            ));
        }
        let (np, ns) = adam_step(&c.adam, &state, &current.values(), &g);
        let next = match snapshot(&np) {
            Ok(s) => s,
            Err(d) => {
                drop(lease);
                return Err(stop(d, Some(current), t - 1));
            }
        };
        drop(lease);
        current = next;
        state = Rc::new(ns);
    }
    let (final_loss, _, _) =
        loss_and_grad(c, &current, &mut control.evaluated_nodes).map_err(|d| stop(d, Some(current.clone()), steps))?;
    losses.push(final_loss);
    let g = c.model.graph();
    let provenance = Provenance {
        parent: s0.id(),
        subject: g.subject.clone(),
        graph: c.model.id(),
        profile: g.profile,
        facts: c.model.fact_bindings(ids),
        loss: c.loss.id(),
        data: data_digest(ids, &c.data),
        deriv: c.deriv.clone(),
        seed: c.seed,
        optimizer: c.adam,
        steps,
        tools: tools(),
        result: current.id(),
        optimizer_state: state.digest(ids),
    };
    Ok(Trained {
        initial: s0,
        snapshot: current,
        state,
        losses,
        provenance,
    })
}

/// Replays a provenance record against `c` (which must bind the same
/// identities) from `parent` (or, when `None`, from the seed) and returns the
/// reproduced training result.
///
/// # Errors
/// [`Stopped`] with `E-SNAPSHOT-STALE` if the model, its fact bindings, the
/// loss graph, the data, the seed, the optimizer bits, the derivative
/// options or the parent differ from the record; `E-SNAPSHOT-REPLAY` if the
/// reproduced snapshot or optimizer state differs; or the code that stopped
/// training.
pub fn replay(
    ids: &dyn IdentityScheme,
    c: &Contract,
    p: &Provenance,
    parent: Option<&ParamSnapshot>,
    control: &mut TrainControl<'_>,
) -> Result<Trained, Box<Stopped>> {
    let refuse = |code: Code, what: &str| {
        Box::new(Stopped {
            diagnostic: Diagnostic::new(code, Site::Graph, format!("provenance {what} differs")),
            last: None,
            published_steps: 0,
        })
    };
    let stale = |what: &str| refuse(Code::SnapshotStale, what);
    if c.model.id() != p.graph {
        return Err(stale("graph"));
    }
    if c.model.fact_bindings(ids) != p.facts {
        return Err(stale("fact bindings"));
    }
    if c.loss.id() != p.loss {
        return Err(stale("loss graph"));
    }
    if data_digest(ids, &c.data) != p.data {
        return Err(stale("data"));
    }
    if c.seed != p.seed || !c.adam.same_bits(&p.optimizer) || c.deriv != p.deriv {
        return Err(stale("seed/optimizer/policy"));
    }
    let parent_id = match parent {
        Some(s) => s.id(),
        None => ParamSnapshot::with_budget(
            ids,
            &c.model,
            &init_params(c.seed, c.params.len()),
            control.budgets.snapshot_bytes,
        )
        .map_err(|d| {
            Box::new(Stopped {
                diagnostic: d,
                last: None,
                published_steps: 0,
            })
        })?
        .id(),
    };
    if parent_id != p.parent {
        return Err(stale("parent"));
    }
    let t = train(ids, c, parent, p.steps, control)?;
    if t.snapshot.id() != p.result || t.provenance.optimizer_state != p.optimizer_state {
        return Err(refuse(Code::SnapshotReplay, "replayed snapshot"));
    }
    Ok(t)
}

/// The derivative policy revision named by every training contract.
#[must_use]
pub const fn policy() -> &'static str {
    DERIV_POLICY
}
