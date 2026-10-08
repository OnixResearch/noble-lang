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
        self.intern_charged(stack, |_| Ok(()))
    }

    /// Charge exactly the encoded keys compared, including the matching key.
    /// The caller owns its bounded work meter; failed charges leave the pool
    /// unchanged, and both paths use the same canonical encoding and limits.
    pub(crate) fn intern_charged(
        &mut self,
        stack: &[noble_kernel::types::Ty],
        mut charge: impl FnMut(usize) -> Result<(), crate::Diagnostic>,
    ) -> Result<u32, crate::Diagnostic> {
        let key = attempt!(walk::encode(stack, self.extended));
        let mut index = 0usize;
        while index < self.keys.len() {
            attempt!(charge(self.keys[index].len().saturating_add(1)));
            if self.keys[index] == key {
                return number(index);
            }
            index += 1;
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
        let mut failure = None;
        while index < self.keys.len() {
            match signature_line(out, index, &self.keys[index]) {
                Ok(()) => index += 1,
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; signature serialization mutates only the fresh bounded compiler sink, not the borrowed interned key."
)]
fn signature_line(
    out: &mut crate::output::Buffer,
    index: usize,
    key: &[u8],
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b";; signature "));
    attempt!(out.index(index));
    attempt!(out.append(b" = "));
    attempt!(out.append(key));
    out.append(b"\n")
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

#[cfg(test)]
mod tests {
    use super::Pool;
    use noble_kernel::types::Ty;

    #[test]
    fn signature_meter_charges_exact_compared_keys_and_rejects_underfunded_miss() {
        let mut pool = Pool::source();
        assert!(matches!(pool.intern(&[]), Ok(0)));
        assert!(matches!(pool.intern(&[Ty::I64]), Ok(1)));
        assert!(matches!(pool.intern(&[Ty::Bool]), Ok(2)));
        let costs: alloc::vec::Vec<usize> = pool.keys.iter()
            .map(|key| key.len().saturating_add(1)).collect();
        let mut charged = 0usize;
        assert!(matches!(pool.intern_charged(&[], |cost| {
            charged += cost;
            Ok(())
        }), Ok(0)));
        assert_eq!(charged, costs[0], "early exact hit charged unused historical keys");
        charged = 0;
        assert!(matches!(pool.intern_charged(&[Ty::Bool], |cost| {
            charged += cost;
            Ok(())
        }), Ok(2)));
        assert_eq!(charged, costs.iter().sum(),
            "late exact hit skipped a compared historical key");
        let full_miss_cost: usize = costs.iter().sum();
        let mut remaining = full_miss_cost - 1;
        assert!(matches!(pool.intern_charged(&[Ty::I64, Ty::Bool], |cost| {
            remaining = remaining.checked_sub(cost)
                .ok_or(crate::Diagnostic::Exhausted)?;
            Ok(())
        }), Err(crate::Diagnostic::Exhausted)));
        assert_eq!(pool.keys.len(), 3, "unfunded signature was published");
        remaining = full_miss_cost;
        assert!(matches!(pool.intern_charged(&[Ty::I64, Ty::Bool], |cost| {
            remaining = remaining.checked_sub(cost)
                .ok_or(crate::Diagnostic::Exhausted)?;
            Ok(())
        }), Ok(3)));
        assert_eq!(remaining, 0);
    }
}
