use super::*;

struct Paths {
    disabled_costs: UncachedCosts,
    disabled: Uncached,
    cold_total_ns: u64,
    cold_stages: StageCosts,
    cold: cache::Result,
    cold_backend_build_ns: u64,
    cold_compiled: Compiled,
    warm_total_ns: u64,
    warm_stages: StageCosts,
    warm: cache::Result,
    warm_backend_build_ns: u64,
    warm_compiled: Compiled,
    disabled_execution_ns: u64,
    disabled_outputs: Vec<Vec<Value>>,
    cold_execution_ns: u64,
    cold_outputs: Vec<Vec<Value>>,
    warm_execution_ns: u64,
    warm_outputs: Vec<Vec<Value>>,
}

fn run_paths(
    lowered: &Lowered,
    root: &str,
    map: &[Option<i64>],
    dynamic_values: &[Vec<Value>],
) -> Paths {
    let (disabled_costs, disabled) = prepare_uncached(lowered, map);
    let mut session = new_session();
    let (cold_total_ns, cold_stages, cold) = request(&mut session, lowered, root, map);
    let cold = cold.expect("cold cache miss");
    assert_eq!(cold.status, cache::Status::MissStored);
    let (warm_total_ns, warm_stages, warm) = request(&mut session, lowered, root, map);
    let warm = warm.expect("warm cache hit");
    assert_eq!(warm.status, cache::Status::Hit);
    assert_eq!(cold.specialization_key, warm.specialization_key);
    assert_eq!(cold.program_key, warm.program_key);

    let (cold_backend_build_ns, cold_compiled) = timed(|| Compiled::new(&BLAKE3, &cold.readmitted));
    let (warm_backend_build_ns, warm_compiled) = timed(|| Compiled::new(&BLAKE3, &warm.readmitted));
    let (disabled_execution_ns, disabled_outputs) = execute(&disabled.compiled, dynamic_values);
    let (cold_execution_ns, cold_outputs) = execute(&cold_compiled, dynamic_values);
    let (warm_execution_ns, warm_outputs) = execute(&warm_compiled, dynamic_values);

    Paths {
        disabled_costs,
        disabled,
        cold_total_ns,
        cold_stages,
        cold,
        cold_backend_build_ns,
        cold_compiled,
        warm_total_ns,
        warm_stages,
        warm,
        warm_backend_build_ns,
        warm_compiled,
        disabled_execution_ns,
        disabled_outputs,
        cold_execution_ns,
        cold_outputs,
        warm_execution_ns,
        warm_outputs,
    }
}

fn assert_equivalence(
    lowered: &Lowered,
    rows: &[Vec<i64>],
    dynamic_values: &[Vec<Value>],
    paths: &Paths,
) {
    assert_eq!(
        paths.disabled_outputs, paths.cold_outputs,
        "cold residual execution matches cache-off"
    );
    assert_eq!(
        paths.disabled_outputs, paths.warm_outputs,
        "warm residual execution matches cache-off"
    );
    for (row, dynamic) in rows.iter().zip(dynamic_values) {
        assert_row(lowered, row, dynamic, paths);
    }
}

fn assert_row(lowered: &Lowered, row: &[i64], dynamic: &[Value], paths: &Paths) {
    let original_inputs = paths
        .disabled
        .residual
        .merge_i64(row)
        .expect("dynamic vector merges");
    let original_l3 = lowered.l3(&original_inputs).expect("original L3");
    let original_l4 = lowered.l4(&original_inputs).expect("original L4");
    let disabled_l3 = pe::observe_l3(&paths.disabled.residual.checked, dynamic)
        .expect("cache-off residual L3")
        .0;
    let disabled_readmitted_l3 = pe::observe_l3(&paths.disabled.readmitted, dynamic)
        .expect("cache-off readmitted L3")
        .0;
    let disabled_readmitted_l4 = pe::observe_l4(&paths.disabled.compiled, dynamic)
        .expect("cache-off readmitted L4")
        .0;
    let cold_l3 = pe::observe_l3(&paths.cold.residual.checked, dynamic)
        .expect("cold residual L3")
        .0;
    let cold_readmitted_l4 = pe::observe_l4(&paths.cold_compiled, dynamic)
        .expect("cold readmitted L4")
        .0;
    let cold_readmitted_l3 = pe::observe_l3(&paths.cold.readmitted, dynamic)
        .expect("cold readmitted L3")
        .0;
    let warm_l3 = pe::observe_l3(&paths.warm.residual.checked, dynamic)
        .expect("warm residual L3")
        .0;
    let warm_readmitted_l4 = pe::observe_l4(&paths.warm_compiled, dynamic)
        .expect("warm readmitted L4")
        .0;
    let warm_readmitted_l3 = pe::observe_l3(&paths.warm.readmitted, dynamic)
        .expect("warm readmitted L3")
        .0;
    for observed in [
        &original_l4,
        &disabled_l3,
        &disabled_readmitted_l3,
        &disabled_readmitted_l4,
        &cold_l3,
        &cold_readmitted_l3,
        &cold_readmitted_l4,
        &warm_l3,
        &warm_readmitted_l3,
        &warm_readmitted_l4,
    ] {
        assert!(
            obs_eq_all(&original_l3, observed),
            "cache differential mismatch on {row:?}"
        );
    }
}

