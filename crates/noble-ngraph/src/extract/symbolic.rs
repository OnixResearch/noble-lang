//! D7 stages 2–5: symbolic stack execution, pruning and the independent
//! graph check.

use crate::identity::IdentityScheme;
/// A symbolic stack item (D7 `Σ`). Scalars are graph value ids; `dup` and
/// captures share the same id because values are immutable.
#[derive(Clone, Debug)]
enum Sym {
    Scalar(u32, crate::ir::ScalarType),
    Unit,
    Pair(alloc::rc::Rc<(Sym, Sym)>),
    /// A statically known Program and its checked type.
    Known(
        alloc::rc::Rc<Recipe>,
        alloc::rc::Rc<noble_kernel::types::Ty>,
    ),
    /// A statically built list (`nil`/`cons`); `ngraph-i64/0` has no index
    /// node, so a table may only be routed or dropped (`E-AGGREGATE`).
    Table(alloc::rc::Rc<noble_kernel::types::Ty>),
}

/// A statically known Program recipe. `compose` is O(1); expansion walks the
/// tree, so a shared recipe expanded twice is charged twice.
#[derive(Debug)]
enum Recipe {
    /// A quotation literal of body `body`.
    Quotation {
        body: usize,
        node: noble_kernel::untrusted::NodeId,
    },
    /// `quote`: pushes the captured value (same id; no copy).
    Capture(Sym),
    /// `compose`.
    Compose(alloc::rc::Rc<Recipe>, alloc::rc::Rc<Recipe>),
}

enum Task {
    Node {
        body: usize,
        node: noble_kernel::untrusted::NodeId,
    },
    Recipe(alloc::rc::Rc<Recipe>),
    Push(Sym),
    Leave,
    /// After a node (and its expansion) completes, `Σ` must match the node's
    /// checked `stack_out`.
    Check {
        body: usize,
        node: noble_kernel::untrusted::NodeId,
    },
}

struct Pending {
    op: crate::ir::Op,
    origin: Option<(crate::ir::SourceRef, u32)>,
}

/// Extraction counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    /// Work units charged (every processed source node and recipe step).
    pub work_units: u64,
    /// Nodes created before pruning (the D5 "expanded nodes" charge).
    pub expanded_nodes: u64,
    /// Deepest inline nesting reached.
    pub max_inline_depth: u64,
    /// Nodes removed by pruning.
    pub pruned_nodes: u64,
    /// The checker-side saturating named-call expansion bound (D7 step 5).
    pub static_expansion_bound: u64,
}

/// A checked extraction result.
#[derive(Debug)]
pub struct Extraction {
    /// The independently checked graph.
    pub checked: crate::check::CheckedGraph,
    /// Extraction counters.
    pub stats: Stats,
}

/// D7 stages 2–5 over an admitted subject.
///
/// # Errors
/// The first D6 refusal in stage order (`E-INTERFACE`, `E-EFFECT`,
/// `E-DYNAMIC-RUN`, `E-LIVE-SLOT`, `E-RECURSION`, `E-CONTROL`,
/// `E-AGGREGATE`, `E-REFLECT`, `E-PROGRAM-OUTPUT`, `E-PROFILE`,
/// `E-BUDGET-WORK`, `E-BUDGET-EXPANSION`, `E-BUDGET-INLINE-DEPTH`, or a
/// graph-checker refusal). No graph exists on refusal, so none is evaluated.
pub fn extract(
    ids: &dyn IdentityScheme,
    admitted: &super::admission::Admitted<'_>,
    budgets: &crate::check::Budgets,
) -> Result<Extraction, crate::diag::Diagnostic> {
    let mut m = Machine::new(admitted);
    let (inputs, outputs) = admitted.root_interface();
    let mut input_shapes = alloc::vec::Vec::with_capacity(inputs.len());
    for ty in inputs {
        let sym = m.input(ty)?;
        input_shapes.push(shape(ty)?);
        m.sigma.push(sym);
    }
    let root = &admitted.bodies[0];
    m.push_entries(0, &root.candidate.body);
    m.run()?;
    if m.sigma.len() != outputs.len() || !m.sigma.iter().zip(outputs).all(|(s, t)| matches(s, t)) {
        return Err(crate::diag::Diagnostic::new(
            crate::diag::Code::Interface,
            crate::diag::Site::Graph,
            "final symbolic stack differs from the checked root output",
        ));
    }
    let mut out_ids = alloc::vec::Vec::new();
    for s in &m.sigma {
        flatten(s, &mut out_ids)?;
    }
    let mut output_shapes = alloc::vec::Vec::with_capacity(outputs.len());
    for ty in outputs {
        output_shapes.push(shape(ty)?);
    }
    let static_bound = static_bound(admitted)?;
    if static_bound > admitted.budget.expanded_nodes {
        return Err(crate::diag::Diagnostic::budget(
            crate::diag::BudgetKind::Expansion,
            admitted.budget.expanded_nodes,
            static_bound,
        ));
    }
    let mut stats = Stats {
        work_units: m.work,
        expanded_nodes: m.nodes.len() as u64,
        max_inline_depth: m.max_depth,
        pruned_nodes: 0,
        static_expansion_bound: static_bound,
    };
    let graph = m.build(&out_ids, input_shapes, output_shapes, &mut stats);
    let checked = crate::check::check_with(ids, graph, budgets)?;
    Ok(Extraction { checked, stats })
}

