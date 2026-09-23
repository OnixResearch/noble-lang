// Same explicit bounded-depth traversal as workflow::encoding, over parsed
// worker observations. Public detail omits all cell handles; raw slots retain them.
#[octet::sealed_enum]
enum Frame<'a> {
    Value(&'a crate::core::report::Value),
    Array(&'a [crate::core::report::Value], bool),
    Object(
        std::collections::btree_map::Iter<'a, std::string::String, crate::core::report::Value>,
        bool,
    ),
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; a closed iterative traversal serializes every parsed Value into growable output, retaining one tail per open container. Public-detail filtering and escaping are total operations, not assertion preconditions."
)]
pub(in crate::core::companions) fn text_of(
    value: &crate::core::report::Value,
) -> std::string::String {
    let mut out = std::string::String::with_capacity(256);
    let mut pending = std::vec::Vec::with_capacity(16);
    pending.push(Frame::Value(value));
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Value(value) => append(value, &mut out, &mut pending),
            Frame::Array(values, is_first) => match values.split_first() {
                None => out.push(']'),
                Some((value, rest)) => {
                    if !is_first {
                        out.push(',');
                    }
                    pending.push(Frame::Array(rest, false));
                    pending.push(Frame::Value(value));
                }
            },
            Frame::Object(mut entries, is_first) => {
                match entries.find(|(key, _)| key.as_str() != "handle") {
                    None => out.push('}'),
                    Some((key, value)) => {
                        if !is_first {
                            out.push(',');
                        }
                        crate::workflow::encoding::quote_json(key, &mut out);
                        out.push(':');
                        pending.push(Frame::Object(entries, false));
                        pending.push(Frame::Value(value));
                    }
                }
            }
        }
    }
    out
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; JSON emission appends into caller-owned growable String and traversal Vec buffers, which cannot be replaced by fixed slices."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; runtime number formatting and appending to String and Vec buffers are not const on the pinned compiler."
)]
fn append<'a>(
    value: &'a crate::core::report::Value,
    out: &mut std::string::String,
    pending: &mut std::vec::Vec<Frame<'a>>,
) {
    match value {
        crate::core::report::Value::Null => out.push_str("null"),
        crate::core::report::Value::Bool(value) => {
            out.push_str(if *value { "true" } else { "false" })
        }
        crate::core::report::Value::Number(value) => out.push_str(&value.to_string()),
        crate::core::report::Value::Text(value) => {
            crate::workflow::encoding::quote_json(value, out)
        }
        crate::core::report::Value::Array(values) => {
            out.push('[');
            pending.push(Frame::Array(values, true));
        }
        crate::core::report::Value::Object(entries) => {
            out.push('{');
            pending.push(Frame::Object(entries.iter(), true));
        }
    }
}

/// The value a guard-checked invocation returned. A wrapper reports the Sum of
/// its decision, so an accepted invocation reports the subject's own value.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; querying runtime BTreeMap report members and serializing into owned JSON strings are not const on the pinned compiler."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; missing or ill-typed worker stacks become JSON null, and Sum payloads are selected only after checked member lookups. Untrusted report shape must not be asserted."
)]
pub(in crate::core::companions) fn top_payload(
    report: &crate::core::report::Value,
) -> crate::workflow::encoding::Json {
    let Some(entry) = report
        .member("stack")
        .and_then(crate::core::report::Value::items)
        .and_then(|items| items.last())
    else {
        return crate::workflow::encoding::Json::Null;
    };
    let is_sum = entry
        .member("type")
        .and_then(crate::core::report::Value::text)
        .map(|text| text == "Sum")
        .unwrap_or(false);
    if is_sum {
        if let Some(value) = entry.member("value") {
            return crate::workflow::encoding::string(
                crate::core::companions::reporting::values::text_of(value),
            );
        }
    }
    crate::workflow::encoding::string(crate::core::companions::reporting::values::text_of(entry))
}

