impl super::super::Arena {
    /// Charge one step, push the empty base, charge one scan per entry from
    /// the top, then build and push every entry bottom first.
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-maintainers; the build pushes terms onto its growable value stack, which a slice cannot express."
    )]
    pub(in crate::inference) fn build_stack_ty(
        &mut self,
        stack: &[noble_kernel::types::Ty],
        values: &mut alloc::vec::Vec<u32>,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        attempt!(meter.charge(1, span));
        values.push(attempt!(self.add(super::super::Term::Empty, span, meter)));
        attempt!(charge_scans(stack.len(), span, meter));
        self.push_ty_entries(stack, values, span, meter)
    }

    /// Charge one step, push the stack-variable or empty base, then build the
    /// remaining entries as `build_stack_ty` does.
    #[expect(
        tigerstyle::borrowed_argument_types,
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; the build pushes terms onto its growable value stack, which a slice cannot express, and charges the non-const work meter while allocating arena terms."
    )]
    pub(in crate::inference) fn build_stack_pattern(
        &mut self,
        stack: &[noble_kernel::shapes::Pattern],
        values: &mut alloc::vec::Vec<u32>,
        variables: &[super::super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        attempt!(meter.charge(1, span));
        let mut start = 0usize;
        let initial = match stack.first() {
            Some(noble_kernel::shapes::Pattern::StackVar(variable)) => {
                start = 1;
                match attempt!(super::super::variable_at(variables, variable.0, span)) {
                    super::super::Variable::Stack(id) => id,
                    super::super::Variable::Value(_)
                    | super::super::Variable::Effect
                    | super::super::Variable::EffectValue(_) => {
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
                | noble_kernel::shapes::Pattern::Contract
                | noble_kernel::shapes::Pattern::Evidence
                | noble_kernel::shapes::Pattern::Certified
                | noble_kernel::shapes::Pattern::Pair(_, _)
                | noble_kernel::shapes::Pattern::Sum(_, _)
                | noble_kernel::shapes::Pattern::List(_)
                | noble_kernel::shapes::Pattern::Program(_, _, _)
                | noble_kernel::shapes::Pattern::LiveRef(_, _, _)
                | noble_kernel::shapes::Pattern::Var(_)
                | noble_kernel::shapes::Pattern::Resource(_)
                | noble_kernel::shapes::Pattern::Nominal(_, _)
                | noble_kernel::shapes::Pattern::GenericNominal(_, _, _),
            )
            | None => attempt!(self.add(super::super::Term::Empty, span, meter)),
        };
        values.push(initial);
        // The entries after the stack-variable prefix.
        let entries = match stack.get(start..) {
            Some(entries) => entries,
            None => return Err(crate::internal(span)),
        };
        attempt!(charge_scans(entries.len(), span, meter));
        self.push_pattern_entries(entries, values, variables, span, meter)
    }

    /// Build and push every stack entry, bottom first, stopping at the first
    /// refusal.
    #[expect(
        tigerstyle::borrowed_argument_types,
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; the build pushes terms onto its growable value stack, which a slice cannot express, and charges the non-const work meter while allocating arena terms."
    )]
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-maintainers; each call consumes one entry of a stack bounded by the admitted stack height and type size; recursion instead of a loop keeps the build's recursive cycle free of loops, which the pinned Aeneas cannot prove monotone."
    )]
    fn push_ty_entries(
        &mut self,
        entries: &[noble_kernel::types::Ty],
        values: &mut alloc::vec::Vec<u32>,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        match entries.split_first() {
            Some((ty, rest)) => {
                attempt!(self.build_ty(ty, values, span, meter));
                attempt!(self.join(super::Join::Push, values, span, meter));
                self.push_ty_entries(rest, values, span, meter)
            }
            None => Ok(()),
        }
    }

    /// Build and push every stack-pattern entry, bottom first, stopping at the
    /// first refusal.
    #[expect(
        tigerstyle::borrowed_argument_types,
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; the build pushes terms onto its growable value stack, which a slice cannot express, and charges the non-const work meter while allocating arena terms."
    )]
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-maintainers; each call consumes one entry of a stack bounded by the admitted stack height and type size; recursion instead of a loop keeps the build's recursive cycle free of loops, which the pinned Aeneas cannot prove monotone."
    )]
    fn push_pattern_entries(
        &mut self,
        entries: &[noble_kernel::shapes::Pattern],
        values: &mut alloc::vec::Vec<u32>,
        variables: &[super::super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        match entries.split_first() {
            Some((pattern, rest)) => {
                attempt!(self.build_pattern(pattern, values, variables, span, meter));
                attempt!(self.join(super::Join::Push, values, span, meter));
                self.push_pattern_entries(rest, values, variables, span, meter)
            }
            None => Ok(()),
        }
    }
}

/// Charge one scan step per stack entry, stopping at the first refusal.
fn charge_scans(
    count: usize,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let mut outcome = Ok(());
    let mut remaining = count;
    while outcome.is_ok() && remaining > 0 {
        outcome = meter.charge(1, span);
        remaining -= 1;
    }
    outcome
}
