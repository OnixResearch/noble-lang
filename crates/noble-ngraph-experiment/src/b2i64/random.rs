//! Seeded random Core-Bootstrap `I64` programs for the B2 extraction leg
//! (`splitmix64/0`; the seed is recorded in the receipt).
//!
//! Programs are generated as ordinary source over a typed stack model so
//! the Core frontend accepts them: literals (including `I64` extremes),
//! `dup`, `drop`, `swap`, `+ - *`, `=`, `[ … ] dip`, `quote [ … ] compose
//! run` (a capture), `[ … ] [ … ] compose run`, `[ u ] dup compose run` (one
//! shared recipe expanded twice) and calls of generated unary/binary named
//! definitions (which may call earlier ones).

use crate::prng::SplitMix64;

/// One generated program.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    /// `def …` submissions, committed in order.
    pub definitions: Vec<String>,
    /// Root body.
    pub root: String,
    /// Number of `I64` inputs (bottom-first).
    pub inputs: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum T {
    I,
    B,
}

const EDGES: [i64; 14] = [
    0,
    1,
    -1,
    2,
    3,
    7,
    i64::MAX,
    i64::MIN,
    i64::MAX - 1,
    i64::MIN + 1,
    1 << 31,
    1 << 32,
    -(1 << 32),
    0x5555_5555_5555_5555,
];

/// An input or literal value: an edge value or a uniformly random word.
pub fn value(rng: &mut SplitMix64) -> i64 {
    if rng.below(3) == 0 {
        EDGES[rng.below(EDGES.len() as u64) as usize]
    } else {
        rng.next_u64() as i64
    }
}

fn small(rng: &mut SplitMix64) -> i64 {
    if rng.below(2) == 0 {
        rng.below(10) as i64 - 3
    } else {
        value(rng)
    }
}

struct Gen<'a> {
    rng: &'a mut SplitMix64,
    unary: Vec<String>,
    binary: Vec<String>,
}

impl Gen<'_> {
    fn unary_chain(&mut self) -> String {
        let n = 1 + self.rng.below(3);
        let mut parts = Vec::new();
        for _ in 0..n {
            let choice = self.rng.below(if self.unary.is_empty() { 6 } else { 7 });
            parts.push(match choice {
                0 => format!("{} +", small(self.rng)),
                1 => format!("{} -", small(self.rng)),
                2 => format!("{} *", small(self.rng)),
                3 => "dup *".to_owned(),
                4 => "dup +".to_owned(),
                5 => "dup 1 + *".to_owned(),
                _ => self.unary[self.rng.below(self.unary.len() as u64) as usize].clone(),
            });
        }
        parts.join(" ")
    }

    fn binary_body(&mut self) -> String {
        let op = ["+", "-", "*"][self.rng.below(3) as usize];
        match self.rng.below(4) {
            0 => op.to_owned(),
            1 => format!("swap {op}"),
            2 => format!("{op} {}", self.unary_chain()),
            _ => format!("{} swap {} {op}", self.unary_chain(), self.unary_chain()),
        }
    }

    fn seq(&mut self, stack: &mut Vec<T>, len: u64, depth: u32) -> Vec<String> {
        let mut out = Vec::new();
        for _ in 0..len {
            let top_i = stack.last() == Some(&T::I);
            let two_i = stack.len() >= 2 && stack[stack.len() - 2..] == [T::I, T::I];
            match self.rng.below(16) {
                0 | 1 => {
                    out.push(small(self.rng).to_string());
                    stack.push(T::I);
                }
                2 if !stack.is_empty() => {
                    out.push("dup".into());
                    let t = stack[stack.len() - 1];
                    stack.push(t);
                }
                3 if stack.len() >= 2 => {
                    out.push("drop".into());
                    stack.pop();
                }
                4 if stack.len() >= 2 => {
                    out.push("swap".into());
                    let n = stack.len();
                    stack.swap(n - 1, n - 2);
                }
                5..=7 if two_i => {
                    out.push(["+", "-", "*"][self.rng.below(3) as usize].into());
                    stack.pop();
                }
                8 if two_i => {
                    out.push("=".into());
                    stack.pop();
                    stack.pop();
                    stack.push(T::B);
                }
                9 if stack.len() >= 2 && depth < 2 => {
                    let top = stack.pop().unwrap_or(T::I);
                    let n = 1 + self.rng.below(3);
                    let inner = self.seq(stack, n, depth + 1);
                    stack.push(top);
                    out.push(format!("[ {} ] dip", inner.join(" ")));
                }
                10 if !stack.is_empty() && depth < 2 => {
                    // `x quote [ s ] compose run` ≡ `x s`: x is a capture.
                    let n = 1 + self.rng.below(3);
                    let inner = self.seq(stack, n, depth + 1);
                    out.push(format!("quote [ {} ] compose run", inner.join(" ")));
                }
                11 if depth < 2 => {
                    let n = 1 + self.rng.below(2);
                    let a = self.seq(stack, n, depth + 1);
                    let n = 1 + self.rng.below(2);
                    let b = self.seq(stack, n, depth + 1);
                    out.push(format!(
                        "[ {} ] [ {} ] compose run",
                        a.join(" "),
                        b.join(" ")
                    ));
                }
                12 if top_i => {
                    out.push(format!("[ {} ] dup compose run", self.unary_chain()));
                }
                13 if top_i && !self.unary.is_empty() => {
                    out.push(self.unary[self.rng.below(self.unary.len() as u64) as usize].clone());
                }
                14 if two_i && !self.binary.is_empty() => {
                    out.push(
                        self.binary[self.rng.below(self.binary.len() as u64) as usize].clone(),
                    );
                    stack.pop();
                }
                _ => {}
            }
        }
        out
    }
}

/// Generates program `index` of the campaign seeded by `seed`.
#[must_use]
pub fn program(seed: u64, index: u64) -> Program {
    let mut rng = SplitMix64::new(seed ^ index.wrapping_mul(0xA076_1D64_78BD_642F));
    let mut g = Gen {
        rng: &mut rng,
        unary: Vec::new(),
        binary: Vec::new(),
    };
    let mut definitions = Vec::new();
    for k in 0..g.rng.below(3) {
        let body = g.unary_chain();
        definitions.push(format!("def u{k} [ {body} ]"));
        g.unary.push(format!("u{k}"));
    }
    for k in 0..g.rng.below(3) {
        let body = g.binary_body();
        definitions.push(format!("def b{k} [ {body} ]"));
        g.binary.push(format!("b{k}"));
    }
    let inputs = 1 + g.rng.below(4) as usize;
    let mut stack = vec![T::I; inputs];
    let len = 4 + g.rng.below(24);
    let mut body = g.seq(&mut stack, len, 0);
    // At most three outputs; Bool results may stay as outputs.
    while stack.len() > 3 {
        let n = stack.len();
        if stack[n - 2..] == [T::I, T::I] {
            body.push(["+", "-", "*"][g.rng.below(3) as usize].into());
        } else {
            body.push("drop".into());
        }
        stack.pop();
    }
    if body.is_empty() {
        body.push("dup *".into());
    }
    Program {
        definitions,
        root: body.join(" "),
        inputs,
    }
}
