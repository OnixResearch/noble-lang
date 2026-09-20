pub(super) fn unit() -> super::WordControl {
    super::WordControl {
        def: 12,
        stack_in: vec![],
        positive: vec![crate::support::segment(vec![])],
        stack_out: vec![noble_kernel::types::Ty::Unit],
        allowed: &[],
        reject_stack_in: vec![noble_kernel::types::Ty::Unit],
        reject: vec![crate::support::segment(vec![noble_kernel::types::Ty::Unit])],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}

pub(super) fn pair() -> super::WordControl {
    super::WordControl {
        def: 13,
        stack_in: vec![noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
        stack_out: vec![super::words::pair_of(
            noble_kernel::types::Ty::I64,
            noble_kernel::types::Ty::Bool,
        )],
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

pub(super) fn unpair() -> super::WordControl {
    super::WordControl {
        def: 14,
        stack_in: vec![super::words::pair_of(
            noble_kernel::types::Ty::I64,
            noble_kernel::types::Ty::Bool,
        )],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
        stack_out: vec![noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool],
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

pub(super) fn left() -> super::WordControl {
    super::WordControl {
        def: 15,
        stack_in: vec![noble_kernel::types::Ty::I64],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
        stack_out: vec![super::words::sum_of(
            noble_kernel::types::Ty::I64,
            noble_kernel::types::Ty::Bool,
        )],
        allowed: &[],
        reject_stack_in: vec![noble_kernel::types::Ty::Text],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}

pub(super) fn right() -> super::WordControl {
    super::WordControl {
        def: 16,
        stack_in: vec![noble_kernel::types::Ty::Bool],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
        stack_out: vec![super::words::sum_of(
            noble_kernel::types::Ty::I64,
            noble_kernel::types::Ty::Bool,
        )],
        allowed: &[],
        reject_stack_in: vec![noble_kernel::types::Ty::Text],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Bool),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}

pub(super) fn nil() -> super::WordControl {
    super::WordControl {
        def: 19,
        stack_in: vec![],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
        ],
        stack_out: vec![super::words::list_of(noble_kernel::types::Ty::I64)],
        allowed: &[],
        reject_stack_in: vec![],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
        ],
        constraint: noble_kernel::untrusted::Constraint::InstantiationArity,
    }
}

pub(super) fn cons() -> super::WordControl {
    super::WordControl {
        def: 20,
        stack_in: vec![
            noble_kernel::types::Ty::I64,
            super::words::list_of(noble_kernel::types::Ty::I64),
        ],
        positive: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
        ],
        stack_out: vec![super::words::list_of(noble_kernel::types::Ty::I64)],
        allowed: &[],
        reject_stack_in: vec![
            noble_kernel::types::Ty::I64,
            super::words::list_of(noble_kernel::types::Ty::Bool),
        ],
        reject: vec![
            crate::support::segment(vec![]),
            noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
        ],
        constraint: noble_kernel::untrusted::Constraint::StackJoin,
    }
}
