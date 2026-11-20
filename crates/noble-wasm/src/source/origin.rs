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
    OriginAction, OriginClass, OriginFixedArithmetic, OriginFixedExpression, OriginI64,
    OriginOperand, OriginOperation, OriginOutput, OriginOwner, OriginProgram, OriginProgramValue,
    OriginSource, OriginTemplate, SelectedOriginMetadata,
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
#[expect(
    clippy::question_mark,
    reason = "Owner: noble-wasm-maintainers; an explicit match moves the mutable slot out of its Option, which the pinned extractor supports where `?` on Option<&mut> is not."
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
            let slot = match body_nodes.get_mut(index) {
                Some(slot) => slot,
                None => return None,
            };
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

/// The replay's state: the value stack, the static-program cache, the step
/// meter, the recursion guard and the open program frames, innermost last.
struct Machine {
    stack: Vec<Value>,
    static_programs: Vec<Option<Rc<OriginProgramValue>>>,
    steps: usize,
    active: [usize; MAX_ORIGIN_DEPTH],
    frames: Vec<Frame>,
}

/// One open program of the replay: its index, its recursion level, the next
/// operation position, and the value a `dip` set aside to restore at its end.
struct Frame {
    index: usize,
    level: usize,
    next: usize,
    hidden: Option<Value>,
}

/// What applying one replayed operation asks of the replay driver.
enum Applied {
    /// The operation's stack effect is complete.
    Done,
    /// A `dip` runs static program `child` with `hidden` set aside above it.
    Dip { child: usize, hidden: Value },
}

// One loop replays program `index` and every `dip` it runs, depth-first over
// an explicit frame stack, so each operation is metered, applied, checked and
// refused in exactly the order of a recursive replay, without a loop inside a
// recursive call.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-wasm-maintainers; it mutates only the caller-owned replay machine for this bounded pass; checked inputs stay immutable."
)]
fn execute(
    programs: &[OriginProgram],
    plan: &Layout,
    index: usize,
    machine: &mut Machine,
) -> Option<()> {
    enter_program(programs, index, 0, None, machine)?;
    let mut is_running = true;
    let mut is_refused = false;
    while is_running {
        match replay_step(programs, plan, machine) {
            Some(is_open) => is_running = is_open,
            None => {
                is_refused = true;
                is_running = false;
            }
        }
    }
    if is_refused {
        return None;
    }
    Some(())
}

/// Open program `index` at `level`: refuse a level past the depth bound, a
/// program already running on the current path, or an unknown program.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-wasm-maintainers; it mutates only the caller-owned replay machine for this bounded pass; checked inputs stay immutable."
)]
fn enter_program(
    programs: &[OriginProgram],
    index: usize,
    level: usize,
    hidden: Option<Value>,
    machine: &mut Machine,
) -> Option<()> {
    if level >= MAX_ORIGIN_DEPTH || machine.active[..level].contains(&index) {
        return None;
    }
    machine.active[level] = index;
    if index >= programs.len() {
        return None;
    }
    machine.frames.push(Frame {
        index,
        level,
        next: 0,
        hidden,
    });
    Some(())
}

/// Advance the replay by one event: replay the innermost frame's next
/// operation, or close that frame once its operations are done. `Some(false)`
/// once the root program has finished.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-wasm-maintainers; it mutates only the caller-owned replay machine for this bounded pass; checked inputs stay immutable."
)]
fn replay_step(programs: &[OriginProgram], plan: &Layout, machine: &mut Machine) -> Option<bool> {
    let top = machine.frames.len() - 1;
    let index = machine.frames[top].index;
    let level = machine.frames[top].level;
    let position = machine.frames[top].next;
    let program_ops = &programs[index].operations;
    if position >= program_ops.len() {
        return close_frame(machine);
    }
    machine.frames[top].next += 1;
    match execute_operation(plan, &program_ops[position], machine)? {
        Applied::Done => Some(true),
        Applied::Dip {
            child,
            hidden: set_aside,
        } => {
            enter_program(programs, child, level + 1, Some(set_aside), machine)?;
            Some(true)
        }
    }
}

/// Close the innermost frame. The root frame ends the replay; a `dip` frame
/// restores the value set aside above it, then applies the stack bound its
/// `dip` operation checks once the dipped program has run.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-wasm-maintainers; it mutates only the caller-owned replay machine for this bounded pass; checked inputs stay immutable."
)]
fn close_frame(machine: &mut Machine) -> Option<bool> {
    let Some(frame) = machine.frames.pop() else {
        return Some(false);
    };
    let Some(restored) = frame.hidden else {
        return Some(false);
    };
    machine.stack.push(restored);
    if machine.stack.len() > MAX_ORIGIN_OPERATIONS {
        return None;
    }
    Some(true)
}

