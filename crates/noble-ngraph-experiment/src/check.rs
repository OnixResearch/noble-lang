//! The independent graph checker (D7 step 5).
//!
//! It re-derives node types from the D2 whitelist signatures and checks
//! topology (unknown nodes, forward references, cycles, dangling nodes),
//! interface slots, D3 payload rules (finite exact constants and
//! coefficients), D9 fact provenance (re-deriving `derived` claims), the facts
//! required by `step`/`select`/`index`, and the D5 post-normalization budgets.
//! It trusts nothing the producer recorded (node origins, counts, fact rules).
//!
//! The checker is ordinary Rust; its soundness is the open obligation NG-PO-B.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::diag::{BudgetKind, Code, Diagnostic, Malformed, Site};
use crate::facts::{Abs, AbsF32, AbsI64, Requirement};
use crate::ir::{
    AffineOperand, Digest, Fact, Graph, NodeId, Op, OutOfRange, Provenance, ScalarType, SlotKind,
    SubProfile,
};

/// D5 budgets with the `/0` policy defaults. A policy may lower a budget;
/// raising one requires a new profile policy revision (refused, `E-PROFILE`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budgets {
    /// Extraction work units (symbolic steps, including repeated expansions).
    pub work_units: u64,
    /// Expanded nodes, before pruning, CSE or fusion.
    pub expanded_nodes: u64,
    /// Inline depth (named call, `run`, `dip` nesting).
    pub inline_depth: u64,
    /// Live nodes after normalization.
    pub live_nodes: u64,
    /// Layers (critical path).
    pub layers: u64,
    /// Inputs and params.
    pub inputs: u64,
    /// Outputs.
    pub outputs: u64,
    /// Terms per `affine`.
    pub affine_terms: u64,
    /// Index table length.
    pub table_length: u64,
    /// Total index table entries.
    pub table_entries: u64,
    /// Facts.
    pub facts: u64,
    /// Retained graph bytes.
    pub graph_bytes: u64,
    /// Snapshot bytes.
    pub snapshot_bytes: u64,
    /// Forward scratch per evaluation (bytes).
    pub forward_scratch: u64,
    /// Backward scratch per evaluation (bytes).
    pub backward_scratch: u64,
    /// Optimizer steps per host call.
    pub optimizer_steps: u64,
}

impl Default for Budgets {
    fn default() -> Self {
        Self {
            work_units: 4_194_304,
            expanded_nodes: 262_144,
            inline_depth: 64,
            live_nodes: 262_144,
            layers: 16_384,
            inputs: 65_536,
            outputs: 65_536,
            affine_terms: 4_096,
            table_length: 65_536,
            table_entries: 1_048_576,
            facts: 262_144,
            graph_bytes: 32 * 1024 * 1024,
            snapshot_bytes: 4 * 1024 * 1024,
            forward_scratch: 64 * 1024 * 1024,
            backward_scratch: 128 * 1024 * 1024,
            optimizer_steps: 1_000_000,
        }
    }
}

impl Budgets {
    fn pairs(&self) -> [u64; 16] {
        [
            self.work_units,
            self.expanded_nodes,
            self.inline_depth,
            self.live_nodes,
            self.layers,
            self.inputs,
            self.outputs,
            self.affine_terms,
            self.table_length,
            self.table_entries,
            self.facts,
            self.graph_bytes,
            self.snapshot_bytes,
            self.forward_scratch,
            self.backward_scratch,
            self.optimizer_steps,
        ]
    }

    /// Refuses a policy that raises any budget above the `/0` default.
    ///
    /// # Errors
    /// `E-PROFILE` if any budget exceeds its default.
    pub fn validate_policy(&self) -> Result<(), Diagnostic> {
        let d = Self::default();
        if self.pairs().iter().zip(d.pairs()).all(|(a, b)| *a <= b) {
            Ok(())
        } else {
            Err(Diagnostic::new(
                Code::Profile,
                Site::Graph,
                "raising a /0 budget requires a new profile policy revision",
            ))
        }
    }
}

