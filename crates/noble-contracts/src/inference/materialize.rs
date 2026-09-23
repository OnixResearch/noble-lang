mod rendering;

#[octet::sealed_enum]
enum Step {
    Visit(u32),
    Finish(super::Term, noble_kernel::types::EffSet),
}

#[octet::sealed_enum]
pub(super) enum Material {
    Value(noble_kernel::types::Ty, u32),
    Stack(alloc::vec::Vec<noble_kernel::types::Ty>, u32),
}

pub(super) struct State {
    steps: alloc::vec::Vec<Step>,
    pub(super) values: alloc::vec::Vec<Material>,
}

impl State {
    pub(super) fn pop(&mut self, span: crate::Span) -> Result<Material, crate::Diagnostic> {
        match self.values.pop() {
            Some(value) => Ok(value),
            None => Err(crate::internal(span)),
        }
    }
}

impl super::Arena {
    pub fn stack_value(
        &self,
        id: u32,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<alloc::vec::Vec<noble_kernel::types::Ty>, crate::Diagnostic> {
        match attempt!(self.materialize(id, span, meter)) {
            Material::Stack(stack, _) => Ok(stack),
            Material::Value(_, _) => Err(crate::internal(span)),
        }
    }

    pub fn instantiation(
        &self,
        variables: &[super::Variable],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<noble_kernel::words::Inst, crate::Diagnostic> {
        let mut bindings = alloc::vec::Vec::with_capacity(variables.len());
        let mut at = 0usize;
        let mut failure = None;
        while at < variables.len() {
            match self.binding(variables.get(at), span, meter) {
                Ok(binding) => {
                    bindings.push(binding);
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
            None => Ok(noble_kernel::words::Inst { bindings }),
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; witness materialization allocates structural Ty values and returns owned diagnostics."
    )]
    fn binding(
        &self,
        variable: Option<&super::Variable>,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<noble_kernel::words::Binding, crate::Diagnostic> {
        attempt!(meter.charge(1, span));
        match variable {
            Some(super::Variable::Effect) => Ok(noble_kernel::words::Binding::Effect(
                noble_kernel::types::EffSet::empty(),
            )),
            Some(super::Variable::EffectValue(id)) => Ok(noble_kernel::words::Binding::Effect(
                attempt!(self.effect_value(*id, span)),
            )),
            Some(super::Variable::Value(id)) => {
                match attempt!(self.materialize(*id, span, meter)) {
                    Material::Value(ty, _) => Ok(noble_kernel::words::Binding::Value(ty)),
                    Material::Stack(_, _) => Err(crate::internal(span)),
                }
            }
            Some(super::Variable::Stack(id)) => {
                match attempt!(self.materialize(*id, span, meter)) {
                    Material::Stack(stack, _) => Ok(noble_kernel::words::Binding::Stack(stack)),
                    Material::Value(_, _) => Err(crate::internal(span)),
                }
            }
            None => Err(crate::internal(span)),
        }
    }

    pub(super) fn materialize(
        &self,
        root: u32,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<Material, crate::Diagnostic> {
        let mut initial = State {
            steps: alloc::vec::Vec::with_capacity(1),
            values: alloc::vec::Vec::new(),
        };
        initial.steps.push(Step::Visit(root));
        let mut outcome = Ok(initial);
        while let Ok(mut state) = outcome {
            match state.steps.pop() {
                Some(step) => outcome = self.read_step(step, state, span, meter),
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
        state.pop(span)
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; materialization steps mutate allocating worklists and propagate non-const metering errors."
    )]
    fn read_step(
        &self,
        step: Step,
        state: State,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<State, crate::Diagnostic> {
        attempt!(meter.charge(1, span));
        match step {
            Step::Visit(id) => self.visit(id, state, span, meter),
            Step::Finish(term, effects) => state.finish(term, effects, span),
        }
    }

    fn visit(
        &self,
        id: u32,
        mut state: State,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<State, crate::Diagnostic> {
        let id = attempt!(self.root(id, span, meter));
        let term = attempt!(self.get(id, span));
        let material = match term {
            super::Term::Hole(_) => {
                return Err(crate::invalid(
                    span,
                    "ambiguous witness; add an explicit typed block or word binding",
                ));
            }
            super::Term::Link(_) => return Err(crate::internal(span)),
            super::Term::Unit => Material::Value(noble_kernel::types::Ty::Unit, 1),
            super::Term::Bool => Material::Value(noble_kernel::types::Ty::Bool, 1),
            super::Term::I64 => Material::Value(noble_kernel::types::Ty::I64, 1),
            super::Term::Text => Material::Value(noble_kernel::types::Ty::Text, 1),
            super::Term::Syntax => Material::Value(noble_kernel::types::Ty::Syntax, 1),
            super::Term::Contract => Material::Value(noble_kernel::types::Ty::Contract, 1),
            super::Term::Evidence => Material::Value(noble_kernel::types::Ty::Evidence, 1),
            super::Term::Certified => Material::Value(noble_kernel::types::Ty::Certified, 1),
            super::Term::Empty => Material::Stack(alloc::vec::Vec::new(), 0),
            super::Term::Pair(a, b)
            | super::Term::Sum(a, b)
            | super::Term::Program(a, b)
            | super::Term::Push(a, b) => {
                let effects = if self.effectful && matches!(term, super::Term::Program(_, _)) {
                    let effect = attempt!(self.program_effect(id, span, meter));
                    attempt!(self.effect_value(effect, span))
                } else {
                    noble_kernel::types::EffSet::empty()
                };
                state.steps.push(Step::Finish(term, effects));
                state.steps.push(Step::Visit(b));
                state.steps.push(Step::Visit(a));
                return Ok(state);
            }
            super::Term::List(item) => {
                state
                    .steps
                    .push(Step::Finish(term, noble_kernel::types::EffSet::empty()));
                state.steps.push(Step::Visit(item));
                return Ok(state);
            }
        };
        state.values.push(material);
        Ok(state)
    }

    /// Diagnostic rendering has a separate fixed output bound and consumes the
    /// existing work budget. A rendering refusal never replaces the type error.
    #[expect(
        tigerstyle::ambiguous_params,
        reason = "Owner: noble-maintainers; expected and actual are ordered IDs in the same term arena, supplied directly from the rejected stack equation only to render its two sides."
    )]
    pub fn join_message(
        &self,
        expected: u32,
        actual: u32,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<alloc::string::String, crate::Diagnostic> {
        let expected = attempt!(rendering::describe(self, expected, span, meter));
        let actual = attempt!(rendering::describe(self, actual, span, meter));
        let mut message = alloc::string::String::from("stack/program join: expected ");
        message.push_str(&expected);
        message.push_str("; actual ");
        message.push_str(&actual);
        Ok(message)
    }
}
