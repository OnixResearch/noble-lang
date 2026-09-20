#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; dip constructs paired positive and stack-join-negative fixtures; the control runner checks both checker outcomes rather than asserting during fixture construction."
)]
pub(super) fn dip() -> super::WordControl {
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    super::WordControl {
        def: 3,
        stack_in: vec![
            noble_kernel::types::Ty::I64,
            super::words::prog(vec![], vec![noble_kernel::types::Ty::Unit], &[]),
        ],
        // `dip`'s branch consumes the same stack tail `S` it runs
        // under: with `S = []` the program reads `[] -- [Unit]`.
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[]),
        ],
        stack_out: vec![noble_kernel::types::Ty::Unit, noble_kernel::types::Ty::I64],
        allowed: &[],
        reject_stack_in: vec![
            noble_kernel::types::Ty::I64,
            super::words::prog(
                vec![noble_kernel::types::Ty::I64],
                vec![noble_kernel::types::Ty::Unit],
                &[],
            ),
        ],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(resource.clone()),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[]),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; capture constructs Data-positive and resource-eligibility-negative fixtures; the control runner checks both outcomes rather than asserting during fixture construction."
)]
pub(super) fn capture() -> super::WordControl {
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    super::WordControl {
        def: 8,
        stack_in: vec![noble_kernel::types::Ty::I64],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            crate::support::segment(vec![noble_kernel::types::Ty::I64]),
        ],
        stack_out: vec![super::words::prog(
            vec![noble_kernel::types::Ty::I64],
            vec![noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64],
            &[],
        )],
        allowed: &[],
        reject_stack_in: vec![resource.clone()],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(resource.clone()),
            crate::support::segment(vec![resource.clone()]),
        ],
        constraint: noble_kernel::untrusted::Constraint::Eligibility(resource.clone()),
    }
}

pub(super) fn compose() -> super::WordControl {
    super::WordControl {
        def: 9,
        stack_in: vec![
            super::words::prog(vec![], vec![noble_kernel::types::Ty::I64], &[]),
            super::words::prog(
                vec![noble_kernel::types::Ty::I64],
                vec![noble_kernel::types::Ty::Unit],
                &[],
            ),
        ],
        positive: vec![
            crate::support::segment(vec![]),
            crate::support::segment(vec![]),
            crate::support::segment(vec![noble_kernel::types::Ty::I64]),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[]),
            crate::support::effect_binding(&[]),
        ],
        stack_out: vec![super::words::prog(
            vec![],
            vec![noble_kernel::types::Ty::Unit],
            &[],
        )],
        allowed: &[],
        reject_stack_in: vec![
            super::words::prog(vec![], vec![noble_kernel::types::Ty::I64], &[]),
            super::words::prog(
                vec![noble_kernel::types::Ty::Bool],
                vec![noble_kernel::types::Ty::Unit],
                &[],
            ),
        ],
        reject: vec![
            crate::support::segment(vec![]),
            crate::support::segment(vec![]),
            crate::support::segment(vec![noble_kernel::types::Ty::I64]),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[]),
            crate::support::effect_binding(&[]),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}

pub(super) fn run() -> super::WordControl {
    super::WordControl {
        def: 10,
        stack_in: vec![
            noble_kernel::types::Ty::I64,
            super::words::prog(
                vec![noble_kernel::types::Ty::I64],
                vec![noble_kernel::types::Ty::Unit],
                &[],
            ),
        ],
        positive: vec![
            crate::support::segment(vec![noble_kernel::types::Ty::I64]),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[]),
        ],
        stack_out: vec![noble_kernel::types::Ty::Unit],
        allowed: &[],
        reject_stack_in: vec![
            noble_kernel::types::Ty::I64,
            super::words::prog(
                vec![noble_kernel::types::Ty::I64],
                vec![noble_kernel::types::Ty::Unit],
                &[],
            ),
        ],
        reject: vec![
            crate::support::segment(vec![noble_kernel::types::Ty::I64]),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[0]),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}

pub(super) fn inspect() -> super::WordControl {
    super::WordControl {
        def: 11,
        stack_in: vec![super::words::prog(vec![], vec![], &[])],
        positive: vec![
            crate::support::segment(vec![]),
            crate::support::segment(vec![]),
            crate::support::segment(vec![]),
            crate::support::effect_binding(&[]),
        ],
        stack_out: vec![noble_kernel::types::Ty::Syntax],
        allowed: &[],
        reject_stack_in: vec![noble_kernel::types::Ty::I64],
        reject: vec![
            crate::support::segment(vec![]),
            crate::support::segment(vec![]),
            crate::support::segment(vec![]),
            crate::support::effect_binding(&[]),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}
