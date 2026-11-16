#![expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; the module path names the domain and the item name keeps its canonical operation spelling"
)]

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
//!
//! A reusable [`Workspace`] belongs to the compiled value that created it and
//! to that value's clones (issue #133). Evaluation and reset refuse every other
//! workspace before writing a register, so one graph's constants or register
//! layout never reach another graph's result.

use crate::identity::IdentityScheme;
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
        below: crate::ir::OutOfRange,
        above: crate::ir::OutOfRange,
        node: u32,
    },
    Guard {
        node: u32,
    },
}

/// The packed I64/Bool instruction stream's element type lives in [`crate::schedule`] so an
/// independent validator can inspect the actual emitted bodies (issue #43).

#[derive(Clone, Debug)]
enum Code {
    F32 {
        ins: alloc::vec::Vec<F>,
        coef: alloc::vec::Vec<f32>,
        src: alloc::vec::Vec<u32>,
    },
    I64 {
        ins: alloc::vec::Vec<crate::schedule::I64Instruction>,
    },
}

/// A compiled graph: instruction stream in layer order plus register layout.
#[derive(Clone, Debug)]
pub struct Compiled {
    checked: crate::check::CheckedGraph,
    code: Code,
    /// Instruction ranges per layer.
    layers: alloc::vec::Vec<(u32, u32)>,
    /// For each layer, the minimum canonical index of a failure point in this
    /// or any later layer (`u32::MAX` if none).
    fail_suffix_min: alloc::vec::Vec<u32>,
    /// Register of each node (`u32::MAX` for guards).
    reg: alloc::vec::Vec<u32>,
    /// Register count (including the constant-one register).
    regs: u32,
    /// Constant registers and values (bits).
    consts: alloc::vec::Vec<(u32, u64)>,
    /// (register, slot) of interface slots.
    slot_regs: alloc::vec::Vec<(u32, u32)>,
    layout_key: crate::ir::Digest,
    /// Identity of this compilation, shared by its clones and its workspaces.
    owner: alloc::rc::Rc<Owner>,
}

/// The identity a [`Workspace`] is bound to: one allocation per
/// [`Compiled::new`] call, shared by that value's clones and by every workspace
/// it creates, and compared by address only. It holds no data, so a workspace
/// keeps neither the graph nor the compiled code alive, and its allocation
/// cannot be reused by another compilation while any workspace still refers to
/// it.
#[derive(Debug)]
struct Owner;

/// Reusable register workspace. Results are copied out of it (D12).
///
/// A workspace is bound to the compiled value that created it
/// ([`Compiled::workspace`]): that value and its clones may evaluate in it and
/// reset it, and every other compiled value is refused with `E-INTERFACE`
/// before any register is written. Another [`Compiled::new`] call is another
/// identity even for the same checked graph. The binding is an identity, not a
/// comparison of register counts, declared layouts or digests, so neither an
/// equal size, an equal layout nor a colliding identity scheme makes another
/// graph's constants, register map, slot map or output map reachable. A
/// workspace never changes its owner or its size, and a cloned workspace keeps
/// the binding.
#[derive(Clone, Debug)]
pub struct Workspace {
    f: alloc::vec::Vec<f32>,
    i: alloc::vec::Vec<i64>,
    owner: alloc::rc::Rc<Owner>,
}

