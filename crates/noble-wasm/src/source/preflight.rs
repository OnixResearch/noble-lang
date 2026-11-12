//! Bounded path-only tree inspection before the kernel's owned clone boundary.
//! Paths contain child indices, never borrowed nodes or recursively cloned input.

#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; validation mutates only preparation-owned work accounting and fresh bounded scratch buffers; the borrowed submission and published compiler remain unchanged."
)]

const STACK_LIMIT: usize = 128;
const TYPE_LIMIT: u32 = 512;
const PATH_LIMIT: usize = 64;

pub(in crate::source) mod concrete;
mod schemes;
mod variables;

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; borrowed preflight checks every candidate, quotation and binding bound before owned kernel cloning; oversized payloads and metered structural walks must return Exhausted, not assert on input."
)]
fn body(
    body: &noble_kernel::execution::Body,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    if body.candidate.nodes.len() > super::NODE_LIMIT
        || body.candidate.body.len() > super::NODE_LIMIT
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(work.entries(body.candidate.body.len()));
    let mut index = 0usize;
    let mut failure = None;
    while index < body.candidate.nodes.len() {
        match node(&body.candidate.nodes[index], limits, work) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; each borrowed node checks quotation and binding bounds before metered binding inspection; the first input failure is returned without asserting on producer data."
)]
fn node(
    node: &noble_kernel::untrusted::Node,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    let inst = match node {
        noble_kernel::untrusted::Node::Literal { inst, .. }
        | noble_kernel::untrusted::Node::Invocation { inst, .. }
        | noble_kernel::untrusted::Node::SignedSelf { inst, .. } => inst,
        noble_kernel::untrusted::Node::Quotation { inst, body } => {
            if body.len() > super::NODE_LIMIT {
                return Err(crate::Diagnostic::Exhausted);
            }
            attempt!(work.entries(body.len()));
            inst
        }
        noble_kernel::untrusted::Node::SlotInvoke {
            forwarded_ref_ordinals,
            ..
        } => {
            if forwarded_ref_ordinals.len() > STACK_LIMIT {
                return Err(crate::Diagnostic::Exhausted);
            }
            return work.entries(forwarded_ref_ordinals.len().saturating_add(1));
        }
    };
    check_inst(inst, limits, work)
}

