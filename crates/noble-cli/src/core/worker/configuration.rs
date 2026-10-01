pub(super) fn build(
    options: &super::super::arguments::Options,
) -> Result<std::string::String, super::super::output::Failure> {
    let destination = attempt!(artifact_destination(options));
    payload(options, destination.as_deref())
}

/// Artifact admission never inherits a declared binding or persistent session.
pub(super) fn admission(optimized: bool) -> std::string::String {
    crate::workflow::encoding::object([
        ("selection", crate::workflow::encoding::string(super::SELECTION)),
        ("abi", crate::workflow::encoding::string(super::ABI)),
        ("optimized", crate::workflow::encoding::Json::Bool(optimized)),
        ("artifacts", crate::workflow::encoding::Json::Null),
        ("declared_modules", crate::workflow::encoding::Json::Bool(false)),
        ("bindings", crate::workflow::encoding::Json::Array(std::vec::Vec::new())),
        ("declared_extension", crate::workflow::encoding::Json::Null),
    ]).encode()
}

/// The worker receives a canonical artifact path; its creation precedes launch.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; canonicalize resolves the emission directory through fallible filesystem I/O and to_string_lossy creates owned path text."
)]
fn artifact_destination(
    options: &super::super::arguments::Options,
) -> Result<Option<std::string::String>, super::super::output::Failure> {
    match &options.emit {
        Some(path) => {
            let path =
                attempt!(std::fs::canonicalize(path).map_err(super::super::framing::io_error));
            Ok(Some(path.to_string_lossy().into_owned()))
        }
        None => Ok(None),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; seven fixed ABI fields must be constructed in protocol order. The manifest and emission path remain checked data, so synthetic assertions would add no independent invariant."
)]
fn payload(
    options: &super::super::arguments::Options,
    destination: Option<&str>,
) -> Result<std::string::String, super::super::output::Failure> {
    let selection = crate::workflow::encoding::string(super::SELECTION);
    let abi = crate::workflow::encoding::string(super::ABI);
    let artifacts = crate::workflow::encoding::optional_string(destination);
    let bindings = attempt!(bindings(options));
    Ok(crate::workflow::encoding::object([
        ("selection", selection),
        ("abi", abi),
        (
            "optimized",
            crate::workflow::encoding::Json::Bool(options.optimized),
        ),
        ("artifacts", artifacts),
        (
            "declared_modules",
            crate::workflow::encoding::Json::Bool(options.declared_modules),
        ),
        ("bindings", bindings),
        (
            "declared_extension",
            crate::workflow::encoding::optional_string(
                options.declared_modules.then_some(super::DECLARED_ABI),
            ),
        ),
    ])
    .encode())
}

fn bindings(options: &super::super::arguments::Options) -> Result<crate::workflow::encoding::Json, super::super::output::Failure> {
    match &options.manifest {
        Some(manifest) => manifest.worker(),
        None => Ok(crate::workflow::encoding::Json::Array(std::vec::Vec::new())),
    }
}