/// Replay one operation against the value stack. A `dip` checks the stack
/// bound only after its program has run, when its frame closes.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-wasm-maintainers; it mutates only the caller-owned replay machine for this bounded pass; checked inputs stay immutable."
)]
fn execute_operation(
    plan: &Layout,
    operation: &OriginOperation,
    machine: &mut Machine,
) -> Option<Applied> {
    machine.steps = machine.steps.checked_add(1)?;
    if machine.steps > MAX_REPLAY_STEPS {
        return None;
    }
    match apply_operation(plan, operation, machine)? {
        Applied::Done => {
            if machine.stack.len() > MAX_ORIGIN_OPERATIONS {
                return None;
            }
            Some(Applied::Done)
        }
        Applied::Dip { child, hidden } => Some(Applied::Dip { child, hidden }),
    }
}

/// Apply the stack effect of one replayed operation; every arm ends the
/// function. A `dip` hands its program and set-aside value to the driver.
#[expect(
    tigerstyle::function_length,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-wasm-maintainers; the function keeps one ordered pass in a single reviewable unit; splitting it would separate each step from its bounds checks; it mutates only the caller-owned replay machine for this bounded pass; checked inputs stay immutable."
)]
#[expect(
    clippy::question_mark,
    reason = "Owner: noble-wasm-maintainers; an explicit match moves the mutable slot out of its Option, which the pinned extractor supports where `?` on Option<&mut> is not."
)]
fn apply_operation(
    plan: &Layout,
    operation: &OriginOperation,
    machine: &mut Machine,
) -> Option<Applied> {
    let stack = &mut machine.stack;
    match operation.action {
        OriginAction::I64(value) => {
            stack.push(Value::I64(OriginI64::Fixed {
                value,
                recipe: Rc::new(OriginFixedExpression::Literal(value)),
            }));
            Some(Applied::Done)
        }
        OriginAction::Dup => {
            let top = stack.last()?.clone();
            stack.push(top);
            Some(Applied::Done)
        }
        OriginAction::Drop => {
            stack.pop()?;
            Some(Applied::Done)
        }
        OriginAction::Swap => {
            let len = stack.len();
            if len < 2 {
                return None;
            }
            stack.swap(len - 1, len - 2);
            Some(Applied::Done)
        }
        OriginAction::Dip => {
            let (helper, _) = program_value(stack.pop()?)?;
            let OriginProgramValue::Static { program_index, .. } = helper.as_ref() else {
                return None;
            };
            let hidden = stack.pop()?;
            Some(Applied::Dip {
                child: *program_index,
                hidden,
            })
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
            Some(Applied::Done)
        }
        OriginAction::StaticProgram(program_index) => {
            let slot = match machine.static_programs.get_mut(program_index) {
                Some(slot) => slot,
                None => return None,
            };
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
            Some(Applied::Done)
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
            Some(Applied::Done)
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
            Some(Applied::Done)
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
    let mut machine = Machine {
        stack,
        static_programs: alloc::vec![None; programs.len()],
        steps: 0,
        active: [0; MAX_ORIGIN_DEPTH],
        frames: Vec::with_capacity(MAX_ORIGIN_DEPTH),
    };
    execute(programs, plan, named_index, &mut machine)?;
    let stack = machine.stack;
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
            programs,
            named_index,
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
fn returned_output(
    ty: &Ty,
    value: &Value,
    stack_position: usize,
    programs: &[OriginProgram],
    named_index: usize,
) -> Option<Option<OriginOutput>> {
    match ty {
        Ty::Program(..) => program_output(value, stack_position, programs, named_index),
        Ty::I64 => i64_output(value),
        _ => None,
    }
}

/// A returned Program entry: a constructed output with its exact template
/// projection, or a refusal for anything else.
fn program_output(
    value: &Value,
    stack_position: usize,
    programs: &[OriginProgram],
    named_index: usize,
) -> Option<Option<OriginOutput>> {
    let Value::Program { result, .. } = value else {
        return None;
    };
    match result.as_ref() {
        // An installed static Program already has its own pinned
        // source/index owner; it is not a constructed returned target.
        OriginProgramValue::Static { .. } => None,
        _ => {
            let template = template_projection(result, programs, named_index)?;
            Some(Some(OriginOutput {
                stack_position,
                result: Rc::clone(result),
                template,
            }))
        }
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
        match constructed.get_mut(*constructed_count) {
            Some(slot) => *slot = Some(Rc::clone(value)),
            None => return None,
        }
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
    /// Count every static body reachable from program `index` once, depth-first
    /// over an explicit frame stack in the order of the recursive count, so no
    /// loop sits inside a recursive call.
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
        if !static_enter(index, programs, seen, total)? {
            return Some(());
        }
        let mut frames = StaticFrames {
            frames: Vec::with_capacity(MAX_ORIGIN_PROGRAMS),
        };
        frames.frames.push((index, 0));
        let mut is_walking = true;
        let mut is_refused = false;
        while is_walking {
            match static_step(programs, seen, total, &mut frames) {
                Some(is_open) => is_walking = is_open,
                None => {
                    is_refused = true;
                    is_walking = false;
                }
            }
        }
        if is_refused {
            return None;
        }
        Some(())
    }
    /// The open static bodies of the count, innermost last: each program
    /// index with the position of its next operation.
    struct StaticFrames {
        frames: Vec<(usize, usize)>,
    }
    /// Enter static program `index`: `false` when it was already counted,
    /// otherwise mark it and charge its cell and its operation graph.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn static_enter(
        index: usize,
        programs: &[OriginProgram],
        seen: &mut [bool],
        total: &mut usize,
    ) -> Option<bool> {
        if *seen.get(index)? {
            return Some(false);
        }
        match seen.get_mut(index) {
            Some(is_seen) => *is_seen = true,
            None => return None,
        }
        charge(total, 1)?;
        let program_ops = &programs.get(index)?.operations;
        charge(total, program_ops.len().checked_mul(2)?.checked_sub(1)?)?;
        Some(true)
    }
    /// Advance the count by one event: enter the static program the innermost
    /// body's next operation installs, or close that body once it is done.
    /// `Some(false)` once the first body has closed.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn static_step(
        programs: &[OriginProgram],
        seen: &mut [bool],
        total: &mut usize,
        frames: &mut StaticFrames,
    ) -> Option<bool> {
        let top = frames.frames.len() - 1;
        let (index, position) = frames.frames[top];
        let program_ops = &programs[index].operations;
        if position >= program_ops.len() {
            frames.frames.truncate(top);
            return Some(top != 0);
        }
        frames.frames[top].1 += 1;
        let action = program_ops[position].action;
        let OriginAction::StaticProgram(child) = action else {
            return Some(true);
        };
        if static_enter(child, programs, seen, total)? {
            frames.frames.push((child, 0));
        }
        Some(true)
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

/// One bounded derivation of a returned template's logical source-binding
/// classes, ordered root captures and recursive-owner classification.
struct TemplateWalk {
    /// Distinct retained cells in first-visit order, with their binding keys.
    cells: Vec<(usize, alloc::string::String)>,
    classes: Vec<OriginClass>,
    captures: Vec<u32>,
    owner: OriginOwner,
    /// Normalized recipe keys of static quotation bodies, by program index.
    static_keys: Vec<Option<alloc::string::String>>,
    static_visiting: Vec<bool>,
}

/// Canonical structural spelling of a fixed I64 source recipe: `#2` for a
/// literal and `add(#1,#1)` for checked input-independent arithmetic.
#[expect(
    tigerstyle::mutating_input_in_pure,
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; the recursion follows the structure of an already checked, size-bounded fixed recipe, and the node meter is the caller's bounded derivation state."
)]
fn recipe_binding(
    recipe: &Rc<OriginFixedExpression>,
    depth: usize,
    nodes: &mut usize,
) -> Option<alloc::string::String> {
    *nodes = nodes.checked_add(1)?;
    if *nodes > MAX_ORIGIN_OPERATIONS || depth > MAX_ORIGIN_DEPTH {
        return None;
    }
    match recipe.as_ref() {
        OriginFixedExpression::Literal(value) => Some(alloc::format!("#{value}")),
        OriginFixedExpression::Arithmetic {
            kind, left, right, ..
        } => {
            let kind = match kind {
                OriginFixedArithmetic::Add => "add",
                OriginFixedArithmetic::Sub => "sub",
                OriginFixedArithmetic::Mul => "mul",
            };
            Some(alloc::format!(
                "{kind}({},{})",
                recipe_binding(left, depth + 1, nodes)?,
                recipe_binding(right, depth + 1, nodes)?
            ))
        }
    }
}

/// Canonical structural spelling of one retained Program cell's logical
/// source binding. A revisited cell keeps the key of its first occurrence, so
/// physical sharing never changes the key.
#[expect(
    tigerstyle::function_length,
    tigerstyle::mutating_input_in_pure,
    tigerstyle::no_recursion,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the walk keeps one ordered derivation of captures, classes and owner in a single reviewable unit; it mutates only its own exclusively borrowed work state, the recursion follows an already checked size-bounded returned graph, and the parameters are the four independent checked inputs plus the shared bound state threaded explicitly."
)]
fn template_key(
    value: &Rc<OriginProgramValue>,
    programs: &[OriginProgram],
    named: usize,
    walk: &mut TemplateWalk,
    depth: usize,
    fixed_nodes: &mut usize,
) -> Option<alloc::string::String> {
    if depth >= MAX_ORIGIN_DEPTH {
        return None;
    }
    let pointer = Rc::as_ptr(value) as usize;
    let mut known = 0usize;
    while known < walk.cells.len() {
        if walk.cells[known].0 == pointer {
            return Some(walk.cells[known].1.clone());
        }
        known += 1;
    }
    let key = match value.as_ref() {
        OriginProgramValue::Static { program_index, .. } => {
            let program = programs.get(*program_index)?;
            if program.source_artifact != OriginSource::Definition {
                return None;
            }
            if *program_index == named {
                // The enclosing named definition is reached from inside its own
                // returned graph: a required lexical recursive owner.
                walk.owner = OriginOwner::SelfNamed;
                alloc::string::String::from("p:named")
            } else {
                program.body_node?;
                static_program_key(*program_index, programs, walk)?
            }
        }
        OriginProgramValue::Quote {
            operand: OriginOperand::I64(OriginI64::RootInput(position)),
            ..
        } => {
            if !walk.captures.contains(position) {
                walk.captures.push(*position);
            }
            alloc::format!("q(r{position})")
        }
        OriginProgramValue::Quote {
            operand: OriginOperand::I64(OriginI64::Fixed { recipe, .. }),
            ..
        } => {
            alloc::format!("q({})", recipe_binding(recipe, 0, fixed_nodes)?)
        }
        OriginProgramValue::Quote {
            operand: OriginOperand::Program(child),
            ..
        } => {
            let mut key = alloc::string::String::from("q(");
            key.push_str(&template_key(
                child,
                programs,
                named,
                walk,
                depth + 1,
                fixed_nodes,
            )?);
            key.push(')');
            key
        }
        OriginProgramValue::Compose { left, right, .. } => {
            let mut key = alloc::string::String::from("c(");
            key.push_str(&template_key(
                left,
                programs,
                named,
                walk,
                depth + 1,
                fixed_nodes,
            )?);
            key.push(',');
            key.push_str(&template_key(
                right,
                programs,
                named,
                walk,
                depth + 1,
                fixed_nodes,
            )?);
            key.push(')');
            key
        }
    };
    let mut class = 0usize;
    while class < walk.classes.len() {
        if walk.classes[class].binding == key {
            walk.classes[class].occurrences = walk.classes[class].occurrences.checked_add(1)?;
            break;
        }
        class += 1;
    }
    if class == walk.classes.len() {
        if walk.classes.len() >= MAX_ORIGIN_PROGRAMS {
            return None;
        }
        walk.classes.push(OriginClass {
            binding: key.clone(),
            occurrences: 1,
        });
    }
    walk.cells.push((pointer, key.clone()));
    Some(key)
}

