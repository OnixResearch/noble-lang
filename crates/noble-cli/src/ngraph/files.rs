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

/// Publishes `bytes` as a new file at `out`: `<out>.partial` is created with
/// create-new, written and synced, hard-linked to `out` (which fails if
/// `out` exists) and removed. No existing path is ever truncated, renamed
/// over or modified; an existing `out` or `<out>.partial` is refused and left
/// untouched.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; publication reports every existing path and I/O failure as a host refusal and removes only the partial file it created itself."
)]
pub(super) fn publish(out: &std::path::Path, bytes: &[u8]) -> Result<(), std::string::String> {
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
    match (linked, removed) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(e), _) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            Err("the output path already exists".into())
        }
        (Err(e), _) => Err(std::format!("output: {e}")),
        (Ok(()), Err(e)) => Err(std::format!("output: removing the .partial file: {e}")),
    }
}
