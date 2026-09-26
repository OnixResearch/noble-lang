const TWO: &[u8] = br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"clock","ready":false,"exit":"withdraw"},{"name":"alarm","ready":true,"exit":"trap"}]}"#;

fn begin_import_finish(
    session: &noble_syndicate::choreography::Session,
    token: &noble_syndicate::choreography::ParticipantToken,
    invocation: (noble_syndicate::choreography::Export, &str, Option<bool>),
    import: noble_syndicate::choreography::Import<'_>,
    result: bool,
) -> Result<(), noble_syndicate::choreography::Error> {
    let (export, name, ready) = invocation;
    session.begin(token, export, name, ready)?;
    assert_eq!(
        session.import(token, import),
        Ok(noble_syndicate::choreography::ImportResult::Boolean(result))
    );
    session.finish(
        token,
        noble_syndicate::choreography::InvocationOutcome::Returned(result),
    )?;
    Ok(())
}

#[path = "choreography/admission.rs"]
mod admission;
#[path = "choreography/failures.rs"]
mod failures;
#[path = "choreography/flow.rs"]
mod flow;
#[path = "choreography/ingress.rs"]
mod ingress;
