//! L4: the compiled CPU layered evaluator (D12 optional backend).
//!
//! Nodes are scheduled into dependency layers (handwired's depth rule; only
//! independent nodes are reordered), packed into one register file per
//! sub-profile, and run as a flat instruction stream. It must match the L3
//! reference under `≡obs`, including which guard fails first: failure points
//! (guards and refusing index reads) are total in this evaluator, so after
//! each layer it stops only once no unevaluated failure point precedes the
//! earliest observed failure in canonical order. Agreement is tested, not
//! proved (NG-PO-C stays open).

use crate::check::CheckedGraph;
use crate::diag::BudgetKind;
use crate::eval::{self, Control, Obs, Report, ScratchLease};
use crate::facts;
use crate::identity::IdentityScheme;
use crate::ir::{AffineOperand, Canon, Digest, Fact, NodeId, Op, OutOfRange, SubProfile};
use crate::sem::{self, Value};
use crate::snapshot::ParamSnapshot;
use alloc::{vec, vec::Vec};

/// Scheduler/compiler revision bound into the `PackedLayoutKey`.
pub const COMPILER_REVISION: &str = "ngraph-layered/0";

#[derive(Clone, Debug)]
enum F {
    Add(u32, u32, u32),
    Sub(u32, u32, u32),
    Mul(u32, u32, u32),
    Affine {
        d: u32,
        bias: f32,
        start: u32,
        end: u32,
    },
    Relu(u32, u32),
    Reglu(u32, u32, u32),
    Step(u32, u32),
    Select {
        d: u32,
        b: u32,
        off: u32,
        on: u32,
    },
    Index {
        d: u32,
        i: u32,
        start: u32,
        end: u32,
        below: OutOfRange,
        above: OutOfRange,
        node: u32,
    },
    Guard {
        node: u32,
    },
}

#[derive(Clone, Debug)]
enum I {
    Add(u32, u32, u32),
    Sub(u32, u32, u32),
    Mul(u32, u32, u32),
    Eq(u32, u32, u32),
    Guard { node: u32 },
}

#[derive(Clone, Debug)]
enum Code {
    F32 {
        ins: Vec<F>,
        coef: Vec<f32>,
        src: Vec<u32>,
    },
    I64 {
        ins: Vec<I>,
    },
}

/// A compiled graph: instruction stream in layer order plus register layout.
#[derive(Clone, Debug)]
pub struct Compiled {
    checked: CheckedGraph,
    code: Code,
    /// Instruction ranges per layer.
    layers: Vec<(u32, u32)>,
    /// For each layer, the minimum canonical index of a failure point in this
    /// or any later layer (`u32::MAX` if none).
    fail_suffix_min: Vec<u32>,
    /// Register of each node (`u32::MAX` for guards).
    reg: Vec<u32>,
    /// Register count (including the constant-one register).
    regs: u32,
    /// Constant registers and values (bits).
    consts: Vec<(u32, u64)>,
    /// (register, slot) of interface slots.
    slot_regs: Vec<(u32, u32)>,
    layout_key: Digest,
}

/// Reusable register workspace. Results are copied out of it (D12).
#[derive(Clone, Debug)]
pub struct Workspace {
    f: Vec<f32>,
    i: Vec<i64>,
}