struct Machine<'a, 's> {
    a: &'a super::admission::Admitted<'s>,
    sigma: alloc::vec::Vec<Sym>,
    tasks: alloc::vec::Vec<Task>,
    nodes: alloc::vec::Vec<Pending>,
    /// Expansion frames `(parent, site)`; frame 0 is the root body.
    frames: alloc::vec::Vec<(u32, crate::ir::SourceRef)>,
    current: u32,
    /// Active expansions: named identity (if any) and the frame to restore.
    active: alloc::vec::Vec<(Option<u64>, u32)>,
    work: u64,
    max_depth: u64,
    input_types: alloc::vec::Vec<crate::ir::ScalarType>,
}

impl<'a, 's> Machine<'a, 's> {
    fn new(a: &'a super::admission::Admitted<'s>) -> Self {
        Self {
            a,
            sigma: alloc::vec::Vec::new(),
            tasks: alloc::vec::Vec::new(),
            nodes: alloc::vec::Vec::new(),
            frames: alloc::vec![(
                0,
                crate::ir::SourceRef {
                    definition: None,
                    node: u32::MAX,
                },
            )],
            current: 0,
            active: alloc::vec::Vec::new(),
            work: 0,
            max_depth: 0,
            input_types: alloc::vec::Vec::new(),
        }
    }

    fn input(&mut self, ty: &noble_kernel::types::Ty) -> Result<Sym, crate::diag::Diagnostic> {
        match ty {
            noble_kernel::types::Ty::Pair(a, b) => {
                let a = self.input(a)?;
                let b = self.input(b)?;
                Ok(Sym::Pair(alloc::rc::Rc::new((a, b))))
            }
            noble_kernel::types::Ty::Unit => Ok(Sym::Unit),
            other => {
                let Some(t) = super::admission::scalar(other) else {
                    return Err(crate::diag::Diagnostic::new(
                        crate::diag::Code::Aggregate,
                        crate::diag::Site::Graph,
                        alloc::format!("input type {other:?}"),
                    ));
                };
                let k = u32::try_from(self.input_types.len()).unwrap_or(u32::MAX);
                self.input_types.push(t);
                let id = self.node(crate::ir::Op::Input(k), None)?;
                Ok(Sym::Scalar(id, t))
            }
        }
    }

    fn charge(&mut self) -> Result<(), crate::diag::Diagnostic> {
        self.work += 1;
        if self.work > self.a.budget.work_units {
            return Err(crate::diag::Diagnostic::budget(
                crate::diag::BudgetKind::Work,
                self.a.budget.work_units,
                self.work,
            ));
        }
        Ok(())
    }

