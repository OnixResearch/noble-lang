//! D7 stages 2–5: symbolic stack execution, pruning and the independent
//! graph check.

use std::rc::Rc;

use noble_kernel::contracts::{Behavior, Definition};
use noble_kernel::types::Ty;
use noble_kernel::untrusted::{Interface, Lit, Node, NodeId as SrcNode};

use crate::check::{expansion_bound, CallGraphEntry, CheckedGraph};
use crate::diag::{BudgetKind, Code, Diagnostic, Site};
use crate::ir::{
    GraphBuilder, Literal, NodeId, Op, Origin, ScalarType, ShapeTree, SourceRef, SubProfile,
};

use super::admission::{scalar, Admitted};

/// A symbolic stack item (D7 `Σ`). Scalars are graph value ids; `dup` and
/// captures share the same id because values are immutable.
#[derive(Clone, Debug)]
enum Sym {
    Scalar(u32, ScalarType),
    Unit,
    Pair(Rc<(Sym, Sym)>),
    /// A statically known Program and its checked type.
    Known(Rc<Recipe>, Rc<Ty>),
    /// A statically built list (`nil`/`cons`); `ngraph-i64/0` has no index
    /// node, so a table may only be routed or dropped (`E-AGGREGATE`).
    Table(Rc<Ty>),
}

/// A statically known Program recipe. `compose` is O(1); expansion walks the
/// tree, so a shared recipe expanded twice is charged twice.
#[derive(Debug)]
enum Recipe {
    /// A quotation literal of body `body`.
    Quotation { body: usize, node: SrcNode },
    /// `quote`: pushes the captured value (same id; no copy).
    Capture(Sym),
    /// `compose`.
    Compose(Rc<Recipe>, Rc<Recipe>),
}

enum Task {
    Node {
        body: usize,
        node: SrcNode,
    },
    Recipe(Rc<Recipe>),
    Push(Sym),
    Leave,
    /// After a node (and its expansion) completes, `Σ` must match the node's
    /// checked `stack_out`.
    Check {
        body: usize,
        node: SrcNode,
    },
}