/// Charges `observed` against `limit`.
///
/// # Errors
/// `E-BUDGET-<kind>` with the limit and observed count.
pub const fn charge(kind: BudgetKind, limit: u64, observed: u64) -> Result<(), BudgetKind> {
    if observed > limit {
        Err(kind)
    } else {
        Ok(())
    }
}

fn budget(kind: BudgetKind, limit: u64, observed: u64) -> Result<(), Diagnostic> {
    charge(kind, limit, observed).map_err(|k| Diagnostic::budget(k, limit, observed))
}

/// An input/param fact validated before evaluation (`E-FACT-INPUT`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputFact {
    /// Fact record index in the graph.
    pub record: usize,
    /// Slot node.
    pub node: NodeId,
    /// Interface slot.
    pub slot: u32,
    /// Slot kind.
    pub kind: SlotKind,
    /// Fact.
    pub fact: Fact,
}

/// A graph accepted by the checker, with re-derived types, layers and facts.
#[derive(Clone, Debug)]
pub struct CheckedGraph {
    graph: Arc<Graph>,
    id: Digest,
    types: Vec<Option<ScalarType>>,
    layer: Vec<u32>,
    layers: u32,
    input_facts: Vec<InputFact>,
    abs: Vec<Abs>,
    slot_nodes: Vec<NodeId>,
    param_slots: Vec<u32>,
    param_layout: Digest,
}

impl CheckedGraph {
    /// The checked graph.
    #[must_use]
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Shared owner of the checked graph (retained, immutable; D12).
    #[must_use]
    pub fn graph_arc(&self) -> Arc<Graph> {
        Arc::clone(&self.graph)
    }

    /// `GraphLogicalId`.
    #[must_use]
    pub const fn id(&self) -> Digest {
        self.id
    }

    /// Re-derived type of node `n` (`None` for guard roots).
    #[must_use]
    pub fn ty(&self, n: NodeId) -> Option<ScalarType> {
        self.types[n.index()]
    }

    /// Dependency layer of node `n` (handwired depth rule: sources at 0).
    #[must_use]
    pub fn layer(&self, n: NodeId) -> u32 {
        self.layer[n.index()]
    }

    /// Number of layers (critical path length + 1).
    #[must_use]
    pub const fn layer_count(&self) -> u32 {
        self.layers
    }

    /// Input/param facts to validate before evaluation.
    #[must_use]
    pub fn input_facts(&self) -> &[InputFact] {
        &self.input_facts
    }

    /// The checker's abstract summary of node `n` at its definition.
    #[must_use]
    pub fn summary(&self, n: NodeId) -> Abs {
        self.abs[n.index()]
    }

    /// Node of interface slot `k`.
    #[must_use]
    pub fn slot_node(&self, k: u32) -> NodeId {
        self.slot_nodes[k as usize]
    }

    /// Ordered param slots (computed once at check).
    #[must_use]
    pub fn param_slots(&self) -> &[u32] {
        &self.param_slots
    }

    /// Param layout key (computed once at check).
    #[must_use]
    pub const fn param_layout(&self) -> Digest {
        self.param_layout
    }

    /// Live node count.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.nodes.len()
    }
}

/// Estimated retained bytes of a graph (charged against `graph_bytes`).
#[must_use]
pub fn retained_bytes(g: &Graph) -> u64 {
    let mut b = std::mem::size_of::<Graph>() as u64;
    b += (g.slots.len() * std::mem::size_of::<crate::ir::Slot>()) as u64;
    b += (g.outputs.len() * 4) as u64;
    for n in &g.nodes {
        b += std::mem::size_of::<crate::ir::Node>() as u64;
        b += match &n.op {
            Op::F32Affine { terms, .. } => (terms.len() * 12) as u64,
            Op::F32Index { table, .. } => (table.len() * 4) as u64,
            Op::Guard {
                fact: Fact::OneHot { group, .. },
                ..
            } => (group.len() * 4) as u64,
            _ => 0,
        };
    }
    b += (g.facts.len() * std::mem::size_of::<crate::ir::FactRecord>()) as u64;
    b
}

