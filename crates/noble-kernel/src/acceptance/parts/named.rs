//! Eligibility checks for polymorphic `Behavior::Named` schemes.
//!
//! Built-in `dup`, `drop`, and `quote` enforce `Data` on their fixed variable
//! slot. A user-defined `Behavior::Named` scheme may carry arbitrary rank-1
//! stack and value variables; any variable used non-linearly (duplicated,
//! dropped, fabricated, or captured into a returned program) must bind only
//! `Data` types at instantiation time.

/// Local bound for one pattern walk, matching `shapes::WORK_CAP`.
const WORK_CAP: usize = 512;

/// Occurrence summary of one scheme variable across `stack_in` and `stack_out`.
#[derive(Clone, Copy)]
struct VarUsage {
    supplied: u32,
    demanded: u32,
    direct_in: bool,
    in_returned_program: bool,
    bare_out: bool,
}

impl VarUsage {
    const fn empty() -> Self {
        Self {
            supplied: 0,
            demanded: 0,
            direct_in: false,
            in_returned_program: false,
            bare_out: false,
        }
    }

    const fn requires_data(self) -> bool {
        (self.direct_in && self.in_returned_program)
            || (self.supplied > 0 && self.supplied != self.demanded)
            || (self.supplied == 0 && self.bare_out)
    }
}

/// Linear-logic polarity and quotation context of one pattern position.
#[derive(Clone, Copy)]
enum Role {
    SupplyOuter,
    SupplyInner { in_returned: bool },
    DemandBare,
    DemandGuarded { in_returned: bool },
}

/// One owned worklist step: a pattern and its polarity role.
struct Step {
    pattern: crate::shapes::Pattern,
    role: Role,
}

/// Enforce `Data` eligibility on any non-linearly used variable of a
/// `Behavior::Named` scheme against its resolved witness.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; check_eligibility rejects non-Data instantiations of non-linear named schemes via Eligibility diagnostics and fails closed on work exhaustion rather than panicking."
)]
pub(crate) fn check_eligibility(
    kind: Option<crate::contracts::Behavior>,
    scheme: &crate::words::Scheme,
    resolved: &crate::words::Inst,
    at: super::Site,
) -> Result<(), super::super::Fail> {
    if !matches!(kind, Some(crate::contracts::Behavior::Named)) {
        return Ok(());
    }
    if !has_non_data_binding(scheme, resolved) {
        return Ok(());
    }
    let usage = attempt!(collect_usage(scheme));
    let mut index = 0usize;
    let mut violating: Option<crate::types::Ty> = None;
    while index < scheme.var_kinds.len() && violating.is_none() {
        if usage[index].requires_data() {
            violating = first_non_data_at(scheme.var_kinds[index], resolved, index);
        }
        index += 1;
    }
    match violating {
        Some(ty) => Err(super::invalid_without_stacks(
            at,
            crate::untrusted::Constraint::Eligibility(ty),
        )),
        None => Ok(()),
    }
}

/// Fast path: skip pattern walking when every value and stack binding is `Data`.
fn has_non_data_binding(scheme: &crate::words::Scheme, resolved: &crate::words::Inst) -> bool {
    let mut index = 0usize;
    let mut found = false;
    while index < scheme.var_kinds.len() && !found {
        found = first_non_data_at(scheme.var_kinds[index], resolved, index).is_some();
        index += 1;
    }
    found
}

/// The first non-`Data` type bound to variable `index`, if any.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; first_non_data_at inspects runtime Inst bindings and clones the first violating Ty for the Eligibility diagnostic."
)]
fn first_non_data_at(
    kind: crate::words::VariableKind,
    resolved: &crate::words::Inst,
    index: usize,
) -> Option<crate::types::Ty> {
    let Ok(slot) = u32::try_from(index) else {
        return None;
    };
    let var = crate::words::Variable(slot);
    match kind {
        crate::words::VariableKind::Value => match resolved.value(var) {
            Some(ty) if !ty.is_data() => Some(ty.clone()),
            Some(_) | None => None,
        },
        crate::words::VariableKind::Stack => match resolved.stack(var) {
            Some(stack) => first_non_data_in_stack(stack),
            None => None,
        },
        crate::words::VariableKind::Effect => None,
    }
}

fn first_non_data_in_stack(stack: &[crate::types::Ty]) -> Option<crate::types::Ty> {
    let mut index = 0usize;
    let mut found = None;
    while index < stack.len() && found.is_none() {
        if !stack[index].is_data() {
            found = Some(stack[index].clone());
        }
        index += 1;
    }
    found
}

