//! `noble_contracts::prepare`: the contract frontend never reports `Internal`,
//! is deterministic and is monotone in its budget.
#![no_main]
#![feature(register_tool)]
#![register_tool(tigerstyle)]

#[allow(dead_code)]
#[path = "../../tests/fuzz/oracles.rs"]
mod oracles;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Err(violation) = oracles::check_prepare(data) {
        panic!("{violation}");
    }
});
