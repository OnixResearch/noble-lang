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
    let mut ids = alloc::vec::Vec::new();
    if bits & 1 != 0 {
        ids.push(noble_kernel::types::EffId(0));
    }
    if bits & 2 != 0 {
        ids.push(noble_kernel::types::EffId(1));
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
