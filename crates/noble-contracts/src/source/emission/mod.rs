mod contracts;
mod materialization;

#[derive(Clone, Copy)]
struct Admission<'a> {
    input_bytes: u32,
    limits: noble_kernel::untrusted::Limits,
    environment: &'a noble_kernel::contracts::Env,
}

struct Assembly {
    definitions: alloc::vec::Vec<noble_kernel::execution::Definition>,
    root: Option<(
        noble_kernel::execution::Body,
        noble_kernel::untrusted::Request,
    )>,
}

struct CheckedBody {
    body: noble_kernel::execution::Body,
    request: noble_kernel::untrusted::Request,
    identity: Option<u64>,
    span: crate::Span,
}

impl Assembly {
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; installation appends owned definitions and can construct checked-ID diagnostics, neither of which is const evaluation."
    )]
    fn install(
        &mut self,
        mut checked: CheckedBody,
        position: usize,
        downstream_work: u32,
    ) -> Result<(), super::Error> {
        match checked.identity {
            Some(identity) => {
                let definition = match crate::index(position.saturating_add(23), checked.span) {
                    Ok(id) => noble_kernel::contracts::Definition(id),
                    Err(error) => return Err(super::Error::at(crate::source::Stage::Check, error)),
                };
                self.definitions.push(noble_kernel::execution::Definition {
                    definition,
                    identity,
                    body: checked.body,
                    expected: checked.request.expected,
                });
            }
            None => {
                // Backend admission is an independent bounded stage, not one
                // more specialization sharing the frontend's reservation.
                checked.request.limits.work = downstream_work;
                self.root = Some((checked.body, checked.request));
            }
        }
        Ok(())
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; closing, concrete interface materialization, and independent body checks propagate typed failures; emission requires a checked root before returning a submission."
)]
pub(super) fn emit(
    mut state: super::inference::State,
    mut environment: noble_kernel::contracts::Env,
    input_bytes: usize,
    meter: &mut crate::Meter,
) -> Result<
    (
        noble_kernel::execution::Submission,
        alloc::vec::Vec<noble_kernel::types::Ty>,
    ),
    super::Error,
> {
    if let Err(error) = state.arena.close(state.span, meter) {
        return Err(super::Error::at(crate::source::Stage::Check, error));
    }
    let input_bytes = match crate::index(input_bytes, state.span) {
        Ok(bytes) => bytes,
        Err(error) => return Err(super::Error::at(crate::source::Stage::Check, error)),
    };
    let expected = match interfaces(&state, &mut environment, meter) {
        Ok(expected) => expected,
        Err(error) => return Err(super::Error::at(crate::source::Stage::Check, error)),
    };
    let span = state.span;
    let assembly = attempt!(assemble(state, expected, &environment, input_bytes, meter));
    let (body, request) = match assembly.root {
        Some(root) => root,
        None => {
            return Err(super::Error::at(
                crate::source::Stage::Check,
                crate::internal(span),
            ))
        }
    };
    let output = request.expected.stack_out.clone();
    Ok((
        noble_kernel::execution::Submission {
            environment,
            definitions: assembly.definitions,
            body,
            request,
        },
        output,
    ))
}

