//! The fixed v0 word-contract table, in `Definition` order.
//!
//! Each helper builds one scheme; `table` assembles them so the environment
//! builder stays one short loop.

const UNIT: crate::shapes::Pattern = crate::shapes::Pattern::Unit;
const BOOL: crate::shapes::Pattern = crate::shapes::Pattern::Bool;
const I64: crate::shapes::Pattern = crate::shapes::Pattern::I64;
const TEXT: crate::shapes::Pattern = crate::shapes::Pattern::Text;
const SYNTAX: crate::shapes::Pattern = crate::shapes::Pattern::Syntax;

fn stack_var(index: u32) -> crate::shapes::StackPart {
    crate::shapes::StackPart::Stack(crate::words::Variable(index))
}

fn value_var(index: u32) -> crate::shapes::StackPart {
    crate::shapes::StackPart::Pattern(crate::shapes::Pattern::Var(crate::words::Variable(index)))
}

fn pattern(item: crate::shapes::Pattern) -> crate::shapes::StackPart {
    crate::shapes::StackPart::Pattern(item)
}

fn effect_var(index: u32) -> crate::shapes::EffectSlot {
    crate::shapes::EffectSlot::Var(crate::words::Variable(index))
}

fn pair(left: crate::shapes::Pattern, right: crate::shapes::Pattern) -> crate::shapes::Pattern {
    crate::shapes::Pattern::Pair(alloc::boxed::Box::new(left), alloc::boxed::Box::new(right))
}

fn sum(left: crate::shapes::Pattern, right: crate::shapes::Pattern) -> crate::shapes::Pattern {
    crate::shapes::Pattern::Sum(alloc::boxed::Box::new(left), alloc::boxed::Box::new(right))
}

fn list(item: crate::shapes::Pattern) -> crate::shapes::Pattern {
    crate::shapes::Pattern::List(alloc::boxed::Box::new(item))
}

fn program(
    item_in: alloc::vec::Vec<crate::shapes::StackPart>,
    item_out: alloc::vec::Vec<crate::shapes::StackPart>,
    effects: alloc::vec::Vec<crate::shapes::EffectSlot>,
) -> crate::shapes::Pattern {
    crate::shapes::Pattern::Program(alloc::boxed::Box::new(crate::shapes::Signature {
        stack_in: item_in,
        stack_out: item_out,
        effects,
    }))
}

fn scheme(
    var_kinds: alloc::vec::Vec<crate::words::VariableKind>,
    stack_in: alloc::vec::Vec<crate::shapes::StackPart>,
    stack_out: alloc::vec::Vec<crate::shapes::StackPart>,
    effects: alloc::vec::Vec<crate::shapes::EffectSlot>,
) -> crate::words::Scheme {
    crate::words::Scheme {
        var_kinds,
        stack_in,
        stack_out,
        effects,
    }
}

fn dup() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value
        ],
        alloc::vec![stack_var(0), value_var(1)],
        alloc::vec![stack_var(0), value_var(1), value_var(1)],
        alloc::vec![],
    )
}

fn drop() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value
        ],
        alloc::vec![stack_var(0), value_var(1)],
        alloc::vec![stack_var(0)],
        alloc::vec![],
    )
}

fn swap() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value,
            crate::words::VariableKind::Value
        ],
        alloc::vec![stack_var(0), value_var(1), value_var(2)],
        alloc::vec![stack_var(0), value_var(2), value_var(1)],
        alloc::vec![],
    )
}

fn dip() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Effect
        ],
        alloc::vec![
            stack_var(0),
            value_var(1),
            pattern(program(
                alloc::vec![stack_var(0)],
                alloc::vec![stack_var(2)],
                alloc::vec![effect_var(3)],
            )),
        ],
        alloc::vec![stack_var(2), value_var(1)],
        alloc::vec![effect_var(3)],
    )
}

fn arith() -> crate::words::Scheme {
    scheme(
        alloc::vec![crate::words::VariableKind::Stack],
        alloc::vec![stack_var(0), pattern(I64), pattern(I64)],
        alloc::vec![stack_var(0), pattern(I64)],
        alloc::vec![],
    )
}

