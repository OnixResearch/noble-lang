#![allow(
    tigerstyle::assertion_density,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::numeric_units,
    tigerstyle::fragile_exhaustive_enum_match,
    tigerstyle::platform_dependent_cast,
    reason = "Owner: noble-contracts-maintainers; this test-only claim harness builds finite subjects, hash blocks and refusal tables, so long setup legs and fixed-width hash arithmetic are the readable form. Test-only lane: matches over the observable/fragment enums are intentional regression tables, panics and ignored cleanup results are fixture assertions, and the bounded checker's digest casts index a 16-entry nibble table."
)]
#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only claim modules import their sibling model types directly so the bounded relation stays readable"
)]
//! `noble-merge-claim/v1` and `noble-merge-evidence/v1` for the test-only
//! `SMERGE/1` lane (issue #30, Phase 3).
//!
//! The claim generator is an **untrusted producer**: it may only render a claim
//! from the preparations the consumer selected. The consumer regenerates the
//! expected claim from its own retained preparations and refuses every producer
//! field that differs (`admit`). Nothing here is wired into `prepare`, the CLI
//! or any admission path; the recorded decision stays no-go unless the
//! change's go/no-go criteria are met.

use super::model::{Op, Precondition, Program};

/// Claim/relation revision.
pub(super) const RELATION_REVISION: &str = "noble-merge-claim/v1";

/// Evidence schema revision.
pub(super) const EVIDENCE_SCHEMA: &str = "noble-merge-evidence/v1";

/// Semantic model revision shared by the producer and the consumer.
pub(super) const SEMANTIC_REVISION: &str = "SMERGE/1";

/// The Lean toolchain every checked claim must select.
pub(super) const LEAN_PIN: &str = "leanprover/lean4:v4.35.0";

/// First-order observable types of `SMERGE/1`.  `Program`, `Syntax`,
/// `Contract`, `Evidence`, `Certified`, `Resource` and `LiveRef` are absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Observable {
    Unit,
    Bool,
    I64,
    Text,
}

impl Observable {
    pub(super) fn lean(self) -> &'static str {
        match self {
            Observable::Unit => "Ty.unit",
            Observable::Bool => "Ty.bool",
            Observable::I64 => "Ty.i64",
            Observable::Text => "Ty.text",
        }
    }
}

/// A retained preparation: the consumer's own record of one exact subject.
#[derive(Clone, Debug)]
pub(super) struct Preparation {
    pub(super) program: Program,
    pub(super) input: Vec<Observable>,
    pub(super) output: Vec<Observable>,
}

impl Preparation {
    /// Lower one `Program` to the MC1 reference code the Lean model runs.
    /// Captures are inlined as literals: a capture is data here, and the
    /// stored preparation keeps the capture list for identity.
    pub(super) fn lean_code(&self) -> String {
        let mut rendered = String::from("[");
        let mut at = 0usize;
        while at < self.program.ops.len() {
            if at != 0 {
                rendered.push_str(", ");
            }
            match self.program.ops[at] {
                Op::Push(value) => {
                    rendered.push_str(&format!("Op.lit (Value.i64 (BitVec.ofNat 64 {value}))"));
                }
                Op::PushCapture(index) => {
                    let value = self.program.captures.get(index).copied().unwrap_or(0);
                    rendered.push_str(&format!("Op.lit (Value.i64 (BitVec.ofNat 64 {value}))"));
                }
                Op::Add => rendered.push_str("Op.word 4"),
                Op::Sub => rendered.push_str("Op.word 5"),
                Op::Swap => rendered.push_str("Op.word 2"),
                Op::Drop => rendered.push_str("Op.word 1"),
            }
            at += 1;
        }
        rendered.push(']');
        rendered
    }

    fn lean_types(types: &[Observable]) -> String {
        let mut rendered = String::from("[");
        let mut at = 0usize;
        while at < types.len() {
            if at != 0 {
                rendered.push_str(", ");
            }
            rendered.push_str(types[at].lean());
            at += 1;
        }
        rendered.push(']');
        rendered
    }