/// Canonical normalized recipe spelling of one static quotation body, resolved
/// by content after recipe normalization rather than by compiled program index.
/// Two independently written but equal quotation bodies therefore share one
/// logical binding class.
#[expect(
    tigerstyle::mutating_input_in_pure,
    tigerstyle::no_recursion,
    reason = "Owner: noble-wasm-maintainers; it mutates only its own exclusively borrowed work state, and the recursion follows an already checked, acyclic size-bounded quotation graph."
)]
fn static_program_key(
    index: usize,
    programs: &[OriginProgram],
    walk: &mut TemplateWalk,
) -> Option<alloc::string::String> {
    if let Some(key) = walk.static_keys.get(index).and_then(|slot| slot.clone()) {
        return Some(key);
    }
    if *walk.static_visiting.get(index)? {
        return None;
    }
    let program = programs.get(index)?;
    if program.source_artifact != OriginSource::Definition
        || program.body_node.is_none()
        || program.operations.len() > MAX_ORIGIN_OPERATIONS
    {
        return None;
    }
    *walk.static_visiting.get_mut(index)? = true;
    let mut key = alloc::format!("p{}:{}[", program.input_signature, program.output_signature);
    for (at, operation) in program.operations.iter().enumerate() {
        if at > 0 {
            key.push(';');
        }
        match operation.action {
            OriginAction::I64(value) => key.push_str(&alloc::format!("#{value}")),
            OriginAction::Add => key.push_str("add"),
            OriginAction::Sub => key.push_str("sub"),
            OriginAction::Mul => key.push_str("mul"),
            OriginAction::Dup => key.push_str("dup"),
            OriginAction::Drop => key.push_str("drop"),
            OriginAction::Swap => key.push_str("swap"),
            OriginAction::Dip => key.push_str("dip"),
            OriginAction::Run => key.push_str("run"),
            OriginAction::Compose => key.push_str("compose"),
            OriginAction::Quote { .. } => key.push_str("quote"),
            OriginAction::StaticProgram(child) => {
                key.push_str(&static_program_key(child, programs, walk)?);
            }
            OriginAction::CallSelected(_) => return None,
        }
    }
    key.push(']');
    *walk.static_visiting.get_mut(index)? = false;
    if let Some(slot) = walk.static_keys.get_mut(index) {
        *slot = Some(key.clone());
    }
    Some(key)
}

