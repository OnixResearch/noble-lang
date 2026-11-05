//! The source frontend (lexer, parser, declared modules and linking) under the
//! Core, host-free Core, module and calculator profiles: never `Internal`,
//! deterministic and monotone in its budget.
#![no_main]
#![feature(register_tool)]
#![register_tool(tigerstyle)]

#[allow(dead_code)]
#[path = "../../tests/fuzz/oracles.rs"]
mod oracles;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Err(violation) = oracles::check_source(data) {
        panic!("{violation}");
    }
});