    /// Exact subject identity: the semantic revision, the complete ordered
    /// interfaces, the program body, its captures and its callee.  Length
    /// delimiters keep an op list, a capture list and a callee distinct even
    /// when their flattened numbers coincide.
    pub(super) fn subject_digest(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(SEMANTIC_REVISION.as_bytes());
        digest.update(b"input");
        digest.update(&(self.input.len() as u64).to_le_bytes());
        for ty in &self.input {
            digest.update(ty.lean().as_bytes());
        }
        digest.update(b"output");
        digest.update(&(self.output.len() as u64).to_le_bytes());
        for ty in &self.output {
            digest.update(ty.lean().as_bytes());
        }
        digest.update(b"program");
        digest.update(&(self.program.ops.len() as u64).to_le_bytes());
        let mut at = 0usize;
        while at < self.program.ops.len() {
            let (code, payload) = match self.program.ops[at] {
                Op::Push(value) => (1u64, value),
                Op::PushCapture(index) => (2u64, index as u64),
                Op::Add => (3, 0),
                Op::Sub => (4, 0),
                Op::Swap => (5, 0),
                Op::Drop => (6, 0),
            };
            digest.update(&code.to_le_bytes());
            digest.update(&payload.to_le_bytes());
            at += 1;
        }
        digest.update(b"captures");
        digest.update(&(self.program.captures.len() as u64).to_le_bytes());
        for value in &self.program.captures {
            digest.update(&value.to_le_bytes());
        }
        digest.update(b"callee");
        digest.update(&self.program.callee.to_le_bytes());
        digest.finish()
    }

    pub(super) fn lean_subject(&self) -> String {
        self.lean_code()
    }
}

/// The four consumer-selected exact subjects and the common observable list.
#[derive(Clone, Debug)]
pub(super) struct Selection {
    pub(super) base: Preparation,
    pub(super) first: Preparation,
    pub(super) second: Preparation,
    pub(super) candidate: Preparation,
    pub(super) input: Vec<Observable>,
    pub(super) output: Vec<Observable>,
    pub(super) precondition: Precondition,
}

impl Selection {
    fn preparations(&self) -> [&Preparation; 4] {
        [&self.base, &self.first, &self.second, &self.candidate]
    }

    fn subject_digests(&self) -> [String; 4] {
        let mut digests = [String::new(), String::new(), String::new(), String::new()];
        for (at, preparation) in self.preparations().into_iter().enumerate() {
            digests[at] = preparation.subject_digest();
        }
        digests
    }
}

/// The exact regenerated claim and everything the consumer derived itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Claim {
    pub(super) relation: &'static str,
    pub(super) semantic_revision: &'static str,
    pub(super) claim_text: String,
    pub(super) subject_digests: [String; 4],
    pub(super) proof_digest: String,
}

impl Claim {
    pub(super) fn digest(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(RELATION_REVISION.as_bytes());
        digest.update(self.claim_text.as_bytes());
        for subject in &self.subject_digests {
            digest.update(subject.as_bytes());
        }
        digest.finish()
    }
}

/// Producer-supplied evidence.  Every field is untrusted: `admit` compares it
/// with the consumer's own regeneration and refuses any difference.
#[derive(Clone, Debug)]
pub(super) struct Evidence {
    pub(super) schema: String,
    pub(super) relation: String,
    pub(super) revision: String,
    pub(super) subjects: [String; 4],
    pub(super) claim: String,
    pub(super) assumptions: Vec<String>,
    pub(super) claim_hash: String,
    pub(super) trusted: bool,
    pub(super) proof: String,
    pub(super) claimed_outcome: Outcome,
}

impl Evidence {
    /// A producer rendering of the claim for a selection it was handed.  The
    /// producer can also lie; the consumer never trusts these fields.
    pub(super) fn render(selection: &Selection, proof: String) -> Result<Self, Refusal> {
        let claim = regenerate(selection)?;
        Ok(Self {
            schema: EVIDENCE_SCHEMA.into(),
            relation: RELATION_REVISION.into(),
            revision: SEMANTIC_REVISION.into(),
            subjects: claim.subject_digests.clone(),
            claim: claim.claim_text.clone(),
            assumptions: Vec::new(),
            claim_hash: claim.digest(),
            trusted: false,
            proof,
            claimed_outcome: Outcome::Unknown,
        })
    }
}

