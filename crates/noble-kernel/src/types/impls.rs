//! Hand-written structural impls for the self-recursive type tree.
//!
//! `Ty` recurses through `alloc` containers, so a derived `Clone` or `Debug`
//! body hands this type's own trait instance to `Vec::clone` and to the
//! formatting machinery. Aeneas emits a trait instance after the method that
//! feeds it, so such a body forward-references a declaration Lean has not seen
//! yet. These bodies call the concrete methods instead: an element clone goes
//! through the local `Clone` impl and nested formatting goes through the local
//! `Debug` impl, so the translated module stays acyclic.

/// Deep-copy one type stack, one element at a time.
///
/// The extraction treats this helper as an assumption: Aeneas would
/// functionalize its loop inside the clone instance.s mutual block, and the
/// Lean backend cannot prove that shape monotone. Cloning stays an explicit,
/// disclosed copy step.
#[charon::opaque]
fn clone_stack(stack: &[crate::types::Ty]) -> alloc::vec::Vec<crate::types::Ty> {
    let mut out: alloc::vec::Vec<crate::types::Ty> = alloc::vec::Vec::with_capacity(stack.len());
    let mut index = 0;
    while index < stack.len() {
        out.push(stack[index].clone());
        index += 1;
    }
    out
}

/// Render one type stack in the list form `[a, b]`.
///
/// The extraction treats this helper as an assumption: formatting is
/// observability only, and Aeneas would functionalize its loop inside the
/// debug instance.s mutual block, which the Lean backend cannot prove
/// monotone.
#[charon::opaque]
fn debug_stack(stack: &[crate::types::Ty], f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

/// Deep-copy one type.
impl Clone for crate::types::Ty {
    fn clone(&self) -> crate::types::Ty {
        match self {
            crate::types::Ty::Unit => crate::types::Ty::Unit,
            crate::types::Ty::Bool => crate::types::Ty::Bool,
            crate::types::Ty::I64 => crate::types::Ty::I64,
            crate::types::Ty::Text => crate::types::Ty::Text,
            crate::types::Ty::Syntax => crate::types::Ty::Syntax,
            crate::types::Ty::Pair(left, right) => crate::types::Ty::Pair(
                alloc::boxed::Box::new((**left).clone()),
                alloc::boxed::Box::new((**right).clone()),
            ),
            crate::types::Ty::Sum(left, right) => crate::types::Ty::Sum(
                alloc::boxed::Box::new((**left).clone()),
                alloc::boxed::Box::new((**right).clone()),
            ),
            crate::types::Ty::List(item) => {
                crate::types::Ty::List(alloc::boxed::Box::new((**item).clone()))
            }
            crate::types::Ty::Program(stack_in, stack_out, effects) => crate::types::Ty::Program(
                alloc::boxed::Box::new(clone_stack(stack_in.as_slice())),
                alloc::boxed::Box::new(clone_stack(stack_out.as_slice())),
                effects.clone(),
            ),
            crate::types::Ty::Resource(kind) => crate::types::Ty::Resource(*kind),
        }
    }
}

/// Render one type in its constructor form.
impl core::fmt::Debug for crate::types::Ty {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            crate::types::Ty::Unit => core::fmt::Formatter::write_str(f, "Unit"),
            crate::types::Ty::Bool => core::fmt::Formatter::write_str(f, "Bool"),
            crate::types::Ty::I64 => core::fmt::Formatter::write_str(f, "I64"),
            crate::types::Ty::Text => core::fmt::Formatter::write_str(f, "Text"),
            crate::types::Ty::Syntax => core::fmt::Formatter::write_str(f, "Syntax"),
            crate::types::Ty::Pair(left, right) => {
                attempt!(core::fmt::Formatter::write_str(f, "Pair("));
                attempt!(core::fmt::Debug::fmt(&**left, f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(core::fmt::Debug::fmt(&**right, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::types::Ty::Sum(left, right) => {
                attempt!(core::fmt::Formatter::write_str(f, "Sum("));
                attempt!(core::fmt::Debug::fmt(&**left, f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(core::fmt::Debug::fmt(&**right, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::types::Ty::List(item) => {
                attempt!(core::fmt::Formatter::write_str(f, "List("));
                attempt!(core::fmt::Debug::fmt(&**item, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::types::Ty::Program(stack_in, stack_out, effects) => {
                attempt!(core::fmt::Formatter::write_str(f, "Program("));
                attempt!(debug_stack(stack_in.as_slice(), f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(debug_stack(stack_out.as_slice(), f));
                attempt!(core::fmt::Formatter::write_str(f, ", "));
                attempt!(core::fmt::Debug::fmt(effects, f));
                core::fmt::Formatter::write_str(f, ")")
            }
            crate::types::Ty::Resource(kind) => {
                attempt!(core::fmt::Formatter::write_str(f, "Resource("));
                attempt!(core::fmt::Debug::fmt(kind, f));
                core::fmt::Formatter::write_str(f, ")")
            }
        }
    }
}

/// Structural equality of two types, decided by one explicit pairwise walk.
///
/// The work stack holds references to the two trees' nodes, so no step
/// recurses and no trait instance is called from inside the loop: the walk
/// stays a plain, acyclic definition the Lean backend can translate.
fn ty_eq(left: &crate::types::Ty, right: &crate::types::Ty) -> bool {
    let mut work: alloc::vec::Vec<(&crate::types::Ty, &crate::types::Ty)> = alloc::vec::Vec::new();
    work.push((left, right));
    let mut outcome = true;
    let mut mismatch = false;
    while !work.is_empty() && !mismatch {
        let pair = work.pop();
        let is_equal = match pair {
            Some((first, second)) => match (first, second) {
                (crate::types::Ty::Unit, crate::types::Ty::Unit) => true,
                (crate::types::Ty::Bool, crate::types::Ty::Bool) => true,
                (crate::types::Ty::I64, crate::types::Ty::I64) => true,
                (crate::types::Ty::Text, crate::types::Ty::Text) => true,
                (crate::types::Ty::Syntax, crate::types::Ty::Syntax) => true,
                (
                    crate::types::Ty::Resource(first_kind),
                    crate::types::Ty::Resource(second_kind),
                ) => first_kind == second_kind,
                (
                    crate::types::Ty::Pair(first_head, first_tail),
                    crate::types::Ty::Pair(second_head, second_tail),
                )
                | (
                    crate::types::Ty::Sum(first_head, first_tail),
                    crate::types::Ty::Sum(second_head, second_tail),
                ) => {
                    work.push((&**first_head, &**second_head));
                    work.push((&**first_tail, &**second_tail));
                    true
                }
                (crate::types::Ty::List(first_item), crate::types::Ty::List(second_item)) => {
                    work.push((&**first_item, &**second_item));
                    true
                }
                (
                    crate::types::Ty::Program(first_in, first_out, first_effects),
                    crate::types::Ty::Program(second_in, second_out, second_effects),
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
impl PartialEq for crate::types::Ty {
    fn eq(&self, other: &crate::types::Ty) -> bool {
        ty_eq(self, other)
    }
}

#[cfg(test)]
impl Eq for crate::types::Ty {}
