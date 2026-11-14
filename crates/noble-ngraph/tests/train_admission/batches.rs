use super::common::{checked, contract, linear, points, IDS};
use super::{code, train, TrainControl};

#[test]
fn minibatches_follow_canonical_contiguous_order_and_replay_with_schedule_identity() {
    let full_contract = contract(checked(linear(false, false)), points());
    let mut batched_control = TrainControl {
        batch_size: Some(2),
        ..TrainControl::default()
    };
    let batched = train::train(&IDS, &full_contract, None, 4, &mut batched_control)
        .expect("four canonical batches train");

    let mut first_batch_contract = full_contract.clone();
    first_batch_contract.data = full_contract.data[..2].to_vec();
    let mut first_batch_control = TrainControl::default();
    let first_batch = train::train(
        &IDS,
        &first_batch_contract,
        None,
        1,
        &mut first_batch_control,
    )
    .expect("first contiguous batch trains");
    assert_eq!(batched.losses[0], first_batch.losses[0]);
    assert_ne!(
        batched.provenance.data,
        train::data_digest(&IDS, &full_contract.data)
    );

    let mut replay_control = TrainControl {
        batch_size: Some(2),
        ..TrainControl::default()
    };
    let replayed = train::replay(
        &IDS,
        &full_contract,
        &batched.provenance,
        None,
        &mut replay_control,
    )
    .expect("the exact batch schedule replays");
    assert_eq!(replayed.snapshot, batched.snapshot);
    assert_eq!(replayed.state, batched.state);

    let mut changed_schedule = TrainControl {
        batch_size: Some(3),
        ..TrainControl::default()
    };
    assert_eq!(
        code(train::replay(
            &IDS,
            &full_contract,
            &batched.provenance,
            None,
            &mut changed_schedule,
        )),
        "E-SNAPSHOT-STALE"
    );

    let mut final_loss_control = TrainControl::default();
    let final_full_loss = train::train(
        &IDS,
        &full_contract,
        Some(&batched.snapshot),
        0,
        &mut final_loss_control,
    )
    .expect("zero-step full-data loss is evaluated");
    assert_eq!(batched.losses.last(), final_full_loss.losses.first());
}

#[test]
fn invalid_minibatch_sizes_refuse_before_publication() {
    let contract = contract(checked(linear(false, false)), points());
    for batch_size in [0, 6] {
        let mut control = TrainControl {
            batch_size: Some(batch_size),
            ..TrainControl::default()
        };
        assert_eq!(
            code(train::train(&IDS, &contract, None, 2, &mut control)),
            "E-INTERFACE"
        );
    }
}

#[test]
fn no_batch_size_preserves_the_full_data_digest() {
    let contract = contract(checked(linear(false, false)), points());
    let mut control = TrainControl::default();
    let trained = train::train(&IDS, &contract, None, 1, &mut control)
        .expect("full-data training remains available");
    assert_eq!(
        trained.provenance.data,
        train::data_digest(&IDS, &contract.data)
    );
}
