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

fn action(
    selected: &CheckedSelectedTarget<'_>,
    plan: &Layout,
    program_index: usize,
    operation: &super::plan::Operation,
) -> Option<OriginAction> {
    let result = match operation.action {
        Action::I64(value) => OriginAction::I64(value),
        Action::Word(0) => OriginAction::Dup,
        Action::Word(1) => OriginAction::Drop,
        Action::Word(2) => OriginAction::Swap,
        Action::Word(3) if selected.is_generated_operation(operation.node) => OriginAction::Dip,
        Action::Word(4) => OriginAction::Add,
        Action::Word(5) => OriginAction::Sub,
        Action::Word(6) => OriginAction::Mul,
        Action::Word(10) => OriginAction::Run,
        Action::Word(9) => OriginAction::Compose,
        Action::Program(index) if index < plan.programs.len() => OriginAction::StaticProgram(index),
        Action::Quote(input, output, witness) if program_index != plan.root => {
            let site = selected.checked_quote_site(operation.node).ok()?;
            let is_program_operand = match site.interface().stack_in.last()? {
                Ty::I64 => false,
                Ty::Program(_, _, effects) if effects.is_empty() => true,
                _ => return None,
            };
            OriginAction::Quote {
                input,
                output,
                witness,
                program_operand: is_program_operand,
            }
        }
        Action::Call(index, identity)
            if program_index == plan.root
                && identity == selected.definition_id().identity()
                && plan.definition_roots.first() == Some(&index) =>
        {
            OriginAction::CallSelected(index)
        }
        _ => return None,
    };
    Some(result)
}

#[expect(
    tigerstyle::compound_condition,
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation; the function keeps one ordered pass in a single reviewable unit; splitting it would separate each step from its bounds checks."
)]
fn programs(selected: &CheckedSelectedTarget<'_>, plan: &Layout) -> Option<Vec<OriginProgram>> {
    if plan.programs.len() > MAX_ORIGIN_PROGRAMS || plan.programs.is_empty() {
        return None;
    }
    let mut count = 0usize;
    let mut result = Vec::with_capacity(plan.programs.len());
    for (program_index, program) in plan.programs.iter().enumerate() {
        if program.operations.is_empty()
            || program.effects != 0
            || program.depth > MAX_ORIGIN_DEPTH as u32
            || program.leaves > MAX_ORIGIN_OPERATIONS as u32
        {
            return None;
        }
        count = count.checked_add(program.operations.len())?;
        if count > MAX_ORIGIN_OPERATIONS {
            return None;
        }
        let source_artifact = if program_index == plan.root {
            OriginSource::Selection
        } else {
            OriginSource::Definition
        };
        if source_artifact == OriginSource::Selection
            && (program.operations.len() != 1
                || !matches!(program.operations[0].action, Action::Call(..)))
        {
            return None;
        }
        let mut operations = Vec::with_capacity(program.operations.len());
        for (ordinal, operation) in program.operations.iter().enumerate() {
            let ordinal = u32::try_from(ordinal).ok()?;
            let table_entry = program.entry.checked_add(ordinal)?;
            if table_entry < plan.first_function
                || table_entry >= plan.functions
                || operations
                    .iter()
                    .any(|prior: &OriginOperation| prior.node == operation.node)
            {
                return None;
            }
            if operation.effects != 0 {
                return None;
            }
            let action = action(selected, plan, program_index, operation)?;
            let site = if source_artifact == OriginSource::Selection {
                if operation.node != NodeId(0) || !matches!(action, OriginAction::CallSelected(..))
                {
                    return None;
                }
                None
            } else {
                Some(selected.checked_operation_site(operation.node).ok()?)
            };
            operations.push(OriginOperation {
                node: operation.node,
                lexical_node: site.as_ref().map(|site| site.lexical_node),
                authored_span: site.as_ref().and_then(|site| site.authored_span),
                generated: site
                    .as_ref()
                    .is_some_and(|site| site.generated_witness.is_some()),
                generated_witness: site.and_then(|site| site.generated_witness),
                ordinal,
                table_entry,
                input_signature: operation.input,
                output_signature: operation.output,
                effect_mask: operation.effects,
                action,
            });
        }
        result.push(OriginProgram {
            source_artifact,
            program_index,
            entry: program.entry,
            body_node: None,
            input_signature: program.input,
            output_signature: program.output,
            effect_mask: program.effects,
            operations,
        });
    }
    let mut body_nodes = alloc::vec![None; result.len()];
    for program in &result {
        for operation in &program.operations {
            if let OriginAction::StaticProgram(index) = operation.action {
                let node = operation.lexical_node?;
                if !matches!(
                    selected.checked_lexical_node(node).ok()?.kind,
                    CheckedLexicalKind::Quotation(_)
                ) || body_nodes.get_mut(index)?.replace(node).is_some()
                {
                    return None;
                }
            }
        }
    }
    let named = plan.definition_roots[0];
    for program in &mut result {
        let lexical_body = if program.program_index == plan.root {
            if body_nodes[program.program_index].is_some() {
                return None;
            }
            continue;
        } else if program.program_index == named {
            if body_nodes[program.program_index].is_some() {
                return None;
            }
            selected.lexical_body()
        } else {
            let body_node = body_nodes[program.program_index]?;
            program.body_node = Some(body_node);
            let CheckedLexicalKind::Quotation(body) =
                selected.checked_lexical_node(body_node).ok()?.kind
            else {
                return None;
            };
            body
        };
        if lexical_body.len() != program.operations.len()
            || !lexical_body
                .iter()
                .zip(&program.operations)
                .all(|(lexical, operation)| operation.lexical_node == Some(*lexical))
        {
            return None;
        }
    }
    Some(result)
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
    tigerstyle::function_length,
    tigerstyle::mutating_input_in_pure,
    tigerstyle::no_recursion,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-wasm-maintainers; the function keeps one ordered pass in a single reviewable unit; splitting it would separate each step from its bounds checks; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the recursion follows the structure of an already checked, size-bounded input; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
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
    for operation in &named.operations {
        *steps = (*steps).checked_add(1)?;
        if *steps > MAX_REPLAY_STEPS {
            return None;
        }
        match operation.action {
            OriginAction::I64(value) => stack.push(Value::I64(OriginI64::Fixed {
                value,
                recipe: Rc::new(OriginFixedExpression::Literal(value)),
            })),
            OriginAction::Dup => stack.push(stack.last()?.clone()),
            OriginAction::Drop => {
                stack.pop()?;
            }
            OriginAction::Swap => {
                let len = stack.len();
                if len < 2 {
                    return None;
                }
                stack.swap(len - 1, len - 2);
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
                let (value, kind) = match operation.action {
                    OriginAction::Add => (left.wrapping_add(right), OriginFixedArithmetic::Add),
                    OriginAction::Sub => (left.wrapping_sub(right), OriginFixedArithmetic::Sub),
                    OriginAction::Mul => (left.wrapping_mul(right), OriginFixedArithmetic::Mul),
                    _ => return None,
                };
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
            }
            OriginAction::Quote {
                program_operand, ..
            } => {
                let (operand, depth) = if program_operand {
                    let (operand, depth) = program_value(stack.pop()?)?;
                    (OriginOperand::Program(operand), depth.checked_add(1)?)
                } else {
                    let Value::I64(operand) = stack.pop()? else {
                        return None;
                    };
                    (OriginOperand::I64(operand), 1)
                };
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
            }
            _ => return None,
        }
        if stack.len() > MAX_ORIGIN_OPERATIONS {
            return None;
        }
    }
    Some(())
}

