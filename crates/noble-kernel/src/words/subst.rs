//! Iterative post-order substitution of one pattern into a concrete type.
//!
//! One bounded work stack drives the walk; nested program patterns expand
//! through explicit segments, and no step recurses.

/// Local bound for one substitution walk; beyond it substitution rejects.
const WORK_CAP: usize = 512;

/// One substitution step: a pattern, a constructor completion, a nested
/// signature expansion, or a pre-built segment emission.
enum Task<'a> {
    Part(&'a crate::shapes::Pattern),
    Finish(&'a crate::shapes::Pattern),
    Expand(&'a crate::shapes::Signature),
    Emit(alloc::vec::Vec<crate::types::Ty>),
}

impl crate::words::Scheme {
    /// Substitute one type pattern to a concrete type.
    pub fn subst_pattern(
        &self,
        pattern: &crate::shapes::Pattern,
        inst: &crate::words::Inst,
    ) -> Result<crate::types::Ty, crate::words::InstError> {
        let mut work: alloc::vec::Vec<Task> = alloc::vec::Vec::with_capacity(8);
        let mut segments: alloc::vec::Vec<alloc::vec::Vec<crate::types::Ty>> =
            alloc::vec::Vec::with_capacity(8);
        work.push(Task::Part(pattern));
        while let Some(task) = work.pop() {
            if work.len() >= WORK_CAP {
                return Err(crate::words::InstError::OversizedType);
            }
            match task {
                Task::Emit(segment) => segments.push(segment),
                Task::Finish(node) => {
                    let right = segments
                        .pop()
                        .ok_or(crate::words::InstError::OversizedType)?;
                    let left = segments
                        .pop()
                        .ok_or(crate::words::InstError::OversizedType)?;
                    let built = match node {
                        crate::shapes::Pattern::Pair(_, _) => pair_segment(left, right, false)?,
                        crate::shapes::Pattern::Sum(_, _) => pair_segment(left, right, true)?,
                        crate::shapes::Pattern::List(_) => {
                            let inner = single(left)?;
                            alloc::vec![crate::types::Ty::List(alloc::boxed::Box::new(inner))]
                        }
                        crate::shapes::Pattern::Var(_)
                        | crate::shapes::Pattern::Program(_)
                        | crate::shapes::Pattern::Unit
                        | crate::shapes::Pattern::Bool
                        | crate::shapes::Pattern::I64
                        | crate::shapes::Pattern::Text
                        | crate::shapes::Pattern::Syntax
                        | crate::shapes::Pattern::Resource(_) => {
                            return Err(crate::words::InstError::KindMismatch)
                        }
                    };
                    segments.push(built);
                }
                Task::Expand(signature) => {
                    let count = signature.stack_in.len() + signature.stack_out.len();
                    let mut collected: alloc::vec::Vec<alloc::vec::Vec<crate::types::Ty>> =
                        alloc::vec::Vec::with_capacity(count.max(4));
                    for _ in 0..count {
                        collected.push(
                            segments
                                .pop()
                                .ok_or(crate::words::InstError::OversizedType)?,
                        );
                    }
                    collected.reverse();
                    let mut parts = collected.into_iter();
                    let mut stack_in: alloc::vec::Vec<crate::types::Ty> =
                        alloc::vec::Vec::with_capacity(signature.stack_in.len().max(4));
                    for _ in 0..signature.stack_in.len() {
                        if let Some(segment) = parts.next() {
                            stack_in.extend(segment);
                        }
                    }
                    let mut stack_out: alloc::vec::Vec<crate::types::Ty> =
                        alloc::vec::Vec::with_capacity(signature.stack_out.len().max(4));
                    for _ in 0..signature.stack_out.len() {
                        if let Some(segment) = parts.next() {
                            stack_out.extend(segment);
                        }
                    }
                    let effects = self.subst_effects(&signature.effects, inst)?;
                    segments.push(alloc::vec![crate::types::Ty::program(
                        stack_in, stack_out, effects
                    )]);
                }
                Task::Part(node) => match node {
                    crate::shapes::Pattern::Unit => {
                        segments.push(alloc::vec![crate::types::Ty::Unit])
                    }
                    crate::shapes::Pattern::Bool => {
                        segments.push(alloc::vec![crate::types::Ty::Bool])
                    }
                    crate::shapes::Pattern::I64 => {
                        segments.push(alloc::vec![crate::types::Ty::I64])
                    }
                    crate::shapes::Pattern::Text => {
                        segments.push(alloc::vec![crate::types::Ty::Text])
                    }
                    crate::shapes::Pattern::Syntax => {
                        segments.push(alloc::vec![crate::types::Ty::Syntax])
                    }
                    crate::shapes::Pattern::Resource(kind) => {
                        segments.push(alloc::vec![crate::types::Ty::Resource(*kind)])
                    }
                    crate::shapes::Pattern::Var(var) => {
                        let ty = inst
                            .value(*var)
                            .cloned()
                            .ok_or(crate::words::InstError::UnknownVariable)?;
                        segments.push(alloc::vec![ty]);
                    }
                    crate::shapes::Pattern::Pair(left, right)
                    | crate::shapes::Pattern::Sum(left, right) => {
                        work.push(Task::Finish(node));
                        work.push(Task::Part(left));
                        work.push(Task::Part(right));
                    }
                    crate::shapes::Pattern::List(item) => {
                        work.push(Task::Finish(node));
                        work.push(Task::Part(item));
                    }
                    crate::shapes::Pattern::Program(signature) => {
                        work.push(Task::Expand(signature));
                        for part in signature
                            .stack_in
                            .iter()
                            .chain(signature.stack_out.iter())
                            .rev()
                        {
                            match part {
                                crate::shapes::StackPart::Pattern(item) => {
                                    work.push(Task::Part(item))
                                }
                                crate::shapes::StackPart::Stack(var) => {
                                    let segment = inst
                                        .stack(*var)
                                        .ok_or(crate::words::InstError::UnknownVariable)?;
                                    work.push(Task::Emit(segment.to_vec()));
                                }
                            }
                        }
                    }
                },
            }
        }
        let mut out = segments
            .pop()
            .ok_or(crate::words::InstError::OversizedType)?;
        if segments.is_empty() && out.len() == 1 {
            match out.pop() {
                Some(ty) => Ok(ty),
                None => Err(crate::words::InstError::OversizedType),
            }
        } else {
            Err(crate::words::InstError::OversizedType)
        }
    }
}

fn single(
    segment: alloc::vec::Vec<crate::types::Ty>,
) -> Result<crate::types::Ty, crate::words::InstError> {
    let mut items = segment;
    if items.len() == 1 {
        match items.pop() {
            Some(ty) => Ok(ty),
            None => Err(crate::words::InstError::OversizedType),
        }
    } else {
        Err(crate::words::InstError::OversizedType)
    }
}

fn pair_segment(
    left: alloc::vec::Vec<crate::types::Ty>,
    right: alloc::vec::Vec<crate::types::Ty>,
    is_sum: bool,
) -> Result<alloc::vec::Vec<crate::types::Ty>, crate::words::InstError> {
    let left_ty = single(left)?;
    let right_ty = single(right)?;
    let built = if is_sum {
        crate::types::Ty::Sum(
            alloc::boxed::Box::new(left_ty),
            alloc::boxed::Box::new(right_ty),
        )
    } else {
        crate::types::Ty::Pair(
            alloc::boxed::Box::new(left_ty),
            alloc::boxed::Box::new(right_ty),
        )
    };
    Ok(alloc::vec![built])
}
