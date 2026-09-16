//! Rank-1 word schemes and their finite instantiation.
//!
//! A scheme quantifies over stack variables, value-type variables, and effect
//! variables. Scheme stacks hold *patterns*: concrete types, whole-stack
//! variables, or type patterns such as `Program<S, T, e>` that mention
//! variables. An instantiation maps every declared variable to one binding of
//! the matching kind; applying it yields the node's concrete interface exactly.

use crate::types::{EffId, EffSet, Ty};
use alloc::boxed::Box;
use alloc::vec::Vec;

/// Index of a variable inside one scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VarId(pub u32);

/// What a scheme variable stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VarKind {
    /// A whole stack segment.
    Stack,
    /// One value type occupying one stack position.
    Value,
    /// An effect set.
    Effect,
}

/// A type pattern: a concrete constructor tree that may mention value
/// variables at any leaf or payload position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PTy {
    /// The unit type.
    Unit,
    /// The boolean type.
    Bool,
    /// The wrapping 64-bit integer type.
    I64,
    /// The text type.
    Text,
    /// Inert syntax.
    Syntax,
    /// A product pattern.
    Pair(Box<PTy>, Box<PTy>),
    /// A sum pattern.
    Sum(Box<PTy>, Box<PTy>),
    /// A list pattern.
    List(Box<PTy>),
    /// A program pattern with its own stack and effect patterns.
    Program(Box<PSig>),
    /// An opaque resource kind.
    Resource(crate::types::ResourceKind),
    /// A value-type variable.
    Var(VarId),
}

/// A program pattern inside a scheme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PSig {
    /// Required stack pattern, bottom-first.
    pub stack_in: Vec<PItem>,
    /// Result stack pattern, bottom-first.
    pub stack_out: Vec<PItem>,
    /// Latent effect pattern.
    pub effects: Vec<EffSlot>,
}

/// One stack position in a pattern stack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PItem {
    /// A type pattern occupying one position.
    Ty(PTy),
    /// A whole-stack variable standing for zero or more positions.
    Stack(VarId),
}

/// One effect position in a scheme bound.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffSlot {
    /// A concrete effect identity.
    Id(EffId),
    /// An effect-set variable.
    Var(VarId),
}

/// A rank-1 scheme over the fixed variable kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scheme {
    /// Kinds of the scheme's variables, indexed by `VarId`.
    pub var_kinds: Vec<VarKind>,
    /// Required invocation stack pattern, bottom-first.
    pub stack_in: Vec<PItem>,
    /// Result stack pattern, bottom-first.
    pub stack_out: Vec<PItem>,
    /// Latent effect bound pattern.
    pub effects: Vec<EffSlot>,
}

/// Why a scheme itself is malformed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemeError {
    /// A variable index exceeds the declared variable count.
    UnknownVariable,
    /// A variable is used in a position that does not match its kind.
    KindMismatch,
}

/// Why an instantiation does not fit its scheme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstError {
    /// A variable's binding kind does not match its declared kind.
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

/// One variable's concrete value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Binding {
    /// A stack segment for a stack variable.
    Stack(Vec<Ty>),
    /// A type for a value variable.
    Value(Ty),
    /// An effect set for an effect variable.
    Effect(EffSet),
}

/// A concrete instantiation: exactly one binding per declared variable, in
/// variable order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inst {
    /// One binding per entry of `Scheme::var_kinds`.
    pub bindings: Vec<Binding>,
}

impl Inst {
    /// The stack bound to a stack variable, if the binding has that kind.
    pub fn stack(&self, var: VarId) -> Option<&[Ty]> {
        match self.bindings.get(var.0 as usize) {
            Some(Binding::Stack(stack)) => Some(stack.as_slice()),
            _ => None,
        }
    }

    /// The type bound to a value variable, if the binding has that kind.
    pub fn value(&self, var: VarId) -> Option<&Ty> {
        match self.bindings.get(var.0 as usize) {
            Some(Binding::Value(ty)) => Some(ty),
            _ => None,
        }
    }

    /// The effect set bound to an effect variable, if the binding has that kind.
    pub fn effects(&self, var: VarId) -> Option<&EffSet> {
        match self.bindings.get(var.0 as usize) {
            Some(Binding::Effect(set)) => Some(set),
            _ => None,
        }
    }
}

impl Scheme {
    fn var_kind(&self, var: VarId) -> Result<VarKind, SchemeError> {
        self.var_kinds
            .get(var.0 as usize)
            .copied()
            .ok_or(SchemeError::UnknownVariable)
    }

    fn check_pty(&self, pty: &PTy) -> Result<(), SchemeError> {
        match pty {
            PTy::Var(var) => match self.var_kind(*var)? {
                VarKind::Value => Ok(()),
                _ => Err(SchemeError::KindMismatch),
            },
            PTy::Pair(a, b) | PTy::Sum(a, b) => {
                self.check_pty(a)?;
                self.check_pty(b)
            }
            PTy::List(item) => self.check_pty(item),
            PTy::Program(sig) => {
                self.check_stack(&sig.stack_in)?;
                self.check_stack(&sig.stack_out)?;
                self.check_effects(&sig.effects)
            }
            PTy::Unit | PTy::Bool | PTy::I64 | PTy::Text | PTy::Syntax | PTy::Resource(_) => Ok(()),
        }
    }

