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

/// Selected signatures stay canonical and permanently interned, but each
/// module carries only the keys new since its published base, and only one
/// host copy occupies ingress scratch at a time. Literal Text uses its
/// separate immutable static region and never borrows this scratch.
pub(super) fn signature_segment(
    out: &mut crate::output::Buffer,
    index: usize,
    key: &[u8],
) -> Result<(), crate::Diagnostic> {
    if key.len() > 65_536 {
        return Err(crate::Diagnostic::Exhausted);
    }
    attempt!(out.append(b"(data $sig"));
    attempt!(out.index(index));
    attempt!(out.append(b" \""));
    let mut start = 0usize;
    for (index, &byte) in key.iter().enumerate() {
        if !(32..=126).contains(&byte) || byte == b'"' || byte == b'\\' {
            attempt!(out.append(&key[start..index]));
            attempt!(escaped_byte(out, byte));
            start = index + 1;
        }
    }
    attempt!(out.append(&key[start..]));
    out.append(b"\")\n")
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

/// `calculator` is `(first, keys)`: this module publishes exactly the IDs
/// `first..descriptors.len()`, whose expanded keys are `keys`. Ordinary
/// profiles keep their complete, byte-identical descriptor functions.
pub(super) fn descriptors(
    out: &mut crate::output::Buffer,
    descriptors: &[(u32, u32)],
    calculator: Option<(usize, &[alloc::vec::Vec<u8>])>,
) -> Result<(), crate::Diagnostic> {
    match calculator {
        Some((first, keys)) => {
            let Some(published) = descriptors.get(first..) else {
                return Err(crate::Diagnostic::Defective);
            };
            attempt!(calculator_signature_range(out, first, descriptors.len()));
            attempt!(calculator_signature_address(out, first, published, keys));
            descriptor_function(out, b"signature_length", first, published, true)
        }
        None => {
            attempt!(descriptor_function(
                out,
                b"signature_address",
                0,
                descriptors,
                false
            ));
            descriptor_function(out, b"signature_length", 0, descriptors, true)
        }
    }
}

/// The trusted host copies IDs `[signature_first, signature_end)` once when
/// this module becomes its latest executed instance, retaining the earlier
/// published prefix and replacing any IDs at or above `signature_first`.
fn calculator_signature_range(
    out: &mut crate::output::Buffer,
    first: usize,
    end: usize,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func (export \"signature_first\") (result i32) "));
    attempt!(out.i32(attempt!(super::super::plan::number(first))));
    attempt!(out.append(b")\n(func (export \"signature_end\") (result i32) "));
    attempt!(out.i32(attempt!(super::super::plan::number(end))));
    out.append(b")\n")
}

