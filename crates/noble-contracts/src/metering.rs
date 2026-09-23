#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; Meter contains only limits and request-local work/node counters; these helpers perform checked work subtraction and limit-guarded node updates without access to source or published compiler state."
)]

impl crate::Meter {
    pub const fn new(limits: crate::Limits) -> Self {
        Self {
            limits,
            work: limits.work,
            nodes: 0,
        }
    }

    pub fn charge(&mut self, amount: u32, span: crate::Span) -> Result<(), crate::Diagnostic> {
        match self.work.checked_sub(amount) {
            Some(remaining) => {
                self.work = remaining;
                Ok(())
            }
            None => Err(crate::Diagnostic::new(
                crate::DiagnosticKind::Exhausted,
                span,
                "frontend work limit exceeded",
            )),
        }
    }

    pub fn node(&mut self, span: crate::Span) -> Result<(), crate::Diagnostic> {
        attempt!(self.charge(1, span));
        if self.nodes >= self.limits.nodes {
            return Err(crate::Diagnostic::new(
                crate::DiagnosticKind::Exhausted,
                span,
                "frontend node limit exceeded",
            ));
        }
        self.nodes += 1;
        Ok(())
    }

    pub fn depth(&mut self, depth: u32, span: crate::Span) -> Result<(), crate::Diagnostic> {
        attempt!(self.charge(1, span));
        if depth > self.limits.depth {
            return Err(crate::Diagnostic::new(
                crate::DiagnosticKind::Exhausted,
                span,
                "frontend depth limit exceeded",
            ));
        }
        Ok(())
    }
}
