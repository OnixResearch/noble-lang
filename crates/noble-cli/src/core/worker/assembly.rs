/// Assemble one accepted WAT module with the selected immutable tools.
///
/// The selected worker owns assembly and validation; this helper starts it once,
/// keeps both artifacts beside the caller's directory, and returns the exact
/// module bytes only after they re-hash to the identity the worker reported. A
/// refusal returns the worker diagnostic unchanged: an assembled module is tool
/// correspondence evidence, never a verified backend.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; selected-worker startup, assembly, artifact reads and the re-read identity check return typed failures. External tool and I/O failures must not become assertion panics."
)]
pub(crate) fn assemble_wat(
    directory: &std::path::Path,
    wat: &[u8],
) -> Result<Assembly, std::string::String> {
    assemble_wat_optimized(directory, wat, false)
}

/// Assemble one accepted WAT module in the selected mode. The optimizer runs
/// only when `optimized` selects it, and the emitted module file is the one
/// that mode produces.
pub(crate) fn assemble_wat_optimized(
    directory: &std::path::Path,
    wat: &[u8],
    optimized: bool,
) -> Result<Assembly, std::string::String> {
    let options = super::super::arguments::Options {
        source: None,
        compile_only: false,
        inputs: std::vec::Vec::new(),
        modules: std::vec::Vec::new(),
        declared_modules: false,
        retained_source_bytes: None,
        signed_self_recursion: false,
        size_change_evidence: false,
        size_change_certificate: None,
        size_change_lean: None,
        size_change_lean_library: None,
        text_byte_cursor: false,
        checked_i64: false,
        bindings: None,
        manifest: None,
        framed: false,
        optimized,
        type_directed_experimental: false,
        lowering_selected: false,
        emit: Some(directory.to_path_buf()),
        limits: super::super::SOURCE_LIMITS,
    };
    let mut engine = attempt!(super::Engine::start(&options).map_err(|failure| failure.message));
    let report = attempt!(engine
        .prepare(wat, &[], 1)
        .map_err(|failure| failure.message));
    if report.outcome != "ready" {
        return Err(report.json);
    }
    let module = directory.join("engine").join(if optimized {
        "module-1-optimized.wasm"
    } else {
        "module-1.wasm"
    });
    let bytes = attempt!(read_artifact(&module, MODULE_LIMIT, "assembled module"));
    // The worker reported the identities of the bytes it assembled, validated
    // and compiled. Re-reading `module-1.wasm` is only evidence while the file
    // still hashes to those identities: a substituted or truncated artifact
    // must refuse here rather than become release evidence.
    attempt!(verify_identity(&report.json, wat, &bytes));
    let tools = attempt!(read_artifact(
        &directory.join("engine").join("tools.json"),
        TOOLS_LIMIT,
        "worker tool record"
    ));
    Ok(Assembly { bytes, tools })
}

/// An assembled module and the selected worker's verified tool record.
pub(crate) struct Assembly {
    pub(crate) bytes: std::vec::Vec<u8>,
    pub(crate) tools: std::vec::Vec<u8>,
}

/// The assembled-module byte bound; the worker refuses a larger `MAX_FRAME`
/// before it writes one, so a larger file on disk is a substituted artifact.
const MODULE_LIMIT: usize = 4_194_304;

/// The worker's tool record is a few hundred bytes; the bound only refuses a
/// substituted file.
const TOOLS_LIMIT: usize = 65_536;

fn read_artifact(
    path: &std::path::Path,
    limit: usize,
    kind: &'static str,
) -> Result<std::vec::Vec<u8>, std::string::String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| std::format!("{kind} is unavailable at {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(std::format!(
            "{kind} is not a regular file: {}",
            path.display()
        ));
    }
    let length = metadata.len();
    if length > limit as u64 {
        return Err(std::format!(
            "{kind} is {length} bytes, over its {limit}-byte bound"
        ));
    }
    std::fs::read(path).map_err(|error| std::format!("{kind} is unreadable: {error}"))
}

/// Refuse unless the re-read module bytes are exactly the ones the worker
/// reported, for exactly the WAT this build offered.
fn verify_identity(report: &str, wat: &[u8], bytes: &[u8]) -> Result<(), std::string::String> {
    let record: serde_json::Value = serde_json::from_str(report)
        .map_err(|error| std::format!("the worker report is not JSON: {error}"))?;
    let module = record
        .get("module")
        .ok_or("the worker report carries no module record")?;
    let reported = |field: &str| -> Result<&str, std::string::String> {
        module
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| std::format!("the worker module record carries no {field}"))
    };
    let wat_sha256 = reported("wat_sha256")?;
    let wasm_sha256 = reported("wasm_sha256")?;
    let observed_wat = crate::workflow::intrinsic::sha256(wat);
    if observed_wat != wat_sha256 {
        return Err(std::format!(
            "the worker assembled a different WAT: offered {observed_wat}, reported {wat_sha256}"
        ));
    }
    let observed = crate::workflow::intrinsic::sha256(bytes);
    if observed != wasm_sha256 {
        return Err(std::format!(
            "the re-read module bytes differ from the worker's assembly: observed {observed}, reported {wasm_sha256}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::verify_identity;

    fn report(wat: &[u8], bytes: &[u8]) -> std::string::String {
        serde_json::json!({
            "outcome": "ready",
            "module": {
                "wasm_sha256": crate::workflow::intrinsic::sha256(bytes),
                "wat_sha256": crate::workflow::intrinsic::sha256(wat),
            },
        })
        .to_string()
    }

    #[test]
    fn the_re_read_bytes_must_be_the_assembled_bytes() {
        let wat = b"(module)";
        let bytes = b"\0asm\x01\0\0\0";
        let report = report(wat, bytes);
        assert!(verify_identity(&report, wat, bytes).is_ok());
        let substituted = [0u8; 9];
        let refused = verify_identity(&report, wat, &substituted);
        let Err(message) = refused else {
            std::panic!("a substituted module must be refused");
        };
        assert!(message.contains("re-read module bytes differ"), "{message}");
    }

    #[test]
    fn a_worker_that_assembled_other_wat_is_refused() {
        let wat = b"(module)";
        let bytes = b"\0asm\x01\0\0\0";
        let report = report(b"(module (unexpected))", bytes);
        let refused = verify_identity(&report, wat, bytes);
        let Err(message) = refused else {
            std::panic!("a different assembled WAT must be refused");
        };
        assert!(message.contains("assembled a different WAT"), "{message}");
    }

    #[test]
    fn a_report_without_module_identities_is_refused() {
        for report in [
            "not json".to_owned(),
            serde_json::json!({ "outcome": "ready" }).to_string(),
            serde_json::json!({ "module": { "wasm_sha256": "00" } }).to_string(),
            serde_json::json!({ "module": { "wat_sha256": "00" } }).to_string(),
        ] {
            let refused = verify_identity(&report, b"(module)", b"\0asm");
            assert!(refused.is_err(), "{report} is refused");
        }
    }
}
