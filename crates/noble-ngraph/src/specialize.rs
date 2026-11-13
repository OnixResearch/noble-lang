//! Bounded static-input specialization for checked `ngraph-i64/0` graphs
//! (issue #32; design `partial-evaluation-v1`, D1–D11).
//!
//! The transformation follows the foundation model
//! `proofs/foundation/NobleFoundation/Specialize.lean`:
//!
//! 1. a **static map** gives one optional `I64` per ordered input leaf; a
//!    static leaf becomes a constant and the remaining leaves are renumbered
//!    by [`rank`] — the model's `rank`/`merge` representation (D2);
//! 2. a node is **folded** only when every operand is statically known, and
//!    only for wrapping `I64` addition, subtraction, multiplication and
//!    equality; nothing is reassociated, commuted or shared (D3);
//! 3. nodes that reach no output are **pruned** and the residual is
//!    renumbered densely, then independently re-checked by [`crate::check`]
//!    (D4);
//! 4. the residual is a **new** subject and graph identity with an explicit
//!    derivation record (D5, D11);
//! 5. residual creation is bounded by the `ngraph-pe/0` caps
//!    [`crate::check::Budgets::residual_nodes`] and
//!    [`crate::check::Budgets::variants`] (D7).
//!
//! [`emit_source`] renders a residual as `let`-based Noble source and
//! [`readmit`] independently re-accepts that source through the ordinary
//! Declared-Text-v1 frontend, the kernel and the extractor, so a residual is a
//! Program on the ordinary path before anything is published (D9).
//!
//! Claims and non-claims: `specialize_sound` is a model theorem about graph
//! values. This module is an implementation with finite differential tests; it
//! does **not** claim Rust/model correspondence, Program refinement, or an
//! equal budget, fuel or work relation between an original and its residual
//! (H-LIMIT-03). Folding `I64Eq` is an extension the model does not cover.

use crate::identity::IdentityScheme;
use crate::ir::{
    Digest, Graph, Literal, NodeId, Op, ScalarType, Slot, SlotKind, Subject, SubjectKind,
};

/// Accounting revision of every cap this module charges and every count it
/// reports (`ngraph-pe/0`, D7).
pub const ACCOUNTING_REVISION: &str = "ngraph-pe/0";

/// One specialization request.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    /// One optional `I64` per ordered input leaf (input slots in slot order).
    /// A length other than the input-leaf count is `E-INTERFACE`.
    pub static_inputs: &'a [Option<i64>],
    /// The `/0` budgets: the two `ngraph-pe/0` caps (a request may only lower
    /// them) and the budgets the residual re-check charges.
    pub budgets: &'a crate::check::Budgets,
}

/// What one specialization did. Counts are charges under
/// [`ACCOUNTING_REVISION`]; they are not a claim that the two sides consume
/// equal work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// [`ACCOUNTING_REVISION`].
    pub accounting: &'static str,
    /// `GraphLogicalId` of the checked original.
    pub original_graph: Digest,
    /// `GraphLogicalId` of the checked residual.
    pub residual_graph: Digest,
    /// The accepted static map, indexed by input leaf.
    pub static_inputs: alloc::vec::Vec<Option<i64>>,
    /// Residual interface input types, in slot order.
    pub dynamic_inputs: alloc::vec::Vec<ScalarType>,
    /// Nodes of the original graph.
    pub original_nodes: u64,
    /// Nodes whose operation became a constant, including every substituted
    /// static input leaf.
    pub folded_nodes: u64,
    /// Nodes removed because they reach no output.
    pub pruned_nodes: u64,
    /// Live nodes of the residual.
    pub residual_nodes: u64,
    /// Residual variants produced by this request (always 1 in v1).
    pub variants: u64,
}

impl Report {
    /// Input leaves that were supplied statically.
    #[must_use]
    pub fn static_leaves(&self) -> u64 {
        self.static_inputs
            .iter()
            .filter(|entry| entry.is_some())
            .count() as u64
    }

    /// Input leaves that remain dynamic.
    #[must_use]
    pub fn dynamic_leaves(&self) -> u64 {
        self.dynamic_inputs.len() as u64
    }
}

/// A checked residual graph and its derivation record.
#[derive(Clone, Debug)]
pub struct Residual {
    /// The independently re-checked residual.
    pub checked: crate::check::CheckedGraph,
    /// Derivation record.
    pub report: Report,
}

impl Residual {
    /// The residual graph.
    #[must_use]
    pub fn graph(&self) -> &Graph {
        self.checked.graph()
    }

    /// Input values of the original that this residual's `dynamic` values
    /// stand for, in original input-leaf order (the model's `merge`).
    ///
    /// Defined for an all-`I64` input interface, which is the v1 slice.
    #[must_use]
    pub fn merge(&self, dynamic: &[i64]) -> Option<alloc::vec::Vec<crate::sem::Value>> {
        merge(&self.report.static_inputs, dynamic)
    }
}

/// The model's `merge`: reconstruct the original input values from the static
/// map and the residual's dynamic input values, in input-leaf order.
///
/// `None` when `dynamic` does not have exactly one value per `None` entry.
/// Only an all-`I64` input interface can be reconstructed this way: the static
/// map is `I64`-valued, so a `Bool` leaf is dynamic (D2).
#[must_use]
pub fn merge(
    static_inputs: &[Option<i64>],
    dynamic: &[i64],
) -> Option<alloc::vec::Vec<crate::sem::Value>> {
    let expected = static_inputs.iter().filter(|entry| entry.is_none()).count();
    if dynamic.len() != expected {
        return None;
    }
    let mut next = 0usize;
    let mut values = alloc::vec::Vec::with_capacity(static_inputs.len());
    for entry in static_inputs {
        let value = match entry {
            Some(value) => *value,
            None => {
                let value = *dynamic.get(next)?;
                next += 1;
                value
            }
        };
        values.push(crate::sem::Value::I64(value));
    }
    Some(values)
}