impl Compiled {
    /// Compiles a checked graph.
    #[must_use]
    pub fn new(ids: &dyn IdentityScheme, checked: &crate::check::CheckedGraph) -> Self {
        let g = checked.graph();
        let n = g.nodes.len();
        // Packed order: (layer, canonical index); guards keep their layer.
        let mut order: alloc::vec::Vec<u32> = (0..n as u32).collect();
        order.sort_by_key(|&i| (checked.layer(crate::ir::NodeId(i)), i));
        let mut reg = alloc::vec![u32::MAX; n];
        let mut next = 0u32;
        for &i in &order {
            if !g.nodes[i as usize].op.is_guard() {
                reg[i as usize] = next;
                next += 1;
            }
        }
        let one_reg = next;
        let regs = next + 1;
        let r = |x: &crate::ir::NodeId| reg[x.index()];
        let mut consts = alloc::vec::Vec::with_capacity(n);
        let mut slot_regs = alloc::vec::Vec::with_capacity(n);
        let layer_count = checked.layer_count() as usize;
        // Exactly the checker's layers, including none for an empty graph: the
        // layout validator requires one range per layer and one trailing
        // failure-suffix entry, which `layer_count + 1` below supplies.
        let mut layers = alloc::vec![(0u32, 0u32); layer_count];
        let mut fail_min_at_layer = alloc::vec![u32::MAX; layer_count];
        let mut fins: alloc::vec::Vec<F> = alloc::vec::Vec::with_capacity(n);
        let mut iins: alloc::vec::Vec<crate::schedule::I64Instruction> =
            alloc::vec::Vec::with_capacity(n);
        let mut coef = alloc::vec::Vec::with_capacity(n);
        let mut src = alloc::vec::Vec::with_capacity(n);
        let mut cur_layer = u32::MAX;
        for &i in &order {
            let node = &g.nodes[i as usize];
            let l = checked.layer(crate::ir::NodeId(i));
            if l != cur_layer {
                cur_layer = l;
                let c = match g.profile {
                    crate::ir::SubProfile::F32 => fins.len() as u32,
                    crate::ir::SubProfile::I64 => iins.len() as u32,
                };
                layers[l as usize] = (c, c);
            }
            let d = reg[i as usize];
            match &node.op {
                crate::ir::Op::Input(k) | crate::ir::Op::Param(k) => slot_regs.push((d, *k)),
                crate::ir::Op::Const(lit) => {
                    let bits = match crate::sem::Value::of_literal(*lit) {
                        crate::sem::Value::F32(b) => u64::from(b),
                        crate::sem::Value::I64(v) => v as u64,
                        crate::sem::Value::Bool(b) => u64::from(b),
                    };
                    consts.push((d, bits));
                }
                crate::ir::Op::I64Add(a, b) => {
                    iins.push(crate::schedule::I64Instruction::Add(d, r(a), r(b)))
                }
                crate::ir::Op::I64Sub(a, b) => {
                    iins.push(crate::schedule::I64Instruction::Sub(d, r(a), r(b)))
                }
                crate::ir::Op::I64Mul(a, b) => {
                    iins.push(crate::schedule::I64Instruction::Mul(d, r(a), r(b)))
                }
                crate::ir::Op::I64Eq(a, b) => {
                    iins.push(crate::schedule::I64Instruction::Eq(d, r(a), r(b)))
                }
                crate::ir::Op::F32Add(a, b) => fins.push(F::Add(d, r(a), r(b))),
                crate::ir::Op::F32Sub(a, b) => fins.push(F::Sub(d, r(a), r(b))),
                crate::ir::Op::F32Mul(a, b) => fins.push(F::Mul(d, r(a), r(b))),
                crate::ir::Op::F32Affine { bias, terms } => {
                    let start = coef.len() as u32;
                    for t in terms {
                        coef.push(f32::from_bits(t.coefficient));
                        src.push(match t.operand {
                            crate::ir::AffineOperand::One => one_reg,
                            crate::ir::AffineOperand::Node(x) => r(&x),
                        });
                    }
                    fins.push(F::Affine {
                        d,
                        bias: f32::from_bits(*bias),
                        start,
                        end: coef.len() as u32,
                    });
                }
                crate::ir::Op::F32Relu(x) => fins.push(F::Relu(d, r(x))),
                crate::ir::Op::F32Reglu { left, gate } => fins.push(F::Reglu(d, r(left), r(gate))),
                crate::ir::Op::F32Step(x) => fins.push(F::Step(d, r(x))),
                crate::ir::Op::F32Select { bit, off, on } => fins.push(F::Select {
                    d,
                    b: r(bit),
                    off: r(off),
                    on: r(on),
                }),
                crate::ir::Op::F32Index {
                    index,
                    below,
                    above,
                    ..
                } => {
                    if *below == crate::ir::OutOfRange::Refuse
                        || *above == crate::ir::OutOfRange::Refuse
                    {
                        let m = &mut fail_min_at_layer[l as usize];
                        *m = (*m).min(i);
                    }
                    fins.push(F::Index {
                        d,
                        i: r(index),
                        below: *below,
                        above: *above,
                        node: i,
                    });
                }
                crate::ir::Op::Guard { .. } => {
                    let m = &mut fail_min_at_layer[l as usize];
                    *m = (*m).min(i);
                    match g.profile {
                        crate::ir::SubProfile::F32 => fins.push(F::Guard { node: i }),
                        crate::ir::SubProfile::I64 => {
                            iins.push(crate::schedule::I64Instruction::Guard { node: i })
                        }
                    }
                }
            }
            layers[l as usize].1 = match g.profile {
                crate::ir::SubProfile::F32 => fins.len() as u32,
                crate::ir::SubProfile::I64 => iins.len() as u32,
            };
        }
        let mut fail_suffix_min = alloc::vec![u32::MAX; layers.len() + 1];
        for l in (0..layers.len()).rev() {
            fail_suffix_min[l] = fail_suffix_min[l + 1].min(fail_min_at_layer[l]);
        }
        let layout_key = crate::layout::packed_layout_key(ids, &checked.id());
        let code = match g.profile {
            crate::ir::SubProfile::F32 => Code::F32 {
                ins: fins,
                coef,
                src,
            },
            crate::ir::SubProfile::I64 => Code::I64 { ins: iins },
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
            owner: alloc::rc::Rc::new(Owner),
        }
    }

