mod walk;

pub(crate) const SOURCE_KEY_LIMIT: usize = 4096;

/// IDs intern complete ordered structural stacks, never just stack heights.
/// The canonical grammar is injective and contains no caller-provided text.
#[derive(Clone)]
pub(crate) struct Pool {
    pub(crate) keys: alloc::vec::Vec<alloc::vec::Vec<u8>>,
    bytes: usize,
    extended: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct Interface {
    pub(crate) input: u32,
    pub(crate) output: u32,
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; interning updates only a fresh compiler-private pool and emission only its fresh output sink; complete caller-owned type stacks and effects are immutable."
)]
impl Pool {
    pub(crate) fn new() -> Self {
        Self {
            keys: alloc::vec::Vec::with_capacity(16),
            bytes: 0,
            extended: false,
        }
    }

    pub(crate) fn source() -> Self {
        Self {
            keys: alloc::vec::Vec::with_capacity(16),
            bytes: 0,
            extended: true,
        }
    }

    pub(crate) fn intern(
        &mut self,
        stack: &[noble_kernel::types::Ty],
    ) -> Result<u32, crate::Diagnostic> {
        let key = attempt!(walk::encode(stack, self.extended));
        let mut index = 0usize;
        while index < self.keys.len() && self.keys[index] != key {
            index += 1;
        }
        if index < self.keys.len() {
            return number(index);
        }
        if self.extended && self.keys.len() >= SOURCE_KEY_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        let bytes = match self.bytes.checked_add(key.len()) {
            Some(bytes) => bytes,
            None => return Err(crate::Diagnostic::Exhausted),
        };
        if bytes > crate::OUTPUT_BYTE_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        let id = attempt!(number(self.keys.len()));
        self.keys.push(key);
        self.bytes = bytes;
        Ok(id)
    }

    pub(crate) fn interface(
        &mut self,
        interface: &noble_kernel::untrusted::Interface,
    ) -> Result<Interface, crate::Diagnostic> {
        if !interface.effects.is_empty() {
            return Err(crate::Diagnostic::Unsupported);
        }
        let input = attempt!(self.intern(&interface.stack_in));
        let output = attempt!(self.intern(&interface.stack_out));
        Ok(Interface { input, output })
    }

    pub(crate) fn produced_program(
        &mut self,
        interface: &noble_kernel::untrusted::Interface,
    ) -> Result<Interface, crate::Diagnostic> {
        match interface.stack_out.last() {
            Some(noble_kernel::types::Ty::Program(input, output, effects)) => {
                if !effects.is_empty() {
                    return Err(crate::Diagnostic::Unsupported);
                }
                let input = attempt!(self.intern(input));
                let output = attempt!(self.intern(output));
                Ok(Interface { input, output })
            }
            Some(_) | None => Err(crate::Diagnostic::Defective),
        }
    }

    pub(crate) fn emit(&self, out: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
        let mut index = 0usize;
        while index < self.keys.len() {
            attempt!(out.append(b";; signature "));
            attempt!(out.index(index));
            attempt!(out.append(b" = "));
            attempt!(out.append(&self.keys[index]));
            attempt!(out.append(b"\n"));
            index += 1;
        }
        Ok(())
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this guarded conversion calls the non-const TryFrom trait, retaining its explicit overflow diagnostic instead of narrowing."
)]
fn number(index: usize) -> Result<u32, crate::Diagnostic> {
    match u32::try_from(index) {
        Ok(index) => Ok(index),
        Err(_) => Err(crate::Diagnostic::Exhausted),
    }
}
