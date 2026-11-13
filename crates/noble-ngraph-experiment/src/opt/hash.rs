//! Domain-separated identities for frozen optimizer artifacts.

use blake3::Hasher;

use super::types::Digest;

pub(crate) fn digest(domain: &[u8], parts: &[&[u8]]) -> Digest {
    let mut hasher = Hasher::new();
    hasher.update(b"noble-ngraph-experiment/opt/v1\0");
    hasher.update(
        &u64::try_from(domain.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    hasher.update(domain);
    for part in parts {
        hasher.update(&u64::try_from(part.len()).unwrap_or(u64::MAX).to_le_bytes());
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

pub(crate) fn bits_hex(value: u32) -> String {
    format!("0x{value:08X}")
}
