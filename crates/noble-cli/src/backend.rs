//! Experimental M3 compiler shell. Execution and measurement are external.

pub const USAGE: &str = "noble wasm-experiment <wasm-gc|managed-linear-memory|reject-owner-slot>";

pub fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    match execute(arguments) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            std::process::ExitCode::from(2)
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; argument, preparation, backend and stdout failures return explicit shell errors; experimental input and broken output streams must not trigger assertion panics."
)]
fn execute(arguments: &[std::ffi::OsString]) -> Result<(), &'static str> {
    if arguments.len() != 2 {
        return Err(USAGE);
    }
    let name = arguments[1].to_str();
    if name == Some("reject-owner-slot") {
        return reject_owner_slot();
    }
    let representation = match name {
        Some("wasm-gc") => noble_wasm::Representation::WasmGc,
        Some("managed-linear-memory") => noble_wasm::Representation::ManagedLinearMemory,
        Some(_) | None => return Err(USAGE),
    };
    let prepared = match noble_contracts::prepare(
        include_bytes!("backend/vocabulary.noble"),
        noble_contracts::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        },
    ) {
        Ok(prepared) => prepared,
        Err(_) => return Err("M3 vocabulary preparation failed"),
    };
    let module = match noble_wasm::compile(representation, prepared.request(), prepared.candidate())
    {
        Ok(module) => module,
        Err(error) => return Err(diagnostic_message(error)),
    };
    match std::io::Write::write_all(&mut std::io::stdout().lock(), &module) {
        Ok(()) => Ok(()),
        Err(_) => Err("could not write M3 module"),
    }
}

const fn diagnostic_message(error: noble_wasm::Diagnostic) -> &'static str {
    match error {
        noble_wasm::Diagnostic::Invalid => "M3 backend rejected an invalid candidate",
        noble_wasm::Diagnostic::Exhausted => "M3 backend preparation quota exhausted",
        noble_wasm::Diagnostic::Unsupported => "M3 backend does not support this vocabulary",
        noble_wasm::Diagnostic::Defective => "M3 backend detected a defective environment",
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this executable negative control requires both real backend calls to return Invalid before reporting rejection; unexpected acceptance or any other failure returns a shell error rather than panicking."
)]
fn reject_owner_slot() -> Result<(), &'static str> {
    let candidate = noble_kernel::untrusted::Candidate {
        format: 1,
        revision: 0,
        nodes: std::vec::Vec::new(),
        body: std::vec::Vec::new(),
    };
    let request = noble_kernel::untrusted::Request {
        input_bytes: 0,
        expected: noble_kernel::untrusted::Expected {
            stack_in: std::vec![
                noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE),
                noble_kernel::types::Ty::I64,
            ],
            stack_out: std::vec![noble_kernel::types::Ty::I64],
            allowed_effects: noble_kernel::types::EffSet::empty(),
        },
        limits: noble_kernel::untrusted::Limits {
            bytes: 1024,
            nodes: 256,
            depth: 64,
            type_size: 512,
            stack_height: 128,
            work: 100_000,
            diagnostics: 8,
        },
    };
    let representations = [
        noble_wasm::Representation::WasmGc,
        noble_wasm::Representation::ManagedLinearMemory,
    ];
    let mut index = 0usize;
    while index < representations.len() {
        match noble_wasm::compile(representations[index], &request, &candidate) {
            Err(noble_wasm::Diagnostic::Invalid) => {}
            Ok(_) | Err(_) => return Err("owner-slot adaptation was not rejected as invalid"),
        }
        index += 1;
    }
    println!(
        "{{\"stage\":\"backend-interface-check\",\"outcome\":\"reject\",\
         \"artifact_emitted\":false,\"guest_requests\":0,\"protected_operations\":0}}"
    );
    Ok(())
}
