pub(super) fn diagnostic(
    error: noble_contracts::component::Error,
) -> crate::workflow::output::Failure {
    let message = std::format!("{:?}: {}", error.stage, error.diagnostic.message);
    match error.diagnostic.kind {
        noble_contracts::DiagnosticKind::Unsupported => {
            crate::workflow::output::Failure::unsupported("component-check", message)
        }
        noble_contracts::DiagnosticKind::Invalid
        | noble_contracts::DiagnosticKind::Exhausted
        | noble_contracts::DiagnosticKind::Internal => {
            crate::workflow::output::Failure::error("component-check", message)
        }
    }
}

pub(super) fn backend(error: noble_wasm::Diagnostic) -> crate::workflow::output::Failure {
    match error {
        noble_wasm::Diagnostic::Unsupported => crate::workflow::output::Failure::unsupported(
            "component-lowering",
            "unsupported component source lowering".into(),
        ),
        noble_wasm::Diagnostic::Invalid => crate::workflow::output::Failure::error(
            "component-lowering",
            "component export or environment rejected".into(),
        ),
        noble_wasm::Diagnostic::Exhausted => crate::workflow::output::Failure::error(
            "component-lowering",
            "component preparation budget exhausted".into(),
        ),
        noble_wasm::Diagnostic::Defective => crate::workflow::output::Failure::error(
            "component-lowering",
            "component lowering invariant failed".into(),
        ),
    }
}

pub(super) fn bindings(
    world: &noble_contracts::component::World,
) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::object([
        (
            "schema",
            crate::workflow::encoding::string("noble-component/v1"),
        ),
        (
            "profile",
            crate::workflow::encoding::string(noble_contracts::component::PROFILE),
        ),
        (
            "outcome",
            crate::workflow::encoding::string("typed-bindings"),
        ),
        ("world", crate::workflow::encoding::string(world.identity())),
        (
            "imports",
            crate::workflow::encoding::Json::Array(world.imports().iter().map(operation).collect()),
        ),
        (
            "exports",
            crate::workflow::encoding::Json::Array(world.exports().iter().map(operation).collect()),
        ),
        (
            "resources",
            crate::workflow::encoding::Json::Array(
                world
                    .resources()
                    .iter()
                    .map(|resource| {
                        crate::workflow::encoding::object([
                            (
                                "identity",
                                crate::workflow::encoding::string(&resource.identity),
                            ),
                            (
                                "kind",
                                crate::workflow::encoding::Json::Number(u64::from(resource.kind.0)),
                            ),
                            ("data", crate::workflow::encoding::Json::Bool(false)),
                            ("capture", crate::workflow::encoding::Json::Bool(false)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "build_context_hex",
            crate::workflow::encoding::string(crate::workflow::encoding::hex(
                &world.build_context(),
            )),
        ),
        (
            "component_emitted",
            crate::workflow::encoding::Json::Bool(false),
        ),
    ])
}

fn operation(operation: &noble_contracts::component::Operation) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::object([
        (
            "identity",
            crate::workflow::encoding::string(&operation.identity),
        ),
        ("word", crate::workflow::encoding::string(&operation.word)),
        (
            "export",
            crate::workflow::encoding::string(&operation.export_name),
        ),
        ("input_types", types(&operation.input_types())),
        ("output_types", types(&operation.output_types())),
        (
            "wit_parameters",
            crate::workflow::encoding::Json::Array(
                operation
                    .parameters
                    .iter()
                    .map(|ty| crate::workflow::encoding::string(std::format!("{ty:?}")))
                    .collect(),
            ),
        ),
        (
            "wit_results",
            crate::workflow::encoding::Json::Array(
                operation
                    .results
                    .iter()
                    .map(|ty| crate::workflow::encoding::string(std::format!("{ty:?}")))
                    .collect(),
            ),
        ),
        (
            "effects",
            crate::workflow::encoding::Json::Array(match operation.effect {
                Some(_) => std::vec![crate::workflow::encoding::string(&operation.identity)],
                None => std::vec::Vec::new(),
            }),
        ),
    ])
}

fn types(types: &[noble_kernel::types::Ty]) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::Json::Array(
        types
            .iter()
            .map(|ty| crate::workflow::encoding::string(std::format!("{ty:?}")))
            .collect(),
    )
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; report construction checks the component byte-count conversion and serializes already independently accepted metadata; it grants no host authority or proof acceptance."
)]
pub(super) fn compiled(
    world: &noble_contracts::component::World,
    artifact: &noble_wasm::component::Artifact,
    sources: &[super::Source],
    component: &super::artifacts::Bundle,
    output: &std::path::Path,
) -> Result<crate::workflow::encoding::Json, crate::workflow::output::Failure> {
    let component_bytes = attempt!(u64::try_from(component.binary.len()).map_err(|error| {
        crate::workflow::output::Failure::error("component-size", error.to_string())
    }));
    Ok(crate::workflow::encoding::object([
        (
            "schema",
            crate::workflow::encoding::string("noble-component/v1"),
        ),
        (
            "profile",
            crate::workflow::encoding::string(noble_contracts::component::PROFILE),
        ),
        ("outcome", crate::workflow::encoding::string("compiled")),
        ("world", crate::workflow::encoding::string(world.identity())),
        (
            "component_emitted",
            crate::workflow::encoding::Json::Bool(true),
        ),
        (
            "independent_kernel_check",
            crate::workflow::encoding::Json::Bool(true),
        ),
        (
            "build_context_hex",
            crate::workflow::encoding::string(crate::workflow::encoding::hex(
                artifact.build_context(),
            )),
        ),
        (
            "component_bytes",
            crate::workflow::encoding::Json::Number(component_bytes),
        ),
        (
            "exports",
            crate::workflow::encoding::Json::Array(
                sources
                    .iter()
                    .enumerate()
                    .map(|(index, source)| {
                        crate::workflow::encoding::object([
                            ("name", crate::workflow::encoding::string(&source.name)),
                            (
                                "source",
                                crate::workflow::encoding::string(std::format!(
                                    "export-{index}.noble"
                                )),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "output",
            crate::workflow::encoding::string(output.to_string_lossy()),
        ),
        (
            "assembler",
            crate::workflow::encoding::string(super::artifacts::WASM_TOOLS),
        ),
        (
            "abi",
            crate::workflow::encoding::string("wasm-tools-1.245.1-sync-memory32-utf8"),
        ),
    ]))
}