impl Compiled {
    /// Compiles a checked graph.
    #[must_use]
    pub fn new(ids: &dyn IdentityScheme, checked: &CheckedGraph) -> Self {
        let g = checked.graph();
        let n = g.nodes.len();
        // Packed order: (layer, canonical index); guards keep their layer.
        let mut order: Vec<u32> = (0..n as u32).collect();
        order.sort_by_key(|&i| (checked.layer(NodeId(i)), i));
        let mut reg = vec![u32::MAX; n];
        let mut next = 0u32;
        for &i in &order {
            if !g.nodes[i as usize].op.is_guard() {
                reg[i as usize] = next;
                next += 1;
            }
        }
        let one_reg = next;
        let regs = next + 1;
        let r = |x: &NodeId| reg[x.index()];
        let mut consts = Vec::new();
        let mut slot_regs = Vec::new();
        let layer_count = checked.layer_count() as usize;
        let mut layers = vec![(0u32, 0u32); layer_count.max(1)];
        let mut fail_min_at_layer = vec![u32::MAX; layer_count.max(1)];
        let mut fins: Vec<F> = Vec::new();
        let mut iins: Vec<I> = Vec::new();
        let mut coef = Vec::new();
        let mut src = Vec::new();
        let mut cur_layer = u32::MAX;
        for &i in &order {
            let node = &g.nodes[i as usize];
            let l = checked.layer(NodeId(i));
            let count = |fins: &Vec<F>, iins: &Vec<I>| match g.profile {
                SubProfile::F32 => fins.len() as u32,
                SubProfile::I64 => iins.len() as u32,
            };
            if l != cur_layer {
                cur_layer = l;
                let c = count(&fins, &iins);
                layers[l as usize] = (c, c);
            }
            let d = reg[i as usize];
            match &node.op {
                Op::Input(k) | Op::Param(k) => slot_regs.push((d, *k)),
                Op::Const(lit) => {
                    let bits = match Value::of_literal(*lit) {
                        Value::F32(b) => u64::from(b),
                        Value::I64(v) => v as u64,
                        Value::Bool(b) => u64::from(b),
                    };
                    consts.push((d, bits));
                }
                Op::I64Add(a, b) => iins.push(I::Add(d, r(a), r(b))),
                Op::I64Sub(a, b) => iins.push(I::Sub(d, r(a), r(b))),
                Op::I64Mul(a, b) => iins.push(I::Mul(d, r(a), r(b))),
                Op::I64Eq(a, b) => iins.push(I::Eq(d, r(a), r(b))),
                Op::F32Add(a, b) => fins.push(F::Add(d, r(a), r(b))),
                Op::F32Sub(a, b) => fins.push(F::Sub(d, r(a), r(b))),
                Op::F32Mul(a, b) => fins.push(F::Mul(d, r(a), r(b))),
                Op::F32Affine { bias, terms } => {
                    let start = coef.len() as u32;
                    for t in terms {
                        coef.push(f32::from_bits(t.coefficient));
                        src.push(match t.operand {
                            AffineOperand::One => one_reg,
                            AffineOperand::Node(x) => r(&x),
                        });
                    }
                    fins.push(F::Affine {
                        d,
                        bias: f32::from_bits(*bias),
                        start,
                        end: coef.len() as u32,
                    });
                }
                Op::F32Relu(x) => fins.push(F::Relu(d, r(x))),
                Op::F32Reglu { left, gate } => fins.push(F::Reglu(d, r(left), r(gate))),
                Op::F32Step(x) => fins.push(F::Step(d, r(x))),
                Op::F32Select { bit, off, on } => fins.push(F::Select {
                    d,
                    b: r(bit),
                    off: r(off),
                    on: r(on),
                }),
                Op::F32Index {
                    index,
                    table,
                    below,
                    above,
                } => {
                    let start = src.len() as u32;
                    for t in table {
                        coef.push(0.0);
                        src.push(r(t));
                    }
                    if *below == OutOfRange::Refuse || *above == OutOfRange::Refuse {
                        let m = &mut fail_min_at_layer[l as usize];
                        *m = (*m).min(i);
                    }
                    fins.push(F::Index {
                        d,
                        i: r(index),
                        start,
                        end: src.len() as u32,
                        below: *below,
                        above: *above,
                        node: i,
                    });
                }
                Op::Guard { .. } => {
                    let m = &mut fail_min_at_layer[l as usize];
                    *m = (*m).min(i);
                    match g.profile {
                        SubProfile::F32 => fins.push(F::Guard { node: i }),
                        SubProfile::I64 => iins.push(I::Guard { node: i }),
                    }
                }
            }
            let c = count(&fins, &iins);
            layers[l as usize].1 = c;
        }
        let mut fail_suffix_min = vec![u32::MAX; layers.len() + 1];
        for l in (0..layers.len()).rev() {
            fail_suffix_min[l] = fail_suffix_min[l + 1].min(fail_min_at_layer[l]);
        }
        let mut c = Canon::new(ids, "noble.ngraph.packed-layout.v0");
        c.bytes(&checked.id());
        c.str(COMPILER_REVISION);
        c.str("order=(layer,canonical);regs=dense;one-reg=last");
        let layout_key = c.finish();
        let code = match g.profile {
            SubProfile::F32 => Code::F32 {
                ins: fins,
                coef,
                src,
            },
            SubProfile::I64 => Code::I64 { ins: iins },
        };
        Self {
            checked: checked.clone(),
            code,
            layers,
            fail_suffix_min,
            reg,
            regs,
            consts,
            slot_regs,
            layout_key,
        }
    }