fn render_result(case: CaseSpec, rows: &[Vec<i64>], paths: &Paths) -> serde_json::Value {
    serde_json::json!({
        "case": {
            "root": case.root,
            "input_leaves": case.input_leaves,
            "static_map": case.static_inputs
        },
        "vectors": rows.len(),
        "agreement": "cache-disabled=cold-miss=warm-hit on L3/L4 and compiled execution",
        "cache_disabled": {
            "specialization_ns": paths.disabled_costs.specialization_ns,
            "source_emission_ns": paths.disabled_costs.source_emission_ns,
            "admission_ns": paths.disabled_costs.admission_ns,
            "backend_build_ns": paths.disabled_costs.backend_build_ns,
            "execution_ns": paths.disabled_execution_ns,
            "retained_bytes": 0,
        },
        "cold_miss": {
            "status": paths.cold.status.name(),
            "lookup_ns": paths.cold_stages.lookup_ns,
            "specialization_ns": paths.cold_stages.specialization_ns,
            "source_emission_ns": paths.cold_stages.source_emission_ns,
            "admission_ns": paths.cold_stages.admission_ns,
            "publication_ns": paths.cold_stages.publication_ns,
            "total_ns": paths.cold_total_ns,
            "backend_build_ns": paths.cold_backend_build_ns,
            "execution_ns": paths.cold_execution_ns,
            "lookup_entries": paths.cold.lookup_entries,
            "retained_bytes": paths.cold.retained_bytes,
        },
        "warm_hit": {
            "status": paths.warm.status.name(),
            "lookup_ns": paths.warm_stages.lookup_ns,
            "specialization_ns": paths.warm_stages.specialization_ns,
            "source_emission_ns": paths.warm_stages.source_emission_ns,
            "admission_ns": paths.warm_stages.admission_ns,
            "publication_ns": paths.warm_stages.publication_ns,
            "total_ns": paths.warm_total_ns,
            "backend_build_ns": paths.warm_backend_build_ns,
            "execution_ns": paths.warm_execution_ns,
            "lookup_entries": paths.warm.lookup_entries,
            "retained_bytes": paths.warm.retained_bytes,
        },
        "backend_retained_bytes": {
            "cache_disabled": paths.disabled.compiled.retained_bytes(),
            "cold_miss": paths.cold_compiled.retained_bytes(),
            "warm_hit": paths.warm_compiled.retained_bytes(),
        },
    })
}

pub(super) fn measure_fixture(case_index: usize, case: CaseSpec) -> serde_json::Value {
    let lowered =
        Lowered::new(&[], case.root, case.input_leaves).expect("retained cache fixture extracts");
    let dynamic_count = case
        .static_inputs
        .iter()
        .filter(|entry| entry.is_none())
        .count();
    let rows = vectors(VectorSpec {
        seed: fixture_seed(case.input_leaves)
            .wrapping_add(u64::try_from(case_index).expect("case index fits")),
        dynamic_inputs: dynamic_count,
        count: 1000,
    });
    let dynamic_values = rows
        .iter()
        .map(|row| row.iter().copied().map(Value::I64).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let paths = run_paths(&lowered, case.root, case.static_inputs, &dynamic_values);
    assert_equivalence(&lowered, &rows, &dynamic_values, &paths);
    render_result(case, &rows, &paths)
}
