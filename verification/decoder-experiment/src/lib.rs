//! Isolated Decoder-Experiment: these bytes never construct accepted Noble state.
//! This is NOT a canonical Noble encoding or a zerocopy-crate adapter.
#![forbid(unsafe_code)]

pub const HEADER_BYTES: usize = 8;
pub const MAX_PAYLOAD_BYTES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    ShortHeader,
    Version,
    Tag,
    Reserved,
    LengthLimit,
    ShortPayload,
    TrailingBytes,
    UnstableExternal,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Owned {
    pub version: u8,
    pub tag: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct View<'a> {
    pub version: u8,
    pub tag: u8,
    pub payload: &'a [u8],
}

/// Validate every header byte, arithmetic bound and exact record size before
/// any payload allocation or borrowed view publication. All integer fields
/// have declared little-endian byte order, never native alignment or padding.
fn payload(record: &[u8]) -> Result<&[u8], Refusal> {
    let header = record.get(..HEADER_BYTES).ok_or(Refusal::ShortHeader)?;
    if header[0] != 1 { return Err(Refusal::Version); }
    if header[1] != 0 { return Err(Refusal::Tag); }
    if header[2] != 0 || header[3] != 0 { return Err(Refusal::Reserved); }
    let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    // Check a u32 on its original width so even 32-bit hosts reject the
    // malicious 0xffffffff before attempting conversion or allocation.
    if len > MAX_PAYLOAD_BYTES as u32 { return Err(Refusal::LengthLimit); }
    let len = usize::try_from(len).map_err(|_| Refusal::LengthLimit)?;
    let end = HEADER_BYTES.checked_add(len).ok_or(Refusal::LengthLimit)?;
    if record.len() < end { return Err(Refusal::ShortPayload); }
    if record.len() > end { return Err(Refusal::TrailingBytes); }
    record.get(HEADER_BYTES..end).ok_or(Refusal::ShortPayload)
}

/// Complete owned snapshot. Its payload survives subsequent changes to the
/// original buffer; callers must obtain external bytes by a safe bounded read.
pub fn decode_owned(record: &[u8]) -> Result<Owned, Refusal> {
    let bytes = payload(record)?;
    Ok(Owned { version: 1, tag: 0, payload: bytes.to_vec() })
}

/// Zero-copy Rust-owned immutable borrow; `View` cannot outlive this record.
/// This API alone makes no claim about a foreign memory owner's stability.
pub fn decode_view(record: &[u8]) -> Result<View<'_>, Refusal> {
    let bytes = payload(record)?;
    Ok(View { version: 1, tag: 0, payload: bytes })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event { CallbackMutates, Suspension }

/// An unprotected external buffer cannot promise stable bytes across either
/// event. Reject *before* publishing a view, even if a copied header looked
/// valid. `header_only_copy` deliberately never stabilizes the payload.
pub fn external_during_event<'a>(
    record: &'a [u8], event: Event, header_only_copy: bool,
) -> Result<View<'a>, Refusal> {
    if header_only_copy {
        // A copied header is safe to inspect but does not own the payload.
        // No unchecked read or payload borrow is published across the event.
        let header = record.get(..HEADER_BYTES).ok_or(Refusal::ShortHeader)?;
        let mut snapshot = [0; HEADER_BYTES];
        snapshot.copy_from_slice(header);
        if snapshot[0] != 1 { return Err(Refusal::Version); }
        if snapshot[1] != 0 { return Err(Refusal::Tag); }
        if snapshot[2] != 0 || snapshot[3] != 0 { return Err(Refusal::Reserved); }
    }
    match event {
        Event::CallbackMutates | Event::Suspension => Err(Refusal::UnstableExternal),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_borrow_and_complete_copy_have_distinct_owners() {
        let mut source = [1, 0, 0, 0, 3, 0, 0, 0, b'a', b'b', b'c'];
        let owned = decode_owned(&source).unwrap();
        {
            let view = decode_view(&source).unwrap();
            assert_eq!(view.payload.as_ptr(), source[HEADER_BYTES..].as_ptr());
            assert_eq!(view.payload, b"abc");
        }
        source[HEADER_BYTES..].copy_from_slice(b"def");
        assert_eq!(owned.payload, b"abc");
        assert_eq!(decode_view(&source).unwrap().payload, b"def");
        let mut offset = [0u8; 12];
        offset[1..].copy_from_slice(&source);
        assert_eq!(decode_view(&offset[1..]).unwrap().payload, b"def");
    }

    #[test]
    fn unprotected_events_refuse_before_view_publication() {
        let source = [1, 0, 0, 0, 3, 0, 0, 0, b'a', b'b', b'c'];
        for event in [Event::CallbackMutates, Event::Suspension] {
            for copied_header in [false, true] {
                assert_eq!(external_during_event(&source, event, copied_header), Err(Refusal::UnstableExternal));
            }
        }
    }

    #[test]
    fn exact_limits_and_malformed_record_refuse_without_allocation() {
        let mut record = vec![1, 0, 0, 0, 16, 0, 0, 0];
        record.extend([0; 16]);
        assert_eq!(decode_owned(&record).unwrap().payload.len(), 16);
        record[4] = 17;
        assert_eq!(decode_owned(&record), Err(Refusal::LengthLimit));
        record[4] = 16;
        record.pop();
        assert_eq!(decode_view(&record), Err(Refusal::ShortPayload));
    }
}
