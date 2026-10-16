//! D10 derivative policy `ngraph-deriv-real/0`: the derivative of the
//! intended real-valued arithmetic, ignoring binary32 rounding. Not the
//! derivative of the floating-point function.
//!
//! * Static training admission: `E-DERIV-I64`, `E-PARAM-UNDECLARED`,
//!   `E-DERIV-BOOLEAN` (a param reaches the loss through `step` or a `select`
//!   gate under the default policy, or a boolean opt-in names no `step` or
//!   `select` it applies to), `E-DERIV-INDEX` (a param reaches an index
//!   operand under the default policy, or an index opt-in names no `index`
//!   node).
//! * Reverse mode visits nodes in reverse canonical order and accumulates each
//!   node's contributions in stored operand/term order, so FP32 gradients are
//!   bitwise reproducible. [`Real`] instantiates it in binary32 (the reference
//!   backward, G2) and binary64 (G1).
//! * Surrogates (`straight-through`, `clamped-linear`, index
//!   `backward-difference`, `select` gate straight-through) are opt-ins and are
//!   labelled as surrogates; they are never described as exact derivatives.

use alloc::collections::BTreeMap;
use alloc::{format, string::String, vec, vec::Vec};

use crate::check::CheckedGraph;
use crate::diag::{Code, Diagnostic, Site};
use crate::ir::{AffineOperand, Graph, NodeId, Op, SlotKind, SubProfile};
use crate::sem;

/// Derivative policy revision.
pub const DERIV_POLICY: &str = "ngraph-deriv-real/0";

/// Policy for hard-boolean paths (`step`, `select` gate).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BooleanPolicy {
    /// Default: static refusal `E-DERIV-BOOLEAN`.
    Refuse,
    /// Surrogate: identity through `step`; `(on − off)` through a `select` gate.
    StraightThrough,
    /// Surrogate: slope `1/w` on `|s − 0.5| < w/2`, otherwise 0 (`step` only).
    ClampedLinear(f32),
}

/// Policy for the index operand of `index` (candidate gradients are always
/// the exact derivative for a fixed index and are allowed by default).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexPolicy {
    /// Default: static refusal `E-DERIV-INDEX`.
    Refuse,
    /// Explicitly no index gradient.
    Zero,
    /// Surrogate: `read(i) − read(i−1)`.
    BackwardDifference,
}

/// Per-node derivative options (defaults: refuse).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DerivOptions {
    /// Per-node boolean policy overrides.
    pub boolean: BTreeMap<NodeId, BooleanPolicy>,
    /// Per-node index policy overrides.
    pub index: BTreeMap<NodeId, IndexPolicy>,
}

impl DerivOptions {
    fn boolean(&self, n: NodeId) -> BooleanPolicy {
        self.boolean
            .get(&n)
            .copied()
            .unwrap_or(BooleanPolicy::Refuse)
    }
    fn index(&self, n: NodeId) -> IndexPolicy {
        self.index.get(&n).copied().unwrap_or(IndexPolicy::Refuse)
    }

    /// Canonical text of every opt-in (recorded in provenance).
    #[must_use]
    pub fn describe(&self) -> String {
        let mut s = String::from(DERIV_POLICY);
        for (n, p) in &self.boolean {
            s.push_str(&match p {
                BooleanPolicy::Refuse => format!(";%{}:boolean=refuse", n.0),
                BooleanPolicy::StraightThrough => {
                    format!(";%{}:boolean=straight-through(surrogate)", n.0)
                }
                BooleanPolicy::ClampedLinear(w) => {
                    format!(
                        ";%{}:boolean=clamped-linear(w=0x{:08x},surrogate)",
                        n.0,
                        w.to_bits()
                    )
                }
            });
        }
        for (n, p) in &self.index {
            s.push_str(&match p {
                IndexPolicy::Refuse => format!(";%{}:index=refuse", n.0),
                IndexPolicy::Zero => format!(";%{}:index=zero", n.0),
                IndexPolicy::BackwardDifference => {
                    format!(";%{}:index=backward-difference(surrogate)", n.0)
                }
            });
        }
        s
    }

