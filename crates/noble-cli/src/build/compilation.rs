/// Compilation, assembly and loading failures all block artifact release.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; prepare rejects invalid or exhausted source, non-expression submissions, lowering failures and assembly/loading failures through typed Failure. User programs and host tool failures must remain reportable rather than become assertion panics."
)]
pub(super) fn prepare(source: &[u8]) -> Result<super::Artifact, crate::workflow::output::Failure> {
    let frontend = noble_contracts::source::Session::new();
    let prepared = match frontend.prepare(source, &[], crate::core::SOURCE_LIMITS) {
        Ok(prepared) => prepared,
        Err(error) => {
            let kind = error.diagnostic().kind;
            let mut failure = crate::workflow::output::Failure::error(
                "artifact-source",
                std::format!("artifact source not accepted: {kind:?}"),
            );
            if kind == noble_contracts::DiagnosticKind::Exhausted {
                failure.outcome = crate::workflow::output::Outcome::Unknown;
                failure.code = "preparation-exhausted";
            }
            return Err(failure);
        }
    };
    let Some(submission) = prepared.submission() else {
        return Err(crate::workflow::output::Failure::error(
            "artifact-source",
            "artifact source is a definition, not an expression".into(),
        ));
    };
    let wat = match noble_wasm::source::Compiler::new().prepare(submission) {
        Ok(compiled) => compiled.wat().to_vec(),
        Err(error) => {
            let mut failure = crate::workflow::output::Failure::error(
                "artifact-lowering",
                "the independent compiler rejected the artifact source".into(),
            );
            if matches!(error, noble_wasm::Diagnostic::Exhausted) {
                failure.outcome = crate::workflow::output::Outcome::Unknown;
                failure.code = "preparation-exhausted";
            }
            return Err(failure);
        }
    };
    let wasm =
        attempt!(
            assemble(&wat).map_err(|message| crate::workflow::output::Failure::unsupported(
                "artifact-assembly",
                message
            ))
        );
    Ok(super::Artifact {
        prepared,
        wat,
        wasm,
    })
}

/// Assemble WAT with the selected worker in a private disposable directory.
/// The module bytes outlive the directory; a cleanup failure never invalidates
/// the returned bytes.
fn assemble(wat: &[u8]) -> Result<std::vec::Vec<u8>, std::string::String> {
    let directory = attempt!(
        crate::workflow::artifacts::Temporary::create().map_err(|failure| failure.message)
    );
    crate::core::assemble_wat(&directory.path, wat)
}
