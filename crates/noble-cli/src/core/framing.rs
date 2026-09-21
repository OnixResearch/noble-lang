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

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; read_submission validates the frame header and byte budget before allocation and reports short reads through Failure. Invalid framing, oversized input and I/O errors must not panic."
)]
pub(super) fn read_submission(
    reader: &mut impl std::io::BufRead,
    framed: bool,
    limit_bytes: u32,
) -> Result<Option<std::vec::Vec<u8>>, super::output::Failure> {
    if !framed {
        return read_line(reader, limit_bytes);
    }
    let header = match attempt!(read_line(reader, 20)) {
        Some(header) => header,
        None => return Ok(None),
    };
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
        return Err(exhausted());
    }
    let count_bytes = attempt!(usize::try_from(count_bytes).map_err(|_| exhausted()));
    let mut source = std::vec![0_u8; count_bytes];
    attempt!(std::io::Read::read_exact(reader, &mut source).map_err(io_error));
    Ok(Some(source))
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

fn exhausted() -> super::output::Failure {
    super::output::Failure::new(
        super::output::ErrorContext {
            stage: "parse",
            outcome: "exhausted",
        },
        "source byte limit exceeded",
    )
}
