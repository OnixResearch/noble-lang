//! DHAT-only process scopes for the frozen #32 heap-high-water protocol.
//! Each selected libtest invocation runs exactly one graph or Program leg.

use noble_kernel::types::Ty;
use noble_ngraph::check::Budgets;
use noble_ngraph::extract::source;
use noble_ngraph::specialize::{self, offset, program, Request};

use super::super::common::IDS;
use super::fixtures::{graph, Fixture};

fn graph_fixture(name: &str) -> Fixture {
    match name {
        "chained-add" => Fixture::Add,
        "chained-sub" => Fixture::Sub,
        "wrapping-boundary" => Fixture::Wrap,
        "shared-subexpression" => Fixture::Shared,
        "non-prefix-static" => Fixture::Nonprefix,
        "dead-total-forward" => Fixture::Dead,
        "growth-cap" => Fixture::Growth,
        other => panic!("unknown frozen graph fixture {other}"),
    }
}

fn run_graph(name: &str, side: &str) {
    let (original, map) = graph(graph_fixture(name));
    if side == "setup" {
        std::hint::black_box(original);
        return;
    }
    let budgets = Budgets::default();
    let request = Request {
        static_inputs: &map,
        budgets: &budgets,
    };
    let checked = match side {
        "baseline" => {
            specialize::specialize(&IDS, &original, &request)
                .expect("D3 residual")
                .checked
        }
        "offset" => {
            offset::compare(&IDS, &original, &request)
                .expect("opt-in normalized residual")
                .normalized
        }
        other => panic!("unknown frozen graph side {other}"),
    };
    let source = specialize::emit_source(&checked).expect("residual source");
    let readmitted = specialize::readmit(
        &IDS,
        &source,
        &specialize::interface_types(&checked).expect("ordered interface"),
        noble_contracts::Limits::default(),
        &budgets,
    )
    .expect("ordinary independently checked source");
    assert_eq!(
        checked.graph().outputs.len(),
        readmitted.graph().outputs.len()
    );
    std::hint::black_box((checked, readmitted));
}

struct ProgramCase {
    root: &'static str,
    definitions: &'static [&'static str],
    inputs: Vec<Ty>,
    tail: usize,
    map: Vec<Option<i64>>,
}

fn nested_pair() -> Ty {
    Ty::Pair(
        Box::new(Ty::I64),
        Box::new(Ty::Pair(Box::new(Ty::Unit), Box::new(Ty::I64))),
    )
}

fn program_fixture(name: &str) -> ProgramCase {
    match name {
        "opaque-tail" => ProgramCase {
            root: "swap -",
            definitions: &[],
            inputs: vec![Ty::Text, Ty::Bool, Ty::I64, Ty::I64],
            tail: 2,
            map: vec![Some(5), None],
        },
        "nested-partial-pair" => ProgramCase {
            root: "drop drop drop dup unpair unpair swap drop +",
            definitions: &[],
            inputs: vec![Ty::Text, nested_pair(), Ty::Unit, Ty::Bool, Ty::I64],
            tail: 1,
            map: vec![Some(2), None, None, Some(4)],
        },
        "named-repeated-call" => ProgramCase {
            root: "sq sq",
            definitions: &["def sq [ dup * ]"],
            inputs: vec![Ty::Text, Ty::I64],
            tail: 1,
            map: vec![Some(5)],
        },
        other => panic!("unknown frozen Program fixture {other}"),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this isolated runtime heap probe performs ordinary source preparation, admission and residualization."
)]
fn run_program(name: &str, side: &str) {
    let case = program_fixture(name);
    let prepared = source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        case.definitions,
        case.root,
        &case.inputs,
    )
    .expect("ordinary source preparation");
    if side == "setup" {
        std::hint::black_box(prepared);
        return;
    }
    assert_eq!(side, "program", "only the frozen D13 residualization leg");
    let residual = program::residualize(
        &IDS,
        &program::Request {
            original: &prepared.submission,
            claimed_closure: &prepared.claimed_closure,
            expected_original: None,
            preserved_tail: case.tail,
            static_inputs: &case.map,
            budgets: &Budgets::default(),
            source_limits: noble_contracts::Limits::default(),
        },
    )
    .expect("D13 ordinary residual Program and independent readmission");
    assert!(residual.prepared.submission().is_some());
    std::hint::black_box(residual);
}

#[test]
#[ignore = "frozen process-global DHAT heap scope; requires NOBLE_PE_HEAP_CASE and NOBLE_PE_HEAP_SIDE"]
#[expect(
    tigerstyle::ambient_env,
    reason = "Owner: noble-maintainers; the ignored source-bound heap producer selects one frozen fixture and side per DHAT process."
)]
fn profile_case() {
    let name = std::env::var("NOBLE_PE_HEAP_CASE").expect("frozen case name");
    let side = std::env::var("NOBLE_PE_HEAP_SIDE").expect("frozen side");
    if matches!(
        name.as_str(),
        "opaque-tail" | "nested-partial-pair" | "named-repeated-call"
    ) {
        run_program(&name, &side);
    } else {
        run_graph(&name, &side);
    }
    println!("PE_HEAP:{name}:{side}:checked");
}
