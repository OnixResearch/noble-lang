//! Issue #34: finite two-byte interpreter, independent oracle and residual tests.
//! Existing foundation proofs concern the abstract graph, not Rust execution.

use noble_ngraph::check::Budgets;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{Op, ScalarType};
use noble_ngraph::sem::Value;
use noble_ngraph::specialize::{self, Request, Residual};
use noble_ngraph_experiment::scheme::BLAKE3;

#[path = "interpreter/measurement.rs"]
mod measurement;
#[path = "interpreter/model.rs"]
mod model;
use model::*;

fn assert_dispatch_removed(residual: &Residual, ops: [Opcode; 2]) {
    let graph = residual.checked.graph();
    assert_eq!(graph.slots.len(), 2, "x and y remain dynamic");
    assert_eq!(
        graph.nodes.len(),
        4,
        "only x, y, and two selected operations remain"
    );
    assert_eq!(graph.outputs.len(), 1);
    for (index, op) in ops.into_iter().enumerate() {
        let node_index = index.checked_add(2).expect("two instruction positions fit");
        let actual = &graph.nodes[node_index].op;
        let is_selected = matches!(
            (op, actual),
            (Opcode::Add, Op::I64Add(..))
                | (Opcode::Sub, Op::I64Sub(..))
                | (Opcode::Mul, Op::I64Mul(..))
        );
        assert!(
            is_selected,
            "selected opcode at node {node_index}: {actual:?}"
        );
    }
    assert!(graph.slots.iter().all(|slot| slot.ty == ScalarType::I64));
}

#[test]
fn first_projection_matches_independent_reference() {
    let lowered = admit();
    for first in Opcode::ALL {
        for second in Opcode::ALL {
            let ops = [first, second];
            let source = [first.byte(), second.byte()];
            let (residual, _, readmitted) = specialize_program(&lowered, ops);
            assert_dispatch_removed(&residual, ops);
            let direct = direct(ops);
            let map = static_map(ops);
            for (x, y) in program_rows() {
                let input = [x, y];
                let full = [
                    map[0].unwrap(),
                    map[1].unwrap(),
                    map[2].unwrap(),
                    map[3].unwrap(),
                    map[4].unwrap(),
                    map[5].unwrap(),
                    x,
                    y,
                ];
                let expected = Obs::Normal(vec![Value::I64(reference(&source, (x, y)).unwrap())]);
                assert_eq!(
                    observe_graph(&lowered.checked, &full),
                    expected,
                    "I({source:?}, {input:?})"
                );
                assert_eq!(
                    observed(&lowered.compiled, &full),
                    expected,
                    "compiled interpreter"
                );
                assert_eq!(
                    observe_graph(&residual.checked, &input),
                    expected,
                    "residual {source:?} {input:?}"
                );
                assert_eq!(
                    observed(&readmitted, &input),
                    expected,
                    "readmitted {source:?} {input:?}"
                );
                assert_eq!(
                    observe_graph(&direct.checked, &input),
                    expected,
                    "direct ordinary source"
                );
                assert_eq!(
                    observed(&direct.compiled, &input),
                    expected,
                    "direct ordinary compiled"
                );
            }
        }
    }
}

#[test]
fn malformed_programs_inputs_and_budget_refusals_are_observed() {
    for source in [
        &[][..],
        &[0][..],
        &[0, 1, 2][..],
        &[3, 0][..],
        &[0, 255][..],
    ] {
        assert!(decode(source).is_err(), "invalid source {source:?}");
        assert!(reference(source, (7, 2)).is_err());
    }
    let lowered = admit();
    let map = static_map([Opcode::Sub, Opcode::Mul]);
    for budgets in [
        Budgets {
            variants: 0,
            ..Budgets::default()
        },
        Budgets {
            residual_nodes: 23,
            ..Budgets::default()
        },
    ] {
        let refusal = specialize::specialize(
            &BLAKE3,
            &lowered.checked,
            &Request {
                static_inputs: &map,
                budgets: &budgets,
            },
        )
        .expect_err("budget refusal");
        assert!(
            matches!(refusal.code, noble_ngraph::diag::Code::Budget(_)),
            "{refusal}"
        );
    }
    let (residual, _, _) = specialize_program(&lowered, [Opcode::Sub, Opcode::Mul]);
    let wrong_arity = eval::reference(
        &residual.checked,
        &[Value::I64(1)],
        None,
        &mut Control::default(),
    )
    .obs;
    let wrong_type = eval::reference(
        &residual.checked,
        &[Value::Bool(false), Value::I64(2)],
        None,
        &mut Control::default(),
    )
    .obs;
    assert!(matches!(wrong_arity, Obs::Refused(..)));
    assert!(matches!(wrong_type, Obs::Refused(..)));
}