/// The number of dynamic input leaves strictly below `slot` (the model's
/// `rank`), given the static map indexed by input-leaf position.
#[must_use]
pub fn rank(static_inputs: &[Option<i64>], slot: usize) -> Option<u32> {
    let mut rank = 0u32;
    for entry in static_inputs.get(..slot)? {
        if entry.is_none() {
            rank = rank.checked_add(1)?;
        }
    }
    Some(rank)
}

fn refuse(
    code: crate::diag::Code,
    detail: impl Into<alloc::string::String>,
) -> crate::diag::Diagnostic {
    crate::diag::Diagnostic::new(code, crate::diag::Site::Graph, detail)
}

fn at(
    code: crate::diag::Code,
    site: crate::diag::Site,
    detail: impl Into<alloc::string::String>,
) -> crate::diag::Diagnostic {
    crate::diag::Diagnostic::new(code, site, detail)
}

/// A statically known value of one node (D3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Known {
    I64(i64),
    Bool(bool),
}

impl Known {
    const fn i64(self) -> Option<i64> {
        match self {
            Self::I64(value) => Some(value),
            Self::Bool(_) => None,
        }
    }

    const fn as_literal(self) -> Literal {
        match self {
            Self::I64(value) => Literal::I64(value),
            Self::Bool(value) => Literal::Bool(value),
        }
    }
}

/// The static value of a literal, if the literal is statically known.
const fn known_literal(literal: Literal) -> Option<Known> {
    match literal {
        Literal::I64(value) => Some(Known::I64(value)),
        Literal::Bool(value) => Some(Known::Bool(value)),
        Literal::F32(_) => None,
    }
}

/// The statically known result of `op` when its operands are `known`,
/// following D3's whitelist exactly.
fn static_result(op: &Op, known: &[Option<Known>]) -> Option<Known> {
    match op {
        Op::I64Add(first, second) | Op::I64Sub(first, second) | Op::I64Mul(first, second) => {
            let x = known.get(first.index())?.as_ref()?.i64()?;
            let y = known.get(second.index())?.as_ref()?.i64()?;
            Some(Known::I64(match op {
                Op::I64Add(..) => x.wrapping_add(y),
                Op::I64Sub(..) => x.wrapping_sub(y),
                _ => x.wrapping_mul(y),
            }))
        }
        Op::I64Eq(first, second) => {
            let x = known.get(first.index())?.as_ref()?.i64()?;
            let y = known.get(second.index())?.as_ref()?.i64()?;
            Some(Known::Bool(x == y))
        }
        _ => None,
    }
}

/// Refuses a graph outside the `ngraph-pe/0` subject slice (D1).
fn admit_slice(checked: &crate::check::CheckedGraph) -> Result<(), crate::diag::Diagnostic> {
    let graph = checked.graph();
    if graph.profile != crate::ir::SubProfile::I64 {
        return Err(refuse(
            crate::diag::Code::Profile,
            alloc::format!(
                "static-input specialization admits ngraph-i64/0 graphs; this graph is {}",
                graph.profile.name()
            ),
        ));
    }
    for (k, slot) in graph.slots.iter().enumerate() {
        if slot.kind == SlotKind::Param {
            return Err(refuse(
                crate::diag::Code::Profile,
                alloc::format!("param slot {k} is outside the ngraph-pe/0 slice"),
            ));
        }
    }
    if graph.input_shape.is_some() || graph.output_shape.is_some() {
        return Err(refuse(
            crate::diag::Code::Aggregate,
            "an interface shape tree is outside the ngraph-pe/0 slice; v1 admits flat scalar interfaces only",
        ));
    }
    if !graph.facts.is_empty() {
        return Err(refuse(
            crate::diag::Code::Profile,
            alloc::format!(
                "{} fact record(s) are outside the ngraph-pe/0 slice",
                graph.facts.len()
            ),
        ));
    }
    for (i, node) in graph.nodes.iter().enumerate() {
        match node.op {
            Op::Input(_) | Op::Const(_) | Op::I64Add(..) | Op::I64Sub(..) | Op::I64Mul(..)
            | Op::I64Eq(..) => {}
            _ => {
                return Err(at(
                    crate::diag::Code::Profile,
                    crate::diag::Site::Node(NodeId(i as u32)),
                    alloc::format!(
                        "{} is outside the ngraph-pe/0 slice: a guard or failing node is an observation the residual cannot preserve",
                        node.op.name()
                    ),
                ))
            }
        }
    }
    Ok(())
}

/// Maps a slot index to its position among the input leaves.
fn leaf_positions(graph: &Graph) -> alloc::vec::Vec<Option<usize>> {
    let mut positions = alloc::vec::Vec::with_capacity(graph.slots.len());
    let mut next = 0usize;
    for slot in &graph.slots {
        if slot.kind == SlotKind::Input {
            positions.push(Some(next));
            next += 1;
        } else {
            positions.push(None);
        }
    }
    positions
}

/// Rewrites `op`'s operands through `map`.
fn renumber(op: &Op, map: &[u32]) -> Op {
    let mapped = |id: NodeId| NodeId(map[id.index()]);
    match op {
        Op::Input(k) => Op::Input(*k),
        Op::Const(literal) => Op::Const(*literal),
        Op::I64Add(first, second) => Op::I64Add(mapped(*first), mapped(*second)),
        Op::I64Sub(first, second) => Op::I64Sub(mapped(*first), mapped(*second)),
        Op::I64Mul(first, second) => Op::I64Mul(mapped(*first), mapped(*second)),
        Op::I64Eq(first, second) => Op::I64Eq(mapped(*first), mapped(*second)),
        // `admit_slice` refused every other operation before this pass.
        other => other.clone(),
    }
}

