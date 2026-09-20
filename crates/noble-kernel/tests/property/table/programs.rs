//! Independent interfaces for program construction and invocation.

pub(super) fn dip(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a, t, e) = (
        super::stack_at(inst, 0)?,
        super::value_at(inst, 1)?,
        super::stack_at(inst, 2)?,
        super::effects_at(inst, 3)?,
    );
    let p = super::super::otypes::Type::Program(s.clone(), t.clone(), e.clone());
    Some(super::Interface {
        input: super::append(s, &[a.clone(), p]),
        output: super::append(t, &[a]),
        latent: e,
    })
}

pub(super) fn quote(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (r, a, s) = (
        super::stack_at(inst, 0)?,
        super::value_at(inst, 1)?,
        super::stack_at(inst, 2)?,
    );
    let p = super::super::otypes::Type::Program(
        s.clone(),
        super::append(s.clone(), std::slice::from_ref(&a)),
        vec![],
    );
    Some(super::Interface {
        input: super::append(r.clone(), &[a]),
        output: super::append(r, &[p]),
        latent: vec![],
    })
}

pub(super) fn compose(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (r, a, b, c, e, f) = (
        super::stack_at(inst, 0)?,
        super::stack_at(inst, 1)?,
        super::stack_at(inst, 2)?,
        super::stack_at(inst, 3)?,
        super::effects_at(inst, 4)?,
        super::effects_at(inst, 5)?,
    );
    let p1 = super::super::otypes::Type::Program(a.clone(), b.clone(), e.clone());
    let p2 = super::super::otypes::Type::Program(b, c.clone(), f.clone());
    let p = super::super::otypes::Type::Program(a, c, super::super::otypes::union(&e, &f));
    Some(super::Interface {
        input: super::append(r, &[p1, p2]),
        output: super::append(super::stack_at(inst, 0)?, &[p]),
        latent: vec![],
    })
}

pub(super) fn apply(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, t, e) = (
        super::stack_at(inst, 0)?,
        super::stack_at(inst, 1)?,
        super::effects_at(inst, 2)?,
    );
    let p = super::super::otypes::Type::Program(s.clone(), t.clone(), e.clone());
    Some(super::Interface {
        input: super::append(s, &[p]),
        output: t,
        latent: e,
    })
}

pub(super) fn reflect(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (r, a, b, e) = (
        super::stack_at(inst, 0)?,
        super::stack_at(inst, 1)?,
        super::stack_at(inst, 2)?,
        super::effects_at(inst, 3)?,
    );
    let p = super::super::otypes::Type::Program(a, b, e);
    Some(super::Interface {
        input: super::append(r.clone(), &[p]),
        output: super::append(r, &[super::super::otypes::Type::Syntax]),
        latent: vec![],
    })
}