/// Refusal kinds.  Every one maps to a listed nonverified outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    IncompatibleInterface,
    UnsupportedObservable,
    VacuousPrecondition,
    UnsupportedFragment,
    StaleSubject,
    WrongClaim,
    WrongAssumptions,
    WrongRevision,
    TrustFlag,
    WrongSchema,
    WrongHash,
    WrongOutcome,
}

impl Refusal {
    pub(super) fn outcome(self) -> Outcome {
        match self {
            Refusal::IncompatibleInterface => Outcome::Unsupported,
            Refusal::UnsupportedObservable => Outcome::Unsupported,
            Refusal::VacuousPrecondition => Outcome::Unsupported,
            Refusal::UnsupportedFragment => Outcome::Unsupported,
            Refusal::StaleSubject => Outcome::Unsupported,
            Refusal::WrongClaim => Outcome::Unknown,
            Refusal::WrongAssumptions => Outcome::Unknown,
            Refusal::WrongRevision => Outcome::Unknown,
            Refusal::TrustFlag => Outcome::Unknown,
            Refusal::WrongSchema => Outcome::Unknown,
            Refusal::WrongHash => Outcome::Unknown,
            Refusal::WrongOutcome => Outcome::Unknown,
        }
    }
}

/// The checker's outcome lattice.  Only `Verified` renders as success.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Outcome {
    Verified,
    Conflict,
    ValidatedCounterexample,
    Unknown,
    Unsupported,
    Exhausted,
    InternalFailure,
}

impl Outcome {
    pub(super) fn name(self) -> &'static str {
        match self {
            Outcome::Verified => "verified",
            Outcome::Conflict => "conflict",
            Outcome::ValidatedCounterexample => "validated-counterexample",
            Outcome::Unknown => "unknown",
            Outcome::Unsupported => "unsupported",
            Outcome::Exhausted => "exhausted",
            Outcome::InternalFailure => "internal-failure",
        }
    }

    pub(super) fn is_verified(self) -> bool {
        self == Outcome::Verified
    }
}

/// The consumer's regeneration: it selects the subjects, interface and
/// observable list itself, requires exact `Ty` equality across the four
/// preparations, an observable-only output stack, `true` preconditions and a
/// nonempty input domain, then renders the exact `MergeRel` proposition.
pub(super) fn regenerate(selection: &Selection) -> Result<Claim, Refusal> {
    if selection.precondition != Precondition::True {
        return Err(Refusal::VacuousPrecondition);
    }
    // The relation quantifies over a nonempty input domain.  Every observable
    // type of the fragment is inhabited, so a nonempty input stack is the
    // non-vacuity witness; an empty input stack would quantify over nothing.
    if selection.input.is_empty() {
        return Err(Refusal::VacuousPrecondition);
    }
    for preparation in selection.preparations() {
        if preparation.input != selection.input || preparation.output != selection.output {
            return Err(Refusal::IncompatibleInterface);
        }
    }
    let claim_text = format!(
        "MergeRel {} {} {} {} {} {}",
        Preparation::lean_types(&selection.input),
        Preparation::lean_types(&selection.output),
        selection.base.lean_subject(),
        selection.first.lean_subject(),
        selection.second.lean_subject(),
        selection.candidate.lean_subject(),
    );
    Ok(Claim {
        relation: RELATION_REVISION,
        semantic_revision: SEMANTIC_REVISION,
        claim_text,
        subject_digests: selection.subject_digests(),
        proof_digest: String::new(),
    })
}