/// Exact bounded template projection of one returned value.
fn template_projection(
    value: &Rc<OriginProgramValue>,
    programs: &[OriginProgram],
    named: usize,
) -> Option<OriginTemplate> {
    let mut walk = TemplateWalk {
        cells: Vec::with_capacity(MAX_ORIGIN_PROGRAMS),
        classes: Vec::with_capacity(MAX_ORIGIN_PROGRAMS),
        captures: Vec::with_capacity(MAX_ORIGIN_OPERATIONS.min(16)),
        owner: OriginOwner::Absent,
        static_keys: alloc::vec![None; programs.len()],
        static_visiting: alloc::vec![false; programs.len()],
    };
    template_key(value, programs, named, &mut walk, 0, &mut 0)?;
    walk.classes
        .sort_by(|left, right| left.binding.cmp(&right.binding));
    Some(OriginTemplate {
        captures: walk.captures,
        classes: walk.classes,
        owner: walk.owner,
    })
}

/// Whether every retained output re-derives to exactly its stored template
/// projection and requires no lexical recursive owner. A stored projection that
/// is absent, forged, truncated or stale is refused.
pub(super) fn verified_templates(origin: &SelectedOriginMetadata) -> bool {
    if origin.outputs.is_empty() || origin.outputs.len() > MAX_ORIGIN_PROGRAMS {
        return false;
    }
    origin.outputs.iter().all(|output| {
        output.template.owner == OriginOwner::Absent
            && output.template.captures.len() <= MAX_ORIGIN_OPERATIONS
            && output.template.classes.len() <= MAX_ORIGIN_PROGRAMS
            && template_projection(&output.result, &origin.programs, origin.named_program_index)
                .is_some_and(|template| {
                    template.owner == OriginOwner::Absent
                        && template.captures == output.template.captures
                        && template.classes == output.template.classes
                })
    })
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
    // A reachable reference to the enclosing named definition requires a
    // semantic lexical recursive owner this bounded subset does not classify.
    if outputs
        .iter()
        .any(|output| output.template.owner != OriginOwner::Absent)
    {
        return None;
    }
    let origin = SelectedOriginMetadata {
        definition_id: selected.definition_id(),
        source_generation: selected.source_generation(),
        named_program_index,
        root_program_index: plan.root,
        programs,
        outputs,
    };
    // Fail closed on any projection that does not re-derive exactly.
    verified_templates(&origin).then_some(origin)
}

