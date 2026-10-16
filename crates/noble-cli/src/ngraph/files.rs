//! The only host effects of `noble ngraph` (design D7, D6): bounded reads of
//! the named input files and create-new publication of the named output.
//! Messages never contain a path (D11).

/// Host input bounds (design D9).
pub(super) const GRAPH_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const VECTORS_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const DATA_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024;
pub(super) const SOURCE_BYTES: u64 = 1024 * 1024;
pub(super) const SUBMISSION_BYTES: u32 = 65_536;

/// Reads a regular file of at most `limit` bytes.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; read rejects non-files, I/O failures and content beyond the byte bound before and after reading. Files may change after metadata observation, so these are reported host refusals rather than assertions."
)]
pub(super) fn read(
    path: &std::path::Path,
    limit: u64,
    what: &str,
) -> Result<std::vec::Vec<u8>, std::string::String> {
    let metadata = attempt!(std::fs::metadata(path).map_err(|e| std::format!("{what}: {e}")));
    if !metadata.is_file() {
        return Err(std::format!("{what} is not a regular file"));
    }
    if metadata.len() > limit {
        return Err(std::format!("{what} exceeds {limit} bytes"));
    }
    let file = attempt!(std::fs::File::open(path).map_err(|e| std::format!("{what}: {e}")));
    let mut bytes = std::vec::Vec::new();
    attempt!(
        std::io::Read::read_to_end(&mut std::io::Read::take(file, limit + 1), &mut bytes)
            .map_err(|e| std::format!("{what}: {e}"))
    );
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > limit {
        return Err(std::format!("{what} exceeds {limit} bytes"));
    }
    Ok(bytes)
}

fn partial(out: &std::path::Path) -> std::path::PathBuf {
    let mut name = out.as_os_str().to_owned();
    name.push(".partial");
    std::path::PathBuf::from(name)
}

/// A completed publication (design D6).
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Published {
    /// `out` holds the bytes and `<out>.partial` is gone.
    Clean,
    /// `out` holds the bytes, but `<out>.partial`, a second name for the
    /// same file, could not be removed (the I/O error, without a path).
    PartialLeft(std::string::String),
}

/// Publishes `bytes` as a new file at `out`: `<out>.partial` is created with
/// create-new, written and synced, hard-linked to `out` (which fails if
/// `out` exists) and removed. No existing path is ever truncated, renamed
/// over or modified; an existing `out` or `<out>.partial` is refused and left
/// untouched. Once the link exists the output is published, even if the
/// removal then fails ([`Published::PartialLeft`]).
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; publication reports every existing path and I/O failure as a host refusal and removes only the partial file it created itself."
)]
pub(super) fn publish(
    out: &std::path::Path,
    bytes: &[u8],
) -> Result<Published, std::string::String> {
    if std::fs::symlink_metadata(out).is_ok() {
        return Err("the output path already exists".into());
    }
    let staging = partial(out);
    let mut file = attempt!(std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => "the output's .partial path already exists".into(),
            _ => std::format!("output: {e}"),
        }));
    let written = std::io::Write::write_all(&mut file, bytes).and_then(|()| file.sync_all());
    drop(file);
    let linked = written.and_then(|()| std::fs::hard_link(&staging, out));
    let removed = std::fs::remove_file(&staging);
    outcome(linked, removed)
}

/// The outcome of linking `<out>.partial` to `out` and then removing it.
/// The link decides: after it, `out` is complete and immutable, so a failed
/// removal leaves an extra name for the same bytes rather than undoing the
/// publication.
fn outcome(
    linked: std::io::Result<()>,
    removed: std::io::Result<()>,
) -> Result<Published, std::string::String> {
    let left = |r: std::io::Result<()>| match r {
        Ok(()) => std::string::String::new(),
        Err(e) => std::format!("; the .partial file could not be removed: {e}"),
    };
    match (linked, removed) {
        (Ok(()), Ok(())) => Ok(Published::Clean),
        (Ok(()), Err(e)) => Ok(Published::PartialLeft(std::format!("{e}"))),
        (Err(e), r) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(std::format!("the output path already exists{}", left(r)))
        }
        (Err(e), r) => Err(std::format!("output: {e}{}", left(r))),
    }
}

#[cfg(test)]
mod tests {
    use super::{outcome, Published};

    fn injected() -> std::io::Error {
        std::io::Error::other("injected")
    }

    #[test]
    fn a_linked_output_is_published_even_if_its_partial_file_stays() {
        assert_eq!(outcome(Ok(()), Ok(())), Ok(Published::Clean));
        assert_eq!(
            outcome(Ok(()), Err(injected())),
            Ok(Published::PartialLeft("injected".into()))
        );
        let exists = || std::io::Error::from(std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            outcome(Err(exists()), Ok(())),
            Err("the output path already exists".into())
        );
        assert_eq!(
            outcome(Err(injected()), Err(injected())),
            Err("output: injected; the .partial file could not be removed: injected".into())
        );
    }
}