    /// The declared packed layout, for independent validation. It carries no
    /// instruction bodies: a validator may only check the register map, the
    /// constant and slot tables, the register count and the build key against the
    /// graph, and the instruction stream stays the scheduler's own business.
    #[must_use]
    pub fn packed_layout(&self) -> crate::layout::PackedLayout {
        crate::layout::PackedLayout {
            revision: alloc::string::String::from(crate::layout::LAYOUT_REVISION),
            compiler_revision: alloc::string::String::from(COMPILER_REVISION),
            regs: self.regs,
            reg: self.reg.clone(),
            consts: self.consts.clone(),
            slot_regs: self.slot_regs.clone(),
            layers: self.layers.clone(),
            fail_suffix_min: self.fail_suffix_min.clone(),
            layout_key: self.layout_key,
        }
    }

    /// `PackedLayoutKey` (`crate::layout::packed_layout_key`): a build key, not semantic identity.
    #[must_use]
    pub const fn layout_key(&self) -> crate::ir::Digest {
        self.layout_key
    }

    /// The actual packed I64/Bool instruction stream, for independent validation
    /// (`crate::schedule::check`). `None` for the F32 sub-profile, whose stream is
    /// not covered by that validator yet.
    #[must_use]
    pub fn i64_instructions(&self) -> Option<&[crate::schedule::I64Instruction]> {
        match &self.code {
            Code::I64 { ins } => Some(ins),
            Code::F32 { .. } => None,
        }
    }

