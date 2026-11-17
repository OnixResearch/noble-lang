//! D-neutral lineage for a selected, independently checked pure-I64 constructed
//! Program. Unsupported source shapes keep ordinary Program execution and
//! saved-owner retention without an invented returned-target identity.

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-wasm-maintainers; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
)]
use alloc::{rc::Rc, vec::Vec};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-wasm-maintainers; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
)]
use noble_contracts::source::proof::{CheckedLexicalKind, CheckedSelectedTarget};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-wasm-maintainers; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
)]
use noble_kernel::{types::Ty, untrusted::NodeId};

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-wasm-maintainers; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
)]
use super::{
    plan::{Action, Layout},
    OriginAction, OriginFixedArithmetic, OriginFixedExpression, OriginI64, OriginOperand,
    OriginOperation, OriginOutput, OriginProgram, OriginProgramValue, OriginSource,
    SelectedOriginMetadata,
};

const MAX_ORIGIN_PROGRAMS: usize = 32;
const MAX_ORIGIN_OPERATIONS: usize = 128;
const MAX_ORIGIN_DEPTH: usize = 32;
// Independent compiler-only observation ceiling. This does not debit the
// source frontend's already-completed Meter; lexical replay separately uses
// the selected request's original Limits.work.
const MAX_REPLAY_STEPS: usize = 4096;

#[derive(Clone)]
enum Value {
    I64(OriginI64),
    Program {
        result: Rc<OriginProgramValue>,
        depth: usize,
    },
}

// Every decision below that branches and then continues is made by a helper
// over copied values: no borrow of the selected target or of a looked-up
// program is live across a join of two continuing paths.
fn action(
    selected: &CheckedSelectedTarget<'_>,
    plan: &Layout,
    program_index: usize,
    node: NodeId,
    operation: Action,
) -> Option<OriginAction> {
    match operation {
        Action::I64(value) => Some(OriginAction::I64(value)),
        Action::Word(0) => Some(OriginAction::Dup),
        Action::Word(1) => Some(OriginAction::Drop),
        Action::Word(2) => Some(OriginAction::Swap),
        Action::Word(3) => selected
            .is_generated_operation(node)
            .then_some(OriginAction::Dip),
        Action::Word(4) => Some(OriginAction::Add),
        Action::Word(5) => Some(OriginAction::Sub),
        Action::Word(6) => Some(OriginAction::Mul),
        Action::Word(10) => Some(OriginAction::Run),
        Action::Word(9) => Some(OriginAction::Compose),
        Action::Program(index) => {
            (index < plan.programs.len()).then_some(OriginAction::StaticProgram(index))
        }
        Action::Quote(input, output, witness) => {
            if program_index == plan.root {
                return None;
            }
            let site = selected.checked_quote_site(node).ok()?;
            let is_program_operand = quote_operand(site.interface().stack_in.last()?)?;
            Some(OriginAction::Quote {
                input,
                output,
                witness,
                program_operand: is_program_operand,
            })
        }
        Action::Call(index, identity) => {
            let is_root = program_index == plan.root;
            let is_selected = identity == selected.definition_id().identity();
            let is_first_root = first_root_is(&plan.definition_roots, index);
            selected_call(is_root, is_selected, is_first_root, index)
        }
        // Every other action has no selected-origin lowering, so the origin is refused.
        _ => None,
    }
}

/// Whether a quotation captures a Program operand, refusing other captures.
fn quote_operand(captured: &Ty) -> Option<bool> {
    match captured {
        Ty::I64 => Some(false),
        Ty::Program(_, _, effects) => effects.is_empty().then_some(true),
        _ => None,
    }
}

/// Whether the first emitted definition root is program `index`.
fn first_root_is(definition_roots: &[usize], index: usize) -> bool {
    definition_roots
        .first()
        .is_some_and(|first| *first == index)
}

/// The selected call of the selection wrapper, when every condition holds.
fn selected_call(
    is_root: bool,
    is_selected: bool,
    is_first_root: bool,
    index: usize,
) -> Option<OriginAction> {
    (is_root && is_selected && is_first_root).then_some(OriginAction::CallSelected(index))
}

