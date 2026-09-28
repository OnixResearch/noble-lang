//! Ownership-preserving transitions of a bounded type traversal.

/// Pending nodes and charged nodes travel together through every step.
pub(super) struct State<'a> {
    pub(super) pending: alloc::vec::Vec<&'a crate::types::Ty>,
    pub(super) visited: u32,
}

impl<'a> State<'a> {
    pub(super) fn for_descriptor(shape: &'a crate::types::NominalShape) -> Option<Self> {
        Self {
            pending: alloc::vec::Vec::with_capacity(4),
            visited: 0,
        }
        .with_descriptor(shape)
    }

    pub(super) fn with_type(mut self, ty: &'a crate::types::Ty) -> Self {
        self.pending.push(ty);
        self
    }

    pub(super) fn with_pair(
        mut self,
        left: &'a crate::types::Ty,
        right: &'a crate::types::Ty,
    ) -> Self {
        self.pending.push(left);
        self.pending.push(right);
        self
    }

    pub(super) fn with_program(
        mut self,
        input: &'a [crate::types::Ty],
        output: &'a [crate::types::Ty],
    ) -> Self {
        self.pending
            .reserve(input.len().saturating_add(output.len()));
        self.pending.extend(input.iter());
        self.pending.extend(output.iter());
        self
    }

    pub(super) fn with_descriptor(self, shape: &'a crate::types::NominalShape) -> Option<Self> {
        if let crate::types::NominalShape::Opaque(ty) = shape {
            return Some(self.with_type(ty));
        }
        if let crate::types::NominalShape::Variant(left, right) = shape {
            return Some(self.with_pair(left, right));
        }
        None
    }
}
