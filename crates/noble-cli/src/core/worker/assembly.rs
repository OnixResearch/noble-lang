/// Assemble one accepted WAT module with the selected immutable tools.
///
/// The selected worker owns assembly and validation; this helper starts it once,
/// keeps both artifacts beside the caller's directory, and returns the exact
/// module bytes. A refusal returns the worker diagnostic unchanged: an
/// assembled module is tool correspondence evidence, never a verified backend.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; selected-worker startup, assembly and artifact reads return typed failures. External tool and I/O failures must not become assertion panics."
)]
pub(crate) fn assemble_wat(
    directory: &std::path::Path,
    wat: &[u8],
) -> Result<std::vec::Vec<u8>, std::string::String> {
    let options = super::super::arguments::Options {
        source: None,
        compile_only: false,
        inputs: std::vec::Vec::new(),
        modules: std::vec::Vec::new(),
        declared_modules: false,
        text_byte_cursor: false,
        bindings: None,
        manifest: None,
        framed: false,
        optimized: false,
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
    std::fs::read(directory.join("engine").join("module-1.wasm"))
        .map_err(|error| std::format!("assembled module is unavailable: {error}"))
}