// A selected target holds borrowed source state, so every walk that consults
// it recurses over a bounded index instead of looping over its borrow.
fn programs(selected: &CheckedSelectedTarget<'_>, plan: &Layout) -> Option<Vec<OriginProgram>> {
    if plan.programs.len() > MAX_ORIGIN_PROGRAMS || plan.programs.is_empty() {
        return None;
    }
    let result = programs_from(
        selected,
        plan,
        0,
        0,
        Vec::with_capacity(plan.programs.len()),
    )?;
    let body_nodes = quotation_bodies(selected, &result, 0, 0, alloc::vec![None; result.len()])?;
    bind_bodies(selected, plan, &body_nodes, result, 0)
}

/// Lower the programs from `program_index` on, after `count` earlier operations.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the recursion follows the structure of an already checked, size-bounded input."
)]
fn programs_from(
    selected: &CheckedSelectedTarget<'_>,
    plan: &Layout,
    program_index: usize,
    count: usize,
    mut result: Vec<OriginProgram>,
) -> Option<Vec<OriginProgram>> {
    let is_selection = program_index == plan.root;
    let count = match program_visit(plan, program_index, count, is_selection) {
        ProgramVisit::End => return Some(result),
        ProgramVisit::Refuse => return None,
        ProgramVisit::Lower(count) => count,
    };
    let operations = operations_from(
        selected,
        plan,
        program_index,
        0,
        Vec::with_capacity(plan.programs[program_index].operations.len()),
    )?;
    result.push(lowered_program(
        plan,
        program_index,
        is_selection,
        operations,
    ));
    programs_from(selected, plan, program_index + 1, count, result)
}

/// How `programs_from` proceeds at one program index.
enum ProgramVisit {
    End,
    Refuse,
    Lower(usize),
}

/// Check program `program_index`, after `count` earlier operations, before lowering it.
#[expect(
    tigerstyle::compound_condition,
    reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation."
)]
fn program_visit(
    plan: &Layout,
    program_index: usize,
    count: usize,
    is_selection: bool,
) -> ProgramVisit {
    let Some(program) = plan.programs.get(program_index) else {
        return ProgramVisit::End;
    };
    if program.operations.is_empty()
        || program.effects != 0
        || program.depth > MAX_ORIGIN_DEPTH as u32
        || program.leaves > MAX_ORIGIN_OPERATIONS as u32
    {
        return ProgramVisit::Refuse;
    }
    let Some(count) = count.checked_add(program.operations.len()) else {
        return ProgramVisit::Refuse;
    };
    if count > MAX_ORIGIN_OPERATIONS {
        return ProgramVisit::Refuse;
    }
    let is_single_call = is_single_call(&program.operations);
    if is_selection && !is_single_call {
        return ProgramVisit::Refuse;
    }
    ProgramVisit::Lower(count)
}

/// Whether a program is exactly one call, as the selection wrapper must be.
fn is_single_call(operations: &[super::plan::Operation]) -> bool {
    operations.len() == 1 && matches!(operations[0].action, Action::Call(..))
}

/// The selection wrapper or a definition body, by whether a program is the root.
fn artifact_of(is_selection: bool) -> OriginSource {
    if is_selection {
        OriginSource::Selection
    } else {
        OriginSource::Definition
    }
}

/// The lowered origin of program `program_index` with its lowered operations.
fn lowered_program(
    plan: &Layout,
    program_index: usize,
    is_selection: bool,
    operations: Vec<OriginOperation>,
) -> OriginProgram {
    let source_artifact = artifact_of(is_selection);
    let program = &plan.programs[program_index];
    OriginProgram {
        source_artifact,
        program_index,
        entry: program.entry,
        body_node: None,
        input_signature: program.input,
        output_signature: program.output,
        effect_mask: program.effects,
        operations,
    }
}

/// Lower the operations of one program from `ordinal` on.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the recursion follows the structure of an already checked, size-bounded input."
)]
fn operations_from(
    selected: &CheckedSelectedTarget<'_>,
    plan: &Layout,
    program_index: usize,
    ordinal: usize,
    mut operations: Vec<OriginOperation>,
) -> Option<Vec<OriginOperation>> {
    if ordinal >= plan.programs[program_index].operations.len() {
        return Some(operations);
    }
    let operation = operation_at(selected, plan, program_index, ordinal, &operations)?;
    operations.push(operation);
    operations_from(selected, plan, program_index, ordinal + 1, operations)
}

/// Lower operation `ordinal` of a program after its earlier `prior` operations.
fn operation_at(
    selected: &CheckedSelectedTarget<'_>,
    plan: &Layout,
    program_index: usize,
    ordinal: usize,
    prior: &[OriginOperation],
) -> Option<OriginOperation> {
    let shape = operation_shape(plan, program_index, ordinal, prior)?;
    let action = action(selected, plan, program_index, shape.node, shape.action)?;
    if shape.is_selection {
        if !is_selected_call(shape.node, action) {
            return None;
        }
        return Some(lowered_operation(shape, action, None));
    }
    let site = selected.checked_operation_site(shape.node).ok()?;
    Some(lowered_operation(shape, action, Some(site)))
}

