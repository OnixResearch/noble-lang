/// The allocation state against which a preparation was checked. The owned
/// prospective compiler retains its immutable base prefixes, so provenance
/// requires only these scalar positions and no second whole-state clone.
pub(super) struct Base {
    live: bool,
    text_cursor: bool,
    calculator: bool,
    generation: u32,
    functions: u32,
    text_end_bytes: u32,
    signature_count: usize,
    descriptor_count: usize,
    identity_count: usize,
    /// Content-addressed calculator literals only grow, like `text_end`.
    static_literal_count: usize,
    /// The committed identity prefix the preparation retained: all of it, or
    /// none at a calculator quiescent boundary.
    identity_base: usize,
    /// The committed signature-pool and descriptor prefixes the preparation
    /// retained: all of each, or none at a calculator Program-free boundary.
    signature_base: usize,
    descriptor_base: usize,
}

impl Base {
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; the content-addressed calculator literal table is a BTreeMap whose len is not const-stable."
    )]
    pub(super) fn capture(
        compiler: &super::Compiler,
        identity_base: usize,
        signature_base: usize,
        descriptor_base: usize,
    ) -> Self {
        Self {
            live: compiler.live,
            text_cursor: compiler.text_cursor,
            calculator: compiler.calculator,
            generation: compiler.generation,
            functions: compiler.functions,
            text_end_bytes: compiler.text_end,
            signature_count: compiler.signatures.len(),
            descriptor_count: compiler.descriptors.len(),
            identity_count: compiler.identities.len(),
            static_literal_count: compiler.static_literals.len(),
            identity_base,
            signature_base,
            descriptor_base,
        }
    }

    pub(super) const fn generation(&self) -> u32 {
        self.generation
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; comparing the content-addressed calculator literal table size uses the non-const BTreeMap::len."
    )]
    fn positions_match(&self, current: &super::Compiler, next: &super::Compiler) -> bool {
        // The descriptor table has exactly one entry for every emitted
        // signature, so its transactional base shares the profile key bound.
        let key_limit = if self.calculator {
            crate::signatures::CALCULATOR_SOURCE_KEY_LIMIT
        } else {
            crate::signatures::SOURCE_KEY_LIMIT
        };
        self.live == current.live
            && self.live == next.live
            && self.text_cursor == current.text_cursor
            && self.text_cursor == next.text_cursor
            && self.calculator == current.calculator
            && self.calculator == next.calculator
            && self.generation == current.generation
            && self.functions == current.functions
            && self.text_end_bytes == current.text_end
            && self.static_literal_count == current.static_literals.len()
            && self.static_literal_count <= next.static_literals.len()
            && self.signature_count == current.signatures.len()
            && self.descriptor_count == current.descriptors.len()
            && self.identity_count == current.identities.len()
            && self.signature_base <= self.signature_count
            && self.descriptor_base <= self.descriptor_count
            && self.signature_base <= next.signatures.len()
            && self.descriptor_base <= next.descriptors.len()
            && self.identity_base <= self.identity_count
            && self.identity_base <= next.identities.len()
            && self.signature_count <= key_limit
            && self.descriptor_count <= key_limit
            && self.identity_count
                <= if self.calculator {
                    super::CALCULATOR_DEFINITION_LIMIT
                } else {
                    super::DEFINITION_LIMIT
                }
    }

    pub(super) fn matches(&self, current: &super::Compiler, next: &super::Compiler) -> bool {
        if !self.positions_match(current, next) {
            return false;
        }
        // The profile-scoped signature pool bounds exact canonical key bytes;
        // provenance still compares every retained record, never just a hash.
        let mut index = 0usize;
        while index < self.signature_base && current.signatures.same_key(&next.signatures, index) {
            index += 1;
        }
        if index != self.signature_base {
            return false;
        }
        index = 0;
        while index < self.descriptor_base && current.descriptors[index] == next.descriptors[index]
        {
            index += 1;
        }
        if index != self.descriptor_base {
            return false;
        }
        index = 0;
        while index < self.identity_base {
            // Every recipe came from the bounded private output buffer.
            let before = &current.identities[index];
            let prepared = &next.identities[index];
            if before.id != prepared.id || before.recipe != prepared.recipe {
                break;
            }
            index += 1;
        }
        index == self.identity_base
    }
}

#[cfg(test)]
mod tests {
    use super::Base;
    use crate::source::Compiler;
    use noble_kernel::types::{ResourceKind, Ty};

    #[test]
    fn calculator_cumulative_prepare_commit_keeps_ids_beyond_ordinary_boundary() {
        let mut compiler = Compiler::new_calculator();
        for index in 0..=crate::signatures::SOURCE_KEY_LIMIT {
            assert_eq!(
                compiler
                    .signatures
                    .intern(&[Ty::Resource(ResourceKind(index as u32))])
                    .ok(),
                Some(index as u32)
            );
        }
        let session = noble_contracts::source::ModuleSession::new_calculator()
            .unwrap_or_else(|_| panic!("calculator session refused"));
        let limits = noble_contracts::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        };
        let input = session
            .prepare(b"", &[], limits)
            .unwrap_or_else(|_| panic!("empty calculator source refused"));
        let submission = input
            .submission()
            .unwrap_or_else(|| panic!("missing checked source submission"));
        let first = compiler
            .prepare(submission)
            .unwrap_or_else(|_| panic!("calculator prepare refused previous signatures"));
        assert!(compiler.can_commit(&first));
        compiler
            .commit(first)
            .unwrap_or_else(|_| panic!("calculator commit refused previous signatures"));
        assert_eq!(
            compiler
                .signatures
                .intern(&[Ty::Resource(ResourceKind(4096))])
                .ok(),
            Some(4096)
        );
        let expected_id = compiler.signatures.len() as u32;
        let next_id = compiler
            .signatures
            .intern(&[Ty::Resource(ResourceKind(4097))])
            .unwrap_or_else(|_| panic!("calculator refused next stable signature"));
        assert_eq!(next_id, expected_id);
        let second = compiler
            .prepare(submission)
            .unwrap_or_else(|_| panic!("calculator cumulative prepare refused"));
        assert!(compiler.can_commit(&second));
        let stale = Base::capture(
            &compiler,
            compiler.identities.len(),
            compiler.signatures.len(),
            compiler.descriptors.len(),
        );
        compiler
            .commit(second)
            .unwrap_or_else(|_| panic!("calculator cumulative commit refused"));
        assert_eq!(
            compiler
                .signatures
                .intern(&[Ty::Resource(ResourceKind(4097))])
                .ok(),
            Some(next_id)
        );
        assert!(!stale.matches(&compiler, &compiler));
        let mut bogus = Base::capture(
            &compiler,
            compiler.identities.len(),
            compiler.signatures.len(),
            compiler.descriptors.len(),
        );
        bogus.signature_count = crate::signatures::CALCULATOR_SOURCE_KEY_LIMIT + 1;
        assert!(!bogus.matches(&compiler, &compiler));
        let mut bogus = Base::capture(
            &compiler,
            compiler.identities.len(),
            compiler.signatures.len(),
            compiler.descriptors.len(),
        );
        bogus.descriptor_count = crate::signatures::CALCULATOR_SOURCE_KEY_LIMIT + 1;
        assert!(!bogus.matches(&compiler, &compiler));
    }
}
