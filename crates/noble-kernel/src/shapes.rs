//! Concrete-constructor patterns with variable holes.
//!
//! Scheme stacks hold *patterns*: constructor trees that may mention value,
//! stack, or effect variables at their leaves. Validation is one bounded,
//! iterative walk; no operation recurses.

/// Local bound for one pattern walk; beyond it validation fails closed.
const WORK_CAP: usize = 512;

/// A type pattern over concrete constructors, value variables, and whole-stack
/// variables.
///
/// The program case carries its stacks and effect pattern directly, so the
/// family is self-recursive: mutually recursive pattern types would leave
/// Aeneas' dependency analysis with mixed declaration groups it refuses.
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
    /// A program pattern: required stack, result stack, effect pattern.
    Program(
        alloc::boxed::Box<alloc::vec::Vec<Pattern>>,
        alloc::boxed::Box<alloc::vec::Vec<Pattern>>,
        alloc::boxed::Box<alloc::vec::Vec<EffectSlot>>,
    ),
    /// An opaque resource kind.
    Resource(crate::types::ResourceKind),
    /// A value-type variable.
    Var(crate::words::Variable),
    /// A whole-stack variable standing for zero or more stack positions.
    StackVar(crate::words::Variable),
}

impl Pattern {
    /// Build a program pattern.
    pub fn program(
        stack_in: alloc::vec::Vec<Pattern>,
        stack_out: alloc::vec::Vec<Pattern>,
        effects: alloc::vec::Vec<EffectSlot>,
    ) -> Pattern {
        Pattern::Program(
            alloc::boxed::Box::new(stack_in),
            alloc::boxed::Box::new(stack_out),
            alloc::boxed::Box::new(effects),
        )
    }
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

/// One walk step: a pattern to inspect, a pattern stack, or an effect-slot list.
///
/// Steps own their patterns: a reference into the signature under validation
/// cannot be carried across the owned queue Aeneas interprets.
enum Step {
    Pattern(Pattern),
    Parts(alloc::vec::Vec<Pattern>),
    Slots(alloc::vec::Vec<EffectSlot>),
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

/// Require every effect slot to match the effect kind.
fn require_slots(kinds: &[crate::words::VariableKind], slots: &[EffectSlot]) -> Result<(), Defect> {
    let mut index = 0;
    let mut defect: Option<Defect> = None;
    while index < slots.len() {
        let step = match &slots[index] {
            EffectSlot::Effect(_) => Ok(()),
            EffectSlot::Var(var) => require_kind(kinds, *var, crate::words::VariableKind::Effect),
        };
        match step {
            Ok(()) => index += 1,
            Err(problem) => {
                defect = Some(problem);
                break;
            }
        }
    }
    match defect {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

/// Require every stack part to match its kind, queueing nested patterns.
fn require_parts(
    kinds: &[crate::words::VariableKind],
    parts: alloc::vec::Vec<Pattern>,
    mut work: alloc::vec::Vec<Step>,
) -> Result<alloc::vec::Vec<Step>, Defect> {
    let mut index = 0;
    let mut defect: Option<Defect> = None;
    while index < parts.len() {
        let step = match &parts[index] {
            Pattern::StackVar(var) => require_kind(kinds, *var, crate::words::VariableKind::Stack),
            part => {
                work.push(Step::Pattern(part.clone()));
                Ok(())
            }
        };
        match step {
            Ok(()) => index += 1,
            Err(problem) => {
                defect = Some(problem);
                break;
            }
        }
    }
    match defect {
        Some(problem) => Err(problem),
        None => Ok(work),
    }
}

/// Require one pattern to match its kind, queueing its sub-patterns.
fn require_pattern(
    kinds: &[crate::words::VariableKind],
    pattern: Pattern,
    mut work: alloc::vec::Vec<Step>,
) -> Result<alloc::vec::Vec<Step>, Defect> {
    match pattern {
        Pattern::Var(var) => match require_kind(kinds, var, crate::words::VariableKind::Value) {
            Ok(()) => Ok(work),
            Err(problem) => Err(problem),
        },
        Pattern::StackVar(var) => {
            match require_kind(kinds, var, crate::words::VariableKind::Stack) {
                Ok(()) => Ok(work),
                Err(problem) => Err(problem),
            }
        }
        Pattern::Pair(left, right) | Pattern::Sum(left, right) => {
            work.push(Step::Pattern(*left));
            work.push(Step::Pattern(*right));
            Ok(work)
        }
        Pattern::List(item) => {
            work.push(Step::Pattern(*item));
            Ok(work)
        }
        Pattern::Program(stack_in, stack_out, effects) => {
            work.push(Step::Parts(*stack_in));
            work.push(Step::Parts(*stack_out));
            work.push(Step::Slots(*effects));
            Ok(work)
        }
        Pattern::Unit
        | Pattern::Bool
        | Pattern::I64
        | Pattern::Text
        | Pattern::Syntax
        | Pattern::Resource(_) => Ok(work),
    }
}

/// Validate one signature's parts and effects against the declared kinds.
pub fn validate(
    kinds: &[crate::words::VariableKind],
    stack_in: &[Pattern],
    stack_out: &[Pattern],
    effects: &[EffectSlot],
) -> Result<(), Defect> {
    let mut work: alloc::vec::Vec<Step> = alloc::vec::Vec::with_capacity(8);
    work.push(Step::Parts(stack_in.to_vec()));
    work.push(Step::Parts(stack_out.to_vec()));
    work.push(Step::Slots(effects.to_vec()));
    let mut defect: Option<Defect> = None;
    while let Some(step) = work.pop() {
        if work.len() >= WORK_CAP {
            defect = Some(Defect::KindMismatch);
            break;
        }
        let outcome = match step {
            Step::Slots(slots) => match require_slots(kinds, &slots) {
                Ok(()) => Ok(work),
                Err(problem) => Err(problem),
            },
            Step::Parts(parts) => require_parts(kinds, parts, work),
            Step::Pattern(pattern) => require_pattern(kinds, pattern, work),
        };
        match outcome {
            Ok(next) => work = next,
            Err(problem) => {
                defect = Some(problem);
                break;
            }
        }
    }
    match defect {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}
