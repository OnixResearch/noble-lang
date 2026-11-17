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
        prospective_owner: None,
    };
    super::inference::infer(scope, super::inference::Mode::Declaration, &[], &mut meter)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
pub(super) fn compatible(
    session: &super::Session,
    previous: &super::Named,
    replacement: &super::Named,
    limits: crate::Limits,
) -> Result<Compatibility, crate::Diagnostic> {
    let environment = session.environment()?;
    // The environment is read through an explicit match on its copy-on-write
    // state; the pinned Aeneas cannot translate `Cow`'s Deref.
    let old = match &environment {
        alloc::borrow::Cow::Borrowed(borrowed) => infer(session, previous, borrowed, limits),
        alloc::borrow::Cow::Owned(owned) => infer(session, previous, owned, limits),
    }?;
    let new = match &environment {
        alloc::borrow::Cow::Borrowed(borrowed) => infer(session, replacement, borrowed, limits),
        alloc::borrow::Cow::Owned(owned) => infer(session, replacement, owned, limits),
    }?;
    let span = replacement.tree.span;
    let (Some(a), Some(b)) = (old.bodies.first(), new.bodies.first()) else {
        return Err(crate::internal(span));
    };
    let old_effects = old.arena.effect_value(a.effect, span)?;
    let new_effects = new.arena.effect_value(b.effect, span)?;
    // A replacement may drop an effect, never add one. An effectful old
    // definition therefore admits a pure new implementation of the same
    // ordered stack interface.
    if crate::scan::find_first(new_effects.as_slice(), |effect| {
        !old_effects.as_slice().contains(effect)
    })
    .is_some()
    {
        return Ok(Compatibility::EffectGrowth);
    }
    let mut comparison = Comparison {
        old: &old.arena,
        new: &new.arena,
        holes: alloc::vec::Vec::new(),
        pending: alloc::vec![(a.input, b.input), (a.output, b.output)],
        span,
        meter: crate::Meter::new(limits),
    };
    Ok(if comparison.compare()? {
        Compatibility::Compatible
    } else {
        Compatibility::InterfaceChanged
    })
}

/// Which replacement contract a completed comparison refused.
///
/// The caller keeps the diagnostic that names the rule; this class only says
/// which rule it was, so no caller infers a cause from a checking stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[octet::sealed_enum]
pub(super) enum Compatibility {
    Compatible,
    /// The ordered input/output stack interface differs.
    InterfaceChanged,
    /// The replacement admits an effect the installed definition does not.
    EffectGrowth,
}

/// The ordered, statically inferred interface of one definition.
///
/// The root frame of a checked definition is its body, so the interface is that
/// frame's own input and output stacks: both sides in source order (bottom
/// first), the finite effect identities its latent bound admits, and whether
/// each side still ends in an unconstrained stack variable. This is an
/// inference result; it is not a runtime observation, not evidence of
/// behavioural equivalence, and never acceptance.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
pub(super) fn ordered(
    session: &super::Session,
    definition: &super::Named,
    limits: crate::Limits,
) -> Result<super::OrderedInterface, crate::Diagnostic> {
    let span = definition.tree.span;
    let environment = session.environment()?;
    let state = match &environment {
        alloc::borrow::Cow::Borrowed(borrowed) => infer(session, definition, borrowed, limits),
        alloc::borrow::Cow::Owned(owned) => infer(session, definition, owned, limits),
    }?;
    let body = state.bodies.first().ok_or_else(|| crate::internal(span))?;
    let (stack_in, stack_in_tail, meter) =
        ordered_side(&state.arena, body.input, span, crate::Meter::new(limits))?;
    let (stack_out, stack_out_tail, mut meter) =
        ordered_side(&state.arena, body.output, span, meter)?;
    let (effects, unresolved_effects) = state.arena.editor_effect(body.effect, span, &mut meter)?;
    Ok(super::OrderedInterface {
        stack_in,
        stack_in_tail,
        stack_out,
        stack_out_tail,
        effects,
        unresolved_effects,
    })
}

/// One ordered stack side and whether it ends in a concrete stack or retains an
/// unconstrained tail variable.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the suffix walk has already refused every shape other than a finite Push chain, so malformed terms return diagnostics instead of assertions"
)]
fn ordered_side(
    arena: &crate::inference::Arena,
    stack: u32,
    span: crate::Span,
    meter: crate::Meter,
) -> Result<
    (
        alloc::vec::Vec<alloc::string::String>,
        super::InterfaceTail,
        crate::Meter,
    ),
    crate::Diagnostic,
> {
    let mut cursor = stack;
    let mut work_meter = meter;
    // The suffix walk refuses any shape other than a finite Push chain ending in
    // one of the three documented tails, so the tail walk below cannot diverge.
    let ordered = arena.editor_stack_suffix(cursor, span, &mut work_meter)?;
    let mut walked = Ok(None);
    while matches!(walked, Ok(None)) {
        let term = match arena.root(cursor, span, &mut work_meter) {
            Ok(id) => arena.get(id, span),
            Err(problem) => Err(problem),
        };
        walked = match term {
            Ok(crate::inference::Term::Push(rest, _)) => {
                cursor = rest;
                Ok(None)
            }
            Ok(crate::inference::Term::Empty) => Ok(Some(super::InterfaceTail::Closed)),
            Ok(
                crate::inference::Term::Hole(crate::inference::Sort::Stack)
                | crate::inference::Term::RigidStack(_),
            ) => Ok(Some(super::InterfaceTail::Unconstrained)),
            Ok(_) => Err(crate::internal(span)),
            Err(problem) => Err(problem),
        };
    }
    match walked {
        Ok(Some(tail)) => Ok((ordered, tail, work_meter)),
        Ok(None) => Err(crate::internal(span)),
        Err(problem) => Err(problem),
    }
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
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; the parameter is the owned builder state this stage accumulates into, not a borrowed input"
    )]
    fn compare(&mut self) -> Result<bool, crate::Diagnostic> {
        let mut outcome = Ok(true);
        while let Some(pair) = self.pending.pop() {
            outcome = self.step(pair);
            if !matches!(outcome, Ok(true)) {
                break;
            }
        }
        outcome
    }

    /// Compare one pending pair of terms, queueing their children.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; the parameter is the owned builder state this stage accumulates into, not a borrowed input"
    )]
    fn step(&mut self, pair: (u32, u32)) -> Result<bool, crate::Diagnostic> {
        let (left, right) = pair;
        self.meter.node(self.span)?;
        let left = self.old.root(left, self.span, &mut self.meter)?;
        let right = self.new.root(right, self.span, &mut self.meter)?;
        let before = self.old.get(left, self.span)?;
        let after = self.new.get(right, self.span)?;
        #[expect(
            tigerstyle::non_trait_imports,
            reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
        )]
        use crate::inference::Term;
        match (before, after) {
            (Term::Hole(a), Term::Hole(b)) if a == b => {
                if crate::scan::find_first(&self.holes, |(l, r)| *l == left || *r == right)
                    .is_some()
                {
                    if !self.holes.contains(&(left, right)) {
                        return Ok(false);
                    }
                } else {
                    self.holes.push((left, right));
                }
            }
            (Term::RigidValue(a), Term::RigidValue(b))
            | (Term::RigidStack(a), Term::RigidStack(b))
                if a == b => {}
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
        Ok(true)
    }
}
