//! Fixed two-opcode source language and ordinarily admitted Noble subjects.

use noble_ngraph::check::{Budgets, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::prng::SplitMix64;

use noble_ngraph::specialize::{self, Request, Residual};
use noble_ngraph_experiment::scheme::BLAKE3;

pub(super) const SEED: u64 = 0x34f0_2026_1114_0001;
pub(super) const INTERPRETER: &str = "\
let y let x let c5 let c4 let c3 let c2 let c1 let c0\n\
x y + let a0 x y - let s0 x y * let m0\n\
c0 a0 * c1 s0 * + c2 m0 * + let acc\n\
acc y + let a1 acc y - let s1 acc y * let m1\n\
c3 a1 * c4 s1 * + c5 m1 * +\n";

// This bytecode alphabet is deliberately closed; adding a case changes the
// source-language encoding, the independent reference and all nine programs.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Opcode {
    Add,
    Sub,
    Mul,
}

#[derive(Clone, Copy)]
pub(super) struct Operands {
    pub(super) left: i64,
    pub(super) right: i64,
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
    pub(super) const fn apply(self, args: Operands) -> i64 {
        match self {
            Self::Add => args.left.wrapping_add(args.right),
            Self::Sub => args.left.wrapping_sub(args.right),
            Self::Mul => args.left.wrapping_mul(args.right),
        }
    }
}

/// Invalid lengths/tags are refusals, never coerced to an opcode.
pub(super) fn decode(source: &[u8]) -> Result<[Opcode; 2], &'static str> {
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
    let [a0, a1, a2] = ops[0].one_hot();
    let [b0, b1, b2] = ops[1].one_hot();
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

/// Independent tagged-byte reference; it does not consume graph coefficients.
pub(super) fn reference(source: &[u8], row: (i64, i64)) -> Result<i64, &'static str> {
    let [first, second] = decode(source)?;
    let (x, y) = row;
    let accumulator = first.apply(Operands { left: x, right: y });
    Ok(second.apply(Operands {
        left: accumulator,
        right: y,
    }))
}

pub(super) fn direct_source(ops: [Opcode; 2]) -> String {
    format!("let y let x x y {} y {}", ops[0].token(), ops[1].token())
}

pub(super) struct Interpreted {
    pub(super) checked: CheckedGraph,
    pub(super) compiled: Compiled,
}

pub(super) fn limits() -> noble_contracts::Limits {
    // Eight live input binders exceed the default 16K-node frontend cap.
    noble_contracts::Limits {
        nodes: 524_288,
        work: 512_000_000,
        ..noble_contracts::Limits::default()
    }
}

pub(super) fn admit() -> Interpreted {
    let checked = specialize::readmit(
        &BLAKE3,
        INTERPRETER,
        &[const { noble_kernel::types::Ty::I64 }; 8],
        limits(),
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

pub(super) fn observe_graph(checked: &CheckedGraph, row: &[i64]) -> Obs {
    eval::reference(
        checked,
        &eval::i64_inputs(row),
        None,
        &mut Control::default(),
    )
    .obs
}

pub(super) fn observed(compiled: &Compiled, row: &[i64]) -> Obs {
    compiled
        .evaluate(&eval::i64_inputs(row), None, &mut Control::default())
        .obs
}