// Test harness only: the recursive forms that the explicit-stack walks
// replaced are kept as differential oracles, and fixtures are built inline.
#[allow(
    tigerstyle::function_length,
    tigerstyle::no_recursion,
    tigerstyle::numeric_units,
    reason = "Owner: noble-wasm-maintainers; test harness: the oracles are the replaced recursive walks, and fixture tables are spelled out in full."
)]
#[cfg(test)]
mod walk_order_tests {
    fn operation(action: super::OriginAction) -> super::OriginOperation {
        super::OriginOperation {
            node: noble_kernel::untrusted::NodeId(1),
            lexical_node: None,
            authored_span: None,
            generated: false,
            generated_witness: None,
            ordinal: 0,
            table_entry: 3,
            input_signature: 0,
            output_signature: 0,
            effect_mask: 0,
            action,
        }
    }

    fn table(
        bodies: &[alloc::vec::Vec<super::OriginAction>],
    ) -> alloc::vec::Vec<super::OriginProgram> {
        let mut programs = alloc::vec::Vec::with_capacity(bodies.len());
        for (program_index, body) in bodies.iter().enumerate() {
            programs.push(super::OriginProgram {
                source_artifact: super::OriginSource::Definition,
                program_index,
                body_node: None,
                entry: 0,
                input_signature: 0,
                output_signature: 0,
                effect_mask: 0,
                operations: body.iter().map(|action| operation(*action)).collect(),
            });
        }
        programs
    }

