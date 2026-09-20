//! Deterministic splitmix64 generator for the bounded property harness.
//!
//! The harness must be reproducible: fixed seeds, no time or environment
//! dependence (DX-PROPERTY-01).

/// One splitmix64 stream. Not cryptographic; deterministic across platforms.
pub struct Stream {
    state: u64,
}

impl Stream {
    /// A stream from an explicit seed.
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// The next raw 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A value below `bound`, which must be positive.
    pub fn below(&mut self, bound: u64) -> u64 {
        assert!(bound > 0, "property stream requires a positive bound");
        self.next_u64() % bound
    }

    /// The low 32 bits of the next draw, preserving intentional truncation.
    pub fn next_u32(&mut self) -> u32 {
        let [a, b, c, d, _, _, _, _] = self.next_u64().to_le_bytes();
        u32::from_le_bytes([a, b, c, d])
    }

    /// Choose one collection position with checked platform conversions.
    pub(super) fn index(&mut self, count: usize) -> Result<usize, String> {
        let bound = u64::try_from(count)
            .map_err(|error| format!("property choice count exceeds u64: {error}"))?;
        usize::try_from(self.below(bound))
            .map_err(|error| format!("property choice position exceeds usize: {error}"))
    }

    /// A bounded identifier with checked conversion from the stream draw.
    pub fn below_u32(&mut self, bound: u32) -> Result<u32, String> {
        u32::try_from(self.below(u64::from(bound)))
            .map_err(|error| format!("property bounded identifier exceeds u32: {error}"))
    }

    /// A pseudo bit.
    pub fn bit(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }
}
