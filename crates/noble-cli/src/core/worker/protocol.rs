const OUTPUT_BYTES: u32 = 4_194_304;

pub(super) fn receive(
    stream: std::process::ChildStdout,
    send: std::sync::mpsc::SyncSender<
        Result<crate::core::output::Report, crate::core::output::Failure>,
    >,
) {
    let mut reader = std::io::BufReader::new(stream);
    loop {
        let reply = read_reply(&mut reader);
        let is_failed = reply.is_err();
        // One command is outstanding. Extra unsolicited replies must not block
        // reader shutdown on a full queue while its owner is reaping the child.
        if send.try_send(reply).is_err() || is_failed {
            break;
        }
    }
}

fn read_reply(
    reader: &mut impl std::io::BufRead,
) -> Result<crate::core::output::Report, crate::core::output::Failure> {
    let header = attempt!(crate::core::framing::read_line(reader, 128));
    let header = attempt!(header.ok_or_else(error));
    let header = attempt!(std::str::from_utf8(&header).map_err(|_| error()));
    let mut words = header.split_whitespace();
    let outcome = attempt!(words.next().ok_or_else(error)).to_owned();
    let count_bytes = attempt!(words
        .next()
        .and_then(|count_bytes| count_bytes.parse::<u32>().ok())
        .ok_or_else(error));
    if words.next().is_some() || count_bytes > OUTPUT_BYTES {
        return Err(error());
    }
    let count_bytes = attempt!(usize::try_from(count_bytes).map_err(|_| error()));
    let mut bytes = std::vec![0_u8; count_bytes];
    attempt!(std::io::Read::read_exact(reader, &mut bytes).map_err(crate::core::framing::io_error));
    let json = attempt!(std::string::String::from_utf8(bytes).map_err(|_| error()));
    Ok(crate::core::output::Report { outcome, json })
}

pub(super) fn error() -> crate::core::output::Failure {
    crate::core::output::Failure::new(
        crate::core::output::ErrorContext {
            stage: "wasm",
            outcome: "internal-failure",
        },
        "invalid bounded engine-worker protocol",
    )
}
