//! Borrowed, bounded estimate of an owned calculator environment clone.
//!
//! Each node is counted at every occurrence: boxed nominal payloads and
//! program stacks cannot hide behind a small top-level scheme count.

use crate::{shapes::Pattern, types::{NominalShape, Ty}, words::Scheme};

/// Calculator source and compiler environments retain at most 896 definition
/// rows. This is independent of the reachable named-body bound.
pub const CALCULATOR_ENVIRONMENT_ROW_LIMIT: usize = 896;
/// One selected clone may reserve at most 16 MiB, including conservative
/// payload rounding and per-allocation overhead. The observed live ASCII
/// clone reserves under 9 MiB; this leaves room for the remaining closure
/// without admitting unbounded nested nominal and program payloads.
pub const CALCULATOR_ENV_CLONE_BYTES_LIMIT: u64 = 16 * 1024 * 1024;
/// Aggregate selected copies reserve at most one GiB, leaving the other half
/// of the compiler's two-billion-Work ceiling for actual checking and
/// lowering. The real 59-definition ASCII root reserves ~501 MiB.
pub const CALCULATOR_ENV_CLONE_FANOUT_LIMIT: u64 = 1024 * 1024 * 1024;
const DEPTH_LIMIT: usize = 64;
const NODE_LIMIT: usize = 512;

/// Payload bytes and allocator-rounded reservation for one complete clone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloneBudget {
    pub bytes: u64,
    pub allocations: u64,
    pub reserved_bytes: u64,
}

struct Counter {
    bytes: u64,
    allocations: u64,
}

impl Counter {
    fn bytes(&mut self, bytes: u64) -> Option<()> {
        self.bytes = self.bytes.checked_add(bytes)?;
        (self.bytes <= CALCULATOR_ENV_CLONE_BYTES_LIMIT).then_some(())
    }

    fn allocation(&mut self) -> Option<()> {
        self.allocations = self.allocations.checked_add(1)?;
        Some(())
    }

    fn vector<T>(&mut self, len: usize) -> Option<()> {
        self.bytes(u64::try_from(len).ok()?
            .checked_mul(u64::try_from(core::mem::size_of::<T>()).ok()?)?)?;
        if len != 0 { self.allocation()?; }
        Some(())
    }

    fn node<T>(&mut self) -> Option<()> {
        self.bytes(u64::try_from(core::mem::size_of::<T>()).ok()?)
    }

    fn boxed_pattern(&mut self, value: &Pattern, depth: usize, nodes: &mut usize) -> Option<()> {
        self.allocation()?;
        self.pattern(value, depth, nodes)
    }

    fn pattern(&mut self, value: &Pattern, depth: usize, nodes: &mut usize) -> Option<()> {
        if depth >= DEPTH_LIMIT || *nodes >= NODE_LIMIT { return None; }
        *nodes += 1;
        self.node::<Pattern>()?;
        match value {
            Pattern::Pair(left, right) | Pattern::Sum(left, right) => {
                self.boxed_pattern(left, depth + 1, nodes)?;
                self.boxed_pattern(right, depth + 1, nodes)?;
            }
            Pattern::List(item) => self.boxed_pattern(item, depth + 1, nodes)?,
            Pattern::Program(input, output, effects) => {
                if input.len() > 128 || output.len() > 128 || effects.len() > 8 { return None; }
                self.node::<alloc::vec::Vec<Pattern>>()?;
                self.node::<alloc::vec::Vec<Pattern>>()?;
                self.node::<alloc::vec::Vec<crate::shapes::EffectSlot>>()?;
                self.allocations = self.allocations.checked_add(3)?;
                if !input.is_empty() { self.allocation()?; }
                if !output.is_empty() { self.allocation()?; }
                self.vector::<crate::shapes::EffectSlot>(effects.len())?;
                for item in input.iter().chain(output.iter()) {
                    self.pattern(item, depth + 1, nodes)?;
                }
            }
            Pattern::Nominal(_, shape) => {
                self.allocation()?;
                self.node::<NominalShape>()?;
                self.nominal(shape, 0, &mut 0)?;
            }
            Pattern::GenericNominal(_, arguments, _) => {
                self.allocation()?;
                self.pattern(&arguments[0], depth + 1, nodes)?;
                self.pattern(&arguments[1], depth + 1, nodes)?;
            }
            // Calculator environments never admit borrowed live references.
            Pattern::LiveRef(..) => return None,
            Pattern::Unit | Pattern::Bool | Pattern::I64 | Pattern::Text
            | Pattern::Syntax | Pattern::Contract | Pattern::Evidence
            | Pattern::Certified | Pattern::Resource(_) | Pattern::Var(_)
            | Pattern::StackVar(_) => {}
        }
        Some(())
    }

    fn boxed_type(&mut self, value: &Ty, depth: usize, nodes: &mut usize) -> Option<()> {
        self.allocation()?;
        self.ty(value, depth, nodes)
    }