    /// `PackedLayoutKey = H(GraphLogicalId ‖ compiler rev ‖ layout options)`
    /// (a build key, not semantic identity).
    #[must_use]
    pub const fn layout_key(&self) -> Digest {
        self.layout_key
    }

    /// Estimated retained bytes of the compiled code (instructions, term
    /// arrays, register map, constants, layer tables), excluding the graph.
    #[must_use]
    pub fn retained_bytes(&self) -> u64 {
        let code = match &self.code {
            Code::F32 { ins, coef, src } => {
                ins.len() * core::mem::size_of::<F>() + coef.len() * 4 + src.len() * 4
            }
            Code::I64 { ins } => ins.len() * core::mem::size_of::<I>(),
        };
        (code
            + self.layers.len() * 8
            + self.fail_suffix_min.len() * 4
            + self.reg.len() * 4
            + self.consts.len() * 12
            + self.slot_regs.len() * 8) as u64
    }

    /// The checked graph.
    #[must_use]
    pub const fn checked(&self) -> &CheckedGraph {
        &self.checked
    }

    /// Scratch bytes of one workspace.
    #[must_use]
    pub fn scratch_bytes(&self) -> u64 {
        u64::from(self.regs)
            * match self.code {
                Code::F32 { .. } => 4,
                Code::I64 { .. } => 8,
            }
    }

    /// A fresh workspace with constants loaded.
    #[must_use]
    pub fn workspace(&self) -> Workspace {
        let n = self.regs as usize;
        let mut w = match self.code {
            Code::F32 { .. } => Workspace {
                f: vec![0.0; n],
                i: Vec::new(),
            },
            Code::I64 { .. } => Workspace {
                f: Vec::new(),
                i: vec![0; n],
            },
        };
        self.reset(&mut w);
        w
    }

    /// Resets the workspace to its post-construction state (constants and
    /// the constant-one register).
    pub fn reset(&self, w: &mut Workspace) {
        match self.code {
            Code::F32 { .. } => {
                for &(r, bits) in &self.consts {
                    w.f[r as usize] = f32::from_bits(bits as u32);
                }
                w.f[self.regs as usize - 1] = 1.0;
            }
            Code::I64 { .. } => {
                for &(r, bits) in &self.consts {
                    w.i[r as usize] = bits as i64;
                }
            }
        }
    }

    /// Evaluates with a fresh workspace.
    #[must_use]
    pub fn evaluate(
        &self,
        inputs: &[Value],
        snapshot: Option<&ParamSnapshot>,
        control: &mut Control<'_>,
    ) -> Report {
        let mut w = self.workspace();
        self.evaluate_in(&mut w, inputs, snapshot, control)
    }

