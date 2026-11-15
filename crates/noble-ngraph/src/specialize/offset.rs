//! Opt-in partially-static add/sub experiment over the D1 checked graph slice.
//!
//! Run the unchanged static-or-residual specializer first, then retain an
//! ordered signed list of *node identities* and one wrapping-I64 offset for
//! each linear expression. At a shared value, a nonlinear operation or an
//! output, materialize exactly one expression; this is let insertion on the
//! new graph, not an in-place rewrite of the original Program. The checked
//! source and the new result have distinct identities. This is finite Rust
//! implementation evidence, not a Rust/Lean or backend refinement theorem.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::ir::{Literal, Node, NodeId, Op};

/// Distinct revision of this explicitly selected research transformation.
pub const REVISION: &str = "ngraph-pe/offset/0";

/// A baseline residual and an independently checked, offset-normalized result.
/// The ordinary `specialize` entry point never calls this module.
#[derive(Debug)]
pub struct Comparison {
    /// The unchanged D3 static-or-residual specialization of the source.
    pub baseline: super::Residual,
    /// The new, separately identified and checked graph.
    pub normalized: crate::check::CheckedGraph,
    /// Number of generated nodes, charged before the checker runs.
    pub generated_nodes: u64,
}

#[derive(Clone, Copy)]
struct Term {
    node: NodeId,
    negative: bool,
}

#[derive(Clone)]
struct Expr {
    atoms: Vec<Term>,
    offset: i64,
}

impl Expr {
    fn constant(offset: i64) -> Self {
        Self { atoms: Vec::new(), offset }
    }

    fn atom(node: NodeId) -> Self {
        Self { atoms: alloc::vec![Term { node, negative: false }], offset: 0 }
    }

    fn append(&mut self, mut right: Self, subtract: bool) {
        self.offset = if subtract {
            self.offset.wrapping_sub(right.offset)
        } else {
            self.offset.wrapping_add(right.offset)
        };
        if subtract {
            for atom in &mut right.atoms {
                atom.negative = !atom.negative;
            }
        }
        self.atoms.append(&mut right.atoms);
    }
}

#[derive(Clone)]
enum Value {
    I64(Expr),
    Bool(NodeId),
}

impl Value {
    fn i64(self) -> Expr {
        match self {
            Self::I64(value) => value,
            Self::Bool(_) => unreachable!("a checked I64 operand is not Bool"),
        }
    }
}

// Only the D1 total whitelist reaches the store; the key also prevents
// duplicate residual arithmetic from losing lexical sharing.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Input(u32),
    I64(i64),
    Bool(bool),
    Add(u32, u32),
    Sub(u32, u32),
    Mul(u32, u32),
    Eq(u32, u32),
}

fn key(op: &Op) -> Key {
    match op {
        Op::Input(k) => Key::Input(*k),
        Op::Const(Literal::I64(v)) => Key::I64(*v),
        Op::Const(Literal::Bool(v)) => Key::Bool(*v),
        Op::I64Add(a, b) => Key::Add(a.0, b.0),
        Op::I64Sub(a, b) => Key::Sub(a.0, b.0),
        Op::I64Mul(a, b) => Key::Mul(a.0, b.0),
        Op::I64Eq(a, b) => Key::Eq(a.0, b.0),
        _ => unreachable!("the baseline admitted only the total D1 whitelist"),
    }
}

struct Store {
    nodes: Vec<Node>,
    shared: BTreeMap<Key, NodeId>,
    limit: u64,
}

impl Store {
    fn new(limit: u64, capacity: usize) -> Self {
        Self { nodes: Vec::with_capacity(capacity), shared: BTreeMap::new(), limit }
    }

    fn emit(&mut self, op: Op) -> Result<NodeId, crate::diag::Diagnostic> {
        let identity = key(&op);
        if let Some(&id) = self.shared.get(&identity) {
            return Ok(id);
        }
        let charged = self.nodes.len() as u64 + 1;
        if charged > self.limit {
            return Err(crate::diag::Diagnostic::budget(
                crate::diag::BudgetKind::ResidualNodes,
                self.limit,
                charged,
            ));
        }
        let id = NodeId(u32::try_from(self.nodes.len()).expect("checked graph node count fits u32"));
        self.nodes.push(Node { op, origin: None });
        self.shared.insert(identity, id);
        Ok(id)
    }

    fn materialize(&mut self, expression: Expr) -> Result<NodeId, crate::diag::Diagnostic> {
        let mut terms = expression.atoms.into_iter();
        let mut result = match terms.next() {
            None => return self.emit(Op::Const(Literal::I64(expression.offset))),
            Some(Term { node, negative: false }) => node,
            Some(Term { node, negative: true }) => {
                let zero = self.emit(Op::Const(Literal::I64(0)))?;
                self.emit(Op::I64Sub(zero, node))?
            }
        };
        for term in terms {
            result = self.emit(if term.negative {
                Op::I64Sub(result, term.node)
            } else {
                Op::I64Add(result, term.node)
            })?;
        }
        if expression.offset != 0 {
            let constant = self.emit(Op::Const(Literal::I64(expression.offset)))?;
            result = self.emit(Op::I64Add(result, constant))?;
        }
        Ok(result)
    }
}

