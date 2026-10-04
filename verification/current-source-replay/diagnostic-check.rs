use noble_contracts::source::{ModuleSession, Stage};
use noble_contracts::{DiagnosticKind, Limits, Span};
use noble_kernel::contracts::FIXTURE_RESOURCE;
use noble_kernel::types::{ResourceKind, Ty};

const LIMITS: Limits = Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let source = args.next().ok_or("usage: diagnostic-check SOURCE")?;
    if args.next().is_some() || source != "dup" {
        return Err("expected exactly the canonical DX-01 resource-duplication source".into());
    }
    if FIXTURE_RESOURCE != ResourceKind(0) {
        return Err("the reserved test.counter resource kind changed".into());
    }
    let session = ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let error = session
        .prepare(source.as_bytes(), &[Ty::Resource(FIXTURE_RESOURCE)], LIMITS)
        .expect_err("resource duplication produced a candidate");
    if error.stage() != Stage::Acceptance || error.diagnostic().kind != DiagnosticKind::Invalid {
        return Err(format!("wrong source/kernel refusal: {error:?}"));
    }
    let diagnostic = error.diagnostic();
    let join = diagnostic.join().ok_or("missing structured word diagnostic")?;
    if join.word != "dup"
        || join.expected_stack != "S Data"
        || join.actual_stack != "S Resource(ResourceKind(0))"
        || join.constraint != "eligibility:Data"
        || join.value_origin.is_some()
        || diagnostic.span != (Span { start: 0, end: 3 })
    {
        return Err(format!("wrong typed source/kernel diagnosis: {error:?}"));
    }
    println!("{{\"source\":\"dup\",\"input_stack\":[\"Resource<test.counter>\"],\"stage\":\"acceptance\",\"outcome\":\"invalid\",\"word_or_join\":\"dup\",\"required_stack\":\"S Data\",\"actual_stack\":\"S Resource(ResourceKind(0))\",\"constraint\":\"eligibility:Data\",\"source_span\":{{\"start\":0,\"end\":3}},\"value_origin_or_unavailable\":\"unavailable\",\"guest_requests\":0,\"protected_operations\":0}}");
    Ok(())
}
