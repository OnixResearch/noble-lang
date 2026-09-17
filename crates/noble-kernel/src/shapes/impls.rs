//! Hand-written structural impls for the self-recursive pattern tree.
//!
//! `Pattern` recurses through `alloc` containers, so a derived `Clone` or
//! `Debug` body hands this type's own trait instance to `Vec::clone` and to
//! the formatting machinery. Aeneas emits a trait instance after the method
//! that feeds it, so such a body forward-references a declaration Lean has not
//! seen yet. These bodies call the concrete methods instead: an element clone
//! goes through the local `Clone` impl and nested formatting goes through the
//! local `Debug` impl, so the translated module stays acyclic.

/// Deep-copy one pattern stack, one element at a time.
///
/// The extraction treats this helper as an assumption: Aeneas would
/// functionalize its loop inside the clone instance.s mutual block, and the
/// Lean backend cannot prove that shape monotone. Cloning stays an explicit,
/// disclosed copy step.
#[charon::opaque]
fn clone_parts(stack: &[crate::shapes::Pattern]) -> alloc::vec::Vec<crate::shapes::Pattern> {
    let mut out: alloc::vec::Vec<crate::shapes::Pattern> =
        alloc::vec::Vec::with_capacity(stack.len());
    let mut index = 0;
    while index < stack.len() {
        out.push(stack[index].clone());
        index += 1;
    }
    out
}

/// Deep-copy one effect-slot stack, one element at a time.
fn clone_slots(stack: &[crate::shapes::EffectSlot]) -> alloc::vec::Vec<crate::shapes::EffectSlot> {
    let mut out: alloc::vec::Vec<crate::shapes::EffectSlot> =
        alloc::vec::Vec::with_capacity(stack.len());
    let mut index = 0;
    while index < stack.len() {
        out.push(stack[index].clone());
        index += 1;
    }
    out
}

