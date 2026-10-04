use noble_decoder_experiment::{decode_owned, decode_view, external_during_event, Event, Refusal};

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    result
}

fn unhex(text: &str) -> Result<Vec<u8>, &'static str> {
    if text.len() > 50 || text.len() % 2 != 0 { return Err("invalid-frame-size"); }
    text.as_bytes().chunks_exact(2).map(|pair| {
        let hi = (pair[0] as char).to_digit(16).ok_or("invalid-hex")?;
        let lo = (pair[1] as char).to_digit(16).ok_or("invalid-hex")?;
        Ok(((hi << 4) | lo) as u8)
    }).collect()
}

fn decode(kind: &str, offset: &str, text: &str) -> Result<String, &'static str> {
    let offset = offset.parse::<usize>().map_err(|_| "invalid-offset")?;
    if offset > 1 { return Err("invalid-offset"); }
    let bytes = unhex(text)?;
    let mut frame = Vec::with_capacity(bytes.len() + offset);
    frame.resize(offset, 0xa5);
    frame.extend_from_slice(&bytes);
    let record = &frame[offset..];
    let result: Result<String, Refusal> = match kind {
        "owned" => decode_owned(record).map(|item| {
            // This command owns a complete allocation before rendering.
            format!("candidate:{}", hex(&item.payload))
        }),
        "view" => decode_view(record).map(|item| format!("candidate:{}", hex(item.payload))),
        _ => return Err("invalid-kind"),
    };
    match result {
        Ok(text) => Ok(text),
        Err(error) => Ok(format!("reject:{error:?}")),
    }
}

fn lifetime(storage: &str, event: &str) -> Result<String, &'static str> {
    let mut record = vec![1, 0, 0, 0, 3, 0, 0, 0, b'a', b'b', b'c'];
    let changed = b"def";
    if storage == "safely-copied-complete-record" && event == "callback-mutates-source" {
        let owned = decode_owned(&record).map_err(|_| "copy-failed")?;
        record[8..].copy_from_slice(changed);
        if owned.payload == b"abc" && &record[8..] == changed {
            return Ok(format!("retain:{}", hex(&owned.payload)));
        }
        return Err("snapshot-invalidated");
    }
    let (event, header_only) = match (storage, event) {
        ("unprotected-external-view", "callback-mutates-payload") => (Event::CallbackMutates, false),
        ("unprotected-external-view", "suspension") => (Event::Suspension, false),
        ("header-copy-with-unprotected-payload", "callback-mutates-payload") => (Event::CallbackMutates, true),
        _ => return Err("unsupported-lifetime-variant"),
    };
    if external_during_event(&record, event, header_only) != Err(Refusal::UnstableExternal) {
        return Err("view-escaped");
    }
    if event == Event::CallbackMutates {
        record[8..].copy_from_slice(changed);
    } else {
        std::thread::yield_now();
    }
    Ok("reject-before-view-escapes".to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.as_slice() {
        [mode, kind, offset, frame] if mode == "decode" => decode(kind, offset, frame),
        [mode, storage, event] if mode == "lifetime" => lifetime(storage, event),
        _ => Err("usage"),
    };
    match result {
        Ok(line) => println!("{line}"),
        Err(error) => { eprintln!("{error}"); std::process::exit(2); }
    }
}
