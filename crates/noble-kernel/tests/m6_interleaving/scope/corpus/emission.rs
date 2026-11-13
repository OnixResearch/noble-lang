type ScopeMemberRef = super::super::manager::MemberRef;

fn model_member(member: ScopeMemberRef) -> Result<&'static str, String> {
    if member.scope != super::super::manager::ScopeId(115)
        || member.table != noble_kernel::async_tasks::TableId(3)
    {
        return Err(format!("unmapped host identity: {member:?}"));
    }
    match (member.handle.slot, member.handle.generation) {
        (0, 1) => Ok("left1"),
        (1, 2) => Ok("right2"),
        _ => Err(format!("unmapped slot generation: {member:?}")),
    }
}

fn member_list(members: impl Iterator<Item = ScopeMemberRef>) -> Result<String, String> {
    let mut rendered = String::from("[");
    for (index, member) in members.enumerate() {
        if index > 0 {
            rendered.push_str(", ");
        }
        rendered.push_str(model_member(member)?);
    }
    rendered.push(']');
    Ok(rendered)
}

fn observation_values(observation: noble_kernel::async_tasks::Observation) -> String {
    format!(
        "[{}, {}, {}, {}, {}, {}]",
        observation.pending,
        observation.ready,
        observation.delivered,
        observation.retiring,
        observation.retired,
        observation.outstanding_pins,
    )
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this fallible generator serializes already-checked runtime witnesses, and formatting failures are propagated rather than asserted."
)]
fn emit_trace_definitions(
    text: &mut impl core::fmt::Write,
    forward: &super::CompletionWitness,
    reverse: &super::CompletionWitness,
) -> Result<(), String> {
    writeln!(
        text,
        "def adapterLeftFirstTrace : List Step := {}",
        forward.model_trace
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterRightFirstTrace : List Step := {}",
        reverse.model_trace
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterLeftFirstResults : List Member := {}",
        member_list(forward.results.iter().map(|result| result.member))?
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterRightFirstResults : List Member := {}",
        member_list(reverse.results.iter().map(|result| result.member))?
    )
    .map_err(|e| e.to_string())?;
    text.write_str("\ntheorem adapter_left_first_results_match_model :\n  (stepThrough adapterLeftFirstTrace (NobleFoundation.TaskScope.emptyTable 2) (emptyScope 7 3)).map (fun pair => pair.2.results) = some adapterLeftFirstResults := by decide\n\n").map_err(|e| e.to_string())?;
    text.write_str("theorem adapter_right_first_results_match_model :\n  (stepThrough adapterRightFirstTrace (NobleFoundation.TaskScope.emptyTable 2) (emptyScope 7 3)).map (fun pair => pair.2.results) = some adapterRightFirstResults := by decide\n\n").map_err(|e| e.to_string())?;
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this fallible generator serializes already-checked delivery observations, and formatting failures are propagated rather than asserted."
)]
fn emit_delivery_observations(
    text: &mut impl core::fmt::Write,
    forward: &super::CompletionWitness,
    reverse: &super::CompletionWitness,
) -> Result<(), String> {
    writeln!(
        text,
        "def adapterOutOfOrderDeliveryRefused : Bool := {}",
        reverse.out_of_order_refused
    )
    .map_err(|e| e.to_string())?;
    text.write_str("theorem adapter_out_of_order_delivery_refusal_matches_model :\n  adapterOutOfOrderDeliveryRefused =\n    ((stepThrough [.admission left1, .attach left1, .admission right2, .attach right2, .succeed right2]\n      (NobleFoundation.TaskScope.emptyTable 2) (emptyScope 7 3)).bind (fun pair => step (.deliver right2) pair.1 pair.2)).isNone := by decide\n\n").map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterLeftFirstDeliveryAccounting : List (Nat × Nat × Nat) := [({}, {}, {}), ({}, {}, {})]",
        forward.accounting[0].inputs,
        forward.accounting[0].results,
        forward.accounting[0].buffers,
        forward.accounting[1].inputs,
        forward.accounting[1].results,
        forward.accounting[1].buffers,
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterRightFirstDeliveryAccounting : List (Nat × Nat × Nat) := [({}, {}, {}), ({}, {}, {})]",
        reverse.accounting[0].inputs,
        reverse.accounting[0].results,
        reverse.accounting[0].buffers,
        reverse.accounting[1].inputs,
        reverse.accounting[1].results,
        reverse.accounting[1].buffers,
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterSettledObservation : List Nat := {}",
        observation_values(forward.settled)
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterInitialPending : Nat := {}",
        forward.initial.pending
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterAfterDeliveryCount : Nat := {}",
        forward.after_delivery.delivered
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn emit_normal_traces(
    text: &mut impl core::fmt::Write,
    forward: &super::CompletionWitness,
    reverse: &super::CompletionWitness,
) -> Result<(), String> {
    emit_trace_definitions(text, forward, reverse)?;
    emit_delivery_observations(text, forward, reverse)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this fallible generator serializes the already-checked refusal witness, and formatting failures are propagated rather than asserted."
)]
fn emit_refusal(
    text: &mut impl core::fmt::Write,
    refusal: &super::RefusalWitness,
) -> Result<(), String> {
    let first = refusal
        .members
        .first()
        .ok_or("refusal trace lost its first member")?;
    let first_name = model_member(*first)?;
    writeln!(
        text,
        "def adapterRefusedMembers : List Member := {}",
        member_list(refusal.members.iter().copied())?
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterRefusalInputReturned : Bool := {}",
        refusal.input_returned
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterRefusalWasTaskCapacity : Bool := {}",
        refusal.error == noble_kernel::async_tasks::Error::TaskCapacity
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterRefusalBefore : List Nat := {}",
        observation_values(refusal.before)
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterRefusalAfter : List Nat := {}",
        observation_values(refusal.after)
    )
    .map_err(|e| e.to_string())?;
    writeln!(text, "def adapterRefusalPrefixTrace : List Step := [.admission {first_name}, .attach {first_name}]").map_err(|e| e.to_string())?;
    text.write_str("\ntheorem adapter_second_refusal_matches_model :\n  (stepThrough adapterRefusalPrefixTrace (NobleFoundation.TaskScope.emptyTable 1) (emptyScope 7 3)).map (fun pair => pair.2.members) = some adapterRefusedMembers ∧\n  ((stepThrough adapterRefusalPrefixTrace (NobleFoundation.TaskScope.emptyTable 1) (emptyScope 7 3)).bind (fun pair => step (.admission right2) pair.1 pair.2)).isNone = true := by decide\n\n").map_err(|e| e.to_string())?;
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this fallible generator serializes already-checked cancellation witnesses, and formatting failures are propagated rather than asserted."
)]
fn emit_cancellation(
    text: &mut impl core::fmt::Write,
    after: &super::CancellationWitness,
    before: &super::CancellationWitness,
) -> Result<(), String> {
    writeln!(
        text,
        "def adapterCancelledResults : List Member := {}",
        member_list(after.results.iter().map(|result| result.member))?
    )
    .map_err(|e| e.to_string())?;
    let cancelled_member =
        if after.primary_failure == Some(noble_kernel::async_tasks::Failure::Cancelled) {
            "right2"
        } else {
            "left1"
        };
    writeln!(
        text,
        "def adapterCancelledPrimaryFailure : Option Member := some {cancelled_member}"
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterCancellationAfterSiblingTrace : List Step := {}",
        after.model_trace
    )
    .map_err(|e| e.to_string())?;
    text.write_str("theorem adapter_cancel_after_sibling_completion_matches_model :\n  (stepThrough adapterCancellationAfterSiblingTrace (NobleFoundation.TaskScope.emptyTable 2) (emptyScope 7 3)).map (fun pair => (pair.2.results, pair.2.cancelled, pair.2.failure, pair.1.right.pinHeld, pair.1.right.retiring)) =\n  some (adapterCancelledResults, [right2], adapterCancelledPrimaryFailure, true, true) := by decide\n\n").map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterCancellationBeforeSiblingTrace : List Step := {}",
        before.model_trace
    )
    .map_err(|e| e.to_string())?;
    text.write_str("theorem adapter_cancel_before_sibling_completion_matches_model :\n  (stepThrough adapterCancellationBeforeSiblingTrace (NobleFoundation.TaskScope.emptyTable 2) (emptyScope 7 3)).map (fun pair => (pair.2.results, pair.2.cancelled, pair.2.failure, pair.1.right.pinHeld, pair.1.right.retiring)) =\n  some (adapterCancelledResults, [right2], adapterCancelledPrimaryFailure, true, true) := by decide\n\n").map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterCancelledPins : Nat := {}",
        after.cancelled_record.pins.count_ones()
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterCancelledRetiringCount : Nat := {}",
        after.after_late_completion.retiring
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterDeliveredObligations : Nat × Nat × Nat := ({}, {}, {})",
        after.delivered.inputs, after.delivered.results, after.delivered.buffers
    )
    .map_err(|e| e.to_string())?;
    writeln!(
        text,
        "def adapterCancellationSettledObservation : List Nat := {}",
        observation_values(after.settled)
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this generator composes fallible trace checks and preserves formatting errors; runtime assertions live in each adapter fixture."
)]
pub(super) fn lean_file() -> Result<String, String> {
    let forward = super::completion_trace(false)?;
    let reverse = super::completion_trace(true)?;
    let refusal = super::refusal_trace()?;
    let cancellation_after = super::cancel::trace(false)?;
    let cancellation_before = super::cancel::trace(true)?;
    let mut text = String::from(
        "-- Generated by crates/noble-kernel/tests/m6_interleaving/scope/corpus.rs.\n\
         -- Runtime constants come from the real Table-backed host adapter; proofs below check only\n\
         -- the finite policy projection and do not establish Rust/Lean refinement.\n\
         import NobleFoundation.TaskScope\n\
         namespace NobleFoundation.TaskScopeCorpus\n\
         open NobleFoundation.TaskInterleaving\n\
         open NobleFoundation.TaskScope\n\n",
    );
    emit_normal_traces(&mut text, &forward, &reverse)?;
    emit_refusal(&mut text, &refusal)?;
    emit_cancellation(&mut text, &cancellation_after, &cancellation_before)?;
    text.push_str("end NobleFoundation.TaskScopeCorpus\n");
    Ok(text)
}
