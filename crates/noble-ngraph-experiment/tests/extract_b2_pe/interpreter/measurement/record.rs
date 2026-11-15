//! One program's independently timed setup, checked rows and warm comparisons.

use super::stages::{
    byte_stats, cache_request, median, original_rows, stats, timed, warm_costs, StageSamples,
};
use super::*;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::specialize::cache::{Limits, Session, Status};

type Json = serde_json::Value;

#[derive(Default)]
struct Setup {
    specialization: Vec<u64>,
    emission: Vec<u64>,
    admission: Vec<u64>,
    build: Vec<u64>,
    direct_build: Vec<u64>,
    cold: Vec<u64>,
    miss: StageSamples,
    hit: StageSamples,
    cache_bytes: Vec<u64>,
    compiled: Option<Compiled>,
    direct: Option<Interpreted>,
    residual: Option<Residual>,
    emitted: Option<String>,
    checked: Option<noble_ngraph::check::CheckedGraph>,
}

fn measure_cache(
    lowered: &Interpreted,
    map: &[Option<i64>],
    emitted: &str,
    residual: &Residual,
    checked: &noble_ngraph::check::CheckedGraph,
    setup: &mut Setup,
) {
    let mut session = Session::new(Limits::default()).expect("bounded default session");
    let miss = cache_request(&mut session, lowered, map, &mut setup.miss);
    assert_eq!(miss.status, Status::MissStored);
    let hit = cache_request(&mut session, lowered, map, &mut setup.hit);
    assert_eq!(hit.status, Status::Hit);
    assert_eq!(hit.source, emitted);
    assert_eq!(hit.readmitted.id(), checked.id());
    assert_eq!(miss.residual.checked.id(), residual.checked.id());
    assert_eq!(hit.lookup_entries, 1);
    setup.cache_bytes.push(session.retained_bytes() as u64);
}

fn measure_repeat(lowered: &Interpreted, ops: [Opcode; 2], map: &[Option<i64>], setup: &mut Setup) {
    let (elapsed, direct) = timed(|| super::direct(ops));
    setup.direct_build.push(elapsed);
    setup.direct = Some(direct);
    let budgets = Budgets::default();
    let (elapsed, residual) = timed(|| {
        specialize::specialize(
            &BLAKE3,
            &lowered.checked,
            &Request {
                static_inputs: map,
                budgets: &budgets,
            },
        )
        .expect("specialization")
    });
    setup.specialization.push(elapsed);
    super::assert_dispatch_removed(&residual, ops);
    let (elapsed, text) =
        timed(|| specialize::emit_source(&residual.checked).expect("source emission"));
    setup.emission.push(elapsed);
    let (elapsed, checked) = timed(|| {
        specialize::readmit(
            &BLAKE3,
            &text,
            &specialize::interface_types(&residual.checked).expect("interface"),
            noble_contracts::Limits::default(),
            &budgets,
        )
        .expect("independent admission")
    });
    setup.admission.push(elapsed);
    let (elapsed, compiled) = timed(|| Compiled::new(&BLAKE3, &checked));
    setup.build.push(elapsed);
    let mut workspace = compiled.workspace();
    let (elapsed, first) = timed(|| {
        compiled
            .evaluate_in(
                &mut workspace,
                &eval::i64_inputs(&[7, 2]),
                None,
                &mut Control::default(),
            )
            .obs
    });
    assert_eq!(
        first,
        Obs::Normal(vec![Value::I64(
            reference(&[ops[0].byte(), ops[1].byte()], (7, 2)).unwrap()
        )])
    );
    setup.cold.push(elapsed);
    measure_cache(lowered, map, &text, &residual, &checked, setup);
    setup.compiled = Some(compiled);
    setup.residual = Some(residual);
    setup.emitted = Some(text);
    setup.checked = Some(checked);
}

fn verify_rows(
    lowered: &Interpreted,
    setup: &Setup,
    source: &[u8; 2],
    pairs: &[(i64, i64)],
    full_rows: &[Vec<Value>],
    dynamic_rows: &[Vec<Value>],
) {
    let compiled = setup.compiled.as_ref().expect("measured graph");
    let direct = setup.direct.as_ref().expect("measured direct graph");
    for (&(x, y), (original, dynamic)) in pairs.iter().zip(full_rows.iter().zip(dynamic_rows)) {
        let expected = Obs::Normal(vec![Value::I64(reference(source, (x, y)).unwrap())]);
        assert_eq!(
            compiled
                .evaluate(dynamic, None, &mut Control::default())
                .obs,
            expected
        );
        assert_eq!(
            lowered
                .compiled
                .evaluate(original, None, &mut Control::default())
                .obs,
            expected
        );
        assert_eq!(
            direct
                .compiled
                .evaluate(dynamic, None, &mut Control::default())
                .obs,
            expected
        );
    }
}

