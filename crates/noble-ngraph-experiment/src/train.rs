//! D11 training contract, `adam/0`, immutable snapshot publication and
//! replayable provenance. The training loop, PRNG and optimizer arithmetic
//! are host-side (D4); the graph is never modified.

use std::sync::Arc;

use crate::check::CheckedGraph;
use crate::diag::{BudgetKind, Code, Diagnostic, Site};
use crate::eval::{ScratchCounters, ScratchLease};
use crate::grad::{self, DerivOptions};
use crate::ir::{hex, Canon, Digest, FACT_RULE_REVISION, IR_VERSION, SEMANTIC_REVISION};
use crate::prng::{SplitMix64, PRNG};
use crate::snapshot::ParamSnapshot;

/// Optimizer revision.
pub const ADAM: &str = "adam/0";

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
    pub fn digest(&self) -> Digest {
        let mut c = Canon::new("noble.ngraph.adam-state.v0");
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
pub fn data_digest(points: &[Point]) -> Digest {
    let mut c = Canon::new("noble.ngraph.training-data.v0");
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

/// Training provenance (D11): everything needed to replay a snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    /// Parent snapshot id (hex).
    pub parent: String,
    /// Subject identity (hex) and closure.
    pub subject: String,
    /// Closure identities.
    pub closure: Vec<u64>,
    /// Model `GraphLogicalId` (hex).
    pub graph: String,
    /// Profile and semantic revisions.
    pub profile: String,
    /// Derivative policy and options.
    pub deriv: String,
    /// Fact bindings digest (hex): graph id ‖ fact-rule rev ‖ fact records.
    pub facts: String,
    /// Loss graph id (hex).
    pub loss: String,
    /// Data digest (hex).
    pub data: String,
    /// Seed and PRNG.
    pub seed: u64,
    /// PRNG revision.
    pub prng: String,
    /// Optimizer and bits.
    pub optimizer: String,
    /// Steps.
    pub steps: u64,
    /// Tool revisions.
    pub tools: String,
    /// Resulting snapshot id (hex).
    pub result: String,
    /// Resulting optimizer-state digest (hex).
    pub optimizer_state: String,
}

/// The result of a training call: the last published snapshot pair and its
/// provenance, plus the per-step losses.
#[derive(Clone, Debug)]
pub struct Trained {
    /// Initial snapshot `S₀`.
    pub initial: ParamSnapshot,
    /// Last published snapshot.
    pub snapshot: ParamSnapshot,
    /// Last published optimizer state.
    pub state: Arc<AdamState>,
    /// Loss before each step, then the final loss (binary64 mean).
    pub losses: Vec<f64>,
    /// Provenance of `snapshot`.
    pub provenance: Provenance,
}

/// Why a training call stopped without completing all steps.
#[derive(Clone, Debug)]
pub struct Stopped {
    /// The refusal (`E-NONFINITE-UPDATE`, `C-CANCELLED`, admission codes).
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
    #[allow(clippy::cast_possible_truncation)]
    (0..n).map(|_| (r.next_f64() - 0.5) as f32).collect()
}

fn facts_digest(model: &CheckedGraph) -> Digest {
    let mut c = Canon::new("noble.ngraph.fact-bindings.v0");
    c.bytes(&model.id());
    c.str(FACT_RULE_REVISION);
    let g = model.graph();
    c.u64(g.facts.len() as u64);
    for f in &g.facts {
        c.u32(f.node.0);
        c.fact(&f.fact);
        c.str(match &f.provenance {
            crate::ir::Provenance::Derived(_) => "derived",
            crate::ir::Provenance::InputValidated => "input-validated",
            crate::ir::Provenance::Evidence(_) => "evidence",
            crate::ir::Provenance::Guard(_) => "guard",
            crate::ir::Provenance::Asserted => "asserted",
        });
    }
    c.finish()
}

/// Tool revisions recorded in provenance.
#[must_use]
pub fn tools() -> String {
    format!(
        "noble-ngraph-experiment {}; {IR_VERSION}; {SEMANTIC_REVISION}; {FACT_RULE_REVISION}; {}",
        env!("CARGO_PKG_VERSION"),
        crate::compiled::COMPILER_REVISION
    )
}

/// Mean loss and binary64 mean gradient over the data, point order fixed.
/// Per-point gradients come from the FP32 reference backward (G2).
fn loss_and_grad(c: &Contract, params: &[f32]) -> (f64, Vec<f64>, bool) {
    let g = c.model.graph();
    let mut slots = vec![0f32; g.slots.len()];
    let mut grad_sum = vec![0f64; c.params.len()];
    let mut loss_sum = 0f64;
    let mut finite = true;
    for p in &c.data {
        let mut it = p.inputs.iter();
        let mut pk = params.iter();
        for (k, s) in g.slots.iter().enumerate() {
            slots[k] = match s.kind {
                crate::ir::SlotKind::Input => *it.next().unwrap_or(&0.0),
                crate::ir::SlotKind::Param => *pk.next().unwrap_or(&0.0),
            };
        }
        let (l, gs) = grad::objective::<f32>(&c.model, &c.loss, &slots, &p.targets, &c.deriv);
        loss_sum += f64::from(l);
        for (acc, k) in grad_sum.iter_mut().zip(&c.params) {
            let gk = gs[*k as usize];
            finite &= gk.is_finite();
            *acc += f64::from(gk);
        }
    }
    #[allow(clippy::cast_precision_loss)]
    let n = c.data.len().max(1) as f64;
    let inv = 1.0 / n;
    (
        loss_sum * inv,
        grad_sum.iter().map(|g| g * inv).collect(),
        finite && loss_sum.is_finite(),
    )
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
        let upd = a.lr * mhat / (vhat.sqrt() + a.eps);
        #[allow(clippy::cast_possible_truncation)]
        out.push((f64::from(p[k]) - upd) as f32);
    }
    (out, n)
}

