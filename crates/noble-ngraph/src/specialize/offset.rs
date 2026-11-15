//! Opt-in partially-static add/sub experiment over the D1 checked graph slice.
//!
//! Run the unchanged static-or-residual specializer first, then retain an
//! ordered signed list of *node identities* and one wrapping-I64 offset for
//! each linear expression. At a shared value, a nonlinear operation or an
//! output, materialize exactly one expression; this is let insertion on the
//! new graph, not an in-place rewrite of the original Program. The checked
//! source and the new result have distinct identities. This is finite Rust
//! implementation evidence, not a Rust/Lean or backend refinement theorem.

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
    node: crate::ir::NodeId,
    negative: bool,
}

#[derive(Clone)]
struct Expr {
    atoms: alloc::vec::Vec<Term>,
    constant_value: i64,
}

impl Expr {
    fn constant(value: i64) -> Self {
        Self {
            atoms: alloc::vec::Vec::new(),
            constant_value: value,
        }
    }

    fn atom(node: crate::ir::NodeId) -> Self {
        Self {
            atoms: alloc::vec![Term {
                node,
                negative: false
            }],
            constant_value: 0,
        }
    }

    fn append(&mut self, mut right: Self, subtract: bool) {
        self.constant_value = if subtract {
            self.constant_value.wrapping_sub(right.constant_value)
        } else {
            self.constant_value.wrapping_add(right.constant_value)
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
    Bool(crate::ir::NodeId),
}

impl Value {
    fn i64(self) -> Result<Expr, crate::diag::Diagnostic> {
        match self {
            Self::I64(value) => Ok(value),
            Self::Bool(_) => Err(super::refuse(
                crate::diag::Code::Interface,
                "a checked I64 operand has Bool type",
            )),
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

fn key(op: &crate::ir::Op) -> Result<Key, crate::diag::Diagnostic> {
    match op {
        crate::ir::Op::Input(k) => Ok(Key::Input(*k)),
        crate::ir::Op::Const(crate::ir::Literal::I64(v)) => Ok(Key::I64(*v)),
        crate::ir::Op::Const(crate::ir::Literal::Bool(v)) => Ok(Key::Bool(*v)),
        crate::ir::Op::I64Add(a, b) => Ok(Key::Add(a.0, b.0)),
        crate::ir::Op::I64Sub(a, b) => Ok(Key::Sub(a.0, b.0)),
        crate::ir::Op::I64Mul(a, b) => Ok(Key::Mul(a.0, b.0)),
        crate::ir::Op::I64Eq(a, b) => Ok(Key::Eq(a.0, b.0)),
        _ => Err(super::refuse(
            crate::diag::Code::Profile,
            "the offset experiment admits only the total D1 whitelist",
        )),
    }
}

struct Store {
    nodes: alloc::vec::Vec<crate::ir::Node>,
    shared: alloc::collections::BTreeMap<Key, crate::ir::NodeId>,
    allowed_nodes: u64,
}

impl Store {
    fn new(allowed_nodes: u64, reserved_nodes: usize) -> Self {
        Self {
            nodes: alloc::vec::Vec::with_capacity(reserved_nodes),
            shared: alloc::collections::BTreeMap::new(),
            allowed_nodes,
        }
    }

    fn emit(&mut self, op: crate::ir::Op) -> Result<crate::ir::NodeId, crate::diag::Diagnostic> {
        let identity = key(&op)?;
        if let Some(&id) = self.shared.get(&identity) {
            return Ok(id);
        }
        let charged = self.nodes.len() as u64 + 1;
        if charged > self.allowed_nodes {
            return Err(crate::diag::Diagnostic::budget(
                crate::diag::BudgetKind::ResidualNodes,
                self.allowed_nodes,
                charged,
            ));
        }
        let id = crate::ir::NodeId(u32::try_from(self.nodes.len()).map_err(|_| {
            crate::diag::Diagnostic::budget(
                crate::diag::BudgetKind::ResidualNodes,
                self.allowed_nodes,
                charged,
            )
        })?);
        self.nodes.push(crate::ir::Node { op, origin: None });
        self.shared.insert(identity, id);
        Ok(id)
    }

    fn materialize(
        &mut self,
        expression: Expr,
    ) -> Result<crate::ir::NodeId, crate::diag::Diagnostic> {
        let mut terms = expression.atoms.into_iter();
        let mut result = match terms.next() {
            None => {
                return self.emit(crate::ir::Op::Const(crate::ir::Literal::I64(
                    expression.constant_value,
                )))
            }
            Some(Term {
                node,
                negative: false,
            }) => node,
            Some(Term {
                node,
                negative: true,
            }) => {
                let zero = self.emit(crate::ir::Op::Const(crate::ir::Literal::I64(0)))?;
                self.emit(crate::ir::Op::I64Sub(zero, node))?
            }
        };
        for term in terms {
            result = self.emit(if term.negative {
                crate::ir::Op::I64Sub(result, term.node)
            } else {
                crate::ir::Op::I64Add(result, term.node)
            })?;
        }
        if expression.constant_value != 0 {
            let constant = self.emit(crate::ir::Op::Const(crate::ir::Literal::I64(
                expression.constant_value,
            )))?;
            result = self.emit(crate::ir::Op::I64Add(result, constant))?;
        }
        Ok(result)
    }
}

fn consume(
    values: &mut [Option<Value>],
    uses: &mut [u32],
    id: crate::ir::NodeId,
) -> Result<Value, crate::diag::Diagnostic> {
    let count = uses.get_mut(id.index()).ok_or_else(|| {
        super::refuse(
            crate::diag::Code::Interface,
            "operand outside the checked graph",
        )
    })?;
    if *count == 0 {
        return Err(super::refuse(
            crate::diag::Code::Interface,
            "checked operand consumed more times than it is referenced",
        ));
    }
    *count -= 1;
    let entry = values.get_mut(id.index()).ok_or_else(|| {
        super::refuse(
            crate::diag::Code::Interface,
            "operand outside the checked graph",
        )
    })?;
    let value = if *count == 0 {
        entry.take()
    } else {
        entry.clone()
    };
    value.ok_or_else(|| {
        super::refuse(
            crate::diag::Code::Interface,
            "checked operand has no earlier materialized value",
        )
    })
}

/// Compare the existing D3 baseline and a bounded, independently checked
/// partially-static offset residual, without changing the ordinary default.
/// The static map, input ordering and baseline budgets are identical on both
/// sides. Additional generated nodes charge the same `residual_nodes` cap.
///
/// # Errors
/// Any baseline refusal, a residual-node growth refusal or independent
/// checker refusal. No new Program is published on refusal.
pub fn compare(
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
            crate::ir::Op::Input(slot) => {
                let input = store.emit(crate::ir::Op::Input(slot))?;
                if graph.slots[slot as usize].ty == crate::ir::ScalarType::I64 {
                    Value::I64(Expr::atom(input))
                } else {
                    Value::Bool(input)
                }
            }
            crate::ir::Op::Const(crate::ir::Literal::I64(v)) => Value::I64(Expr::constant(v)),
            crate::ir::Op::Const(crate::ir::Literal::Bool(v)) => {
                Value::Bool(store.emit(crate::ir::Op::Const(crate::ir::Literal::Bool(v)))?)
            }
            crate::ir::Op::I64Add(a, b) | crate::ir::Op::I64Sub(a, b) => {
                let mut left = consume(&mut values, &mut remaining, a)?.i64()?;
                let right = consume(&mut values, &mut remaining, b)?.i64()?;
                left.append(right, matches!(node.op, crate::ir::Op::I64Sub(..)));
                Value::I64(left)
            }
            crate::ir::Op::I64Mul(a, b) | crate::ir::Op::I64Eq(a, b) => {
                let left = store.materialize(consume(&mut values, &mut remaining, a)?.i64()?)?;
                let right = store.materialize(consume(&mut values, &mut remaining, b)?.i64()?)?;
                let computed = store.emit(if matches!(node.op, crate::ir::Op::I64Mul(..)) {
                    crate::ir::Op::I64Mul(left, right)
                } else {
                    crate::ir::Op::I64Eq(left, right)
                })?;
                if matches!(node.op, crate::ir::Op::I64Mul(..)) {
                    Value::I64(Expr::atom(computed))
                } else {
                    Value::Bool(computed)
                }
            }
            _ => {
                return Err(super::refuse(
                    crate::diag::Code::Profile,
                    "the baseline residual contains an operation outside the D1 whitelist",
                ))
            }
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
    let mut outputs = alloc::vec::Vec::with_capacity(graph.outputs.len());
    for output in &graph.outputs {
        let value = consume(&mut values, &mut remaining, *output)?;
        outputs.push(match value {
            Value::I64(expr) => store.materialize(expr)?,
            Value::Bool(node) => node,
        });
    }
    let generated_nodes = store.nodes.len() as u64;
    let subject = crate::ir::Subject::derive(
        ids,
        crate::ir::SubjectKind::Extracted,
        &[
            b"partial-static-offset",
            REVISION.as_bytes(),
            &baseline.report.original_graph,
            &baseline.report.residual_graph,
        ],
        graph.subject.closure.clone(),
    );
    let normalized = crate::check::check_with(
        ids,
        crate::ir::Graph {
            profile: graph.profile,
            slots: graph.slots.clone(),
            nodes: store.nodes,
            outputs,
            facts: alloc::vec::Vec::new(),
            subject,
            input_shape: None,
            output_shape: None,
        },
        request.budgets,
    )?;
    Ok(Comparison {
        baseline,
        normalized,
        generated_nodes,
    })
}
