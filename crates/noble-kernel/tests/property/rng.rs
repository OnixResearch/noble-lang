//! Deterministic splitmix64 generator for the bounded property harness.
//!
//! The harness must be reproducible: fixed seeds, no time or environment
//! dependence (DX-PROPERTY-01).

/// One splitmix64 stream. Not cryptographic; deterministic across platforms.
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A stream from an explicit seed.
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
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
        self.next_u64() % bound
    }

    /// A pseudo bit.
    pub fn bit(&mut self) -> bool {
        self.next_u64() & 1 == 1
    }
}
