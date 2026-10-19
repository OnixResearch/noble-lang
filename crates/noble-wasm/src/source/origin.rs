//! D-neutral lineage for an independently checked selected, linear source body.
//! Unsupported source shapes return no metadata; they keep ordinary Program
//! execution and saved-owner retention, without an invented target identity.

const MAX_ORIGIN_PROGRAMS: usize = 32;
const MAX_ORIGIN_OPERATIONS: usize = 128;
const MAX_ORIGIN_DEPTH: usize = 32;

enum Value {
    I64(super::OriginI64),
    Static {
        node: noble_kernel::untrusted::NodeId,
        program: usize,
    },
    Dynamic {
        result: alloc::boxed::Box<super::OriginProgramValue>,
        depth: usize,
    },
}

fn action(
    selected: &noble_contracts::source::proof::CheckedSelectedTarget<'_>,
    plan: &super::plan::Layout,
    program_index: usize,
    operation: &super::plan::Operation,
) -> Option<super::OriginAction> {
    let result = match operation.action {
        super::plan::Action::I64(value) => super::OriginAction::I64(value),
        super::plan::Action::Word(0) => super::OriginAction::Dup,
        super::plan::Action::Word(1) => super::OriginAction::Drop,
        super::plan::Action::Word(2) => super::OriginAction::Swap,
        super::plan::Action::Word(4) => super::OriginAction::Add,
        super::plan::Action::Word(5) => super::OriginAction::Sub,
        super::plan::Action::Word(6) => super::OriginAction::Mul,
        super::plan::Action::Word(9) => super::OriginAction::Compose,
        super::plan::Action::Program(index) if index < plan.programs.len() => {
            super::OriginAction::StaticProgram(index)
        }
        super::plan::Action::Quote(input, output, witness) if program_index != plan.root => {
            let site = selected.checked_quote_site(operation.node).ok()?;
            if site.node() != operation.node
                || site.interface().stack_in.last() != Some(&noble_kernel::types::Ty::I64)
            {
                return None;
            }
            super::OriginAction::QuoteI64 {
                input,
                output,
                witness,
            }
        }
        super::plan::Action::Call(index, identity)
            if program_index == plan.root
                && identity == selected.definition_id().identity()
                && plan.definition_roots.first() == Some(&index) =>
        {
            super::OriginAction::CallSelected(index)
        }
        _ => return None,
    };
    Some(result)
}

fn programs(
    selected: &noble_contracts::source::proof::CheckedSelectedTarget<'_>,
    plan: &super::plan::Layout,
) -> Option<alloc::vec::Vec<super::OriginProgram>> {
    if plan.programs.len() > MAX_ORIGIN_PROGRAMS || plan.programs.is_empty() {
        return None;
    }
    let mut count = 0usize;
    let mut result = alloc::vec::Vec::with_capacity(plan.programs.len());
    for (program_index, program) in plan.programs.iter().enumerate() {
        if program.operations.is_empty() {
            return None;
        }
        count = count.checked_add(program.operations.len())?;
        if count > MAX_ORIGIN_OPERATIONS {
            return None;
        }
        let source_artifact = if program_index == plan.root {
            super::OriginSource::Selection
        } else {
            super::OriginSource::Definition
        };
        if source_artifact == super::OriginSource::Selection
            && (program.operations.len() != 1
                || !matches!(program.operations[0].action, super::plan::Action::Call(..)))
        {
            return None;
        }
        let mut operations = alloc::vec::Vec::with_capacity(program.operations.len());
        for (ordinal, operation) in program.operations.iter().enumerate() {
            let ordinal = u32::try_from(ordinal).ok()?;
            let table_entry = program.entry.checked_add(ordinal)?;
            if table_entry < plan.first_function
                || table_entry >= plan.functions
                || operations
                    .iter()
                    .any(|prior: &super::OriginOperation| prior.node == operation.node)
            {
                return None;
            }
            let action = action(selected, plan, program_index, operation)?;
            let source_span = if source_artifact == super::OriginSource::Selection {
                if operation.node != noble_kernel::untrusted::NodeId(0)
                    || !matches!(action, super::OriginAction::CallSelected(..))
                {
                    return None;
                }
                None
            } else {
                Some(selected.checked_operation_span(operation.node).ok()?)
            };
            operations.push(super::OriginOperation {
                node: operation.node,
                source_span,
                ordinal,
                table_entry,
                input_signature: operation.input,
                output_signature: operation.output,
                effect_mask: operation.effects,
                action,
            });
        }
        result.push(super::OriginProgram {
            source_artifact,
            program_index,
            entry: program.entry,
            input_signature: program.input,
            output_signature: program.output,
            effect_mask: program.effects,
            operations,
        });
    }
    Some(result)
}

fn dynamic(value: Value) -> Option<(alloc::boxed::Box<super::OriginProgramValue>, usize)> {
    match value {
        Value::Dynamic { result, depth } => Some((result, depth)),
        Value::I64(_) | Value::Static { .. } => None,
    }
}