#[expect(
    tigerstyle::compound_condition,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-wasm-maintainers; each compound guard states one ordered admission rule verbatim; extracting its operands would change short-circuit evaluation; growth is bounded by the checked input or admission limits that drive the loop."
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
        || inputs.iter().any(|ty| *ty != Ty::I64)
        || !selected.body().interface.effects.is_empty()
    {
        return None;
    }
    let named = programs.get(named_index)?;
    let mut stack = Vec::with_capacity(inputs.len().saturating_add(named.operations.len()));
    for position in 0..inputs.len() {
        stack.push(Value::I64(OriginI64::RootInput(
            u32::try_from(position).ok()?,
        )));
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
    for (stack_position, (ty, value)) in outputs.iter().zip(stack).enumerate() {
        if matches!(ty, Ty::Program(..)) {
            let (result_value, _) = program_value(value)?;
            if matches!(result_value.as_ref(), OriginProgramValue::Static { .. }) {
                // An installed static Program already has its own pinned
                // source/index owner; it is not a constructed returned target.
                return None;
            }
            result.push(OriginOutput {
                stack_position,
                result: result_value,
            });
        } else if *ty != Ty::I64 || !matches!(value, Value::I64(_)) {
            return None;
        }
    }
    // Cross-output aliasing requires a single graph receipt spanning multiple
    // independently saved owners. Do not claim identity for that profile.
    if result.len() != 1 {
        return None;
    }
    Some(result)
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
    constructed: &mut [usize; MAX_ORIGIN_PROGRAMS],
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
    #[expect(
        tigerstyle::mutating_input_in_pure,
        tigerstyle::no_recursion,
        reason = "Owner: noble-wasm-maintainers; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the recursion follows the structure of an already checked, size-bounded input."
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
        for operation in operations {
            if let OriginAction::StaticProgram(child) = operation.action {
                static_body(child, programs, seen, total)?;
            }
        }
        Some(())
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
        if let OriginFixedExpression::Arithmetic { left, right, .. } = recipe.as_ref() {
            fixed_recipe(left, depth + 1, nodes)?;
            fixed_recipe(right, depth + 1, nodes)?;
        }
        Some(())
    }
    match value.as_ref() {
        OriginProgramValue::Static { program_index, .. } => {
            static_body(*program_index, programs, static_programs, total)
        }
        OriginProgramValue::Quote { operand, .. } => {
            let pointer = Rc::as_ptr(value) as usize;
            if constructed[..*constructed_count].contains(&pointer) {
                return Some(());
            }
            *constructed.get_mut(*constructed_count)? = pointer;
            *constructed_count += 1;
            match operand {
                OriginOperand::I64(scalar) => {
                    if let OriginI64::Fixed { recipe, .. } = scalar {
                        fixed_recipe(recipe, 1, fixed_nodes)?;
                    }
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
            let pointer = Rc::as_ptr(value) as usize;
            if constructed[..*constructed_count].contains(&pointer) {
                return Some(());
            }
            *constructed.get_mut(*constructed_count)? = pointer;
            *constructed_count += 1;
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
    let [definition] = selected.submission().definitions.as_slice() else {
        return None;
    };
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
    let mut constructed = [0; MAX_ORIGIN_PROGRAMS];
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