/// Specializes `checked` under `request.static_inputs` (D1–D7, D11).
///
/// # Errors
/// `E-PROFILE` for a raised cap or a subject outside the slice, `E-INTERFACE`
/// for a static map of the wrong arity or a static `Bool` leaf,
/// `E-BUDGET-RESIDUAL-NODES` / `E-BUDGET-VARIANTS` for the caps, and any
/// refusal of the independent residual re-check.
pub fn specialize(
    ids: &dyn IdentityScheme,
    checked: &crate::check::CheckedGraph,
    request: &Request<'_>,
) -> Result<Residual, crate::diag::Diagnostic> {
    request.budgets.validate_policy()?;
    admit_slice(checked)?;
    let graph = checked.graph();
    let positions = leaf_positions(graph);
    let leaves = positions.iter().filter(|p| p.is_some()).count();
    if request.static_inputs.len() != leaves {
        return Err(at(
            crate::diag::Code::Interface,
            crate::diag::Site::Graph,
            alloc::format!(
                "the static map has {} entr(y/ies) for {leaves} input leaf/leaves",
                request.static_inputs.len()
            ),
        ));
    }
    if request.budgets.variants < 1 {
        return Err(crate::diag::Diagnostic::budget(
            crate::diag::BudgetKind::Variants,
            request.budgets.variants,
            1,
        ));
    }
    for (slot, position) in positions.iter().enumerate() {
        let Some(position) = position else { continue };
        if request.static_inputs[*position].is_some() && graph.slots[slot].ty != ScalarType::I64 {
            return Err(at(
                crate::diag::Code::Interface,
                crate::diag::Site::Slot(slot as u32),
                "the static map is I64-valued; a Bool input leaf cannot be static in ngraph-pe/0",
            ));
        }
    }
    // The model's `rank` for every slot, and the residual's slots.
    let mut rank_of: alloc::vec::Vec<Option<u32>> =
        alloc::vec::Vec::with_capacity(graph.slots.len());
    let mut slots: alloc::vec::Vec<Slot> = alloc::vec::Vec::new();
    let mut next_rank = 0u32;
    for (slot, position) in positions.iter().enumerate() {
        match position {
            None => rank_of.push(None),
            Some(position) => {
                if request.static_inputs[*position].is_some() {
                    rank_of.push(None);
                } else {
                    rank_of.push(Some(next_rank));
                    slots.push(Slot {
                        kind: SlotKind::Input,
                        ty: graph.slots[slot].ty,
                    });
                    next_rank = next_rank.saturating_add(1);
                }
            }
        }
    }
    let original_nodes = graph.nodes.len();
    if original_nodes as u64 > request.budgets.residual_nodes {
        return Err(crate::diag::Diagnostic::budget(
            crate::diag::BudgetKind::ResidualNodes,
            request.budgets.residual_nodes,
            original_nodes as u64,
        ));
    }
    // Substitution and folding, index-preserving (D2, D3).
    let mut known: alloc::vec::Vec<Option<Known>> = alloc::vec![None; original_nodes];
    let mut built: alloc::vec::Vec<Op> = alloc::vec::Vec::with_capacity(original_nodes);
    let mut folded = 0u64;
    for (index, node) in graph.nodes.iter().enumerate() {
        let mut value = None;
        let op = match &node.op {
            Op::Input(k) => {
                let position = positions
                    .get(*k as usize)
                    .copied()
                    .flatten()
                    .ok_or_else(|| {
                        at(
                            crate::diag::Code::Interface,
                            crate::diag::Site::Node(NodeId(index as u32)),
                            alloc::format!("input node names undeclared slot {k}"),
                        )
                    })?;
                match request.static_inputs[position] {
                    Some(v) => {
                        value = Some(Known::I64(v));
                        folded = folded.saturating_add(1);
                        Op::Const(Literal::I64(v))
                    }
                    None => Op::Input(rank_of[*k as usize].ok_or_else(|| {
                        at(
                            crate::diag::Code::Interface,
                            crate::diag::Site::Node(NodeId(index as u32)),
                            alloc::format!("slot {k} has no dynamic rank"),
                        )
                    })?),
                }
            }
            Op::Const(literal) => {
                value = known_literal(*literal);
                Op::Const(*literal)
            }
            other => match static_result(other, &known) {
                Some(folded_value) => {
                    value = Some(folded_value);
                    folded = folded.saturating_add(1);
                    Op::Const(folded_value.as_literal())
                }
                None => other.clone(),
            },
        };
        known[index] = value;
        built.push(op);
    }
    // Pruning: keep what the outputs reach, in canonical order (D4).
    let mut keep = alloc::vec![false; original_nodes];
    let mut pending: alloc::vec::Vec<u32> = graph.outputs.iter().map(|o| o.0).collect();
    while let Some(index) = pending.pop() {
        let slot = match keep.get_mut(index as usize) {
            Some(slot) => slot,
            None => {
                return Err(at(
                    crate::diag::Code::Interface,
                    crate::diag::Site::Graph,
                    "an output names a node that does not exist",
                ))
            }
        };
        if *slot {
            continue;
        }
        *slot = true;
        pending.extend(built[index as usize].operands().iter().map(|id| id.0));
    }
    // Renumber densely in canonical order. Operand ids of a kept node are
    // always kept (they are what made it reachable), so the map is complete
    // before any node is rewritten.
    let mut map = alloc::vec![u32::MAX; original_nodes];
    let mut next = 0u32;
    for (index, kept) in keep.iter().enumerate() {
        if *kept {
            map[index] = next;
            next = next.saturating_add(1);
        }
    }
    let mut nodes = alloc::vec::Vec::with_capacity(next as usize);
    for (index, op) in built.iter().enumerate() {
        if keep[index] {
            nodes.push(crate::ir::Node {
                op: renumber(op, &map),
                // A residual node's provenance is the derivation record: the
                // source site it came from describes a Program the residual no
                // longer is (D5, D9).
                origin: None,
            });
        }
    }
    let mut outputs = alloc::vec::Vec::with_capacity(graph.outputs.len());
    for output in &graph.outputs {
        let id = map[output.index()];
        if id == u32::MAX {
            return Err(at(
                crate::diag::Code::Interface,
                crate::diag::Site::Graph,
                alloc::format!("%{} is an output but was pruned", output.0),
            ));
        }
        outputs.push(NodeId(id));
    }
    // The residual is a new subject: the original identity, the accounting
    // revision and the static map under one derivation domain (D5).
    let mut encoded: alloc::vec::Vec<u8> = alloc::vec::Vec::with_capacity(1 + 9 * leaves);
    encoded.push(0x01);
    for entry in request.static_inputs {
        match entry {
            Some(value) => {
                encoded.push(0x01);
                encoded.extend_from_slice(&value.to_le_bytes());
            }
            None => encoded.push(0x00),
        }
    }
    let original_graph = checked.id();
    let subject = Subject::derive(
        ids,
        SubjectKind::Extracted,
        &[
            b"partial-evaluation",
            ACCOUNTING_REVISION.as_bytes(),
            &original_graph,
            &encoded,
        ],
        graph.subject.closure.clone(),
    );
    let residual = Graph {
        profile: crate::ir::SubProfile::I64,
        slots: slots.clone(),
        nodes,
        outputs,
        facts: alloc::vec::Vec::new(),
        subject,
        input_shape: None,
        output_shape: None,
    };
    let kept = residual.nodes.len() as u64;
    let rechecked = crate::check::check_with(ids, residual, request.budgets)?;
    let report = Report {
        accounting: ACCOUNTING_REVISION,
        original_graph,
        residual_graph: rechecked.id(),
        static_inputs: request.static_inputs.to_vec(),
        dynamic_inputs: slots.iter().map(|slot| slot.ty).collect(),
        original_nodes: original_nodes as u64,
        folded_nodes: folded,
        pruned_nodes: (original_nodes as u64).saturating_sub(kept),
        residual_nodes: kept,
        variants: 1,
    };
    Ok(Residual {
        checked: rechecked,
        report,
    })
}

