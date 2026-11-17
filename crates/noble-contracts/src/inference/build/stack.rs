impl super::super::Arena {
    /// Charge one step, push the empty base, charge one scan per entry from
    /// the top, then build and push every entry bottom first.
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
        let mut failure = None;
        let mut at = 0;
        while at < stack.len() {
            if let Err(problem) = self.push_ty_entry(&stack[at], values, span, meter) {
                failure = Some(problem);
                break;
            }
            at += 1;
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }

    /// Charge one step, push the stack-variable or empty base, then build the
    /// remaining entries as `build_stack_ty` does.
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
        let mut failure = None;
        let mut at = 0;
        while at < entries.len() {
            if let Err(problem) =
                self.push_pattern_entry(&entries[at], values, variables, span, meter)
            {
                failure = Some(problem);
                break;
            }
            at += 1;
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }

    /// Build one stack entry and push it onto the stack below it.
    fn push_ty_entry(
        &mut self,
        ty: &noble_kernel::types::Ty,
        values: &mut alloc::vec::Vec<u32>,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        attempt!(self.build_ty(ty, values, span, meter));
        self.join(super::Join::Push, values, span, meter)
    }

    /// Build one stack-pattern entry and push it onto the stack below it.
    fn push_pattern_entry(
        &mut self,
        pattern: &noble_kernel::shapes::Pattern,
        values: &mut alloc::vec::Vec<u32>,
        variables: &[super::super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        attempt!(self.build_pattern(pattern, values, variables, span, meter));
        self.join(super::Join::Push, values, span, meter)
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
