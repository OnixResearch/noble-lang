pub(super) fn case() -> super::WordControl {
    super::WordControl {
        def: 17,
        stack_in: vec![
            super::words::sum_of(noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool),
            super::words::prog(
                vec![noble_kernel::types::Ty::I64],
                vec![noble_kernel::types::Ty::Unit],
                &[],
            ),
            super::words::prog(
                vec![noble_kernel::types::Ty::Bool],
                vec![noble_kernel::types::Ty::Unit],
                &[],
            ),
        ],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[]),
            crate::support::effect_binding(&[]),
        ],
        stack_out: vec![noble_kernel::types::Ty::Unit],
        allowed: &[],
        reject_stack_in: vec![
            super::words::sum_of(noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool),
            super::words::prog(
                vec![noble_kernel::types::Ty::I64],
                vec![noble_kernel::types::Ty::Unit],
                &[],
            ),
            super::words::prog(
                vec![noble_kernel::types::Ty::Bool],
                vec![noble_kernel::types::Ty::I64],
                &[],
            ),
        ],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[]),
            crate::support::effect_binding(&[]),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}

pub(super) fn conditional() -> super::WordControl {
    super::WordControl {
        def: 18,
        stack_in: vec![
            noble_kernel::types::Ty::Bool,
            super::words::prog(vec![], vec![noble_kernel::types::Ty::Unit], &[]),
            super::words::prog(vec![], vec![noble_kernel::types::Ty::Unit], &[]),
        ],
        positive: vec![
            crate::support::segment(vec![]),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[]),
            crate::support::effect_binding(&[]),
        ],
        stack_out: vec![noble_kernel::types::Ty::Unit],
        allowed: &[],
        reject_stack_in: vec![
            noble_kernel::types::Ty::Bool,
            super::words::prog(vec![], vec![noble_kernel::types::Ty::Unit], &[]),
            super::words::prog(vec![], vec![noble_kernel::types::Ty::Unit], &[]),
        ],
        reject: vec![
            crate::support::segment(vec![]),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[0]),
            crate::support::effect_binding(&[0]),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}

pub(super) fn list_case() -> super::WordControl {
    super::WordControl {
        def: 21,
        stack_in: vec![
            super::words::list_of(noble_kernel::types::Ty::I64),
            super::words::prog(vec![], vec![noble_kernel::types::Ty::Unit], &[]),
            super::words::prog(
                vec![
                    noble_kernel::types::Ty::I64,
                    super::words::list_of(noble_kernel::types::Ty::I64),
                ],
                vec![noble_kernel::types::Ty::Unit],
                &[],
            ),
        ],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[]),
            crate::support::effect_binding(&[]),
        ],
        stack_out: vec![noble_kernel::types::Ty::Unit],
        allowed: &[],
        reject_stack_in: vec![
            super::words::list_of(noble_kernel::types::Ty::I64),
            super::words::prog(vec![], vec![noble_kernel::types::Ty::Unit], &[]),
            super::words::prog(
                vec![
                    noble_kernel::types::Ty::I64,
                    super::words::list_of(noble_kernel::types::Ty::I64),
                ],
                vec![noble_kernel::types::Ty::I64],
                &[],
            ),
        ],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            crate::support::segment(vec![noble_kernel::types::Ty::Unit]),
            crate::support::effect_binding(&[]),
            crate::support::effect_binding(&[]),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}
