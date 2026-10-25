pub(super) fn read_file(
    path: &std::path::Path,
    limit_bytes: u32,
) -> Result<std::vec::Vec<u8>, super::output::Failure> {
    let file = attempt!(std::fs::File::open(path).map_err(io_error));
    let read_limit_bytes = attempt!(u64::from(limit_bytes).checked_add(1).ok_or_else(exhausted));
    let mut source = std::vec::Vec::new();
    attempt!(std::io::Read::read_to_end(
        &mut std::io::Read::take(file, read_limit_bytes),
        &mut source,
    )
    .map_err(io_error));
    let length_bytes = attempt!(u64::try_from(source.len()).map_err(|_| exhausted()));
    if length_bytes > u64::from(limit_bytes) {
        return Err(exhausted());
    }
    Ok(source)
}

pub(crate) enum Submission {
    Source(std::vec::Vec<u8>),
    Exhausted,
    ReleaseObserved { store: std::string::String, id: u64 },
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; read_submission validates the frame header and byte budget before allocation, boundedly drains recoverable declared-session frames, and reports short reads through Failure. Invalid framing, oversized input and I/O errors must not panic."
)]
pub(crate) fn read_submission(
    reader: &mut impl std::io::BufRead,
    framed: bool,
    limit_bytes: u32,
    recover_bounded_oversize: bool,
    allow_observed_release: bool,
) -> Result<Option<Submission>, super::output::Failure> {
    if !framed {
        return read_line(reader, limit_bytes).map(|source| source.map(Submission::Source));
    }
    let header = match attempt!(read_line(
        reader,
        if allow_observed_release { 96 } else { 20 }
    )) {
        Some(header) => header,
        None => return Ok(None),
    };
    if allow_observed_release && header.starts_with(b":release-observed-roots ") {
        let text = attempt!(std::str::from_utf8(&header).map_err(|_| invalid_header()));
        let mut fields = text
            .trim_end_matches('\n')
            .strip_prefix(":release-observed-roots ")
            .ok_or_else(invalid_header)?
            .split(' ');
        let store = attempt!(fields
            .next()
            .filter(|store| store.len() == 36
                && store
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() || byte == b'-'))
            .ok_or_else(invalid_header));
        let id = attempt!(fields
            .next()
            .and_then(|id| id.parse::<u64>().ok())
            .filter(|id| (1..=9_007_199_254_740_991).contains(id))
            .ok_or_else(invalid_header));
        if fields.next().is_some() {
            return Err(invalid_header());
        }
        return Ok(Some(Submission::ReleaseObserved {
            store: store.to_owned(),
            id,
        }));
    }
    let count_bytes = attempt!(std::str::from_utf8(&header)
        .ok()
        .and_then(|header| header.trim_end().parse::<u32>().ok())
        .ok_or_else(|| super::output::Failure::new(
            super::output::ErrorContext {
                stage: "parse",
                outcome: "invalid-input",
            },
            "invalid source frame byte count",
        )));
    if count_bytes > limit_bytes {
        if recover_bounded_oversize
            && count_bytes <= noble_contracts::source::RetainedSourceLimit::MAX_BYTES
        {
            attempt!(drain_frame(reader, count_bytes));
            return Ok(Some(Submission::Exhausted));
        }
        return Err(exhausted());
    }
    let count_bytes = attempt!(usize::try_from(count_bytes).map_err(|_| exhausted()));
    let mut source = std::vec![0_u8; count_bytes];
    attempt!(std::io::Read::read_exact(reader, &mut source).map_err(io_error));
    Ok(Some(Submission::Source(source)))
}

fn invalid_header() -> super::output::Failure {
    super::output::Failure::new(
        super::output::ErrorContext {
            stage: "parse",
            outcome: "invalid-input",
        },
        "invalid signed source control header",
    )
}