    /// A real lowered layout with 41 programs, so every fixture program index
    /// has an installed depth to read.
    fn layout() -> super::Layout {
        let mut text = alloc::string::String::new();
        for _ in 0..40 {
            text.push_str("[ 1 ] drop ");
        }
        let limits = noble_contracts::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let session = noble_contracts::source::Session::new();
        let prepared = session
            .prepare(text.as_bytes(), &[], limits)
            .unwrap_or_else(|_| panic!("fixture source refused"));
        let submission = prepared
            .submission()
            .unwrap_or_else(|| panic!("missing fixture submission"));
        let compiler = super::super::Compiler::new();
        let (_, plan) = compiler
            .prepare_layout(submission, compiler.functions, false, false, false)
            .unwrap_or_else(|_| panic!("fixture lowering refused"));
        assert!(
            plan.programs.len() > 40,
            "the fixture layout has 41 programs"
        );
        plan
    }

    fn machine(programs: &[super::OriginProgram]) -> super::Machine {
        super::Machine {
            stack: alloc::vec![
                super::Value::I64(super::OriginI64::RootInput(0)),
                super::Value::I64(super::OriginI64::RootInput(1)),
            ],
            static_programs: alloc::vec![None; programs.len()],
            steps: 0,
            active: [0; super::MAX_ORIGIN_DEPTH],
            frames: alloc::vec::Vec::with_capacity(super::MAX_ORIGIN_DEPTH),
        }
    }

    fn rendered(machine: &super::Machine) -> alloc::string::String {
        let mut text = alloc::string::String::new();
        for value in &machine.stack {
            match value {
                super::Value::I64(value) => text.push_str(&alloc::format!("I64({value:?}) ")),
                super::Value::Program { result, depth } => {
                    text.push_str(&alloc::format!("Program({result:?}, {depth}) "));
                }
            }
        }
        text.push_str(&alloc::format!(
            "| steps {} | active {:?} | cache {:?}",
            machine.steps,
            machine.active,
            machine.static_programs
        ));
        text
    }

    /// The replaced recursive replay: a `dip` runs its program as a nested
    /// call, then restores the set-aside value and checks the stack bound.
    fn recursive_execute(
        programs: &[super::OriginProgram],
        plan: &super::Layout,
        index: usize,
        level: usize,
        machine: &mut super::Machine,
    ) -> Option<()> {
        if level >= super::MAX_ORIGIN_DEPTH || machine.active[..level].contains(&index) {
            return None;
        }
        machine.active[level] = index;
        let named = programs.get(index)?;
        for operation in &named.operations {
            machine.steps = machine.steps.checked_add(1)?;
            if machine.steps > super::MAX_REPLAY_STEPS {
                return None;
            }
            match super::apply_operation(plan, operation, machine)? {
                super::Applied::Done => {}
                super::Applied::Dip { child, hidden } => {
                    recursive_execute(programs, plan, child, level + 1, machine)?;
                    machine.stack.push(hidden);
                }
            }
            if machine.stack.len() > super::MAX_ORIGIN_OPERATIONS {
                return None;
            }
        }
        Some(())
    }

    /// Both replays agree on the result and the whole machine state; returns the result.
    fn assert_replays_agree(
        name: &str,
        plan: &super::Layout,
        bodies: &[alloc::vec::Vec<super::OriginAction>],
        root: usize,
    ) -> Option<()> {
        let programs = table(bodies);
        let mut expected = machine(&programs);
        let expected_result = recursive_execute(&programs, plan, root, 0, &mut expected);
        let mut actual = machine(&programs);
        let actual_result = super::execute(&programs, plan, root, &mut actual);
        assert_eq!(actual_result, expected_result, "{name}: result");
        assert_eq!(rendered(&actual), rendered(&expected), "{name}: state");
        actual_result
    }

