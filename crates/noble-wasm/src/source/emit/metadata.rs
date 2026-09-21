#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; emission mutates only the fresh private output sink; checked plans and caller inputs remain immutable and no partial output escapes preparation."
)]

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; admitted ordered type IDs generate explicit runtime slot checks through the bounded output sink; integer conversion and output exhaustion propagate diagnostics."
)]
pub(super) fn stack_check(
    out: &mut crate::output::Buffer,
    name: &[u8],
    types: &[u32],
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func "));
    attempt!(out.append(name));
    attempt!(out.append(b"\n(if (i32.ne (global.get $sp) "));
    attempt!(out.i32(attempt!(super::super::plan::number(types.len()))));
    attempt!(out.append(b") (then (call $fail (i32.const 3)) (return)))\n"));
    let mut index = 0usize;
    let mut failure = None;
    while index < types.len() {
        match stack_slot(out, types[index], index) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => out.append(b")\n"),
    }
}

fn stack_slot(
    out: &mut crate::output::Buffer,
    ty: u32,
    index: usize,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.eqz (call $t"));
    attempt!(out.number(u64::from(ty)));
    attempt!(out.append(b" (call $slot_kind "));
    attempt!(out.i32(attempt!(super::super::plan::number(index))));
    attempt!(out.append(b") (call $slot_value "));
    attempt!(out.i32(attempt!(super::super::plan::number(index))));
    out.append(b"))) (then (call $fail (i32.const 3)) (return)))\n")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; byte nibbles bound each hexadecimal lookup to the fixed alphabet, and every escaped byte is appended through the bounded private output sink with first-error propagation."
)]
pub(super) fn data_segment(
    out: &mut crate::output::Buffer,
    segment: usize,
    bytes: &[u8],
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(data $d"));
    attempt!(out.index(segment));
    attempt!(out.append(b" \""));
    let mut index = 0usize;
    let mut failure = None;
    while index < bytes.len() {
        match escaped_byte(out, bytes[index]) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => out.append(b"\")\n"),
    }
}

fn escaped_byte(out: &mut crate::output::Buffer, byte: u8) -> Result<(), crate::Diagnostic> {
    const HEX: &[u8] = b"0123456789abcdef";
    let escaped = [
        b'\\',
        HEX[usize::from(byte >> 4)],
        HEX[usize::from(byte & 15)],
    ];
    out.append(&escaped)
}

pub(super) fn descriptors(
    out: &mut crate::output::Buffer,
    descriptors: &[(u32, u32)],
) -> Result<(), crate::Diagnostic> {
    attempt!(descriptor_function(
        out,
        b"signature_address",
        descriptors,
        false
    ));
    descriptor_function(out, b"signature_length", descriptors, true)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; validated descriptor rows are emitted in order through the bounded sink, including an explicit runtime rejection for unknown identities; no untrusted metadata requires a Rust assertion."
)]
fn descriptor_function(
    out: &mut crate::output::Buffer,
    name: &[u8],
    descriptors: &[(u32, u32)],
    is_length: bool,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func (export \""));
    attempt!(out.append(name));
    attempt!(out.append(b"\") (param $id i32) (result i32)\n"));
    let mut index = 0usize;
    let mut failure = None;
    while index < descriptors.len() {
        match descriptor_entry(out, index, descriptors[index], is_length) {
            Ok(()) => index += 1,
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    match failure {
        Some(problem) => Err(problem),
        None => out.append(b"(call $fail (i32.const 4))\n(i32.const 0))\n"),
    }
}

fn descriptor_entry(
    out: &mut crate::output::Buffer,
    index: usize,
    descriptor: (u32, u32),
    is_length: bool,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.eq (local.get $id) "));
    attempt!(out.i32(attempt!(super::super::plan::number(index))));
    attempt!(out.append(b") (then (return "));
    attempt!(out.i32(if is_length {
        descriptor.1
    } else {
        descriptor.0
    }));
    out.append(b")))\n")
}
