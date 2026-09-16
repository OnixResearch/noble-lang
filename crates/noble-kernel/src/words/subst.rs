//! Iterative post-order substitution of one pattern into a concrete type.
//!
//! One bounded work stack drives the walk; nested program patterns expand
//! through explicit segments, and no step recurses. Each step threads its
//! stacks in and returns them, so no loop body returns early.

/// Local bound for one substitution walk; beyond it substitution rejects.
const WORK_CAP: usize = 512;

/// One substitution step: a pattern, a constructor completion, a nested
/// program expansion, or a pre-built segment emission.
enum Task<'a> {
    Part(&'a crate::shapes::Pattern),
    Finish(&'a crate::shapes::Pattern),
    Expand(&'a crate::shapes::Pattern),
    Emit(alloc::vec::Vec<crate::types::Ty>),
}

/// The state one substitution walk threads through.
struct Walk<'a> {
    /// Pending tasks.
    work: alloc::vec::Vec<Task<'a>>,
    /// Finished segments, oldest segment first.
    segments: alloc::vec::Vec<alloc::vec::Vec<crate::types::Ty>>,
}

/// One walk step's outcome.
enum StepResult {
    /// More tasks remain.
    Continue,
    /// Every task was processed.
    Done,
    /// The walk rejected the pattern.
    Failed(crate::words::InstError),
}

/// One step's returned state: the walk and the step outcome.
type StepState<'a> = (Walk<'a>, Result<(), crate::words::InstError>);

impl crate::words::Scheme {
    /// Substitute one type pattern to a concrete type.
    pub fn subst_pattern(
        &self,
        pattern: &crate::shapes::Pattern,
        inst: &crate::words::Inst,
    ) -> Result<crate::types::Ty, crate::words::InstError> {
        let mut walk = Walk {
            work: alloc::vec::Vec::with_capacity(8),
            segments: alloc::vec::Vec::with_capacity(8),
        };
        walk.work.push(Task::Part(pattern));
        let mut outcome = StepResult::Continue;
        while matches!(outcome, StepResult::Continue) {
            let (next, step) = walk_step(self, inst, walk);
            walk = next;
            outcome = step;
        }
        match outcome {
            StepResult::Failed(problem) => Err(problem),
            StepResult::Continue | StepResult::Done => {
                let out = match walk.segments.pop() {
                    Some(segment) => segment,
                    None => return Err(crate::words::InstError::OversizedType),
                };
                if walk.segments.is_empty() {
                    super::segments::single(out)
                } else {
                    Err(crate::words::InstError::OversizedType)
                }
            }
        }
    }
}

/// Run one substitution step, returning the updated walk and its outcome.
fn walk_step<'a>(
    scheme: &crate::words::Scheme,
    inst: &crate::words::Inst,
    walk: Walk<'a>,
) -> (Walk<'a>, StepResult) {
    let mut walk = walk;
    let task = match walk.work.pop() {
        Some(task) => task,
        None => return (walk, StepResult::Done),
    };
    if walk.work.len() >= WORK_CAP {
        return (
            walk,
            StepResult::Failed(crate::words::InstError::OversizedType),
        );
    }
    let (next, outcome) = match task {
        Task::Emit(segment) => {
            walk.segments.push(segment);
            (walk, Ok(()))
        }
        Task::Finish(node) => finish_task(node, walk),
        Task::Expand(node) => expand_task(scheme, node, inst, walk),
        Task::Part(node) => part_task(node, inst, walk),
    };
    match outcome {
        Ok(()) => (next, StepResult::Continue),
        Err(problem) => (next, StepResult::Failed(problem)),
    }
}

/// Complete one `pair`, `sum`, or `list` pattern from its finished segments.
fn finish_task<'a>(node: &'a crate::shapes::Pattern, mut walk: Walk<'a>) -> StepState<'a> {
    let right = match walk.segments.pop() {
        Some(segment) => segment,
        None => return (walk, Err(crate::words::InstError::OversizedType)),
    };
    let left = match walk.segments.pop() {
        Some(segment) => segment,
        None => return (walk, Err(crate::words::InstError::OversizedType)),
    };
    let built = match node {
        crate::shapes::Pattern::Pair(_, _) => super::segments::pair_segment(left, right, false),
        crate::shapes::Pattern::Sum(_, _) => super::segments::pair_segment(left, right, true),
        crate::shapes::Pattern::List(_) => match super::segments::single(left) {
            Ok(inner) => Ok(alloc::vec![crate::types::Ty::List(alloc::boxed::Box::new(
                inner
            ))]),
            Err(problem) => Err(problem),
        },
        crate::shapes::Pattern::Var(_)
        | crate::shapes::Pattern::StackVar(_)
        | crate::shapes::Pattern::Program(_, _, _)
        | crate::shapes::Pattern::Unit
        | crate::shapes::Pattern::Bool
        | crate::shapes::Pattern::I64
        | crate::shapes::Pattern::Text
        | crate::shapes::Pattern::Syntax
        | crate::shapes::Pattern::Resource(_) => Err(crate::words::InstError::KindMismatch),
    };
    match built {
        Ok(segment) => {
            walk.segments.push(segment);
            (walk, Ok(()))
        }
        Err(problem) => (walk, Err(problem)),
    }
}