    #[test]
    fn explicit_frame_replay_matches_the_recursive_replay() {
        let int = super::OriginAction::I64;
        let install = super::OriginAction::StaticProgram;
        let dip = super::OriginAction::Dip;
        let discard = super::OriginAction::Drop;
        let add = super::OriginAction::Add;
        let dup = super::OriginAction::Dup;
        let swap = super::OriginAction::Swap;
        let compose = super::OriginAction::Compose;
        let plan = layout();
        let quote = super::OriginAction::Quote {
            input: 0,
            output: 0,
            witness: 0,
            program_operand: false,
        };
        let completed = [
            assert_replays_agree("arithmetic", &plan, &[alloc::vec![int(1), int(2), add]], 0),
            assert_replays_agree(
                "dip",
                &plan,
                &[
                    alloc::vec![int(5), install(1), dip, dup],
                    alloc::vec![int(7), int(8), add, swap],
                ],
                0,
            ),
            assert_replays_agree(
                "nested dips",
                &plan,
                &[
                    alloc::vec![int(1), install(1), dip, int(9)],
                    alloc::vec![int(2), install(2), dip, int(3)],
                    alloc::vec![int(4), install(3), install(3), compose],
                    alloc::vec![int(6)],
                ],
                0,
            ),
            assert_replays_agree(
                "repeated dip of one program",
                &plan,
                &[
                    alloc::vec![int(1), install(1), dip, int(2), install(1), dip, discard],
                    alloc::vec![int(3), discard],
                ],
                0,
            ),
        ];
        assert!(
            completed.iter().all(Option::is_some),
            "the well-formed fixtures replay to completion"
        );
        let mut refused = alloc::vec![assert_replays_agree(
            "dip cycle",
            &plan,
            &[
                alloc::vec![int(1), install(1), dip],
                alloc::vec![int(2), install(1), dip],
            ],
            0,
        )];
        let mut chain = alloc::vec::Vec::with_capacity(35);
        for index in 0..34 {
            chain.push(alloc::vec![int(index), install(index as usize + 1), dip]);
        }
        chain.push(alloc::vec![int(0)]);
        refused.push(assert_replays_agree("depth bound", &plan, &chain, 0));
        let mut busy = alloc::vec::Vec::with_capacity(200);
        for _ in 0..50 {
            busy.extend_from_slice(&[int(1), install(1), dip, discard]);
        }
        let mut body = alloc::vec::Vec::with_capacity(200);
        for _ in 0..100 {
            body.extend_from_slice(&[int(1), discard]);
        }
        refused.push(assert_replays_agree("step bound", &plan, &[busy, body], 0));
        let mut tall = alloc::vec::Vec::with_capacity(127);
        for value in 0..124 {
            tall.push(int(value));
        }
        tall.extend_from_slice(&[int(1), install(1), dip]);
        refused.push(assert_replays_agree(
            "stack bound after a dip",
            &plan,
            &[tall, alloc::vec![int(2), int(3)]],
            0,
        ));
        let mut overfull = alloc::vec::Vec::with_capacity(130);
        for value in 0..130 {
            overfull.push(int(value));
        }
        refused.push(assert_replays_agree("stack bound", &plan, &[overfull], 0));
        refused.push(assert_replays_agree(
            "non-static dip helper",
            &plan,
            &[alloc::vec![int(1), int(2), quote, dip]],
            0,
        ));
        refused.push(assert_replays_agree(
            "refusal inside a dip",
            &plan,
            &[alloc::vec![int(1), install(1), dip], alloc::vec![add]],
            0,
        ));
        refused.push(assert_replays_agree(
            "unknown root",
            &plan,
            &[alloc::vec![int(1)]],
            1,
        ));
        assert!(
            refused.iter().all(Option::is_none),
            "every bound, cycle and malformed fixture is refused"
        );
    }

