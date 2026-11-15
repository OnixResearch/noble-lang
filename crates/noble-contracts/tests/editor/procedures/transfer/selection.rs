use super::*;

fn encode_pair_size(tasks: [SourceTask; 2], namespace: &Session) -> Result<usize, String> {
    let mut library = ProcedureLibrary::new(super::revision(7));
    for task in tasks {
        let record = super::producer::task_record(task, Producer::Synthesis)?;
        library
            .admit(namespace, None, record)
            .map_err(|error| format!("size-pair admission {}: {error:?}", task.name()))?;
    }
    Ok(library
        .encode()
        .map_err(|error| format!("size-pair encoding: {error:?}"))?
        .len())
}

pub(super) fn choose_control_pair(namespace: &Session) -> Result<ControlSelection, String> {
    let target_bytes = encode_pair_size([SourceTask::Commuted, SourceTask::Subtract], namespace)?;
    let mut best: Option<([SourceTask; 2], usize, usize)> = None;
    for (left_index, left) in CONTROL_POOL.iter().copied().enumerate() {
        for right in CONTROL_POOL
            .iter()
            .copied()
            .skip(left_index.saturating_add(1))
        {
            let pair = [left, right];
            let candidate_bytes = encode_pair_size(pair, namespace)?;
            let difference = candidate_bytes.abs_diff(target_bytes);
            if best.is_none_or(|(_, _, best_difference)| difference < best_difference) {
                best = Some((pair, candidate_bytes, difference));
            }
        }
    }
    let (tasks, candidate_bytes, difference) = best.ok_or("no eligible size-control pair")?;
    Ok(ControlSelection {
        tasks,
        target_bytes,
        candidate_bytes,
        within_ten_percent: difference.saturating_mul(10) <= target_bytes,
    })
}
