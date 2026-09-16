//! Rank-1 word schemes, finite instantiation, and substitution.
//!
//! An instantiation maps every declared variable to one binding of the
//! matching kind; substituting a scheme's patterns yields the concrete
//! interface exactly. Substitution is one bounded, iterative post-order walk.

/// Local bound for one substitution walk; beyond it substitution rejects.
const WORK_CAP: usize = 512;

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

fn index(var: Variable) -> usize {
    usize::try_from(var.0).unwrap_or(usize::MAX)
}

impl Inst {
    /// The stack bound to a stack variable, if the binding has that kind.
    pub fn stack(&self, var: Variable) -> Option<&[crate::types::Ty]> {
        match self.bindings.get(index(var)) {
            Some(Binding::Stack(segment)) => Some(segment.as_slice()),
            _ => None,
        }
    }

    /// The type bound to a value variable, if the binding has that kind.
    pub fn value(&self, var: Variable) -> Option<&crate::types::Ty> {
        match self.bindings.get(index(var)) {
            Some(Binding::Value(ty)) => Some(ty),
            _ => None,
        }
    }

    /// The effect set bound to an effect variable, if the binding has that kind.
    pub fn effects(&self, var: Variable) -> Option<&crate::types::EffSet> {
        match self.bindings.get(index(var)) {
            Some(Binding::Effect(set)) => Some(set),
            _ => None,
        }
    }
}