/// Expand one `program` pattern from the segments of its parts.
fn expand_task<'a>(
    scheme: &crate::words::Scheme,
    node: &'a crate::shapes::Pattern,
    inst: &crate::words::Inst,
    mut walk: Walk<'a>,
) -> StepState<'a> {
    let (parts_in, parts_out, slots) = match node {
        crate::shapes::Pattern::Program(stack_in, stack_out, effects) => (
            stack_in.as_slice(),
            stack_out.as_slice(),
            effects.as_slice(),
        ),
        _ => return (walk, Err(crate::words::InstError::KindMismatch)),
    };
    let wanted = parts_in.len() + parts_out.len();
    let mut collected: alloc::vec::Vec<alloc::vec::Vec<crate::types::Ty>> =
        alloc::vec::Vec::with_capacity(wanted.max(4));
    let mut popped = 0;
    let mut is_missing = false;
    while popped < wanted {
        match walk.segments.pop() {
            Some(segment) => {
                collected.push(segment);
                popped += 1;
            }
            None => {
                is_missing = true;
                break;
            }
        }
    }
    if is_missing {
        return (walk, Err(crate::words::InstError::OversizedType));
    }
    collected.reverse();
    let mut parts = collected.into_iter();
    let mut stack_in: alloc::vec::Vec<crate::types::Ty> =
        alloc::vec::Vec::with_capacity(parts_in.len().max(4));
    let mut in_step = 0;
    while in_step < parts_in.len() {
        if let Some(segment) = parts.next() {
            stack_in.extend(segment);
        }
        in_step += 1;
    }
    let mut stack_out: alloc::vec::Vec<crate::types::Ty> =
        alloc::vec::Vec::with_capacity(parts_out.len().max(4));
    let mut out_step = 0;
    while out_step < parts_out.len() {
        if let Some(segment) = parts.next() {
            stack_out.extend(segment);
        }
        out_step += 1;
    }
    let effects = match scheme.subst_effects(slots, inst) {
        Ok(effects) => effects,
        Err(problem) => return (walk, Err(problem)),
    };
    walk.segments.push(alloc::vec![crate::types::Ty::program(
        stack_in, stack_out, effects
    )]);
    (walk, Ok(()))
}

/// Emit the segment of one pattern, queueing its sub-patterns.
fn part_task<'a>(
    node: &'a crate::shapes::Pattern,
    inst: &crate::words::Inst,
    mut walk: Walk<'a>,
) -> StepState<'a> {
    match node {
        crate::shapes::Pattern::Unit => walk.segments.push(alloc::vec![crate::types::Ty::Unit]),
        crate::shapes::Pattern::Bool => walk.segments.push(alloc::vec![crate::types::Ty::Bool]),
        crate::shapes::Pattern::I64 => walk.segments.push(alloc::vec![crate::types::Ty::I64]),
        crate::shapes::Pattern::Text => walk.segments.push(alloc::vec![crate::types::Ty::Text]),
        crate::shapes::Pattern::Syntax => walk.segments.push(alloc::vec![crate::types::Ty::Syntax]),
        crate::shapes::Pattern::Resource(kind) => walk
            .segments
            .push(alloc::vec![crate::types::Ty::Resource(*kind)]),
        crate::shapes::Pattern::Var(var) => {
            let ty = match inst.value(*var) {
                Some(ty) => ty.clone(),
                None => return (walk, Err(crate::words::InstError::UnknownVariable)),
            };
            walk.segments.push(alloc::vec![ty]);
        }
        crate::shapes::Pattern::StackVar(var) => match inst.stack(*var) {
            Some(segment) => walk.work.push(Task::Emit(segment.to_vec())),
            None => return (walk, Err(crate::words::InstError::UnknownVariable)),
        },
        crate::shapes::Pattern::Pair(left, right) | crate::shapes::Pattern::Sum(left, right) => {
            walk.work.push(Task::Finish(node));
            walk.work.push(Task::Part(left));
            walk.work.push(Task::Part(right));
        }
        crate::shapes::Pattern::List(item) => {
            walk.work.push(Task::Finish(node));
            walk.work.push(Task::Part(item));
        }
        crate::shapes::Pattern::Program(stack_in, stack_out, _) => {
            walk.work.push(Task::Expand(node));
            walk = queue_parts(stack_in.as_slice(), stack_out.as_slice(), walk);
        }
    }
    (walk, Ok(()))
}

/// Queue the parts of one program pattern, result stack first.
fn queue_parts<'a>(
    stack_in: &'a [crate::shapes::Pattern],
    stack_out: &'a [crate::shapes::Pattern],
    mut walk: Walk<'a>,
) -> Walk<'a> {
    let out_len = stack_out.len();
    let in_len = stack_in.len();
    let mut part_index = 0;
    while part_index < in_len + out_len {
        let part = if part_index < out_len {
            &stack_out[out_len - 1 - part_index]
        } else {
            &stack_in[in_len + out_len - 1 - part_index]
        };
        walk.work.push(Task::Part(part));
        part_index += 1;
    }
    walk
}
