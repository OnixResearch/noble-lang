pub(super) struct Output<'a> {
    pub(super) compiler: &'a mut super::super::super::Compiler,
    pub(super) layout: &'a mut super::super::Layout,
    pub(super) work: &'a mut super::super::super::Work,
    pub(super) env: &'a noble_kernel::contracts::Env,
    pub(super) interface: &'a noble_kernel::untrusted::Interface,
}

impl<'a> Output<'a> {
    pub(super) fn new_action(
        self,
        id: noble_kernel::types::NominalTypeId,
    ) -> Result<super::super::Action, crate::Diagnostic> {
        let Output {
            compiler,
            layout,
            work,
            env,
            interface,
        } = self;
        let decl = match env.nominal(id) {
            Some(decl) => decl,
            None => return Err(crate::Diagnostic::Invalid),
        };
        let payload = match &decl.shape {
            noble_kernel::types::NominalShape::Opaque(payload) => payload,
            noble_kernel::types::NominalShape::Variant(_, _) => {
                return Err(crate::Diagnostic::Invalid);
            }
        };
        let len = interface.stack_in.len();
        let top = match len.checked_sub(1) {
            Some(index) => index,
            None => return Err(crate::Diagnostic::Invalid),
        };
        if &interface.stack_in[top] != payload.as_ref() {
            return Err(crate::Diagnostic::Invalid);
        }
        let witness = attempt!(layout.types.single(payload, compiler, work));
        Ok(super::super::Action::NominalNew(
            id.module, id.ordinal, witness,
        ))
    }

    pub(super) fn bound_action(self, slot: u32) -> Result<super::super::Action, crate::Diagnostic> {
        let env = self.env;
        let mut index = 0;
        while index < env.bound_adapters.len() {
            if env.bound_adapters[index].adapter_slot == slot {
                return Ok(super::super::Action::EmitBound(slot));
            }
            index += 1;
        }
        Err(crate::Diagnostic::Invalid)
    }

    pub(super) fn clock_action(self, slot: u32) -> Result<super::super::Action, crate::Diagnostic> {
        for row in &self.env.bound_adapters {
            if row.adapter_slot == slot
                && row.input.is_empty()
                && row.output.as_slice() == [noble_kernel::types::Ty::I64]
                && row.effects.as_slice() == [noble_kernel::contracts::TEST_CLOCK] {
                return Ok(super::super::Action::ClockBound(slot));
            }
        }
        Err(crate::Diagnostic::Invalid)
    }

    pub(super) fn into_action(
        self,
        id: noble_kernel::types::NominalTypeId,
        input_index: usize,
    ) -> Result<super::super::Action, crate::Diagnostic> {
        let Output {
            compiler,
            layout,
            work,
            interface,
            ..
        } = self;
        let ty = &interface.stack_in[input_index];
        let witness = attempt!(layout.types.single(ty, compiler, work));
        Ok(super::super::Action::NominalInto(
            id.module, id.ordinal, witness,
        ))
    }

    pub(super) fn variant_action(
        self,
        is_left: bool,
        id: noble_kernel::types::NominalTypeId,
    ) -> Result<super::super::Action, crate::Diagnostic> {
        let env = self.env;
        let decl = match env.nominal(id) {
            Some(decl) => decl,
            None => return Err(crate::Diagnostic::Invalid),
        };
        self.variant_action_for_shape(is_left, id, &decl.shape)
    }

    pub(super) fn generic_variant_action(
        self,
        is_left: bool,
        id: noble_kernel::types::NominalTypeId,
    ) -> Result<super::super::Action, crate::Diagnostic> {
        let env = self.env;
        let interface = self.interface;
        let ty = match interface.stack_out.last() {
            Some(ty) if env.generic_instance_matches(ty, id) => ty,
            Some(_) | None => return Err(crate::Diagnostic::Invalid),
        };
        let noble_kernel::types::Ty::GenericNominal(_, _, shape) = ty else {
            return Err(crate::Diagnostic::Invalid);
        };
        self.variant_action_for_shape(is_left, id, shape)
    }

    fn variant_action_for_shape(
        self,
        is_left: bool,
        id: noble_kernel::types::NominalTypeId,
        shape: &noble_kernel::types::NominalShape,
    ) -> Result<super::super::Action, crate::Diagnostic> {
        let Output {
            compiler,
            layout,
            work,
            interface,
            ..
        } = self;
        let noble_kernel::types::NominalShape::Variant(left, right) = shape else {
            return Err(crate::Diagnostic::Invalid);
        };
        let len = interface.stack_in.len();
        let top = match len.checked_sub(1) {
            Some(index) => index,
            None => return Err(crate::Diagnostic::Invalid),
        };
        let payload = if is_left { left } else { right };
        if &interface.stack_in[top] != payload.as_ref() {
            return Err(crate::Diagnostic::Invalid);
        }
        let witness = attempt!(layout.types.single(payload, compiler, work));
        if is_left {
            Ok(super::super::Action::NominalLeft(
                id.module, id.ordinal, witness,
            ))
        } else {
            Ok(super::super::Action::NominalRight(
                id.module, id.ordinal, witness,
            ))
        }
    }

