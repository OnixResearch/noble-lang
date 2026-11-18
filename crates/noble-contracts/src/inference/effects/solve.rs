#[derive(Clone, Copy, PartialEq, Eq)]
struct Symbolic {
    concrete: u64,
    rigid: u64,
    unknown: bool,
}

impl Symbolic {
    const fn empty() -> Self {
        Self {
            concrete: 0,
            rigid: 0,
            unknown: false,
        }
    }

    const fn union(self, other: Self) -> Self {
        Self {
            concrete: self.concrete | other.concrete,
            rigid: self.rigid | other.rigid,
            unknown: self.unknown || other.unknown,
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
/// A declaration-level effect variable is an arbitrary set, not a hole whose
/// bound may be narrowed. Compare the body's union formula with its declared
/// formula before the concrete finite-bit solver runs.
pub(super) fn rigid(
    arena: &crate::inference::Arena,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    if arena.rigid_effect_goals.is_empty() {
        return Ok(());
    }
    let mut facts = alloc::vec::Vec::with_capacity(arena.effects.len());
    let mut failure = None;
    let mut at = 0;
    while at < arena.effects.len() {
        match initial_fact(&arena.effects[at], span) {
            Ok(fact) => facts.push(fact),
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
        at += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    let mut is_changed = true;
    // Concrete and rigid bits only accumulate; an unconstrained hole can
    // only become constrained. The work meter bounds this finite fixed point
    // even when several independent equations constrain the same hole.
    while is_changed {
        match rigid_pass(arena, &mut facts, span, meter) {
            Ok(has_changed) => is_changed = has_changed,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    at = 0;
    while at < arena.effect_equations.len() {
        let (a, b) = arena.effect_equations[at];
        if let Err(problem) = rigid_equation_witness(&facts, a, b, span) {
            failure = Some(problem);
            break;
        }
        at += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    at = 0;
    while at < arena.rigid_effect_goals.len() {
        let (body, declared) = arena.rigid_effect_goals[at];
        if let Err(problem) = rigid_goal(&facts, body, declared, span) {
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

/// The symbolic summary an effect starts from before propagation.
fn initial_fact(effect: &super::Effect, span: crate::Span) -> Result<Symbolic, crate::Diagnostic> {
    Ok(match effect {
        super::Effect::Constant(bits) => Symbolic {
            concrete: *bits,
            ..Symbolic::empty()
        },
        super::Effect::Rigid(index) => Symbolic {
            rigid: attempt!(1u64
                .checked_shl(*index)
                .ok_or_else(|| { crate::invalid(span, "rigid effect binder limit exceeded") })),
            ..Symbolic::empty()
        },
        super::Effect::Hole => Symbolic {
            unknown: true,
            ..Symbolic::empty()
        },
        super::Effect::Union(_, _) => Symbolic::empty(),
    })
}

/// One charged propagation pass over unions, then equations; whether any fact changed.
fn rigid_pass(
    arena: &crate::inference::Arena,
    facts: &mut [Symbolic],
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    attempt!(meter.charge(1, span));
    let has_union_changed = attempt!(rigid_unions(&arena.effects, facts, span, meter));
    let has_equation_changed = attempt!(rigid_equations(arena, facts, span, meter));
    Ok(has_union_changed || has_equation_changed)
}

/// Recompute every union fact in order, stopping at the first refusal.
fn rigid_unions(
    effects: &[super::Effect],
    facts: &mut [Symbolic],
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    let mut is_changed = false;
    let mut outcome = Ok(());
    let mut at = 0;
    while at < effects.len() {
        match rigid_union(&effects[at], at, facts, span, meter) {
            Ok(has_changed) => is_changed |= has_changed,
            Err(problem) => {
                outcome = Err(problem);
                break;
            }
        }
        at += 1;
    }
    attempt!(outcome);
    Ok(is_changed)
}

/// Charge one effect and refresh its fact when it is a union.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; metering and symbolic-fact lookup are non-const and refusals construct owned diagnostics."
)]
fn rigid_union(
    effect: &super::Effect,
    at: usize,
    facts: &mut [Symbolic],
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    attempt!(meter.charge(1, span));
    if let super::Effect::Union(a, b) = effect {
        let left = attempt!(symbol(facts, *a, span));
        let right = attempt!(symbol(facts, *b, span));
        let next = left.union(right);
        if facts[at] != next {
            facts[at] = next;
            return Ok(true);
        }
    }
    Ok(false)
}

/// Propagate every effect equation in order, stopping at the first refusal.
fn rigid_equations(
    arena: &crate::inference::Arena,
    facts: &mut [Symbolic],
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    let mut is_changed = false;
    let mut outcome = Ok(());
    let mut at = 0;
    while at < arena.effect_equations.len() {
        let (a, b) = arena.effect_equations[at];
        match rigid_equation(&arena.effects, facts, a, b, span, meter) {
            Ok(has_changed) => is_changed |= has_changed,
            Err(problem) => {
                outcome = Err(problem);
                break;
            }
        }
        at += 1;
    }
    attempt!(outcome);
    Ok(is_changed)
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameter list is the internal call shape; bundling it would change the extracted subject"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
/// Charge one equation and push its combined summary through both sides.
fn rigid_equation(
    effects: &[super::Effect],
    facts: &mut [Symbolic],
    a: u32,
    b: u32,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    attempt!(meter.charge(1, span));
    let left = attempt!(symbol(facts, a, span));
    let right = attempt!(symbol(facts, b, span));
    let combined = Symbolic {
        concrete: left.concrete | right.concrete,
        rigid: left.rigid | right.rigid,
        unknown: left.unknown && right.unknown,
    };
    let mut is_changed = attempt!(equate_through_pure(
        effects, facts, a, combined, span, meter
    ));
    is_changed |= attempt!(equate_through_pure(
        effects, facts, b, combined, span, meter
    ));
    Ok(is_changed)
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
/// Refuse an equation whose two sides do not share one symbolic witness.
fn rigid_equation_witness(
    facts: &[Symbolic],
    a: u32,
    b: u32,
    span: crate::Span,
) -> Result<(), crate::Diagnostic> {
    let left = attempt!(symbol(facts, a, span));
    let right = attempt!(symbol(facts, b, span));
    // Assign every still-open hole the union of its accumulated concrete
    // effects and rigid binders. Equal summaries give one simultaneous
    // symbolic witness for all equations under every binder instantiation.
    // An unequal open equation may have another existential solution, but
    // without a verified witness it cannot authorize a signed body.
    if left.concrete != right.concrete || left.rigid != right.rigid {
        let reason = if left.unknown || right.unknown {
            "unresolved universal effect equation has no verified witness"
        } else {
            "signature narrows a universal effect"
        };
        return Err(crate::invalid(span, reason));
    }
    Ok(())
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; symbolic-fact lookup and refusal diagnostics are non-const."
)]
/// Refuse a body whose effect summary differs from its declared universal bound.
fn rigid_goal(
    facts: &[Symbolic],
    body: u32,
    declared: u32,
    span: crate::Span,
) -> Result<(), crate::Diagnostic> {
    let actual = attempt!(symbol(facts, body, span));
    let expected = attempt!(symbol(facts, declared, span));
    if actual.unknown || expected.unknown || actual != expected {
        return Err(crate::invalid(
            span,
            "source body does not preserve its universal effect bound",
        ));
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameter list is the internal call shape; bundling it would change the extracted subject"
)]
/// An equality with `pure ∪ x` constrains `x` exactly. Propagate only
/// through the neutral side; an arbitrary union cannot attribute effects to
/// either operand and remains unknown until another equation constrains it.
fn equate_through_pure(
    effects: &[super::Effect],
    facts: &mut [Symbolic],
    mut id: u32,
    value: Symbolic,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    let mut remaining = effects.len();
    while remaining != 0 {
        attempt!(meter.charge(1, span));
        let index = attempt!(crate::offset(id, span));
        match effects.get(index) {
            Some(super::Effect::Hole) => {
                let current = facts[index];
                let joined = Symbolic {
                    concrete: current.concrete | value.concrete,
                    rigid: current.rigid | value.rigid,
                    unknown: current.unknown && value.unknown,
                };
                if current != joined {
                    facts[index] = joined;
                    return Ok(true);
                }
                return Ok(false);
            }
            Some(super::Effect::Union(left, right)) => {
                let left_value = attempt!(symbol(facts, *left, span));
                let right_value = attempt!(symbol(facts, *right, span));
                if left_value == Symbolic::empty() {
                    id = *right;
                } else if right_value == Symbolic::empty() {
                    id = *left;
                } else {
                    return Ok(false);
                }
            }
            Some(super::Effect::Constant(_) | super::Effect::Rigid(_)) => return Ok(false),
            None => return Err(crate::internal(span)),
        }
        remaining -= 1;
    }
    Err(crate::invalid(span, "cyclic effect equation"))
}

fn symbol(facts: &[Symbolic], id: u32, span: crate::Span) -> Result<Symbolic, crate::Diagnostic> {
    facts
        .get(attempt!(crate::offset(id, span)))
        .copied()
        .ok_or_else(|| crate::internal(span))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each fixed-point pass and each constraint consumes work, bounds only lose finite effect bits, and concrete effects are checked after convergence with the first diagnostic retained."
)]
pub(super) fn run(
    arena: &mut crate::inference::Arena,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let mut has_changed = true;
    let mut failure = None;
    while has_changed {
        let result = match meter.charge(1, span) {
            Ok(()) => pass(arena, span, meter),
            Err(problem) => Err(problem),
        };
        match result {
            Ok(is_changed) => has_changed = is_changed,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    let mut at = 0;
    let mut failure = None;
    while at < arena.effects.len() {
        if let Err(problem) = constant(arena, at, span, meter) {
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

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; one pass scans union constraints then equality constraints in their original order, charging every step and validating all referenced IDs before narrowing finite bounds."
)]
fn pass(
    arena: &mut crate::inference::Arena,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    let mut has_changed = false;
    let mut at = 0;
    let mut failure = None;
    while at < arena.effects.len() {
        match union(arena, at, span, meter) {
            Ok(is_changed) => has_changed |= is_changed,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
        at += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    let mut at = 0;
    let mut failure = None;
    while at < arena.effect_equations.len() {
        match equation(arena, at, span, meter) {
            Ok(is_changed) => has_changed |= is_changed,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
        at += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(has_changed),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; union narrowing charges work and checks effect IDs before updating three bounds, returning owned diagnostics for invalid arena state."
)]
fn union(
    arena: &mut crate::inference::Arena,
    at: usize,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    attempt!(meter.charge(1, span));
    let id = attempt!(crate::index(at, span));
    let effect = match arena.effects.get(at) {
        Some(effect) => *effect,
        None => return Err(crate::internal(span)),
    };
    let mut has_changed = false;
    if let super::Effect::Union(a, b) = effect {
        let upper = attempt!(arena.effect_bound(a, span)) | attempt!(arena.effect_bound(b, span));
        has_changed |= attempt!(arena.narrow_effect(id, upper, span));
        let upper = attempt!(arena.effect_bound(id, span));
        has_changed |= attempt!(arena.narrow_effect(a, upper, span));
        has_changed |= attempt!(arena.narrow_effect(b, upper, span));
    }
    Ok(has_changed)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; equality narrowing charges work and validates both effect IDs before intersecting their bounds, with allocating diagnostics on malformed state."
)]
fn equation(
    arena: &mut crate::inference::Arena,
    at: usize,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    attempt!(meter.charge(1, span));
    let (a, b) = match arena.effect_equations.get(at) {
        Some(pair) => *pair,
        None => return Err(crate::internal(span)),
    };
    let upper = attempt!(arena.effect_bound(a, span)) & attempt!(arena.effect_bound(b, span));
    let mut has_changed = attempt!(arena.narrow_effect(a, upper, span));
    has_changed |= attempt!(arena.narrow_effect(b, upper, span));
    Ok(has_changed)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; post-solve concrete-effect validation charges work and returns an allocating latent-effect mismatch diagnostic when a concrete bound was narrowed."
)]
fn constant(
    arena: &crate::inference::Arena,
    at: usize,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    attempt!(meter.charge(1, span));
    if let Some(super::Effect::Constant(bits)) = arena.effects.get(at) {
        let id = attempt!(crate::index(at, span));
        let bound = attempt!(arena.effect_bound(id, span));
        if *bits != bound {
            return Err(crate::invalid(
                span,
                "program join has incompatible latent effect bounds",
            ));
        }
    }
    Ok(())
}

pub(super) fn value(
    arena: &crate::inference::Arena,
    id: u32,
    span: crate::Span,
) -> Result<noble_kernel::types::EffSet, crate::Diagnostic> {
    let bits = attempt!(arena.effect_bound(id, span));
    let effect_count = attempt!(crate::offset(bits.count_ones(), span));
    let mut ids = alloc::vec::Vec::with_capacity(effect_count);
    let mut remaining = bits;
    let mut at = 0u32;
    while remaining != 0 {
        if remaining & 1 != 0 {
            ids.push(noble_kernel::types::EffId(at));
        }
        remaining >>= 1;
        at = at.saturating_add(1);
    }
    Ok(noble_kernel::types::EffSet::from_ids(&ids))
}

pub(super) fn close(
    arena: &mut crate::inference::Arena,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let mut at = 0usize;
    let mut failure = None;
    while at < arena.terms.len() {
        if let Err(problem) = close_term(arena.terms.get_mut(at), span, meter) {
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

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; each term is charged before closing an inference hole, and an invalid table position returns an owned diagnostic."
)]
fn close_term(
    term: Option<&mut crate::inference::Term>,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    attempt!(meter.charge(1, span));
    let term = match term {
        Some(term) => term,
        None => return Err(crate::internal(span)),
    };
    match term {
        crate::inference::Term::Hole(crate::inference::Sort::Value) => {
            *term = crate::inference::Term::Unit;
        }
        crate::inference::Term::Hole(crate::inference::Sort::Stack) => {
            *term = crate::inference::Term::Empty;
        }
        _ => {}
    }
    Ok(())
}

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
// Test harness only: scenario-named tests over checked fixtures.
#[allow(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; test harness: descriptive scenario names, fixture imports and long setup legs are the readable form."
)]
#[cfg(test)]
mod tests {
    use super::super::Effect;

    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
    )]
    #[test]
    fn independent_pure_constraints_on_shared_branch_effect_converge_without_losing_host_bits() {
        // A generic match can equate the same inferred branch effect with a
        // pure signature and with the union of that branch with itself. The
        // latter initially reads the prior pass's unknown symbolic fact.
        let mut arena = crate::inference::Arena::new();
        arena.effects = alloc::vec![
            Effect::Hole,
            Effect::Hole,
            Effect::Constant(0),
            Effect::Union(1, 1),
        ];
        arena.effect_equations = alloc::vec![(2, 1), (0, 3)];
        arena.rigid_effect_goals = alloc::vec![(0, 2), (3, 2)];
        let span = crate::Span { start: 0, end: 0 };
        let mut meter = crate::Meter::new(crate::Limits::default());
        super::rigid(&arena, span, &mut meter).expect("independent pure constraints converge");

        arena.effects.push(Effect::Constant(1));
        arena.effect_equations.push((1, 4));
        let mut meter = crate::Meter::new(crate::Limits::default());
        assert!(
            super::rigid(&arena, span, &mut meter).is_err(),
            "a concrete host effect cannot satisfy the pure signature"
        );
    }

    const SPAN: crate::Span = crate::Span { start: 0, end: 0 };

    #[test]
    fn independent_universals_cannot_hide_in_an_open_effect_equation(
    ) -> Result<(), crate::Diagnostic> {
        let mut meter = crate::Meter::new(crate::Limits::default());
        let mut arena = crate::inference::Arena::source(1, false);
        let a = arena.rigid_effect(0, SPAN, &mut meter)?;
        let b = arena.rigid_effect(1, SPAN, &mut meter)?;
        let hole = arena.effect_hole(SPAN, &mut meter)?;
        let joined = arena.effect_union(b, hole, SPAN, &mut meter)?;
        arena.equate_effects(a, joined, SPAN, &mut meter)?;
        let pure = arena.effect_empty(SPAN, &mut meter)?;
        arena.require_rigid_effect(pure, pure, SPAN, &mut meter)?;
        assert!(matches!(
            super::rigid(&arena, SPAN, &mut meter),
            Err(error) if error.kind == crate::DiagnosticKind::Invalid
        ));
        Ok(())
    }

    #[test]
    fn mandatory_host_effect_cannot_hide_in_an_open_effect_equation(
    ) -> Result<(), crate::Diagnostic> {
        let mut meter = crate::Meter::new(crate::Limits::default());
        let mut arena = crate::inference::Arena::source(1, false);
        let pure = arena.effect_empty(SPAN, &mut meter)?;
        let emit = arena.effect_constant(
            &noble_kernel::types::EffSet::from_ids(&[noble_kernel::contracts::TEST_EMIT]),
            SPAN,
            &mut meter,
        )?;
        let hole = arena.effect_hole(SPAN, &mut meter)?;
        let joined = arena.effect_union(emit, hole, SPAN, &mut meter)?;
        arena.equate_effects(joined, pure, SPAN, &mut meter)?;
        arena.require_rigid_effect(pure, pure, SPAN, &mut meter)?;
        assert!(matches!(
            super::rigid(&arena, SPAN, &mut meter),
            Err(error) if error.kind == crate::DiagnosticKind::Invalid
        ));
        Ok(())
    }

    #[test]
    fn a_shared_hole_cannot_bypass_two_universal_constraints() -> Result<(), crate::Diagnostic> {
        let mut meter = crate::Meter::new(crate::Limits::default());
        let mut arena = crate::inference::Arena::source(1, false);
        let a = arena.rigid_effect(0, SPAN, &mut meter)?;
        let b = arena.rigid_effect(1, SPAN, &mut meter)?;
        let hole = arena.effect_hole(SPAN, &mut meter)?;
        let a_joined = arena.effect_union(a, hole, SPAN, &mut meter)?;
        let b_joined = arena.effect_union(b, hole, SPAN, &mut meter)?;
        arena.equate_effects(a, a_joined, SPAN, &mut meter)?;
        arena.equate_effects(hole, b_joined, SPAN, &mut meter)?;
        let pure = arena.effect_empty(SPAN, &mut meter)?;
        arena.require_rigid_effect(pure, pure, SPAN, &mut meter)?;
        assert!(matches!(
            super::rigid(&arena, SPAN, &mut meter),
            Err(error) if error.kind == crate::DiagnosticKind::Invalid
        ));
        Ok(())
    }

    #[test]
    fn distributed_holes_cannot_claim_an_independent_universal() -> Result<(), crate::Diagnostic> {
        let mut meter = crate::Meter::new(crate::Limits::default());
        let mut arena = crate::inference::Arena::source(1, false);
        let a = arena.rigid_effect(0, SPAN, &mut meter)?;
        let b = arena.rigid_effect(1, SPAN, &mut meter)?;
        let c = arena.rigid_effect(2, SPAN, &mut meter)?;
        let first = arena.effect_hole(SPAN, &mut meter)?;
        let second = arena.effect_hole(SPAN, &mut meter)?;
        let a_joined = arena.effect_union(a, first, SPAN, &mut meter)?;
        let b_joined = arena.effect_union(b, second, SPAN, &mut meter)?;
        let holes_joined = arena.effect_union(first, second, SPAN, &mut meter)?;
        arena.equate_effects(a, a_joined, SPAN, &mut meter)?;
        arena.equate_effects(b, b_joined, SPAN, &mut meter)?;
        arena.equate_effects(c, holes_joined, SPAN, &mut meter)?;
        let pure = arena.effect_empty(SPAN, &mut meter)?;
        arena.require_rigid_effect(pure, pure, SPAN, &mut meter)?;
        assert!(matches!(
            super::rigid(&arena, SPAN, &mut meter),
            Err(error) if error.kind == crate::DiagnosticKind::Invalid
        ));
        Ok(())
    }

    #[test]
    fn same_universal_may_join_a_neutral_hole() -> Result<(), crate::Diagnostic> {
        let mut meter = crate::Meter::new(crate::Limits::default());
        let mut arena = crate::inference::Arena::source(1, false);
        let a = arena.rigid_effect(0, SPAN, &mut meter)?;
        let hole = arena.effect_hole(SPAN, &mut meter)?;
        let joined = arena.effect_union(a, hole, SPAN, &mut meter)?;
        arena.equate_effects(a, joined, SPAN, &mut meter)?;
        let pure = arena.effect_empty(SPAN, &mut meter)?;
        arena.require_rigid_effect(pure, pure, SPAN, &mut meter)?;
        super::rigid(&arena, SPAN, &mut meter)
    }

    #[test]
    fn rigid_fixed_point_charges_every_pass_effect_equation_and_propagation_step() {
        // Three passes (the third changes nothing), each charging the pass,
        // three effects, one equation and one propagation step per side.
        let mut arena = crate::inference::Arena::new();
        arena.effects = alloc::vec![Effect::Constant(1), Effect::Hole, Effect::Union(0, 1)];
        arena.effect_equations = alloc::vec![(1, 0)];
        arena.rigid_effect_goals = alloc::vec![(2, 2)];
        let limits = |work| crate::Limits {
            work,
            ..crate::Limits::default()
        };
        let mut meter = crate::Meter::new(limits(21));
        assert!(super::rigid(&arena, SPAN, &mut meter).is_ok());
        assert_eq!(meter.spent(), 21);
        let mut meter = crate::Meter::new(limits(20));
        let refused = super::rigid(&arena, SPAN, &mut meter)
            .expect_err("the last propagation step needs the 21st unit");
        assert_eq!(refused.message, "frontend work limit exceeded");
        assert_eq!(meter.spent(), 20);
    }
}