/// Iteratively count signed occurrences of each variable in `scheme`.
fn collect_usage(
    scheme: &crate::words::Scheme,
) -> Result<alloc::vec::Vec<VarUsage>, super::super::Fail> {
    let mut usage = alloc::vec![VarUsage::empty(); scheme.var_kinds.len()];
    let mut work = alloc::vec::Vec::with_capacity(
        scheme.stack_in.len().saturating_add(scheme.stack_out.len()),
    );
    push_slice(&scheme.stack_in, Role::SupplyOuter, &mut work);
    push_slice(&scheme.stack_out, Role::DemandBare, &mut work);
    let mut steps = 0usize;
    let mut exhausted = false;
    while let Some(step) = work.pop() {
        if steps >= WORK_CAP {
            exhausted = true;
            break;
        }
        steps += 1;
        visit_pattern(step.pattern, step.role, &mut usage, &mut work);
    }
    if exhausted {
        Err(super::super::Fail::Exhausted(
            crate::untrusted::LimitKind::Work,
        ))
    } else {
        Ok(usage)
    }
}

fn push_slice(patterns: &[crate::shapes::Pattern], role: Role, work: &mut alloc::vec::Vec<Step>) {
    let mut index = 0usize;
    while index < patterns.len() {
        work.push(Step {
            pattern: patterns[index].clone(),
            role,
        });
        index += 1;
    }
}

fn record_var(usage: &mut [VarUsage], var: crate::words::Variable, role: Role) {
    let Ok(index) = usize::try_from(var.0) else {
        return;
    };
    let Some(entry) = usage.get_mut(index) else {
        return;
    };
    match role {
        Role::SupplyOuter => {
            entry.supplied = entry.supplied.saturating_add(1);
            entry.direct_in = true;
        }
        Role::SupplyInner { in_returned } => {
            entry.supplied = entry.supplied.saturating_add(1);
            if in_returned {
                entry.in_returned_program = true;
            }
        }
        Role::DemandBare => {
            entry.demanded = entry.demanded.saturating_add(1);
            entry.bare_out = true;
        }
        Role::DemandGuarded { in_returned } => {
            entry.demanded = entry.demanded.saturating_add(1);
            if in_returned {
                entry.in_returned_program = true;
            }
        }
    }
}

const fn guarded_child_role(role: Role) -> Role {
    match role {
        Role::DemandBare => Role::DemandGuarded { in_returned: false },
        Role::SupplyOuter | Role::SupplyInner { .. } | Role::DemandGuarded { .. } => role,
    }
}

const fn program_child_roles(role: Role) -> (Role, Role) {
    match role {
        Role::SupplyOuter | Role::SupplyInner { in_returned: false } => (
            Role::DemandGuarded { in_returned: false },
            Role::SupplyInner { in_returned: false },
        ),
        Role::DemandBare | Role::DemandGuarded { .. } | Role::SupplyInner { in_returned: true } => {
            (
                Role::SupplyInner { in_returned: true },
                Role::DemandGuarded { in_returned: true },
            )
        }
    }
}

#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; every Pattern constructor must classify its children's linearity role explicitly so new pattern forms cannot silently bypass named-scheme Data eligibility."
)]
fn visit_pattern(
    pattern: crate::shapes::Pattern,
    role: Role,
    usage: &mut [VarUsage],
    work: &mut alloc::vec::Vec<Step>,
) {
    match pattern {
        crate::shapes::Pattern::Var(var) | crate::shapes::Pattern::StackVar(var) => {
            record_var(usage, var, role);
        }
        crate::shapes::Pattern::Pair(left, right) => {
            work.push(Step {
                pattern: *left,
                role,
            });
            work.push(Step {
                pattern: *right,
                role,
            });
        }
        crate::shapes::Pattern::Sum(left, right) => {
            let child = guarded_child_role(role);
            work.push(Step {
                pattern: *left,
                role: child,
            });
            work.push(Step {
                pattern: *right,
                role: child,
            });
        }
        crate::shapes::Pattern::List(elem) => {
            let child = guarded_child_role(role);
            work.push(Step {
                pattern: *elem,
                role: child,
            });
        }
        crate::shapes::Pattern::GenericNominal(_, args, _) => {
            let child = guarded_child_role(role);
            push_slice(args.as_slice(), child, work);
        }
        crate::shapes::Pattern::Program(stack_in, stack_out, _)
        | crate::shapes::Pattern::LiveRef(stack_in, stack_out, _) => {
            let (in_role, out_role) = program_child_roles(role);
            push_slice(&stack_in, in_role, work);
            push_slice(&stack_out, out_role, work);
        }
        crate::shapes::Pattern::Unit
        | crate::shapes::Pattern::Bool
        | crate::shapes::Pattern::I64
        | crate::shapes::Pattern::Text
        | crate::shapes::Pattern::Syntax
        | crate::shapes::Pattern::Contract
        | crate::shapes::Pattern::Evidence
        | crate::shapes::Pattern::Certified
        | crate::shapes::Pattern::Resource(_)
        | crate::shapes::Pattern::Nominal(_, _) => {}
    }
}
