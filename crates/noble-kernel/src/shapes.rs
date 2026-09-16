//! Concrete-constructor patterns with variable holes.
//!
//! Scheme stacks hold *patterns*: constructor trees that may mention value,
//! stack, or effect variables at their leaves. Validation is one bounded,
//! iterative walk; no operation recurses.

/// Local bound for one pattern walk; beyond it validation fails closed.
const WORK_CAP: usize = 512;

/// A type pattern over concrete constructors and value variables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pattern {
    /// The unit type.
    Unit,
    /// The boolean type.
    Bool,
    /// The wrapping 64-bit integer type.
    I64,
    /// The text type.
    Text,
    /// Inert syntax.
    Syntax,
    /// A product pattern.
    Pair(alloc::boxed::Box<Pattern>, alloc::boxed::Box<Pattern>),
    /// A sum pattern.
    Sum(alloc::boxed::Box<Pattern>, alloc::boxed::Box<Pattern>),
    /// A list pattern.
    List(alloc::boxed::Box<Pattern>),
    /// A program pattern with its own stack and effect patterns.
    Program(alloc::boxed::Box<Signature>),
    /// An opaque resource kind.
    Resource(crate::types::ResourceKind),
    /// A value-type variable.
    Var(crate::words::Variable),
}

/// A program pattern inside a scheme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    /// Required stack pattern, bottom-first.
    pub stack_in: alloc::vec::Vec<StackPart>,
    /// Result stack pattern, bottom-first.
    pub stack_out: alloc::vec::Vec<StackPart>,
    /// Latent effect pattern.
    pub effects: alloc::vec::Vec<EffectSlot>,
}

/// One stack position in a pattern stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StackPart {
    /// A pattern occupying one position.
    Pattern(Pattern),
    /// A whole-stack variable standing for zero or more positions.
    Stack(crate::words::Variable),
}

/// One effect position in a scheme bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectSlot {
    /// A concrete effect identity.
    Effect(crate::types::EffId),
    /// An effect-set variable.
    Var(crate::words::Variable),
}

/// Why a pattern or scheme is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Defect {
    /// A variable index exceeds the declared variable count.
    UnknownVariable,
    /// A variable is used in a position that does not match its kind.
    KindMismatch,
}

/// One walk step: a pattern to inspect, part list, or effect-slot list.
enum Step<'a> {
    Pattern(&'a Pattern),
    Parts(&'a [StackPart]),
    Slots(&'a [EffectSlot]),
}

/// Require one variable to carry exactly the declared kind.
fn require_kind(
    kinds: &[crate::words::VariableKind],
    var: crate::words::Variable,
    expected: crate::words::VariableKind,
) -> Result<(), Defect> {
    let index = match usize::try_from(var.0) {
        Ok(index) => index,
        Err(_) => return Err(Defect::UnknownVariable),
    };
    let kind = match kinds.get(index) {
        Some(kind) => *kind,
        None => return Err(Defect::UnknownVariable),
    };
    if kind == expected {
        Ok(())
    } else {
        Err(Defect::KindMismatch)
    }
}

/// Validate one signature's parts and effects against the declared kinds.
pub fn validate(
    kinds: &[crate::words::VariableKind],
    stack_in: &[StackPart],
    stack_out: &[StackPart],
    effects: &[EffectSlot],
) -> Result<(), Defect> {
    let mut work: alloc::vec::Vec<Step> = alloc::vec::Vec::with_capacity(8);
    work.push(Step::Parts(stack_in));
    work.push(Step::Parts(stack_out));
    work.push(Step::Slots(effects));
    while let Some(step) = work.pop() {
        if work.len() >= WORK_CAP {
            return Err(Defect::KindMismatch);
        }
        match step {
            Step::Slots(slots) => {
                let mut index = 0;
                while index < slots.len() {
                    if let EffectSlot::Var(var) = &slots[index] {
                        attempt!(require_kind(
                            kinds,
                            *var,
                            crate::words::VariableKind::Effect
                        ));
                    }
                    index += 1;
                }
            }
            Step::Parts(parts) => {
                let mut index = 0;
                while index < parts.len() {
                    match &parts[index] {
                        StackPart::Stack(var) => {
                            attempt!(require_kind(kinds, *var, crate::words::VariableKind::Stack))
                        }
                        StackPart::Pattern(pattern) => work.push(Step::Pattern(pattern)),
                    }
                    index += 1;
                }
            }
            Step::Pattern(pattern) => match pattern {
                Pattern::Var(var) => {
                    attempt!(require_kind(kinds, *var, crate::words::VariableKind::Value))
                }
                Pattern::Pair(left, right) | Pattern::Sum(left, right) => {
                    work.push(Step::Pattern(left));
                    work.push(Step::Pattern(right));
                }
                Pattern::List(item) => work.push(Step::Pattern(item)),
                Pattern::Program(signature) => {
                    work.push(Step::Parts(&signature.stack_in));
                    work.push(Step::Parts(&signature.stack_out));
                    work.push(Step::Slots(&signature.effects));
                }
                Pattern::Unit
                | Pattern::Bool
                | Pattern::I64
                | Pattern::Text
                | Pattern::Syntax
                | Pattern::Resource(_) => {}
            },
        }
    }
    Ok(())
}
