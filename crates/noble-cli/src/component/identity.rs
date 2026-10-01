//! A shared, never-reused namespace for all simultaneously live component hosts.
//! Reserving contiguous identifiers lets one invocation keep distinct owner
//! contexts without aliasing another host Store's TableId or context.
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_IDENTITY: AtomicU64 = AtomicU64::new(1);

pub(super) fn reserve(count: u64) -> anyhow::Result<u64> {
    if count == 0 { anyhow::bail!("empty component identity reservation"); }
    NEXT_IDENTITY.fetch_update(Ordering::Relaxed, Ordering::Relaxed,
        |current| current.checked_add(count))
        .map_err(|_| anyhow::anyhow!("component invocation identity exhausted"))
}