fn setup_stats(setup: &mut Setup) -> Json {
    serde_json::json!({
        "specialization": stats(core::mem::take(&mut setup.specialization)),
        "source_emission": stats(core::mem::take(&mut setup.emission)),
        "independent_admission": stats(core::mem::take(&mut setup.admission)),
        "backend_build": stats(core::mem::take(&mut setup.build)),
        "direct_ordinary_frontend_extraction_and_build": stats(core::mem::take(&mut setup.direct_build)),
        "cold_first_residual_execution": stats(core::mem::take(&mut setup.cold)),
    })
}

fn amortization(warm: &Json, stage: &Json) -> (String, u128, u128, Option<u128>) {
    let saving = i128::from(median(&warm["interpreter"])) - i128::from(median(&warm["residual"]));
    let setup = [
        "specialization",
        "source_emission",
        "independent_admission",
        "backend_build",
    ]
    .into_iter()
    .map(|name| u128::from(median(&stage[name])))
    .sum::<u128>();
    let direct = u128::from(median(
        &stage["direct_ordinary_frontend_extraction_and_build"],
    ));
    // The interpreter graph is already built on both sides; this is the full
    // incremental residual setup, not a cache hit or a direct-source offset.
    let break_even = if saving > 0 {
        let positive = u128::try_from(saving).expect("positive savings");
        assert!(positive != 0, "the saving must be nonzero before division");
        Some(
            setup
                .saturating_mul(BATCH as u128)
                .saturating_add(positive.saturating_sub(1))
                / positive,
        )
    } else {
        None
    };
    (saving.to_string(), setup, direct, break_even)
}

fn build_entry(
    lowered: &Interpreted,
    setup: Setup,
    ops: [Opcode; 2],
    map: [Option<i64>; 8],
    pairs: &[(i64, i64)],
    warm: Json,
    stage: Json,
) -> Json {
    let (savings, incremental, direct_setup, break_even) = amortization(&warm, &stage);
    let compiled = setup.compiled.expect("compiled graph");
    let direct = setup.direct.expect("direct graph");
    let residual = setup.residual.expect("residual");
    let emitted = setup.emitted.expect("emitted source");
    let checked = setup.checked.expect("admitted graph");
    serde_json::json!({
        "source_tags": [ops[0].byte(), ops[1].byte()], "one_hot_static_map": map,
        "interpreter_graph": noble_ngraph::ir::hex(&lowered.checked.id()),
        "residual_graph": noble_ngraph::ir::hex(&residual.checked.id()),
        "residual_graph_text": noble_ngraph::format::graph::print(residual.checked.graph()),
        "readmitted_graph": noble_ngraph::ir::hex(&checked.id()),
        "direct_graph": noble_ngraph::ir::hex(&direct.checked.id()),
        "residual_source": emitted, "direct_source": direct_source(ops),
        "residual_source_bytes": emitted.len(), "original_source_bytes": INTERPRETER.len(),
        "original_nodes": lowered.checked.graph().nodes.len(), "residual_nodes": residual.report.residual_nodes,
        "variants": residual.report.variants, "cache_retained_bytes": byte_stats(setup.cache_bytes),
        "compiled_retained_bytes": {"interpreter": lowered.compiled.retained_bytes(),
            "residual": compiled.retained_bytes(), "direct": direct.compiled.retained_bytes()},
        "workspace_scratch_bytes": {"interpreter": lowered.compiled.scratch_bytes(),
            "residual": compiled.scratch_bytes(), "direct": direct.compiled.scratch_bytes()},
        "stage_ns": stage, "cache_miss_ns": setup.miss.report(), "cache_hit_ns": setup.hit.report(),
        "warm_batch_ns": warm, "batch_savings_ns": savings,
        "incremental_setup_ns": incremental, "direct_setup_ns_not_offset_against_interpretation": direct_setup,
        "break_even_calls": break_even,
        "break_even_reason_if_null": if break_even.is_none() { Some("no positive warmed per-batch savings") } else { None },
        "differential_vectors": pairs.len(), "differential_agreement": true,
    })
}

pub(super) fn measure_case(lowered: &Interpreted, ops: [Opcode; 2], pairs: &[(i64, i64)]) -> Json {
    let source = [ops[0].byte(), ops[1].byte()];
    let map = static_map(ops);
    let mut setup = Setup::default();
    for _ in 0..REPEATS {
        measure_repeat(lowered, ops, &map, &mut setup);
    }
    let dynamic_rows: Vec<Vec<Value>> = pairs
        .iter()
        .map(|&(x, y)| eval::i64_inputs(&[x, y]))
        .collect();
    let full_rows = original_rows(&map, pairs);
    verify_rows(lowered, &setup, &source, pairs, &full_rows, &dynamic_rows);
    let warm = warm_costs(
        &lowered.compiled,
        setup.compiled.as_ref().unwrap(),
        &setup.direct.as_ref().unwrap().compiled,
        &full_rows,
        &dynamic_rows,
        ops,
        pairs,
    );
    let stage = setup_stats(&mut setup);
    build_entry(lowered, setup, ops, map, pairs, warm, stage)
}
