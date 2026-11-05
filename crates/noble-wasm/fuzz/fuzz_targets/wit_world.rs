//! `World::parse` and `World::parse_checked_u64`: the WIT parser never reports
//! `Internal`, is deterministic and is monotone in its budget.
#![no_main]
#![feature(register_tool)]
#![register_tool(tigerstyle)]

#[allow(dead_code)]
#[path = "../../tests/fuzz/oracles.rs"]
mod oracles;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Err(violation) = oracles::check_wit_world(data) {
        panic!("{violation}");
    }
});