    /// Whether any surrogate is in use.
    #[must_use]
    pub fn uses_surrogate(&self) -> bool {
        self.boolean.values().any(|p| *p != BooleanPolicy::Refuse)
            || self
                .index
                .values()
                .any(|p| *p == IndexPolicy::BackwardDifference)
    }
}

/// Static training admission (D10/D11) for `model` with trainable `params`
/// (slot indices). Every opt-in must name a node of `model` that its policy
/// applies to: `straight-through` and `refuse` a `step` or `select`,
/// `clamped-linear` a `step`, and every index policy an `index` (design D6
/// and D10 of `numeric-graph-v1`; replay re-admits recorded opt-ins here).
///
/// # Errors
/// `E-DERIV-I64`, `E-PARAM-UNDECLARED`, `E-DERIV-BOOLEAN` or `E-DERIV-INDEX`.
pub fn admit(model: &CheckedGraph, params: &[u32], opts: &DerivOptions) -> Result<(), Diagnostic> {
    let g = model.graph();
    if g.profile == SubProfile::I64 {
        return Err(Diagnostic::new(
            Code::DerivI64,
            Site::Graph,
            "training contracts are refused on ngraph-i64/0",
        ));
    }
    for &k in params {
        if g.slots
            .get(k as usize)
            .is_none_or(|s| s.kind != SlotKind::Param)
        {
            return Err(Diagnostic::new(
                Code::ParamUndeclared,
                Site::Slot(k),
                "gradient requested for a non-param slot",
            ));
        }
    }
    admit_options(g, opts)?;
    let n = g.nodes.len();
    let mut dep = vec![false; n];
    for (i, node) in g.nodes.iter().enumerate() {
        let mut depends = false;
        if let Op::Param(k) = node.op {
            depends = params.contains(&k);
        } else {
            node.op.visit_operands(|o| depends |= dep[o.index()]);
        }
        dep[i] = depends;
    }
    let mut reaches = vec![false; n];
    for o in &g.outputs {
        reaches[o.index()] = true;
    }
    for i in (0..n).rev() {
        if reaches[i] && !g.nodes[i].op.is_guard() {
            g.nodes[i].op.visit_operands(|o| reaches[o.index()] = true);
        }
    }
    for (i, node) in g.nodes.iter().enumerate() {
        if !reaches[i] {
            continue;
        }
        let id = NodeId(i as u32);
        match &node.op {
            Op::F32Step(s) if dep[s.index()] && opts.boolean(id) == BooleanPolicy::Refuse => {
                return Err(Diagnostic::new(
                    Code::DerivBoolean,
                    Site::Node(id),
                    "a param reaches the loss through step",
                ));
            }
            Op::F32Select { bit, .. }
                if dep[bit.index()]
                    && !matches!(opts.boolean(id), BooleanPolicy::StraightThrough) =>
            {
                return Err(Diagnostic::new(
                    Code::DerivBoolean,
                    Site::Node(id),
                    "a param reaches the loss through a select gate",
                ));
            }
            Op::F32Index { index, .. }
                if dep[index.index()] && opts.index(id) == IndexPolicy::Refuse =>
            {
                return Err(Diagnostic::new(
                    Code::DerivIndex,
                    Site::Node(id),
                    "a param reaches an index operand",
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Every opt-in names a node of `g` that its policy applies to.
fn admit_options(g: &Graph, opts: &DerivOptions) -> Result<(), Diagnostic> {
    let op = |n: NodeId| g.nodes.get(n.index()).map(|node| &node.op);
    let refuse = |code: Code, n: NodeId, what: &str| {
        let why = match op(n) {
            Some(o) => format!("the {what} opt-in does not apply to {}", o.name()),
            None => format!("the {what} opt-in names no node of the model"),
        };
        Err(Diagnostic::new(code, Site::Node(n), why))
    };
    for (&n, policy) in &opts.boolean {
        let applies = matches!(
            (policy, op(n)),
            (BooleanPolicy::ClampedLinear(_), Some(Op::F32Step(_)))
                | (
                    BooleanPolicy::Refuse | BooleanPolicy::StraightThrough,
                    Some(Op::F32Step(_) | Op::F32Select { .. }),
                )
        );
        if !applies {
            return refuse(Code::DerivBoolean, n, "boolean");
        }
    }
    for &n in opts.index.keys() {
        if !matches!(op(n), Some(Op::F32Index { .. })) {
            return refuse(Code::DerivIndex, n, "index");
        }
    }
    Ok(())
}

/// Scalar arithmetic used by reverse mode (binary32 or binary64, RNE).
pub trait Real: Copy + PartialOrd + core::fmt::Debug {
    /// `+0`.
    const ZERO: Self;
    /// From binary32 (exact).
    fn of_f32(x: f32) -> Self;
    /// To binary64 (exact).
    fn to_f64(self) -> f64;
    /// `rn(a+b)`.
    fn add(self, o: Self) -> Self;
    /// `rn(a−b)`.
    fn sub(self, o: Self) -> Self;
    /// `rn(a×b)`.
    fn mul(self, o: Self) -> Self;
    /// `rn(a÷b)`.
    fn div(self, o: Self) -> Self;
    /// Whether the value is exactly `1`.
    fn is_one(self) -> bool;
}

impl Real for f32 {
    const ZERO: Self = 0.0;
    fn of_f32(x: f32) -> Self {
        x
    }
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn add(self, o: Self) -> Self {
        self + o
    }
    fn sub(self, o: Self) -> Self {
        self - o
    }
    fn mul(self, o: Self) -> Self {
        self * o
    }
    fn div(self, o: Self) -> Self {
        self / o
    }
    fn is_one(self) -> bool {
        self.to_bits() == sem::ONE
    }
}

impl Real for f64 {
    const ZERO: Self = 0.0;
    fn of_f32(x: f32) -> Self {
        Self::from(x)
    }
    fn to_f64(self) -> f64 {
        self
    }
    fn add(self, o: Self) -> Self {
        self + o
    }
    fn sub(self, o: Self) -> Self {
        self - o
    }
    fn mul(self, o: Self) -> Self {
        self * o
    }
    fn div(self, o: Self) -> Self {
        self / o
    }
    fn is_one(self) -> bool {
        self == 1.0
    }
}

fn relu<R: Real>(x: R) -> R {
    if x > R::ZERO || x.to_f64().is_nan() {
        x
    } else {
        R::ZERO
    }
}

fn position<R: Real>(
    i: R,
    len: usize,
    below: crate::ir::OutOfRange,
    above: crate::ir::OutOfRange,
) -> Option<Option<usize>> {
    let f = i.to_f64() as f32;
    sem::index_position(f, len, below, above).ok()
}

/// `R`-valued forward pass (guards are not evaluated; callers sample points
/// where guards and index reads succeed). In binary32 this equals the L3
/// reference bitwise on such points.
#[must_use]
pub fn forward<R: Real>(g: &CheckedGraph, slots: &[R]) -> Vec<R> {
    forward_observed(g, slots, &mut None)
}

fn forward_observed<R: Real>(g: &CheckedGraph, slots: &[R], evaluated_nodes: &mut Option<&mut u64>) -> Vec<R> {
    let graph = g.graph();
    let mut v: Vec<R> = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        // Guards are actually tested by first_failure_observed, not by this
        // forward pass's zero placeholder. Count only real value operations.
        if !node.op.is_guard() {
            if let Some(counter) = evaluated_nodes.as_deref_mut() {
                *counter += 1;
            }
        }
        let x = |n: &NodeId| v[n.index()];
        let r = match &node.op {
            Op::Input(k) | Op::Param(k) => slots[*k as usize],
            Op::Const(crate::ir::Literal::F32(b)) => R::of_f32(f32::from_bits(*b)),
            Op::F32Add(a, b) => x(a).add(x(b)),
            Op::F32Sub(a, b) => x(a).sub(x(b)),
            Op::F32Mul(a, b) => x(a).mul(x(b)),
            Op::F32Affine { bias, terms } => {
                let mut acc = R::of_f32(f32::from_bits(*bias));
                for t in terms {
                    let o = match t.operand {
                        AffineOperand::One => R::of_f32(1.0),
                        AffineOperand::Node(n) => x(&n),
                    };
                    acc = acc.add(R::of_f32(f32::from_bits(t.coefficient)).mul(o));
                }
                acc
            }
            Op::F32Relu(a) => relu(x(a)),
            Op::F32Reglu { left, gate } => x(left).mul(relu(x(gate))),
            Op::F32Step(s) => {
                if x(s).to_f64() >= 0.5 {
                    R::of_f32(1.0)
                } else {
                    R::ZERO
                }
            }
            Op::F32Select { bit, off, on } => {
                if x(bit).is_one() {
                    x(on)
                } else {
                    x(off)
                }
            }
            Op::F32Index {
                index,
                table,
                below,
                above,
            } => match position(x(index), table.len(), *below, *above) {
                Some(Some(k)) => x(&table[k]),
                _ => R::ZERO,
            },
            _ => R::ZERO,
        };
        v.push(r);
    }
    v
}

/// Reverse mode: given forward values and output seeds `(output position,
/// gradient)`, returns the gradient of every node.
#[must_use]
pub fn backward<R: Real>(g: &CheckedGraph, vals: &[R], seeds: &[R], opts: &DerivOptions) -> Vec<R> {
    let graph = g.graph();
    let mut gr = vec![R::ZERO; graph.nodes.len()];
    for (o, s) in graph.outputs.iter().zip(seeds) {
        gr[o.index()] = gr[o.index()].add(*s);
    }
    for (i, node) in graph.nodes.iter().enumerate().rev() {
        let id = NodeId(i as u32);
        let gn = gr[i];
        let acc = |n: &NodeId, d: R, gr: &mut Vec<R>| {
            gr[n.index()] = gr[n.index()].add(d);
        };
        match &node.op {
            Op::F32Add(a, b) => {
                acc(a, gn, &mut gr);
                acc(b, gn, &mut gr);
            }
            Op::F32Sub(a, b) => {
                acc(a, gn, &mut gr);
                acc(b, R::ZERO.sub(gn), &mut gr);
            }
            Op::F32Mul(a, b) => {
                let (va, vb) = (vals[a.index()], vals[b.index()]);
                acc(a, gn.mul(vb), &mut gr);
                acc(b, gn.mul(va), &mut gr);
            }
            Op::F32Affine { terms, .. } => {
                for t in terms {
                    if let AffineOperand::Node(n) = t.operand {
                        acc(
                            &n,
                            gn.mul(R::of_f32(f32::from_bits(t.coefficient))),
                            &mut gr,
                        );
                    }
                }
            }
            Op::F32Relu(a) => {
                let d = if vals[a.index()] > R::ZERO {
                    gn
                } else {
                    R::ZERO
                };
                acc(a, d, &mut gr);
            }
            Op::F32Reglu { left, gate } => {
                let (vl, vg) = (vals[left.index()], vals[gate.index()]);
                acc(left, gn.mul(relu(vg)), &mut gr);
                let d = if vg > R::ZERO { gn.mul(vl) } else { R::ZERO };
                acc(gate, d, &mut gr);
            }
            Op::F32Step(s) => {
                let d = match opts.boolean(id) {
                    BooleanPolicy::Refuse => R::ZERO,
                    BooleanPolicy::StraightThrough => gn,
                    BooleanPolicy::ClampedLinear(w) => {
                        let dist = (vals[s.index()].to_f64() - 0.5).abs();
                        if dist < f64::from(w) / 2.0 {
                            gn.div(R::of_f32(w))
                        } else {
                            R::ZERO
                        }
                    }
                };
                acc(s, d, &mut gr);
            }
            Op::F32Select { bit, off, on } => {
                let one = vals[bit.index()].is_one();
                let gate = match opts.boolean(id) {
                    BooleanPolicy::StraightThrough => {
                        gn.mul(vals[on.index()].sub(vals[off.index()]))
                    }
                    _ => R::ZERO,
                };
                acc(bit, gate, &mut gr);
                if one {
                    acc(on, gn, &mut gr);
                } else {
                    acc(off, gn, &mut gr);
                }
            }
            Op::F32Index {
                index,
                table,
                below,
                above,
            } => {
                let i = vals[index.index()];
                let d = match opts.index(id) {
                    IndexPolicy::BackwardDifference => {
                        let read = |p: Option<Option<usize>>| match p {
                            Some(Some(k)) => vals[table[k].index()],
                            _ => R::ZERO,
                        };
                        let here = read(position(i, table.len(), *below, *above));
                        let prev =
                            read(position(i.sub(R::of_f32(1.0)), table.len(), *below, *above));
                        gn.mul(here.sub(prev))
                    }
                    _ => R::ZERO,
                };
                acc(index, d, &mut gr);
                if let Some(Some(k)) = position(i, table.len(), *below, *above) {
                    acc(&table[k], gn, &mut gr);
                }
            }
            _ => {}
        }
    }
    gr
}

/// Gradient of every param slot (in slot order of `params`) from node gradients.
#[must_use]
pub fn param_grads<R: Real>(g: &CheckedGraph, node_grads: &[R], params: &[u32]) -> Vec<R> {
    params
        .iter()
        .map(|k| node_grads[g.slot_node(*k).index()])
        .collect()
}

/// Gradient of every slot (inputs and params) in slot order.
#[must_use]
pub fn slot_grads<R: Real>(g: &CheckedGraph, node_grads: &[R]) -> Vec<R> {
    (0..g.graph().slots.len() as u32)
        .map(|k| node_grads[g.slot_node(k).index()])
        .collect()
}

/// Composed scalar objective: `loss(model(inputs, params), targets)`, with
/// the model output vector feeding the loss graph's first inputs and
/// `targets` its remaining inputs. Returns `(loss, d loss / d slot of model)`.
#[must_use]
pub fn objective<R: Real>(
    model: &CheckedGraph,
    loss: &CheckedGraph,
    model_slots: &[R],
    targets: &[R],
    opts: &DerivOptions,
) -> (R, Vec<R>) {
    let mv = forward(model, model_slots);
    let ys: Vec<R> = model
        .graph()
        .outputs
        .iter()
        .map(|o| mv[o.index()])
        .collect();
    let mut ls = ys.clone();
    ls.extend_from_slice(targets);
    let lv = forward(loss, &ls);
    let l = lv[loss.graph().outputs[0].index()];
    let lg = backward(loss, &lv, &[R::of_f32(1.0)], &DerivOptions::default());
    let dy: Vec<R> = slot_grads(loss, &lg)[..ys.len()].to_vec();
    let mg = backward(model, &mv, &dy, opts);
    (l, slot_grads(model, &mg))
}

/// [`objective`] in binary32 with the reference failure semantics (design
/// D10): the model's first failing guard or refusing index read in canonical
/// order, then the loss graph's input facts and its first such failure, stop
/// the point. On success the result equals `objective::<f32>` bitwise.
///
/// # Errors
/// `G-FACT-FALSE` (model or loss graph) or `E-FACT-INPUT` (loss graph).
pub fn objective_checked(
    model: &CheckedGraph,
    loss: &CheckedGraph,
    model_slots: &[f32],
    targets: &[f32],
    opts: &DerivOptions,
) -> Result<(f32, Vec<f32>), Diagnostic> {
    objective_observed(model, loss, model_slots, targets, opts, &mut None)
}

pub(crate) fn objective_observed(
    model: &CheckedGraph,
    loss: &CheckedGraph,
    model_slots: &[f32],
    targets: &[f32],
    opts: &DerivOptions,
    evaluated_nodes: &mut Option<&mut u64>,
) -> Result<(f32, Vec<f32>), Diagnostic> {
    let mv = forward_observed(model, model_slots, evaluated_nodes);
    if let Some((node, fact)) = crate::eval::first_failure_observed(model, &mv, evaluated_nodes) {
        return Err(Diagnostic::new(
            Code::FactFalse,
            Site::Node(node),
            "model guard failed on a training point",
        )
        .with_fact(fact));
    }
    let ys: Vec<f32> = model
        .graph()
        .outputs
        .iter()
        .map(|o| mv[o.index()])
        .collect();
    let mut ls = ys.clone();
    ls.extend_from_slice(targets);
    crate::eval::preflight(loss, &crate::eval::f32_inputs(&ls), None)?;
    let lv = forward_observed(loss, &ls, evaluated_nodes);
    if let Some((node, fact)) = crate::eval::first_failure_observed(loss, &lv, evaluated_nodes) {
        return Err(Diagnostic::new(
            Code::FactFalse,
            Site::Node(node),
            "loss guard failed on a training point",
        )
        .with_fact(fact));
    }
    let l = lv[loss.graph().outputs[0].index()];
    let lg = backward(loss, &lv, &[1.0], &DerivOptions::default());
    let dy: Vec<f32> = slot_grads(loss, &lg)[..ys.len()].to_vec();
    let mg = backward(model, &mv, &dy, opts);
    Ok((l, slot_grads(model, &mg)))
}

/// Scale for the G2 rounding bound: the same reverse pass over absolute
/// values of every local derivative and forward value (a running-error
/// magnitude, ≥ |gradient|).
#[must_use]
pub fn magnitude(
    model: &CheckedGraph,
    loss: &CheckedGraph,
    model_slots: &[f64],
    targets: &[f64],
    opts: &DerivOptions,
) -> Vec<f64> {
    let mv: Vec<f64> = forward(model, model_slots);
    let sa: Vec<f64> = model_slots.iter().map(|v| v.abs()).collect();
    let ma = forward_abs(model, &mv, &sa);
    let outs = &model.graph().outputs;
    let mut ls: Vec<f64> = outs.iter().map(|o| mv[o.index()]).collect();
    let mut la: Vec<f64> = outs.iter().map(|o| ma[o.index()]).collect();
    ls.extend_from_slice(targets);
    la.extend(targets.iter().map(|t| t.abs()));
    let lv: Vec<f64> = forward(loss, &ls);
    let lva = forward_abs(loss, &lv, &la);
    let lg = backward_abs_with_signs(loss, &lv, &lva, &[1.0], &DerivOptions::default());
    let dy: Vec<f64> = slot_grads(loss, &lg)[..outs.len()].to_vec();
    let mg = backward_abs_with_signs(model, &mv, &ma, &dy, opts);
    slot_grads(model, &mg)
}

/// Forward magnitudes: the computation over absolute values (`|a|+|b|` for
/// `±`, `|a|·|b|` for `×`), with branches taken from the signed forward values.
fn forward_abs(g: &CheckedGraph, signed: &[f64], slot_abs: &[f64]) -> Vec<f64> {
    let graph = g.graph();
    let mut v: Vec<f64> = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        let r = match &node.op {
            Op::Input(k) | Op::Param(k) => slot_abs[*k as usize],
            Op::Const(crate::ir::Literal::F32(b)) => f64::from(f32::from_bits(*b)).abs(),
            Op::F32Add(a, b) | Op::F32Sub(a, b) => v[a.index()] + v[b.index()],
            Op::F32Mul(a, b) => v[a.index()] * v[b.index()],
            Op::F32Affine { bias, terms } => {
                let mut acc = f64::from(f32::from_bits(*bias)).abs();
                for t in terms {
                    let x = match t.operand {
                        AffineOperand::One => 1.0,
                        AffineOperand::Node(n) => v[n.index()],
                    };
                    acc += f64::from(f32::from_bits(t.coefficient)).abs() * x;
                }
                acc
            }
            Op::F32Relu(x) if signed[x.index()] > 0.0 => v[x.index()],
            Op::F32Reglu { left, gate } if signed[gate.index()] > 0.0 => {
                v[left.index()] * v[gate.index()]
            }
            Op::F32Step(_) | Op::F32Select { .. } | Op::F32Index { .. } => signed[v.len()].abs(),
            _ => 0.0,
        };
        v.push(r);
    }
    v
}

fn backward_abs_with_signs(
    g: &CheckedGraph,
    vals: &[f64],
    absvals: &[f64],
    seeds: &[f64],
    opts: &DerivOptions,
) -> Vec<f64> {
    // Same structure as `backward`, with |local derivative| and |value|;
    // branch decisions use the signed forward values.
    let graph = g.graph();
    let mut gr = vec![0.0f64; graph.nodes.len()];
    for (o, s) in graph.outputs.iter().zip(seeds) {
        gr[o.index()] += s.abs();
    }
    for (i, node) in graph.nodes.iter().enumerate().rev() {
        let id = NodeId(i as u32);
        let gn = gr[i];
        match &node.op {
            Op::F32Add(a, b) | Op::F32Sub(a, b) => {
                gr[a.index()] += gn;
                gr[b.index()] += gn;
            }
            Op::F32Mul(a, b) => {
                gr[a.index()] += gn * absvals[b.index()];
                gr[b.index()] += gn * absvals[a.index()];
            }
            Op::F32Affine { terms, .. } => {
                for t in terms {
                    if let AffineOperand::Node(n) = t.operand {
                        gr[n.index()] += gn * f64::from(f32::from_bits(t.coefficient)).abs();
                    }
                }
            }
            Op::F32Relu(a) if vals[a.index()] > 0.0 => gr[a.index()] += gn,
            Op::F32Reglu { left, gate } => {
                let vg = vals[gate.index()];
                gr[left.index()] += gn * if vg > 0.0 { absvals[gate.index()] } else { 0.0 };
                if vg > 0.0 {
                    gr[gate.index()] += gn * absvals[left.index()];
                }
            }
            Op::F32Step(s) => {
                if let BooleanPolicy::StraightThrough = opts.boolean(id) {
                    gr[s.index()] += gn;
                }
            }
            Op::F32Select { bit, off, on } => {
                if vals[bit.index()] == 1.0 {
                    gr[on.index()] += gn;
                } else {
                    gr[off.index()] += gn;
                }
            }
            Op::F32Index {
                index,
                table,
                below,
                above,
            } => {
                if let Some(Some(k)) = position(vals[index.index()], table.len(), *below, *above) {
                    gr[table[k].index()] += gn;
                }
            }
            _ => {}
        }
    }
    gr
}

/// Binary64 central difference of the composed objective in slot `k`.
#[must_use]
pub fn central_difference(
    model: &CheckedGraph,
    loss: &CheckedGraph,
    model_slots: &[f64],
    targets: &[f64],
    k: usize,
    h: f64,
) -> f64 {
    let mut plus = model_slots.to_vec();
    let mut minus = model_slots.to_vec();
    plus[k] += h;
    minus[k] -= h;
    let opts = DerivOptions::default();
    let (lp, _) = objective(model, loss, &plus, targets, &opts);
    let (lm, _) = objective(model, loss, &minus, targets, &opts);
    (lp - lm) / (2.0 * h)
}

/// Critical-path depth of the composed objective (model layers + loss layers).
#[must_use]
pub fn depth(model: &CheckedGraph, loss: &CheckedGraph) -> u32 {
    model.layer_count() + loss.layer_count()
}
