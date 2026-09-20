//! Independent interfaces for product, sum, and list construction.

pub(super) fn pair(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a, b) = (
        super::stack_at(inst, 0)?,
        super::value_at(inst, 1)?,
        super::value_at(inst, 2)?,
    );
    Some(super::Interface {
        input: super::append(s.clone(), &[a, b.clone()]),
        output: super::append(
            s,
            &[super::super::otypes::Type::Pair(
                std::boxed::Box::new(super::value_at(inst, 1)?),
                std::boxed::Box::new(b),
            )],
        ),
        latent: vec![],
    })
}

pub(super) fn unpair(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a, b) = (
        super::stack_at(inst, 0)?,
        super::value_at(inst, 1)?,
        super::value_at(inst, 2)?,
    );
    let joined = super::super::otypes::Type::Pair(
        std::boxed::Box::new(a.clone()),
        std::boxed::Box::new(b.clone()),
    );
    Some(super::Interface {
        input: super::append(s.clone(), &[joined]),
        output: super::append(s, &[a, b]),
        latent: vec![],
    })
}

pub(super) fn left(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a, b) = (
        super::stack_at(inst, 0)?,
        super::value_at(inst, 1)?,
        super::value_at(inst, 2)?,
    );
    Some(super::Interface {
        input: super::append(s.clone(), &[a]),
        output: super::append(
            s,
            &[super::super::otypes::Type::Sum(
                std::boxed::Box::new(super::value_at(inst, 1)?),
                std::boxed::Box::new(b),
            )],
        ),
        latent: vec![],
    })
}

pub(super) fn right(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a, b) = (
        super::stack_at(inst, 0)?,
        super::value_at(inst, 1)?,
        super::value_at(inst, 2)?,
    );
    Some(super::Interface {
        input: super::append(s.clone(), std::slice::from_ref(&b)),
        output: super::append(
            s,
            &[super::super::otypes::Type::Sum(
                std::boxed::Box::new(a),
                std::boxed::Box::new(b),
            )],
        ),
        latent: vec![],
    })
}

pub(super) fn empty_list(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a) = (super::stack_at(inst, 0)?, super::value_at(inst, 1)?);
    Some(super::Interface {
        input: s.clone(),
        output: super::append(
            s,
            &[super::super::otypes::Type::List(std::boxed::Box::new(a))],
        ),
        latent: vec![],
    })
}

pub(super) fn prepend(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a) = (super::stack_at(inst, 0)?, super::value_at(inst, 1)?);
    Some(super::Interface {
        input: super::append(
            s.clone(),
            &[
                a.clone(),
                super::super::otypes::Type::List(std::boxed::Box::new(super::value_at(inst, 1)?)),
            ],
        ),
        output: super::append(
            s,
            &[super::super::otypes::Type::List(std::boxed::Box::new(a))],
        ),
        latent: vec![],
    })
}