    fn get(&self, w: &Workspace, n: NodeId) -> Value {
        let r = self.reg[n.index()] as usize;
        match self.checked.ty(n) {
            Some(crate::ir::ScalarType::F32) => Value::F32(w.f[r].to_bits()),
            Some(crate::ir::ScalarType::Bool) => Value::Bool(w.i[r] != 0),
            _ => Value::I64(w.i[r]),
        }
    }

    /// Evaluates in a reusable workspace. Results are fresh values.
    #[must_use]
    pub fn evaluate_in(
        &self,
        w: &mut Workspace,
        inputs: &[Value],
        snapshot: Option<&ParamSnapshot>,
        control: &mut Control<'_>,
    ) -> Report {
        let slots = match eval::preflight(&self.checked, inputs, snapshot) {
            Ok(v) => v,
            Err(d) => {
                return Report {
                    obs: Obs::Refused(d),
                    evaluated_nodes: 0,
                }
            }
        };
        let bytes = self.scratch_bytes();
        if let Err(d) = eval::scratch_preflight(bytes, control) {
            return Report {
                obs: Obs::Refused(d),
                evaluated_nodes: 0,
            };
        }
        let _lease = ScratchLease::acquire(control.scratch.as_deref_mut(), bytes);
        for &(r, k) in &self.slot_regs {
            match slots[k as usize] {
                Value::F32(b) => w.f[r as usize] = f32::from_bits(b),
                Value::I64(v) => w.i[r as usize] = v,
                Value::Bool(b) => w.i[r as usize] = i64::from(b),
            }
        }
        let mut evaluated = (self.consts.len() + self.slot_regs.len()) as u64;
        let mut fail: Option<(u32, Fact)> = None;
        for (l, &(start, end)) in self.layers.iter().enumerate() {
            if control.cancel.is_some_and(|c| c(l as u32)) {
                return Report {
                    obs: Obs::Cancelled,
                    evaluated_nodes: evaluated,
                };
            }
            let width = u64::from(end - start);
            if control.fuel.is_some_and(|f| evaluated + width > f) {
                return Report {
                    obs: Obs::Exhausted(BudgetKind::Fuel),
                    evaluated_nodes: evaluated,
                };
            }
            evaluated += width;
            let f = match &self.code {
                Code::F32 { ins, coef, src } => {
                    self.run_f32(&ins[start as usize..end as usize], coef, src, w)
                }
                Code::I64 { ins } => self.run_i64(&ins[start as usize..end as usize], w),
            };
            if let Some((node, fact)) = f {
                if fail.as_ref().is_none_or(|(m, _)| node < *m) {
                    fail = Some((node, fact));
                }
            }
            if let Some((m, _)) = &fail {
                if *m < self.fail_suffix_min[l + 1] {
                    break;
                }
            }
        }
        if let Some((node, fact)) = fail {
            return Report {
                obs: Obs::GuardFailed {
                    node: NodeId(node),
                    fact,
                },
                evaluated_nodes: evaluated,
            };
        }
        let outs = self
            .checked
            .graph()
            .outputs
            .iter()
            .map(|o| self.get(w, *o))
            .collect();
        Report {
            obs: Obs::Normal(outs),
            evaluated_nodes: evaluated,
        }
    }

