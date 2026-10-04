#![forbid(unsafe_code)]
use std::mem::{align_of, offset_of, size_of};

#[repr(C)]
struct TagThenLength { tag: u8, length: u32 }
#[repr(C)]
struct LengthThenTag { length: u32, tag: u8 }

// These contain the same logical fields, but layout order/padding is a Rust
// native ABI property, not a selected portable Noble identity encoding.
fn main() {
    let first = (size_of::<TagThenLength>(), align_of::<TagThenLength>(),
        offset_of!(TagThenLength, tag), offset_of!(TagThenLength, length));
    let second = (size_of::<LengthThenTag>(), align_of::<LengthThenTag>(),
        offset_of!(LengthThenTag, tag), offset_of!(LengthThenTag, length));
    assert_ne!((first.2, first.3), (second.2, second.3));
    assert!(first.0 >= 5 && second.0 >= 5);
    println!("tag-then-length:{first:?};length-then-tag:{second:?};native-layout-cannot-be-canonical-id");
}
