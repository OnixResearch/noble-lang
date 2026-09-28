//! Canonical typed schemes for declared nominal operations.

macro_rules! present {
    ($candidate:expr) => {
        match $candidate {
            Some(value) => value,
            None => return None,
        }
    };
}

fn stack(index: u32) -> crate::shapes::Pattern {
    crate::shapes::Pattern::StackVar(crate::words::Variable(index))
}

fn declared_pattern(decl: &super::super::NominalDecl) -> crate::shapes::Pattern {
    crate::shapes::Pattern::Nominal(decl.id, alloc::boxed::Box::new(decl.shape.clone()))
}

pub(super) fn conversion(
    decl: &super::super::NominalDecl,
    into: bool,
) -> Option<crate::words::Scheme> {
    let crate::types::NominalShape::Opaque(ty) = &decl.shape else {
        return None;
    };
    let representation = present!(super::payload::pattern(ty));
    if into {
        return Some(conversion_scheme(declared_pattern(decl), representation));
    }
    Some(conversion_scheme(representation, declared_pattern(decl)))
}

fn conversion_scheme(
    input: crate::shapes::Pattern,
    output: crate::shapes::Pattern,
) -> crate::words::Scheme {
    crate::words::Scheme {
        var_kinds: alloc::vec![crate::words::VariableKind::Stack],
        stack_in: alloc::vec![stack(0), input],
        stack_out: alloc::vec![stack(0), output],
        effects: alloc::vec![],
    }
}

pub(super) fn arm(decl: &super::super::NominalDecl, right: bool) -> Option<crate::words::Scheme> {
    let crate::types::NominalShape::Variant(left, other) = &decl.shape else {
        return None;
    };
    let representation = if right {
        super::payload::pattern(other)
    } else {
        super::payload::pattern(left)
    };
    let representation = present!(representation);
    Some(crate::words::Scheme {
        var_kinds: alloc::vec![crate::words::VariableKind::Stack],
        stack_in: alloc::vec![stack(0), representation],
        stack_out: alloc::vec![stack(0), declared_pattern(decl)],
        effects: alloc::vec![],
    })
}

pub(super) fn matcher(decl: &super::super::NominalDecl) -> Option<crate::words::Scheme> {
    let crate::types::NominalShape::Variant(left, right) = &decl.shape else {
        return None;
    };
    let left = present!(super::payload::pattern(left));
    let right = present!(super::payload::pattern(right));
    Some(crate::words::Scheme {
        var_kinds: match_variables(),
        stack_in: match_input(decl, left, right),
        stack_out: alloc::vec![stack(1)],
        effects: alloc::vec![
            crate::shapes::EffectSlot::Var(crate::words::Variable(2)),
            crate::shapes::EffectSlot::Var(crate::words::Variable(3)),
        ],
    })
}

fn match_variables() -> alloc::vec::Vec<crate::words::VariableKind> {
    alloc::vec![
        crate::words::VariableKind::Stack,
        crate::words::VariableKind::Stack,
        crate::words::VariableKind::Effect,
        crate::words::VariableKind::Effect,
    ]
}

fn match_input(
    decl: &super::super::NominalDecl,
    left: crate::shapes::Pattern,
    right: crate::shapes::Pattern,
) -> alloc::vec::Vec<crate::shapes::Pattern> {
    alloc::vec![
        stack(0),
        declared_pattern(decl),
        match_handler(left, 2),
        match_handler(right, 3),
    ]
}

fn match_handler(arm: crate::shapes::Pattern, effect: u32) -> crate::shapes::Pattern {
    crate::shapes::Pattern::program(
        alloc::vec![stack(0), arm],
        alloc::vec![stack(1)],
        alloc::vec![crate::shapes::EffectSlot::Var(crate::words::Variable(
            effect
        ))],
    )
}

pub(super) fn emit_scheme() -> crate::words::Scheme {
    crate::words::Scheme {
        var_kinds: alloc::vec![crate::words::VariableKind::Stack],
        stack_in: alloc::vec![stack(0), crate::shapes::Pattern::Text],
        stack_out: alloc::vec![stack(0)],
        effects: alloc::vec![crate::shapes::EffectSlot::Effect(super::super::TEST_EMIT)],
    }
}

pub(super) fn valid_emit_scheme(actual: &crate::words::Scheme) -> bool {
    actual.var_kinds.as_slice() == [crate::words::VariableKind::Stack]
        && actual.stack_in.as_slice() == [stack(0), crate::shapes::Pattern::Text]
        && actual.stack_out.as_slice() == [stack(0)]
        && actual.effects.as_slice() == [crate::shapes::EffectSlot::Effect(super::super::TEST_EMIT)]
}

pub(super) fn same_scheme(actual: &crate::words::Scheme, expected: &crate::words::Scheme) -> bool {
    actual.var_kinds == expected.var_kinds
        && actual.stack_in == expected.stack_in
        && actual.stack_out == expected.stack_out
        && actual.effects == expected.effects
}

pub(super) fn expected_scheme(
    decl: &super::super::NominalDecl,
    kind: super::super::Behavior,
) -> Option<crate::words::Scheme> {
    if let super::super::Behavior::NominalNew(id) = kind {
        if decl.id == id && matches!(decl.shape, crate::types::NominalShape::Opaque(_)) {
            conversion(decl, false)
        } else {
            None
        }
    } else if let super::super::Behavior::NominalInto(id) = kind {
        if decl.id == id && matches!(decl.shape, crate::types::NominalShape::Opaque(_)) {
            conversion(decl, true)
        } else {
            None
        }
    } else if let super::super::Behavior::NominalLeft(id) = kind {
        if decl.id == id && matches!(decl.shape, crate::types::NominalShape::Variant(_, _)) {
            arm(decl, false)
        } else {
            None
        }
    } else if let super::super::Behavior::NominalRight(id) = kind {
        if decl.id == id && matches!(decl.shape, crate::types::NominalShape::Variant(_, _)) {
            arm(decl, true)
        } else {
            None
        }
    } else if let super::super::Behavior::NominalMatch(id) = kind {
        if decl.id == id && matches!(decl.shape, crate::types::NominalShape::Variant(_, _)) {
            matcher(decl)
        } else {
            None
        }
    } else {
        // Other behaviors, including Named and BoundEmit, have no nominal scheme.
        None
    }
}
