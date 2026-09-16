//! Rank-1 word schemes, finite instantiation, and interface substitution.
//!
//! An instantiation maps every declared variable to one binding of the
//! matching kind; substituting a scheme's patterns yields the concrete
//! interface exactly. Substitution lives in `subst` and is one bounded,
//! iterative post-order walk.

pub mod subst;

/// Index of a variable inside one scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Variable(pub u32);

/// What a scheme variable stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VariableKind {
    /// A whole stack segment.
    Stack,
    /// One value type occupying one stack position.
    Value,
    /// An effect set.
    Effect,
}

/// A rank-1 scheme over the fixed variable kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scheme {
    /// Kinds of the scheme's variables, indexed by `Variable`.
    pub var_kinds: alloc::vec::Vec<VariableKind>,
    /// Required invocation stack pattern, bottom-first.
    pub stack_in: alloc::vec::Vec<crate::shapes::StackPart>,
    /// Result stack pattern, bottom-first.
    pub stack_out: alloc::vec::Vec<crate::shapes::StackPart>,
    /// Latent effect bound pattern.
    pub effects: alloc::vec::Vec<crate::shapes::EffectSlot>,
}

/// One variable's concrete value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Binding {
    /// A stack segment for a stack variable.
    Stack(alloc::vec::Vec<crate::types::Ty>),
    /// A type for a value variable.
    Value(crate::types::Ty),
    /// An effect set for an effect variable.
    Effect(crate::types::EffSet),
}

/// A concrete instantiation: one binding per declared variable, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inst {
    /// One binding per entry of `Scheme::var_kinds`.
    pub bindings: alloc::vec::Vec<Binding>,
}

/// Why an instantiation does not fit its scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstError {
    /// A binding's kind does not match its declared kind.
    KindMismatch,
    /// A variable index exceeds the declared variable count.
    UnknownVariable,
    /// The instantiation carries a wrong number of bindings.
    ArityMismatch,
    /// A bound stack exceeds the declared stack-height limit.
    OversizedStack,
    /// A bound type exceeds the declared type-size limit.
    OversizedType,
    /// A bound effect set exceeds the environment's effect-identity count.
    OversizedEffects,
}

fn slot(var: Variable) -> Option<usize> {
    usize::try_from(var.0).ok()
}

impl Inst {
    /// The stack bound to a stack variable, if the binding has that kind.
    pub fn stack(&self, var: Variable) -> Option<&[crate::types::Ty]> {
        match slot(var).and_then(|index| self.bindings.get(index)) {
            Some(Binding::Stack(segment)) => Some(segment.as_slice()),
            Some(Binding::Value(_)) | Some(Binding::Effect(_)) | None => None,
        }
    }

    /// The type bound to a value variable, if the binding has that kind.
    pub fn value(&self, var: Variable) -> Option<&crate::types::Ty> {
        match slot(var).and_then(|index| self.bindings.get(index)) {
            Some(Binding::Value(ty)) => Some(ty),
            Some(Binding::Stack(_)) | Some(Binding::Effect(_)) | None => None,
        }
    }

    /// The effect set bound to an effect variable, if the binding has that kind.
    pub fn effects(&self, var: Variable) -> Option<&crate::types::EffSet> {
        match slot(var).and_then(|index| self.bindings.get(index)) {
            Some(Binding::Effect(set)) => Some(set),
            Some(Binding::Stack(_)) | Some(Binding::Value(_)) | None => None,
        }
    }
}

impl Scheme {
    /// Validate that every variable use matches its declared kind.
    pub fn validate(&self) -> Result<(), crate::shapes::Defect> {
        crate::shapes::validate(
            &self.var_kinds,
            &self.stack_in,
            &self.stack_out,
            &self.effects,
        )
    }

    /// Check that an instantiation fits this scheme within the given bounds.
    pub fn check_inst(
        &self,
        inst: &Inst,
        max_stack: u32,
        max_type: u32,
        max_effects: u64,
    ) -> Result<(), InstError> {
        if inst.bindings.len() != self.var_kinds.len() {
            return Err(InstError::ArityMismatch);
        }
        for (binding, kind) in inst.bindings.iter().zip(self.var_kinds.iter()) {
            match (binding, kind) {
                (Binding::Stack(segment), VariableKind::Stack) => {
                    let height = match u64::try_from(segment.len()) {
                        Ok(height) => height,
                        Err(_) => return Err(InstError::OversizedStack),
                    };
                    if height > u64::from(max_stack) {
                        return Err(InstError::OversizedStack);
                    }
                    for ty in segment {
                        let size = ty.size().ok_or(InstError::OversizedType)?;
                        if size > max_type {
                            return Err(InstError::OversizedType);
                        }
                    }
                }
                (Binding::Value(ty), VariableKind::Value) => {
                    let size = ty.size().ok_or(InstError::OversizedType)?;
                    if size > max_type {
                        return Err(InstError::OversizedType);
                    }
                }
                (Binding::Effect(set), VariableKind::Effect) => {
                    let count = match set.len() {
                        Some(count) => count,
                        None => return Err(InstError::OversizedEffects),
                    };
                    if count > max_effects {
                        return Err(InstError::OversizedEffects);
                    }
                }
                (Binding::Stack(_), VariableKind::Value)
                | (Binding::Stack(_), VariableKind::Effect)
                | (Binding::Value(_), VariableKind::Stack)
                | (Binding::Value(_), VariableKind::Effect)
                | (Binding::Effect(_), VariableKind::Stack)
                | (Binding::Effect(_), VariableKind::Value) => return Err(InstError::KindMismatch),
            }
        }
        Ok(())
    }

    /// Substitute a stack pattern to a concrete stack.
    pub fn subst_stack(
        &self,
        parts: &[crate::shapes::StackPart],
        inst: &Inst,
    ) -> Result<alloc::vec::Vec<crate::types::Ty>, InstError> {
        let mut out: alloc::vec::Vec<crate::types::Ty> =
            alloc::vec::Vec::with_capacity(parts.len().max(4));
        for part in parts {
            match part {
                crate::shapes::StackPart::Pattern(pattern) => {
                    out.push(self.subst_pattern(pattern, inst)?)
                }
                crate::shapes::StackPart::Stack(var) => {
                    let segment = inst.stack(*var).ok_or(InstError::UnknownVariable)?;
                    out.extend_from_slice(segment);
                }
            }
        }
        Ok(out)
    }

    /// Substitute an effect pattern to a concrete set.
    pub fn subst_effects(
        &self,
        slots: &[crate::shapes::EffectSlot],
        inst: &Inst,
    ) -> Result<crate::types::EffSet, InstError> {
        let bound = slots.len().saturating_mul(4).saturating_add(4);
        let mut ids: alloc::vec::Vec<crate::types::EffId> = alloc::vec::Vec::with_capacity(bound);
        for slot in slots {
            match slot {
                crate::shapes::EffectSlot::Effect(id) => ids.push(*id),
                crate::shapes::EffectSlot::Var(var) => {
                    let set = inst.effects(*var).ok_or(InstError::UnknownVariable)?;
                    ids.extend_from_slice(set.as_slice());
                }
            }
        }
        Ok(crate::types::EffSet::from_ids(&ids))
    }
}
