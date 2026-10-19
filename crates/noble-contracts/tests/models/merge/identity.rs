#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only model modules import their sibling relation, identity and fixture types directly so the bounded model stays readable"
)]
use super::model::{Op, Program};
pub(super) fn subject_hash(program: &Program) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    let mut at = 0usize;
    while at < program.ops.len() {
        hash = fold(hash, op_code(program.ops[at]));
        match program.ops[at] {
            Op::Push(value) => hash = fold(hash, value),
            Op::PushCapture(index) => hash = fold(hash, index as u64),
            _ => {}
        }
        at += 1;
    }
    let mut at = 0usize;
    while at < program.captures.len() {
        hash = fold(hash, program.captures[at]);
        at += 1;
    }
    fold(hash, program.callee)
}

pub(super) fn op_code(op: Op) -> u64 {
    match op {
        Op::Push(_) => 1,
        Op::PushCapture(_) => 2,
        Op::Add => 3,
        Op::Sub => 4,
        Op::Swap => 5,
        Op::Drop => 6,
    }
}

pub(super) fn fold(mut hash: u64, value: u64) -> u64 {
    let mut byte = 0u32;
    while byte < 8 {
        hash ^= (value >> (byte * 8)) & 0xff;
        hash = hash.wrapping_mul(0x100_0000_01b3);
        byte += 1;
    }
    hash
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Claim {
    pub(super) base: u64,
    pub(super) first: u64,
    pub(super) second: u64,
    pub(super) candidate: u64,
    pub(super) output_len: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Evidence {
    pub(super) claim: Claim,
    pub(super) proof_claim: Claim,
    pub(super) revision: u64,
}

pub(super) fn check_evidence(evidence: &Evidence, selected: &Claim, revision: u64) -> bool {
    evidence.claim == *selected
        && evidence.proof_claim == *selected
        && evidence.revision == revision
}