fn op_profile(op: &Op) -> Option<SubProfile> {
    match op {
        Op::Input(_) | Op::Param(_) | Op::Const(_) | Op::Guard { .. } => None,
        Op::I64Add(..) | Op::I64Sub(..) | Op::I64Mul(..) | Op::I64Eq(..) => Some(SubProfile::I64),
        _ => Some(SubProfile::F32),
    }
}

/// Checks `graph` under the `/0` default budgets.
///
/// # Errors
/// The first D6 refusal in stage order.
pub fn check(graph: Graph) -> Result<CheckedGraph, Diagnostic> {
    check_with(graph, &Budgets::default())
}

/// Checks `graph` under `budgets`. Stage order: policy, interface/size
/// budgets, references and topology, slots, types and payloads, dangling
/// nodes, facts, layers, retained bytes.
///
/// # Errors
/// The first D6 refusal in stage order.
#[allow(clippy::too_many_lines)]
pub fn check_with(graph: Graph, budgets: &Budgets) -> Result<CheckedGraph, Diagnostic> {
    budgets.validate_policy()?;
    let n = graph.nodes.len();
    budget(BudgetKind::LiveNodes, budgets.live_nodes, n as u64)?;
    budget(BudgetKind::Inputs, budgets.inputs, graph.slots.len() as u64)?;
    budget(
        BudgetKind::Outputs,
        budgets.outputs,
        graph.outputs.len() as u64,
    )?;
    budget(BudgetKind::Facts, budgets.facts, graph.facts.len() as u64)?;
    let mut table_entries = 0u64;
    for node in &graph.nodes {
        match &node.op {
            Op::F32Affine { terms, .. } => {
                budget(
                    BudgetKind::AffineTerms,
                    budgets.affine_terms,
                    terms.len() as u64,
                )?;
            }
            Op::F32Index { table, .. } => {
                budget(
                    BudgetKind::TableLength,
                    budgets.table_length,
                    table.len() as u64,
                )?;
                table_entries += table.len() as u64;
                budget(
                    BudgetKind::TableEntries,
                    budgets.table_entries,
                    table_entries,
                )?;
            }
            _ => {}
        }
    }
    budget(
        BudgetKind::GraphBytes,
        budgets.graph_bytes,
        retained_bytes(&graph),
    )?;

    check_topology(&graph)?;
    let slot_nodes = check_slots(&graph)?;
    let types = check_types(&graph)?;
    check_dangling(&graph)?;
    let (abs, input_facts) = check_facts(&graph, &types)?;
    let (layer, layers) = layering(&graph);
    budget(BudgetKind::Layers, budgets.layers, u64::from(layers))?;
    let id = graph.logical_id();
    let param_slots = graph.param_slots();
    let param_layout = crate::snapshot::layout_key(&graph);
    Ok(CheckedGraph {
        graph: Arc::new(graph),
        id,
        types,
        layer,
        layers,
        input_facts,
        abs,
        slot_nodes,
        param_slots,
        param_layout,
    })
}

fn check_topology(g: &Graph) -> Result<(), Diagnostic> {
    let n = g.nodes.len();
    let mut forward = None;
    for (i, node) in g.nodes.iter().enumerate() {
        let site = Site::Node(NodeId(i as u32));
        for o in node.op.operands() {
            if o.index() >= n {
                return Err(Diagnostic::malformed(
                    Malformed::UnknownNode,
                    site,
                    format!("operand %{} does not exist", o.0),
                ));
            }
            if o.index() >= i && forward.is_none() {
                forward = Some((i, o));
            }
            if g.nodes[o.index()].op.is_guard() {
                return Err(Diagnostic::malformed(
                    Malformed::GuardAsValue,
                    site,
                    "a guard root is not a value",
                ));
            }
        }
    }
    for (k, o) in g.outputs.iter().enumerate() {
        if o.index() >= n {
            return Err(Diagnostic::malformed(
                Malformed::UnknownNode,
                Site::Graph,
                format!("output {k} names missing node %{}", o.0),
            ));
        }
        if g.nodes[o.index()].op.is_guard() {
            return Err(Diagnostic::malformed(
                Malformed::GuardAsValue,
                Site::Node(*o),
                "a guard root is not an output",
            ));
        }
    }
    for (k, f) in g.facts.iter().enumerate() {
        let mut refs = vec![f.node];
        if let Fact::OneHot { group, .. } = &f.fact {
            refs.extend_from_slice(group);
        }
        if let Provenance::Guard(gd) = &f.provenance {
            refs.push(*gd);
        }
        if refs.iter().any(|r| r.index() >= n) {
            return Err(Diagnostic::malformed(
                Malformed::UnknownNode,
                Site::Fact(k),
                "fact names a missing node",
            ));
        }
    }
    if let Some((at, operand)) = forward {
        // Distinguish a genuine cycle from an acyclic forward reference.
        if let Some(c) = find_cycle(g) {
            return Err(Diagnostic::malformed(
                Malformed::Cycle,
                Site::Node(c),
                "dependency cycle",
            ));
        }
        return Err(Diagnostic::malformed(
            Malformed::ForwardReference,
            Site::Node(NodeId(at as u32)),
            format!("operand %{} is defined later", operand.0),
        ));
    }
    Ok(())
}