/// The copied facts of one operation that survive its table and duplicate checks.
#[derive(Clone, Copy)]
struct OperationShape {
    node: NodeId,
    action: Action,
    ordinal: u32,
    table_entry: u32,
    input: u32,
    output: u32,
    effects: u32,
    is_selection: bool,
}

/// Check operation `ordinal` of a program against its table span and `prior` nodes.
fn operation_shape(
    plan: &Layout,
    program_index: usize,
    ordinal: usize,
    prior: &[OriginOperation],
) -> Option<OperationShape> {
    let program = &plan.programs[program_index];
    let operation = &program.operations[ordinal];
    let ordinal = u32::try_from(ordinal).ok()?;
    let table_entry = program.entry.checked_add(ordinal)?;
    let node = operation.node;
    if table_entry < plan.first_function
        || table_entry >= plan.functions
        || crate::scan::find_first(prior, |earlier| earlier.node == node).is_some()
    {
        return None;
    }
    if operation.effects != 0 {
        return None;
    }
    Some(OperationShape {
        node,
        action: operation.action,
        ordinal,
        table_entry,
        input: operation.input,
        output: operation.output,
        effects: operation.effects,
        is_selection: program_index == plan.root,
    })
}

/// Whether the selection wrapper's only operation is its selected call at node 0.
fn is_selected_call(node: NodeId, action: OriginAction) -> bool {
    node == NodeId(0) && matches!(action, OriginAction::CallSelected(..))
}

/// The lowered origin of one checked operation and its lexical site, if any.
fn lowered_operation(
    shape: OperationShape,
    action: OriginAction,
    site: Option<noble_contracts::source::proof::CheckedOperationSite>,
) -> OriginOperation {
    OriginOperation {
        node: shape.node,
        lexical_node: site.as_ref().map(|site| site.lexical_node),
        authored_span: site.as_ref().and_then(|site| site.authored_span),
        generated: site
            .as_ref()
            .is_some_and(|site| site.generated_witness.is_some()),
        generated_witness: site.and_then(|site| site.generated_witness),
        ordinal: shape.ordinal,
        table_entry: shape.table_entry,
        input_signature: shape.input,
        output_signature: shape.output,
        effect_mask: shape.effects,
        action,
    }
}

/// Record each static quotation's lexical body node, visiting the operations
/// of program `program` from `ordinal` on and then every later program.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the recursion follows the structure of an already checked, size-bounded input."
)]
fn quotation_bodies(
    selected: &CheckedSelectedTarget<'_>,
    result: &[OriginProgram],
    program: usize,
    ordinal: usize,
    mut body_nodes: Vec<Option<u32>>,
) -> Option<Vec<Option<u32>>> {
    match quotation_cursor(result, program, ordinal) {
        QuotationCursor::End => Some(body_nodes),
        QuotationCursor::NextProgram => {
            quotation_bodies(selected, result, program + 1, 0, body_nodes)
        }
        QuotationCursor::NextOperation => {
            quotation_bodies(selected, result, program, ordinal + 1, body_nodes)
        }
        QuotationCursor::Static { index, node } => {
            let node = node?;
            let kind = selected.checked_lexical_node(node).ok()?.kind;
            if !is_quotation(kind) {
                return None;
            }
            let slot = body_nodes.get_mut(index)?;
            if slot.replace(node).is_some() {
                return None;
            }
            quotation_bodies(selected, result, program, ordinal + 1, body_nodes)
        }
    }
}

/// Where the quotation-body walk stands at one program and operation.
enum QuotationCursor {
    End,
    NextProgram,
    NextOperation,
    Static { index: usize, node: Option<u32> },
}

/// Classify operation `ordinal` of program `program` for the quotation-body walk.
fn quotation_cursor(result: &[OriginProgram], program: usize, ordinal: usize) -> QuotationCursor {
    let Some(origin) = result.get(program) else {
        return QuotationCursor::End;
    };
    let Some(operation) = origin.operations.get(ordinal) else {
        return QuotationCursor::NextProgram;
    };
    match operation.action {
        OriginAction::StaticProgram(index) => QuotationCursor::Static {
            index,
            node: operation.lexical_node,
        },
        _ => QuotationCursor::NextOperation,
    }
}