fn check_inst(
    inst: &noble_kernel::words::Inst,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    if inst.bindings.len() > 8 {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(work.entries(inst.bindings.len().saturating_add(1)));
    let mut index = 0usize;
    let mut failure = None;
    while index < inst.bindings.len() {
        match binding(&inst.bindings[index], limits, work) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; borrowed binding inspection uses the non-const effect accessor and allocates bounded path scratch for metered structural type walks."
)]
fn binding(
    binding: &noble_kernel::words::Binding,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    match binding {
        noble_kernel::words::Binding::Stack(value) => concrete::stack(value, limits, work),
        noble_kernel::words::Binding::Value(value) => {
            concrete::sized(value, limits.type_size, work)
        }
        noble_kernel::words::Binding::Effect(value) => {
            if value.as_slice().len() > 6 {
                Err(crate::Diagnostic::Exhausted)
            } else {
                Ok(())
            }
        }
        noble_kernel::words::Binding::Ref(_) => Ok(()),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; effect slice access and the metered borrowed stack traversal are non-const operations; this helper preserves their diagnostic ordering before kernel cloning."
)]
fn expected(
    expected: &noble_kernel::untrusted::Expected,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    if expected.allowed_effects.as_slice().len() > 8 {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(concrete::stack(&expected.stack_in, limits, work));
    concrete::stack(&expected.stack_out, limits, work)
}

fn definition(
    definition: &noble_kernel::execution::Definition,
    limits: noble_kernel::untrusted::Limits,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    attempt!(expected(&definition.expected, limits, work));
    if let Some(inst) = definition.owner_inst.as_ref() {
        attempt!(check_inst(inst, limits, work));
    }
    body(&definition.body, limits, work)
}

/// The profile's definition limit.
const fn definition_limit(calculator: bool) -> usize {
    if calculator {
        super::CALCULATOR_DEFINITION_LIMIT
    } else {
        super::DEFINITION_LIMIT
    }
}

/// The profile's environment row cap: its definition limit plus the fixed rows
/// that only this profile installs. The calculator owns 32 fixed rows, the pure
/// byte cursor 27 and declared text 28; ordinary profiles own 26.
const fn maximum_rows(calculator: bool, text_cursor: bool, declared_text: bool) -> usize {
    if calculator {
        noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT
    } else if declared_text {
        definition_limit(false).saturating_add(28)
    } else if text_cursor {
        definition_limit(false).saturating_add(27)
    } else {
        definition_limit(false).saturating_add(26)
    }
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-wasm-maintainers; the selected lowering or preflight stage keeps its checked step order in one reviewable unit, and splitting it would separate the emitted sequence from its bounds checks."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; bounded environment schemes, expected interfaces and every body are inspected with one shared meter before any recursive clone; oversized untrusted submissions return diagnostics rather than panic."
)]
pub(super) fn check(
    submission: &noble_kernel::execution::Submission,
    calculator: bool,
    text_cursor: bool,
    declared_text: bool,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    // Profile selection is the compiler's own flags, never an untrusted Env
    // marker. The cap is the profile's definition limit plus its own fixed
    // prefix, exactly as `admission::environment` counts rows: the calculator
    // adds 32 rows, the pure byte cursor adds its row (27) and declared text
    // its signed-comparison row (28). A narrower prefix here would refuse a
    // valid full-definition environment before the profile check ran.
    let maximum_rows = maximum_rows(calculator, text_cursor, declared_text);
    if submission.environment.defs.len() > maximum_rows
        || submission.definitions.len() > definition_limit(calculator)
    {
        return Err(crate::Diagnostic::Exhausted);
    }
    if calculator {
        let budget = attempt!(submission
            .environment
            .calculator_clone_budget()
            .ok_or(crate::Diagnostic::Exhausted));
        let count = attempt!(submission
            .definitions
            .len()
            .checked_add(1)
            .ok_or(crate::Diagnostic::Exhausted));
        let copies = attempt!(u64::try_from(count).map_err(|_| crate::Diagnostic::Exhausted));
        attempt!(budget
            .reserved_bytes
            .checked_mul(copies)
            .filter(|total| *total <= noble_kernel::contracts::CALCULATOR_ENV_CLONE_FANOUT_LIMIT)
            .ok_or(crate::Diagnostic::Exhausted));
        attempt!(work.charge(attempt!(budget
            .bytes
            .checked_mul(copies)
            .ok_or(crate::Diagnostic::Exhausted))));
    }
    let mut index = 0usize;
    let mut failure = None;
    // The selected clone-budget walk has already inspected every scheme
    // constructor, embedded nominal descriptor and depth/arity bound.
    // Do not repeat the old borrowed tree walk before independent admission.
    if !calculator {
        while index < submission.environment.defs.len() {
            match schemes::check(&submission.environment.defs[index], work) {
                Ok(()) => index += 1,
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    if calculator {
        attempt!(variables::schemes(submission, work));
    }
    if !calculator {
        index = 0;
        while index < submission.environment.nominals.len() {
            let shape = &submission.environment.nominals[index].shape;
            match nominal_shape(shape, work) {
                Ok(()) => index += 1,
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    attempt!(expected(
        &submission.request.expected,
        submission.request.limits,
        work
    ));
    attempt!(body(&submission.body, submission.request.limits, work));
    index = 0;
    while index < submission.definitions.len() {
        match definition(
            &submission.definitions[index],
            submission.request.limits,
            work,
        ) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None if calculator => variables::check(submission, work),
        None => Ok(()),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; pinned March and August const trials reject concrete::ty as nonconst (E0015) for both opaque and variant shapes; reassess when concrete::ty becomes const-capable."
)]
fn nominal_shape(
    shape: &noble_kernel::types::NominalShape,
    work: &mut super::Work,
) -> Result<(), crate::Diagnostic> {
    match shape {
        noble_kernel::types::NominalShape::Opaque(ty) => concrete::ty(ty, TYPE_LIMIT, work),
        noble_kernel::types::NominalShape::Variant(left, right) => {
            attempt!(concrete::ty(left, TYPE_LIMIT, work));
            concrete::ty(right, TYPE_LIMIT, work)
        }
    }
}

// Test harness only: the profile row cap and a full-definition environment.
#[allow(
    tigerstyle::function_length,
    tigerstyle::non_trait_imports,
    tigerstyle::numeric_units,
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; test harness: descriptive scenario names, fixture imports and long setup legs are the readable form."
)]
#[cfg(test)]
mod row_cap_tests {
    use super::super::{Work, DEFINITION_LIMIT};
    use super::{check, maximum_rows};
    use alloc::format;
    use alloc::string::String;
    use alloc::vec::Vec;
    use noble_contracts::source::{ModuleSession, RetainedSourceLimit};

    fn budget(limit: u32) -> Work {
        Work {
            remaining: u64::from(limit),
            issued: u64::from(limit),
            calculator_preflight: false,
            calculator_plan: false,
            max_type_nodes: 0,
        }
    }

    #[test]
    fn maximum_rows_follows_the_profile_prefix() {
        // Ordinary profiles own 26 fixed rows, the pure byte cursor 27 and
        // declared text 28; the calculator owns its 32-row table.
        assert_eq!(maximum_rows(false, false, false), DEFINITION_LIMIT + 26);
        assert_eq!(maximum_rows(false, true, false), DEFINITION_LIMIT + 27);
        assert_eq!(maximum_rows(false, false, true), DEFINITION_LIMIT + 28);
        assert_eq!(maximum_rows(false, true, true), DEFINITION_LIMIT + 28);
        assert_eq!(
            maximum_rows(true, false, false),
            noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT
        );
    }

    fn bulk_module(named: usize) -> Vec<u8> {
        let mut source = String::from("module bulk@1 [");
        for index in 0..named {
            source.push_str(&format!(
                " signature w{index} forall<S:stack> [ S -- S I64 ! pure ] export w{index} def w{index} [ 1 ]"
            ));
        }
        source.push_str(" ]");
        source.into_bytes()
    }

    /// Declared text is the only session-selectable profile that installs the
    /// byte cursor row (26) and a further row (27), so it is the profile whose
    /// full 256-definition environment the old 256+26 cap refused before the
    /// profile check ran. The +27 branch is pinned above.
    #[test]
    fn declared_text_environment_of_256_definitions_passes_preflight() {
        let limits = noble_contracts::Limits {
            bytes: 65_536,
            nodes: 262_144,
            depth: 64,
            work: 512_000_000,
        };
        let retained = RetainedSourceLimit::new(65_536).expect("valid retained source cap");
        let session = ModuleSession::new_declared_text(&[], false, retained)
            .unwrap_or_else(|_| panic!("selected source profile"));
        let pending = session
            .prepare(&bulk_module(DEFINITION_LIMIT), &[], limits)
            .unwrap_or_else(|error| {
                panic!("module with {DEFINITION_LIMIT} definitions: {error:?}")
            });
        let (session, installed) = session.commit(pending);
        installed.unwrap_or_else(|error| {
            panic!("module with {DEFINITION_LIMIT} definitions refused: {error:?}")
        });
        // Reference every export so the query closure carries all 256 definitions.
        let mut expression = String::new();
        for index in 0..DEFINITION_LIMIT {
            if index != 0 {
                expression.push_str(" drop ");
            }
            expression.push_str(&format!("bulk@1.w{index}"));
        }
        let query = session
            .prepare(expression.as_bytes(), &[], limits)
            .unwrap_or_else(|_| panic!("query over the full namespace"));
        let submission = query
            .submission()
            .unwrap_or_else(|| panic!("missing submission"));
        assert_eq!(
            submission.environment.defs.len(),
            28 + DEFINITION_LIMIT,
            "the declared-text environment installs 28 fixed rows"
        );
        let mut work = budget(submission.request.limits.work);
        assert!(
            check(submission, false, true, true, &mut work).is_ok(),
            "a full-definition declared-text environment must pass preflight"
        );
    }
}
