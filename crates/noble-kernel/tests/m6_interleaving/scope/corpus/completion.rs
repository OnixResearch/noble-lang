#[path = "sequence.rs"]
mod sequence;

struct CompletionObservations {
    initial: noble_kernel::async_tasks::Observation,
    after_first: noble_kernel::async_tasks::Observation,
    after_delivery: noble_kernel::async_tasks::Observation,
    settled: noble_kernel::async_tasks::Observation,
    accounting: [noble_kernel::async_tasks::Obligations; 2],
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; the compared observations and accounting are produced by runtime calls into the M6 table adapter."
)]
fn validate_completion_observations(observations: CompletionObservations) {
    let CompletionObservations {
        initial,
        after_first,
        after_delivery,
        settled,
        accounting,
    } = observations;
    let expected_delivery = noble_kernel::async_tasks::Obligations {
        inputs: 1,
        results: 3,
        buffers: 2,
    };
    let has_expected_accounting = accounting == [expected_delivery, expected_delivery];
    let has_expected_first_delivery = after_first.ready == 1 && after_first.delivered == 1;
    let has_expected_settled_state = settled.pending == 0
        && settled.ready == 0
        && settled.delivered == 2
        && settled.retired == 0
        && settled.outstanding_pins == 0
        && settled.reserved == noble_kernel::async_tasks::Footprint::empty();
    let has_expected_delivery_counts = initial.pending == 2 && after_delivery.delivered == 2;
    assert!(
        has_expected_accounting,
        "each child owns one delivery charge"
    );
    assert!(has_expected_first_delivery, "the first delivery is visible");
    assert!(has_expected_settled_state, "all delivered work is settled");
    assert!(
        has_expected_delivery_counts,
        "both admitted children are delivered"
    );
}

pub(super) fn completion_trace(reverse: bool) -> Result<super::CompletionWitness, String> {
    let mut table = super::super::support::table(2)?;
    let registry = super::super::registry::ScopeRegistry::shared();
    let mut scope = super::super::manager::Scope::new(
        super::super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    let left = super::super::scope_support::admit(&mut scope, &mut table, 1_u8)?;
    let right = super::super::scope_support::admit(&mut scope, &mut table, 2_u8)?;
    let initial = table.observation();
    let progress = sequence::run_completion_schedule(
        reverse,
        sequence::CompletionRun {
            table: &mut table,
            scope: &mut scope,
            left,
            right,
        },
    )?;
    let after_first = progress
        .after_first
        .ok_or("completion trace did not deliver its first child")?;
    let after_delivery = table.observation();
    let results = scope.results();
    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0].member, left,
        "result order follows attachment order"
    );
    assert_eq!(
        results[1].member, right,
        "completion order cannot reorder results"
    );
    assert_eq!(progress.out_of_order_refused, reverse);
    super::super::scope_support::settle_delivered(&mut table, &mut scope, left)?;
    super::super::scope_support::settle_delivered(&mut table, &mut scope, right)?;
    scope.close().map_err(|error| format!("{error:?}"))?;
    let settled = table.observation();
    let accounting: [noble_kernel::async_tasks::Obligations; 2] = progress
        .accounting
        .try_into()
        .map_err(|_| "completion trace did not deliver exactly two children")?;
    validate_completion_observations(CompletionObservations {
        initial,
        after_first,
        after_delivery,
        settled,
        accounting,
    });
    Ok(super::CompletionWitness {
        model_trace: format!("[{}]", progress.model_steps.join(", ")),
        out_of_order_refused: progress.out_of_order_refused,
        results,
        accounting,
        initial,
        after_delivery,
        settled,
    })
}
