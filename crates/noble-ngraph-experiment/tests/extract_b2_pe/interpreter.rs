//! Issue #34: finite two-byte interpreter, independent oracle, and residual tests.
//! The existing foundation proof concerns its abstract packed graph, not this
//! Rust extraction, optimizer, ordinary admission, or execution path.

use noble_ngraph::check::{Budgets, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{Op, ScalarType};
use noble_ngraph::prng::SplitMix64;
use noble_ngraph::sem::Value;
use noble_ngraph::specialize::{self, Request, Residual};
use noble_ngraph_experiment::scheme::BLAKE3;

#[path = "interpreter/measurement.rs"]
mod measurement;

pub(super) const SEED: u64 = 0x34f0_2026_1114_0001;
pub(super) const INTERPRETER: &str = "\
let y let x let c5 let c4 let c3 let c2 let c1 let c0\n\
x y + let a0 x y - let s0 x y * let m0\n\
c0 a0 * c1 s0 * + c2 m0 * + let acc\n\
acc y + let a1 acc y - let s1 acc y * let m1\n\
c3 a1 * c4 s1 * + c5 m1 * +\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Opcode {
    Add,
    Sub,
    Mul,
}

impl Opcode {
    pub(super) const ALL: [Self; 3] = [Self::Add, Self::Sub, Self::Mul];

    pub(super) const fn byte(self) -> u8 {
        match self {
            Self::Add => 0,
            Self::Sub => 1,
            Self::Mul => 2,
        }
    }

    pub(super) const fn token(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
        }
    }

    const fn one_hot(self) -> [i64; 3] {
        match self {
            Self::Add => [1, 0, 0],
            Self::Sub => [0, 1, 0],
            Self::Mul => [0, 0, 1],
        }
    }

    pub(super) const fn apply(self, left: i64, right: i64) -> i64 {
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::Mul => left.wrapping_mul(right),
        }
    }
}

/// Separate tagged source encoding; invalid lengths/tags are refusals, never
/// coerced to an opcode. This family has no stack instructions or stack error.
fn decode(source: &[u8]) -> Result<[Opcode; 2], &'static str> {
    if source.len() != 2 {
        return Err("source must be exactly two tagged instructions");
    }
    let op = |tag| match tag {
        0 => Ok(Opcode::Add),
        1 => Ok(Opcode::Sub),
        2 => Ok(Opcode::Mul),
        _ => Err("unrecognized instruction tag"),
    };
    Ok([op(source[0])?, op(source[1])?])
}

pub(super) fn static_map(ops: [Opcode; 2]) -> [Option<i64>; 8] {
    let [a, b] = ops;
    let [a0, a1, a2] = a.one_hot();
    let [b0, b1, b2] = b.one_hot();
    [
        Some(a0),
        Some(a1),
        Some(a2),
        Some(b0),
        Some(b1),
        Some(b2),
        None,
        None,
    ]
}

/// Reference interprets *tags*, not the one-hot coefficients or the Noble
/// graph. Reversing subtraction changes this oracle independently.
pub(super) fn reference(source: &[u8], x: i64, y: i64) -> Result<i64, &'static str> {
    let [first, second] = decode(source)?;
    Ok(second.apply(first.apply(x, y), y))
}

pub(super) fn direct_source(ops: [Opcode; 2]) -> String {
    format!("let y let x x y {} y {}", ops[0].token(), ops[1].token())
}

pub(super) struct Interpreted {
    pub(super) checked: CheckedGraph,
    pub(super) compiled: Compiled,
}

fn interpreter_limits() -> noble_contracts::Limits {
    // Constraint expansion for eight live input binders exceeds the default
    // 16K node limit; keep this experimental admission independently checked.
    noble_contracts::Limits {
        nodes: 524_288,
        work: 512_000_000,
        ..noble_contracts::Limits::default()
    }
}

pub(super) fn interpreter() -> Interpreted {
    let checked = specialize::readmit(
        &BLAKE3,
        INTERPRETER,
        &[const { noble_kernel::types::Ty::I64 }; 8],
        interpreter_limits(),
        &Budgets::default(),
    )
    .expect("interpreter accepted as a declared-text Noble Program");
    assert_eq!(checked.graph().slots.len(), 8);
    let compiled = Compiled::new(&BLAKE3, &checked);
    Interpreted { checked, compiled }
}

pub(super) fn direct(ops: [Opcode; 2]) -> Interpreted {
    let checked = specialize::readmit(
        &BLAKE3,
        &direct_source(ops),
        &[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64],
        noble_contracts::Limits::default(),
        &Budgets::default(),
    )
    .expect("direct ordinary Noble Program admits");
    let compiled = Compiled::new(&BLAKE3, &checked);
    Interpreted { checked, compiled }
}