/// The residual interface as kernel stack types, in slot order.
///
/// # Errors
/// `E-INTERFACE` if the graph still has a param slot (outside the slice).
pub fn interface_types(
    checked: &crate::check::CheckedGraph,
) -> Result<alloc::vec::Vec<noble_kernel::types::Ty>, crate::diag::Diagnostic> {
    let mut types = alloc::vec::Vec::with_capacity(checked.graph().slots.len());
    for (k, slot) in checked.graph().slots.iter().enumerate() {
        match (slot.kind, slot.ty) {
            (SlotKind::Input, ScalarType::I64) => types.push(noble_kernel::types::Ty::I64),
            (SlotKind::Input, ScalarType::Bool) => types.push(noble_kernel::types::Ty::Bool),
            (SlotKind::Input, ScalarType::F32) => {
                return Err(at(
                    crate::diag::Code::Profile,
                    crate::diag::Site::Slot(k as u32),
                    "an F32 input slot is outside the ngraph-pe/0 slice",
                ))
            }
            (SlotKind::Param, _) => {
                return Err(at(
                    crate::diag::Code::Profile,
                    crate::diag::Site::Slot(k as u32),
                    "a param slot is outside the ngraph-pe/0 slice",
                ))
            }
        }
    }
    Ok(types)
}

/// One emitted `let` binding: the label and the code that produces its value.
struct Binding {
    label: alloc::string::String,
    code: alloc::string::String,
}

