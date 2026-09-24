#[test]
fn live_async_values_remain_non_data_and_non_capture_in_nested_payloads() -> Result<(), String> {
    let world = super::world(super::ASYNC_WIT, "main")?;
    let session = world.session().map_err(|error| format!("{error:?}"))?;
    for (word, ty) in [
        ("service.bytes", noble_contracts::component::Type::StreamU8),
        (
            "service.number",
            noble_contracts::component::Type::FutureS64,
        ),
        (
            "service.outcome",
            noble_contracts::component::Type::FutureResultS64String,
        ),
    ] {
        let live = ty.noble();
        for (source, expected) in [
            (word.to_string(), live.clone()),
            (
                format!("{word} 7 pair"),
                noble_kernel::types::Ty::Pair(
                    Box::new(live.clone()),
                    Box::new(noble_kernel::types::Ty::I64),
                ),
            ),
            (
                format!("{word} nil cons"),
                noble_kernel::types::Ty::List(Box::new(live.clone())),
            ),
            (
                format!("true [ {word} inl ] [ 7 inr ] if"),
                noble_kernel::types::Ty::Sum(
                    Box::new(live.clone()),
                    Box::new(noble_kernel::types::Ty::I64),
                ),
            ),
        ] {
            let prepared = session
                .prepare(source.as_bytes(), &[], super::LIMITS)
                .map_err(|error| format!("{source}: {error:?}"))?;
            assert_eq!(prepared.output(), std::slice::from_ref(&expected));
            assert!(!expected.is_data());
            // Reflection can expose an inert recipe only after capture; no live
            // handle may enter that recipe through a generic quote.
            for operation in ["dup", "drop", "quote reflect"] {
                super::rejects_source(&session, &format!("{source} {operation}"))?;
            }
        }
    }
    let data = session
        .prepare(b"7 true pair quote reflect", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(data.output(), &[noble_kernel::types::Ty::Syntax]);
    Ok(())
}

#[test]
fn stream_completion_returns_typed_bytes_or_domain_error_without_a_live_owner() -> Result<(), String>
{
    let world = super::world(
        b"package test:drain@1.0.0; world main { import start: async func() -> stream<u8>; import drain: async func(value: stream<u8>) -> result<list<u8>, string>; export read: async func() -> result<list<u8>, string>; export succeeded: async func() -> bool; }",
        "main",
    )?;
    let prepared = world
        .prepare_export("read", b"start drain", super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let submission = prepared.submission().ok_or("missing typed stream result")?;
    let expected = noble_kernel::types::Ty::Sum(
        Box::new(noble_kernel::types::Ty::List(Box::new(
            noble_kernel::types::Ty::I64,
        ))),
        Box::new(noble_kernel::types::Ty::Text),
    );
    assert_eq!(
        submission.request.expected.stack_out,
        vec![expected.clone()]
    );
    assert!(expected.is_data());
    let environment = world.environment().map_err(|error| format!("{error:?}"))?;
    match noble_kernel::acceptance::check(
        &environment,
        &submission.request,
        &submission.body.candidate,
    ) {
        noble_kernel::untrusted::Outcome::Accepted(checked) => {
            assert_eq!(checked.interface.stack_out, vec![expected.clone()]);
        }
        outcome => {
            return Err(format!(
                "regenerated stream-result contract rejected: {outcome:?}"
            ))
        }
    }
    let status = world
        .prepare_export(
            "succeeded",
            b"start drain [ drop true ] [ drop false ] case",
            super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    let status = status
        .submission()
        .ok_or("missing terminal outcome branch")?;
    assert_eq!(
        status.request.expected.stack_out,
        vec![noble_kernel::types::Ty::Bool]
    );
    let session = world.session().map_err(|error| format!("{error:?}"))?;
    let copied = session
        .prepare(b"start drain dup drop", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(copied.output(), &[expected]);
    super::rejects_source(&session, "start dup")?;
    Ok(())
}