fn find_cycle(g: &Graph) -> Option<NodeId> {
    // Iterative three-color DFS.
    let n = g.nodes.len();
    let mut color = vec![0u8; n];
    let ops: Vec<Vec<NodeId>> = g.nodes.iter().map(|x| x.op.operands()).collect();
    for start in 0..n {
        if color[start] != 0 {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        color[start] = 1;
        while let Some((v, next)) = stack.last_mut() {
            let v = *v;
            if let Some(&w) = ops[v].get(*next) {
                *next += 1;
                match color[w.index()] {
                    0 => {
                        color[w.index()] = 1;
                        stack.push((w.index(), 0));
                    }
                    1 => return Some(w),
                    _ => {}
                }
            } else {
                color[v] = 2;
                stack.pop();
            }
        }
    }
    None
}

fn check_slots(g: &Graph) -> Result<Vec<NodeId>, Diagnostic> {
    let mut seen: Vec<Option<NodeId>> = vec![None; g.slots.len()];
    for (i, node) in g.nodes.iter().enumerate() {
        let id = NodeId(i as u32);
        let (k, kind) = match node.op {
            Op::Input(k) => (k, SlotKind::Input),
            Op::Param(k) => (k, SlotKind::Param),
            _ => continue,
        };
        let Some(slot) = g.slots.get(k as usize) else {
            return Err(Diagnostic::malformed(
                Malformed::SlotMismatch,
                Site::Node(id),
                format!("slot {k} is not declared"),
            ));
        };
        if slot.kind != kind {
            return Err(Diagnostic::malformed(
                Malformed::SlotMismatch,
                Site::Node(id),
                format!("slot {k} kind differs from its node"),
            ));
        }
        if let Some(prev) = seen[k as usize] {
            return Err(Diagnostic::malformed(
                Malformed::DuplicateSlot,
                Site::Node(id),
                format!("slot {k} already defined by %{}", prev.0),
            ));
        }
        seen[k as usize] = Some(id);
    }
    let mut out = Vec::with_capacity(seen.len());
    for (k, (s, slot)) in seen.iter().zip(&g.slots).enumerate() {
        let Some(id) = s else {
            return Err(Diagnostic::malformed(
                Malformed::SlotMismatch,
                Site::Slot(k as u32),
                "slot has no node",
            ));
        };
        if !g.profile.admits(slot.ty) {
            return Err(Diagnostic::new(
                Code::TypeMix,
                Site::Slot(k as u32),
                format!("{} slot in {}", slot.ty.name(), g.profile.name()),
            ));
        }
        if slot.kind == SlotKind::Param && g.profile != SubProfile::F32 {
            return Err(Diagnostic::malformed(
                Malformed::SlotMismatch,
                Site::Slot(k as u32),
                "param slots exist only in ngraph-f32/0",
            ));
        }
        out.push(*id);
    }
    Ok(out)
}

fn finite_bits(bits: u32) -> bool {
    f32::from_bits(bits).is_finite()
}

#[allow(clippy::too_many_lines)]
fn check_types(g: &Graph) -> Result<Vec<Option<ScalarType>>, Diagnostic> {
    let mut types: Vec<Option<ScalarType>> = Vec::with_capacity(g.nodes.len());
    for (i, node) in g.nodes.iter().enumerate() {
        let site = Site::Node(NodeId(i as u32));
        if let Some(p) = op_profile(&node.op) {
            if p != g.profile {
                return Err(Diagnostic::new(
                    Code::TypeMix,
                    site,
                    format!("{} in {}", node.op.name(), g.profile.name()),
                ));
            }
        }
        let want = |o: NodeId, ty: ScalarType| -> Result<(), Diagnostic> {
            match types[o.index()] {
                Some(t) if t == ty => Ok(()),
                Some(t) if !g.profile.admits(t) || !g.profile.admits(ty) => Err(Diagnostic::new(
                    Code::TypeMix,
                    site.clone(),
                    format!("operand %{} is {}", o.0, t.name()),
                )),
                _ => Err(Diagnostic::malformed(
                    Malformed::TypeError,
                    site.clone(),
                    format!("operand %{} is not {}", o.0, ty.name()),
                )),
            }
        };
        let ty = match &node.op {
            Op::Input(k) | Op::Param(k) => Some(g.slots[*k as usize].ty),
            Op::Const(l) => {
                if !g.profile.admits(l.ty()) {
                    return Err(Diagnostic::new(
                        Code::TypeMix,
                        site,
                        format!("{} literal in {}", l.ty().name(), g.profile.name()),
                    ));
                }
                if let crate::ir::Literal::F32(b) = l {
                    if !finite_bits(*b) {
                        return Err(Diagnostic::new(
                            Code::LiteralNonfinite,
                            site,
                            "non-finite f32 constant",
                        ));
                    }
                }
                Some(l.ty())
            }
            Op::I64Add(a, b) | Op::I64Sub(a, b) | Op::I64Mul(a, b) => {
                want(*a, ScalarType::I64)?;
                want(*b, ScalarType::I64)?;
                Some(ScalarType::I64)
            }
            Op::I64Eq(a, b) => {
                want(*a, ScalarType::I64)?;
                want(*b, ScalarType::I64)?;
                Some(ScalarType::Bool)
            }
            Op::F32Add(a, b) | Op::F32Sub(a, b) | Op::F32Mul(a, b) => {
                want(*a, ScalarType::F32)?;
                want(*b, ScalarType::F32)?;
                Some(ScalarType::F32)
            }
            Op::F32Affine { bias, terms } => {
                if !finite_bits(*bias) || terms.iter().any(|t| !finite_bits(t.coefficient)) {
                    return Err(Diagnostic::new(
                        Code::LiteralNonfinite,
                        site,
                        "non-finite affine bias or coefficient",
                    ));
                }
                for t in terms {
                    if let AffineOperand::Node(x) = t.operand {
                        want(x, ScalarType::F32)?;
                    }
                }
                Some(ScalarType::F32)
            }
            Op::F32Relu(x) | Op::F32Step(x) => {
                want(*x, ScalarType::F32)?;
                Some(ScalarType::F32)
            }
            Op::F32Reglu { left, gate } => {
                want(*left, ScalarType::F32)?;
                want(*gate, ScalarType::F32)?;
                Some(ScalarType::F32)
            }
            Op::F32Select { bit, off, on } => {
                for x in [bit, off, on] {
                    want(*x, ScalarType::F32)?;
                }
                Some(ScalarType::F32)
            }
            Op::F32Index { index, table, .. } => {
                if table.is_empty() {
                    return Err(Diagnostic::malformed(
                        Malformed::Payload,
                        site,
                        "index table is empty",
                    ));
                }
                want(*index, ScalarType::F32)?;
                for x in table {
                    want(*x, ScalarType::F32)?;
                }
                Some(ScalarType::F32)
            }
            Op::Guard { fact, value } => {
                let t = types[value.index()].unwrap_or(ScalarType::Bool);
                if !crate::facts::payload_ok(fact, t) {
                    return Err(Diagnostic::malformed(
                        Malformed::Payload,
                        site,
                        format!("{} fact payload for {}", fact.kind_name(), t.name()),
                    ));
                }
                if let Fact::OneHot { group, .. } = fact {
                    if group.first() != Some(value) {
                        return Err(Diagnostic::malformed(
                            Malformed::Payload,
                            site,
                            "one-hot fact must be attached to its first member",
                        ));
                    }
                    for m in group {
                        want(*m, t)?;
                    }
                }
                None
            }
        };
        types.push(ty);
    }
    Ok(types)
}

fn check_dangling(g: &Graph) -> Result<(), Diagnostic> {
    let n = g.nodes.len();
    let mut live = vec![false; n];
    let mut work: Vec<NodeId> = g.outputs.clone();
    for (i, node) in g.nodes.iter().enumerate() {
        if node.op.is_guard() {
            work.push(NodeId(i as u32));
        }
    }
    while let Some(v) = work.pop() {
        if std::mem::replace(&mut live[v.index()], true) {
            continue;
        }
        work.extend(g.nodes[v.index()].op.operands());
    }
    for (i, node) in g.nodes.iter().enumerate() {
        if !live[i] && !matches!(node.op, Op::Input(_) | Op::Param(_)) {
            return Err(Diagnostic::malformed(
                Malformed::DanglingNode,
                Site::Node(NodeId(i as u32)),
                "node reaches no output and no guard",
            ));
        }
    }
    Ok(())
}

fn unchecked(site: Site, fact: Fact, why: &str) -> Diagnostic {
    Diagnostic::new(Code::FactUnchecked, site, why).with_fact(fact)
}

#[allow(clippy::too_many_lines)]
fn check_facts(
    g: &Graph,
    types: &[Option<ScalarType>],
) -> Result<(Vec<Abs>, Vec<InputFact>), Diagnostic> {
    // Provenance of each record (before any use).
    let mut input_facts = Vec::new();
    let mut derived_claims: BTreeMap<usize, Vec<(usize, Fact)>> = BTreeMap::new();
    for (k, rec) in g.facts.iter().enumerate() {
        let site = Site::Fact(k);
        let ty = types[rec.node.index()];
        let Some(ty) = ty else {
            return Err(unchecked(site, rec.fact.clone(), "fact on a guard root"));
        };
        if !crate::facts::payload_ok(&rec.fact, ty) {
            return Err(Diagnostic::malformed(
                Malformed::Payload,
                site,
                "fact payload does not fit the value type",
            ));
        }
        match &rec.provenance {
            Provenance::Asserted => {
                return Err(unchecked(
                    site,
                    rec.fact.clone(),
                    "caller assertion without derived, input-validated or guard provenance",
                ));
            }
            Provenance::Evidence(_) => {
                return Err(unchecked(
                    site,
                    rec.fact.clone(),
                    "evidence facts are excluded from Phase B (approval P5)",
                ));
            }
            Provenance::InputValidated => {
                let (slot, kind) = match g.nodes[rec.node.index()].op {
                    Op::Input(s) => (s, SlotKind::Input),
                    Op::Param(s) => (s, SlotKind::Param),
                    _ => {
                        return Err(unchecked(
                            site,
                            rec.fact.clone(),
                            "input-validated provenance on a non-interface node",
                        ));
                    }
                };
                if let Fact::OneHot { group, .. } = &rec.fact {
                    if group.first() != Some(&rec.node)
                        || group
                            .iter()
                            .any(|m| !matches!(g.nodes[m.index()].op, Op::Input(_) | Op::Param(_)))
                    {
                        return Err(unchecked(
                            site,
                            rec.fact.clone(),
                            "input-validated one-hot group must be interface slots led by the record node",
                        ));
                    }
                }
                input_facts.push(InputFact {
                    record: k,
                    node: rec.node,
                    slot,
                    kind,
                    fact: rec.fact.clone(),
                });
            }
            Provenance::Guard(gd) => match &g.nodes[gd.index()].op {
                Op::Guard { fact, value } if *value == rec.node && *fact == rec.fact => {}
                _ => {
                    return Err(unchecked(
                        site,
                        rec.fact.clone(),
                        "guard provenance does not name a matching guard",
                    ));
                }
            },
            Provenance::Derived(_) => {
                derived_claims
                    .entry(rec.node.index())
                    .or_default()
                    .push((k, rec.fact.clone()));
            }
        }
    }

    // Abstract interpretation in canonical order. `cur` holds the summary of
    // each value as refined by input validation and earlier guards.
    let n = g.nodes.len();
    let mut cur: Vec<Abs> = Vec::with_capacity(n);
    let mut at_def: Vec<Abs> = Vec::with_capacity(n);
    let f = |cur: &[Abs], x: NodeId| -> AbsF32 {
        match cur[x.index()] {
            Abs::F32(a) => a,
            _ => AbsF32::UNKNOWN,
        }
    };
    let iv = |cur: &[Abs], x: NodeId| -> AbsI64 {
        match cur[x.index()] {
            Abs::I64(a) => a,
            _ => AbsI64::UNKNOWN,
        }
    };
    for (i, node) in g.nodes.iter().enumerate() {
        let id = NodeId(i as u32);
        let need = |cur: &[Abs], x: NodeId, r: Requirement| -> Result<(), Diagnostic> {
            if r.satisfied_by(&f(cur, x)) {
                Ok(())
            } else {
                Err(unchecked(
                    Site::Node(id),
                    r.fact(),
                    &format!(
                        "{} operand %{} lacks a checked {} fact",
                        node.op.name(),
                        x.0,
                        r.fact().kind_name()
                    ),
                ))
            }
        };
        let a = match &node.op {
            Op::Input(_) | Op::Param(_) => {
                let mut a = match types[i] {
                    Some(ScalarType::F32) => Abs::F32(AbsF32::UNKNOWN),
                    Some(ScalarType::I64) => Abs::I64(AbsI64::UNKNOWN),
                    _ => Abs::Bool,
                };
                for inf in &input_facts {
                    if inf.node == id {
                        a = a.refine(&inf.fact);
                    }
                    if let Fact::OneHot { group, .. } = &inf.fact {
                        if group.contains(&id) {
                            a = a.refine(&inf.fact);
                        }
                    }
                }
                a
            }
            Op::Const(l) => match l {
                crate::ir::Literal::F32(b) => Abs::F32(AbsF32::exact(f32::from_bits(*b))),
                crate::ir::Literal::I64(v) => Abs::I64(AbsI64 { lo: *v, hi: *v }),
                crate::ir::Literal::Bool(_) => Abs::Bool,
            },
            Op::I64Add(a, b) => Abs::I64(AbsI64::radd(iv(&cur, *a), iv(&cur, *b))),
            Op::I64Sub(a, b) => Abs::I64(AbsI64::rsub(iv(&cur, *a), iv(&cur, *b))),
            Op::I64Mul(a, b) => Abs::I64(AbsI64::rmul(iv(&cur, *a), iv(&cur, *b))),
            Op::I64Eq(..) => Abs::Bool,
            Op::F32Add(a, b) => Abs::F32(AbsF32::radd(f(&cur, *a), f(&cur, *b))),
            Op::F32Sub(a, b) => Abs::F32(AbsF32::rsub(f(&cur, *a), f(&cur, *b))),
            Op::F32Mul(a, b) => Abs::F32(AbsF32::rmul(f(&cur, *a), f(&cur, *b))),
            Op::F32Affine { bias, terms } => {
                let mut v = AbsF32::exact(f32::from_bits(*bias));
                for t in terms {
                    let x = match t.operand {
                        AffineOperand::One => AbsF32::exact(1.0),
                        AffineOperand::Node(x) => f(&cur, x),
                    };
                    let p = AbsF32::rmul(AbsF32::exact(f32::from_bits(t.coefficient)), x);
                    v = AbsF32::radd(v, p);
                }
                Abs::F32(v)
            }
            Op::F32Relu(x) => Abs::F32(AbsF32::relu(f(&cur, *x))),
            Op::F32Reglu { left, gate } => {
                Abs::F32(AbsF32::rmul(f(&cur, *left), AbsF32::relu(f(&cur, *gate))))
            }
            Op::F32Step(s) => {
                need(&cur, *s, Requirement::UnitRange)?;
                Abs::F32(AbsF32::step())
            }
            Op::F32Select { bit, off, on } => {
                need(&cur, *bit, Requirement::Bit)?;
                Abs::F32(AbsF32::hull(f(&cur, *off), f(&cur, *on)))
            }
            Op::F32Index {
                index,
                table,
                below,
                above,
            } => {
                need(&cur, *index, Requirement::Index)?;
                let mut h = f(&cur, table[0]);
                for t in &table[1..] {
                    h = AbsF32::hull(h, f(&cur, *t));
                }
                if *below == OutOfRange::Zero || *above == OutOfRange::Zero {
                    h = AbsF32::hull(h, AbsF32::exact(0.0));
                }
                Abs::F32(h)
            }
            Op::Guard { fact, value } => {
                cur[value.index()] = cur[value.index()].refine(fact);
                if let Fact::OneHot { group, .. } = fact {
                    for m in group {
                        cur[m.index()] = cur[m.index()].refine(fact);
                    }
                }
                Abs::None
            }
        };
        cur.push(a);
        at_def.push(a);
        if let Some(claims) = derived_claims.get(&i) {
            for (k, fact) in claims {
                let structural = matches!((fact, &node.op), (Fact::Shape(s), Op::F32Index { table, .. }) if *s as usize == table.len());
                if !structural && !a.implies(fact) {
                    return Err(unchecked(
                        Site::Fact(*k),
                        fact.clone(),
                        "the checker's transfer rules do not derive this fact",
                    ));
                }
            }
        }
    }
    Ok((at_def, input_facts))
}

/// Layers by handwired's depth rule: sources (inputs, params, constants) at 0,
/// otherwise `1 + max(operand layers)`. Returns per-node layers and the count.
fn layering(g: &Graph) -> (Vec<u32>, u32) {
    let mut layer = vec![0u32; g.nodes.len()];
    let mut max = 0u32;
    for (i, node) in g.nodes.iter().enumerate() {
        let l = node
            .op
            .operands()
            .iter()
            .map(|o| layer[o.index()] + 1)
            .max()
            .unwrap_or(0);
        layer[i] = l;
        max = max.max(l);
    }
    (layer, if g.nodes.is_empty() { 0 } else { max + 1 })
}

/// One closure definition for the named-call expansion bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallGraphEntry {
    /// Exact definition identity.
    pub identity: u64,
    /// Nodes created by the body excluding named calls.
    pub local_nodes: u64,
    /// Named callees, one entry per call site (repeats count).
    pub callees: Vec<u64>,
}