    /// Estimated retained bytes of the compiled code (instructions, term
    /// arrays, register map, constants, layer tables), excluding the graph.
    #[must_use]
    pub fn retained_bytes(&self) -> u64 {
        let code = match &self.code {
            Code::F32 { ins, coef, src } => {
                ins.len() * core::mem::size_of::<F>() + coef.len() * 4 + src.len() * 4
            }
            Code::I64 { ins } => {
                ins.len() * core::mem::size_of::<crate::schedule::I64Instruction>()
            }
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
    pub const fn checked(&self) -> &crate::check::CheckedGraph {
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

    /// A fresh workspace bound to this compiled value (see [`Workspace`]): every
    /// register zero except this graph's constants and, for F32, the
    /// constant-one register.
    ///
    /// It allocates [`Compiled::scratch_bytes`] of registers without consulting
    /// a budget, so a caller that keeps it for several [`Compiled::evaluate_in`]
    /// calls asks [`Compiled::scratch_refusal`] first.
    #[must_use]
    pub fn workspace(&self) -> Workspace {
        let n = self.regs as usize;
        let mut w = match self.code {
            Code::F32 { .. } => Workspace {
                f: alloc::vec![0.0; n],
                i: alloc::vec::Vec::new(),
                owner: alloc::rc::Rc::clone(&self.owner),
            },
            Code::I64 { .. } => Workspace {
                f: alloc::vec::Vec::new(),
                i: alloc::vec![0; n],
                owner: alloc::rc::Rc::clone(&self.owner),
            },
        };
        self.load_constants(&mut w);
        w
    }

    /// Restores a workspace bound to this compiled value to the state
    /// [`Compiled::workspace`] creates: every register zero, then this graph's
    /// constants and, for F32, the constant-one register. It never allocates.
    ///
    /// Evaluation never writes a constant or the constant-one register, so a
    /// bound workspace needs no reset between evaluations; a reset only clears
    /// values that earlier runs left in other registers.
    ///
    /// # Errors
    /// `E-INTERFACE` when `w` is bound to another compiled value, under the same
    /// contract as [`Compiled::evaluate_in`]; `w` is then left unchanged. A reset
    /// never rebinds a workspace to another graph: that graph's own
    /// [`Compiled::workspace`] provides one.
    pub fn reset(&self, w: &mut Workspace) -> Result<(), crate::diag::Diagnostic> {
        if let Some(refusal) = self.binding_refusal(w) {
            return Err(refusal);
        }
        w.f.fill(0.0);
        w.i.fill(0);
        self.load_constants(w);
        Ok(())
    }

    /// Writes this graph's constants and, for F32, the constant-one register
    /// into a workspace bound to it.
    #[expect(
        tigerstyle::unchecked_narrowing,
        reason = "Owner: noble-maintainers; F32 constants are stored from f32::to_bits and already fit u32"
    )]
    fn load_constants(&self, w: &mut Workspace) {
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

    /// The refusal of a workspace bound to another compiled value, or `None`
    /// when `w` belongs to this one or to one of its clones.
    fn binding_refusal(&self, w: &Workspace) -> Option<crate::diag::Diagnostic> {
        if alloc::rc::Rc::ptr_eq(&w.owner, &self.owner) {
            return None;
        }
        Some(crate::diag::Diagnostic::new(
            crate::diag::Code::Interface,
            crate::diag::Site::Graph,
            "workspace belongs to another compiled graph; evaluate in this graph's own workspace",
        ))
    }

    /// Evaluates with a fresh workspace.
    #[must_use]
    pub fn evaluate(
        &self,
        inputs: &[crate::sem::Value],
        snapshot: Option<&crate::snapshot::ParamSnapshot>,
        control: &mut crate::eval::Control<'_>,
    ) -> crate::eval::Report {
        if let Some(refusal) = self.scratch_refusal(control) {
            return refusal;
        }
        let mut w = self.workspace();
        self.evaluate_in(&mut w, inputs, snapshot, control)
    }

    /// The refusal of one workspace against the control's scratch limit, or
    /// `None` when it fits.
    ///
    /// The register file is the scratch the budget bounds, so a caller that
    /// allocates one workspace for several [`Compiled::evaluate_in`] calls must
    /// ask this *before* allocating; [`Compiled::evaluate`] asks first itself.
    /// The refusal is the one every evaluation would report under the same
    /// control, with no node evaluated.
    #[must_use]
    pub fn scratch_refusal(
        &self,
        control: &crate::eval::Control<'_>,
    ) -> Option<crate::eval::Report> {
        crate::eval::scratch_preflight(self.scratch_bytes(), control)
            .err()
            .map(|d| crate::eval::Report {
                obs: crate::eval::Obs::Refused(d),
                evaluated_nodes: 0,
            })
    }

    fn get(&self, w: &Workspace, n: crate::ir::NodeId) -> crate::sem::Value {
        let r = self.reg[n.index()] as usize;
        match self.checked.ty(n) {
            Some(crate::ir::ScalarType::F32) => crate::sem::Value::F32(w.f[r].to_bits()),
            Some(crate::ir::ScalarType::Bool) => crate::sem::Value::Bool(w.i[r] != 0),
            Some(crate::ir::ScalarType::I64) | None => crate::sem::Value::I64(w.i[r]),
        }
    }

    /// Evaluates in a reusable workspace. Results are fresh values copied out of
    /// it.
    ///
    /// `w` must be bound to this compiled value: created by
    /// [`Compiled::workspace`] on it or on one of its clones (see [`Workspace`]).
    /// Any other workspace, including one of the same profile and size or of an
    /// equal declared layout, is refused with `E-INTERFACE` first: before the
    /// input preflight, before any scratch lease or caller counter is touched
    /// and before any register is written, with no node evaluated. The refused
    /// workspace still serves its owner unchanged. The check compares one
    /// identity, so the valid repeated path adds no allocation and no per-node
    /// work.
    ///
    /// The control's scratch limit is checked on every call, so a limit lowered
    /// after the workspace was allocated still refuses with
    /// `E-BUDGET-FORWARD-SCRATCH`.
    ///
    /// Failure atomicity: every refusal leaves the workspace and the caller's
    /// counters unchanged. A guard failure, fuel exhaustion or cancellation may
    /// leave the values of executed instructions in their registers. A later
    /// successful evaluation writes each of those registers before reading it and
    /// never writes a constant or the constant-one register, so no value of an
    /// earlier call reaches its outputs.
    #[must_use]
    pub fn evaluate_in(
        &self,
        w: &mut Workspace,
        inputs: &[crate::sem::Value],
        snapshot: Option<&crate::snapshot::ParamSnapshot>,
        control: &mut crate::eval::Control<'_>,
    ) -> crate::eval::Report {
        if let Some(refusal) = self.binding_refusal(w) {
            return crate::eval::Report {
                obs: crate::eval::Obs::Refused(refusal),
                evaluated_nodes: 0,
            };
        }
        let slots = match crate::eval::preflight(&self.checked, inputs, snapshot) {
            Ok(v) => v,
            Err(d) => {
                return crate::eval::Report {
                    obs: crate::eval::Obs::Refused(d),
                    evaluated_nodes: 0,
                }
            }
        };
        let bytes = self.scratch_bytes();
        if let Err(d) = crate::eval::scratch_preflight(bytes, control) {
            return crate::eval::Report {
                obs: crate::eval::Obs::Refused(d),
                evaluated_nodes: 0,
            };
        }
        let _lease = crate::eval::ScratchLease::acquire(control.scratch.as_deref_mut(), bytes);
        for &(r, k) in &self.slot_regs {
            if let Some(counter) = control.evaluated_nodes.as_deref_mut() {
                *counter += 1;
            }
            match slots[k as usize] {
                crate::sem::Value::F32(b) => w.f[r as usize] = f32::from_bits(b),
                crate::sem::Value::I64(v) => w.i[r as usize] = v,
                crate::sem::Value::Bool(b) => w.i[r as usize] = i64::from(b),
            }
        }
        let mut evaluated = (self.consts.len() + self.slot_regs.len()) as u64;
        let mut fail: Option<(u32, crate::ir::Fact)> = None;
        for (l, &(start, end)) in self.layers.iter().enumerate() {
            if control.cancel.is_some_and(|c| c(l as u32)) {
                return crate::eval::Report {
                    obs: crate::eval::Obs::Cancelled,
                    evaluated_nodes: evaluated,
                };
            }
            let width = u64::from(end - start);
            if control.fuel.is_some_and(|f| evaluated + width > f) {
                return crate::eval::Report {
                    obs: crate::eval::Obs::Exhausted(crate::diag::BudgetKind::Fuel),
                    evaluated_nodes: evaluated,
                };
            }
            evaluated += width;
            // Select once per layer: the ordinary specialization contains no
            // per-instruction observation work. Both execute the same stream.
            let f = match &self.code {
                Code::F32 { ins, coef, src } if control.evaluated_nodes.is_some() => self
                    .run_f32::<true>(
                        &ins[start as usize..end as usize],
                        coef,
                        src,
                        w,
                        &mut control.evaluated_nodes,
                    ),
                Code::F32 { ins, coef, src } => self.run_f32::<false>(
                    &ins[start as usize..end as usize],
                    coef,
                    src,
                    w,
                    &mut control.evaluated_nodes,
                ),
                Code::I64 { ins } if control.evaluated_nodes.is_some() => self.run_i64::<true>(
                    &ins[start as usize..end as usize],
                    w,
                    &mut control.evaluated_nodes,
                ),
                Code::I64 { ins } => self.run_i64::<false>(
                    &ins[start as usize..end as usize],
                    w,
                    &mut control.evaluated_nodes,
                ),
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
            return crate::eval::Report {
                obs: crate::eval::Obs::GuardFailed {
                    node: crate::ir::NodeId(node),
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
        crate::eval::Report {
            obs: crate::eval::Obs::Normal(outs),
            evaluated_nodes: evaluated,
        }
    }

    /// Runs one layer; returns the earliest (canonical) failure in it.
    #[expect(
        tigerstyle::no_panic,
        reason = "Owner: noble-maintainers; the checked graph retains this exact index node, so the match arm is unreachable by construction"
    )]
    fn run_f32<const OBSERVED: bool>(
        &self,
        ins: &[F],
        coef: &[f32],
        src: &[u32],
        w: &mut Workspace,
        evaluated_nodes: &mut Option<&mut u64>,
    ) -> Option<(u32, crate::ir::Fact)> {
        let mut fail: Option<(u32, crate::ir::Fact)> = None;
        let mut note = |node: u32, fact: crate::ir::Fact| {
            if fail.as_ref().is_none_or(|(m, _)| node < *m) {
                fail = Some((node, fact));
            }
        };
        let rf = &mut w.f;
        for x in ins {
            if OBSERVED {
                if let Some(counter) = evaluated_nodes.as_deref_mut() {
                    *counter += 1;
                }
            }
            match *x {
                F::Add(d, a, b) => rf[d as usize] = crate::sem::add(rf[a as usize], rf[b as usize]),
                F::Sub(d, a, b) => rf[d as usize] = crate::sem::sub(rf[a as usize], rf[b as usize]),
                F::Mul(d, a, b) => rf[d as usize] = crate::sem::mul(rf[a as usize], rf[b as usize]),
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
                F::Relu(d, a) => rf[d as usize] = crate::sem::relu(rf[a as usize]),
                F::Reglu(d, a, b) => {
                    rf[d as usize] = crate::sem::reglu(rf[a as usize], rf[b as usize])
                }
                F::Step(d, a) => rf[d as usize] = crate::sem::step(rf[a as usize]),
                F::Select { d, b, off, on } => {
                    rf[d as usize] = f32::from_bits(crate::sem::select(
                        rf[b as usize].to_bits(),
                        rf[off as usize].to_bits(),
                        rf[on as usize].to_bits(),
                    ));
                }
                F::Index {
                    d,
                    i,
                    below,
                    above,
                    node,
                } => {
                    // The checked graph already retains this exact ordered
                    // table. Only the selected operand needs a register lookup.
                    let crate::ir::Op::F32Index { table, .. } =
                        &self.checked.graph().nodes[node as usize].op
                    else {
                        unreachable!("compiled index comes from a checked index node");
                    };
                    let len = table.len();
                    match crate::sem::index_position(rf[i as usize], len, below, above) {
                        Ok(Some(k)) => rf[d as usize] = rf[self.reg[table[k].index()] as usize],
                        Ok(None) => rf[d as usize] = 0.0,
                        Err(crate::sem::IndexRefused) => {
                            note(node, crate::eval::index_refusal_fact(len))
                        }
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

    fn run_i64<const OBSERVED: bool>(
        &self,
        ins: &[crate::schedule::I64Instruction],
        w: &mut Workspace,
        evaluated_nodes: &mut Option<&mut u64>,
    ) -> Option<(u32, crate::ir::Fact)> {
        let mut fail: Option<(u32, crate::ir::Fact)> = None;
        let ri = &mut w.i;
        for x in ins {
            if OBSERVED {
                if let Some(counter) = evaluated_nodes.as_deref_mut() {
                    *counter += 1;
                }
            }
            match *x {
                crate::schedule::I64Instruction::Add(d, a, b) => {
                    ri[d as usize] = crate::sem::i64_add(ri[a as usize], ri[b as usize])
                }
                crate::schedule::I64Instruction::Sub(d, a, b) => {
                    ri[d as usize] = crate::sem::i64_sub(ri[a as usize], ri[b as usize])
                }
                crate::schedule::I64Instruction::Mul(d, a, b) => {
                    ri[d as usize] = crate::sem::i64_mul(ri[a as usize], ri[b as usize])
                }
                crate::schedule::I64Instruction::Eq(d, a, b) => {
                    ri[d as usize] = i64::from(ri[a as usize] == ri[b as usize])
                }
                crate::schedule::I64Instruction::Guard { node } => {
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

    fn guard_fails(&self, node: u32, view: View<'_>) -> Option<crate::ir::Fact> {
        let crate::ir::Op::Guard { fact, value } = &self.checked.graph().nodes[node as usize].op
        else {
            return None;
        };
        let get = |n: crate::ir::NodeId| {
            let r = self.reg[n.index()] as usize;
            match (view, self.checked.ty(n)) {
                (View::F(f), _) => crate::sem::Value::F32(f[r].to_bits()),
                (View::I(i), Some(crate::ir::ScalarType::Bool)) => {
                    crate::sem::Value::Bool(i[r] != 0)
                }
                (View::I(i), _) => crate::sem::Value::I64(i[r]),
            }
        };
        let group = crate::facts::group_values(fact, get);
        let shape = match &self.checked.graph().nodes[value.index()].op {
            crate::ir::Op::F32Index { table, .. } => Some(table.len() as u32),
            _ => None,
        };
        if crate::facts::holds(fact, get(*value), &group, shape) {
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