/// Whether a checked lexical node is a quotation.
fn is_quotation(kind: CheckedLexicalKind<'_>) -> bool {
    matches!(kind, CheckedLexicalKind::Quotation(_))
}

/// Bind the programs from `index` on to their lexical bodies.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the recursion follows the structure of an already checked, size-bounded input."
)]
fn bind_bodies(
    selected: &CheckedSelectedTarget<'_>,
    plan: &Layout,
    body_nodes: &[Option<u32>],
    mut result: Vec<OriginProgram>,
    index: usize,
) -> Option<Vec<OriginProgram>> {
    if index >= result.len() {
        return Some(result);
    }
    bind_body(selected, plan, body_nodes, &mut result[index])?;
    bind_bodies(selected, plan, body_nodes, result, index + 1)
}

/// Bind one program to its lexical body and check its operations follow it.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
fn bind_body(
    selected: &CheckedSelectedTarget<'_>,
    plan: &Layout,
    body_nodes: &[Option<u32>],
    program: &mut OriginProgram,
) -> Option<()> {
    let named = plan.definition_roots[0];
    let index = program.program_index;
    if index == plan.root {
        if body_nodes[index].is_some() {
            return None;
        }
        return Some(());
    }
    if index == named {
        if body_nodes[index].is_some() {
            return None;
        }
        return follows_checked_body(selected.lexical_body(), &program.operations);
    }
    let body_node = body_nodes[index]?;
    program.body_node = Some(body_node);
    let CheckedLexicalKind::Quotation(body) = selected.checked_lexical_node(body_node).ok()?.kind
    else {
        return None;
    };
    follows_checked_body(body, &program.operations)
}

/// `Some(())` when the operations follow the lexical body exactly.
fn follows_checked_body(lexical_body: &[u32], operations: &[OriginOperation]) -> Option<()> {
    if lexical_body.len() != operations.len() || !follows_lexical_body(lexical_body, operations) {
        return None;
    }
    Some(())
}

/// Whether each operation names the lexical node at its position, as far as both run.
fn follows_lexical_body(lexical_body: &[u32], operations: &[OriginOperation]) -> bool {
    let mut index = 0usize;
    let mut is_following = true;
    while is_following && index < lexical_body.len() && index < operations.len() {
        is_following = names_lexical_node(&operations[index], lexical_body[index]);
        index += 1;
    }
    is_following
}

/// Whether an operation names `lexical` as its lexical occurrence.
fn names_lexical_node(operation: &OriginOperation, lexical: u32) -> bool {
    operation.lexical_node == Some(lexical)
}

fn program_value(value: Value) -> Option<(Rc<OriginProgramValue>, usize)> {
    match value {
        Value::Program { result, depth } => Some((result, depth)),
        Value::I64(_) => None,
    }
}

fn fixed_dimensions(recipe: &OriginFixedExpression) -> (usize, usize) {
    match recipe {
        OriginFixedExpression::Literal(_) => (1, 1),
        OriginFixedExpression::Arithmetic { nodes, depth, .. } => (*nodes, *depth),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-wasm-maintainers; the bounded origin interpreter threads its program table, layout, value stack, recursion guard, depth, static cache and step meter explicitly through each recursive step."
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-wasm-maintainers; the replayed operand stack is pushed and popped, so it needs the growable vector."
)]
fn execute(
    programs: &[OriginProgram],
    plan: &Layout,
    index: usize,
    stack: &mut Vec<Value>,
    active: &mut [usize; MAX_ORIGIN_DEPTH],
    level: usize,
    static_programs: &mut [Option<Rc<OriginProgramValue>>],
    steps: &mut usize,
) -> Option<()> {
    if level >= MAX_ORIGIN_DEPTH || active[..level].contains(&index) {
        return None;
    }
    active[level] = index;
    let named = programs.get(index)?;
    let mut at = 0usize;
    let mut outcome = Some(());
    while outcome.is_some() && at < named.operations.len() {
        outcome = execute_operation(
            programs,
            plan,
            &named.operations[at],
            stack,
            active,
            level,
            static_programs,
            steps,
        );
        at += 1;
    }
    outcome
}

