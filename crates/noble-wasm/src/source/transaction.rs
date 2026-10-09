/// The allocation state against which a preparation was checked. The owned
/// prospective compiler retains its immutable base prefixes, so provenance
/// requires only these scalar positions and no second whole-state clone.
pub(super) struct Base {
    live: bool,
    declared_live: bool,
    text_cursor: bool,
    declared_text: bool,
    generation: u32,
    source_epoch_generation: u32,
    functions: u32,
    text_end_bytes: u32,
    signature_count: usize,
    descriptor_count: usize,
    identity_count: usize,
    universal_code_count: usize,
}

impl Base {
    pub(super) const fn capture(compiler: &super::Compiler) -> Self {
        Self {
            live: compiler.live,
            declared_live: compiler.declared_live,
            text_cursor: compiler.text_cursor,
            declared_text: compiler.declared_text,
            generation: compiler.generation,
            source_epoch_generation: compiler.source_epoch_generation,
            functions: compiler.functions,
            text_end_bytes: compiler.text_end,
            signature_count: compiler.signatures.keys.len(),
            descriptor_count: compiler.descriptors.len(),
            identity_count: compiler.identities.len(),
            universal_code_count: compiler.universal_code.len(),
        }
    }

    const fn positions_match(&self, current: &super::Compiler, next: &super::Compiler) -> bool {
        self.live == current.live
            && self.live == next.live
            && self.declared_live == current.declared_live
            && self.declared_live == next.declared_live
            && self.text_cursor == current.text_cursor
            && self.text_cursor == next.text_cursor
            && self.declared_text == current.declared_text
            && self.declared_text == next.declared_text
            && self.generation == current.generation
            && self.source_epoch_generation == current.source_epoch_generation
            && self.source_epoch_generation == next.source_epoch_generation
            && self.functions == current.functions
            && self.text_end_bytes == current.text_end
            && self.signature_count == current.signatures.keys.len()
            && self.descriptor_count == current.descriptors.len()
            && self.identity_count == current.identities.len()
            && self.universal_code_count == current.universal_code.len()
            && self.universal_code_count <= next.universal_code.len()
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
        if index != self.identity_count {
            return false;
        }
        index = 0;
        while index < self.universal_code_count {
            let before = &current.universal_code[index];
            let after = &next.universal_code[index];
            if before.definition != after.definition
                || before.definition_slot != after.definition_slot
                || before.definition_identity != after.definition_identity
                || before.module_identity != after.module_identity
                || before.module_version != after.module_version
                || before.definition_ordinal != after.definition_ordinal
                || !alloc::sync::Arc::ptr_eq(&before.source, &after.source)
                || before.stamp != after.stamp
                || before.span != after.span || before.body_entry != after.body_entry
                || before.quotes != after.quotes
            {
                break;
            }
            index += 1;
        }
        index == self.universal_code_count
    }
}
