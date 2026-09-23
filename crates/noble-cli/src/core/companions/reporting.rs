pub(super) mod inputs;
pub(super) mod values;

pub(in crate::core::companions) const fn evidence_class_name(
    class: noble_contracts::companion::EvidenceClass,
) -> &'static str {
    match class {
        noble_contracts::companion::EvidenceClass::LeanExact => "lean-exact",
        noble_contracts::companion::EvidenceClass::Replay => "replay",
        noble_contracts::companion::EvidenceClass::LeanRefutation => "lean-refutation",
        noble_contracts::companion::EvidenceClass::Assumption => "assumption",
    }
}

/// The compiler's closed diagnostic set as report text.
pub(in crate::core::companions) const fn backend_diagnostic(
    diagnostic: noble_wasm::Diagnostic,
) -> &'static str {
    match diagnostic {
        noble_wasm::Diagnostic::Invalid => "the candidate is invalid",
        noble_wasm::Diagnostic::Exhausted => "the compilation budget is exhausted",
        noble_wasm::Diagnostic::Unsupported => "the candidate is outside Core-Bootstrap",
        noble_wasm::Diagnostic::Defective => "the compiler refused a defective module",
    }
}

pub(in crate::core::companions) fn parse_report(
    document: &str,
) -> crate::core::companions::Attempt<crate::core::report::Value> {
    crate::core::report::parse(document).map_err(|message| {
        crate::core::companions::Refused::new("malformed", "engine-report", message)
    })
}

/// One report line. `extras` carries the operation's own observable fields.
pub(in crate::core::companions) struct Header<'a> {
    pub operation: &'static str,
    pub outcome: &'a str,
}

pub(in crate::core::companions) fn line(
    header: crate::core::companions::reporting::Header<'_>,
    extras: std::vec::Vec<(&'static str, crate::workflow::encoding::Json)>,
) -> crate::workflow::encoding::Json {
    let mut entries: std::vec::Vec<(&'static str, crate::workflow::encoding::Json)> =
        std::vec::Vec::with_capacity(extras.len().saturating_add(4));
    entries.push((
        "schema",
        crate::workflow::encoding::string("noble-companions-report/v1"),
    ));
    entries.push((
        "operation",
        crate::workflow::encoding::string(header.operation),
    ));
    entries.push(("outcome", crate::workflow::encoding::string(header.outcome)));
    entries.extend(extras);
    crate::workflow::encoding::Json::Object(entries)
}

pub(in crate::core::companions) fn emit(
    report: &crate::workflow::encoding::Json,
) -> Result<(), std::string::String> {
    let mut text = report.encode();
    text.push('\n');
    let mut stdout = std::io::stdout().lock();
    std::io::Write::write_all(&mut stdout, text.as_bytes())
        .and_then(|()| std::io::Write::flush(&mut stdout))
        .map_err(|error| std::format!("stdout: {error}"))
}

pub(in crate::core::companions) fn refusal_line(
    operation: &'static str,
    refusal: &crate::core::companions::Refused,
) -> crate::workflow::encoding::Json {
    crate::core::companions::reporting::line(
        crate::core::companions::reporting::Header {
            operation,
            outcome: refusal.outcome,
        },
        std::vec::Vec::from([
            (
                "diagnostic",
                crate::workflow::encoding::object([
                    ("code", crate::workflow::encoding::string(&refusal.code)),
                    (
                        "message",
                        crate::workflow::encoding::string(&refusal.message),
                    ),
                ]),
            ),
            ("prover_calls", crate::workflow::encoding::Json::Number(0)),
            ("guest_requests", crate::workflow::encoding::Json::Number(0)),
            (
                "protected_operations",
                crate::workflow::encoding::Json::Number(0),
            ),
            (
                "implicit_evidence_fetches",
                crate::workflow::encoding::Json::Number(0),
            ),
            (
                "candidate_body_started",
                crate::workflow::encoding::Json::Bool(false),
            ),
        ]),
    )
}

pub(in crate::core::companions) fn index_of(
    token: &str,
    height: usize,
) -> Result<usize, crate::core::companions::Refused> {
    let index = attempt!(token.parse::<usize>().map_err(|_| {
        crate::core::companions::Refused::script(std::format!("'{token}' is not a stack index"))
    }));
    if index >= height {
        return Err(crate::core::companions::Refused::new(
            "malformed",
            "stack-index",
            std::format!("stack index {index} is outside the {height}-slot stack"),
        ));
    }
    Ok(index)
}

pub(in crate::core::companions) fn integer_of(
    token: &str,
) -> Result<i64, crate::core::companions::Refused> {
    token.parse::<i64>().map_err(|_| {
        crate::core::companions::Refused::script(std::format!(
            "'{token}' is not a signed 64-bit integer"
        ))
    })
}

pub(in crate::core::companions) fn runtime_count(
    report: &crate::core::report::Value,
    field: &str,
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    report
        .member(field)
        .and_then(crate::core::report::Value::index)
        .map(crate::workflow::encoding::Json::Number)
        .ok_or_else(|| {
            crate::core::companions::Refused::script(std::format!("runtime omitted {field}"))
        })
}

pub(in crate::core::companions) fn program_interfaces(
    ty: &noble_kernel::types::Ty,
) -> crate::core::companions::Attempt<(
    std::vec::Vec<noble_kernel::types::Ty>,
    std::vec::Vec<noble_kernel::types::Ty>,
)> {
    match ty {
        noble_kernel::types::Ty::Program(input, output, _effects) => {
            Ok(((**input).clone(), (**output).clone()))
        }
        _ => Err(crate::core::companions::Refused::new(
            "malformed",
            "stack-type",
            std::string::String::from("stack slot is not a program"),
        )),
    }
}

pub(in crate::core::companions) fn nested_number(
    value: &crate::core::report::Value,
    path: [&str; 2],
) -> crate::core::companions::Attempt<u32> {
    let [outer, inner] = path;
    value
        .member(outer)
        .and_then(|value| value.member(inner))
        .and_then(crate::core::report::Value::index)
        .and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| {
            crate::core::companions::Refused::script(std::format!(
                "companion {outer} has no {inner}"
            ))
        })
}
