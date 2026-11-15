use super::*;

pub(super) fn build_row(input: ReportInput<'_>) -> Result<(String, ArmMetrics, bool), String> {
    let ReportInput {
        namespace,
        arm,
        seed,
        library,
        sources,
        build_ns,
        sweeps,
        control,
        target_set,
    } = input;
    let final_counts = sweeps
        .last()
        .map(|(_, counts)| counts)
        .ok_or("transfer arm has no sweep")?;
    let mut retrieval_queries = 0u32;
    let mut retrieval_examined = 0u32;
    let mut max_examined = 0u32;
    let mut retrieval_ns = 0u64;
    let mut proposer_attempts = 0u32;
    let mut teacher_attempts = 0u32;
    for (_, counts) in sweeps {
        retrieval_queries = retrieval_queries.saturating_add(counts.retrieval_queries);
        retrieval_examined = retrieval_examined.saturating_add(counts.retrieval_examined);
        max_examined = max_examined.max(counts.max_examined);
        retrieval_ns = retrieval_ns.saturating_add(counts.retrieval_ns);
        proposer_attempts = proposer_attempts.saturating_add(counts.proposer_attempts);
        teacher_attempts = teacher_attempts.saturating_add(counts.teacher_attempts);
    }
    if proposer_attempts != 0 || teacher_attempts != 0 {
        return Err(String::from(
            "a scored transfer arm attempted a runtime proposer or teacher call",
        ));
    }
    let encoded = library
        .encode()
        .map_err(|error| format!("library encode: {error:?}"))?;
    let empty_bytes = ProcedureLibrary::new(super::revision(7))
        .encode()
        .map_err(|error| format!("empty library encode: {error:?}"))?
        .len();
    let manifest = library
        .identity_manifest()
        .map_err(|error| format!("identity manifest: {error:?}"))?;
    let restarted = ProcedureLibrary::load(&encoded, namespace)
        .map_err(|error| format!("restart load: {error:?}"))?;
    let is_restart_identity_match = restarted
        .identity_manifest()
        .map_err(|error| format!("restart manifest: {error:?}"))?
        == manifest
        && restarted
            .canonical_chunk_order()
            .map_err(|error| format!("restart chunk order: {error:?}"))?
            == library
                .canonical_chunk_order()
                .map_err(|error| format!("chunk order: {error:?}"))?;
    let manifest_text =
        core::str::from_utf8(&manifest).map_err(|error| format!("manifest UTF-8: {error}"))?;
    for task in target_set.tasks {
        if manifest_text.contains(task.name) {
            return Err(format!(
                "target id {} leaked into procedure identity manifest",
                task.name
            ));
        }
    }
    if library.records().len() != sources.len()
        || library
            .records()
            .iter()
            .any(|record| record.provenance.producer == Producer::Composition)
    {
        return Err(String::from(
            "composed held-out target was retained in the procedure-library index",
        ));
    }
    if library.records().iter().any(|record| {
        record.chunk.interface.stack_in.len() != 2 || record.chunk.interface.stack_out != [Ty::I64]
    }) {
        return Err(String::from(
            "a three-input target or non-source interface entered the acquisition index",
        ));
    }
    let record_count = u32::try_from(library.records().len())
        .map_err(|_| String::from("library record count exceeds the metrics domain"))?;
    let source_count = u32::try_from(sources.len())
        .map_err(|_| String::from("source count exceeds the metrics domain"))?;
    let generation_count = u32::try_from(library.generation_records().len())
        .map_err(|_| String::from("generation count exceeds the metrics domain"))?;
    let added_bytes = encoded
        .len()
        .checked_sub(empty_bytes)
        .ok_or_else(|| String::from("retained library is smaller than the empty baseline"))?;
    let growth_bytes_added = u32::try_from(added_bytes)
        .map_err(|_| String::from("library growth exceeds the metrics byte domain"))?;
    let metrics_arm = ArmMetrics {
        arm: String::from(arm),
        seed,
        acquisition_entries: record_count,
        acquisition_tasks: source_count,
        surviving_generations: generation_count,
        held_out_tasks: 2,
        transfer_passed: final_counts.transfer_passed,
        earlier_tasks: final_counts.earlier_tasks,
        regression_failed: final_counts.regression_failed,
        growth_entries_added: record_count,
        growth_bytes_added,
        retrieval_queries,
        retrieval_examined,
        max_examined,
        retrieval_overhead_ns: retrieval_ns,
        restart_identity_match: is_restart_identity_match,
        proposer_calls: 0,
        teacher_calls: 0,
        break_even_invocations: None,
        break_even_reason: String::from("not estimable: the lane measures retrieval overhead only and does not measure a per-invocation execution-cost difference"),
    };
    let metrics = Metrics {
        index_build_steps: record_count,
        index_build_monotonic_ns: build_ns,
        arms: vec![metrics_arm.clone()],
        model_synthesis: if target_set.programs.iter().all(Option::is_none) {
            String::from("source candidates, grammar revision and certificates are fixed by verification/synthesis/receipts/2026-11-13/synthesis-v2.json; synthesis is not run in this lane")
        } else {
            String::from("acquisition candidates come from the fixed #116 source receipt; target candidates come from the separately bound #116 producer-target receipt; synthesis is not run during scored arms")
        },
        executed_mechanics: String::from("retrieval identities, source composition, fresh Session preparation, typed-chunk/1 re-admission and held-out fixture execution"),
        correspondence: String::from("no source-to-graph correspondence or end-to-end synthesis correctness claim"),
        assumptions: if target_set.programs.iter().all(Option::is_none) {
            String::from("held-out formulas and inputs are frozen; composition order is supplied by task description")
        } else {
            String::from("held-out references, inputs and composition order were frozen before the producer target run")
        },
    };
    let metrics_json = core::str::from_utf8(
        &library
            .metrics_record(&metrics)
            .map_err(|error| format!("metrics record: {error:?}"))?,
    )
    .map_err(|error| format!("metrics UTF-8: {error}"))?
    .to_string();
    let manifest_json = manifest_text.to_string();
    let is_control = arm == "size_matched_nontransfer";
    let is_control_size_matched = !is_control || control.within_ten_percent;
    let mut sweeps_json = String::from("[");
    for (index, (sweep, _)) in sweeps.iter().enumerate() {
        if index != 0 {
            sweeps_json.push(',');
        }
        sweeps_json.push_str(sweep);
    }
    sweeps_json.push(']');
    let output = format!(
        "{{\"arm\":\"{arm}\",\"seed\":{},\"generation_count\":{},\"record_count\":{},\"canonical_library_bytes\":{},\"identity_manifest\":{manifest_json},\"metrics_record\":{metrics_json},\"restart_identity_match\":{},\"control_candidate_pool\":[\"arithScaled\",\"arithProduct\",\"bitXor\"],\"control_pair_excluded\":\"bitXor has a non-null producer precondition\",\"selected_control_pair\":[\"{}\",\"{}\"],\"control_target_bytes\":{},\"control_candidate_bytes\":{},\"control_size_match\":{},\"proposer_calls\":0,\"teacher_calls\":0,\"proposer_attempts\":{proposer_attempts},\"teacher_attempts\":{teacher_attempts},\"teacher_refusals\":0,\"heldout_tasks_passed\":{},\"heldout_tasks_total\":2,\"heldout_examples_passed\":{},\"heldout_examples_total\":{},\"sweeps\":{sweeps_json}}}",
        seed, library.generation_records().len(), library.records().len(), encoded.len(),
        is_restart_identity_match, control.tasks[0].name(), control.tasks[1].name(),
        control.target_bytes, control.candidate_bytes, is_control_size_matched,
        final_counts.transfer_passed, final_counts.heldout_examples_passed,
        final_counts.heldout_examples,
    );
    Ok((output, metrics_arm, is_control_size_matched))
}
