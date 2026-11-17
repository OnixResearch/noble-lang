impl super::Arena {
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-maintainers; child types are built by direct post-order recursion, each step charged to the work meter in the original worklist order; a borrowed work stack is untranslatable by the pinned Aeneas."
    )]
    /// Charge one step, then build `ty` after its children and push its term.
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-maintainers; the build pushes terms onto its growable value stack, which a slice cannot express."
    )]
    pub(super) fn build_ty(
        &mut self,
        ty: &noble_kernel::types::Ty,
        values: &mut alloc::vec::Vec<u32>,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        attempt!(meter.charge(1, span));
        let term = match ty {
            noble_kernel::types::Ty::Unit => super::Term::Unit,
            noble_kernel::types::Ty::Bool => super::Term::Bool,
            noble_kernel::types::Ty::I64 => super::Term::I64,
            noble_kernel::types::Ty::Text => super::Term::Text,
            noble_kernel::types::Ty::Syntax => super::Term::Syntax,
            noble_kernel::types::Ty::Contract => super::Term::Contract,
            noble_kernel::types::Ty::Evidence => super::Term::Evidence,
            noble_kernel::types::Ty::Certified => super::Term::Certified,
            noble_kernel::types::Ty::Nominal(id, shape) => {
                if crate::scan::find_first(&self.nominals, |(known, checked)| {
                    *known == *id && *checked == **shape
                })
                .is_none()
                {
                    return Err(crate::invalid(
                        span,
                        "unregistered nominal type or changed representation",
                    ));
                }
                super::Term::Nominal(*id)
            }
            noble_kernel::types::Ty::GenericNominal(id, arguments, shape) => {
                let Some(declaration) =
                    crate::scan::find_first(&self.generic_nominals, |decl| decl.id == *id)
                else {
                    return Err(crate::invalid(span, "unregistered generic nominal family"));
                };
                let expected = noble_kernel::types::NominalShape::Variant(
                    alloc::boxed::Box::new(
                        arguments[usize::from(declaration.payload_params[0])].clone(),
                    ),
                    alloc::boxed::Box::new(
                        arguments[usize::from(declaration.payload_params[1])].clone(),
                    ),
                );
                if **shape != expected {
                    return Err(crate::invalid(
                        span,
                        "changed generic nominal representation",
                    ));
                }
                attempt!(self.build_ty(&arguments[0], values, span, meter));
                attempt!(self.build_ty(&arguments[1], values, span, meter));
                return self.join(super::build::Join::GenericNominal(*id), values, span, meter);
            }
            noble_kernel::types::Ty::Pair(a, b) | noble_kernel::types::Ty::Sum(a, b) => {
                let join = if matches!(ty, noble_kernel::types::Ty::Pair(_, _)) {
                    super::build::Join::Pair
                } else {
                    super::build::Join::Sum
                };
                attempt!(self.build_ty(a, values, span, meter));
                attempt!(self.build_ty(b, values, span, meter));
                return self.join(join, values, span, meter);
            }
            noble_kernel::types::Ty::List(item) => {
                attempt!(self.build_ty(item, values, span, meter));
                return self.join(super::build::Join::List, values, span, meter);
            }
            noble_kernel::types::Ty::Program(inputs, outputs, effects) => {
                if !self.effectful && !effects.is_empty() {
                    return Err(crate::Diagnostic::new(
                        crate::DiagnosticKind::Unsupported,
                        span,
                        "effectful Program is outside the pure fragment",
                    ));
                }
                let effect = if self.effectful {
                    Some(attempt!(
                        self.program_type_effect_constant(effects, span, meter)
                    ))
                } else {
                    None
                };
                attempt!(self.build_stack_ty(inputs, values, span, meter));
                attempt!(self.build_stack_ty(outputs, values, span, meter));
                return self.join(super::build::Join::Program(effect), values, span, meter);
            }
            noble_kernel::types::Ty::LiveRef(..) => {
                return Err(crate::invalid(
                    span,
                    "LiveRef is not a value; it belongs only to a live root input sidecar",
                ));
            }
            noble_kernel::types::Ty::Resource(kind) => {
                if self.resources {
                    values.push(attempt!(self.add(
                        super::Term::Resource(*kind),
                        span,
                        meter
                    )));
                    return Ok(());
                }
                return Err(crate::Diagnostic::new(
                    crate::DiagnosticKind::Unsupported,
                    span,
                    "host resources are outside the pure fragment",
                ));
            }
        };
        values.push(attempt!(self.add(term, span, meter)));
        Ok(())
    }

    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-maintainers; child patterns are built by direct post-order recursion, each step charged to the work meter in the original worklist order; a borrowed work stack is untranslatable by the pinned Aeneas."
    )]
    /// Charge one step, then build `pattern` after its children and push its term.
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-maintainers; the build pushes terms onto its growable value stack, which a slice cannot express."
    )]
    pub(super) fn build_pattern(
        &mut self,
        pattern: &noble_kernel::shapes::Pattern,
        values: &mut alloc::vec::Vec<u32>,
        variables: &[super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        attempt!(meter.charge(1, span));
        let term = match pattern {
            noble_kernel::shapes::Pattern::Unit => super::Term::Unit,
            noble_kernel::shapes::Pattern::Bool => super::Term::Bool,
            noble_kernel::shapes::Pattern::I64 => super::Term::I64,
            noble_kernel::shapes::Pattern::Text => super::Term::Text,
            noble_kernel::shapes::Pattern::Syntax => super::Term::Syntax,
            noble_kernel::shapes::Pattern::Contract => super::Term::Contract,
            noble_kernel::shapes::Pattern::Evidence => super::Term::Evidence,
            noble_kernel::shapes::Pattern::Certified => super::Term::Certified,
            noble_kernel::shapes::Pattern::Nominal(id, shape) => {
                if crate::scan::find_first(&self.nominals, |(known, checked)| {
                    *known == *id && *checked == **shape
                })
                .is_none()
                {
                    return Err(crate::invalid(
                        span,
                        "unregistered nominal pattern or changed representation",
                    ));
                }
                super::Term::Nominal(*id)
            }
            noble_kernel::shapes::Pattern::GenericNominal(id, arguments, payload_params) => {
                if crate::scan::find_first(&self.generic_nominals, |declaration| {
                    declaration.id == *id && declaration.payload_params == *payload_params
                })
                .is_none()
                {
                    return Err(crate::invalid(span, "unregistered generic nominal pattern"));
                }
                attempt!(self.build_pattern(&arguments[0], values, variables, span, meter));
                attempt!(self.build_pattern(&arguments[1], values, variables, span, meter));
                return self.join(super::build::Join::GenericNominal(*id), values, span, meter);
            }
            noble_kernel::shapes::Pattern::Pair(a, b)
            | noble_kernel::shapes::Pattern::Sum(a, b) => {
                let join = if matches!(pattern, noble_kernel::shapes::Pattern::Pair(_, _)) {
                    super::build::Join::Pair
                } else {
                    super::build::Join::Sum
                };
                attempt!(self.build_pattern(a, values, variables, span, meter));
                attempt!(self.build_pattern(b, values, variables, span, meter));
                return self.join(join, values, span, meter);
            }
            noble_kernel::shapes::Pattern::List(item) => {
                attempt!(self.build_pattern(item, values, variables, span, meter));
                return self.join(super::build::Join::List, values, span, meter);
            }
            noble_kernel::shapes::Pattern::Program(inputs, outputs, effects) => {
                let effect = if self.effectful {
                    Some(attempt!(self.program_type_effect_pattern(
                        effects, variables, span, meter
                    )))
                } else {
                    attempt!(super::pure_effects(effects, variables, span, meter));
                    None
                };
                attempt!(self.build_stack_pattern(inputs, values, variables, span, meter));
                attempt!(self.build_stack_pattern(outputs, values, variables, span, meter));
                return self.join(super::build::Join::Program(effect), values, span, meter);
            }
            noble_kernel::shapes::Pattern::LiveRef(..) => {
                return Err(crate::invalid(
                    span,
                    "LiveRef cannot be instantiated as a value or captured in a Program",
                ));
            }
            noble_kernel::shapes::Pattern::Var(variable) => {
                match super::variable_at(variables, variable.0, span) {
                    Ok(super::Variable::Value(id)) => values.push(id),
                    Ok(
                        super::Variable::Stack(_)
                        | super::Variable::Effect
                        | super::Variable::EffectValue(_),
                    )
                    | Err(_) => {
                        return Err(crate::internal(span));
                    }
                }
                return Ok(());
            }
            noble_kernel::shapes::Pattern::Resource(kind) => {
                if !self.resources {
                    return Err(crate::invalid(
                        span,
                        "resource pattern requires a component environment",
                    ));
                }
                super::Term::Resource(*kind)
            }
            noble_kernel::shapes::Pattern::StackVar(_) => return Err(crate::internal(span)),
        };
        values.push(attempt!(self.add(term, span, meter)));
        Ok(())
    }
}