/// Renders `checked` as `let`-based Noble source (D9).
///
/// The program's stack effect is the graph's interface: dynamic inputs enter
/// bottom-first, every node is bound by `let v<node>`, and the outputs are read
/// in output order. The result is not accepted here; [`readmit`] is the
/// independent acceptance step.
///
/// # Errors
/// `E-PROFILE` for a graph outside the slice (a guard, a fact record or an
/// operation the emitter cannot spell) and `E-AGGREGATE` for an interface
/// shape tree.
pub fn emit_source(
    checked: &crate::check::CheckedGraph,
) -> Result<alloc::string::String, crate::diag::Diagnostic> {
    admit_slice(checked)?;
    let graph = checked.graph();
    if graph.outputs.is_empty() {
        return Err(refuse(
            crate::diag::Code::Interface,
            "a residual Program needs at least one output",
        ));
    }
    let label = |id: NodeId| alloc::format!("v{}", id.0);
    // Inputs are bound deepest last: the residual's stack enters bottom-first.
    let mut dynamic: alloc::vec::Vec<NodeId> = alloc::vec::Vec::new();
    for (k, slot) in graph.slots.iter().enumerate() {
        if slot.kind != SlotKind::Input {
            continue;
        }
        let node = checked.slot_node(k as u32);
        match graph.nodes[node.index()].op {
            Op::Input(slot_index) if slot_index == k as u32 => dynamic.push(node),
            _ => {
                return Err(at(
                    crate::diag::Code::Interface,
                    crate::diag::Site::Slot(k as u32),
                    "an input slot has no input node",
                ))
            }
        }
    }
    let mut bindings: alloc::vec::Vec<Binding> = alloc::vec::Vec::with_capacity(graph.nodes.len());
    for node in dynamic.iter().rev() {
        bindings.push(Binding {
            label: label(*node),
            code: alloc::string::String::new(),
        });
    }
    let mut bound = alloc::vec![false; graph.nodes.len()];
    for node in &dynamic {
        bound[node.index()] = true;
    }
    for (index, node) in graph.nodes.iter().enumerate() {
        if bound[index] {
            continue;
        }
        let id = NodeId(index as u32);
        let code = match &node.op {
            Op::Input(_) => {
                return Err(at(
                    crate::diag::Code::Interface,
                    crate::diag::Site::Node(id),
                    "a static input leaf is a constant, not an input node",
                ))
            }
            Op::Const(Literal::I64(value)) => alloc::format!("{value}"),
            Op::Const(Literal::Bool(value)) => {
                if *value {
                    alloc::string::String::from("true")
                } else {
                    alloc::string::String::from("false")
                }
            }
            Op::Const(Literal::F32(_)) => {
                return Err(at(
                    crate::diag::Code::Profile,
                    crate::diag::Site::Node(id),
                    "an F32 constant is outside the ngraph-pe/0 slice",
                ))
            }
            Op::I64Add(first, second) | Op::I64Sub(first, second) | Op::I64Mul(first, second) => {
                alloc::format!(
                    "{} {} {}",
                    label(*first),
                    label(*second),
                    match node.op {
                        Op::I64Add(..) => "+",
                        Op::I64Sub(..) => "-",
                        _ => "*",
                    }
                )
            }
            Op::I64Eq(first, second) => alloc::format!("{} {} =", label(*first), label(*second)),
            other => {
                return Err(at(
                    crate::diag::Code::Profile,
                    crate::diag::Site::Node(id),
                    alloc::format!("{} cannot be emitted as Noble source", other.name()),
                ))
            }
        };
        bound[index] = true;
        bindings.push(Binding {
            label: label(id),
            code,
        });
    }
    let mut text = alloc::string::String::new();
    for binding in &bindings {
        if !binding.code.is_empty() {
            text.push_str(&binding.code);
            text.push(' ');
        }
        text.push_str("let ");
        text.push_str(&binding.label);
        text.push('\n');
    }
    for output in &graph.outputs {
        text.push_str(&label(*output));
        text.push('\n');
    }
    Ok(text)
}

fn source_refusal(error: noble_contracts::source::Error) -> crate::diag::Diagnostic {
    refuse(
        crate::diag::Code::Subject,
        alloc::format!("the residual source was refused: {error:?}"),
    )
}