/// Plain source frames for profiles without declared-session oversize
/// recovery or observed-root release controls (calculator, numeric graph).
/// Neither control variant can be produced with both options disabled; they
/// fail closed rather than being treated as source.
pub(crate) fn read_source(
    reader: &mut impl std::io::BufRead,
    framed: bool,
    limit_bytes: u32,
) -> Result<Option<std::vec::Vec<u8>>, super::output::Failure> {
    match attempt!(read_submission(reader, framed, limit_bytes, false, false)) {
        Some(Submission::Source(source)) => Ok(Some(source)),
        Some(Submission::Exhausted | Submission::ReleaseObserved { .. }) => Err(exhausted()),
        None => Ok(None),
    }
}

fn drain_frame(
    reader: &mut impl std::io::BufRead,
    count_bytes: u32,
) -> Result<(), super::output::Failure> {
    let mut remaining = attempt!(usize::try_from(count_bytes).map_err(|_| exhausted()));
    let mut buffer = [0_u8; 8192];
    while remaining > 0 {
        let length = remaining.min(buffer.len());
        attempt!(std::io::Read::read_exact(reader, &mut buffer[..length]).map_err(io_error));
        remaining -= length;
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; read_line checks cumulative byte counts and slices before consuming buffered input, returning exhaustion or I/O diagnostics. Stream lengths and failures are external data, not assertion preconditions."
)]
pub(super) fn read_line(
    reader: &mut impl std::io::BufRead,
    limit_bytes: u32,
) -> Result<Option<std::vec::Vec<u8>>, super::output::Failure> {
    let limit_bytes = attempt!(usize::try_from(limit_bytes).map_err(|_| exhausted()));
    let mut bytes = std::vec::Vec::new();
    loop {
        let available = attempt!(reader.fill_buf().map_err(io_error));
        if available.is_empty() {
            return Ok(if bytes.is_empty() { None } else { Some(bytes) });
        }
        let count_bytes = match available.iter().position(|byte| *byte == b'\n') {
            Some(offset_bytes) => attempt!(offset_bytes.checked_add(1).ok_or_else(exhausted)),
            None => available.len(),
        };
        let length_bytes = attempt!(bytes.len().checked_add(count_bytes).ok_or_else(exhausted));
        if length_bytes > limit_bytes {
            return Err(exhausted());
        }
        let chunk = attempt!(available.get(..count_bytes).ok_or_else(exhausted));
        let is_complete = chunk.ends_with(b"\n");
        bytes.extend_from_slice(chunk);
        reader.consume(count_bytes);
        if is_complete {
            return Ok(Some(bytes));
        }
    }
}

pub(super) fn io_error(error: std::io::Error) -> super::output::Failure {
    super::output::Failure::new(
        super::output::ErrorContext {
            stage: "shell",
            outcome: "internal-failure",
        },
        error.to_string(),
    )
}

pub(super) fn write_new(
    path: &std::path::Path,
    bytes: &[u8],
) -> Result<(), super::output::Failure> {
    let mut file = attempt!(std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(io_error));
    std::io::Write::write_all(&mut file, bytes).map_err(io_error)
}

/// Why publishing a fresh package file left no destination behind.
#[derive(Debug)]
pub(super) enum PublishRefusal {
    /// The destination, or the staging name for it, already exists. Both are
    /// untouched and no bytes were truncated.
    DestinationExists,
    /// The staging file could not be created, or could not be linked to the
    /// destination after it was written.
    CannotCreate(std::io::Error),
    /// The staging file could not be written. It is removed again, so the
    /// destination and its staging name are both absent afterwards.
    WriteFailed(std::io::Error),
}

/// Drops a removal result: a staging file that survives is an extra name for
/// bytes nobody published, never a reason to undo a refusal.
fn clear_staging(staging: &std::path::Path) {
    drop(std::fs::remove_file(staging));
}

/// Publishes `bytes` as a new file at `destination` without ever replacing an
/// existing path: `<destination>.partial` is created with create-new, written
/// and synced, hard-linked to `destination` (which refuses an existing one)
/// and removed. A failed write removes the staging file itself, so a refused
/// publication leaves neither the destination nor a partial package behind
/// and a retry to the same fresh destination can succeed.
pub(super) fn publish_new(
    destination: &std::path::Path,
    bytes: &[u8],
) -> Result<(), PublishRefusal> {
    publish_bytes(destination, |file| {
        std::io::Write::write_all(file, bytes).and_then(|()| file.sync_all())
    })
}