pub(super) fn specialize_program(
    lowered: &Interpreted,
    ops: [Opcode; 2],
) -> (Residual, String, Compiled) {
    let budgets = Budgets::default();
    let residual = specialize::specialize(
        &BLAKE3,
        &lowered.checked,
        &Request {
            static_inputs: &static_map(ops),
            budgets: &budgets,
        },
    )
    .expect("checked interpreter specializes");
    let source = specialize::emit_source(&residual.checked).expect("residual source emitted");
    let readmitted = specialize::readmit(
        &BLAKE3,
        &source,
        &specialize::interface_types(&residual.checked).expect("flat I64 interface"),
        noble_contracts::Limits::default(),
        &budgets,
    )
    .expect("ordinary-path independent residual admission");
    (residual, source, Compiled::new(&BLAKE3, &readmitted))
}

pub(super) fn program_rows() -> Vec<(i64, i64)> {
    let mut rows = vec![
        (i64::MIN, i64::MAX),
        (i64::MAX, 1),
        (i64::MIN, -1),
        (i64::MIN, 0),
        (0, i64::MIN),
        (-1, -1),
        (0, 0),
        (1, 1),
        (7, 2),
        (2, 7),
    ];
    let mut rng = SplitMix64::new(SEED);
    for _ in 0..256 {
        rows.push((rng.next_u64() as i64, rng.next_u64() as i64));
    }
    rows
}

pub(super) fn observe_graph(checked: &noble_ngraph::check::CheckedGraph, row: &[i64]) -> Obs {
    eval::reference(
        checked,
        &eval::i64_inputs(row),
        None,
        &mut Control::default(),
    )
    .obs
}

fn observed(compiled: &Compiled, row: &[i64]) -> Obs {
    compiled
        .evaluate(&eval::i64_inputs(row), None, &mut Control::default())
        .obs
}

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
        let actual = &graph.nodes[index + 2].op;
        let ok = matches!(
            (op, actual),
            (Opcode::Add, Op::I64Add(..))
                | (Opcode::Sub, Op::I64Sub(..))
                | (Opcode::Mul, Op::I64Mul(..))
        );
        assert!(ok, "selected opcode at node {}: {actual:?}", index + 2);
    }
    assert!(graph.slots.iter().all(|s| s.ty == ScalarType::I64));
}

#[test]
fn first_projection_matches_interpreter_and_independent_reference() {
    let lowered = interpreter();
    for first in Opcode::ALL {
        for second in Opcode::ALL {
            let ops = [first, second];
            let source = [first.byte(), second.byte()];
            let (residual, _, readmitted) = specialize_program(&lowered, ops);
            assert_dispatch_removed(&residual, ops);
            let direct = direct(ops);
            for (x, y) in program_rows() {
                let input = [x, y];
                let map = static_map(ops);
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
                let expected = Obs::Normal(vec![Value::I64(reference(&source, x, y).unwrap())]);
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
        assert!(reference(source, 7, 2).is_err());
    }
    let lowered = interpreter();
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

#[test]
fn seeded_wrong_binding_opcode_operands_and_captured_program_are_killed() {
    let lowered = interpreter();
    let correct = [Opcode::Sub, Opcode::Add];
    let [x, y] = [7, 2];
    let expected = reference(&[1, 0], x, y).unwrap();
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
            observe_graph(&mutant.checked, &[x, y]),
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
        interpreter_limits(),
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
        x,
        y,
    ];
    assert_ne!(
        observe_graph(&wrong_interpreter, &full),
        Obs::Normal(vec![Value::I64(expected)]),
        "swapped interpreter operands disagree with independently decoded source"
    );
    let stale = specialize_program(&lowered, [Opcode::Add, Opcode::Add]).0;
    assert_ne!(
        observe_graph(&stale.checked, &[x, y]),
        Obs::Normal(vec![Value::I64(expected)]),
        "stale retained subject with older opcode binding is caught by independent reference"
    );
    let actual = specialize_program(&lowered, correct).0;
    assert_eq!(
        observe_graph(&actual.checked, &[x, y]),
        Obs::Normal(vec![Value::I64(expected)])
    );
    let old_map = static_map([Opcode::Add, Opcode::Add]);
    let correct_map = static_map(correct);
    let mut session = noble_ngraph::specialize::cache::Session::default();
    let budgets = Budgets::default();
    let old = session
        .specialize(noble_ngraph::specialize::cache::Invocation::new(
            &BLAKE3,
            &lowered.checked,
            INTERPRETER.as_bytes(),
            Request {
                static_inputs: &old_map,
                budgets: &budgets,
            },
            noble_contracts::Limits::default(),
        ))
        .expect("old residual cached");
    assert_eq!(
        old.status,
        noble_ngraph::specialize::cache::Status::MissStored
    );
    let rebound = session
        .specialize(noble_ngraph::specialize::cache::Invocation::new(
            &BLAKE3,
            &lowered.checked,
            INTERPRETER.as_bytes(),
            Request {
                static_inputs: &correct_map,
                budgets: &budgets,
            },
            noble_contracts::Limits::default(),
        ))
        .expect("new source binding cached separately");
    assert_eq!(
        rebound.status,
        noble_ngraph::specialize::cache::Status::MissStored,
        "a stale captured static map must not hit after rebinding"
    );
    assert_ne!(old.specialization_key, rebound.specialization_key);
    assert_eq!(
        observe_graph(&rebound.readmitted, &[x, y]),
        Obs::Normal(vec![Value::I64(expected)])
    );
}
