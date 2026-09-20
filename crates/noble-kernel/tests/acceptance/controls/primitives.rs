pub(super) fn duplicate() -> super::WordControl {
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    super::WordControl {
        def: 0,
        stack_in: vec![noble_kernel::types::Ty::I64],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
        ],
        stack_out: vec![noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64],
        allowed: &[],
        reject_stack_in: vec![noble_kernel::types::Ty::I64],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(resource.clone()),
        ],
        constraint: noble_kernel::untrusted::Constraint::Eligibility(resource.clone()),
    }
}

pub(super) fn discard() -> super::WordControl {
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    super::WordControl {
        def: 1,
        stack_in: vec![noble_kernel::types::Ty::I64],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
        ],
        stack_out: vec![],
        allowed: &[],
        reject_stack_in: vec![resource.clone()],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(resource.clone()),
        ],
        constraint: noble_kernel::untrusted::Constraint::Eligibility(resource.clone()),
    }
}

pub(super) fn swap() -> super::WordControl {
    super::WordControl {
        def: 2,
        stack_in: vec![noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
        stack_out: vec![noble_kernel::types::Ty::Bool, noble_kernel::types::Ty::I64],
        allowed: &[],
        reject_stack_in: vec![noble_kernel::types::Ty::I64],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}

pub(super) fn integer_binary(
    definition: u32,
    result: noble_kernel::types::Ty,
) -> super::WordControl {
    super::WordControl {
        def: definition,
        stack_in: vec![noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64],
        positive: vec![crate::support::segment(vec![])],
        stack_out: vec![result],
        allowed: &[],
        reject_stack_in: vec![noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool],
        reject: vec![crate::support::segment(vec![])],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}

pub(super) fn emit() -> super::WordControl {
    super::WordControl {
        def: 22,
        stack_in: vec![noble_kernel::types::Ty::Text],
        positive: vec![crate::support::segment(vec![])],
        stack_out: vec![noble_kernel::types::Ty::Unit],
        allowed: &[0],
        reject_stack_in: vec![noble_kernel::types::Ty::Text],
        reject: vec![crate::support::segment(vec![])],
        constraint: noble_kernel::untrusted::Constraint::EffectInclusion(
            noble_kernel::types::EffId(0),
        ),
    }
}