/// One substitution step: a pattern, a constructor completion, a nested
/// signature expansion, or a pre-built segment emission.
enum Task<'a> {
    Part(&'a crate::shapes::Pattern),
    Finish(&'a crate::shapes::Pattern),
    Expand(&'a crate::shapes::Signature),
    Emit(alloc::vec::Vec<crate::types::Ty>),
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
                    if u64::try_from(segment.len()).unwrap_or(u64::MAX) > u64::from(max_stack) {
                        return Err(InstError::OversizedStack);
                    }
                    for ty in segment {
                        if ty.size() > max_type {
                            return Err(InstError::OversizedType);
                        }
                    }
                }
                (Binding::Value(ty), VariableKind::Value) => {
                    if ty.size() > max_type {
                        return Err(InstError::OversizedType);
                    }
                }
                (Binding::Effect(set), VariableKind::Effect) => {
                    if set.len() > max_effects {
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
        let mut total = slots.len();
        for slot in slots {
            if let crate::shapes::EffectSlot::Var(var) = slot {
                match inst.effects(*var) {
                    Some(set) => {
                        total =
                            total.saturating_add(usize::try_from(set.len()).unwrap_or(usize::MAX));
                    }
                    None => return Err(InstError::UnknownVariable),
                }
            }
        }
        let mut ids: alloc::vec::Vec<crate::types::EffId> =
            alloc::vec::Vec::with_capacity(total.max(4));
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

    /// Substitute one type pattern to a concrete type.
    pub fn subst_pattern(
        &self,
        pattern: &crate::shapes::Pattern,
        inst: &Inst,
    ) -> Result<crate::types::Ty, InstError> {
        let mut work: alloc::vec::Vec<Task> = alloc::vec::Vec::with_capacity(8);
        let mut segments: alloc::vec::Vec<alloc::vec::Vec<crate::types::Ty>> =
            alloc::vec::Vec::with_capacity(8);
        work.push(Task::Part(pattern));
        while let Some(task) = work.pop() {
            if work.len() >= WORK_CAP {
                return Err(InstError::OversizedType);
            }
            match task {
                Task::Emit(segment) => segments.push(segment),
                Task::Finish(node) => {
                    let right = segments.pop().ok_or(InstError::OversizedType)?;
                    let left = segments.pop().ok_or(InstError::OversizedType)?;
                    let built = match node {
                        crate::shapes::Pattern::Pair(_, _) => {
                            segments_from_pair(left, right, false)?
                        }
                        crate::shapes::Pattern::Sum(_, _) => segments_from_pair(left, right, true)?,
                        crate::shapes::Pattern::List(_) => {
                            let inner = single(left)?;
                            alloc::vec![crate::types::Ty::List(alloc::boxed::Box::new(inner))]
                        }
                        crate::shapes::Pattern::Var(_)
                        | crate::shapes::Pattern::Program(_)
                        | crate::shapes::Pattern::Unit
                        | crate::shapes::Pattern::Bool
                        | crate::shapes::Pattern::I64
                        | crate::shapes::Pattern::Text
                        | crate::shapes::Pattern::Syntax
                        | crate::shapes::Pattern::Resource(_) => {
                            return Err(InstError::KindMismatch)
                        }
                    };
                    segments.push(built);
                }
                Task::Expand(signature) => {
                    let count = signature.stack_in.len() + signature.stack_out.len();
                    let mut collected: alloc::vec::Vec<alloc::vec::Vec<crate::types::Ty>> =
                        alloc::vec::Vec::with_capacity(count.max(4));
                    for _ in 0..count {
                        collected.push(segments.pop().ok_or(InstError::OversizedType)?);
                    }
                    collected.reverse();
                    let mut parts = collected.into_iter();
                    let mut stack_in: alloc::vec::Vec<crate::types::Ty> =
                        alloc::vec::Vec::with_capacity(signature.stack_in.len().max(4));
                    for _ in 0..signature.stack_in.len() {
                        if let Some(segment) = parts.next() {
                            stack_in.extend(segment);
                        }
                    }
                    let mut stack_out: alloc::vec::Vec<crate::types::Ty> =
                        alloc::vec::Vec::with_capacity(signature.stack_out.len().max(4));
                    for _ in 0..signature.stack_out.len() {
                        if let Some(segment) = parts.next() {
                            stack_out.extend(segment);
                        }
                    }
                    let effects = self.subst_effects(&signature.effects, inst)?;
                    segments.push(alloc::vec![crate::types::Ty::program(
                        stack_in, stack_out, effects
                    )]);
                }
                Task::Part(node) => match node {
                    crate::shapes::Pattern::Unit => {
                        segments.push(alloc::vec![crate::types::Ty::Unit])
                    }
                    crate::shapes::Pattern::Bool => {
                        segments.push(alloc::vec![crate::types::Ty::Bool])
                    }
                    crate::shapes::Pattern::I64 => {
                        segments.push(alloc::vec![crate::types::Ty::I64])
                    }
                    crate::shapes::Pattern::Text => {
                        segments.push(alloc::vec![crate::types::Ty::Text])
                    }
                    crate::shapes::Pattern::Syntax => {
                        segments.push(alloc::vec![crate::types::Ty::Syntax])
                    }
                    crate::shapes::Pattern::Resource(kind) => {
                        segments.push(alloc::vec![crate::types::Ty::Resource(*kind)])
                    }
                    crate::shapes::Pattern::Var(var) => {
                        let ty = inst
                            .value(*var)
                            .cloned()
                            .ok_or(InstError::UnknownVariable)?;
                        segments.push(alloc::vec![ty]);
                    }
                    crate::shapes::Pattern::Pair(left, right)
                    | crate::shapes::Pattern::Sum(left, right) => {
                        work.push(Task::Finish(node));
                        work.push(Task::Part(left));
                        work.push(Task::Part(right));
                    }
                    crate::shapes::Pattern::List(item) => {
                        work.push(Task::Finish(node));
                        work.push(Task::Part(item));
                    }
                    crate::shapes::Pattern::Program(signature) => {
                        work.push(Task::Expand(signature));
                        for part in signature
                            .stack_in
                            .iter()
                            .chain(signature.stack_out.iter())
                            .rev()
                        {
                            match part {
                                crate::shapes::StackPart::Pattern(item) => {
                                    work.push(Task::Part(item))
                                }
                                crate::shapes::StackPart::Stack(var) => {
                                    let segment =
                                        inst.stack(*var).ok_or(InstError::UnknownVariable)?;
                                    work.push(Task::Emit(segment.to_vec()));
                                }
                            }
                        }
                    }
                },
            }
        }
        let mut out = segments.pop().ok_or(InstError::OversizedType)?;
        if segments.is_empty() && out.len() == 1 {
            match out.pop() {
                Some(ty) => Ok(ty),
                None => Err(InstError::OversizedType),
            }
        } else {
            Err(InstError::OversizedType)
        }
    }
}

fn single(segment: alloc::vec::Vec<crate::types::Ty>) -> Result<crate::types::Ty, InstError> {
    let mut items = segment;
    if items.len() == 1 {
        match items.pop() {
            Some(ty) => Ok(ty),
            None => Err(InstError::OversizedType),
        }
    } else {
        Err(InstError::OversizedType)
    }
}

fn segments_from_pair(
    left: alloc::vec::Vec<crate::types::Ty>,
    right: alloc::vec::Vec<crate::types::Ty>,
    is_sum: bool,
) -> Result<alloc::vec::Vec<crate::types::Ty>, InstError> {
    let left_ty = single(left)?;
    let right_ty = single(right)?;
    let built = if is_sum {
        crate::types::Ty::Sum(
            alloc::boxed::Box::new(left_ty),
            alloc::boxed::Box::new(right_ty),
        )
    } else {
        crate::types::Ty::Pair(
            alloc::boxed::Box::new(left_ty),
            alloc::boxed::Box::new(right_ty),
        )
    };
    Ok(alloc::vec![built])
}