    /// The replaced recursive static count over the same cell cap.
    fn recursive_static(
        index: usize,
        programs: &[super::OriginProgram],
        seen: &mut [bool],
        total: &mut usize,
    ) -> Option<()> {
        fn charge(total: &mut usize, cells: usize) -> Option<()> {
            *total = total.checked_add(cells)?;
            (*total <= super::MAX_ORIGIN_PROGRAMS).then_some(())
        }
        if *seen.get(index)? {
            return Some(());
        }
        seen[index] = true;
        charge(total, 1)?;
        let program_ops = &programs.get(index)?.operations;
        charge(total, program_ops.len().checked_mul(2)?.checked_sub(1)?)?;
        for operation in program_ops {
            if let super::OriginAction::StaticProgram(child) = operation.action {
                recursive_static(child, programs, seen, total)?;
            }
        }
        Some(())
    }

    /// Both counts agree on the result, the seen set and the cells; returns the result.
    fn assert_counts_agree(
        name: &str,
        bodies: &[alloc::vec::Vec<super::OriginAction>],
    ) -> Option<()> {
        let programs = table(bodies);
        let mut expected_seen = alloc::vec![false; programs.len()];
        let mut expected_total = 0usize;
        let expected = recursive_static(0, &programs, &mut expected_seen, &mut expected_total);
        let mut seen = alloc::vec![false; programs.len()];
        let mut total = 0usize;
        let mut constructed: [Option<alloc::rc::Rc<super::OriginProgramValue>>;
            super::MAX_ORIGIN_PROGRAMS] = core::array::from_fn(|_| None);
        let mut constructed_count = 0usize;
        let mut fixed_nodes = 0usize;
        let root = alloc::rc::Rc::new(super::OriginProgramValue::Static {
            node: noble_kernel::untrusted::NodeId(1),
            program_index: 0,
        });
        let actual = super::bounded_graph_cells(
            &root,
            &programs,
            &mut constructed,
            &mut constructed_count,
            &mut seen,
            &mut total,
            &mut fixed_nodes,
        );
        assert_eq!(actual, expected, "{name}: result");
        assert_eq!(seen, expected_seen, "{name}: seen");
        assert_eq!(total, expected_total, "{name}: cells");
        actual
    }

    #[test]
    fn explicit_frame_static_count_matches_the_recursive_count() {
        let int = super::OriginAction::I64;
        let install = super::OriginAction::StaticProgram;
        let discard = super::OriginAction::Drop;
        let counted = [
            assert_counts_agree("single", &[alloc::vec![int(1)]]),
            assert_counts_agree(
                "diamond",
                &[
                    alloc::vec![install(1), install(2)],
                    alloc::vec![install(2)],
                    alloc::vec![int(1)],
                ],
            ),
            assert_counts_agree(
                "cycle",
                &[alloc::vec![install(1), discard], alloc::vec![install(0)]],
            ),
        ];
        assert!(
            counted.iter().all(Option::is_some),
            "the bounded fixtures are counted"
        );
        let mut chain = alloc::vec::Vec::with_capacity(21);
        for index in 0..20 {
            chain.push(alloc::vec![install(index + 1)]);
        }
        chain.push(alloc::vec![int(1)]);
        let refused = [
            assert_counts_agree("empty body", &[alloc::vec![install(1)], alloc::vec![]]),
            assert_counts_agree("unknown child", &[alloc::vec![int(1), install(5)]]),
            assert_counts_agree("cell cap", &chain),
        ];
        assert!(
            refused.iter().all(Option::is_none),
            "empty, unknown and over-cap fixtures are refused"
        );
    }

    #[test]
    fn template_projection_requires_a_classified_recursive_owner() {
        let mut programs = table(&[alloc::vec::Vec::new(), alloc::vec::Vec::new()]);
        programs[0].source_artifact = super::OriginSource::Selection;
        let named = 1usize;
        let self_reference = alloc::rc::Rc::new(super::OriginProgramValue::Static {
            node: noble_kernel::untrusted::NodeId(2),
            program_index: named,
        });
        let owner = super::template_projection(&self_reference, &programs, named)
            .map(|template| template.owner);
        assert_eq!(
            owner,
            Some(super::OriginOwner::SelfNamed),
            "a reachable reference to the enclosing named definition is a required recursive owner"
        );
        let quotation = alloc::rc::Rc::new(super::OriginProgramValue::Quote {
            node: noble_kernel::untrusted::NodeId(3),
            table_entry: 3,
            operand: super::OriginOperand::I64(super::OriginI64::RootInput(0)),
        });
        let owner =
            super::template_projection(&quotation, &programs, named).map(|template| template.owner);
        assert_eq!(
            owner,
            Some(super::OriginOwner::Absent),
            "a bounded root capture requires no lexical recursive owner"
        );
    }
}
