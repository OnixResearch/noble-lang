//! Bootstrap word numbers and their documented witness-variable order.

pub(super) const LITERAL: &[noble_kernel::words::VariableKind] =
    &[noble_kernel::words::VariableKind::Stack];
pub(super) const QUOTATION: &[noble_kernel::words::VariableKind] = &[
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Effect,
];
const STACK_VALUE: &[noble_kernel::words::VariableKind] = &[
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Value,
];
const STACK_TWO_VALUES: &[noble_kernel::words::VariableKind] = &[
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Value,
    noble_kernel::words::VariableKind::Value,
];
const DIP: &[noble_kernel::words::VariableKind] = &[
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Value,
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Effect,
];
const QUOTE: &[noble_kernel::words::VariableKind] = &[
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Value,
    noble_kernel::words::VariableKind::Stack,
];
const COMPOSE: &[noble_kernel::words::VariableKind] = &[
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Effect,
    noble_kernel::words::VariableKind::Effect,
];
const RUN: &[noble_kernel::words::VariableKind] = &[
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Effect,
];
const BRANCH: &[noble_kernel::words::VariableKind] = &[
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Effect,
    noble_kernel::words::VariableKind::Effect,
];
const CASE: &[noble_kernel::words::VariableKind] = &[
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Value,
    noble_kernel::words::VariableKind::Value,
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Effect,
    noble_kernel::words::VariableKind::Effect,
];
const LIST_CASE: &[noble_kernel::words::VariableKind] = &[
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Value,
    noble_kernel::words::VariableKind::Stack,
    noble_kernel::words::VariableKind::Effect,
    noble_kernel::words::VariableKind::Effect,
];

pub(super) fn def_of(
    name: &str,
) -> Result<(u32, &'static [noble_kernel::words::VariableKind]), String> {
    Ok(match name {
        "dup" => (0, STACK_VALUE),
        "drop" => (1, STACK_VALUE),
        "swap" => (2, STACK_TWO_VALUES),
        "dip" => (3, DIP),
        "+" => (4, LITERAL),
        "-" => (5, LITERAL),
        "*" => (6, LITERAL),
        "=" => (7, LITERAL),
        "quote" => (8, QUOTE),
        "compose" => (9, COMPOSE),
        "run" => (10, RUN),
        "reflect" => (11, QUOTATION),
        "unit" => (12, LITERAL),
        "pair" => (13, STACK_TWO_VALUES),
        "unpair" => (14, STACK_TWO_VALUES),
        "inl" => (15, STACK_TWO_VALUES),
        "inr" => (16, STACK_TWO_VALUES),
        "case" => (17, CASE),
        "if" => (18, BRANCH),
        "nil" => (19, STACK_VALUE),
        "cons" => (20, STACK_VALUE),
        "list.case" => (21, LIST_CASE),
        "test.emit" => (22, LITERAL),
        _ => {
            return Err(format!(
                "unknown word `{name}` for the bootstrap environment"
            ))
        }
    })
}