/// Replay one operation of the program at `level` against the value stack.
#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-wasm-maintainers; the bounded origin interpreter threads its program table, layout, value stack, recursion guard, depth, static cache and step meter explicitly through each recursive step."
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-wasm-maintainers; the replayed operand stack is pushed and popped, so it needs the growable vector."
)]
fn execute_operation(
    programs: &[OriginProgram],
    plan: &Layout,
    operation: &OriginOperation,
    stack: &mut Vec<Value>,
    active: &mut [usize; MAX_ORIGIN_DEPTH],
    level: usize,
    static_programs: &mut [Option<Rc<OriginProgramValue>>],
    steps: &mut usize,
) -> Option<()> {
    *steps = (*steps).checked_add(1)?;
    if *steps > MAX_REPLAY_STEPS {
        return None;
    }
    apply_operation(
        programs,
        plan,
        operation,
        stack,
        active,
        level,
        static_programs,
        steps,
    )?;
    if stack.len() > MAX_ORIGIN_OPERATIONS {
        return None;
    }
    Some(())
}

/// Apply the stack effect of one replayed operation; every arm ends the function.
#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-wasm-maintainers; the bounded origin interpreter threads its program table, layout, value stack, recursion guard, depth, static cache and step meter explicitly through each recursive step."
)]
#[expect(
    tigerstyle::function_length,
    tigerstyle::mutating_input_in_pure,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the function keeps one ordered pass in a single reviewable unit; splitting it would separate each step from its bounds checks; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-wasm-maintainers; the replayed operand stack is pushed and popped, so it needs the growable vector."
)]
fn apply_operation(
    programs: &[OriginProgram],
    plan: &Layout,
    operation: &OriginOperation,
    stack: &mut Vec<Value>,
    active: &mut [usize; MAX_ORIGIN_DEPTH],
    level: usize,
    static_programs: &mut [Option<Rc<OriginProgramValue>>],
    steps: &mut usize,
) -> Option<()> {
    match operation.action {
        OriginAction::I64(value) => {
            stack.push(Value::I64(OriginI64::Fixed {
                value,
                recipe: Rc::new(OriginFixedExpression::Literal(value)),
            }));
            Some(())
        }
        OriginAction::Dup => {
            let top = stack.last()?.clone();
            stack.push(top);
            Some(())
        }
        OriginAction::Drop => {
            stack.pop()?;
            Some(())
        }
        OriginAction::Swap => {
            let len = stack.len();
            if len < 2 {
                return None;
            }
            stack.swap(len - 1, len - 2);
            Some(())
        }
        OriginAction::Dip => {
            let (helper, _) = program_value(stack.pop()?)?;
            let OriginProgramValue::Static { program_index, .. } = helper.as_ref() else {
                return None;
            };
            let hidden = stack.pop()?;
            execute(
                programs,
                plan,
                *program_index,
                stack,
                active,
                level + 1,
                static_programs,
                steps,
            )?;
            stack.push(hidden);
            Some(())
        }
        OriginAction::Add | OriginAction::Sub | OriginAction::Mul => {
            let Value::I64(OriginI64::Fixed {
                value: right,
                recipe: right_recipe,
            }) = stack.pop()?
            else {
                return None;
            };
            let Value::I64(OriginI64::Fixed {
                value: left,
                recipe: left_recipe,
            }) = stack.pop()?
            else {
                return None;
            };
            let (value, kind) = fixed_arithmetic(operation.action, left, right)?;
            let (left_nodes, left_depth) = fixed_dimensions(&left_recipe);
            let (right_nodes, right_depth) = fixed_dimensions(&right_recipe);
            let nodes = left_nodes.checked_add(right_nodes)?.checked_add(1)?;
            let depth = left_depth.max(right_depth).checked_add(1)?;
            if nodes > MAX_ORIGIN_OPERATIONS || depth > MAX_ORIGIN_DEPTH {
                return None;
            }
            stack.push(Value::I64(OriginI64::Fixed {
                value,
                recipe: Rc::new(OriginFixedExpression::Arithmetic {
                    kind,
                    left: left_recipe,
                    right: right_recipe,
                    nodes,
                    depth,
                }),
            }));
            Some(())
        }
        OriginAction::StaticProgram(program_index) => {
            let slot = static_programs.get_mut(program_index)?;
            let depth = usize::try_from(plan.programs.get(program_index)?.depth).ok()?;
            if depth > MAX_ORIGIN_DEPTH {
                return None;
            }
            let result = slot
                .get_or_insert_with(|| {
                    Rc::new(OriginProgramValue::Static {
                        node: operation.node,
                        program_index,
                    })
                })
                .clone();
            stack.push(Value::Program { result, depth });
            Some(())
        }
        OriginAction::Quote {
            program_operand, ..
        } => {
            let (operand, depth) = quoted_operand(stack.pop()?, program_operand)?;
            if depth > MAX_ORIGIN_DEPTH {
                return None;
            }
            stack.push(Value::Program {
                result: Rc::new(OriginProgramValue::Quote {
                    node: operation.node,
                    table_entry: operation.table_entry,
                    operand,
                }),
                depth,
            });
            Some(())
        }
        OriginAction::Compose => {
            let (right, rdepth) = program_value(stack.pop()?)?;
            let (left, ldepth) = program_value(stack.pop()?)?;
            let depth = ldepth.max(rdepth).checked_add(1)?;
            if depth > MAX_ORIGIN_DEPTH {
                return None;
            }
            stack.push(Value::Program {
                result: Rc::new(OriginProgramValue::Compose {
                    node: operation.node,
                    table_entry: operation.table_entry,
                    left,
                    right,
                }),
                depth,
            });
            Some(())
        }
        // Run and the selected call are never replayed, so the origin is refused.
        _ => None,
    }
}