/// Render one pattern stack in the list form `[a, b]`.
///
/// The extraction treats this helper as an assumption: formatting is
/// observability only, and Aeneas would functionalize its loop inside the
/// debug instance.s mutual block, which the Lean backend cannot prove
/// monotone.
#[charon::opaque]
fn debug_parts(
    stack: &[crate::shapes::Pattern],
    f: &mut core::fmt::Formatter<'_>,
) -> core::fmt::Result {
    attempt!(core::fmt::Formatter::write_str(f, "["));
    let mut index = 0;
    let mut failure: Option<core::fmt::Error> = None;
    while index < stack.len() {
        let step = if index == 0 {
            core::fmt::Debug::fmt(&stack[index], f)
        } else {
            match core::fmt::Formatter::write_str(f, ", ") {
                Ok(()) => core::fmt::Debug::fmt(&stack[index], f),
                Err(problem) => Err(problem),
            }
        };
        match step {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => core::fmt::Formatter::write_str(f, "]"),
    }
}

/// Render one effect-slot stack in the list form `[a, b]`.
///
/// The extraction treats this helper as an assumption, for the same reason
/// `debug_parts` is one.
#[charon::opaque]
fn debug_slots(
    stack: &[crate::shapes::EffectSlot],
    f: &mut core::fmt::Formatter<'_>,
) -> core::fmt::Result {
    attempt!(core::fmt::Formatter::write_str(f, "["));
    let mut index = 0;
    let mut failure: Option<core::fmt::Error> = None;
    while index < stack.len() {
        let step = if index == 0 {
            core::fmt::Debug::fmt(&stack[index], f)
        } else {
            match core::fmt::Formatter::write_str(f, ", ") {
                Ok(()) => core::fmt::Debug::fmt(&stack[index], f),
                Err(problem) => Err(problem),
            }
        };
        match step {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => core::fmt::Formatter::write_str(f, "]"),
    }
}

/// Deep-copy one pattern.
impl Clone for crate::shapes::Pattern {
    fn clone(&self) -> crate::shapes::Pattern {
        match self {
            crate::shapes::Pattern::Unit => crate::shapes::Pattern::Unit,
            crate::shapes::Pattern::Bool => crate::shapes::Pattern::Bool,
            crate::shapes::Pattern::I64 => crate::shapes::Pattern::I64,
            crate::shapes::Pattern::Text => crate::shapes::Pattern::Text,
            crate::shapes::Pattern::Syntax => crate::shapes::Pattern::Syntax,
            crate::shapes::Pattern::Pair(left, right) => crate::shapes::Pattern::Pair(
                alloc::boxed::Box::new((**left).clone()),
                alloc::boxed::Box::new((**right).clone()),
            ),
            crate::shapes::Pattern::Sum(left, right) => crate::shapes::Pattern::Sum(
                alloc::boxed::Box::new((**left).clone()),
                alloc::boxed::Box::new((**right).clone()),
            ),
            crate::shapes::Pattern::List(item) => {
                crate::shapes::Pattern::List(alloc::boxed::Box::new((**item).clone()))
            }
            crate::shapes::Pattern::Program(stack_in, stack_out, effects) => {
                crate::shapes::Pattern::Program(
                    alloc::boxed::Box::new(clone_parts(stack_in.as_slice())),
                    alloc::boxed::Box::new(clone_parts(stack_out.as_slice())),
                    alloc::boxed::Box::new(clone_slots(effects.as_slice())),
                )
            }
            crate::shapes::Pattern::Resource(kind) => crate::shapes::Pattern::Resource(*kind),
            crate::shapes::Pattern::Var(var) => crate::shapes::Pattern::Var(*var),
            crate::shapes::Pattern::StackVar(var) => crate::shapes::Pattern::StackVar(*var),
        }
    }
}

/// Render one pattern in its constructor form.
impl core::fmt::Debug for crate::shapes::Pattern {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            crate::shapes::Pattern::Unit => core::fmt::Formatter::write_str(f, "Unit"),
            crate::shapes::Pattern::Bool => core::fmt::Formatter::write_str(f, "Bool"),
            crate::shapes::Pattern::I64 => core::fmt::Formatter::write_str(f, "I64"),
            crate::shapes::Pattern::Text => core::fmt::Formatter::write_str(f, "Text"),
            crate::shapes::Pattern::Syntax => core::fmt::Formatter::write_str(f, "Syntax"),
            crate::shapes::Pattern::Pair(left, right) => {
                attempt!(core::fmt::Formatter::write_str(f, "Pair("));
                attempt!(core::fmt::Debug::fmt(&**left, f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(core::fmt::Debug::fmt(&**right, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::Sum(left, right) => {
                attempt!(core::fmt::Formatter::write_str(f, "Sum("));
                attempt!(core::fmt::Debug::fmt(&**left, f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(core::fmt::Debug::fmt(&**right, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::List(item) => {
                attempt!(core::fmt::Formatter::write_str(f, "List("));
                attempt!(core::fmt::Debug::fmt(&**item, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::Program(stack_in, stack_out, effects) => {
                attempt!(core::fmt::Formatter::write_str(f, "Program("));
                attempt!(debug_parts(stack_in.as_slice(), f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(debug_parts(stack_out.as_slice(), f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(debug_slots(effects.as_slice(), f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::Resource(kind) => {
                attempt!(core::fmt::Formatter::write_str(f, "Resource("));
                attempt!(core::fmt::Debug::fmt(kind, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::Var(var) => {
                attempt!(core::fmt::Formatter::write_str(f, "Var("));
                attempt!(core::fmt::Debug::fmt(var, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::shapes::Pattern::StackVar(var) => {
                attempt!(core::fmt::Formatter::write_str(f, "StackVar("));
                attempt!(core::fmt::Debug::fmt(var, f));
                core::fmt::Formatter::write_str(f, ")")
            }
        }
    }
}

/// Structural equality of two patterns, decided by one explicit pairwise walk.
///
/// The work stack holds references to the two trees' nodes, so no step
/// recurses and no trait instance is called from inside the loop: the walk
/// stays a plain, acyclic definition the Lean backend can translate.
fn pattern_eq(left: &crate::shapes::Pattern, right: &crate::shapes::Pattern) -> bool {
    let mut work: alloc::vec::Vec<(&crate::shapes::Pattern, &crate::shapes::Pattern)> =
        alloc::vec::Vec::new();
    work.push((left, right));
    let mut outcome = true;
    let mut mismatch = false;
    while !work.is_empty() && !mismatch {
        let pair = work.pop();
        let is_equal = match pair {
            Some((first, second)) => match (first, second) {
                (crate::shapes::Pattern::Unit, crate::shapes::Pattern::Unit) => true,
                (crate::shapes::Pattern::Bool, crate::shapes::Pattern::Bool) => true,
                (crate::shapes::Pattern::I64, crate::shapes::Pattern::I64) => true,
                (crate::shapes::Pattern::Text, crate::shapes::Pattern::Text) => true,
                (crate::shapes::Pattern::Syntax, crate::shapes::Pattern::Syntax) => true,
                (
                    crate::shapes::Pattern::Resource(first_kind),
                    crate::shapes::Pattern::Resource(second_kind),
                ) => first_kind == second_kind,
                (
                    crate::shapes::Pattern::Var(first_var),
                    crate::shapes::Pattern::Var(second_var),
                ) => first_var == second_var,
                (
                    crate::shapes::Pattern::StackVar(first_var),
                    crate::shapes::Pattern::StackVar(second_var),
                ) => first_var == second_var,
                (
                    crate::shapes::Pattern::Pair(first_head, first_tail),
                    crate::shapes::Pattern::Pair(second_head, second_tail),
                )
                | (
                    crate::shapes::Pattern::Sum(first_head, first_tail),
                    crate::shapes::Pattern::Sum(second_head, second_tail),
                ) => {
                    work.push((&**first_head, &**second_head));
                    work.push((&**first_tail, &**second_tail));
                    true
                }
                (
                    crate::shapes::Pattern::List(first_item),
                    crate::shapes::Pattern::List(second_item),
                ) => {
                    work.push((&**first_item, &**second_item));
                    true
                }
                (
                    crate::shapes::Pattern::Program(first_in, first_out, first_effects),
                    crate::shapes::Pattern::Program(second_in, second_out, second_effects),
                ) => {
                    if first_in.len() != second_in.len()
                        || first_out.len() != second_out.len()
                        || first_effects.len() != second_effects.len()
                    {
                        false
                    } else {
                        let mut index = 0;
                        while index < first_in.len() {
                            work.push((&first_in[index], &second_in[index]));
                            index += 1;
                        }
                        index = 0;
                        while index < first_out.len() {
                            work.push((&first_out[index], &second_out[index]));
                            index += 1;
                        }
                        if first_effects != second_effects {
                            mismatch = true;
                            outcome = false;
                        }
                        true
                    }
                }
                _ => false,
            },
            None => true,
        };
        if !is_equal {
            mismatch = true;
            outcome = false;
        }
    }
    outcome
}

/// Equality: the explicit pairwise walk above.
impl PartialEq for crate::shapes::Pattern {
    fn eq(&self, other: &crate::shapes::Pattern) -> bool {
        pattern_eq(self, other)
    }
}

#[cfg(test)]
impl Eq for crate::shapes::Pattern {}
