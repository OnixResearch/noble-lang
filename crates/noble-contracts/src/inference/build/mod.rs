mod stack;

/// A constructor applied once its operands are built: it pops them from the
/// value stack and pushes the new term.
#[octet::sealed_enum]
pub(super) enum Join {
    Pair,
    Sum,
    GenericNominal(noble_kernel::types::NominalTypeId),
    List,
    Program(Option<u32>),
    Push,
}

impl super::Arena {
    pub fn rigid_variables(
        &mut self,
        kinds: &[noble_kernel::words::VariableKind],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<alloc::vec::Vec<super::Variable>, crate::Diagnostic> {
        let mut variables = alloc::vec::Vec::with_capacity(kinds.len());
        let mut failure = None;
        let mut at = 0;
        while at < kinds.len() {
            match self.rigid_variable(&kinds[at], span, meter) {
                Ok(variable) => variables.push(variable),
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
            at += 1;
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(variables),
        }
    }

    /// Allocate the next rigid binder of `kind`.
    fn rigid_variable(
        &mut self,
        kind: &noble_kernel::words::VariableKind,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<super::Variable, crate::Diagnostic> {
        attempt!(meter.node(span));
        let identity = self.rigid_count;
        self.rigid_count = attempt!(identity
            .checked_add(1)
            .ok_or_else(|| { crate::invalid(span, "rigid signature binder limit exceeded") }));
        Ok(match kind {
            noble_kernel::words::VariableKind::Value => super::Variable::Value(attempt!(self.add(
                super::Term::RigidValue(identity),
                span,
                meter
            ))),
            noble_kernel::words::VariableKind::Stack => super::Variable::Stack(attempt!(self.add(
                super::Term::RigidStack(identity),
                span,
                meter
            ))),
            noble_kernel::words::VariableKind::Effect => {
                super::Variable::EffectValue(attempt!(self.rigid_effect(identity, span, meter)))
            }
        })
    }

    pub fn stack(
        &mut self,
        stack: &[noble_kernel::types::Ty],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<u32, crate::Diagnostic> {
        let mut values = alloc::vec::Vec::new();
        attempt!(self.build_stack_ty(stack, &mut values, span, meter));
        built(values, span)
    }

    pub fn ty(
        &mut self,
        ty: &noble_kernel::types::Ty,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<u32, crate::Diagnostic> {
        let mut values = alloc::vec::Vec::new();
        attempt!(self.build_ty(ty, &mut values, span, meter));
        built(values, span)
    }

    pub fn pattern_stack(
        &mut self,
        stack: &[noble_kernel::shapes::Pattern],
        variables: &[super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<u32, crate::Diagnostic> {
        let mut values = alloc::vec::Vec::new();
        attempt!(self.build_stack_pattern(stack, &mut values, variables, span, meter));
        built(values, span)
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; joining charges work, pops built operands and allocates the joined term, with typed internal errors for missing operands."
    )]
    /// Charge one step, pop the operands of `join` and push its new term.
    pub(super) fn join(
        &mut self,
        join: Join,
        values: &mut alloc::vec::Vec<u32>,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        attempt!(meter.charge(1, span));
        match join {
            Join::Pair | Join::Sum | Join::GenericNominal(_) | Join::Program(_) | Join::Push => {
                let b = attempt!(require_id(values.pop(), span));
                let a = attempt!(require_id(values.pop(), span));
                if let Join::Program(Some(effect)) = join {
                    values.push(attempt!(self.program(
                        super::Program {
                            input: a,
                            output: b,
                            effect,
                        },
                        span,
                        meter
                    )));
                    return Ok(());
                }
                let term = match join {
                    Join::Pair => super::Term::Pair(a, b),
                    Join::Sum => super::Term::Sum(a, b),
                    Join::GenericNominal(id) => super::Term::GenericNominal(id, a, b),
                    Join::Program(_) => super::Term::Program(a, b),
                    Join::Push => super::Term::Push(a, b),
                    Join::List => return Err(crate::internal(span)),
                };
                values.push(attempt!(self.add(term, span, meter)));
                Ok(())
            }
            Join::List => {
                let item = attempt!(require_id(values.pop(), span));
                values.push(attempt!(self.add(super::Term::List(item), span, meter)));
                Ok(())
            }
        }
    }

    pub fn variables(
        &mut self,
        kinds: &[noble_kernel::words::VariableKind],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<alloc::vec::Vec<super::Variable>, crate::Diagnostic> {
        let mut variables = alloc::vec::Vec::with_capacity(kinds.len());
        let mut at = 0usize;
        let mut failure = None;
        while at < kinds.len() {
            match self.variable(kinds.get(at), span, meter) {
                Ok(variable) => {
                    variables.push(variable);
                    at += 1;
                }
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(variables),
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; creating a value or stack witness allocates a term in the arena."
    )]
    fn variable(
        &mut self,
        kind: Option<&noble_kernel::words::VariableKind>,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<super::Variable, crate::Diagnostic> {
        attempt!(meter.charge(1, span));
        match kind {
            Some(noble_kernel::words::VariableKind::Value) => Ok(super::Variable::Value(attempt!(
                self.add(super::Term::Hole(super::Sort::Value), span, meter)
            ))),
            Some(noble_kernel::words::VariableKind::Stack) => Ok(super::Variable::Stack(attempt!(
                self.add(super::Term::Hole(super::Sort::Stack), span, meter)
            ))),
            Some(noble_kernel::words::VariableKind::Effect) => {
                if self.effectful {
                    Ok(super::Variable::EffectValue(attempt!(
                        self.effect_hole(span, meter)
                    )))
                } else {
                    Ok(super::Variable::Effect)
                }
            }
            None => Err(crate::internal(span)),
        }
    }
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

/// The single built root left on the value stack.
fn built(mut values: alloc::vec::Vec<u32>, span: crate::Span) -> Result<u32, crate::Diagnostic> {
    if values.len() != 1 {
        return Err(crate::internal(span));
    }
    require_id(values.pop(), span)
}