/// One stack slot as `(type name, cell handle)`.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; a missing worker stack returns Refused and optional type/handle fields retain their existing empty/None representation after typed lookups. External report validity must not be asserted."
)]
pub(in crate::core::companions) fn slots(
    report: &crate::core::report::Value,
) -> crate::core::companions::Attempt<std::vec::Vec<(std::string::String, Option<u64>)>> {
    let entries = attempt!(report
        .member("stack")
        .and_then(crate::core::report::Value::items)
        .ok_or_else(|| {
            crate::core::companions::Refused::new(
                "malformed",
                "engine-report",
                std::string::String::from("reply has no stack"),
            )
        }));
    let mut out = std::vec::Vec::with_capacity(entries.len());
    let mut entry_index = 0;
    while entry_index < entries.len() {
        let entry = &entries[entry_index];
        let kind = entry
            .member("type")
            .and_then(crate::core::report::Value::text)
            .unwrap_or_default();
        let handle = entry
            .member("handle")
            .and_then(crate::core::report::Value::index);
        out.push((std::string::String::from(kind), handle));
        entry_index += 1;
    }
    Ok(out)
}

/// The complete bounded reflection events one observation produced.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; absent events, invalid event fields, unsigned value parsing and out-of-domain kinds return typed Refused errors. Engine-supplied data must be validated rather than asserted."
)]
pub(in crate::core::companions) fn reflection_events(
    report: &crate::core::report::Value,
) -> crate::core::companions::Attempt<std::vec::Vec<(u32, u64)>> {
    let entries = attempt!(report
        .member("events")
        .and_then(crate::core::report::Value::items)
        .ok_or_else(|| {
            crate::core::companions::Refused::new(
                "malformed",
                "engine-report",
                std::string::String::from("reply has no events"),
            )
        }));
    let mut out = std::vec::Vec::with_capacity(entries.len());
    let mut entry_index = 0;
    while entry_index < entries.len() {
        let entry = &entries[entry_index];
        let kind = attempt!(entry
            .member("kind")
            .and_then(crate::core::report::Value::index)
            .ok_or_else(|| {
                crate::core::companions::Refused::new(
                    "malformed",
                    "engine-report",
                    std::string::String::from("event without kind"),
                )
            }));
        let raw = attempt!(entry
            .member("value")
            .and_then(crate::core::report::Value::text)
            .ok_or_else(|| {
                crate::core::companions::Refused::new(
                    "malformed",
                    "engine-report",
                    std::string::String::from("event without value"),
                )
            }));
        let value = attempt!(raw.parse::<u64>().map_err(|_| {
            crate::core::companions::Refused::new(
                "malformed",
                "engine-report",
                std::string::String::from("event value is not an unsigned 64-bit integer"),
            )
        }));
        let kind =
            attempt!(
                u32::try_from(kind).map_err(|_| crate::core::companions::Refused::script(
                    "event kind exceeds the engine kind domain"
                ))
            );
        out.push((kind, value));
        entry_index += 1;
    }
    Ok(out)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; a missing worker stack returns Refused and optional fields are encoded through typed lookups. Growable output includes sanitized detail without asserting untrusted worker structure."
)]
pub(in crate::core::companions) fn stack_json(
    report: &crate::core::report::Value,
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let entries = attempt!(report
        .member("stack")
        .and_then(crate::core::report::Value::items)
        .ok_or_else(|| {
            crate::core::companions::Refused::new(
                "malformed",
                "engine-report",
                std::string::String::from("reply has no stack"),
            )
        }));
    let mut out = std::vec::Vec::with_capacity(entries.len());
    let mut entry_index = 0;
    while entry_index < entries.len() {
        let entry = &entries[entry_index];
        out.push(crate::workflow::encoding::object([
            (
                "type",
                crate::workflow::encoding::string(
                    entry
                        .member("type")
                        .and_then(crate::core::report::Value::text)
                        .unwrap_or_default(),
                ),
            ),
            (
                "handle",
                match entry
                    .member("handle")
                    .and_then(crate::core::report::Value::index)
                {
                    Some(handle) => crate::workflow::encoding::Json::Number(handle),
                    None => crate::workflow::encoding::Json::Null,
                },
            ),
            (
                "detail",
                crate::workflow::encoding::string(
                    crate::core::companions::reporting::values::text_of(entry),
                ),
            ),
        ]));
        entry_index += 1;
    }
    Ok(crate::workflow::encoding::Json::Array(out))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; runtime BTreeMap and Option lookups feed an owned JSON string serializer, whose allocation and formatting APIs are not const on the pinned compiler."
)]
pub(in crate::core::companions) fn top_value(
    report: &crate::core::report::Value,
) -> crate::workflow::encoding::Json {
    match report
        .member("stack")
        .and_then(crate::core::report::Value::items)
        .and_then(|items| items.last())
    {
        Some(entry) => crate::workflow::encoding::string(
            crate::core::companions::reporting::values::text_of(entry),
        ),
        None => crate::workflow::encoding::Json::Null,
    }
}
