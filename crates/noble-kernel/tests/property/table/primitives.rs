//! Independent interfaces for primitive stack and scalar words.

pub(super) fn duplicate(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a) = (super::stack_at(inst, 0)?, super::value_at(inst, 1)?);
    Some(super::Interface {
        input: super::append(s.clone(), std::slice::from_ref(&a)),
        output: super::append(s, &[a.clone(), a]),
        latent: vec![],
    })
}

pub(super) fn discard(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a) = (super::stack_at(inst, 0)?, super::value_at(inst, 1)?);
    Some(super::Interface {
        input: super::append(s.clone(), &[a]),
        output: s,
        latent: vec![],
    })
}

pub(super) fn swap(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let (s, a, b) = (
        super::stack_at(inst, 0)?,
        super::value_at(inst, 1)?,
        super::value_at(inst, 2)?,
    );
    Some(super::Interface {
        input: super::append(s.clone(), &[a, b.clone()]),
        output: super::append(s, &[b, super::value_at(inst, 1)?]),
        latent: vec![],
    })
}

pub(super) fn integer_result(
    inst: &noble_kernel::words::Inst,
    output: super::super::otypes::Type,
) -> Option<super::Interface> {
    let s = super::stack_at(inst, 0)?;
    Some(super::Interface {
        input: super::append(
            s.clone(),
            &[
                super::super::otypes::Type::I64,
                super::super::otypes::Type::I64,
            ],
        ),
        output: super::append(s, &[output]),
        latent: vec![],
    })
}

pub(super) fn unit(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let s = super::stack_at(inst, 0)?;
    Some(super::Interface {
        input: s.clone(),
        output: super::append(s, &[super::super::otypes::Type::Unit]),
        latent: vec![],
    })
}

pub(super) fn print(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let s = super::stack_at(inst, 0)?;
    Some(super::Interface {
        input: super::append(s.clone(), &[super::super::otypes::Type::Text]),
        output: super::append(s, &[super::super::otypes::Type::Unit]),
        latent: vec![0],
    })
}

pub(super) fn resource(inst: &noble_kernel::words::Inst) -> Option<super::Interface> {
    let s = super::stack_at(inst, 0)?;
    Some(super::Interface {
        input: s.clone(),
        output: super::append(s, &[super::super::otypes::Type::Resource]),
        latent: vec![],
    })
}
