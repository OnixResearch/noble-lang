//! A test-only identity scheme: four SipHash-1-3 streams (std's
//! `DefaultHasher` with fixed zero keys), each prefixed with its index,
//! concatenated to 32 bytes. Deterministic for one toolchain; it keeps the
//! library's tests free of third-party crates. BLAKE3 identities are pinned
//! by the experiment's `identity_agreement` test.

#![allow(dead_code)]

use std::hash::{DefaultHasher, Hasher};

use noble_ngraph::ir::Digest;
use noble_ngraph::{IdentityHasher, IdentityScheme};

/// The test scheme.
pub struct TestScheme;

/// The scheme instance used by the library tests.
pub static IDS: TestScheme = TestScheme;

struct Streams([DefaultHasher; 4]);

impl IdentityHasher for Streams {
    fn update(&mut self, bytes: &[u8]) {
        for h in &mut self.0 {
            h.write(bytes);
        }
    }
    fn finish(self: Box<Self>) -> Digest {
        let mut out = [0u8; 32];
        for (i, h) in self.0.iter().enumerate() {
            out[i * 8..i * 8 + 8].copy_from_slice(&h.finish().to_le_bytes());
        }
        out
    }
}

impl IdentityScheme for TestScheme {
    fn name(&self) -> &'static str {
        "test-siphash13x4"
    }
    fn start(&self) -> Box<dyn IdentityHasher + '_> {
        let mut hs = [
            DefaultHasher::new(),
            DefaultHasher::new(),
            DefaultHasher::new(),
            DefaultHasher::new(),
        ];
        for (i, h) in hs.iter_mut().enumerate() {
            h.write_u8(u8::try_from(i).unwrap_or(0));
        }
        Box::new(Streams(hs))
    }
}