fn quote() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value,
            crate::words::VariableKind::Stack
        ],
        alloc::vec![stack_var(0), value_var(1)],
        alloc::vec![
            stack_var(0),
            pattern(program(
                alloc::vec![stack_var(2)],
                alloc::vec![stack_var(2), value_var(1)],
                alloc::vec![],
            )),
        ],
        alloc::vec![],
    )
}

fn compose() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Effect,
            crate::words::VariableKind::Effect
        ],
        alloc::vec![
            stack_var(0),
            pattern(program(
                alloc::vec![stack_var(1)],
                alloc::vec![stack_var(2)],
                alloc::vec![effect_var(4)],
            )),
            pattern(program(
                alloc::vec![stack_var(2)],
                alloc::vec![stack_var(3)],
                alloc::vec![effect_var(5)],
            )),
        ],
        alloc::vec![
            stack_var(0),
            pattern(program(
                alloc::vec![stack_var(1)],
                alloc::vec![stack_var(3)],
                alloc::vec![effect_var(4), effect_var(5)],
            )),
        ],
        alloc::vec![],
    )
}

fn run() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Effect
        ],
        alloc::vec![
            stack_var(0),
            pattern(program(
                alloc::vec![stack_var(0)],
                alloc::vec![stack_var(1)],
                alloc::vec![effect_var(2)],
            )),
        ],
        alloc::vec![stack_var(1)],
        alloc::vec![effect_var(2)],
    )
}

fn reflect() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Effect
        ],
        alloc::vec![
            stack_var(0),
            pattern(program(
                alloc::vec![stack_var(1)],
                alloc::vec![stack_var(2)],
                alloc::vec![effect_var(3)],
            )),
        ],
        alloc::vec![stack_var(0), pattern(SYNTAX)],
        alloc::vec![],
    )
}

fn unit() -> crate::words::Scheme {
    scheme(
        alloc::vec![crate::words::VariableKind::Stack],
        alloc::vec![stack_var(0)],
        alloc::vec![stack_var(0), pattern(UNIT)],
        alloc::vec![],
    )
}

fn pair_word() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value,
            crate::words::VariableKind::Value
        ],
        alloc::vec![stack_var(0), value_var(1), value_var(2)],
        alloc::vec![
            stack_var(0),
            pattern(pair(
                crate::shapes::Pattern::Var(crate::words::Variable(1)),
                crate::shapes::Pattern::Var(crate::words::Variable(2))
            )),
        ],
        alloc::vec![],
    )
}

fn unpair() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value,
            crate::words::VariableKind::Value
        ],
        alloc::vec![
            stack_var(0),
            pattern(pair(
                crate::shapes::Pattern::Var(crate::words::Variable(1)),
                crate::shapes::Pattern::Var(crate::words::Variable(2))
            )),
        ],
        alloc::vec![stack_var(0), value_var(1), value_var(2)],
        alloc::vec![],
    )
}

fn inl() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value,
            crate::words::VariableKind::Value
        ],
        alloc::vec![stack_var(0), value_var(1)],
        alloc::vec![
            stack_var(0),
            pattern(sum(
                crate::shapes::Pattern::Var(crate::words::Variable(1)),
                crate::shapes::Pattern::Var(crate::words::Variable(2))
            )),
        ],
        alloc::vec![],
    )
}

fn inr() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value,
            crate::words::VariableKind::Value
        ],
        alloc::vec![stack_var(0), value_var(2)],
        alloc::vec![
            stack_var(0),
            pattern(sum(
                crate::shapes::Pattern::Var(crate::words::Variable(1)),
                crate::shapes::Pattern::Var(crate::words::Variable(2))
            )),
        ],
        alloc::vec![],
    )
}

fn case() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value,
            crate::words::VariableKind::Value,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Effect,
            crate::words::VariableKind::Effect
        ],
        alloc::vec![
            stack_var(0),
            pattern(sum(
                crate::shapes::Pattern::Var(crate::words::Variable(1)),
                crate::shapes::Pattern::Var(crate::words::Variable(2))
            )),
            pattern(program(
                alloc::vec![stack_var(0), value_var(1)],
                alloc::vec![stack_var(3)],
                alloc::vec![effect_var(4)],
            )),
            pattern(program(
                alloc::vec![stack_var(0), value_var(2)],
                alloc::vec![stack_var(3)],
                alloc::vec![effect_var(5)],
            )),
        ],
        alloc::vec![stack_var(3)],
        alloc::vec![effect_var(4), effect_var(5)],
    )
}