/// The folded value and recipe kind of one fixed arithmetic operation.
fn fixed_arithmetic(
    action: OriginAction,
    left: i64,
    right: i64,
) -> Option<(i64, OriginFixedArithmetic)> {
    match action {
        OriginAction::Add => Some((left.wrapping_add(right), OriginFixedArithmetic::Add)),
        OriginAction::Sub => Some((left.wrapping_sub(right), OriginFixedArithmetic::Sub)),
        OriginAction::Mul => Some((left.wrapping_mul(right), OriginFixedArithmetic::Mul)),
        // Only Add, Sub and Mul reach the fold.
        _ => None,
    }
}

/// The operand a quotation captures from `captured`, and the depth of the value it builds.
fn quoted_operand(captured: Value, is_program_operand: bool) -> Option<(OriginOperand, usize)> {
    if is_program_operand {
        let (operand, depth) = program_value(captured)?;
        return Some((OriginOperand::Program(operand), depth.checked_add(1)?));
    }
    let Value::I64(operand) = captured else {
        return None;
    };
    Some((OriginOperand::I64(operand), 1))
}

#[expect(
    tigerstyle::compound_condition,
    reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation."
)]
fn returned_outputs(
    selected: &CheckedSelectedTarget<'_>,
    plan: &Layout,
    programs: &[OriginProgram],
    named_index: usize,
) -> Option<Vec<OriginOutput>> {
    let inputs = &selected.body().interface.stack_in;
    let outputs = &selected.body().interface.stack_out;
    if inputs.len() > MAX_ORIGIN_OPERATIONS
        || outputs.len() > MAX_ORIGIN_OPERATIONS
        || crate::scan::find_first(inputs, |ty| *ty != Ty::I64).is_some()
        || !selected.body().interface.effects.is_empty()
    {
        return None;
    }
    replay_outputs(inputs, outputs, plan, programs, named_index)
}

/// Replay the named program over symbolic I64 inputs and classify its outputs.
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-wasm-maintainers; growth is bounded by the checked input or admission limits that drive the loop."
)]
fn replay_outputs(
    inputs: &[Ty],
    outputs: &[Ty],
    plan: &Layout,
    programs: &[OriginProgram],
    named_index: usize,
) -> Option<Vec<OriginOutput>> {
    let named = programs.get(named_index)?;
    let mut stack = Vec::with_capacity(inputs.len().saturating_add(named.operations.len()));
    let mut position = 0usize;
    let mut is_refused = false;
    while position < inputs.len() {
        match u32::try_from(position) {
            Ok(input) => {
                stack.push(Value::I64(OriginI64::RootInput(input)));
                position += 1;
            }
            Err(_) => {
                is_refused = true;
                break;
            }
        }
    }
    if is_refused {
        return None;
    }
    let mut steps = 0;
    let mut static_programs: [Option<Rc<OriginProgramValue>>; MAX_ORIGIN_PROGRAMS] =
        core::array::from_fn(|_| None);
    let mut active = [0; MAX_ORIGIN_DEPTH];
    execute(
        programs,
        plan,
        named_index,
        &mut stack,
        &mut active,
        0,
        &mut static_programs[..programs.len()],
        &mut steps,
    )?;
    if stack.len() != outputs.len() {
        return None;
    }
    let mut result = Vec::new();
    let mut stack_position = 0usize;
    while stack_position < outputs.len() {
        match returned_output(
            &outputs[stack_position],
            &stack[stack_position],
            stack_position,
        ) {
            Some(Some(output)) => {
                result.push(output);
                stack_position += 1;
            }
            Some(None) => stack_position += 1,
            None => {
                is_refused = true;
                break;
            }
        }
    }
    if is_refused {
        return None;
    }
    // Cross-output aliasing requires a single graph receipt spanning multiple
    // independently saved owners. Do not claim identity for that profile.
    if result.len() != 1 {
        return None;
    }
    Some(result)
}