/// The consumer's admission rule: regenerate, then refuse every producer
/// field that differs.  On success the returned claim is the consumer's own
/// and the proof bytes are bound to it.
pub(super) fn admit(selection: &Selection, evidence: &Evidence) -> Result<Claim, Refusal> {
    let mut claim = regenerate(selection)?;
    if evidence.schema != EVIDENCE_SCHEMA {
        return Err(Refusal::WrongSchema);
    }
    if evidence.relation != RELATION_REVISION || evidence.revision != SEMANTIC_REVISION {
        return Err(Refusal::WrongRevision);
    }
    if !evidence.assumptions.is_empty() {
        return Err(Refusal::WrongAssumptions);
    }
    if evidence.trusted {
        return Err(Refusal::TrustFlag);
    }
    if evidence.subjects != claim.subject_digests {
        return Err(Refusal::StaleSubject);
    }
    if evidence.claim != claim.claim_text {
        return Err(Refusal::WrongClaim);
    }
    if evidence.claim_hash != claim.digest() {
        return Err(Refusal::WrongHash);
    }
    if evidence.claimed_outcome.is_verified() {
        return Err(Refusal::WrongOutcome);
    }
    claim.proof_digest = sha256(evidence.proof.as_bytes());
    Ok(claim)
}

/// The only path to `Verified`: the isolated pinned Lean consumer accepted the
/// exact regenerated claim for the bound proof bytes.
pub(super) fn checked(claim: &Claim, accepted: bool, axioms: &[String]) -> Outcome {
    let allowed = ["propext", "Classical.choice", "Quot.sound"];
    let mut is_allowed = true;
    let mut at = 0usize;
    while at < axioms.len() {
        is_allowed = is_allowed && allowed.iter().any(|name| axioms[at] == *name);
        at += 1;
    }
    // `admit` binds the proof bytes before the checker runs; a claim whose
    // proof digest is unset was never admitted and cannot be verified.
    if accepted && is_allowed && claim.proof_digest.len() == 64 {
        Outcome::Verified
    } else {
        Outcome::Unknown
    }
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    digest.finish()
}

/// A bounded SHA-256 for source-bound evidence digests.  The digest is an
/// integrity index, not proof validity.
struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    buffered: usize,
    length: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            block: [0; 64],
            buffered: 0,
            length: 0,
        }
    }

    fn update(&mut self, mut input: &[u8]) {
        while !input.is_empty() {
            let take = core::cmp::min(64 - self.buffered, input.len());
            let mut at = 0usize;
            while at < take {
                self.block[self.buffered + at] = input[at];
                at += 1;
            }
            self.buffered += take;
            self.length += take as u64;
            input = &input[take..];
            if self.buffered == 64 {
                let block = self.block;
                self.compress(&block);
                self.buffered = 0;
            }
        }
    }

    fn finish(mut self) -> String {
        let bits = self.length.wrapping_mul(8);
        self.update(&[0x80]);
        while self.buffered != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut rendered = String::with_capacity(64);
        for word in self.state {
            let mut shift = 28i32;
            while shift >= 0 {
                rendered.push(HEX[((word >> shift) & 0xf) as usize] as char);
                shift -= 4;
            }
        }
        rendered
    }

    fn compress(&mut self, block: &[u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut schedule = [0u32; 64];
        let mut at = 0usize;
        while at < 16 {
            schedule[at] = u32::from_be_bytes([
                block[at * 4],
                block[at * 4 + 1],
                block[at * 4 + 2],
                block[at * 4 + 3],
            ]);
            at += 1;
        }
        while at < 64 {
            let s0 = schedule[at - 15].rotate_right(7)
                ^ schedule[at - 15].rotate_right(18)
                ^ (schedule[at - 15] >> 3);
            let s1 = schedule[at - 2].rotate_right(17)
                ^ schedule[at - 2].rotate_right(19)
                ^ (schedule[at - 2] >> 10);
            schedule[at] = schedule[at - 16]
                .wrapping_add(s0)
                .wrapping_add(schedule[at - 7])
                .wrapping_add(s1);
            at += 1;
        }
        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];
        let mut round = 0usize;
        while round < 64 {
            let big_s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ (!e & g);
            let first = h
                .wrapping_add(big_s1)
                .wrapping_add(choose)
                .wrapping_add(K[round])
                .wrapping_add(schedule[round]);
            let big_s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let second = big_s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(first);
            d = c;
            c = b;
            b = a;
            a = first.wrapping_add(second);
            round += 1;
        }
        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }
}
