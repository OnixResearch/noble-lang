pub(super) fn describe(
    prepared: &noble_contracts::Prepared,
    source: &[u8],
) -> super::encoding::Json {
    super::encoding::object([
        ("name", super::encoding::string(prepared.name())),
        (
            "candidate_format",
            super::encoding::Json::Number(u64::from(prepared.candidate().format)),
        ),
        (
            "semantic_revision",
            super::encoding::Json::Number(u64::from(prepared.candidate().revision)),
        ),
        ("inputs", bindings(prepared.inputs())),
        ("outputs", bindings(prepared.outputs())),
        ("ghost_parameters", bindings(prepared.params())),
        ("logical_definitions", definitions(prepared)),
        ("requires", predicate(prepared, source, prepared.requires())),
        ("ensures", predicate(prepared, source, prepared.ensures())),
        ("typed_ir", expressions(prepared)),
        (
            "accepted_candidate",
            super::encoding::string(std::format!("{:?}", prepared.candidate())),
        ),
        (
            "acceptance_request",
            super::encoding::string(std::format!("{:?}", prepared.request())),
        ),
        (
            "stack_order",
            super::encoding::string("bottom-first; the last entry is top"),
        ),
    ])
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; predicate uses checked index conversions, expression lookup, span slicing and UTF-8 decoding. Unavailable source excerpts intentionally encode as JSON null instead of panicking during report construction."
)]
fn predicate(
    prepared: &noble_contracts::Prepared,
    source: &[u8],
    index: u32,
) -> super::encoding::Json {
    let expression = usize::try_from(index)
        .ok()
        .and_then(|index| prepared.expressions().get(index));
    let text = expression
        .and_then(|expression| {
            usize::try_from(expression.span.start)
                .ok()
                .zip(usize::try_from(expression.span.end).ok())
                .and_then(|(start, end)| source.get(start..end))
        })
        .and_then(|bytes| std::str::from_utf8(bytes).ok());
    super::encoding::object([
        (
            "expression",
            super::encoding::Json::Number(u64::from(index)),
        ),
        ("source", super::encoding::optional_string(text)),
    ])
}

fn bindings(values: &[noble_contracts::NamedType]) -> super::encoding::Json {
    super::encoding::Json::Array(
        values
            .iter()
            .map(|binding| {
                super::encoding::object([
                    ("name", super::encoding::string(&binding.name)),
                    (
                        "type",
                        super::encoding::string(std::format!("{:?}", binding.ty)),
                    ),
                ])
            })
            .collect(),
    )
}

fn definitions(prepared: &noble_contracts::Prepared) -> super::encoding::Json {
    super::encoding::Json::Array(
        prepared
            .definitions()
            .iter()
            .map(|definition| {
                super::encoding::object([
                    ("name", super::encoding::string(&definition.name)),
                    (
                        "type",
                        super::encoding::string(std::format!("{:?}", definition.ty)),
                    ),
                    (
                        "body",
                        super::encoding::Json::Number(u64::from(definition.body)),
                    ),
                ])
            })
            .collect(),
    )
}

fn expressions(prepared: &noble_contracts::Prepared) -> super::encoding::Json {
    super::encoding::Json::Array(
        prepared
            .expressions()
            .iter()
            .map(|expression| {
                super::encoding::object([
                    (
                        "kind",
                        super::encoding::string(std::format!("{:?}", expression.kind)),
                    ),
                    (
                        "type",
                        super::encoding::string(std::format!("{:?}", expression.ty)),
                    ),
                    (
                        "span",
                        super::encoding::object([
                            (
                                "start",
                                super::encoding::Json::Number(u64::from(expression.span.start)),
                            ),
                            (
                                "end",
                                super::encoding::Json::Number(u64::from(expression.span.end)),
                            ),
                        ]),
                    ),
                ])
            })
            .collect(),
    )
}
