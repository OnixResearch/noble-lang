//! Independent interfaces for sum, boolean, and list branching.

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; sum_cases independently derives both payload-typed branch interfaces and their effect union, returning None for malformed bindings; the oracle's join checks decide validity."
)]
pub(super) fn sum_cases(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a, b, t, e, f) = (
        super::stack_at(inst, 0)?,
        super::value_at(inst, 1)?,
        super::value_at(inst, 2)?,
        super::stack_at(inst, 3)?,
        super::effects_at(inst, 4)?,
        super::effects_at(inst, 5)?,
    );
    let left = super::super::otypes::Type::Program(
        super::append(s.clone(), std::slice::from_ref(&a)),
        t.clone(),
        e.clone(),
    );
    let right = super::super::otypes::Type::Program(
        super::append(s.clone(), std::slice::from_ref(&b)),
        t.clone(),
        f.clone(),
    );
    Some(super::Interface {
        input: super::append(
            s,
            &[
                super::super::otypes::Type::Sum(std::boxed::Box::new(a), std::boxed::Box::new(b)),
                left,
                right,
            ],
        ),
        output: t,
        latent: super::super::otypes::union(&e, &f),
    })
}

pub(super) fn select(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, t, e, f) = (
        super::stack_at(inst, 0)?,
        super::stack_at(inst, 1)?,
        super::effects_at(inst, 2)?,
        super::effects_at(inst, 3)?,
    );
    let p1 = super::super::otypes::Type::Program(s.clone(), t.clone(), e.clone());
    let p2 = super::super::otypes::Type::Program(s.clone(), t.clone(), f.clone());
    Some(super::Interface {
        input: super::append(s, &[super::super::otypes::Type::Bool, p1, p2]),
        output: t,
        latent: super::super::otypes::union(&e, &f),
    })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; list_cases independently constructs nil/cons interfaces and the explicit effect union with fallible binding decoding; input claims must be rejected through Option rather than assertions."
)]
pub(super) fn list_cases(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a, t, e, f) = (
        super::stack_at(inst, 0)?,
        super::value_at(inst, 1)?,
        super::stack_at(inst, 2)?,
        super::effects_at(inst, 3)?,
        super::effects_at(inst, 4)?,
    );
    let empty_branch = super::super::otypes::Type::Program(s.clone(), t.clone(), e.clone());
    let cons_branch = super::super::otypes::Type::Program(
        super::append(
            s.clone(),
            &[
                a.clone(),
                super::super::otypes::Type::List(std::boxed::Box::new(a.clone())),
            ],
        ),
        t.clone(),
        f.clone(),
    );
    Some(super::Interface {
        input: super::append(
            s,
            &[
                super::super::otypes::Type::List(std::boxed::Box::new(super::value_at(inst, 1)?)),
                empty_branch,
                cons_branch,
            ],
        ),
        output: t,
        latent: super::super::otypes::union(&e, &f),
    })
}