fn if_word() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Effect,
            crate::words::VariableKind::Effect
        ],
        alloc::vec![
            stack_var(0),
            pattern(BOOL),
            pattern(program(
                alloc::vec![stack_var(0)],
                alloc::vec![stack_var(1)],
                alloc::vec![effect_var(2)],
            )),
            pattern(program(
                alloc::vec![stack_var(0)],
                alloc::vec![stack_var(1)],
                alloc::vec![effect_var(3)],
            )),
        ],
        alloc::vec![stack_var(1)],
        alloc::vec![effect_var(2), effect_var(3)],
    )
}

fn nil() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value
        ],
        alloc::vec![stack_var(0)],
        alloc::vec![
            stack_var(0),
            pattern(list(crate::shapes::Pattern::Var(crate::words::Variable(1)))),
        ],
        alloc::vec![],
    )
}

fn cons() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value
        ],
        alloc::vec![
            stack_var(0),
            value_var(1),
            pattern(list(crate::shapes::Pattern::Var(crate::words::Variable(1)))),
        ],
        alloc::vec![
            stack_var(0),
            pattern(list(crate::shapes::Pattern::Var(crate::words::Variable(1)))),
        ],
        alloc::vec![],
    )
}

fn list_case() -> crate::words::Scheme {
    scheme(
        alloc::vec![
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Value,
            crate::words::VariableKind::Stack,
            crate::words::VariableKind::Effect,
            crate::words::VariableKind::Effect
        ],
        alloc::vec![
            stack_var(0),
            pattern(list(crate::shapes::Pattern::Var(crate::words::Variable(1)))),
            pattern(program(
                alloc::vec![stack_var(0)],
                alloc::vec![stack_var(2)],
                alloc::vec![effect_var(3)],
            )),
            pattern(program(
                alloc::vec![
                    stack_var(0),
                    value_var(1),
                    pattern(list(crate::shapes::Pattern::Var(crate::words::Variable(1))))
                ],
                alloc::vec![stack_var(2)],
                alloc::vec![effect_var(4)],
            )),
        ],
        alloc::vec![stack_var(2)],
        alloc::vec![effect_var(3), effect_var(4)],
    )
}

fn test_emit() -> crate::words::Scheme {
    scheme(
        alloc::vec![crate::words::VariableKind::Stack],
        alloc::vec![stack_var(0), pattern(TEXT)],
        alloc::vec![stack_var(0), pattern(UNIT)],
        alloc::vec![crate::shapes::EffectSlot::Effect(
            crate::contracts::TEST_EMIT
        )],
    )
}

/// The bootstrap table in `Definition` order.
pub fn table() -> alloc::vec::Vec<(crate::contracts::Behavior, crate::words::Scheme)> {
    alloc::vec![
        (crate::contracts::Behavior::Dup, dup()),
        (crate::contracts::Behavior::Drop, drop()),
        (crate::contracts::Behavior::Swap, swap()),
        (crate::contracts::Behavior::Dip, dip()),
        (crate::contracts::Behavior::Arith, arith()),
        (crate::contracts::Behavior::Arith, arith()),
        (crate::contracts::Behavior::Arith, arith()),
        (crate::contracts::Behavior::Quote, quote()),
        (crate::contracts::Behavior::Compose, compose()),
        (crate::contracts::Behavior::Run, run()),
        (crate::contracts::Behavior::Reflect, reflect()),
        (crate::contracts::Behavior::Unit, unit()),
        (crate::contracts::Behavior::Pair, pair_word()),
        (crate::contracts::Behavior::Unpair, unpair()),
        (crate::contracts::Behavior::Inl, inl()),
        (crate::contracts::Behavior::Inr, inr()),
        (crate::contracts::Behavior::Case, case()),
        (crate::contracts::Behavior::If, if_word()),
        (crate::contracts::Behavior::Nil, nil()),
        (crate::contracts::Behavior::Cons, cons()),
        (crate::contracts::Behavior::ListCase, list_case()),
        (crate::contracts::Behavior::TestEmit, test_emit()),
    ]
}
