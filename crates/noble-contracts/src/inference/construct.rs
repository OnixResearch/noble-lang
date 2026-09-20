impl super::Arena {
    pub(super) fn build_ty<'a>(
        &mut self,
        ty: &'a noble_kernel::types::Ty,
        mut state: super::build::State<'a>,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<super::build::State<'a>, crate::Diagnostic> {
        let term = match ty {
            noble_kernel::types::Ty::Unit => super::Term::Unit,
            noble_kernel::types::Ty::Bool => super::Term::Bool,
            noble_kernel::types::Ty::I64 => super::Term::I64,
            noble_kernel::types::Ty::Text => super::Term::Text,
            noble_kernel::types::Ty::Syntax => super::Term::Syntax,
            noble_kernel::types::Ty::Pair(a, b) | noble_kernel::types::Ty::Sum(a, b) => {
                state
                    .steps
                    .push(if matches!(ty, noble_kernel::types::Ty::Pair(_, _)) {
                        super::build::Step::Pair
                    } else {
                        super::build::Step::Sum
                    });
                state.steps.push(super::build::Step::Ty(b));
                state.steps.push(super::build::Step::Ty(a));
                return Ok(state);
            }
            noble_kernel::types::Ty::List(item) => {
                state.steps.push(super::build::Step::List);
                state.steps.push(super::build::Step::Ty(item));
                return Ok(state);
            }
            noble_kernel::types::Ty::Program(inputs, outputs, effects) => {
                if !effects.is_empty() {
                    return Err(crate::Diagnostic::new(
                        crate::DiagnosticKind::Unsupported,
                        span,
                        "effectful Program is outside the pure fragment",
                    ));
                }
                state.steps.push(super::build::Step::Program);
                state.steps.push(super::build::Step::StackTy(outputs));
                state.steps.push(super::build::Step::StackTy(inputs));
                return Ok(state);
            }
            noble_kernel::types::Ty::Resource(_) => {
                return Err(crate::Diagnostic::new(
                    crate::DiagnosticKind::Unsupported,
                    span,
                    "host resources are outside the pure fragment",
                ));
            }
        };
        state.values.push(attempt!(self.add(term, span, meter)));
        Ok(state)
    }

    pub(super) fn build_pattern<'a>(
        &mut self,
        pattern: &'a noble_kernel::shapes::Pattern,
        mut state: super::build::State<'a>,
        variables: &[super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<super::build::State<'a>, crate::Diagnostic> {
        let term = match pattern {
            noble_kernel::shapes::Pattern::Unit => super::Term::Unit,
            noble_kernel::shapes::Pattern::Bool => super::Term::Bool,
            noble_kernel::shapes::Pattern::I64 => super::Term::I64,
            noble_kernel::shapes::Pattern::Text => super::Term::Text,
            noble_kernel::shapes::Pattern::Syntax => super::Term::Syntax,
            noble_kernel::shapes::Pattern::Pair(a, b)
            | noble_kernel::shapes::Pattern::Sum(a, b) => {
                state.steps.push(
                    if matches!(pattern, noble_kernel::shapes::Pattern::Pair(_, _)) {
                        super::build::Step::Pair
                    } else {
                        super::build::Step::Sum
                    },
                );
                state.steps.push(super::build::Step::Pattern(b));
                state.steps.push(super::build::Step::Pattern(a));
                return Ok(state);
            }
            noble_kernel::shapes::Pattern::List(item) => {
                state.steps.push(super::build::Step::List);
                state.steps.push(super::build::Step::Pattern(item));
                return Ok(state);
            }
            noble_kernel::shapes::Pattern::Program(inputs, outputs, effects) => {
                attempt!(super::pure_effects(effects, variables, span, meter));
                state.steps.push(super::build::Step::Program);
                state.steps.push(super::build::Step::StackPattern(outputs));
                state.steps.push(super::build::Step::StackPattern(inputs));
                return Ok(state);
            }
            noble_kernel::shapes::Pattern::Var(variable) => {
                match super::variable_at(variables, variable.0, span) {
                    Ok(super::Variable::Value(id)) => state.values.push(id),
                    Ok(super::Variable::Stack(_) | super::Variable::Effect) | Err(_) => {
                        return Err(crate::internal(span));
                    }
                }
                return Ok(state);
            }
            noble_kernel::shapes::Pattern::Resource(_)
            | noble_kernel::shapes::Pattern::StackVar(_) => return Err(crate::internal(span)),
        };
        state.values.push(attempt!(self.add(term, span, meter)));
        Ok(state)
    }
}