    fn node(
        &mut self,
        op: crate::ir::Op,
        site: Option<crate::ir::SourceRef>,
    ) -> Result<u32, crate::diag::Diagnostic> {
        let observed = self.nodes.len() as u64 + 1;
        if observed > self.a.budget.expanded_nodes {
            return Err(crate::diag::Diagnostic::budget(
                crate::diag::BudgetKind::Expansion,
                self.a.budget.expanded_nodes,
                observed,
            ));
        }
        let id = u32::try_from(self.nodes.len()).unwrap_or(u32::MAX);
        self.nodes.push(Pending {
            op,
            origin: site.map(|s| (s, self.current)),
        });
        Ok(id)
    }

    fn push_entries(&mut self, body: usize, entries: &[noble_kernel::untrusted::NodeId]) {
        self.tasks
            .extend(entries.iter().rev().map(|&node| Task::Node { body, node }));
    }

    fn site(&self, body: usize, node: noble_kernel::untrusted::NodeId) -> crate::ir::SourceRef {
        crate::ir::SourceRef {
            definition: self.a.bodies[body].identity,
            node: node.0,
        }
    }

    fn enter(
        &mut self,
        site: crate::ir::SourceRef,
        identity: Option<u64>,
    ) -> Result<(), crate::diag::Diagnostic> {
        let depth = self.active.len() as u64 + 1;
        if depth > self.a.budget.inline_depth {
            return Err(crate::diag::Diagnostic::budget(
                crate::diag::BudgetKind::InlineDepth,
                self.a.budget.inline_depth,
                depth,
            ));
        }
        self.max_depth = self.max_depth.max(depth);
        self.frames.push((self.current, site));
        self.active.push((identity, self.current));
        self.current = u32::try_from(self.frames.len() - 1).unwrap_or(u32::MAX);
        self.tasks.push(Task::Leave);
        Ok(())
    }

