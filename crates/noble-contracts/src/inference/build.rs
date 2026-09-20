#[octet::sealed_enum]
pub(super) enum Step<'a> {
    Ty(&'a noble_kernel::types::Ty),
    Pattern(&'a noble_kernel::shapes::Pattern),
    StackTy(&'a [noble_kernel::types::Ty]),
    StackPattern(&'a [noble_kernel::shapes::Pattern]),
    StackTyParts(&'a [noble_kernel::types::Ty], usize),
    StackPatternParts(&'a [noble_kernel::shapes::Pattern], usize, usize),
    Pair,
    Sum,
    List,
    Program,
    Push,
}

pub(super) struct State<'a> {
    pub(super) steps: alloc::vec::Vec<Step<'a>>,
    pub(super) values: alloc::vec::Vec<u32>,
}

impl super::Arena {
    pub fn stack(
        &mut self,
        stack: &[noble_kernel::types::Ty],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<u32, crate::Diagnostic> {
        self.build(Step::StackTy(stack), &[], span, meter)
    }

    pub fn ty(
        &mut self,
        ty: &noble_kernel::types::Ty,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<u32, crate::Diagnostic> {
        self.build(Step::Ty(ty), &[], span, meter)
    }

    pub fn pattern_stack(
        &mut self,
        stack: &[noble_kernel::shapes::Pattern],
        variables: &[super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<u32, crate::Diagnostic> {
        self.build(Step::StackPattern(stack), variables, span, meter)
    }

    fn build(
        &mut self,
        root: Step<'_>,
        variables: &[super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<u32, crate::Diagnostic> {
        let mut initial = State {
            steps: alloc::vec::Vec::with_capacity(1),
            values: alloc::vec::Vec::new(),
        };
        initial.steps.push(root);
        let mut outcome = Ok(initial);
        while let Ok(mut state) = outcome {
            match state.steps.pop() {
                Some(step) => outcome = self.build_step(step, state, variables, span, meter),
                None => {
                    outcome = Ok(state);
                    break;
                }
            }
        }
        let mut state = attempt!(outcome);
        if state.values.len() != 1 {
            return Err(crate::internal(span));
        }
        require_id(state.values.pop(), span)
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; this allocating worklist interpreter must explicitly handle every Step variant, including impossible constructor cases as typed internal errors."
    )]
    fn build_step<'a>(
        &mut self,
        step: Step<'a>,
        mut state: State<'a>,
        variables: &[super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<State<'a>, crate::Diagnostic> {
        attempt!(meter.charge(1, span));
        match step {
            Step::Ty(ty) => self.build_ty(ty, state, span, meter),
            Step::Pattern(pattern) => self.build_pattern(pattern, state, variables, span, meter),
            Step::StackTy(stack) => self.build_stack_ty(stack, state, span, meter),
            Step::StackPattern(stack) => {
                self.build_stack_pattern(stack, state, variables, span, meter)
            }
            Step::StackTyParts(stack, at) => stack_ty_step(stack, at, state, span),
            Step::StackPatternParts(stack, start, at) => {
                stack_pattern_step(stack, start, at, state, span)
            }
            Step::Pair | Step::Sum | Step::Program | Step::Push => {
                let b = attempt!(require_id(state.values.pop(), span));
                let a = attempt!(require_id(state.values.pop(), span));
                let term = match step {
                    Step::Pair => super::Term::Pair(a, b),
                    Step::Sum => super::Term::Sum(a, b),
                    Step::Program => super::Term::Program(a, b),
                    Step::Push => super::Term::Push(a, b),
                    Step::Ty(_)
                    | Step::Pattern(_)
                    | Step::StackTy(_)
                    | Step::StackPattern(_)
                    | Step::StackTyParts(_, _)
                    | Step::StackPatternParts(_, _, _)
                    | Step::List => return Err(crate::internal(span)),
                };
                state.values.push(attempt!(self.add(term, span, meter)));
                Ok(state)
            }
            Step::List => {
                let item = attempt!(require_id(state.values.pop(), span));
                state
                    .values
                    .push(attempt!(self.add(super::Term::List(item), span, meter)));
                Ok(state)
            }
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-maintainers; this step allocates arena/worklist entries. The immediately preceding nonempty check proves len - 1 cannot underflow."
    )]
    fn build_stack_ty<'a>(
        &mut self,
        stack: &'a [noble_kernel::types::Ty],
        mut state: State<'a>,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<State<'a>, crate::Diagnostic> {
        state
            .values
            .push(attempt!(self.add(super::Term::Empty, span, meter)));
        if !stack.is_empty() {
            state.steps.push(Step::StackTyParts(stack, stack.len() - 1));
        }
        Ok(state)
    }

    #[expect(
        tigerstyle::missing_const_fn,
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-maintainers; this step allocates arena/worklist entries. len > start and start >= 0 prove len - 1 cannot underflow."
    )]
    fn build_stack_pattern<'a>(
        &mut self,
        stack: &'a [noble_kernel::shapes::Pattern],
        mut state: State<'a>,
        variables: &[super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<State<'a>, crate::Diagnostic> {
        let mut start = 0usize;
        let initial = match stack.first() {
            Some(noble_kernel::shapes::Pattern::StackVar(variable)) => {
                start = 1;
                match attempt!(super::variable_at(variables, variable.0, span)) {
                    super::Variable::Stack(id) => id,
                    super::Variable::Value(_) | super::Variable::Effect => {
                        return Err(crate::internal(span));
                    }
                }
            }
            Some(
                noble_kernel::shapes::Pattern::Unit
                | noble_kernel::shapes::Pattern::Bool
                | noble_kernel::shapes::Pattern::I64
                | noble_kernel::shapes::Pattern::Text
                | noble_kernel::shapes::Pattern::Syntax
                | noble_kernel::shapes::Pattern::Pair(_, _)
                | noble_kernel::shapes::Pattern::Sum(_, _)
                | noble_kernel::shapes::Pattern::List(_)
                | noble_kernel::shapes::Pattern::Program(_, _, _)
                | noble_kernel::shapes::Pattern::Var(_)
                | noble_kernel::shapes::Pattern::Resource(_),
            )
            | None => attempt!(self.add(super::Term::Empty, span, meter)),
        };
        state.values.push(initial);
        if stack.len() > start {
            state
                .steps
                .push(Step::StackPatternParts(stack, start, stack.len() - 1));
        }
        Ok(state)
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; scheduling grows a Vec and is non-const. The local at > 0 guard proves at - 1 cannot underflow."
)]
fn stack_ty_step<'a>(
    stack: &'a [noble_kernel::types::Ty],
    at: usize,
    mut state: State<'a>,
    span: crate::Span,
) -> Result<State<'a>, crate::Diagnostic> {
    let ty = match stack.get(at) {
        Some(ty) => ty,
        None => return Err(crate::internal(span)),
    };
    state.steps.push(Step::Push);
    state.steps.push(Step::Ty(ty));
    // Scan first, then evaluate: every scan keeps its original charge and order.
    // The continuation and its borrowed elements move together through build.
    if at > 0 {
        state.steps.push(Step::StackTyParts(stack, at - 1));
    }
    Ok(state)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the missing-ID path constructs an owned internal diagnostic."
)]
fn require_id(value: Option<u32>, span: crate::Span) -> Result<u32, crate::Diagnostic> {
    match value {
        Some(value) => Ok(value),
        None => Err(crate::internal(span)),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-maintainers; scheduling grows a Vec. start and at are related bounds of the same reverse scan; at > start proves at - 1 is safe and keeps the prefix excluded."
)]
fn stack_pattern_step<'a>(
    stack: &'a [noble_kernel::shapes::Pattern],
    start: usize,
    at: usize,
    mut state: State<'a>,
    span: crate::Span,
) -> Result<State<'a>, crate::Diagnostic> {
    let pattern = match stack.get(at) {
        Some(pattern) => pattern,
        None => return Err(crate::internal(span)),
    };
    state.steps.push(Step::Push);
    state.steps.push(Step::Pattern(pattern));
    if at > start {
        state
            .steps
            .push(Step::StackPatternParts(stack, start, at - 1));
    }
    Ok(state)
}
