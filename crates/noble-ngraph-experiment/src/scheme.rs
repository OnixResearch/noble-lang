//! The BLAKE3 identity scheme injected into `noble-ngraph` (design D2). It
//! streams exactly the bytes the library emits into `blake3 =1.8.7`, so
//! every identity equals the pre-cutover experiment's (design D3,
//! `tests/identity_agreement.rs`). A hash identifies bytes; it proves nothing.

use noble_ngraph::ir::Digest;
use noble_ngraph::{IdentityHasher, IdentityScheme};

/// BLAKE3-256.
#[derive(Clone, Copy, Debug, Default)]
pub struct Blake3Scheme;

/// The scheme instance used throughout the harness.
pub static BLAKE3: Blake3Scheme = Blake3Scheme;

struct Running(blake3::Hasher);

impl IdentityHasher for Running {
    fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }
    fn finish(self: Box<Self>) -> Digest {
        *self.0.finalize().as_bytes()
    }
}

impl IdentityScheme for Blake3Scheme {
    fn name(&self) -> &'static str {
        "blake3"
    }
    fn start(&self) -> Box<dyn IdentityHasher + '_> {
        Box::new(Running(blake3::Hasher::new()))
    }
}