/// One returned stack entry: a constructed Program output, an I64, or a refusal.
fn returned_output(ty: &Ty, value: &Value, stack_position: usize) -> Option<Option<OriginOutput>> {
    match ty {
        Ty::Program(..) => program_output(value, stack_position),
        Ty::I64 => i64_output(value),
        _ => None,
    }
}

/// A returned Program entry: a constructed output, or a refusal for anything else.
fn program_output(value: &Value, stack_position: usize) -> Option<Option<OriginOutput>> {
    let Value::Program { result, .. } = value else {
        return None;
    };
    match result.as_ref() {
        // An installed static Program already has its own pinned
        // source/index owner; it is not a constructed returned target.
        OriginProgramValue::Static { .. } => None,
        _ => Some(Some(OriginOutput {
            stack_position,
            result: Rc::clone(result),
        })),
    }
}

/// A returned I64 entry, which carries no output, or a refusal for anything else.
fn i64_output(value: &Value) -> Option<Option<OriginOutput>> {
    match value {
        Value::I64(_) => Some(None),
        Value::Program { .. } => None,
    }
}

#[expect(
    tigerstyle::function_length,
    tigerstyle::mutating_input_in_pure,
    tigerstyle::no_recursion,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the function keeps one ordered pass in a single reviewable unit; splitting it would separate each step from its bounds checks; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the recursion follows the structure of an already checked, size-bounded input; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
fn bounded_graph_cells(
    value: &Rc<OriginProgramValue>,
    programs: &[OriginProgram],
    constructed: &mut [Option<Rc<OriginProgramValue>>; MAX_ORIGIN_PROGRAMS],
    constructed_count: &mut usize,
    static_programs: &mut [bool],
    total: &mut usize,
    fixed_nodes: &mut usize,
) -> Option<()> {
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn charge(total: &mut usize, cells: usize) -> Option<()> {
        *total = total.checked_add(cells)?;
        (*total <= MAX_ORIGIN_PROGRAMS).then_some(())
    }
    /// Record a constructed cell's allocation; `true` if it was already counted.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn counted_before(
        value: &Rc<OriginProgramValue>,
        constructed: &mut [Option<Rc<OriginProgramValue>>; MAX_ORIGIN_PROGRAMS],
        constructed_count: &mut usize,
    ) -> Option<bool> {
        let mut index = 0usize;
        let mut is_counted = false;
        while !is_counted && index < *constructed_count {
            is_counted = is_same_cell(&constructed[index], value);
            index += 1;
        }
        if is_counted {
            return Some(true);
        }
        *constructed.get_mut(*constructed_count)? = Some(Rc::clone(value));
        *constructed_count += 1;
        Some(false)
    }
    /// Whether a recorded constructed cell is the allocation `value`.
    fn is_same_cell(
        prior: &Option<Rc<OriginProgramValue>>,
        value: &Rc<OriginProgramValue>,
    ) -> bool {
        prior.as_ref().is_some_and(|prior| Rc::ptr_eq(prior, value))
    }
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn static_body(
        index: usize,
        programs: &[OriginProgram],
        seen: &mut [bool],
        total: &mut usize,
    ) -> Option<()> {
        if *seen.get(index)? {
            return Some(());
        }
        *seen.get_mut(index)? = true;
        charge(total, 1)?;
        let operations = &programs.get(index)?.operations;
        charge(total, operations.len().checked_mul(2)?.checked_sub(1)?)?;
        let mut at = 0usize;
        let mut outcome = Some(());
        while outcome.is_some() && at < operations.len() {
            outcome = static_child(operations[at].action, programs, seen, total);
            at += 1;
        }
        outcome
    }
    /// Count the static body a static-program operation installs, if it is one.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn static_child(
        action: OriginAction,
        programs: &[OriginProgram],
        seen: &mut [bool],
        total: &mut usize,
    ) -> Option<()> {
        match action {
            OriginAction::StaticProgram(child) => static_body(child, programs, seen, total),
            _ => Some(()),
        }
    }
    #[expect(
        tigerstyle::mutating_input_in_pure,
        tigerstyle::no_recursion,
        reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the recursion follows the structure of an already checked, size-bounded input."
    )]
    fn fixed_recipe(
        recipe: &Rc<OriginFixedExpression>,
        depth: usize,
        nodes: &mut usize,
    ) -> Option<()> {
        *nodes = nodes.checked_add(1)?;
        if *nodes > MAX_ORIGIN_OPERATIONS || depth > MAX_ORIGIN_DEPTH {
            return None;
        }
        match recipe.as_ref() {
            OriginFixedExpression::Arithmetic { left, right, .. } => {
                fixed_recipe(left, depth + 1, nodes)?;
                fixed_recipe(right, depth + 1, nodes)
            }
            OriginFixedExpression::Literal(_) => Some(()),
        }
    }
    /// Count the fixed recipe a captured scalar carries, if it has one.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn scalar_recipe(scalar: &OriginI64, fixed_nodes: &mut usize) -> Option<()> {
        match scalar {
            OriginI64::Fixed { recipe, .. } => fixed_recipe(recipe, 1, fixed_nodes),
            OriginI64::RootInput(_) => Some(()),
        }
    }
    match value.as_ref() {
        OriginProgramValue::Static { program_index, .. } => {
            static_body(*program_index, programs, static_programs, total)
        }
        OriginProgramValue::Quote { operand, .. } => {
            if counted_before(value, constructed, constructed_count)? {
                return Some(());
            }
            match operand {
                OriginOperand::I64(scalar) => {
                    scalar_recipe(scalar, fixed_nodes)?;
                    charge(total, 3)
                }
                OriginOperand::Program(child) => {
                    charge(total, 2)?;
                    bounded_graph_cells(
                        child,
                        programs,
                        constructed,
                        constructed_count,
                        static_programs,
                        total,
                        fixed_nodes,
                    )
                }
            }
        }
        OriginProgramValue::Compose { left, right, .. } => {
            if counted_before(value, constructed, constructed_count)? {
                return Some(());
            }
            charge(total, 2)?;
            bounded_graph_cells(
                left,
                programs,
                constructed,
                constructed_count,
                static_programs,
                total,
                fixed_nodes,
            )?;
            bounded_graph_cells(
                right,
                programs,
                constructed,
                constructed_count,
                static_programs,
                total,
                fixed_nodes,
            )
        }
    }
}