/// Independently re-accepts emitted residual source on the ordinary path (D9).
///
/// The source is prepared by the ordinary Declared-Text-v1 frontend under the
/// default source limits, then its kernel-checked submission is re-admitted
/// and re-extracted, so the returned graph is a checked `ngraph-i64/0` graph
/// reached through the same frontend, kernel and checker as any other subject.
///
/// # Errors
/// `E-SUBJECT` for a frontend or kernel refusal, the admission and extraction
/// codes, and whatever the residual re-check reports.
pub fn readmit(
    ids: &dyn IdentityScheme,
    source: &str,
    inputs: &[noble_kernel::types::Ty],
    budgets: &crate::check::Budgets,
) -> Result<crate::check::CheckedGraph, crate::diag::Diagnostic> {
    let limits = noble_contracts::Limits::default();
    let retained =
        noble_contracts::source::RetainedSourceLimit::new(limits.bytes).ok_or_else(|| {
            refuse(
                crate::diag::Code::Profile,
                "the default source limit exceeds the retained-source cap",
            )
        })?;
    let session = noble_contracts::source::ModuleSession::new_declared_text(&[], false, retained)
        .map_err(source_refusal)?;
    let prepared = session
        .prepare(source.as_bytes(), inputs, limits)
        .map_err(source_refusal)?;
    let submission = prepared.submission().ok_or_else(|| {
        refuse(
            crate::diag::Code::Subject,
            "declared-text preparation produced no submission",
        )
    })?;
    let mut claimed: alloc::vec::Vec<u64> = submission
        .definitions
        .iter()
        .map(|definition| definition.identity)
        .collect();
    claimed.sort_unstable();
    claimed.dedup();
    let admitted = crate::extract::admit(
        ids,
        submission,
        &crate::extract::Request {
            profile: "ngraph-i64/0",
            claimed_closure: &claimed,
            budget: crate::extract::Budget {
                work_units: budgets.work_units,
                expanded_nodes: budgets.expanded_nodes,
                inline_depth: budgets.inline_depth,
            },
        },
    )?;
    Ok(crate::extract::extract(ids, &admitted, budgets)?.checked)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHasher(u64);

    impl crate::identity::IdentityHasher for TestHasher {
        fn update(&mut self, bytes: &[u8]) {
            for &byte in bytes {
                self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(byte));
            }
        }

        fn finish(self: alloc::boxed::Box<Self>) -> Digest {
            let mut digest = [0u8; 32];
            digest[..8].copy_from_slice(&self.0.to_le_bytes());
            digest
        }
    }

    struct TestIds;

    impl IdentityScheme for TestIds {
        fn name(&self) -> &'static str {
            "test"
        }

        fn start(&self) -> alloc::boxed::Box<dyn crate::identity::IdentityHasher + '_> {
            alloc::boxed::Box::new(TestHasher(0))
        }
    }

    fn checked(graph: Graph) -> crate::check::CheckedGraph {
        crate::check::check(&TestIds, graph).expect("the fixture graph is checked")
    }

    /// The model's mixed fixture: `%0 = input 0; %1 = input 1;
    /// %2 = i64.mul %0 %1; %3 = i64.add %2 %0; out %3` (`y·x + x`).
    fn product_plus() -> crate::check::CheckedGraph {
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            Subject::authored(&TestIds, "pe-product-plus"),
        );
        let x = builder.input(ScalarType::I64);
        let y = builder.input(ScalarType::I64);
        let product = builder.op(Op::I64Mul(x, y));
        let sum = builder.op(Op::I64Add(product, x));
        builder.output(sum);
        checked(builder.finish())
    }

    fn run(
        checked: &crate::check::CheckedGraph,
        inputs: &[i64],
    ) -> alloc::vec::Vec<crate::sem::Value> {
        match crate::eval::reference(
            checked,
            &crate::eval::i64_inputs(inputs),
            None,
            &mut crate::eval::Control::default(),
        )
        .obs
        {
            crate::eval::Obs::Normal(values) => values,
            other => panic!("unexpected observation {other:?}"),
        }
    }

    fn specialize_with(
        checked: &crate::check::CheckedGraph,
        static_inputs: &[Option<i64>],
    ) -> Result<Residual, crate::diag::Diagnostic> {
        let budgets = crate::check::Budgets::default();
        specialize(
            &TestIds,
            checked,
            &Request {
                static_inputs,
                budgets: &budgets,
            },
        )
    }

    #[test]
    fn no_static_inputs_keeps_the_graph_and_index_order() {
        let original = product_plus();
        let residual = specialize_with(&original, &[None, None]).expect("specialized");
        assert_eq!(residual.report.folded_nodes, 0);
        assert_eq!(residual.report.pruned_nodes, 0);
        assert_eq!(residual.report.residual_nodes, 4);
        assert_eq!(
            residual.report.dynamic_inputs,
            alloc::vec![ScalarType::I64; 2]
        );
        for (x, y) in [(5i64, 4i64), (-1, i64::MAX), (0, 0)] {
            assert_eq!(
                run(&residual.checked, &[x, y]),
                run(&original, &[x, y]),
                "no-static ({x}, {y})"
            );
        }
    }

    #[test]
    fn static_prefix_leaf_becomes_a_constant_and_collapses() {
        let original = product_plus();
        // `x = 5` static: `%2 = 5·y` stays residual, `%3 = %2 + 5`.
        let residual = specialize_with(&original, &[Some(5), None]).expect("specialized");
        assert_eq!(residual.report.folded_nodes, 1);
        assert_eq!(
            residual.report.dynamic_inputs,
            alloc::vec![ScalarType::I64; 1]
        );
        for y in [0i64, 4, -3, i64::MIN] {
            assert_eq!(
                run(&residual.checked, &[y]),
                run(&original, &[5, y]),
                "static-prefix y = {y}"
            );
            assert_eq!(
                residual.merge(&[y]).expect("merge"),
                crate::eval::i64_inputs(&[5, y])
            );
        }
    }

    #[test]
    fn non_prefix_static_slot_is_ranked_and_the_prefix_control_disagrees() {
        let original = product_plus();
        // `y = 7` static, `x` dynamic: the residual's only input is `x` (rank 0).
        let residual = specialize_with(&original, &[None, Some(7)]).expect("specialized");
        assert_eq!(
            residual.report.dynamic_inputs,
            alloc::vec![ScalarType::I64; 1]
        );
        for x in [0i64, 3, -9] {
            assert_eq!(
                run(&residual.checked, &[x]),
                run(&original, &[x, 7]),
                "non-prefix x = {x}"
            );
        }
        // The prefix control: renumber as if the static leaf were a prefix.
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            Subject::authored(&TestIds, "pe-prefix-control"),
        );
        let x = builder.input(ScalarType::I64);
        let seven = builder.i64_const(7);
        let product = builder.op(Op::I64Mul(x, seven));
        let sum = builder.op(Op::I64Add(product, x));
        builder.output(sum);
        let prefix = checked(builder.finish());
        // Both graphs take one input; the prefix control still computes the
        // same function here, so its disagreement is in the input *position*:
        // it consumes the static leaf's value from the dynamic stack.
        assert_eq!(run(&prefix, &[3]), run(&original, &[3, 7]));
        assert_ne!(
            rank(&[None, Some(7)], 1),
            rank(&[Some(7), None], 1),
            "the prefix map ranks the dynamic slot differently"
        );
    }

    #[test]
    fn all_static_inputs_fold_the_whole_graph() {
        let original = product_plus();
        let residual = specialize_with(&original, &[Some(6), Some(7)]).expect("specialized");
        // Two static leaves and two folded arithmetic nodes.
        assert_eq!(residual.report.folded_nodes, 4);
        assert_eq!(residual.report.dynamic_inputs, alloc::vec::Vec::new());
        assert_eq!(residual.report.residual_nodes, 1);
        assert_eq!(residual.report.pruned_nodes, 3);
        assert_eq!(run(&residual.checked, &[]), run(&original, &[6, 7]));
        assert_eq!(
            run(&residual.checked, &[]),
            alloc::vec![crate::sem::Value::I64(48)]
        );
    }

    #[test]
    fn wraparound_folds_to_the_wrapping_value() {
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            Subject::authored(&TestIds, "pe-wrap"),
        );
        let max = builder.i64_const(i64::MAX);
        let one = builder.i64_const(1);
        let sum = builder.op(Op::I64Add(max, one));
        builder.output(sum);
        let original = checked(builder.finish());
        let residual = specialize_with(&original, &[]).expect("specialized");
        assert_eq!(
            run(&residual.checked, &[]),
            alloc::vec![crate::sem::Value::I64(i64::MIN)]
        );
        assert_eq!(run(&residual.checked, &[]), run(&original, &[]));
    }

    #[test]
    fn equality_folds_to_a_bool_constant() {
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            Subject::authored(&TestIds, "pe-eq"),
        );
        let x = builder.input(ScalarType::I64);
        let three = builder.i64_const(3);
        let same = builder.op(Op::I64Eq(x, three));
        builder.output(same);
        let original = checked(builder.finish());
        let residual = specialize_with(&original, &[Some(3)]).expect("specialized");
        assert_eq!(residual.report.folded_nodes, 2);
        assert_eq!(
            run(&residual.checked, &[]),
            alloc::vec![crate::sem::Value::Bool(true)]
        );
        assert_eq!(run(&residual.checked, &[]), run(&original, &[3]));
        let unequal = specialize_with(&original, &[Some(4)]).expect("specialized");
        assert_eq!(
            run(&unequal.checked, &[]),
            alloc::vec![crate::sem::Value::Bool(false)]
        );
    }

    #[test]
    fn operand_order_survives_subtraction() {
        // `%0 = input 0; %1 = input 1; %2 = i64.sub %0 %1` keeps deeper-first.
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            Subject::authored(&TestIds, "pe-sub"),
        );
        let x = builder.input(ScalarType::I64);
        let y = builder.input(ScalarType::I64);
        let difference = builder.op(Op::I64Sub(x, y));
        builder.output(difference);
        let original = checked(builder.finish());
        let residual = specialize_with(&original, &[Some(10), None]).expect("specialized");
        assert_eq!(residual.report.folded_nodes, 1);
        assert_eq!(run(&residual.checked, &[4]), run(&original, &[10, 4]));
        assert_eq!(
            run(&residual.checked, &[4]),
            alloc::vec![crate::sem::Value::I64(6)]
        );
    }

    #[test]
    fn mixed_expression_keeps_its_shape_and_sharing() {
        // `(x + 2) + 3` with `x` dynamic stays two additions: no reassociation.
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            Subject::authored(&TestIds, "pe-offset"),
        );
        let x = builder.input(ScalarType::I64);
        let two = builder.i64_const(2);
        let inner = builder.op(Op::I64Add(x, two));
        let three = builder.i64_const(3);
        let outer = builder.op(Op::I64Add(inner, three));
        builder.output(outer);
        let original = checked(builder.finish());
        let residual = specialize_with(&original, &[None]).expect("specialized");
        assert_eq!(residual.report.folded_nodes, 0);
        assert_eq!(residual.report.residual_nodes, 5);
        assert_eq!(residual.report.pruned_nodes, 0);
        assert_eq!(run(&residual.checked, &[4]), run(&original, &[4]));
    }

    #[test]
    fn pruning_renumbers_the_operands_of_kept_nodes() {
        // `%0 = input 0; %1 = input 1; %2 = mul %0 %1` (dead once `%0` is a
        // constant of a graph whose output ignores `%2`); `%3 = const 5`;
        // `%4 = add %1 %3`. With `x = 2` static, nodes `%0` and `%2` are
        // pruned, so the kept `%4` must be renumbered and its operands
        // rewritten: an unrenumbered residual names a node that no longer
        // exists and the independent re-check refuses it.
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            Subject::authored(&TestIds, "pe-prune"),
        );
        let x = builder.input(ScalarType::I64);
        let y = builder.input(ScalarType::I64);
        let product = builder.op(Op::I64Mul(x, y));
        let five = builder.i64_const(5);
        let sum = builder.op(Op::I64Add(y, five));
        builder.output(sum);
        let original = checked(builder.finish());
        assert_eq!(
            run(&original, &[4, 3]),
            alloc::vec![crate::sem::Value::I64(8)]
        );
        let residual = specialize_with(&original, &[Some(2), None]).expect("specialized");
        assert_eq!(residual.report.pruned_nodes, 2);
        assert_eq!(residual.report.residual_nodes, 3);
        assert_eq!(
            residual.checked.graph().nodes[2].op,
            Op::I64Add(NodeId(0), NodeId(1)),
            "the kept addition is renumbered onto its kept operands"
        );
        assert!(residual.checked.graph().nodes.len() < original.graph().nodes.len());
        for y in [0i64, 3, -2] {
            assert_eq!(
                run(&residual.checked, &[y]),
                run(&original, &[2, y]),
                "pruned y = {y}"
            );
        }
        // The emitted source only names the kept nodes, so it re-admits.
        let source = emit_source(&residual.checked).expect("emitted");
        assert!(!source.contains("v3") && !source.contains("v4"), "{source}");
        let readmitted = readmit(
            &TestIds,
            &source,
            &interface_types(&residual.checked).expect("types"),
            &crate::check::Budgets::default(),
        )
        .expect("re-admitted");
        for y in [0i64, 3] {
            assert_eq!(run(&readmitted, &[y]), run(&residual.checked, &[y]));
        }
    }

    #[test]
    fn residual_identity_is_new_and_the_closure_is_kept() {
        let original = product_plus();
        let residual = specialize_with(&original, &[Some(5), None]).expect("specialized");
        assert_ne!(
            residual.report.residual_graph,
            residual.report.original_graph
        );
        assert_eq!(residual.report.original_graph, original.id());
        assert_eq!(residual.checked.id(), residual.report.residual_graph);
        assert_eq!(
            residual.checked.graph().subject.closure,
            original.graph().subject.closure
        );
        assert_ne!(
            residual.checked.graph().subject.identity,
            original.graph().subject.identity
        );
        assert_eq!(residual.report.accounting, ACCOUNTING_REVISION);
        assert_eq!(residual.report.variants, 1);
        // A different static map is a different residual identity (D11).
        let other = specialize_with(&original, &[Some(6), None]).expect("specialized");
        assert_ne!(other.report.residual_graph, residual.report.residual_graph);
    }

    #[test]
    fn wrong_arity_is_refused_with_interface() {
        let original = product_plus();
        for map in [&[Some(1)][..], &[Some(1), None, None][..]] {
            let refusal = specialize_with(&original, map).expect_err("refused");
            assert_eq!(refusal.code, crate::diag::Code::Interface, "{refusal}");
            assert_eq!(refusal.site, crate::diag::Site::Graph);
        }
    }

    #[test]
    fn caps_are_charged_and_a_raised_cap_is_a_profile_refusal() {
        let original = product_plus();
        let default = crate::check::Budgets::default();
        let below = crate::check::Budgets {
            residual_nodes: 3,
            ..default.clone()
        };
        let refusal = specialize(
            &TestIds,
            &original,
            &Request {
                static_inputs: &[Some(5), None],
                budgets: &below,
            },
        )
        .expect_err("the fourth created node exceeds the cap");
        assert_eq!(
            refusal.code,
            crate::diag::Code::Budget(crate::diag::BudgetKind::ResidualNodes)
        );
        assert_eq!(refusal.budget, Some((3, 4)));
        let at = crate::check::Budgets {
            residual_nodes: 4,
            ..default.clone()
        };
        assert!(
            specialize(
                &TestIds,
                &original,
                &Request {
                    static_inputs: &[Some(5), None],
                    budgets: &at,
                },
            )
            .is_ok(),
            "the cap is inclusive"
        );
        let zero_variants = crate::check::Budgets {
            variants: 0,
            ..default.clone()
        };
        let refusal = specialize(
            &TestIds,
            &original,
            &Request {
                static_inputs: &[Some(5), None],
                budgets: &zero_variants,
            },
        )
        .expect_err("a zero variant cap refuses the one variant");
        assert_eq!(
            refusal.code,
            crate::diag::Code::Budget(crate::diag::BudgetKind::Variants)
        );
        assert_eq!(refusal.budget, Some((0, 1)));
        let raised = crate::check::Budgets {
            residual_nodes: default.residual_nodes + 1,
            ..default.clone()
        };
        let refusal = specialize(
            &TestIds,
            &original,
            &Request {
                static_inputs: &[Some(5), None],
                budgets: &raised,
            },
        )
        .expect_err("a raised cap is a profile refusal");
        assert_eq!(refusal.code, crate::diag::Code::Profile);
    }

    #[test]
    fn a_fact_or_guard_graph_is_refused_before_any_node() {
        // An `I64` input-validated range fact is admissible for the profile and
        // outside the `ngraph-pe/0` slice: specialization changes the interface
        // the fact speaks about. A guard needs a checked fact, so this control
        // covers both refusals' shared cause.
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            Subject::authored(&TestIds, "pe-fact"),
        );
        let x = builder.input(ScalarType::I64);
        let one = builder.i64_const(1);
        let sum = builder.op(Op::I64Add(x, one));
        builder.fact(
            x,
            crate::ir::Fact::Range {
                lo: crate::ir::Literal::I64(0),
                hi: crate::ir::Literal::I64(10),
            },
            crate::ir::Provenance::InputValidated,
        );
        builder.output(sum);
        let graph = checked(builder.finish());
        let refusal = specialize_with(&graph, &[None]).expect_err("refused");
        assert_eq!(refusal.code, crate::diag::Code::Profile, "{refusal}");
        assert!(refusal.detail.contains("fact record"), "{refusal}");
    }

    #[test]
    fn emitting_and_readmitting_a_residual_agrees_with_the_residual() {
        let original = product_plus();
        let residual = specialize_with(&original, &[Some(5), None]).expect("specialized");
        let source = emit_source(&residual.checked).expect("emitted");
        assert!(source.contains("let v0"), "{source}");
        assert!(source.contains("5 let"), "{source}");
        let types = interface_types(&residual.checked).expect("types");
        assert_eq!(types.len(), 1);
        let budgets = crate::check::Budgets::default();
        let readmitted = readmit(&TestIds, &source, &types, &budgets).expect("re-admitted");
        for y in [0i64, 4, -3] {
            assert_eq!(
                run(&readmitted, &[y]),
                run(&residual.checked, &[y]),
                "re-admitted y = {y}"
            );
            assert_eq!(run(&readmitted, &[y]), run(&original, &[5, y]));
        }
    }

    #[test]
    fn merge_reports_its_arity_and_rejects_a_short_vector() {
        assert_eq!(
            merge(&[Some(1), None, None], &[7, 8]),
            Some(crate::eval::i64_inputs(&[1, 7, 8]))
        );
        assert_eq!(merge(&[Some(1), None], &[]), None);
        assert_eq!(merge(&[None], &[1, 2]), None);
        assert_eq!(rank(&[None, Some(2)], 1), Some(1));
        assert_eq!(rank(&[Some(2), None], 1), Some(0));
        assert_eq!(rank(&[None, Some(2)], 2), Some(1));
        assert_eq!(rank(&[None], 1), Some(1));
        assert_eq!(rank(&[None], 2), None);
    }
}
