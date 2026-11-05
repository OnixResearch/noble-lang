//! The backend accepts what the frontend accepts: every prepared contract either
//! compiles, in both representations, to WAT that parses and to a module that
//! validates, or is refused as unsupported or exhausted.
#![no_main]
#![feature(register_tool)]
#![register_tool(tigerstyle)]

#[allow(dead_code)]
#[path = "../../tests/fuzz/oracles.rs"]
mod oracles;

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    let modules = match oracles::check_prepare_to_wasm(data) {
        Ok(modules) => modules,
        Err(violation) => panic!("{violation}"),
    };
    for module in modules {
        let mut validator =
            wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all());
        if let Err(error) = validator.validate_all(&module) {
            panic!("emitted module does not validate: {error}");
        }
    }
});
