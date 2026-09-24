impl super::materialize::State {
    pub(super) fn finish(
        mut self,
        term: super::Term,
        effects: noble_kernel::types::EffSet,
        span: crate::Span,
    ) -> Result<Self, crate::Diagnostic> {
        let b = attempt!(self.pop(span));
        let material = match term {
            super::Term::List(_) => list(b, span),
            super::Term::Pair(_, _) | super::Term::Sum(_, _) => {
                let a = attempt!(self.pop(span));
                pair(matches!(term, super::Term::Pair(_, _)), a, b, span)
            }
            super::Term::Program(_, _) => {
                let a = attempt!(self.pop(span));
                program(a, b, effects, span)
            }
            super::Term::Push(_, _) => {
                let a = attempt!(self.pop(span));
                push(a, b, span)
            }
            super::Term::Hole(_)
            | super::Term::Link(_)
            | super::Term::Unit
            | super::Term::Bool
            | super::Term::I64
            | super::Term::Text
            | super::Term::Syntax
            | super::Term::Contract
            | super::Term::Evidence
            | super::Term::Certified
            | super::Term::Resource(_)
            | super::Term::Empty => Err(crate::internal(span)),
        };
        self.values.push(attempt!(material));
        Ok(self)
    }
}

fn list(
    item: super::materialize::Material,
    span: crate::Span,
) -> Result<super::materialize::Material, crate::Diagnostic> {
    match item {
        super::materialize::Material::Value(ty, size) => Ok(super::materialize::Material::Value(
            noble_kernel::types::Ty::List(alloc::boxed::Box::new(ty)),
            attempt!(type_size(size.saturating_add(1), span)),
        )),
        super::materialize::Material::Stack(_, _) => Err(crate::internal(span)),
    }
}

fn pair(
    is_pair: bool,
    a: super::materialize::Material,
    b: super::materialize::Material,
    span: crate::Span,
) -> Result<super::materialize::Material, crate::Diagnostic> {
    match (a, b) {
        (
            super::materialize::Material::Value(a, sa),
            super::materialize::Material::Value(b, sb),
        ) => {
            let constructors = attempt!(type_size(sa.saturating_add(sb).saturating_add(1), span));
            let ty = if is_pair {
                noble_kernel::types::Ty::Pair(alloc::boxed::Box::new(a), alloc::boxed::Box::new(b))
            } else {
                noble_kernel::types::Ty::Sum(alloc::boxed::Box::new(a), alloc::boxed::Box::new(b))
            };
            Ok(super::materialize::Material::Value(ty, constructors))
        }
        (super::materialize::Material::Stack(_, _), _)
        | (super::materialize::Material::Value(_, _), super::materialize::Material::Stack(_, _)) => {
            Err(crate::internal(span))
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; Ty::program owns its allocated input/output stacks and constructor-limit errors allocate diagnostics."
)]
fn program(
    inputs: super::materialize::Material,
    outputs: super::materialize::Material,
    effects: noble_kernel::types::EffSet,
    span: crate::Span,
) -> Result<super::materialize::Material, crate::Diagnostic> {
    match (inputs, outputs) {
        (
            super::materialize::Material::Stack(a, sa),
            super::materialize::Material::Stack(b, sb),
        ) => Ok(super::materialize::Material::Value(
            noble_kernel::types::Ty::program(a, b, effects),
            attempt!(type_size(sa.saturating_add(sb).saturating_add(1), span)),
        )),
        (super::materialize::Material::Value(_, _), _)
        | (super::materialize::Material::Stack(_, _), super::materialize::Material::Value(_, _)) => {
            Err(crate::internal(span))
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; pushing a materialized value grows a Vec and stack-cap errors construct diagnostics."
)]
fn push(
    stack: super::materialize::Material,
    value: super::materialize::Material,
    span: crate::Span,
) -> Result<super::materialize::Material, crate::Diagnostic> {
    match (stack, value) {
        (
            super::materialize::Material::Stack(mut stack, sa),
            super::materialize::Material::Value(ty, sb),
        ) => {
            if stack.len() >= attempt!(crate::offset(super::STACK_CAP, span)) {
                return Err(crate::Diagnostic::new(
                    crate::DiagnosticKind::Exhausted,
                    span,
                    "stack height limit (256 values) exceeded",
                ));
            }
            stack.push(ty);
            Ok(super::materialize::Material::Stack(
                stack,
                sa.saturating_add(sb),
            ))
        }
        (super::materialize::Material::Value(_, _), _)
        | (super::materialize::Material::Stack(_, _), super::materialize::Material::Stack(_, _)) => {
            Err(crate::internal(span))
        }
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; constructor-limit failure constructs an owned diagnostic String."
)]
fn type_size(constructors: u32, span: crate::Span) -> Result<u32, crate::Diagnostic> {
    if constructors > crate::syntax::TYPE_CAP {
        Err(crate::Diagnostic::new(
            crate::DiagnosticKind::Exhausted,
            span,
            "inferred type exceeds the 256-constructor limit",
        ))
    } else {
        Ok(constructors)
    }
}