    pub(super) fn match_action(
        self,
        id: noble_kernel::types::NominalTypeId,
        input_index: usize,
    ) -> Result<super::super::Action, crate::Diagnostic> {
        let env = self.env;
        let decl = match env.nominal(id) {
            Some(decl) => decl,
            None => return Err(crate::Diagnostic::Invalid),
        };
        match &decl.shape {
            noble_kernel::types::NominalShape::Variant(_, _) => {}
            noble_kernel::types::NominalShape::Opaque(_) => {
                return Err(crate::Diagnostic::Invalid);
            }
        }
        self.checked_variant_match_action(id, input_index)
    }

    fn checked_variant_match_action(
        self,
        id: noble_kernel::types::NominalTypeId,
        input_index: usize,
    ) -> Result<super::super::Action, crate::Diagnostic> {
        let Output {
            compiler,
            layout,
            work,
            interface,
            ..
        } = self;
        let len = interface.stack_in.len();
        let left_index = match len.checked_sub(2) {
            Some(index) => index,
            None => return Err(crate::Diagnostic::Invalid),
        };
        let left_ty = &interface.stack_in[left_index];
        match left_ty {
            noble_kernel::types::Ty::Program(..) => {}
            _ => return Err(crate::Diagnostic::Invalid),
        }
        let left = attempt!(layout.types.single(left_ty, compiler, work));
        let right_index = match len.checked_sub(1) {
            Some(index) => index,
            None => return Err(crate::Diagnostic::Invalid),
        };
        let right_ty = &interface.stack_in[right_index];
        match right_ty {
            noble_kernel::types::Ty::Program(..) => {}
            _ => return Err(crate::Diagnostic::Invalid),
        };
        let right = attempt!(layout.types.single(right_ty, compiler, work));
        let nominal_ty = &interface.stack_in[input_index];
        let nominal = attempt!(layout.types.single(nominal_ty, compiler, work));
        Ok(super::super::Action::NominalMatch(
            id.module, id.ordinal, nominal, left, right,
        ))
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; pinned const trials fail on NominalTypeId equality, Env::nominal and Box<NominalShape>::as_ref (August E0015; March E0277 for nonconst traits); reassess when these operations become const-capable."
    )]
    fn matches_declared_nominal(
        env: &noble_kernel::contracts::Env,
        ty: &noble_kernel::types::Ty,
        id: noble_kernel::types::NominalTypeId,
    ) -> bool {
        let noble_kernel::types::Ty::Nominal(actual, shape) = ty else {
            return false;
        };
        if *actual != id {
            return false;
        }
        let decl = match env.nominal(id) {
            Some(decl) => decl,
            None => return false,
        };
        shape.as_ref() == &decl.shape
    }

    pub(super) fn checked_input(
        env: &noble_kernel::contracts::Env,
        interface: &noble_kernel::untrusted::Interface,
        id: noble_kernel::types::NominalTypeId,
        nth_from_top: usize,
    ) -> Result<usize, crate::Diagnostic> {
        Self::checked_input_kind(env, interface, id, nth_from_top, false)
    }

    fn checked_generic_input(
        env: &noble_kernel::contracts::Env,
        interface: &noble_kernel::untrusted::Interface,
        id: noble_kernel::types::NominalTypeId,
        nth_from_top: usize,
    ) -> Result<usize, crate::Diagnostic> {
        Self::checked_input_kind(env, interface, id, nth_from_top, true)
    }

    fn checked_input_kind(
        env: &noble_kernel::contracts::Env,
        interface: &noble_kernel::untrusted::Interface,
        id: noble_kernel::types::NominalTypeId,
        nth_from_top: usize,
        generic: bool,
    ) -> Result<usize, crate::Diagnostic> {
        let len = interface.stack_in.len();
        if nth_from_top == 0 {
            return Err(crate::Diagnostic::Invalid);
        }
        let index = match len.checked_sub(nth_from_top) {
            Some(index) => index,
            None => return Err(crate::Diagnostic::Invalid),
        };
        let ty = &interface.stack_in[index];
        let valid = if generic {
            env.generic_instance_matches(ty, id)
        } else {
            Self::matches_declared_nominal(env, ty, id)
        };
        if !valid {
            return Err(crate::Diagnostic::Invalid);
        }
        Ok(index)
    }

    pub(super) fn generic_match_action(
        self,
        id: noble_kernel::types::NominalTypeId,
    ) -> Result<super::super::Action, crate::Diagnostic> {
        let input_index = attempt!(Self::checked_generic_input(self.env, self.interface, id, 3));
        self.checked_variant_match_action(id, input_index)
    }
}