/// Saturating upper bound on expanded nodes of `root` over the closure call
/// graph, memoized per definition (D7 step 5; does not trust the extractor's
/// count). `None` if the call graph is recursive or names a missing identity.
#[must_use]
pub fn expansion_bound(closure: &[CallGraphEntry], root: u64) -> Option<u64> {
    let by_id: BTreeMap<u64, &CallGraphEntry> = closure.iter().map(|e| (e.identity, e)).collect();
    let mut memo: BTreeMap<u64, u64> = BTreeMap::new();
    let mut on_stack: BTreeMap<u64, ()> = BTreeMap::new();
    // Iterative post-order.
    let mut stack: Vec<(u64, usize)> = vec![(root, 0)];
    on_stack.insert(root, ());
    while let Some((id, next)) = stack.last_mut() {
        let id = *id;
        let e = by_id.get(&id)?;
        if let Some(&c) = e.callees.get(*next) {
            *next += 1;
            if memo.contains_key(&c) {
                continue;
            }
            if on_stack.contains_key(&c) {
                return None;
            }
            on_stack.insert(c, ());
            stack.push((c, 0));
        } else {
            let mut total = e.local_nodes;
            for c in &e.callees {
                total = total.saturating_add(*memo.get(c)?);
            }
            memo.insert(id, total);
            on_stack.remove(&id);
            stack.pop();
        }
    }
    memo.get(&root).copied()
}