    /// Runs one layer; returns the earliest (canonical) failure in it.
    fn run_f32(
        &self,
        ins: &[F],
        coef: &[f32],
        src: &[u32],
        w: &mut Workspace,
    ) -> Option<(u32, Fact)> {
        let mut fail: Option<(u32, Fact)> = None;
        let mut note = |node: u32, fact: Fact| {
            if fail.as_ref().is_none_or(|(m, _)| node < *m) {
                fail = Some((node, fact));
            }
        };
        let rf = &mut w.f;
        for x in ins {
            match *x {
                F::Add(d, a, b) => rf[d as usize] = sem::add(rf[a as usize], rf[b as usize]),
                F::Sub(d, a, b) => rf[d as usize] = sem::sub(rf[a as usize], rf[b as usize]),
                F::Mul(d, a, b) => rf[d as usize] = sem::mul(rf[a as usize], rf[b as usize]),
                F::Affine {
                    d,
                    bias,
                    start,
                    end,
                } => {
                    let (s, e) = (start as usize, end as usize);
                    let mut v = bias;
                    for (c, r) in coef[s..e].iter().zip(&src[s..e]) {
                        v += c * rf[*r as usize];
                    }
                    rf[d as usize] = v;
                }
                F::Relu(d, a) => rf[d as usize] = sem::relu(rf[a as usize]),
                F::Reglu(d, a, b) => rf[d as usize] = sem::reglu(rf[a as usize], rf[b as usize]),
                F::Step(d, a) => rf[d as usize] = sem::step(rf[a as usize]),
                F::Select { d, b, off, on } => {
                    rf[d as usize] = f32::from_bits(sem::select(
                        rf[b as usize].to_bits(),
                        rf[off as usize].to_bits(),
                        rf[on as usize].to_bits(),
                    ));
                }
                F::Index {
                    d,
                    i,
                    start,
                    end,
                    below,
                    above,
                    node,
                } => {
                    let len = (end - start) as usize;
                    match sem::index_position(rf[i as usize], len, below, above) {
                        Ok(Some(k)) => rf[d as usize] = rf[src[start as usize + k] as usize],
                        Ok(None) => rf[d as usize] = 0.0,
                        Err(sem::IndexRefused) => note(node, eval::index_refusal_fact(len)),
                    }
                }
                F::Guard { node } => {
                    if let Some(f) = self.guard_fails(node, w_view_f(rf)) {
                        note(node, f);
                    }
                }
            }
        }
        fail
    }

    fn run_i64(&self, ins: &[I], w: &mut Workspace) -> Option<(u32, Fact)> {
        let mut fail: Option<(u32, Fact)> = None;
        let ri = &mut w.i;
        for x in ins {
            match *x {
                I::Add(d, a, b) => ri[d as usize] = sem::i64_add(ri[a as usize], ri[b as usize]),
                I::Sub(d, a, b) => ri[d as usize] = sem::i64_sub(ri[a as usize], ri[b as usize]),
                I::Mul(d, a, b) => ri[d as usize] = sem::i64_mul(ri[a as usize], ri[b as usize]),
                I::Eq(d, a, b) => ri[d as usize] = i64::from(ri[a as usize] == ri[b as usize]),
                I::Guard { node } => {
                    if let Some(f) = self.guard_fails(node, View::I(ri)) {
                        if fail.as_ref().is_none_or(|(m, _)| node < *m) {
                            fail = Some((node, f));
                        }
                    }
                }
            }
        }
        fail
    }

    fn guard_fails(&self, node: u32, view: View<'_>) -> Option<Fact> {
        let Op::Guard { fact, value } = &self.checked.graph().nodes[node as usize].op else {
            return None;
        };
        let get = |n: NodeId| {
            let r = self.reg[n.index()] as usize;
            match (view, self.checked.ty(n)) {
                (View::F(f), _) => Value::F32(f[r].to_bits()),
                (View::I(i), Some(crate::ir::ScalarType::Bool)) => Value::Bool(i[r] != 0),
                (View::I(i), _) => Value::I64(i[r]),
            }
        };
        let group = facts::group_values(fact, get);
        let shape = match &self.checked.graph().nodes[value.index()].op {
            Op::F32Index { table, .. } => Some(table.len() as u32),
            _ => None,
        };
        if facts::holds(fact, get(*value), &group, shape) {
            None
        } else {
            Some(fact.clone())
        }
    }
}

#[derive(Clone, Copy)]
enum View<'a> {
    F(&'a [f32]),
    I(&'a [i64]),
}

fn w_view_f(rf: &[f32]) -> View<'_> {
    View::F(rf)
}
