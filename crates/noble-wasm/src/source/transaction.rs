/// The allocation state against which a preparation was checked. The owned
/// prospective compiler retains its immutable base prefixes, so provenance
/// requires only these scalar positions and no second whole-state clone.
pub(super) struct Base {
    generation: u32,
    functions: u32,
    text_end_bytes: u32,
    signature_count: usize,
    descriptor_count: usize,
    identity_count: usize,
}

impl Base {
    pub(super) const fn capture(compiler: &super::Compiler) -> Self {
        Self {
            generation: compiler.generation,
            functions: compiler.functions,
            text_end_bytes: compiler.text_end,
            signature_count: compiler.signatures.keys.len(),
            descriptor_count: compiler.descriptors.len(),
            identity_count: compiler.identities.len(),
        }
    }

    const fn positions_match(&self, current: &super::Compiler, next: &super::Compiler) -> bool {
        self.generation == current.generation
            && self.functions == current.functions
            && self.text_end_bytes == current.text_end
            && self.signature_count == current.signatures.keys.len()
            && self.descriptor_count == current.descriptors.len()
            && self.identity_count == current.identities.len()
            && self.signature_count <= next.signatures.keys.len()
            && self.descriptor_count <= next.descriptors.len()
            && self.identity_count <= next.identities.len()
            && self.signature_count <= crate::signatures::SOURCE_KEY_LIMIT
            && self.descriptor_count <= crate::signatures::SOURCE_KEY_LIMIT
            && self.identity_count <= super::DEFINITION_LIMIT
    }

    pub(super) fn matches(&self, current: &super::Compiler, next: &super::Compiler) -> bool {
        if !self.positions_match(current, next) {
            return false;
        }
        // The private signature pool caps aggregate key bytes at OUTPUT_BYTE_LIMIT.
        let mut index = 0usize;
        while index < self.signature_count
            && current.signatures.keys[index] == next.signatures.keys[index]
        {
            index += 1;
        }
        if index != self.signature_count {
            return false;
        }
        index = 0;
        while index < self.descriptor_count && current.descriptors[index] == next.descriptors[index]
        {
            index += 1;
        }
        if index != self.descriptor_count {
            return false;
        }
        index = 0;
        while index < self.identity_count {
            // Every recipe came from the bounded private output buffer.
            let before = &current.identities[index];
            let prepared = &next.identities[index];
            if before.id != prepared.id || before.recipe != prepared.recipe {
                break;
            }
            index += 1;
        }
        index == self.identity_count
    }
}