    fn nominal(&mut self, shape: &NominalShape, depth: usize, nodes: &mut usize) -> Option<()> {
        match shape {
            NominalShape::Opaque(value) => self.boxed_type(value, depth, nodes)?,
            NominalShape::Variant(left, right) => {
                self.boxed_type(left, depth, nodes)?;
                self.boxed_type(right, depth, nodes)?;
            }
        }
        Some(())
    }

    fn ty(&mut self, value: &Ty, depth: usize, nodes: &mut usize) -> Option<()> {
        if depth >= DEPTH_LIMIT || *nodes >= NODE_LIMIT { return None; }
        *nodes += 1;
        self.node::<Ty>()?;
        match value {
            Ty::Pair(left, right) | Ty::Sum(left, right) => {
                self.boxed_type(left, depth + 1, nodes)?;
                self.boxed_type(right, depth + 1, nodes)?;
            }
            Ty::List(item) => self.boxed_type(item, depth + 1, nodes)?,
            Ty::Nominal(_, shape) => {
                self.allocation()?;
                self.node::<NominalShape>()?;
                self.nominal(shape, depth + 1, nodes)?;
            }
            Ty::GenericNominal(_, arguments, shape) => {
                if matches!(shape.as_ref(), NominalShape::Opaque(_)) { return None; }
                self.allocation()?;
                self.ty(&arguments[0], depth + 1, nodes)?;
                self.ty(&arguments[1], depth + 1, nodes)?;
                self.allocation()?;
                self.node::<NominalShape>()?;
                self.nominal(shape, depth + 1, nodes)?;
            }
            Ty::Program(input, output, effects) => {
                if input.len() > 128 || output.len() > 128 || effects.as_slice().len() > 2 {
                    return None;
                }
                self.node::<alloc::vec::Vec<Ty>>()?;
                self.node::<alloc::vec::Vec<Ty>>()?;
                self.allocations = self.allocations.checked_add(2)?;
                if !input.is_empty() { self.allocation()?; }
                if !output.is_empty() { self.allocation()?; }
                self.vector::<crate::types::EffId>(effects.as_slice().len())?;
                for item in input.iter().chain(output.iter()) {
                    self.ty(item, depth + 1, nodes)?;
                }
            }
            Ty::LiveRef(..) => return None,
            Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Syntax
            | Ty::Contract | Ty::Evidence | Ty::Certified | Ty::Resource(_) => {}
        }
        Some(())
    }

    fn scheme(&mut self, scheme: &Scheme) -> Option<()> {
        if scheme.var_kinds.len() > 8 || scheme.effects.len() > 8
            || scheme.stack_in.len() > 128 || scheme.stack_out.len() > 128
        { return None; }
        self.vector::<crate::words::VariableKind>(scheme.var_kinds.len())?;
        self.vector::<crate::shapes::EffectSlot>(scheme.effects.len())?;
        if !scheme.stack_in.is_empty() { self.allocation()?; }
        if !scheme.stack_out.is_empty() { self.allocation()?; }
        for value in scheme.stack_in.iter().chain(scheme.stack_out.iter()) {
            self.pattern(value, 0, &mut 0)?;
        }
        Some(())
    }
}

impl super::Env {
    /// Count one clone without cloning input or allocating. Twice the
    /// requested payload plus 64 bytes per allocation is a conservative
    /// admission estimate for allocator rounding, not a claim to cap RSS
    /// from the entire process. Every addition fails closed before source
    /// or kernel owns another Env.
    pub fn calculator_clone_budget(&self) -> Option<CloneBudget> {
        if !self.calculator || !self.schemas.is_empty() || !self.bound_adapters.is_empty() {
            return None;
        }
        let mut cost = Counter { bytes: 0, allocations: 0 };
        cost.node::<super::Env>()?;
        cost.vector::<Scheme>(self.defs.len())?;
        cost.vector::<super::Behavior>(self.kinds.len())?;
        cost.vector::<alloc::vec::Vec<super::Definition>>(self.deps.len())?;
        cost.vector::<super::NominalDecl>(self.nominals.len())?;
        cost.vector::<super::GenericVariantDecl>(self.generic_variants.len())?;
        cost.vector::<crate::types::ResourceKind>(self.resource_kinds.len())?;
        cost.vector::<Option<u64>>(self.definition_owners.len())?;
        cost.vector::<crate::types::EffId>(self.effects.len())?;
        for row in &self.deps { cost.vector::<super::Definition>(row.len())?; }
        for scheme in &self.defs { cost.scheme(scheme)?; }
        for decl in &self.nominals { cost.nominal(&decl.shape, 0, &mut 0)?; }
        let reserved_bytes = cost.bytes.checked_mul(2)?
            .checked_add(cost.allocations.checked_mul(64)?)?;
        (reserved_bytes <= CALCULATOR_ENV_CLONE_BYTES_LIMIT).then_some(CloneBudget {
            bytes: cost.bytes, allocations: cost.allocations, reserved_bytes,
        })
    }
}