fn consume(values: &mut [Option<Value>], uses: &mut [u32], id: NodeId) -> Value {
    let count = &mut uses[id.index()];
    assert!(*count > 0, "each checked operand/output use is counted");
    *count -= 1;
    if *count == 0 {
        values[id.index()].take().expect("every operand has an earlier value")
    } else {
        values[id.index()].as_ref().expect("every operand has an earlier value").clone()
    }
}

/// Compare the existing D3 baseline and a bounded, independently checked
/// partially-static offset residual, without changing the ordinary default.
/// The static map, input ordering and baseline budgets are identical on both
/// sides. Additional generated nodes charge the same `residual_nodes` cap.
///
/// # Errors
/// Any baseline refusal, a residual-node growth refusal or independent
/// checker refusal. No new Program is published on refusal.
pub fn specialize(
    ids: &dyn crate::identity::IdentityScheme,
    checked: &crate::check::CheckedGraph,
    request: &super::Request<'_>,
) -> Result<Comparison, crate::diag::Diagnostic> {
    let baseline = super::specialize(ids, checked, request)?;
    let graph = baseline.checked.graph();
    let mut uses = alloc::vec![0u32; graph.nodes.len()];
    for node in &graph.nodes {
        for operand in node.op.operands() {
            uses[operand.index()] += 1;
        }
    }
    for output in &graph.outputs {
        uses[output.index()] += 1;
    }
    let mut remaining = uses.clone();
    let mut values = alloc::vec![None; graph.nodes.len()];
    let mut store = Store::new(request.budgets.residual_nodes, graph.nodes.len());
    for (index, node) in graph.nodes.iter().enumerate() {
        let value = match node.op {
            Op::Input(slot) => {
                let input = store.emit(Op::Input(slot))?;
                if graph.slots[slot as usize].ty == crate::ir::ScalarType::I64 {
                    Value::I64(Expr::atom(input))
                } else {
                    Value::Bool(input)
                }
            }
            Op::Const(Literal::I64(v)) => Value::I64(Expr::constant(v)),
            Op::Const(Literal::Bool(v)) => Value::Bool(store.emit(Op::Const(Literal::Bool(v)))?),
            Op::I64Add(a, b) | Op::I64Sub(a, b) => {
                let mut left = consume(&mut values, &mut remaining, a).i64();
                let right = consume(&mut values, &mut remaining, b).i64();
                left.append(right, matches!(node.op, Op::I64Sub(..)));
                Value::I64(left)
            }
            Op::I64Mul(a, b) | Op::I64Eq(a, b) => {
                let left = store.materialize(consume(&mut values, &mut remaining, a).i64())?;
                let right = store.materialize(consume(&mut values, &mut remaining, b).i64())?;
                let computed = store.emit(if matches!(node.op, Op::I64Mul(..)) {
                    Op::I64Mul(left, right)
                } else {
                    Op::I64Eq(left, right)
                })?;
                if matches!(node.op, Op::I64Mul(..)) {
                    Value::I64(Expr::atom(computed))
                } else {
                    Value::Bool(computed)
                }
            }
            _ => unreachable!("baseline specialization admitted only total D1 operations"),
        };
        values[index] = Some(if uses[index] > 1 {
            match value {
                Value::I64(expr) if !expr.atoms.is_empty() => {
                    Value::I64(Expr::atom(store.materialize(expr)?))
                }
                other => other,
            }
        } else {
            value
        });
    }
    let mut outputs = Vec::with_capacity(graph.outputs.len());
    for output in &graph.outputs {
        let value = consume(&mut values, &mut remaining, *output);
        outputs.push(match value {
            Value::I64(expr) => store.materialize(expr)?,
            Value::Bool(node) => node,
        });
    }
    let generated_nodes = store.nodes.len() as u64;
    let subject = crate::ir::Subject::derive(
        ids,
        crate::ir::SubjectKind::Extracted,
        &[b"partial-static-offset", REVISION.as_bytes(), &baseline.report.original_graph,
            &baseline.report.residual_graph],
        graph.subject.closure.clone(),
    );
    let normalized = crate::check::check_with(ids, crate::ir::Graph {
        profile: graph.profile,
        slots: graph.slots.clone(),
        nodes: store.nodes,
        outputs,
        facts: Vec::new(),
        subject,
        input_shape: None,
        output_shape: None,
    }, request.budgets)?;
    Ok(Comparison { baseline, normalized, generated_nodes })
}


