impl<'a> super::build::State<'a> {
    pub(super) fn build_ty(
        mut self,
        arena: &mut super::Arena,
        ty: &'a noble_kernel::types::Ty,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<Self, crate::Diagnostic> {
        let term = match ty {
            noble_kernel::types::Ty::Unit => super::Term::Unit,
            noble_kernel::types::Ty::Bool => super::Term::Bool,
            noble_kernel::types::Ty::I64 => super::Term::I64,
            noble_kernel::types::Ty::Text => super::Term::Text,
            noble_kernel::types::Ty::Syntax => super::Term::Syntax,
            noble_kernel::types::Ty::Pair(a, b) | noble_kernel::types::Ty::Sum(a, b) => {
                self.steps
                    .push(if matches!(ty, noble_kernel::types::Ty::Pair(_, _)) {
                        super::build::Step::Pair
                    } else {
                        super::build::Step::Sum
                    });
                self.steps.push(super::build::Step::Ty(b));
                self.steps.push(super::build::Step::Ty(a));
                return Ok(self);
            }
            noble_kernel::types::Ty::List(item) => {
                self.steps.push(super::build::Step::List);
                self.steps.push(super::build::Step::Ty(item));
                return Ok(self);
            }
            noble_kernel::types::Ty::Program(inputs, outputs, effects) => {
                if !arena.effectful && !effects.is_empty() {
                    return Err(crate::Diagnostic::new(
                        crate::DiagnosticKind::Unsupported,
                        span,
                        "effectful Program is outside the pure fragment",
                    ));
                }
                let effect = if arena.effectful {
                    Some(attempt!(arena.effect_constant(effects, span, meter)))
                } else {
                    None
                };
                self.steps.push(super::build::Step::Program(effect));
                self.steps.push(super::build::Step::StackTy(outputs));
                self.steps.push(super::build::Step::StackTy(inputs));
                return Ok(self);
            }
            noble_kernel::types::Ty::Resource(_) => {
                return Err(crate::Diagnostic::new(
                    crate::DiagnosticKind::Unsupported,
                    span,
                    "host resources are outside the pure fragment",
                ));
            }
        };
        self.values.push(attempt!(arena.add(term, span, meter)));
        Ok(self)
    }

    pub(super) fn build_pattern(
        mut self,
        arena: &mut super::Arena,
        pattern: &'a noble_kernel::shapes::Pattern,
        variables: &[super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<Self, crate::Diagnostic> {
        let term = match pattern {
            noble_kernel::shapes::Pattern::Unit => super::Term::Unit,
            noble_kernel::shapes::Pattern::Bool => super::Term::Bool,
            noble_kernel::shapes::Pattern::I64 => super::Term::I64,
            noble_kernel::shapes::Pattern::Text => super::Term::Text,
            noble_kernel::shapes::Pattern::Syntax => super::Term::Syntax,
            noble_kernel::shapes::Pattern::Pair(a, b)
            | noble_kernel::shapes::Pattern::Sum(a, b) => {
                self.steps.push(
                    if matches!(pattern, noble_kernel::shapes::Pattern::Pair(_, _)) {
                        super::build::Step::Pair
                    } else {
                        super::build::Step::Sum
                    },
                );
                self.steps.push(super::build::Step::Pattern(b));
                self.steps.push(super::build::Step::Pattern(a));
                return Ok(self);
            }
            noble_kernel::shapes::Pattern::List(item) => {
                self.steps.push(super::build::Step::List);
                self.steps.push(super::build::Step::Pattern(item));
                return Ok(self);
            }
            noble_kernel::shapes::Pattern::Program(inputs, outputs, effects) => {
                let effect = if arena.effectful {
                    Some(attempt!(
                        arena.effect_pattern(effects, variables, span, meter)
                    ))
                } else {
                    attempt!(super::pure_effects(effects, variables, span, meter));
                    None
                };
                self.steps.push(super::build::Step::Program(effect));
                self.steps.push(super::build::Step::StackPattern(outputs));
                self.steps.push(super::build::Step::StackPattern(inputs));
                return Ok(self);
            }
            noble_kernel::shapes::Pattern::Var(variable) => {
                match super::variable_at(variables, variable.0, span) {
                    Ok(super::Variable::Value(id)) => self.values.push(id),
                    Ok(
                        super::Variable::Stack(_)
                        | super::Variable::Effect
                        | super::Variable::EffectValue(_),
                    )
                    | Err(_) => {
                        return Err(crate::internal(span));
                    }
                }
                return Ok(self);
            }
            noble_kernel::shapes::Pattern::Resource(_)
            | noble_kernel::shapes::Pattern::StackVar(_) => return Err(crate::internal(span)),
        };
        self.values.push(attempt!(arena.add(term, span, meter)));
        Ok(self)
    }
}