fn returned_outputs(
    selected: &noble_contracts::source::proof::CheckedSelectedTarget<'_>,
    plan: &super::plan::Layout,
    named_index: usize,
) -> Option<alloc::vec::Vec<super::OriginOutput>> {
    let named = plan.programs.get(named_index)?;
    let inputs = &selected.body().interface.stack_in;
    let outputs = &selected.body().interface.stack_out;
    if inputs.len() > MAX_ORIGIN_OPERATIONS
        || outputs.len() > MAX_ORIGIN_OPERATIONS
        || inputs.iter().any(|ty| *ty != noble_kernel::types::Ty::I64)
    {
        return None;
    }
    let mut stack =
        alloc::vec::Vec::with_capacity(inputs.len().saturating_add(named.operations.len()));
    for position in 0..inputs.len() {
        stack.push(Value::I64(super::OriginI64::RootInput(
            u32::try_from(position).ok()?,
        )));
    }
    for (ordinal, operation) in named.operations.iter().enumerate() {
        let table_entry = named.entry.checked_add(u32::try_from(ordinal).ok()?)?;
        match operation.action {
            super::plan::Action::I64(value) => {
                stack.push(Value::I64(super::OriginI64::Fixed(value)))
            }
            super::plan::Action::Word(0) => {
                let copied = match stack.last()? {
                    Value::I64(value) => Value::I64(*value),
                    Value::Static { node, program } => Value::Static {
                        node: *node,
                        program: *program,
                    },
                    Value::Dynamic { .. } => return None,
                };
                stack.push(copied);
            }
            super::plan::Action::Word(1) => {
                stack.pop()?;
            }
            super::plan::Action::Word(2) => {
                let len = stack.len();
                if len < 2 {
                    return None;
                }
                stack.swap(len - 1, len - 2);
            }
            super::plan::Action::Word(word @ (4..=6)) => {
                let Value::I64(super::OriginI64::Fixed(right)) = stack.pop()? else {
                    return None;
                };
                let Value::I64(super::OriginI64::Fixed(left)) = stack.pop()? else {
                    return None;
                };
                let value = match word {
                    4 => left.wrapping_add(right),
                    5 => left.wrapping_sub(right),
                    6 => left.wrapping_mul(right),
                    _ => return None,
                };
                stack.push(Value::I64(super::OriginI64::Fixed(value)));
            }
            super::plan::Action::Program(program) if program < plan.programs.len() => {
                stack.push(Value::Static {
                    node: operation.node,
                    program,
                })
            }
            super::plan::Action::Quote(..) => {
                selected.checked_quote_site(operation.node).ok()?;
                let Value::I64(operand) = stack.pop()? else {
                    return None;
                };
                stack.push(Value::Dynamic {
                    result: alloc::boxed::Box::new(super::OriginProgramValue::QuoteI64 {
                        node: operation.node,
                        table_entry,
                        operand,
                    }),
                    depth: 1,
                });
            }
            super::plan::Action::Word(9) => {
                let Value::Static {
                    node: right_node,
                    program: right_program_index,
                } = stack.pop()?
                else {
                    return None;
                };
                let (left, depth) = dynamic(stack.pop()?)?;
                if depth >= MAX_ORIGIN_DEPTH {
                    return None;
                }
                stack.push(Value::Dynamic {
                    result: alloc::boxed::Box::new(super::OriginProgramValue::Compose {
                        node: operation.node,
                        table_entry,
                        left,
                        right_node,
                        right_program_index,
                    }),
                    depth: depth + 1,
                });
            }
            _ => return None,
        }
        if stack.len() > MAX_ORIGIN_OPERATIONS {
            return None;
        }
    }
    if stack.len() != outputs.len() {
        return None;
    }
    let mut result = alloc::vec::Vec::new();
    for (stack_position, (ty, value)) in outputs.iter().zip(stack).enumerate() {
        if matches!(ty, noble_kernel::types::Ty::Program(..)) {
            let (program, _) = dynamic(value)?;
            result.push(super::OriginOutput {
                stack_position,
                result: *program,
            });
        } else if matches!(value, Value::Dynamic { .. } | Value::Static { .. }) {
            return None;
        }
    }
    if result.is_empty() {
        return None;
    }
    Some(result)
}

pub(super) fn build(
    selected: &noble_contracts::source::proof::CheckedSelectedTarget<'_>,
    plan: &super::plan::Layout,
) -> Option<super::SelectedOriginMetadata> {
    let [definition] = selected.submission().definitions.as_slice() else {
        return None;
    };
    if definition.identity != selected.definition_id().identity()
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
    let outputs = returned_outputs(selected, plan, named_program_index)?;
    Some(super::SelectedOriginMetadata {
        definition_id: selected.definition_id(),
        source_generation: selected.source_generation(),
        named_program_index,
        root_program_index: plan.root,
        programs,
        outputs,
    })
}