fn calculator_signature_address(
    out: &mut crate::output::Buffer,
    first: usize,
    descriptors: &[(u32, u32)],
    keys: &[alloc::vec::Vec<u8>],
) -> Result<(), crate::Diagnostic> {
    if descriptors.len() != keys.len() {
        return Err(crate::Diagnostic::Defective);
    }
    attempt!(out.append(b"(func (export \"signature_address\") (param $id i32) (result i32)\n"));
    // Reject a missing scratch page before any passive segment can overwrite
    // memory. The selected module imports at least sixteen 64-KiB pages.
    attempt!(out.append(b"(if (i32.lt_u (memory.size) (i32.const 16)) (then (call $fail (i32.const 4)) (return (i32.const 0))))\n"));
    for (index, key) in keys.iter().enumerate() {
        let Some(&(address, length)) = descriptors.get(index) else {
            return Err(crate::Diagnostic::Defective);
        };
        if address != 983_040 || usize::try_from(length).ok() != Some(key.len()) || length > 65_536
        {
            return Err(crate::Diagnostic::Defective);
        }
        let Some(id) = first.checked_add(index) else {
            return Err(crate::Diagnostic::Defective);
        };
        attempt!(out.append(b"(if (i32.eq (local.get $id) "));
        attempt!(out.i32(attempt!(super::super::plan::number(id))));
        attempt!(out.append(b") (then\n"));
        // The guard above reserves sixteen pages before any branch. This
        // checked key is at most 65,536 bytes at 983,040, so even its final
        // byte ends at 1,048,576; Wasm memory can grow but never shrink.
        attempt!(out.append(b"(memory.init $sig"));
        attempt!(out.index(id));
        attempt!(out.append(b" (i32.const 983040) (i32.const 0) "));
        attempt!(out.i32(length));
        attempt!(out.append(b")\n(return (i32.const 983040))))\n"));
    }
    out.append(b"(call $fail (i32.const 4))\n(i32.const 0))\n")
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; validated descriptor rows are emitted in order through the bounded sink, including an explicit runtime rejection for unknown identities; no untrusted metadata requires a Rust assertion."
)]
fn descriptor_function(
    out: &mut crate::output::Buffer,
    name: &[u8],
    first: usize,
    descriptors: &[(u32, u32)],
    is_length: bool,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(func (export \""));
    attempt!(out.append(name));
    attempt!(out.append(b"\") (param $id i32) (result i32)\n"));
    let mut index = 0usize;
    let mut failure = None;
    while index < descriptors.len() {
        let entry = match first.checked_add(index) {
            Some(id) => descriptor_entry(out, id, descriptors[index], is_length),
            None => Err(crate::Diagnostic::Defective),
        };
        match entry {
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

#[cfg(test)]
mod tests {
    #![allow(
        tigerstyle::function_length,
        reason = "Owner: noble-wasm-maintainers; this fixture instantiates one module and asserts the complete exported signature range in a single scenario."
    )]

    #[test]
    fn calculator_observes_exact_signatures_at_sixteen_page_scratch_boundary() {
        // ID 0 was published by an earlier module; this one carries IDs 1..4.
        let keys = alloc::vec![
            alloc::vec![b'a', b'"', b'\\', 0],
            alloc::vec![b'b', b'"', b'\\', 0],
            alloc::vec![b'z'; 65_536],
        ];
        let rows = core::iter::once((983_040, 9))
            .chain(keys.iter().map(|key| (983_040, key.len() as u32)))
            .collect::<alloc::vec::Vec<_>>();
        let mut out = crate::output::Buffer::new(131_072);
        assert!(out.append(b"(module\n(import \"noble\" \"memory\" (memory 16 64))\n(func $fail (param i32))\n").is_ok());
        for (index, key) in keys.iter().enumerate() {
            assert!(super::signature_segment(&mut out, index + 1, key).is_ok());
        }
        assert!(super::descriptors(&mut out, &rows, Some((1, &keys))).is_ok());
        assert!(out.append(b")\n").is_ok());
        let wat = out.finish();
        let wasm = wat::parse_bytes(&wat)
            .unwrap_or_else(|_| panic!("bounded signature WAT did not assemble"));
        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::new(&engine, wasm.as_ref())
            .unwrap_or_else(|_| panic!("selected signature WAT did not validate"));

        let mut short_store = wasmtime::Store::new(&engine, ());
        let short_memory =
            wasmtime::Memory::new(&mut short_store, wasmtime::MemoryType::new(15, Some(64)))
                .unwrap_or_else(|_| panic!("fifteen-page memory failed to allocate"));
        let mut short_linker = wasmtime::Linker::new(&engine);
        short_linker
            .define(&mut short_store, "noble", "memory", short_memory)
            .unwrap_or_else(|_| panic!("fifteen-page import failed to register"));
        assert!(
            short_linker.instantiate(&mut short_store, &module).is_err(),
            "the selected module must not import less than sixteen pages"
        );

        let mut store = wasmtime::Store::new(&engine, ());
        let memory = wasmtime::Memory::new(&mut store, wasmtime::MemoryType::new(16, Some(64)))
            .unwrap_or_else(|_| panic!("sixteen-page memory failed to allocate"));
        let mut linker = wasmtime::Linker::new(&engine);
        linker
            .define(&mut store, "noble", "memory", memory)
            .unwrap_or_else(|_| panic!("selected memory import failed to register"));
        let instance = linker
            .instantiate(&mut store, &module)
            .unwrap_or_else(|_| panic!("sixteen-page selected module refused"));
        let address = instance
            .get_typed_func::<i32, i32>(&mut store, "signature_address")
            .unwrap_or_else(|_| panic!("signature address export missing"));
        let length = instance
            .get_typed_func::<i32, i32>(&mut store, "signature_length")
            .unwrap_or_else(|_| panic!("signature length export missing"));
        let range = |name: &str, store: &mut wasmtime::Store<()>| {
            instance
                .get_typed_func::<(), i32>(&mut *store, name)
                .unwrap_or_else(|_| panic!("signature range export missing"))
                .call(store, ())
                .ok()
        };
        assert_eq!(range("signature_first", &mut store), Some(1));
        assert_eq!(range("signature_end", &mut store), Some(4));
        for id in [3, 1, 2, 3] {
            let key = &keys[id as usize - 1];
            assert_eq!(address.call(&mut store, id).ok(), Some(983_040));
            assert_eq!(length.call(&mut store, id).ok(), Some(key.len() as i32));
            let end = 983_040 + key.len();
            assert_eq!(
                &memory.data(&store)[983_040..end],
                key,
                "source identity must copy complete exact canonical bytes"
            );
        }
        // A previously published ID is not re-emitted, and neither is an
        // unknown one: both refuse rather than copying another key's bytes.
        assert_eq!(address.call(&mut store, 0).ok(), Some(0));
        assert_eq!(length.call(&mut store, 0).ok(), Some(0));
        assert_eq!(address.call(&mut store, 4).ok(), Some(0));

        let oversized = alloc::vec![b'z'; 65_537];
        let mut rejected = crate::output::Buffer::new(128);
        assert!(matches!(
            super::signature_segment(&mut rejected, 0, &oversized),
            Err(crate::Diagnostic::Exhausted)
        ));
        assert!(matches!(
            super::calculator_signature_address(
                &mut rejected,
                0,
                &[(983_040, 65_537)],
                &[oversized]
            ),
            Err(crate::Diagnostic::Defective)
        ));
        assert!(matches!(
            super::descriptors(&mut rejected, &rows, Some((5, &[][..]))),
            Err(crate::Diagnostic::Defective)
        ));
    }

    #[test]
    fn ordinary_signature_address_still_returns_its_original_descriptor() {
        let mut out = crate::output::Buffer::new(1024);
        assert!(out.append(b"(module\n(func $fail (param i32))\n").is_ok());
        assert!(super::descriptors(&mut out, &[(12_345, 7)], None).is_ok());
        assert!(out.append(b")\n").is_ok());
        let wat = out.finish();
        let wasm = wat::parse_bytes(&wat)
            .unwrap_or_else(|_| panic!("ordinary descriptor WAT did not assemble"));
        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::new(&engine, wasm.as_ref())
            .unwrap_or_else(|_| panic!("ordinary descriptor WAT did not validate"));
        let mut store = wasmtime::Store::new(&engine, ());
        let instance = wasmtime::Linker::new(&engine)
            .instantiate(&mut store, &module)
            .unwrap_or_else(|_| panic!("ordinary descriptor module refused"));
        let address = instance
            .get_typed_func::<i32, i32>(&mut store, "signature_address")
            .unwrap_or_else(|_| panic!("ordinary address export missing"));
        let length = instance
            .get_typed_func::<i32, i32>(&mut store, "signature_length")
            .unwrap_or_else(|_| panic!("ordinary length export missing"));
        assert_eq!(address.call(&mut store, 0).ok(), Some(12_345));
        assert_eq!(length.call(&mut store, 0).ok(), Some(7));
        assert_eq!(address.call(&mut store, 1).ok(), Some(0));
    }
}
