//! Independent, static S-CASE-04 resource-eligibility check. This peer prepares
//! source and exports, but never lowers, instantiates, or runs a component.

use noble_contracts::component::{self, World};
use noble_contracts::source::{self, Session};
use noble_contracts::{Diagnostic, DiagnosticKind, Limits};
use noble_kernel::types::{ResourceKind, Ty};

const LIMITS: Limits = Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};
const SUM_SOURCE: &str =
    "false [ counters.open inl ] [ 7 inr ] if [ counters.close ] [ dup drop ] case";
const OPEN_CLOSE: &str = "counters.open counters.close";
const PAIR_CLOSE: &str = "\"tag\" counters.open pair unpair counters.close swap drop";
const DATA_REFLECT: &str = "7 true pair quote reflect";

fn json(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len() + 2);
    encoded.push('"');
    for ch in text.chars() {
        match ch {
            '"' => encoded.push_str("\\\""),
            '\\' => encoded.push_str("\\\\"),
            '\u{0}'..='\u{1f}' => {
                encoded.push_str("\\u00");
                let digits = b"0123456789abcdef";
                let byte = ch as u8;
                encoded.push(digits[(byte >> 4) as usize] as char);
                encoded.push(digits[(byte & 15) as usize] as char);
            }
            _ => encoded.push(ch),
        }
    }
    encoded.push('"');
    encoded
}

fn positive_source(session: &Session, source: &str, output: &Ty) -> Result<(), String> {
    let prepared = session
        .prepare(source.as_bytes(), &[], LIMITS)
        .map_err(|error| format!("{source}: source preparation failed: {error:?}"))?;
    if prepared.is_definition()
        || prepared.submission().is_none()
        || prepared.output() != std::slice::from_ref(output)
    {
        return Err(format!(
            "{source}: expected executable output {output:?}, got {:?}",
            prepared.output()
        ));
    }
    Ok(())
}

fn positive_export(world: &World, source: &str) -> Result<(), String> {
    let checked = world
        .prepare_export("check", source.as_bytes(), LIMITS)
        .map_err(|error| format!("{source}: export preparation failed: {error:?}"))?;
    if checked.submission().is_none() {
        return Err(format!("{source}: missing checked export submission"));
    }
    Ok(())
}

fn eligibility(diagnostic: &Diagnostic, word: &str, actual: &str) -> Result<(), String> {
    if diagnostic.kind != DiagnosticKind::Invalid {
        return Err(format!("{word}: wrong diagnostic kind: {diagnostic:?}"));
    }
    let join = diagnostic
        .join()
        .ok_or_else(|| format!("{word}: no structured eligibility diagnostic: {diagnostic:?}"))?;
    if join.word != word
        || join.expected_stack != "S Data"
        || join.actual_stack != actual
        || join.constraint != "eligibility:Data"
        || join.value_origin.is_some()
    {
        return Err(format!(
            "{word}: wrong whole-value eligibility diagnostic: {join:?} (expected {actual})"
        ));
    }
    Ok(())
}

fn negative(world: &World, session: &Session, source: &str, word: &str, ty: &Ty) -> Result<(), String> {
    let actual = format!("S {ty:?}");
    let source_error = match session.prepare(source.as_bytes(), &[], LIMITS) {
        Ok(prepared) => {
            return Err(format!(
                "{source}: source accepted in place of resource refusal: {:?}",
                prepared.output()
            ));
        }
        Err(error) => error,
    };
    if source_error.stage() != source::Stage::Acceptance {
        return Err(format!("{source}: wrong source stage: {source_error:?}"));
    }
    eligibility(source_error.diagnostic(), word, &actual)?;

    // No unwrap_err: CheckedExport intentionally has no Debug implementation.
    let export_error = match world.prepare_export("check", source.as_bytes(), LIMITS) {
        Ok(_) => return Err(format!("{source}: component export accepted a resource")),
        Err(error) => error,
    };
    if export_error.stage != component::Stage::Export {
        return Err(format!("{source}: wrong export stage: {export_error:?}"));
    }
    eligibility(&export_error.diagnostic, word, &actual)?;
    Ok(())
}

