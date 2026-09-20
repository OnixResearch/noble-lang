pub(crate) mod topology;

#[derive(Clone, Copy)]
pub(crate) enum Operation {
    I64(i64),
    Bool(bool),
    Unit,
    Program(crate::signatures::Interface),
    Word(u32),
    Quote(crate::signatures::Interface),
}

#[derive(Clone, Copy)]
pub(crate) struct Program {
    pub(crate) owner: Option<u32>,
    pub(crate) entry: u32,
    pub(crate) interface: crate::signatures::Interface,
}

pub(crate) struct Plan {
    pub(crate) nodes: alloc::vec::Vec<Option<Operation>>,
    pub(crate) programs: alloc::vec::Vec<Program>,
    pub(crate) signatures: crate::signatures::Pool,
    pub(crate) host_signatures: [u32; 3],
    pub(crate) functions: u32,
    pub(crate) registry_slots: u32,
}

impl Plan {
    pub(crate) fn operation(
        &self,
        id: noble_kernel::untrusted::NodeId,
    ) -> Result<Operation, crate::Diagnostic> {
        match self.nodes.get(attempt!(crate::admission::index(id))) {
            Some(Some(operation)) => Ok(*operation),
            Some(None) | None => Err(crate::Diagnostic::Defective),
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; complete checked interfaces are interned before construction, and every scope, quota and topology failure propagates as a diagnostic rather than asserting on producer input."
)]
pub(crate) fn lower(
    candidate: &noble_kernel::untrusted::Candidate,
    checked: &noble_kernel::untrusted::Checked,
) -> Result<Plan, crate::Diagnostic> {
    let mut pool = crate::signatures::Pool::new();
    let empty = attempt!(pool.intern(&[]));
    let one_i64 = attempt!(pool.intern(&[noble_kernel::types::Ty::I64]));
    let two_i64 =
        attempt!(pool.intern(&[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64,]));
    let root = attempt!(pool.interface(&checked.interface));
    let nodes = attempt!(lower_nodes(candidate, checked, &mut pool));
    let registry_slots = match u32::try_from(checked.interface.stack_out.len()) {
        Ok(registry_slots) => registry_slots,
        Err(_) => return Err(crate::Diagnostic::Exhausted),
    };
    let mut plan = Plan {
        nodes,
        programs: alloc::vec::Vec::with_capacity(crate::BODY_LIMIT),
        signatures: pool,
        host_signatures: [empty, one_i64, two_i64],
        functions: 3,
        registry_slots,
    };
    attempt!(topology::order(candidate, &mut plan, root));
    Ok(plan)
}

fn empty_nodes(node_count: usize) -> alloc::vec::Vec<Option<Operation>> {
    let mut nodes = alloc::vec::Vec::with_capacity(node_count);
    let mut index = 0usize;
    while index < node_count {
        nodes.push(None);
        index += 1;
    }
    nodes
}

#[expect(
    tigerstyle::assertion_density,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; invalid derivation indices and unsupported nodes return diagnostics; only the freshly owned private signature pool is mutated, never the borrowed candidate or checked result."
)]
fn lower_nodes(
    candidate: &noble_kernel::untrusted::Candidate,
    checked: &noble_kernel::untrusted::Checked,
    pool: &mut crate::signatures::Pool,
) -> Result<alloc::vec::Vec<Option<Operation>>, crate::Diagnostic> {
    let mut nodes = empty_nodes(candidate.nodes.len());
    let mut index = 0usize;
    while index < checked.derivations.len() {
        let derivation = &checked.derivations[index];
        let node_index = attempt!(crate::admission::index(derivation.node));
        let existing = match nodes.get(node_index) {
            Some(existing) => *existing,
            None => return Err(crate::Diagnostic::Defective),
        };
        // A witness belongs to its immutable node, not a use site. Repeated
        // occurrences retain body order but need only one checked lowering.
        if existing.is_none() {
            attempt!(pool.interface(&derivation.interface));
            let node = attempt!(crate::admission::node(candidate, derivation.node));
            let operation = attempt!(operation(node, &derivation.interface, pool));
            nodes[node_index] = Some(operation);
        }
        index += 1;
    }
    Ok(nodes)
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; structural interface interning allocates in the private compiler-owned pool and cannot be const; the borrowed node and checked interface remain unchanged."
)]
fn operation(
    node: &noble_kernel::untrusted::Node,
    checked: &noble_kernel::untrusted::Interface,
    pool: &mut crate::signatures::Pool,
) -> Result<Operation, crate::Diagnostic> {
    match node {
        noble_kernel::untrusted::Node::Literal { lit, .. } => match lit {
            noble_kernel::untrusted::Lit::I64(value) => Ok(Operation::I64(*value)),
            noble_kernel::untrusted::Lit::Bool(value) => Ok(Operation::Bool(*value)),
            noble_kernel::untrusted::Lit::Unit => Ok(Operation::Unit),
            noble_kernel::untrusted::Lit::Text => Err(crate::Diagnostic::Unsupported),
        },
        noble_kernel::untrusted::Node::Quotation { .. } => {
            Ok(Operation::Program(attempt!(pool.produced_program(checked))))
        }
        noble_kernel::untrusted::Node::Invocation { def, .. } => {
            if def.0 == 8 {
                attempt!(scalar_capture(&checked.stack_in));
                return Ok(Operation::Quote(attempt!(pool.produced_program(checked))));
            }
            if def.0 == 7 {
                attempt!(scalar_equality(checked));
            }
            if !crate::operations::supported(def.0) {
                return Err(crate::Diagnostic::Unsupported);
            }
            Ok(Operation::Word(def.0))
        }
    }
}

#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the immediately preceding nonempty check proves stack.len() - 1 cannot underflow, and indexing selects exactly the captured top slot."
)]
const fn scalar_capture(stack: &[noble_kernel::types::Ty]) -> Result<(), crate::Diagnostic> {
    if stack.is_empty() {
        return Err(crate::Diagnostic::Defective);
    }
    match &stack[stack.len() - 1] {
        noble_kernel::types::Ty::I64 => Ok(()),
        _ => Err(crate::Diagnostic::Unsupported),
    }
}

#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; slot_count >= 2 is checked before subtracting two, and the bounded scan examines exactly those top two equality operands."
)]
fn scalar_equality(checked: &noble_kernel::untrusted::Interface) -> Result<(), crate::Diagnostic> {
    let slot_count = checked.stack_in.len();
    if slot_count < 2 {
        return Err(crate::Diagnostic::Defective);
    }
    let mut index = slot_count - 2;
    while index < slot_count {
        match &checked.stack_in[index] {
            noble_kernel::types::Ty::I64
            | noble_kernel::types::Ty::Bool
            | noble_kernel::types::Ty::Unit => {}
            _ => return Err(crate::Diagnostic::Unsupported),
        }
        index += 1;
    }
    Ok(())
}