    fn check_stack(&self, stack: &[PItem]) -> Result<(), SchemeError> {
        for item in stack {
            match item {
                PItem::Ty(pty) => self.check_pty(pty)?,
                PItem::Stack(var) => match self.var_kind(*var)? {
                    VarKind::Stack => {}
                    _ => return Err(SchemeError::KindMismatch),
                },
            }
        }
        Ok(())
    }

    fn check_effects(&self, slots: &[EffSlot]) -> Result<(), SchemeError> {
        for slot in slots {
            if let EffSlot::Var(var) = slot {
                match self.var_kind(*var)? {
                    VarKind::Effect => {}
                    _ => return Err(SchemeError::KindMismatch),
                }
            }
        }
        Ok(())
    }

    /// Validate that every variable use matches its declared kind.
    pub fn validate(&self) -> Result<(), SchemeError> {
        self.check_stack(&self.stack_in)?;
        self.check_stack(&self.stack_out)?;
        self.check_effects(&self.effects)
    }

    /// Check that an instantiation is well-formed for this scheme, bounded by a
    /// maximum stack height, type size, and effect-set size.
    pub fn check_inst(
        &self,
        inst: &Inst,
        max_stack: u32,
        max_type: u32,
        max_effects: u32,
    ) -> Result<(), InstError> {
        if inst.bindings.len() != self.var_kinds.len() {
            return Err(InstError::ArityMismatch);
        }
        for (binding, kind) in inst.bindings.iter().zip(self.var_kinds.iter()) {
            match (binding, kind) {
                (Binding::Stack(stack), VarKind::Stack) => {
                    if stack.len() as u64 > u64::from(max_stack) {
                        return Err(InstError::OversizedStack);
                    }
                    for ty in stack {
                        if ty.size() > max_type {
                            return Err(InstError::OversizedType);
                        }
                    }
                }
                (Binding::Value(ty), VarKind::Value) => {
                    if ty.size() > max_type {
                        return Err(InstError::OversizedType);
                    }
                }
                (Binding::Effect(set), VarKind::Effect) => {
                    if set.len() as u64 > u64::from(max_effects) {
                        return Err(InstError::OversizedEffects);
                    }
                }
                _ => return Err(InstError::KindMismatch),
            }
        }
        Ok(())
    }

    /// Substitute a type pattern to a concrete type.
    pub fn subst_pty(&self, pty: &PTy, inst: &Inst) -> Result<Ty, InstError> {
        match pty {
            PTy::Unit => Ok(Ty::Unit),
            PTy::Bool => Ok(Ty::Bool),
            PTy::I64 => Ok(Ty::I64),
            PTy::Text => Ok(Ty::Text),
            PTy::Syntax => Ok(Ty::Syntax),
            PTy::Resource(kind) => Ok(Ty::Resource(*kind)),
            PTy::Var(var) => inst.value(*var).cloned().ok_or(InstError::UnknownVariable),
            PTy::Pair(a, b) => Ok(Ty::Pair(
                Box::new(self.subst_pty(a, inst)?),
                Box::new(self.subst_pty(b, inst)?),
            )),
            PTy::Sum(a, b) => Ok(Ty::Sum(
                Box::new(self.subst_pty(a, inst)?),
                Box::new(self.subst_pty(b, inst)?),
            )),
            PTy::List(item) => Ok(Ty::List(Box::new(self.subst_pty(item, inst)?))),
            PTy::Program(sig) => Ok(Ty::program(
                self.subst_stack(&sig.stack_in, inst)?,
                self.subst_stack(&sig.stack_out, inst)?,
                self.subst_effects(&sig.effects, inst)?,
            )),
        }
    }

    /// Substitute a pattern stack to a concrete stack.
    pub fn subst_stack(&self, stack: &[PItem], inst: &Inst) -> Result<Vec<Ty>, InstError> {
        let mut out = Vec::new();
        for item in stack {
            match item {
                PItem::Ty(pty) => out.push(self.subst_pty(pty, inst)?),
                PItem::Stack(var) => {
                    let segment = inst.stack(*var).ok_or(InstError::UnknownVariable)?;
                    out.extend_from_slice(segment);
                }
            }
        }
        Ok(out)
    }

    /// Substitute an effect pattern to a concrete set.
    pub fn subst_effects(&self, slots: &[EffSlot], inst: &Inst) -> Result<EffSet, InstError> {
        let mut ids: Vec<EffId> = Vec::new();
        for slot in slots {
            match slot {
                EffSlot::Id(id) => ids.push(*id),
                EffSlot::Var(var) => {
                    let set = inst.effects(*var).ok_or(InstError::UnknownVariable)?;
                    ids.extend_from_slice(set.as_slice());
                }
            }
        }
        Ok(EffSet::from_ids(&ids))
    }
}