fn unrelated_source(
    world: &World,
    session: &Session,
    source: &str,
    expected_stage: source::Stage,
) -> Result<(), String> {
    let error = match session.prepare(source.as_bytes(), &[], LIMITS) {
        Ok(_) => return Err(format!("{source}: unrelated source control was accepted")),
        Err(error) => error,
    };
    if error.stage() != expected_stage
        || error.diagnostic().kind != DiagnosticKind::Invalid
        || error
            .diagnostic()
            .join()
            .is_some_and(|join| join.constraint == "eligibility:Data")
    {
        return Err(format!("{source}: wrong unrelated source refusal: {error:?}"));
    }
    let export_error = match world.prepare_export("check", source.as_bytes(), LIMITS) {
        Ok(_) => return Err(format!("{source}: unrelated export control was accepted")),
        Err(error) => error,
    };
    if export_error.stage != component::Stage::Export
        || export_error.diagnostic.kind != DiagnosticKind::Invalid
        || export_error
            .diagnostic
            .join()
            .is_some_and(|join| join.constraint == "eligibility:Data")
    {
        return Err(format!("{source}: wrong unrelated export refusal: {export_error:?}"));
    }
    Ok(())
}

fn main() -> Result<(), String> {
    // The gate supplies the same immutable source files to this typed peer
    // and to the CLI. Their bytes must match the canonical cases exactly.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 18 {
        return Err(
            "usage: check WIT_PATH NEG_0_0 ... NEG_3_3 POSITIVE_SUM_PATH".into(),
        );
    }
    let wit_path = &args[0];
    let wit = std::fs::read(wit_path).map_err(|error| format!("{wit_path}: {error}"))?;
    let world = World::parse(&wit, "demo", LIMITS)
        .map_err(|error| format!("resource WIT did not parse: {error:?}"))?;
    if world.identity() != "noble-test:recursive-resource/demo@1.0.0" {
        return Err(format!("wrong selected WIT world: {}", world.identity()));
    }
    let [resource] = world.resources() else {
        return Err(format!("expected exactly one WIT resource: {:?}", world.resources()));
    };
    let kind = resource.kind;
    if kind != ResourceKind(1)
        || resource.identity != "noble-test:recursive-resource/counters@1.0.0#counter"
        || resource.interface != "counters"
        || resource.name != "counter"
    {
        return Err(format!("wrong parsed resource identity or kind: {resource:?}"));
    }
    let [open, close] = world.imports() else {
        return Err(format!("expected just the open/close WIT imports: {:?}", world.imports()));
    };
    if open.word != "counters.open"
        || !open.parameters.is_empty()
        || !matches!(open.results.as_slice(), [component::Type::Own(found)] if *found == kind)
        || close.word != "counters.close"
        || !matches!(close.parameters.as_slice(), [component::Type::Own(found)] if *found == kind)
        || !matches!(close.results.as_slice(), [component::Type::S64])
    {
        return Err(format!("WIT imports do not bind R to open/close: {open:?} {close:?}"));
    }
    let [check] = world.exports() else {
        return Err(format!("expected just the check WIT export: {:?}", world.exports()));
    };
    if check.export_name != "check"
        || !check.parameters.is_empty()
        || !matches!(check.results.as_slice(), [component::Type::S64])
    {
        return Err(format!("wrong closed WIT check signature: {check:?}"));
    }
    let session = world
        .session()
        .map_err(|error| format!("world bindings failed: {error:?}"))?;
    let live = Ty::Resource(kind);
    let prefixes = [
        ("Resource<R>", "counters.open", live.clone()),
        (
            "Pair<Text,Resource<R>>",
            "\"tag\" counters.open pair",
            Ty::Pair(Box::new(Ty::Text), Box::new(live.clone())),
        ),
        (
            "Sum<Resource<R>,I64>",
            "false [ counters.open inl ] [ 7 inr ] if",
            Ty::Sum(Box::new(live.clone()), Box::new(Ty::I64)),
        ),
        (
            "List<Resource<R>>",
            "counters.open nil cons",
            Ty::List(Box::new(live)),
        ),
    ];
    let operations = [
        ("dup", "dup", "dup"),
        ("drop", "drop", "drop"),
        ("quote", "quote", "quote"),
        ("generic-serialization", "quote reflect", "quote"),
    ];
    let mut prefix_rows = Vec::with_capacity(prefixes.len());
    let mut negative_rows = Vec::with_capacity(prefixes.len() * operations.len());
    for (prefix_index, (label, prefix, ty)) in prefixes.iter().enumerate() {
        positive_source(&session, prefix, ty)?;
        if ty.is_data() {
            return Err(format!("{label}: resource-bearing type became Data: {ty:?}"));
        }
        prefix_rows.push(format!(
            "{{\"type\":{},\"actual_type\":{},\"source\":{}}}",
            json(label),
            json(&format!("{ty:?}")),
            json(prefix)
        ));
        for (operation_index, (operation, suffix, word)) in operations.iter().enumerate() {
            let path = &args[1 + prefix_index * operations.len() + operation_index];
            let bytes = std::fs::read(path).map_err(|error| format!("{path}: {error}"))?;
            let canonical = format!("{prefix} {suffix}");
            if bytes != canonical.as_bytes() {
                return Err(format!("{path}: not the exact canonical {label}/{operation} source"));
            }
            let source = std::str::from_utf8(&bytes).map_err(|error| format!("{path}: {error}"))?;
            negative(&world, &session, source, word, ty)?;
            negative_rows.push(format!(
                "{{\"type\":{},\"operation\":{},\"word\":{},\"actual_stack\":{},\"constraint\":\"eligibility:Data\"}}",
                json(label),
                json(operation),
                json(word),
                json(&format!("S {ty:?}"))
            ));
        }
    }
    let sum_path = &args[17];
    let sum_bytes = std::fs::read(sum_path).map_err(|error| format!("{sum_path}: {error}"))?;
    if sum_bytes != SUM_SOURCE.as_bytes() {
        return Err(format!("{sum_path}: not the exact selected static sum control"));
    }
    let sum_source =
        std::str::from_utf8(&sum_bytes).map_err(|error| format!("{sum_path}: {error}"))?;
    positive_source(&session, sum_source, &Ty::I64)?;
    positive_export(&world, sum_source)?;
    positive_source(&session, OPEN_CLOSE, &Ty::I64)?;
    positive_export(&world, OPEN_CLOSE)?;
    positive_source(&session, PAIR_CLOSE, &Ty::I64)?;
    positive_export(&world, PAIR_CLOSE)?;
    positive_source(&session, DATA_REFLECT, &Ty::Syntax)?;

    let unsupported_wit =
        "package noble-test:recursive-resource@1.0.0; world demo { export check: func() -> u32; }";
    let unsupported = match World::parse(unsupported_wit.as_bytes(), "demo", LIMITS) {
        Ok(_) => return Err("unsupported u32 WIT shape was admitted".into()),
        Err(error) => error,
    };
    if unsupported.stage != component::Stage::Wit
        || unsupported.diagnostic.kind != DiagnosticKind::Unsupported
        || unsupported.diagnostic.join().is_some()
    {
        return Err(format!("wrong unsupported WIT refusal: {unsupported:?}"));
    }
    let unbound_source = "7 not-a-word";
    let type_source = "true 1 +";
    unrelated_source(&world, &session, unbound_source, source::Stage::Resolve)?;
    unrelated_source(&world, &session, type_source, source::Stage::Check)?;

    let controls = format!(
        "{{\"wit_unsupported\":{{\"source\":{},\"stage\":\"wit\",\"kind\":\"unsupported\",\"eligibility_join\":false}},\"source_unbound\":{{\"source\":{},\"stage\":\"resolve\",\"kind\":\"invalid\",\"eligibility_join\":false,\"export_stage\":\"export\"}},\"source_type\":{{\"source\":{},\"stage\":\"check\",\"kind\":\"invalid\",\"eligibility_join\":false,\"export_stage\":\"export\"}},\"resource_open_close\":{{\"source\":{},\"output_type\":\"I64\"}},\"resource_pair\":{{\"source\":{},\"output_type\":\"I64\"}},\"data_quote_reflect\":{{\"source\":{},\"output_type\":\"Syntax\"}},\"list_constructor\":{{\"source\":{},\"output_type\":{},\"lowered\":false}}}}",
        json(unsupported_wit),
        json(unbound_source),
        json(type_source),
        json(OPEN_CLOSE),
        json(PAIR_CLOSE),
        json(DATA_REFLECT),
        json(prefixes[3].1),
        json(&format!("{:?}", prefixes[3].2)),
    );
    println!(
        "{{\"schema\":\"noble-scase04-typed/v1\",\"case_id\":\"S-CASE-04\",\"resource_kind\":{},\"prefixes\":[{}],\"negatives\":[{}],\"positive_sum\":{{\"source\":{},\"output_type\":\"I64\",\"lowered\":false}},\"controls\":{}}}",
        kind.0,
        prefix_rows.join(","),
        negative_rows.join(","),
        json(sum_source),
        controls
    );
    Ok(())
}
