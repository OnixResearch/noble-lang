#[path = "../src/lib.rs"]
mod decoder;

// A returned view cannot outlive its backing Vec, even after a valid decode.
fn escaped() -> decoder::View<'static> {
    let bytes = vec![1, 0, 0, 0, 3, 0, 0, 0, b'a', b'b', b'c'];
    decoder::decode_view(&bytes).unwrap()
}

// A mutable callback cannot change Rust-owned storage during a live borrow.
fn mutates_live_view() {
    let mut bytes = vec![1, 0, 0, 0, 3, 0, 0, 0, b'a', b'b', b'c'];
    let view = decoder::decode_view(&bytes).unwrap();
    bytes[8] = b'd';
    assert_eq!(view.payload, b"abc");
}

fn main() { let _ = escaped(); mutates_live_view(); }