struct Pending {
    op: Op,
    origin: Option<(SourceRef, u32)>,
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
    pub checked: CheckedGraph,
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
pub fn extract(admitted: &Admitted<'_>) -> Result<Extraction, Diagnostic> {
    let mut m = Machine::new(admitted);
    let (inputs, outputs) = admitted.root_interface();
    let mut input_shapes = Vec::with_capacity(inputs.len());
    for ty in inputs {
        let sym = m.input(ty)?;
        input_shapes.push(shape(ty));
        m.sigma.push(sym);
    }
    let root = &admitted.bodies[0];
    m.push_entries(0, &root.candidate.body);
    m.run()?;
    if m.sigma.len() != outputs.len() || !m.sigma.iter().zip(outputs).all(|(s, t)| matches(s, t)) {
        return Err(Diagnostic::new(
            Code::Interface,
            Site::Graph,
            "final symbolic stack differs from the checked root output",
        ));
    }
    let mut out_ids = Vec::new();
    for s in &m.sigma {
        flatten(s, &mut out_ids)?;
    }
    let output_shapes: Vec<ShapeTree> = outputs.iter().map(shape).collect();
    let static_bound = static_bound(admitted)?;
    if static_bound > admitted.budget.expanded_nodes {
        return Err(Diagnostic::budget(
            BudgetKind::Expansion,
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
    let checked = crate::check::check(graph)?;
    Ok(Extraction { checked, stats })
}

struct Machine<'a, 's> {
    a: &'a Admitted<'s>,
    sigma: Vec<Sym>,
    tasks: Vec<Task>,
    nodes: Vec<Pending>,
    /// Expansion frames `(parent, site)`; frame 0 is the root body.
    frames: Vec<(u32, SourceRef)>,
    current: u32,
    /// Active expansions: named identity (if any) and the frame to restore.
    active: Vec<(Option<u64>, u32)>,
    work: u64,
    max_depth: u64,
    input_types: Vec<ScalarType>,
}

impl<'a, 's> Machine<'a, 's> {
    fn new(a: &'a Admitted<'s>) -> Self {
        Self {
            a,
            sigma: Vec::new(),
            tasks: Vec::new(),
            nodes: Vec::new(),
            frames: vec![(
                0,
                SourceRef {
                    definition: None,
                    node: u32::MAX,
                },
            )],
            current: 0,
            active: Vec::new(),
            work: 0,
            max_depth: 0,
            input_types: Vec::new(),
        }
    }

    fn input(&mut self, ty: &Ty) -> Result<Sym, Diagnostic> {
        match ty {
            Ty::Pair(a, b) => {
                let a = self.input(a)?;
                let b = self.input(b)?;
                Ok(Sym::Pair(Rc::new((a, b))))
            }
            Ty::Unit => Ok(Sym::Unit),
            other => {
                let Some(t) = scalar(other) else {
                    return Err(Diagnostic::new(
                        Code::Aggregate,
                        Site::Graph,
                        format!("input type {other:?}"),
                    ));
                };
                let k = u32::try_from(self.input_types.len()).unwrap_or(u32::MAX);
                self.input_types.push(t);
                let id = self.node(Op::Input(k), None)?;
                Ok(Sym::Scalar(id, t))
            }
        }
    }

    fn charge(&mut self) -> Result<(), Diagnostic> {
        self.work += 1;
        if self.work > self.a.budget.work_units {
            return Err(Diagnostic::budget(
                BudgetKind::Work,
                self.a.budget.work_units,
                self.work,
            ));
        }
        Ok(())
    }

    fn node(&mut self, op: Op, site: Option<SourceRef>) -> Result<u32, Diagnostic> {
        let observed = self.nodes.len() as u64 + 1;
        if observed > self.a.budget.expanded_nodes {
            return Err(Diagnostic::budget(
                BudgetKind::Expansion,
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

    fn push_entries(&mut self, body: usize, entries: &[SrcNode]) {
        self.tasks
            .extend(entries.iter().rev().map(|&node| Task::Node { body, node }));
    }

    fn site(&self, body: usize, node: SrcNode) -> SourceRef {
        SourceRef {
            definition: self.a.bodies[body].identity,
            node: node.0,
        }
    }

    fn enter(&mut self, site: SourceRef, identity: Option<u64>) -> Result<(), Diagnostic> {
        let depth = self.active.len() as u64 + 1;
        if depth > self.a.budget.inline_depth {
            return Err(Diagnostic::budget(
                BudgetKind::InlineDepth,
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

    fn run(&mut self) -> Result<(), Diagnostic> {
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
                            let Node::Quotation { body: entries, .. } =
                                &self.a.bodies[*body].candidate.nodes[node.0 as usize]
                            else {
                                return Err(Diagnostic::new(
                                    Code::Interface,
                                    Site::Graph,
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
                            Code::Interface,
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

    fn err(&self, code: Code, body: usize, node: SrcNode, detail: impl Into<String>) -> Diagnostic {
        Diagnostic::new(code, Site::Source(self.site(body, node)), detail)
    }

    fn interface(&self, body: usize, node: SrcNode) -> Result<&'a Interface, Diagnostic> {
        self.a.bodies[body]
            .interfaces
            .get(node.0 as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                self.err(
                    Code::Interface,
                    body,
                    node,
                    "missing or inconsistent checked derivation",
                )
            })
    }

    fn pop(&mut self, body: usize, node: SrcNode) -> Result<Sym, Diagnostic> {
        self.sigma
            .pop()
            .ok_or_else(|| self.err(Code::Interface, body, node, "symbolic stack underflow"))
    }

    fn pop_known(&mut self, body: usize, node: SrcNode) -> Result<Rc<Recipe>, Diagnostic> {
        match self.pop(body, node)? {
            Sym::Known(r, _) => Ok(r),
            _ => Err(self.err(
                Code::DynamicRun,
                body,
                node,
                "Program is not statically known",
            )),
        }
    }

    fn step(&mut self, body: usize, node: SrcNode) -> Result<(), Diagnostic> {
        let iface = self.interface(body, node)?;
        if !iface.effects.is_empty() {
            return Err(self.err(
                Code::Effect,
                body,
                node,
                format!("nonempty effect bound {:?}", iface.effects),
            ));
        }
        if !suffix_matches(&self.sigma, &iface.stack_in) {
            return Err(self.err(
                Code::Interface,
                body,
                node,
                "symbolic stack differs from the checked stack_in",
            ));
        }
        let site = self.site(body, node);
        let out_ty = || Rc::new(iface.stack_out.last().cloned().unwrap_or(Ty::Unit));
        self.tasks.push(Task::Check { body, node });
        match &self.a.bodies[body].candidate.nodes[node.0 as usize] {
            Node::Literal { lit, .. } => match lit {
                Lit::I64(v) => {
                    let id = self.node(Op::Const(Literal::I64(*v)), Some(site))?;
                    self.sigma.push(Sym::Scalar(id, ScalarType::I64));
                }
                Lit::Bool(v) => {
                    let id = self.node(Op::Const(Literal::Bool(*v)), Some(site))?;
                    self.sigma.push(Sym::Scalar(id, ScalarType::Bool));
                }
                Lit::Unit => self.sigma.push(Sym::Unit),
                Lit::Text => return Err(self.err(Code::Aggregate, body, node, "Text literal")),
            },
            Node::Quotation { .. } => {
                self.sigma.push(Sym::Known(
                    Rc::new(Recipe::Quotation { body, node }),
                    out_ty(),
                ));
            }
            Node::SlotInvoke { .. } => {
                return Err(self.err(Code::LiveSlot, body, node, "live-slot dispatch"));
            }
            Node::Invocation { def, .. } => self.invoke(body, node, *def, site, out_ty)?,
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn invoke(
        &mut self,
        body: usize,
        node: SrcNode,
        def: Definition,
        site: SourceRef,
        out_ty: impl Fn() -> Rc<Ty>,
    ) -> Result<(), Diagnostic> {
        let Some(kind) = self.a.submission.environment.kind(def) else {
            return Err(self.err(Code::Interface, body, node, "unknown definition"));
        };
        match kind {
            Behavior::Dup => {
                let top = self.pop(body, node)?;
                self.sigma.push(top.clone());
                self.sigma.push(top);
            }
            Behavior::Drop => {
                self.pop(body, node)?;
            }
            Behavior::Swap => {
                let b = self.pop(body, node)?;
                let a = self.pop(body, node)?;
                self.sigma.push(b);
                self.sigma.push(a);
            }
            Behavior::Dip => {
                let q = self.pop_known(body, node)?;
                let x = self.pop(body, node)?;
                self.tasks.push(Task::Push(x));
                self.enter(site, None)?;
                self.tasks.push(Task::Recipe(q));
            }
            Behavior::Arith => {
                let b = self.pop(body, node)?;
                let a = self.pop(body, node)?;
                let (Sym::Scalar(a, ScalarType::I64), Sym::Scalar(b, ScalarType::I64)) = (a, b)
                else {
                    return Err(self.err(
                        Code::Interface,
                        body,
                        node,
                        "arithmetic on non-I64 symbols",
                    ));
                };
                let (a, b) = (NodeId(a), NodeId(b));
                // K-NUM-01: `a` is the deeper operand.
                let op = match def.0 {
                    4 => Op::I64Add(a, b),
                    5 => Op::I64Sub(a, b),
                    6 => Op::I64Mul(a, b),
                    _ => {
                        return Err(self.err(Code::Interface, body, node, "unknown arithmetic row"))
                    }
                };
                let id = self.node(op, Some(site))?;
                self.sigma.push(Sym::Scalar(id, ScalarType::I64));
            }
            Behavior::Equals => {
                let b = self.pop(body, node)?;
                let a = self.pop(body, node)?;
                match (a, b) {
                    (Sym::Scalar(a, ScalarType::I64), Sym::Scalar(b, ScalarType::I64)) => {
                        let id = self.node(Op::I64Eq(NodeId(a), NodeId(b)), Some(site))?;
                        self.sigma.push(Sym::Scalar(id, ScalarType::Bool));
                    }
                    (Sym::Table(_), _) | (_, Sym::Table(_)) => {
                        return Err(self.err(Code::Aggregate, body, node, "a table reaches `=`"));
                    }
                    _ => {
                        return Err(self.err(
                            Code::Interface,
                            body,
                            node,
                            "`=` outside the i64.eq whitelist signature (I64 I64 -- Bool)",
                        ));
                    }
                }
            }
            Behavior::Quote => {
                let v = self.pop(body, node)?;
                self.sigma
                    .push(Sym::Known(Rc::new(Recipe::Capture(v)), out_ty()));
            }
            Behavior::Compose => {
                let y = self.pop_known(body, node)?;
                let x = self.pop_known(body, node)?;
                self.sigma
                    .push(Sym::Known(Rc::new(Recipe::Compose(x, y)), out_ty()));
            }
            Behavior::Run => {
                let q = self.pop_known(body, node)?;
                self.enter(site, None)?;
                self.tasks.push(Task::Recipe(q));
            }
            Behavior::Named => {
                let Some(target) = self.a.definition_body(def) else {
                    return Err(self.err(Code::Effect, body, node, "host operation row"));
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
                        Code::Recursion,
                        body,
                        node,
                        format!(
                            "definition identity {identity:?} is already on the expansion stack"
                        ),
                    ));
                }
                self.enter(site, identity)?;
                let entries = &self.a.bodies[target].candidate.body;
                self.push_entries(target, entries);
            }
            Behavior::Reflect => return Err(self.err(Code::Reflect, body, node, "reflect")),
            Behavior::Unit => self.sigma.push(Sym::Unit),
            Behavior::Pair => {
                let b = self.pop(body, node)?;
                let a = self.pop(body, node)?;
                self.sigma.push(Sym::Pair(Rc::new((a, b))));
            }
            Behavior::Unpair => match self.pop(body, node)? {
                Sym::Pair(p) => {
                    self.sigma.push(p.0.clone());
                    self.sigma.push(p.1.clone());
                }
                _ => {
                    return Err(self.err(
                        Code::Interface,
                        body,
                        node,
                        "unpair of a non-pair symbol",
                    ))
                }
            },
            Behavior::Nil => self.sigma.push(Sym::Table(out_ty())),
            Behavior::Cons => {
                self.pop(body, node)?;
                self.pop(body, node)?;
                self.sigma.push(Sym::Table(out_ty()));
            }
            Behavior::Inl | Behavior::Inr | Behavior::Case | Behavior::If | Behavior::ListCase => {
                return Err(self.err(Code::Control, body, node, format!("{kind:?}")));
            }
            Behavior::TestEmit | Behavior::BoundEmit(_) | Behavior::BoundClock(_) => {
                return Err(self.err(Code::Effect, body, node, format!("host operation {kind:?}")));
            }
            Behavior::TextByte
            | Behavior::CalculatorTextFromAscii
            | Behavior::CalculatorTextLength
            | Behavior::NominalNew(_)
            | Behavior::NominalInto(_)
            | Behavior::NominalLeft(_)
            | Behavior::NominalRight(_)
            | Behavior::NominalMatch(_)
            | Behavior::GenericLeft(_)
            | Behavior::GenericRight(_)
            | Behavior::GenericMatch(_) => {
                return Err(self.err(Code::Aggregate, body, node, format!("{kind:?}")));
            }
            Behavior::CalculatorIterate
            | Behavior::CalculatorCompare
            | Behavior::CalculatorDivRem
            | Behavior::Reserved => {
                return Err(self.err(
                    Code::Profile,
                    body,
                    node,
                    format!("{kind:?} is outside ngraph-i64/0"),
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
        input_shapes: Vec<ShapeTree>,
        output_shapes: Vec<ShapeTree>,
        stats: &mut Stats,
    ) -> crate::ir::Graph {
        let n = self.nodes.len();
        let mut live = vec![false; n];
        let mut work: Vec<u32> = outputs.to_vec();
        while let Some(v) = work.pop() {
            let v = v as usize;
            if std::mem::replace(&mut live[v], true) {
                continue;
            }
            work.extend(self.nodes[v].op.operands().into_iter().map(|o| o.0));
        }
        let mut builder = GraphBuilder::new(SubProfile::I64, self.a.subject().clone());
        let mut remap = vec![NodeId(u32::MAX); n];
        for (i, p) in self.nodes.into_iter().enumerate() {
            if let Op::Input(k) = p.op {
                // Inputs are created first, in slot order.
                remap[i] = builder.input(self.input_types[k as usize]);
                continue;
            }
            if !live[i] {
                stats.pruned_nodes += 1;
                continue;
            }
            let op = remap_op(&p.op, &remap);
            let origin = p.origin.map(|(site, frame)| Origin {
                site,
                expansion: expansion(&self.frames, frame),
            });
            remap[i] = builder.push(op, origin);
        }
        for o in outputs {
            builder.output(remap[*o as usize]);
        }
        let flat = |s: &[ShapeTree]| s.iter().all(|t| matches!(t, ShapeTree::Scalar));
        if !flat(&input_shapes) || !flat(&output_shapes) {
            builder.shapes(Some(input_shapes), Some(output_shapes));
        }
        builder.finish()
    }
}

fn remap_op(op: &Op, remap: &[NodeId]) -> Op {
    let r = |x: &NodeId| remap[x.index()];
    match op {
        Op::I64Add(a, b) => Op::I64Add(r(a), r(b)),
        Op::I64Sub(a, b) => Op::I64Sub(r(a), r(b)),
        Op::I64Mul(a, b) => Op::I64Mul(r(a), r(b)),
        Op::I64Eq(a, b) => Op::I64Eq(r(a), r(b)),
        other => other.clone(),
    }
}

fn expansion(frames: &[(u32, SourceRef)], mut frame: u32) -> Vec<SourceRef> {
    let mut out = Vec::new();
    while frame != 0 {
        let (parent, site) = frames[frame as usize];
        out.push(site);
        frame = parent;
    }
    out.reverse();
    out
}

fn matches(s: &Sym, t: &Ty) -> bool {
    match (s, t) {
        (Sym::Scalar(_, ScalarType::I64), Ty::I64)
        | (Sym::Scalar(_, ScalarType::Bool), Ty::Bool)
        | (Sym::Unit, Ty::Unit) => true,
        (Sym::Pair(p), Ty::Pair(a, b)) => matches(&p.0, a) && matches(&p.1, b),
        (Sym::Known(_, ty) | Sym::Table(ty), t) => **ty == *t,
        _ => false,
    }
}

fn suffix_matches(sigma: &[Sym], stack: &[Ty]) -> bool {
    sigma.len() >= stack.len()
        && sigma[sigma.len() - stack.len()..]
            .iter()
            .zip(stack)
            .all(|(s, t)| matches(s, t))
}

fn flatten(s: &Sym, out: &mut Vec<u32>) -> Result<(), Diagnostic> {
    match s {
        Sym::Scalar(id, _) => out.push(*id),
        Sym::Unit => {}
        Sym::Pair(p) => {
            flatten(&p.0, out)?;
            flatten(&p.1, out)?;
        }
        Sym::Known(..) => {
            return Err(Diagnostic::new(
                Code::ProgramOutput,
                Site::Graph,
                "Program-valued output",
            ));
        }
        Sym::Table(_) => {
            return Err(Diagnostic::new(
                Code::Aggregate,
                Site::Graph,
                "table-valued output",
            ));
        }
    }
    Ok(())
}

fn shape(ty: &Ty) -> ShapeTree {
    match ty {
        Ty::Unit => ShapeTree::Unit,
        Ty::Pair(a, b) => ShapeTree::Pair(Box::new(shape(a)), Box::new(shape(b))),
        _ => ShapeTree::Scalar,
    }
}

/// D7 step 5: the checker's saturating named-call expansion bound,
/// recomputed from the closure call graph (one entry per body; the root uses
/// an identity outside the closure).
fn static_bound(a: &Admitted<'_>) -> Result<u64, Diagnostic> {
    let env = &a.submission.environment;
    let mut root_id = u64::MAX;
    while a.closure().binary_search(&root_id).is_ok() {
        root_id -= 1;
    }
    let mut entries: Vec<CallGraphEntry> = Vec::new();
    for (i, body) in a.bodies.iter().enumerate() {
        let identity = match body.identity {
            None if i == 0 => root_id,
            Some(id) if a.closure().binary_search(&id).is_ok() => id,
            _ => continue,
        };
        let mut local = 0u64;
        let mut callees = Vec::new();
        for node in &body.candidate.nodes {
            match node {
                Node::Literal {
                    lit: Lit::I64(_) | Lit::Bool(_),
                    ..
                } => local += 1,
                Node::Invocation { def, .. } => match env.kind(*def) {
                    Some(Behavior::Arith | Behavior::Equals) => local += 1,
                    Some(Behavior::Named) => {
                        if let Some(t) = a.definition_body(*def) {
                            if let Some(id) = a.bodies[t].identity {
                                callees.push(id);
                            }
                        }
                    }
                    _ => {}
                },
                Node::Literal { .. } | Node::Quotation { .. } | Node::SlotInvoke { .. } => {}
            }
        }
        // Several specializations may share one identity: keep the largest.
        if let Some(e) = entries.iter_mut().find(|e| e.identity == identity) {
            if local + callees.len() as u64 > e.local_nodes + e.callees.len() as u64 {
                e.local_nodes = local;
                e.callees = callees;
            }
        } else {
            entries.push(CallGraphEntry {
                identity,
                local_nodes: local,
                callees,
            });
        }
    }
    let inputs = a.root_interface().0.len() as u64;
    expansion_bound(&entries, root_id)
        .map(|b| b.saturating_add(inputs))
        .ok_or_else(|| {
            Diagnostic::new(Code::Recursion, Site::Graph, "recursive closure call graph")
        })
}
