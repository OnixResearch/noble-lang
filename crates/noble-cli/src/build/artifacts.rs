//! `--out` emission. A fresh directory is created and never overwritten.

pub(super) struct Emission<'a> {
    pub(super) contract: &'a [u8],
    pub(super) wrapper: Option<&'a str>,
    pub(super) artifact_wat: Option<&'a [u8]>,
    pub(super) artifact_wasm: Option<&'a [u8]>,
    pub(super) proof: Option<&'a [u8]>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; emit creates a fresh destination with create_dir and writes only present artifacts with create_new. An existing destination and partial filesystem failures are reportable Failure, not assertion preconditions."
)]
pub(super) fn emit(
    path: &std::path::Path,
    report: &mut super::report::Document,
    emission: &Emission<'_>,
) -> Result<(), crate::workflow::output::Failure> {
    attempt!(std::fs::create_dir(path).map_err(|error| {
        crate::workflow::output::Failure::error(
            "emit-destination",
            std::format!("new output directory {}: {error}", path.display()),
        )
    }));
    attempt!(super::super::workflow::artifacts::write_source(
        &path.join("contract.noble"),
        emission.contract
    ));
    if let Some(wrapper) = emission.wrapper {
        attempt!(super::super::workflow::artifacts::write_source(
            &path.join("wrapper.noble"),
            wrapper.as_bytes()
        ));
    }
    if let Some(wat) = emission.artifact_wat {
        attempt!(super::super::workflow::artifacts::write_source(
            &path.join("artifact.wat"),
            wat
        ));
    }
    if let Some(wasm) = emission.artifact_wasm {
        attempt!(super::super::workflow::artifacts::write_source(
            &path.join("artifact.wasm"),
            wasm
        ));
    }
    if let Some(proof) = emission.proof {
        attempt!(super::super::workflow::artifacts::write_source(
            &path.join("proof.lean"),
            proof
        ));
    }
    report.emitted = Some(path.to_string_lossy().into_owned());
    let mut encoded = report.json().encode();
    encoded.push('\n');
    super::super::workflow::artifacts::write_source(&path.join("report.json"), encoded.as_bytes())
}