    fn run(&mut self) -> Result<(), crate::diag::Diagnostic> {
        while let Some(task) = self.tasks.pop() {
            match task {
                Task::Node { body, node } => {
                    self.charge()?;
                    self.step(body, node)?;
                }
                Task::Recipe(r) => {
                    self.charge()?;
                    match &*r {
                        Recipe::Quotation { body, node } => {
                            let noble_kernel::untrusted::Node::Quotation { body: entries, .. } =
                                &self.a.bodies[*body].candidate.nodes[node.0 as usize]
                            else {
                                return Err(crate::diag::Diagnostic::new(
                                    crate::diag::Code::Interface,
                                    crate::diag::Site::Graph,
                                    "recipe is not a quotation",
                                ));
                            };
                            self.push_entries(*body, entries);
                        }
                        Recipe::Capture(s) => self.sigma.push(s.clone()),
                        Recipe::Compose(x, y) => {
                            self.tasks.push(Task::Recipe(y.clone()));
                            self.tasks.push(Task::Recipe(x.clone()));
                        }
                    }
                }
                Task::Push(s) => self.sigma.push(s),
                Task::Leave => {
                    if let Some((_, frame)) = self.active.pop() {
                        self.current = frame;
                    }
                }
                Task::Check { body, node } => {
                    let iface = self.interface(body, node)?;
                    if !suffix_matches(&self.sigma, &iface.stack_out) {
                        return Err(self.err(
                            crate::diag::Code::Interface,
                            body,
                            node,
                            "symbolic result differs from the checked derivation's stack_out",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn err(
        &self,
        code: crate::diag::Code,
        body: usize,
        node: noble_kernel::untrusted::NodeId,
        detail: impl Into<alloc::string::String>,
    ) -> crate::diag::Diagnostic {
        crate::diag::Diagnostic::new(
            code,
            crate::diag::Site::Source(self.site(body, node)),
            detail,
        )
    }

    fn interface(
        &self,
        body: usize,
        node: noble_kernel::untrusted::NodeId,
    ) -> Result<&'a noble_kernel::untrusted::Interface, crate::diag::Diagnostic> {
        self.a.bodies[body]
            .interfaces
            .get(node.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                self.err(
                    crate::diag::Code::Interface,
                    body,
                    node,
                    "missing or inconsistent checked derivation",
                )
            })
    }

    fn pop(
        &mut self,
        body: usize,
        node: noble_kernel::untrusted::NodeId,
    ) -> Result<Sym, crate::diag::Diagnostic> {
        self.sigma.pop().ok_or_else(|| {
            self.err(
                crate::diag::Code::Interface,
                body,
                node,
                "symbolic stack underflow",
            )
        })
    }

    fn pop_known(
        &mut self,
        body: usize,
        node: noble_kernel::untrusted::NodeId,
    ) -> Result<alloc::rc::Rc<Recipe>, crate::diag::Diagnostic> {
        match self.pop(body, node)? {
            Sym::Known(r, _) => Ok(r),
            _ => Err(self.err(
                crate::diag::Code::DynamicRun,
                body,
                node,
                "Program is not statically known",
            )),
        }
    }

    fn step(
        &mut self,
        body: usize,
        node: noble_kernel::untrusted::NodeId,
    ) -> Result<(), crate::diag::Diagnostic> {
        let iface = self.interface(body, node)?;
        if !iface.effects.is_empty() {
            return Err(self.err(
                crate::diag::Code::Effect,
                body,
                node,
                alloc::format!("nonempty effect bound {:?}", iface.effects),
            ));
        }
        if !suffix_matches(&self.sigma, &iface.stack_in) {
            return Err(self.err(
                crate::diag::Code::Interface,
                body,
                node,
                "symbolic stack differs from the checked stack_in",
            ));
        }
        let site = self.site(body, node);
        let out_ty = || {
            alloc::rc::Rc::new(
                iface
                    .stack_out
                    .last()
                    .cloned()
                    .unwrap_or(noble_kernel::types::Ty::Unit),
            )
        };
        self.tasks.push(Task::Check { body, node });
        match &self.a.bodies[body].candidate.nodes[node.0 as usize] {
            noble_kernel::untrusted::Node::Literal { lit, .. } => match lit {
                noble_kernel::untrusted::Lit::I64(v) => {
                    let id = self.node(
                        crate::ir::Op::Const(crate::ir::Literal::I64(*v)),
                        Some(site),
                    )?;
                    self.sigma.push(Sym::Scalar(id, crate::ir::ScalarType::I64));
                }
                noble_kernel::untrusted::Lit::Bool(v) => {
                    let id = self.node(
                        crate::ir::Op::Const(crate::ir::Literal::Bool(*v)),
                        Some(site),
                    )?;
                    self.sigma
                        .push(Sym::Scalar(id, crate::ir::ScalarType::Bool));
                }
                noble_kernel::untrusted::Lit::Unit => self.sigma.push(Sym::Unit),
                noble_kernel::untrusted::Lit::Text => {
                    return Err(self.err(crate::diag::Code::Aggregate, body, node, "Text literal"))
                }
            },
            noble_kernel::untrusted::Node::Quotation { .. } => {
                self.sigma.push(Sym::Known(
                    alloc::rc::Rc::new(Recipe::Quotation { body, node }),
                    out_ty(),
                ));
            }
            noble_kernel::untrusted::Node::SlotInvoke { .. } => {
                return Err(self.err(
                    crate::diag::Code::LiveSlot,
                    body,
                    node,
                    "live-slot dispatch",
                ));
            }
            noble_kernel::untrusted::Node::Invocation { def, .. } => {
                self.invoke(body, node, *def, site, out_ty)?
            }
        }
        Ok(())
    }

    fn invoke(
        &mut self,
        body: usize,
        node: noble_kernel::untrusted::NodeId,
        def: noble_kernel::contracts::Definition,
        site: crate::ir::SourceRef,
        out_ty: impl Fn() -> alloc::rc::Rc<noble_kernel::types::Ty>,
    ) -> Result<(), crate::diag::Diagnostic> {
        let Some(kind) = self.a.submission.environment.kind(def) else {
            return Err(self.err(
                crate::diag::Code::Interface,
                body,
                node,
                "unknown definition",
            ));
        };
        match kind {
            noble_kernel::contracts::Behavior::Dup => {
                let top = self.pop(body, node)?;
                self.sigma.push(top.clone());
                self.sigma.push(top);
            }
            noble_kernel::contracts::Behavior::Drop => {
                self.pop(body, node)?;
            }
            noble_kernel::contracts::Behavior::Swap => {
                let b = self.pop(body, node)?;
                let a = self.pop(body, node)?;
                self.sigma.push(b);
                self.sigma.push(a);
            }
            noble_kernel::contracts::Behavior::Dip => {
                let q = self.pop_known(body, node)?;
                let x = self.pop(body, node)?;
                self.tasks.push(Task::Push(x));
                self.enter(site, None)?;
                self.tasks.push(Task::Recipe(q));
            }
            noble_kernel::contracts::Behavior::Arith => {
                let b = self.pop(body, node)?;
                let a = self.pop(body, node)?;
                let (
                    Sym::Scalar(a, crate::ir::ScalarType::I64),
                    Sym::Scalar(b, crate::ir::ScalarType::I64),
                ) = (a, b)
                else {
                    return Err(self.err(
                        crate::diag::Code::Interface,
                        body,
                        node,
                        "arithmetic on non-I64 symbols",
                    ));
                };
                let (a, b) = (crate::ir::NodeId(a), crate::ir::NodeId(b));
                // K-NUM-01: `a` is the deeper operand.
                let op = match def.0 {
                    4 => crate::ir::Op::I64Add(a, b),
                    5 => crate::ir::Op::I64Sub(a, b),
                    6 => crate::ir::Op::I64Mul(a, b),
                    _ => {
                        return Err(self.err(
                            crate::diag::Code::Interface,
                            body,
                            node,
                            "unknown arithmetic row",
                        ))
                    }
                };
                let id = self.node(op, Some(site))?;
                self.sigma.push(Sym::Scalar(id, crate::ir::ScalarType::I64));
            }
            noble_kernel::contracts::Behavior::Equals => {
                let b = self.pop(body, node)?;
                let a = self.pop(body, node)?;
                match (a, b) {
                    (
                        Sym::Scalar(a, crate::ir::ScalarType::I64),
                        Sym::Scalar(b, crate::ir::ScalarType::I64),
                    ) => {
                        let id = self.node(
                            crate::ir::Op::I64Eq(crate::ir::NodeId(a), crate::ir::NodeId(b)),
                            Some(site),
                        )?;
                        self.sigma
                            .push(Sym::Scalar(id, crate::ir::ScalarType::Bool));
                    }
                    (Sym::Table(_), _) | (_, Sym::Table(_)) => {
                        return Err(self.err(
                            crate::diag::Code::Aggregate,
                            body,
                            node,
                            "a table reaches `=`",
                        ));
                    }
                    _ => {
                        return Err(self.err(
                            crate::diag::Code::Interface,
                            body,
                            node,
                            "`=` outside the i64.eq whitelist signature (I64 I64 -- Bool)",
                        ));
                    }
                }
            }
            noble_kernel::contracts::Behavior::Quote => {
                let v = self.pop(body, node)?;
                self.sigma
                    .push(Sym::Known(alloc::rc::Rc::new(Recipe::Capture(v)), out_ty()));
            }
            noble_kernel::contracts::Behavior::Compose => {
                let y = self.pop_known(body, node)?;
                let x = self.pop_known(body, node)?;
                self.sigma.push(Sym::Known(
                    alloc::rc::Rc::new(Recipe::Compose(x, y)),
                    out_ty(),
                ));
            }
            noble_kernel::contracts::Behavior::Run => {
                let q = self.pop_known(body, node)?;
                self.enter(site, None)?;
                self.tasks.push(Task::Recipe(q));
            }
            noble_kernel::contracts::Behavior::Named => {
                let Some(target) = self.a.definition_body(def) else {
                    return Err(self.err(
                        crate::diag::Code::Effect,
                        body,
                        node,
                        "host operation row",
                    ));
                };
                let identity = self.a.bodies[target].identity;
                // D7's expansion-stack rule, kept as defense in depth:
                // admission already refuses a closure whose identity call
                // graph (over every arena node, quotations included)
                // recurses, and the kernel refuses a cycle recorded in
                // `Env::deps`, so this is not expected to fire.
                if self
                    .active
                    .iter()
                    .any(|(id, _)| id.is_some() && *id == identity)
                {
                    return Err(self.err(
                        crate::diag::Code::Recursion,
                        body,
                        node,
                        alloc::format!(
                            "definition identity {identity:?} is already on the expansion stack"
                        ),
                    ));
                }
                self.enter(site, identity)?;
                let entries = &self.a.bodies[target].candidate.body;
                self.push_entries(target, entries);
            }
            noble_kernel::contracts::Behavior::Reflect => {
                return Err(self.err(crate::diag::Code::Reflect, body, node, "reflect"))
            }
            noble_kernel::contracts::Behavior::Unit => self.sigma.push(Sym::Unit),
            noble_kernel::contracts::Behavior::Pair => {
                let b = self.pop(body, node)?;
                let a = self.pop(body, node)?;
                self.sigma.push(Sym::Pair(alloc::rc::Rc::new((a, b))));
            }
            noble_kernel::contracts::Behavior::Unpair => match self.pop(body, node)? {
                Sym::Pair(p) => {
                    self.sigma.push(p.0.clone());
                    self.sigma.push(p.1.clone());
                }
                _ => {
                    return Err(self.err(
                        crate::diag::Code::Interface,
                        body,
                        node,
                        "unpair of a non-pair symbol",
                    ))
                }
            },
            noble_kernel::contracts::Behavior::Nil => self.sigma.push(Sym::Table(out_ty())),
            noble_kernel::contracts::Behavior::Cons => {
                self.pop(body, node)?;
                self.pop(body, node)?;
                self.sigma.push(Sym::Table(out_ty()));
            }
            noble_kernel::contracts::Behavior::Inl
            | noble_kernel::contracts::Behavior::Inr
            | noble_kernel::contracts::Behavior::Case
            | noble_kernel::contracts::Behavior::If
            | noble_kernel::contracts::Behavior::ListCase => {
                return Err(self.err(
                    crate::diag::Code::Control,
                    body,
                    node,
                    alloc::format!("{kind:?}"),
                ));
            }
            noble_kernel::contracts::Behavior::TestEmit
            | noble_kernel::contracts::Behavior::BoundEmit(_)
            | noble_kernel::contracts::Behavior::BoundClock(_) => {
                return Err(self.err(
                    crate::diag::Code::Effect,
                    body,
                    node,
                    alloc::format!("host operation {kind:?}"),
                ));
            }
            noble_kernel::contracts::Behavior::TextByte
            | noble_kernel::contracts::Behavior::CalculatorTextFromAscii
            | noble_kernel::contracts::Behavior::CalculatorTextLength
            | noble_kernel::contracts::Behavior::NominalNew(_)
            | noble_kernel::contracts::Behavior::NominalInto(_)
            | noble_kernel::contracts::Behavior::NominalLeft(_)
            | noble_kernel::contracts::Behavior::NominalRight(_)
            | noble_kernel::contracts::Behavior::NominalMatch(_)
            | noble_kernel::contracts::Behavior::GenericLeft(_)
            | noble_kernel::contracts::Behavior::GenericRight(_)
            | noble_kernel::contracts::Behavior::GenericMatch(_) => {
                return Err(self.err(
                    crate::diag::Code::Aggregate,
                    body,
                    node,
                    alloc::format!("{kind:?}"),
                ));
            }
            noble_kernel::contracts::Behavior::CalculatorIterate
            | noble_kernel::contracts::Behavior::CalculatorCompare
            | noble_kernel::contracts::Behavior::CalculatorDivRem
            | noble_kernel::contracts::Behavior::Reserved => {
                return Err(self.err(
                    crate::diag::Code::Profile,
                    body,
                    node,
                    alloc::format!("{kind:?} is outside ngraph-i64/0"),
                ));
            }
        }
        Ok(())
    }

    /// Prunes nodes that reach no output (input slots stay), renumbers in
    /// creation order and attaches extraction provenance.
    fn build(
        self,
        outputs: &[u32],
        input_shapes: alloc::vec::Vec<crate::ir::ShapeTree>,
        output_shapes: alloc::vec::Vec<crate::ir::ShapeTree>,
        stats: &mut Stats,
    ) -> crate::ir::Graph {
        let n = self.nodes.len();
        let mut live = alloc::vec![false; n];
        let mut work: alloc::vec::Vec<u32> = outputs.to_vec();
        while let Some(v) = work.pop() {
            let v = v as usize;
            if core::mem::replace(&mut live[v], true) {
                continue;
            }
            work.extend(self.nodes[v].op.operands().into_iter().map(|o| o.0));
        }
        let mut builder =
            crate::ir::GraphBuilder::new(crate::ir::SubProfile::I64, self.a.subject().clone());
        let mut remap = alloc::vec![crate::ir::NodeId(u32::MAX); n];
        for (i, p) in self.nodes.into_iter().enumerate() {
            if let crate::ir::Op::Input(k) = p.op {
                // Inputs are created first, in slot order.
                remap[i] = builder.input(self.input_types[k as usize]);
                continue;
            }
            if !live[i] {
                stats.pruned_nodes += 1;
                continue;
            }
            let op = remap_op(&p.op, &remap);
            let origin = p.origin.map(|(site, frame)| crate::ir::Origin {
                site,
                expansion: expansion(&self.frames, frame),
            });
            remap[i] = builder.push(op, origin);
        }
        for o in outputs {
            builder.output(remap[*o as usize]);
        }
        let flat = |s: &[crate::ir::ShapeTree]| s.iter().all(crate::ir::ShapeTree::is_scalar);
        if !flat(&input_shapes) || !flat(&output_shapes) {
            builder.shapes(Some(input_shapes), Some(output_shapes));
        }
        builder.finish()
    }
}

fn remap_op(op: &crate::ir::Op, remap: &[crate::ir::NodeId]) -> crate::ir::Op {
    let r = |x: &crate::ir::NodeId| remap[x.index()];
    match op {
        crate::ir::Op::I64Add(a, b) => crate::ir::Op::I64Add(r(a), r(b)),
        crate::ir::Op::I64Sub(a, b) => crate::ir::Op::I64Sub(r(a), r(b)),
        crate::ir::Op::I64Mul(a, b) => crate::ir::Op::I64Mul(r(a), r(b)),
        crate::ir::Op::I64Eq(a, b) => crate::ir::Op::I64Eq(r(a), r(b)),
        other => other.clone(),
    }
}

fn expansion(
    frames: &[(u32, crate::ir::SourceRef)],
    mut frame: u32,
) -> alloc::vec::Vec<crate::ir::SourceRef> {
    let mut out = alloc::vec::Vec::new();
    while frame != 0 {
        let (parent, site) = frames[frame as usize];
        out.push(site);
        frame = parent;
    }
    out.reverse();
    out
}

fn matches(s: &Sym, t: &noble_kernel::types::Ty) -> bool {
    match (s, t) {
        (Sym::Scalar(_, crate::ir::ScalarType::I64), noble_kernel::types::Ty::I64)
        | (Sym::Scalar(_, crate::ir::ScalarType::Bool), noble_kernel::types::Ty::Bool)
        | (Sym::Unit, noble_kernel::types::Ty::Unit) => true,
        (Sym::Pair(p), noble_kernel::types::Ty::Pair(a, b)) => matches(&p.0, a) && matches(&p.1, b),
        (Sym::Known(_, ty) | Sym::Table(ty), t) => **ty == *t,
        _ => false,
    }
}

fn suffix_matches(sigma: &[Sym], stack: &[noble_kernel::types::Ty]) -> bool {
    sigma.len() >= stack.len()
        && sigma[sigma.len() - stack.len()..]
            .iter()
            .zip(stack)
            .all(|(s, t)| matches(s, t))
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the flattened result is an append-only vector whose final length is not known in advance"
)]
fn flatten(s: &Sym, out: &mut alloc::vec::Vec<u32>) -> Result<(), crate::diag::Diagnostic> {
    match s {
        Sym::Scalar(id, _) => out.push(*id),
        Sym::Unit => {}
        Sym::Pair(p) => {
            flatten(&p.0, out)?;
            flatten(&p.1, out)?;
        }
        Sym::Known(..) => {
            return Err(crate::diag::Diagnostic::new(
                crate::diag::Code::ProgramOutput,
                crate::diag::Site::Graph,
                "Program-valued output",
            ));
        }
        Sym::Table(_) => {
            return Err(crate::diag::Diagnostic::new(
                crate::diag::Code::Aggregate,
                crate::diag::Site::Graph,
                "table-valued output",
            ));
        }
    }
    Ok(())
}

/// The shape tree of a root interface type. Admission has already refused
/// an item nesting more than `MAX_SHAPE_DEPTH` pairs, which bounds this
/// recursion; [`crate::ir::ShapeTree::pair`] refuses one again if it ever arrives.
fn shape(ty: &noble_kernel::types::Ty) -> Result<crate::ir::ShapeTree, crate::diag::Diagnostic> {
    match ty {
        noble_kernel::types::Ty::Unit => Ok(crate::ir::ShapeTree::UNIT),
        noble_kernel::types::Ty::Pair(a, b) => crate::ir::ShapeTree::pair(shape(a)?, shape(b)?)
            .map_err(|_| {
                crate::diag::Diagnostic::new(
                    crate::diag::Code::Aggregate,
                    crate::diag::Site::Graph,
                    "a root interface item nests too many pairs for a shape tree",
                )
            }),
        _ => Ok(crate::ir::ShapeTree::SCALAR),
    }
}

/// D7 step 5: the checker's saturating named-call expansion bound,
/// recomputed from the closure call graph (one entry per body; the root uses
/// an identity outside the closure).
fn static_bound(a: &super::admission::Admitted<'_>) -> Result<u64, crate::diag::Diagnostic> {
    let env = &a.submission.environment;
    let mut root_id = u64::MAX;
    while a.closure().binary_search(&root_id).is_ok() {
        root_id -= 1;
    }
    let mut entries: alloc::vec::Vec<crate::check::CallGraphEntry> = alloc::vec::Vec::new();
    for (i, body) in a.bodies.iter().enumerate() {
        let identity = match body.identity {
            None if i == 0 => root_id,
            Some(id) if a.closure().binary_search(&id).is_ok() => id,
            _ => continue,
        };
        let mut local = 0u64;
        let mut callees = alloc::vec::Vec::new();
        for node in &body.candidate.nodes {
            match node {
                noble_kernel::untrusted::Node::Literal {
                    lit:
                        noble_kernel::untrusted::Lit::I64(_) | noble_kernel::untrusted::Lit::Bool(_),
                    ..
                } => local += 1,
                noble_kernel::untrusted::Node::Invocation { def, .. } => match env.kind(*def) {
                    Some(
                        noble_kernel::contracts::Behavior::Arith
                        | noble_kernel::contracts::Behavior::Equals,
                    ) => local += 1,
                    Some(noble_kernel::contracts::Behavior::Named) => {
                        if let Some(t) = a.definition_body(*def) {
                            if let Some(id) = a.bodies[t].identity {
                                callees.push(id);
                            }
                        }
                    }
                    _ => {}
                },
                noble_kernel::untrusted::Node::Literal { .. }
                | noble_kernel::untrusted::Node::Quotation { .. }
                | noble_kernel::untrusted::Node::SlotInvoke { .. } => {}
            }
        }
        // Several specializations may share one identity: keep the largest.
        if let Some(e) = entries.iter_mut().find(|e| e.identity == identity) {
            if local + callees.len() as u64 > e.local_nodes + e.callees.len() as u64 {
                e.local_nodes = local;
                e.callees = callees;
            }
        } else {
            entries.push(crate::check::CallGraphEntry {
                identity,
                local_nodes: local,
                callees,
            });
        }
    }
    let inputs = a.root_interface().0.len() as u64;
    crate::check::expansion_bound(&entries, root_id)
        .map(|b| b.saturating_add(inputs))
        .ok_or_else(|| {
            crate::diag::Diagnostic::new(
                crate::diag::Code::Recursion,
                crate::diag::Site::Graph,
                "recursive closure call graph",
            )
        })
}
