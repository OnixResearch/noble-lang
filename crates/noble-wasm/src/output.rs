/// A single bounded byte sink for module text and canonical interface keys.
/// Only fixed syntax and checked scalar encodings reach the output.
pub(crate) struct Buffer {
    bytes: alloc::vec::Vec<u8>,
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; this private sink mutates only fresh compiler-owned bytes, never caller input or host state; compile returns the completed owned buffer and exposes no mutable capability."
)]
impl Buffer {
    pub(crate) fn new(capacity_bytes: usize) -> Self {
        Self {
            bytes: alloc::vec::Vec::with_capacity(capacity_bytes),
        }
    }

    pub(crate) fn append(&mut self, bytes: &[u8]) -> Result<(), crate::Diagnostic> {
        let length_bytes = match self.bytes.len().checked_add(bytes.len()) {
            Some(length_bytes) => length_bytes,
            None => return Err(crate::Diagnostic::Exhausted),
        };
        if length_bytes > crate::OUTPUT_BYTE_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-maintainers; value modulo ten is 0..=9, so adding ASCII zero is at most 57 and fits u8; reassess if the radix or digit alphabet changes."
    )]
    pub(crate) fn number(&mut self, mut value: u64) -> Result<(), crate::Diagnostic> {
        let mut digits = [0u8; 20];
        let mut start = digits.len();
        loop {
            start = match start.checked_sub(1) {
                Some(start) => start,
                None => return Err(crate::Diagnostic::Defective),
            };
            let digit = match u8::try_from(value % 10) {
                Ok(digit) => digit,
                Err(_) => return Err(crate::Diagnostic::Defective),
            };
            digits[start] = b'0' + digit;
            value /= 10;
            if value == 0 {
                break;
            }
        }
        self.append(&digits[start..])
    }

    pub(crate) fn index(&mut self, value: usize) -> Result<(), crate::Diagnostic> {
        let value = match u64::try_from(value) {
            Ok(value) => value,
            Err(_) => return Err(crate::Diagnostic::Exhausted),
        };
        self.number(value)
    }

    pub(crate) fn signed(&mut self, value: i64) -> Result<(), crate::Diagnostic> {
        if value < 0 {
            attempt!(self.append(b"-"));
        }
        self.number(value.unsigned_abs())
    }

    pub(crate) fn i32(&mut self, value: u32) -> Result<(), crate::Diagnostic> {
        attempt!(self.append(b"(i32.const "));
        attempt!(self.number(u64::from(value)));
        self.append(b")")
    }

    pub(crate) fn i64(&mut self, value: i64) -> Result<(), crate::Diagnostic> {
        attempt!(self.append(b"(i64.const "));
        attempt!(self.signed(value));
        self.append(b")")
    }

    pub(crate) fn program_global(&mut self, owner: Option<u32>) -> Result<(), crate::Diagnostic> {
        attempt!(self.append(b"$p"));
        match owner {
            Some(node) => self.number(u64::from(node)),
            None => self.append(b"_root"),
        }
    }

    pub(crate) fn finish(self) -> alloc::vec::Vec<u8> {
        self.bytes
    }
}