fn interfaces(
    state: &super::inference::State,
    environment: &mut noble_kernel::contracts::Env,
    meter: &mut crate::Meter,
) -> Result<alloc::vec::Vec<noble_kernel::untrusted::Expected>, crate::Diagnostic> {
    let mut expected = alloc::vec::Vec::with_capacity(state.bodies.len());
    let mut at = 0usize;
    let mut failure = None;
    while at < state.bodies.len() {
        match install_interface(&state.bodies[at], &state.arena, environment, meter) {
            Ok(interface) => expected.push(interface),
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
        at += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(expected),
    }
}

fn install_interface(
    body: &super::inference::Body,
    arena: &crate::inference::Arena,
    environment: &mut noble_kernel::contracts::Env,
    meter: &mut crate::Meter,
) -> Result<noble_kernel::untrusted::Expected, crate::Diagnostic> {
    let interface = attempt!(materialization::interface(arena, body, meter));
    if body.identity.is_some() {
        let scheme = attempt!(contracts::scheme(&interface, body.span, meter));
        let deps = attempt!(materialization::dependencies(body, meter));
        environment.defs.reserve(1);
        environment.kinds.reserve(1);
        environment.deps.reserve(1);
        environment.defs.push(scheme);
        environment
            .kinds
            .push(noble_kernel::contracts::Behavior::Named);
        environment.deps.push(deps);
    }
    Ok(interface)
}

#[expect(
    clippy::while_let_on_iterator,
    reason = "Owner: noble-maintainers; the explicit owned iterator avoids the pinned architecture collector's UnsupportedExpansion for Desugaring(ForLoop); retain complete required compiler facts until that desugaring is supported."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; allowance uses checked division, every body must have its matching expected interface, and checked installation preserves the root's independent full downstream work limit."
)]
fn assemble(
    state: super::inference::State,
    expected: alloc::vec::Vec<noble_kernel::untrusted::Expected>,
    environment: &noble_kernel::contracts::Env,
    input_bytes: u32,
    meter: &mut crate::Meter,
) -> Result<Assembly, super::Error> {
    let limits = match allowance(state.bodies.len(), state.span, meter) {
        Ok(limits) => limits,
        Err(error) => return Err(super::Error::at(crate::source::Stage::Check, error)),
    };
    let admission = Admission {
        input_bytes,
        limits,
        environment,
    };
    let mut assembly = Assembly {
        definitions: alloc::vec::Vec::with_capacity(state.bodies.len().saturating_sub(1)),
        root: None,
    };
    let mut expected = expected.into_iter();
    let arena = state.arena;
    let mut drafts = state.bodies.into_iter();
    let mut position = 0usize;
    let mut failure = None;
    while let Some(draft) = drafts.next() {
        let result = match checked(draft, expected.next(), &arena, admission, meter) {
            Ok(body) => assembly.install(body, position, meter.limits.work),
            Err(problem) => Err(problem),
        };
        if let Err(problem) = result {
            failure = Some(problem);
            break;
        }
        position = position.saturating_add(1);
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(assembly),
    }
}

fn allowance(
    body_count: usize,
    span: crate::Span,
    meter: &crate::Meter,
) -> Result<noble_kernel::untrusted::Limits, crate::Diagnostic> {
    // Reserve at most half the remaining frontend work for all independent
    // dependency and node checks. Unused checker work is not reclaimed.
    let body_count = attempt!(crate::index(body_count, span));
    let work = match body_count
        .checked_mul(4)
        .and_then(|count| meter.work.checked_div(count))
    {
        Some(allowance) => allowance,
        None => return Err(crate::internal(span)),
    };
    Ok(noble_kernel::untrusted::Limits {
        bytes: meter.limits.bytes,
        nodes: meter.limits.nodes,
        depth: meter.limits.depth,
        type_size: crate::syntax::TYPE_CAP,
        stack_height: crate::inference::STACK_CAP,
        work,
        diagnostics: 8,
    })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; witness materialization, reserved checker work, and independent kernel checking are fallible admission checks, not conditions that may panic on source input."
)]
fn checked(
    draft: super::inference::Body,
    expected: Option<noble_kernel::untrusted::Expected>,
    arena: &crate::inference::Arena,
    admission: Admission<'_>,
    meter: &mut crate::Meter,
) -> Result<CheckedBody, super::Error> {
    let expected = match expected {
        Some(expected) => expected,
        None => {
            return Err(super::Error::at(
                crate::source::Stage::Check,
                crate::internal(draft.span),
            ))
        }
    };
    let request = noble_kernel::untrusted::Request {
        input_bytes: admission.input_bytes,
        expected,
        limits: admission.limits,
    };
    let span = draft.span;
    let identity = draft.identity;
    let (body, spans) = match materialization::body(draft, arena, meter) {
        Ok(body) => body,
        Err(error) => return Err(super::Error::at(crate::source::Stage::Check, error)),
    };
    // Every concrete body derives independently: the root invocation contract
    // alone is not evidence for any definition's implementation.
    if let Err(error) = meter.charge(admission.limits.work.saturating_mul(2), span) {
        return Err(super::Error::at(crate::source::Stage::Acceptance, error));
    }
    if let Err(error) = crate::program::check(
        admission.environment,
        &body.candidate,
        &request,
        &spans,
        span,
    ) {
        return Err(super::Error::at(crate::source::Stage::Acceptance, error));
    }
    Ok(CheckedBody {
        body,
        request,
        identity,
        span,
    })
}