fn assert_wrong_bindings(lowered: &Interpreted, correct: [Opcode; 2], expected: i64) {
    let mut wrong_binding = static_map(correct);
    wrong_binding.swap(0, 1);
    let wrong_opcode = static_map([Opcode::Mul, Opcode::Add]);
    for (name, map) in [
        ("static slots swapped", wrong_binding),
        ("sub decoded as mul", wrong_opcode),
    ] {
        let budgets = Budgets::default();
        let mutant = specialize::specialize(
            &BLAKE3,
            &lowered.checked,
            &Request {
                static_inputs: &map,
                budgets: &budgets,
            },
        )
        .expect("mutant residual");
        assert_ne!(
            observe_graph(&mutant.checked, &[7, 2]),
            Obs::Normal(vec![Value::I64(expected)]),
            "{name}"
        );
    }
    let mutant_source = INTERPRETER.replacen("x y - let s0", "y x - let s0", 1);
    assert_ne!(mutant_source, INTERPRETER);
    let wrong_interpreter = specialize::readmit(
        &BLAKE3,
        &mutant_source,
        &[const { noble_kernel::types::Ty::I64 }; 8],
        limits(),
        &Budgets::default(),
    )
    .expect("swapped-operand interpreter admits");
    let coeff = static_map(correct);
    let full = [
        coeff[0].unwrap(),
        coeff[1].unwrap(),
        coeff[2].unwrap(),
        coeff[3].unwrap(),
        coeff[4].unwrap(),
        coeff[5].unwrap(),
        7,
        2,
    ];
    assert_ne!(
        observe_graph(&wrong_interpreter, &full),
        Obs::Normal(vec![Value::I64(expected)]),
        "swapped interpreter operands disagree"
    );
}

fn assert_cache_rebinding(lowered: &Interpreted, correct: [Opcode; 2], expected: i64) {
    let stale = specialize_program(lowered, [Opcode::Add, Opcode::Add]).0;
    assert_ne!(
        observe_graph(&stale.checked, &[7, 2]),
        Obs::Normal(vec![Value::I64(expected)]),
        "stale captured program disagrees"
    );
    let actual = specialize_program(lowered, correct).0;
    assert_eq!(
        observe_graph(&actual.checked, &[7, 2]),
        Obs::Normal(vec![Value::I64(expected)])
    );
    let old_map = static_map([Opcode::Add, Opcode::Add]);
    let correct_map = static_map(correct);
    let mut session = noble_ngraph::specialize::cache::Session::default();
    let budgets = Budgets::default();
    let invoke = |map| {
        noble_ngraph::specialize::cache::Invocation::new(
            &BLAKE3,
            &lowered.checked,
            INTERPRETER.as_bytes(),
            Request {
                static_inputs: map,
                budgets: &budgets,
            },
            noble_contracts::Limits::default(),
        )
    };
    let old = session
        .specialize(invoke(&old_map))
        .expect("old residual cached");
    assert_eq!(
        old.status,
        noble_ngraph::specialize::cache::Status::MissStored
    );
    let rebound = session
        .specialize(invoke(&correct_map))
        .expect("new binding cached separately");
    assert_eq!(
        rebound.status,
        noble_ngraph::specialize::cache::Status::MissStored,
        "a stale static map must not hit after rebinding"
    );
    assert_ne!(old.specialization_key, rebound.specialization_key);
    assert_eq!(
        observe_graph(&rebound.readmitted, &[7, 2]),
        Obs::Normal(vec![Value::I64(expected)])
    );
}

#[test]
fn seeded_wrong_binding_opcode_operands_and_captured_program_are_killed() {
    let lowered = admit();
    let correct = [Opcode::Sub, Opcode::Add];
    let expected = reference(&[1, 0], (7, 2)).unwrap();
    assert_wrong_bindings(&lowered, correct, expected);
    assert_cache_rebinding(&lowered, correct, expected);
}
