//! Revision-3 output shape, erasure and fan-out fixtures in the B2 target.

use super::{i64s, Lowered, Value};
use noble_ngraph::net::Obs;

#[test]
fn unit_and_empty_outputs_read_back_without_a_partial_prefix() {
    let lowered = Lowered::new(&[], "drop", 1).expect("discarded input extracts");
    assert_eq!(lowered.outputs(), 0);
    let report = lowered.l5(&i64s(&[i64::MIN])).expect("net lowering");
    assert_eq!(report.obs, Obs::Normal(Vec::new()));
    assert_eq!(report.interactions, 1);
    assert_eq!(report.final_wires, Some(0));
}

#[test]
fn ordered_multi_output_shapes_preserve_repetition_and_shared_values() {
    let repeated = Lowered::new(&[], "dup", 1).expect("repeated output extracts");
    let repeated_input = i64s(&[i64::MAX]);
    assert_eq!(repeated.outputs(), 2);
    assert_eq!(
        repeated.l3(&repeated_input).expect("L3"),
        vec![Value::I64(i64::MAX), Value::I64(i64::MAX)]
    );
    let repeated_net = repeated.l5(&repeated_input).expect("repeated net");
    assert_eq!(
        repeated_net.obs,
        Obs::Normal(vec![Value::I64(i64::MAX), Value::I64(i64::MAX)])
    );
    assert_eq!(repeated_net.interactions, 1);
    assert_eq!(repeated_net.final_wires, Some(2));

    let shared = Lowered::new(&[], "dup 1 +", 1).expect("shared result extracts");
    let shared_input = i64s(&[41]);
    assert_eq!(shared.outputs(), 2);
    let reference = shared.l3(&shared_input).expect("shared L3");
    assert_eq!(reference, vec![Value::I64(41), Value::I64(42)]);
    let net = shared.l5(&shared_input).expect("shared net");
    assert_eq!(net.obs, Obs::Normal(reference));
}

#[test]
fn nested_pair_outputs_flatten_in_order_for_the_net() {
    let lowered = Lowered::new(&[], "dup dup 1 + swap pair pair", 1).expect("nested pair extracts");
    let inputs = i64s(&[41]);
    let expected = vec![Value::I64(41), Value::I64(42), Value::I64(41)];
    assert_eq!(lowered.root_stack_arity, 1);
    assert_eq!(lowered.outputs(), expected.len());
    assert_eq!(lowered.l3(&inputs).expect("reference outputs"), expected);
    let report = lowered.l5(&inputs).expect("nested-pair net");
    assert_eq!(report.obs, Obs::Normal(expected));
    assert_eq!(report.interactions, 4);
}
