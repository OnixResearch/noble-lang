//! Compare independently inferred open interfaces rather than source topology.
//! Holes are alpha-renamed, but equal holes must remain equal in both bodies.

fn infer(
    session: &super::Session,
    definition: &super::Named,
    environment: &noble_kernel::contracts::Env,
    limits: crate::Limits,
) -> Result<super::inference::State, crate::Diagnostic> {
    let mut meter = crate::Meter::new(limits);
    let scope = super::inference::Scope {
        root: &definition.tree,
        session,
        environment,
        root_signature: definition.signature.as_ref(),
    };
    super::inference::infer(scope, super::inference::Mode::Declaration, &[], &mut meter)
}

pub(super) fn compatible(
    session: &super::Session,
    previous: &super::Named,
    replacement: &super::Named,
    limits: crate::Limits,
) -> Result<bool, crate::Diagnostic> {
    let environment = session.environment()?;
    let old = infer(session, previous, &environment, limits)?;
    let new = infer(session, replacement, &environment, limits)?;
    let span = replacement.tree.span;
    let (Some(a), Some(b)) = (old.bodies.first(), new.bodies.first()) else {
        return Err(crate::internal(span));
    };
    let old_effects = old.arena.effect_value(a.effect, span)?;
    let new_effects = new.arena.effect_value(b.effect, span)?;
    // A replacement may drop an effect, never add one. An effectful old
    // definition therefore admits a pure new implementation of the same
    // ordered stack interface.
    if new_effects.as_slice().iter().any(|effect| !old_effects.as_slice().contains(effect)) {
        return Ok(false);
    }
    let mut comparison = Comparison {
        old: &old.arena,
        new: &new.arena,
        holes: alloc::vec::Vec::new(),
        pending: alloc::vec![(a.input, b.input), (a.output, b.output)],
        span,
        meter: crate::Meter::new(limits),
    };
    comparison.compare()
}

struct Comparison<'a> {
    old: &'a crate::inference::Arena,
    new: &'a crate::inference::Arena,
    holes: alloc::vec::Vec<(u32, u32)>,
    pending: alloc::vec::Vec<(u32, u32)>,
    span: crate::Span,
    meter: crate::Meter,
}

impl Comparison<'_> {
    fn compare(&mut self) -> Result<bool, crate::Diagnostic> {
        while let Some((left, right)) = self.pending.pop() {
            self.meter.node(self.span)?;
            let left = self.old.root(left, self.span, &mut self.meter)?;
            let right = self.new.root(right, self.span, &mut self.meter)?;
            let before = self.old.get(left, self.span)?;
            let after = self.new.get(right, self.span)?;
            use crate::inference::Term;
            match (before, after) {
                (Term::Hole(a), Term::Hole(b)) if a == b => {
                    if self.holes.iter().any(|(l, r)| *l == left || *r == right) {
                        if !self.holes.contains(&(left, right)) {
                            return Ok(false);
                        }
                    } else {
                        self.holes.push((left, right));
                    }
                }
                (Term::RigidValue(a), Term::RigidValue(b))
                | (Term::RigidStack(a), Term::RigidStack(b)) if a == b => {}
                (Term::Unit, Term::Unit)
                | (Term::Bool, Term::Bool)
                | (Term::I64, Term::I64)
                | (Term::Text, Term::Text)
                | (Term::Syntax, Term::Syntax)
                | (Term::Contract, Term::Contract)
                | (Term::Evidence, Term::Evidence)
                | (Term::Certified, Term::Certified)
                | (Term::Empty, Term::Empty) => {}
                (Term::Resource(a), Term::Resource(b)) if a == b => {}
                (Term::Nominal(a), Term::Nominal(b)) if a == b => {}
                (Term::GenericNominal(a, x, y), Term::GenericNominal(b, u, v)) if a == b => {
                    self.pending.push((x, u));
                    self.pending.push((y, v));
                }
                (Term::Pair(a, b), Term::Pair(c, d))
                | (Term::Sum(a, b), Term::Sum(c, d))
                | (Term::Push(a, b), Term::Push(c, d)) => {
                    self.pending.push((a, c));
                    self.pending.push((b, d));
                }
                (Term::List(a), Term::List(b)) => self.pending.push((a, b)),
                (Term::Program(a, b), Term::Program(c, d)) => {
                    let before = self.old.program_effect(left, self.span, &mut self.meter)?;
                    let after = self.new.program_effect(right, self.span, &mut self.meter)?;
                    if self.old.effect_value(before, self.span)?
                        != self.new.effect_value(after, self.span)?
                    {
                        return Ok(false);
                    }
                    self.pending.push((a, c));
                    self.pending.push((b, d));
                }
                _ => return Ok(false),
            }
        }
        Ok(true)
    }
}