#[expect(
    tigerstyle::compound_condition,
    reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation."
)]
pub(super) fn build(
    selected: &CheckedSelectedTarget<'_>,
    plan: &Layout,
) -> Option<SelectedOriginMetadata> {
    let definitions = selected.submission().definitions.as_slice();
    if definitions.len() != 1 {
        return None;
    }
    let definition = &definitions[0];
    if definition.identity != selected.definition_id().identity()
        || selected.lexical_nodes_len() > 512
        || selected.source().len() > 131_072
        || core::str::from_utf8(selected.source()).is_err()
        || plan.definition_roots.len() != 1
        || plan.root >= plan.programs.len()
    {
        return None;
    }
    let named_program_index = plan.definition_roots[0];
    if named_program_index == plan.root || named_program_index >= plan.programs.len() {
        return None;
    }
    let programs = programs(selected, plan)?;
    let outputs = returned_outputs(selected, plan, &programs, named_program_index)?;
    // Classify only graphs the independently observing worker can retain and
    // inspect in full. Larger pure Programs still execute/save ordinarily.
    // Count each distinct constructed Rc once and every reachable pinned
    // static Program plus its complete recipe atom/concat DAG once.
    let mut cells = 0;
    let mut fixed_nodes = 0;
    let mut constructed: [Option<Rc<OriginProgramValue>>; MAX_ORIGIN_PROGRAMS] =
        core::array::from_fn(|_| None);
    let mut constructed_count = 0;
    let mut static_programs = [false; MAX_ORIGIN_PROGRAMS];
    bounded_graph_cells(
        &outputs[0].result,
        &programs,
        &mut constructed,
        &mut constructed_count,
        &mut static_programs[..programs.len()],
        &mut cells,
        &mut fixed_nodes,
    )?;
    Some(SelectedOriginMetadata {
        definition_id: selected.definition_id(),
        source_generation: selected.source_generation(),
        named_program_index,
        root_program_index: plan.root,
        programs,
        outputs,
    })
}