/// Host controls for a training call.
#[derive(Default)]
pub struct TrainControl<'a> {
    /// Cancel before step `t` (1-based) when this returns `true`.
    pub cancel_before_step: Option<&'a dyn Fn(u64) -> bool>,
    /// Scratch instrumentation (one lease per step).
    pub scratch: Option<&'a ScratchCounters>,
}

/// Runs `steps` `adam/0` steps from `init` (or from the seed). Each step
/// publishes a new immutable snapshot; a failing step publishes nothing.
///
/// # Errors
/// [`Stopped`] with the code and the last published snapshot.
pub fn train(
    c: &Contract,
    init: Option<&ParamSnapshot>,
    steps: u64,
    control: &TrainControl<'_>,
) -> Result<Trained, Box<Stopped>> {
    let stop = |d: Diagnostic, last: Option<ParamSnapshot>, k: u64| {
        Box::new(Stopped {
            diagnostic: d,
            last,
            published_steps: k,
        })
    };
    if let Err(d) = c
        .model
        .graph()
        .param_slots()
        .iter()
        .zip(&c.params)
        .try_for_each(|(a, b)| {
            if a == b {
                Ok(())
            } else {
                Err(Diagnostic::new(
                    Code::ParamUndeclared,
                    Site::Slot(*b),
                    "params must be every param slot in order",
                ))
            }
        })
    {
        return Err(stop(d, None, 0));
    }
    grad::admit(&c.model, &c.params, &c.deriv).map_err(|d| stop(d, None, 0))?;
    let limit = crate::check::Budgets::default().optimizer_steps;
    if steps > limit {
        return Err(stop(
            Diagnostic::budget(BudgetKind::Steps, limit, steps),
            None,
            0,
        ));
    }
    let s0 = match init {
        Some(s) => {
            s.check_use(&c.model).map_err(|d| stop(d, None, 0))?;
            s.clone()
        }
        None => ParamSnapshot::new(&c.model, &init_params(c.seed, c.params.len()))
            .map_err(|d| stop(d, None, 0))?,
    };
    let mut current = s0.clone();
    let mut state = Arc::new(AdamState::new(c.params.len()));
    let mut losses = Vec::with_capacity(steps as usize + 1);
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
        let lease = ScratchLease::acquire(
            control.scratch,
            (c.model.node_count() + c.loss.node_count()) as u64 * 16,
        );
        let p = current.values();
        let (l, g, finite) = loss_and_grad(c, &p);
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
        let (np, ns) = adam_step(&c.adam, &state, &p, &g);
        let next = match ParamSnapshot::new(&c.model, &np) {
            Ok(s) => s,
            Err(d) => {
                drop(lease);
                return Err(stop(d, Some(current), t - 1));
            }
        };
        drop(lease);
        current = next;
        state = Arc::new(ns);
    }
    let (final_loss, _, _) = loss_and_grad(c, &current.values());
    losses.push(final_loss);
    let g = c.model.graph();
    let provenance = Provenance {
        parent: hex(&s0.id()),
        subject: hex(&g.subject.identity),
        closure: g.subject.closure.clone(),
        graph: hex(&c.model.id()),
        profile: format!("{} {SEMANTIC_REVISION}", g.profile.name()),
        deriv: c.deriv.describe(),
        facts: hex(&facts_digest(&c.model)),
        loss: hex(&c.loss.id()),
        data: hex(&data_digest(&c.data)),
        seed: c.seed,
        prng: PRNG.to_owned(),
        optimizer: c.adam.describe(),
        steps,
        tools: tools(),
        result: hex(&current.id()),
        optimizer_state: hex(&state.digest()),
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
/// identities) and returns the reproduced snapshot.
///
/// # Errors
/// [`Stopped`] if the contract does not match the provenance or training stops.
pub fn replay(c: &Contract, p: &Provenance) -> Result<ParamSnapshot, Box<Stopped>> {
    let mismatch = |what: &str| {
        Box::new(Stopped {
            diagnostic: Diagnostic::new(
                Code::SnapshotStale,
                Site::Graph,
                format!("provenance {what} differs"),
            ),
            last: None,
            published_steps: 0,
        })
    };
    if hex(&c.model.id()) != p.graph {
        return Err(mismatch("graph"));
    }
    if hex(&c.loss.id()) != p.loss {
        return Err(mismatch("loss graph"));
    }
    if hex(&data_digest(&c.data)) != p.data {
        return Err(mismatch("data"));
    }
    if c.seed != p.seed || c.adam.describe() != p.optimizer || c.deriv.describe() != p.deriv {
        return Err(mismatch("seed/optimizer/policy"));
    }
    let t = train(c, None, p.steps, &TrainControl::default())?;
    if hex(&t.initial.id()) != p.parent {
        return Err(mismatch("parent"));
    }
    Ok(t.snapshot)
}
