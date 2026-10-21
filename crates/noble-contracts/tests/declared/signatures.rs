#[test]
fn stack_variables_in_value_positions_are_rejected() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[], super::retained())
        .map_err(|error| format!("{error:?}"))?;
    for ty in [
        "Pair<S,I64>",
        "Pair<I64,S>",
        "Sum<S,I64>",
        "Sum<I64,S>",
        "List<S>",
        "Choice<S,I64>",
        "Choice<I64,S>",
        "Program<S,List<S>,pure>",
        "Pair<Program<S,S,pure>,S>",
    ] {
        let interface = format!("[ S -- {ty} ! pure ]");
        let source = format!(
            "module malformed@1 [ variant Choice<A,B> Left A public Right B public signature f forall<S:stack> {interface} def f [ ] ]"
        );
        let error = match session.prepare(source.as_bytes(), &[], super::LIMITS) {
            Err(error) => error,
            Ok(_) => return Err(format!("malformed signature accepted: {ty}")),
        };
        assert_eq!(
            error.stage(),
            noble_contracts::source::Stage::Check,
            "{ty}: {error:?}"
        );
        assert_eq!(
            error.diagnostic().kind,
            noble_contracts::DiagnosticKind::Invalid,
            "{ty}: {error:?}"
        );
        let span = error.diagnostic().span;
        let start = usize::try_from(span.start).map_err(|error| error.to_string())?;
        let end = usize::try_from(span.end).map_err(|error| error.to_string())?;
        assert_eq!(
            source.get(start..end),
            Some(interface.as_str()),
            "{ty}: {error:?}"
        );
        assert_eq!(session.generation(), 0);
    }
    Ok(())
}

#[test]
fn value_binders_and_nested_program_stack_lists_remain_polymorphic() -> Result<(), String> {
    let session = noble_contracts::source::ModuleSession::new(&[], super::retained())
        .map_err(|error| format!("{error:?}"))?;
    let session = super::commit(
        session,
        b"module kinded@1 [ signature identity forall<T:stack,A:value> [ T A -- T A ! pure ] export identity def identity [ ] signature keep forall<T:stack,S:stack> [ T Pair<Program<S,S,pure>,I64> -- T Pair<Program<S,S,pure>,I64> ! pure ] export keep def keep [ ] ]",
    )?;
    let scalars = [noble_kernel::types::Ty::Text, noble_kernel::types::Ty::Bool];
    let prepared = session
        .prepare(b"kinded@1.identity", &scalars, super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(prepared.output(), &scalars);
    for segment in [
        vec![],
        vec![noble_kernel::types::Ty::Text],
        vec![noble_kernel::types::Ty::Text, noble_kernel::types::Ty::Bool],
    ] {
        let input = [noble_kernel::types::Ty::Pair(
            Box::new(noble_kernel::types::Ty::program(
                segment.clone(),
                segment,
                noble_kernel::types::EffSet::empty(),
            )),
            Box::new(noble_kernel::types::Ty::I64),
        )];
        let prepared = session
            .prepare(b"kinded@1.keep", &input, super::LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        assert_eq!(prepared.output(), &input);
    }
    Ok(())
}