/// The publication sequence shared by every package writer, with the write
/// step itself supplied. The staging file is always removed again when the
/// supplied write fails, so no partial package and no staging name survives.
pub(super) fn publish_bytes(
    destination: &std::path::Path,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<()>,
) -> Result<(), PublishRefusal> {
    let mut staging_name = destination.as_os_str().to_os_string();
    staging_name.push(".partial");
    let staging = std::path::Path::new(&staging_name);
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(staging)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(PublishRefusal::DestinationExists)
        }
        Err(error) => return Err(PublishRefusal::CannotCreate(error)),
    };
    let written = write(&mut file);
    drop(file);
    if let Err(error) = written {
        clear_staging(staging);
        return Err(PublishRefusal::WriteFailed(error));
    }
    match std::fs::hard_link(staging, destination) {
        Ok(()) => {
            clear_staging(staging);
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            clear_staging(staging);
            Err(PublishRefusal::DestinationExists)
        }
        Err(error) => {
            clear_staging(staging);
            Err(PublishRefusal::CannotCreate(error))
        }
    }
}

pub(super) fn exhausted() -> super::output::Failure {
    super::output::Failure::new(
        super::output::ErrorContext {
            stage: "parse",
            outcome: "exhausted",
        },
        "source byte limit exceeded",
    )
}

#[cfg(test)]
mod tests {
    use super::{publish_bytes, publish_new, PublishRefusal};

    /// A private directory that owns its own destination names.
    struct Scratch(std::path::PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            drop(std::fs::remove_dir_all(&self.0));
        }
    }

    fn scratch(label: &str) -> Result<Scratch, std::string::String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_nanos();
        let path = std::env::temp_dir().join(std::format!(
            "noble-publish-{label}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::DirBuilder::new()
            .create(&path)
            .map_err(|error| error.to_string())?;
        Ok(Scratch(path))
    }

    fn staging_of(destination: &std::path::Path) -> std::path::PathBuf {
        let mut name = destination.as_os_str().to_os_string();
        name.push(".partial");
        std::path::PathBuf::from(name)
    }

    /// A write failure after the staging file exists leaves neither the
    /// destination nor its staging name, and a retry to the same fresh
    /// destination publishes the complete bytes.
    #[test]
    fn a_failed_write_leaves_no_package_and_a_retry_succeeds() -> Result<(), std::string::String> {
        let scratch = scratch("failed-write")?;
        let destination = scratch.0.join("package.json");
        let refusal = publish_bytes(&destination, |_file| {
            Err(std::io::Error::other("injected write failure"))
        });
        assert!(matches!(refusal, Err(PublishRefusal::WriteFailed(_))));
        assert!(
            !destination.exists(),
            "a refused publication publishes nothing"
        );
        assert!(
            !staging_of(&destination).exists(),
            "a refused publication leaves no partial package"
        );
        let bytes = b"complete package bytes";
        assert!(publish_new(&destination, bytes).is_ok());
        assert_eq!(
            std::fs::read(&destination).map_err(|error| error.to_string())?,
            bytes.to_vec()
        );
        assert!(!staging_of(&destination).exists());
        Ok(())
    }

    /// An existing destination is refused and left untouched, without a
    /// staging name appearing beside it.
    #[test]
    fn an_existing_destination_is_never_overwritten() -> Result<(), std::string::String> {
        let scratch = scratch("existing")?;
        let destination = scratch.0.join("package.json");
        std::fs::write(&destination, b"earlier bytes").map_err(|error| error.to_string())?;
        let refusal = publish_new(&destination, b"replacement bytes");
        assert!(matches!(refusal, Err(PublishRefusal::DestinationExists)));
        assert_eq!(
            std::fs::read(&destination).map_err(|error| error.to_string())?,
            b"earlier bytes".to_vec()
        );
        assert!(!staging_of(&destination).exists());
        Ok(())
    }
}
