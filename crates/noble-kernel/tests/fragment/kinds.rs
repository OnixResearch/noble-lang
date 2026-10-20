#[test]
fn stack_variables_reject_value_constructor_positions() {
    let stack = noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0));
    let family = noble_kernel::types::NominalTypeId {
        module: 1,
        ordinal: 0,
    };
    for pattern in [
        noble_kernel::shapes::Pattern::Pair(
            Box::new(stack.clone()),
            Box::new(noble_kernel::shapes::Pattern::I64),
        ),
        noble_kernel::shapes::Pattern::Pair(
            Box::new(noble_kernel::shapes::Pattern::I64),
            Box::new(stack.clone()),
        ),
        noble_kernel::shapes::Pattern::Sum(
            Box::new(stack.clone()),
            Box::new(noble_kernel::shapes::Pattern::I64),
        ),
        noble_kernel::shapes::Pattern::Sum(
            Box::new(noble_kernel::shapes::Pattern::I64),
            Box::new(stack.clone()),
        ),
        noble_kernel::shapes::Pattern::List(Box::new(stack.clone())),
        noble_kernel::shapes::Pattern::GenericNominal(
            family,
            Box::new([stack.clone(), noble_kernel::shapes::Pattern::I64]),
            [0, 1],
        ),
        noble_kernel::shapes::Pattern::GenericNominal(
            family,
            Box::new([noble_kernel::shapes::Pattern::I64, stack.clone()]),
            [0, 1],
        ),
        noble_kernel::shapes::Pattern::program(
            vec![noble_kernel::shapes::Pattern::List(Box::new(stack.clone()))],
            vec![stack.clone()],
            vec![],
        ),
        noble_kernel::shapes::Pattern::live_ref(
            vec![stack.clone()],
            vec![noble_kernel::shapes::Pattern::List(Box::new(stack.clone()))],
            vec![],
        ),
    ] {
        let mut scheme = noble_kernel::words::Scheme {
            var_kinds: vec![noble_kernel::words::VariableKind::Stack],
            stack_in: vec![stack.clone()],
            stack_out: vec![pattern],
            effects: vec![],
        };
        assert_eq!(
            scheme.validate(),
            Err(noble_kernel::shapes::Defect::KindMismatch),
            "output pattern: {:?}",
            scheme.stack_out,
        );
        core::mem::swap(&mut scheme.stack_in, &mut scheme.stack_out);
        assert_eq!(
            scheme.validate(),
            Err(noble_kernel::shapes::Defect::KindMismatch),
            "input pattern: {:?}",
            scheme.stack_in,
        );
    }
}

#[test]
fn nested_program_stacks_preserve_binding_cardinality() -> Result<(), String> {
    let stack = noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0));
    let value = noble_kernel::shapes::Pattern::Var(noble_kernel::words::Variable(1));
    let effect = noble_kernel::shapes::EffectSlot::Var(noble_kernel::words::Variable(2));
    let scheme = noble_kernel::words::Scheme {
        var_kinds: vec![
            noble_kernel::words::VariableKind::Stack,
            noble_kernel::words::VariableKind::Value,
            noble_kernel::words::VariableKind::Effect,
        ],
        stack_in: vec![stack.clone(), value.clone()],
        stack_out: vec![
            stack.clone(),
            noble_kernel::shapes::Pattern::Pair(
                Box::new(noble_kernel::shapes::Pattern::program(
                    vec![stack.clone(), value.clone()],
                    vec![stack.clone(), value.clone()],
                    vec![effect.clone()],
                )),
                Box::new(noble_kernel::shapes::Pattern::List(Box::new(value))),
            ),
        ],
        effects: vec![],
    };
    assert_eq!(scheme.validate(), Ok(()));
    for segment in [
        vec![],
        vec![noble_kernel::types::Ty::Text],
        vec![noble_kernel::types::Ty::Text, noble_kernel::types::Ty::Bool],
    ] {
        let effects = noble_kernel::types::EffSet::from_ids(&[noble_kernel::contracts::TEST_EMIT]);
        let inst = noble_kernel::words::Inst {
            bindings: vec![
                noble_kernel::words::Binding::Stack(segment.clone()),
                noble_kernel::words::Binding::Value(noble_kernel::types::Ty::I64),
                noble_kernel::words::Binding::Effect(effects.clone()),
            ],
        };
        assert_eq!(scheme.check_inst(&inst, 64, 256, 64), Ok(()));
        let mut program_stack = segment.clone();
        program_stack.push(noble_kernel::types::Ty::I64);
        assert_eq!(
            scheme
                .subst_stack(&scheme.stack_in, &inst)
                .map_err(|error| format!("{error:?}"))?,
            program_stack,
        );
        let mut expected = segment.clone();
        expected.push(noble_kernel::types::Ty::Pair(
            Box::new(noble_kernel::types::Ty::program(
                program_stack.clone(),
                program_stack,
                effects,
            )),
            Box::new(noble_kernel::types::Ty::List(Box::new(
                noble_kernel::types::Ty::I64,
            ))),
        ));
        assert_eq!(
            scheme
                .subst_stack(&scheme.stack_out, &inst)
                .map_err(|error| format!("{error:?}"))?,
            expected,
        );
    }
    Ok(())
}

#[test]
fn nested_live_reference_stacks_preserve_binding_cardinality() -> Result<(), String> {
    let stack = noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0));
    let scheme = noble_kernel::words::Scheme {
        var_kinds: vec![
            noble_kernel::words::VariableKind::Stack,
            noble_kernel::words::VariableKind::Effect,
        ],
        stack_in: vec![stack.clone()],
        stack_out: vec![noble_kernel::shapes::Pattern::Pair(
            Box::new(noble_kernel::shapes::Pattern::I64),
            Box::new(noble_kernel::shapes::Pattern::live_ref(
                vec![stack.clone()],
                vec![stack],
                vec![noble_kernel::shapes::EffectSlot::Var(
                    noble_kernel::words::Variable(1),
                )],
            )),
        )],
        effects: vec![],
    };
    assert_eq!(scheme.validate(), Ok(()));
    for segment in [
        vec![],
        vec![noble_kernel::types::Ty::Text],
        vec![noble_kernel::types::Ty::Text, noble_kernel::types::Ty::Bool],
    ] {
        let effects = noble_kernel::types::EffSet::from_ids(&[noble_kernel::contracts::TEST_EMIT]);
        let inst = noble_kernel::words::Inst {
            bindings: vec![
                noble_kernel::words::Binding::Stack(segment.clone()),
                noble_kernel::words::Binding::Effect(effects.clone()),
            ],
        };
        assert_eq!(scheme.check_inst(&inst, 64, 256, 64), Ok(()));
        assert_eq!(
            scheme
                .subst_stack(&scheme.stack_in, &inst)
                .map_err(|error| format!("{error:?}"))?,
            segment,
        );
        assert_eq!(
            scheme
                .subst_stack(&scheme.stack_out, &inst)
                .map_err(|error| format!("{error:?}"))?,
            vec![noble_kernel::types::Ty::Pair(
                Box::new(noble_kernel::types::Ty::I64),
                Box::new(noble_kernel::types::Ty::live_ref(
                    segment.clone(),
                    segment,
                    effects
                )),
            )],
        );
    }
    Ok(())
}
