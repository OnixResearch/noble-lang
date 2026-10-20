//! Persistent Core-Bootstrap compilation. All supplied payloads are untrusted.
//!
//! Preparation checks the exact environment, every concrete definition and the
//! root before emitting code. It never changes this compiler. Commit consumes
//! the preparation and rejects stale or different base allocation states.
//! Runtime instances must share the objects described in
//! `noble-cli/src/core/runtime/abi.json`; no recipes execute.

mod admission;
mod emit;
mod plan;
mod preflight;
mod transaction;
mod types;

const TABLE_LIMIT: u32 = 16_384;
// Calculator table slots 0..=4 hold the generation-0 core entries; compiled
// code is allocated from here.
const CALCULATOR_FUNCTION_BASE: u32 = 5;
const TEXT_START: u32 = 262_144;
const CALCULATOR_TEXT_START: u32 = 524_288;
const MEMORY_BYTES: u32 = 1_048_576;
const NODE_LIMIT: usize = 4096;
const DEFINITION_LIMIT: usize = 256;
// The opt-in calculator's checked source closure can include numeric modules
// whose 321 reachable named bodies exceed the older 256-definition profile.
const CALCULATOR_DEFINITION_LIMIT: usize = 512;
// Calculator source and compiler share a 896-row bound. The independently
// bounded named-body count and per-clone/fanout budgets are separate gates.
// Retain the complete E×B scans rather than discounting them. This 2b limit
// caps issued preparation accounting, NOT physical CPU instructions or
// resident bytes; preclone structural guards still reject amplification.
pub(super) const CALCULATOR_COMPILER_WORK_LIMIT: u64 = 2_000_000_000;

// Core live sessions reserve 24/25 for their two checked operations.
// Ordinary and declared sessions retain their original fixed prefixes.
fn builtin_count(env: &noble_kernel::contracts::Env) -> u32 {
    if env.calculator && env.declared_modules {
        32
    } else if env.text_cursor && !env.declared_modules {
        27
    } else if !env.declared_modules && env.effects.contains(&noble_kernel::types::EffId(3)) {
        26
    } else if !env.declared_modules && env.effects.contains(&noble_kernel::types::EffId(1)) {
        24
    } else {
        23
    }
}

struct Work {
    // Spent is always issued - remaining; both the original caller budget
    // and every selected allowance stay in the same cumulative ledger.
    remaining: u64,
    issued: u64,
    // Only borrowed calculator preflight may charge its measured inspection
    // on top of the caller's still-reserved kernel acceptance budget.
    calculator_preflight: bool,
    calculator_plan: bool,
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the budget belongs to one private preparation and tracks consumed work without modifying the borrowed submission or published compiler."
)]
impl Work {
    const fn charge(&mut self, amount: u64) -> Result<(), crate::Diagnostic> {
        if self.calculator_preflight && amount != 0 {
            // No owned kernel clone can begin until the complete borrowed
            // preflight succeeds. Keep every actual inspection charge in the
            // same finite ledger without spending the caller's body budget.
            attempt!(self.grant_calculator(amount));
        } else if self.calculator_plan && amount > self.remaining {
            // The independent kernel, preflight and environment admission have
            // already finished. Issue only the shortfall for this measured
            // plan operation; never reset or discount previously charged work.
            attempt!(self.grant_calculator(amount - self.remaining));
        }
        self.remaining = match self.remaining.checked_sub(amount) {
            Some(value) => value,
            None => return Err(crate::Diagnostic::Exhausted),
        };
        Ok(())
    }

    /// Only the opt-in calculator grants measured structural compilation
    /// work. The cumulative issued amount, not just unspent work, is capped.
    const fn grant_calculator(&mut self, amount: u64) -> Result<(), crate::Diagnostic> {
        let Some(issued) = self.issued.checked_add(amount) else {
            return Err(crate::Diagnostic::Exhausted);
        };
        let Some(remaining) = self.remaining.checked_add(amount) else {
            return Err(crate::Diagnostic::Exhausted);
        };
        if issued > CALCULATOR_COMPILER_WORK_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        self.issued = issued;
        self.remaining = remaining;
        Ok(())
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; charging a platform-sized entry count uses non-const TryFrom and preserves Exhausted when the count cannot fit the u64 work meter."
    )]
    fn entries(&mut self, count: usize) -> Result<(), crate::Diagnostic> {
        match u64::try_from(count) {
            Ok(count) => self.charge(count),
            Err(_) => Err(crate::Diagnostic::Exhausted),
        }
    }
}

/// Persistent semantic identities and disjoint compiled-code allocations.
pub struct Compiler {
    live: bool,
    text_cursor: bool,
    calculator: bool,
    generation: u32,
    functions: u32,
    text_end: u32,
    signatures: crate::signatures::Pool,
    descriptors: alloc::vec::Vec<(u32, u32)>,
    identities: alloc::vec::Vec<Identity>,
}

#[derive(Clone)]
struct Identity {
    id: u64,
    recipe: alloc::vec::Vec<u8>,
}

/// Complete checked WAT plus a private prospective compiler state.
pub struct Prepared {
    base: transaction::Base,
    next: Compiler,
    wat: alloc::vec::Vec<u8>,
}

impl Prepared {
    /// The complete module. Instantiation does not execute its body.
    pub fn wat(&self) -> &[u8] {
        &self.wat
    }
}

impl Compiler {
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; signature interning updates only the unpublished prospective compiler and its preparation-owned meter; caller type stacks remain immutable."
    )]
    fn signature(
        &mut self,
        stack: &[noble_kernel::types::Ty],
        work: &mut Work,
    ) -> Result<u32, crate::Diagnostic> {
        let bytes = attempt!(admission::meter::stack(stack, work));
        attempt!(work.charge(bytes));
        self.signatures.intern_charged(stack, |visited| work.entries(visited))
    }

    /// A fresh compiler requires a fresh runtime import-object set.
    pub fn new() -> Self {
        Self {
            live: false,
            text_cursor: false,
            calculator: false,
            generation: 0,
            functions: 4,
            text_end: TEXT_START,
            signatures: crate::signatures::Pool::source(),
            descriptors: alloc::vec::Vec::new(),
            identities: alloc::vec::Vec::new(),
        }
    }

    /// A separate admission profile: the ordinary compiler never emits the
    /// proposal or generation imports.
    pub fn new_live() -> Self {
        Self { live: true, ..Self::new() }
    }

    /// The pure byte cursor is a separate compiler admission profile.
    pub fn new_text_cursor() -> Self {
        Self { text_cursor: true, ..Self::new() }
    }

    /// Independent calculator admission with one reserved trampoline slot.
    pub fn new_calculator() -> Self {
        let mut compiler = Self::new();
        compiler.calculator = true;
        compiler.functions = CALCULATOR_FUNCTION_BASE;
        compiler.text_end = CALCULATOR_TEXT_START;
        compiler.signatures.select_calculator();
        compiler
    }

    /// Independently accept and lower without changing existing code identities.
    pub fn prepare(
        &self,
        submission: &noble_kernel::execution::Submission,
    ) -> Result<Prepared, crate::Diagnostic> {
        self.prepare_from(submission, self.functions, false, false)
    }

    /// Calculator-only preparation at a quiescent submission boundary: the
    /// runtime reported that no Program reachable from the operand stack runs
    /// compiled code. Every module re-lowers its own definition closure, so
    /// all committed compiled-code slots are then dead and this module's
    /// table initialization overwrites them from the fixed base. The
    /// definition-identity ledger likewise restarts empty: identities are
    /// session-stable ids checked against their recipes within one epoch, and
    /// no earlier epoch's compiled body can run again. Only the prospective
    /// state rewinds; commit publishes it, refusal changes nothing.
    pub fn prepare_quiescent(
        &self,
        submission: &noble_kernel::execution::Submission,
    ) -> Result<Prepared, crate::Diagnostic> {
        if !self.calculator {
            return Err(crate::Diagnostic::Invalid);
        }
        self.prepare_from(submission, CALCULATOR_FUNCTION_BASE, true, false)
    }

    /// Calculator-only preparation at a Program-free submission boundary: the
    /// runtime reported that no Program, Syntax or recipe cell is reachable
    /// from the operand stack. Signature IDs are held only by those cells, so
    /// beyond the quiescent rewind the signature pool and its published
    /// descriptors restart empty too, and this module republishes its keys
    /// from ID 0. Static literal Text is never rewound.
    pub fn prepare_program_free(
        &self,
        submission: &noble_kernel::execution::Submission,
    ) -> Result<Prepared, crate::Diagnostic> {
        if !self.calculator {
            return Err(crate::Diagnostic::Invalid);
        }
        self.prepare_from(submission, CALCULATOR_FUNCTION_BASE, true, true)
    }

    fn prepare_from(
        &self,
        submission: &noble_kernel::execution::Submission,
        functions: u32,
        quiescent: bool,
        program_free: bool,
    ) -> Result<Prepared, crate::Diagnostic> {
        if self.calculator
            && u64::from(submission.request.limits.work) > CALCULATOR_COMPILER_WORK_LIMIT
        {
            return Err(crate::Diagnostic::Exhausted);
        }
        let mut work = Work {
            remaining: u64::from(submission.request.limits.work),
            issued: u64::from(submission.request.limits.work),
            calculator_preflight: self.calculator,
            calculator_plan: false,
        };
        attempt!(preflight::check(submission, self.calculator, &mut work));
        work.calculator_preflight = false;
        let checked = attempt!(admission::check(
            submission,
            self.live,
            self.text_cursor,
            self.calculator,
            &mut work,
        ));
        // A Program-free epoch interns into a fresh calculator pool, so no
        // committed key is retained, copied or charged.
        let fresh_signatures = if program_free {
            let mut pool = crate::signatures::Pool::source();
            pool.select_calculator();
            Some(pool)
        } else {
            None
        };
        attempt!(admission::meter::compilation(
            &checked,
            fresh_signatures.as_ref().unwrap_or(&self.signatures),
            &mut work
        ));
        // A quiescent epoch starts with an empty identity ledger: nothing of
        // the committed ledger is retained, so none of it is copied or charged.
        let retained = if quiescent { 0 } else { self.identities.len() };
        let mut identity = 0usize;
        let mut failure = None;
        while identity < retained {
            match work.entries(self.identities[identity].recipe.len()) {
                Ok(()) => identity += 1,
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        let mut next = Self {
            live: self.live,
            text_cursor: self.text_cursor,
            calculator: self.calculator,
            generation: match self.generation.checked_add(1) {
                Some(value) => value,
                None => return Err(crate::Diagnostic::Exhausted),
            },
            functions,
            text_end: self.text_end,
            signatures: match fresh_signatures {
                Some(pool) => pool,
                None => self.signatures.clone(),
            },
            descriptors: if program_free {
                alloc::vec::Vec::new()
            } else {
                self.descriptors.clone()
            },
            identities: if quiescent {
                alloc::vec::Vec::new()
            } else {
                self.identities.clone()
            },
        };
        attempt!(admission::identity::check(
            submission,
            &mut next.identities,
            next.calculator,
            &mut work
        ));
        work.calculator_plan = self.calculator;
        let plan = attempt!(plan::lower(submission, &checked, &mut next, &mut work));
        // A calculator module publishes only the signatures new since the
        // committed compiler's descriptor table (all of its own after a
        // Program-free rewind); the trusted engine retains that published
        // prefix, so per-module metadata is history-independent.
        let published = if program_free { 0 } else { self.descriptors.len() };
        let wat = if next.calculator {
            let keys = attempt!(next.signatures.expanded_keys(published, |bytes| work.entries(bytes)));
            attempt!(emit::module(&plan, self.generation, published, &keys))
        } else {
            attempt!(emit::module(&plan, self.generation, 0, &next.signatures.keys))
        };
        Ok(Prepared {
            base: transaction::Base::capture(
                self,
                retained,
                if program_free { 0 } else { self.signatures.len() },
                published,
            ),
            next,
            wat,
        })
    }

    /// Check publication provenance before allowing a live worker to install
    /// prospective bytes in a persistent arena. No compiler state is changed.
    pub fn can_commit(&self, prepared: &Prepared) -> bool {
        prepared.base.matches(self, &prepared.next)
    }

    /// Publish only after the shell has accepted/instantiated the complete module.
    /// A stale or different base allocation state is rejected without mutation.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; commit is the explicit compiler-state transition: it consumes the owned preparation, checks generation and immutable allocation provenance, then publishes only the matching prospective state."
    )]
    pub fn commit(&mut self, prepared: Prepared) -> Result<(), crate::Diagnostic> {
        if !prepared.base.matches(self, &prepared.next) {
            return Err(crate::Diagnostic::Invalid);
        }
        *self = prepared.next;
        Ok(())
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{transaction, Compiler, Identity, Work, CALCULATOR_COMPILER_WORK_LIMIT, CALCULATOR_DEFINITION_LIMIT};
    use noble_kernel::types::{ResourceKind, Ty};

    #[test]
    fn borrowed_calculator_preflight_preserves_caller_work_without_escaping_its_ceiling() {
        let mut work = Work {
            remaining: 7,
            issued: CALCULATOR_COMPILER_WORK_LIMIT - 8,
            calculator_preflight: true,
            calculator_plan: false,
        };
        assert!(work.charge(7).is_ok());
        assert_eq!(work.remaining, 7);
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT - 1);
        assert!(work.charge(1).is_ok());
        assert_eq!(work.remaining, 7);
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT);
        assert!(matches!(work.charge(1), Err(crate::Diagnostic::Exhausted)));
        assert_eq!(work.remaining, 7);
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT);

        work.calculator_preflight = false;
        assert!(work.charge(7).is_ok());
        assert_eq!(work.remaining, 0);
        assert!(matches!(work.charge(1), Err(crate::Diagnostic::Exhausted)));
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT);

        let mut ordinary = Work {
            remaining: 7,
            issued: 7,
            calculator_preflight: false,
            calculator_plan: false,
        };
        assert!(matches!(ordinary.charge(8), Err(crate::Diagnostic::Exhausted)));
        assert_eq!(ordinary.issued, 7);
        assert_eq!(ordinary.remaining, 7);
    }

    #[test]
    fn calculator_quiescent_epochs_restart_the_identity_ledger() {
        let limits = noble_contracts::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let mut session = noble_contracts::source::Session::new_calculator();
        let defined = session.prepare(b"def identity_epoch_word [ 1 drop ]", &[], limits)
            .unwrap_or_else(|_| panic!("calculator definition refused"));
        session.commit(defined).unwrap_or_else(|_| panic!("calculator definition commit refused"));
        let input = session.prepare(b"identity_epoch_word", &[], limits)
            .unwrap_or_else(|_| panic!("calculator call refused"));
        let submission = input.submission()
            .unwrap_or_else(|| panic!("missing checked source submission"));
        assert!(!submission.definitions.is_empty());
        // A committed ledger already holding the limit of unrelated identities,
        // as after many earlier modules of one long session.
        let full = |count: usize| {
            let mut compiler = Compiler::new_calculator();
            compiler.identities = (0..count as u64)
                .map(|index| Identity { id: u64::MAX - index, recipe: alloc::vec![0] })
                .collect();
            compiler
        };

        let mut cumulative = full(CALCULATOR_DEFINITION_LIMIT);
        assert!(matches!(cumulative.prepare(submission), Err(crate::Diagnostic::Exhausted)),
            "a non-quiescent preparation still refuses a new identity at the limit");
        assert_eq!(cumulative.identities.len(), CALCULATOR_DEFINITION_LIMIT);

        let mut epochs = full(CALCULATOR_DEFINITION_LIMIT);
        let stale = transaction::Base::capture(&epochs, epochs.identities.len(), epochs.signatures.len(), epochs.descriptors.len());
        let prepared = epochs.prepare_quiescent(submission)
            .unwrap_or_else(|_| panic!("quiescent preparation refused past the cumulative limit"));
        assert!(epochs.can_commit(&prepared));
        assert!(!stale.matches(&epochs, &prepared.next),
            "a whole-ledger base cannot vouch for the restarted epoch");
        epochs.commit(prepared).unwrap_or_else(|_| panic!("quiescent commit refused"));
        assert_eq!(epochs.identities.len(), submission.definitions.len(),
            "the new epoch holds only this module's identities");

        // The limit is still enforced within an epoch: an ordinary preparation
        // retains the epoch's ledger and refuses once it is full again.
        let missing = CALCULATOR_DEFINITION_LIMIT - epochs.identities.len();
        epochs.identities.extend((0..missing as u64)
            .map(|index| Identity { id: u64::MAX - index, recipe: alloc::vec![0] }));
        let kept = epochs.prepare(submission)
            .unwrap_or_else(|_| panic!("known identities need no new ledger entry"));
        assert_eq!(kept.next.identities.len(), CALCULATOR_DEFINITION_LIMIT);
        assert!(epochs.can_commit(&kept));
    }

    #[test]
    fn calculator_program_free_epochs_restart_the_signature_pool() {
        let limits = noble_contracts::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let mut session = noble_contracts::source::Session::new_calculator();
        let defined = session.prepare(b"def signature_epoch_word [ 1 drop ]", &[], limits)
            .unwrap_or_else(|_| panic!("calculator definition refused"));
        session.commit(defined).unwrap_or_else(|_| panic!("calculator definition commit refused"));
        let input = session.prepare(b"signature_epoch_word", &[], limits)
            .unwrap_or_else(|_| panic!("calculator call refused"));
        let submission = input.submission()
            .unwrap_or_else(|| panic!("missing checked source submission"));
        // A committed pool already holding the key limit of unrelated
        // signatures, as after many earlier modules of one long session.
        let full = || {
            let mut compiler = Compiler::new_calculator();
            for index in 0..crate::signatures::CALCULATOR_SOURCE_KEY_LIMIT as u32 {
                assert_eq!(compiler.signatures.intern(&[Ty::Resource(ResourceKind(index))]).ok(),
                    Some(index));
            }
            compiler
        };

        let mut cumulative = full();
        assert!(matches!(cumulative.prepare(submission), Err(crate::Diagnostic::Exhausted)),
            "an ordinary preparation still refuses a new key at the limit");
        assert!(matches!(cumulative.prepare_quiescent(submission), Err(crate::Diagnostic::Exhausted)),
            "a merely quiescent preparation keeps every committed key");
        assert_eq!(cumulative.signatures.len(), crate::signatures::CALCULATOR_SOURCE_KEY_LIMIT);

        let mut epochs = full();
        let stale = transaction::Base::capture(&epochs, epochs.identities.len(),
            epochs.signatures.len(), epochs.descriptors.len());
        let prepared = epochs.prepare_program_free(submission)
            .unwrap_or_else(|_| panic!("Program-free preparation refused past the cumulative limit"));
        assert!(epochs.can_commit(&prepared));
        assert!(!stale.matches(&epochs, &prepared.next),
            "a whole-pool base cannot vouch for the restarted epoch");
        let fresh = prepared.next.signatures.len();
        assert!(fresh > 0 && fresh < 64, "the new epoch holds only this module's keys");
        assert_eq!(prepared.next.descriptors.len(), fresh);
        epochs.commit(prepared).unwrap_or_else(|_| panic!("Program-free commit refused"));
        assert_eq!(epochs.signatures.len(), fresh);

        // The key limit still applies within an epoch, and an ordinary
        // preparation keeps and checks the epoch's whole pool.
        let kept = epochs.prepare(submission)
            .unwrap_or_else(|_| panic!("known keys need no new pool entry"));
        assert_eq!(kept.next.signatures.len(), fresh);
        assert!(epochs.can_commit(&kept));
        assert!(matches!(Compiler::new().prepare_program_free(submission),
            Err(crate::Diagnostic::Invalid)), "only the calculator profile may restart signatures");
    }

    #[test]
    fn calculator_many_signatures_cannot_exceed_cumulative_plan_ceiling() {
        let mut compiler = Compiler::new_calculator();
        let mut work = Work {
            remaining: 0,
            issued: CALCULATOR_COMPILER_WORK_LIMIT - 1_000_000,
            calculator_preflight: false,
            calculator_plan: true,
        };
        for length in 1..=64 {
            assert!(compiler.signature(&alloc::vec![Ty::I64; length], &mut work).is_ok(),
                "finite distinct structural signatures fit the remaining ceiling");
        }
        assert_eq!(compiler.signatures.len(), 64);
        assert!(work.issued < CALCULATOR_COMPILER_WORK_LIMIT);

        // A repeated first key compares once; scanning and reserving all 64
        // possible keys would falsely refuse this valid near-ceiling lookup.
        work.issued = CALCULATOR_COMPILER_WORK_LIMIT - 4_096;
        work.remaining = 0;
        assert_eq!(compiler.signature(&[Ty::I64], &mut work).ok(), Some(0));
        assert!(work.issued <= CALCULATOR_COMPILER_WORK_LIMIT);
        assert_eq!(compiler.signatures.len(), 64);

        // The next distinct key must compare against the existing pool. Even
        // though just one unit can be issued, a longer comparison must fail
        // before subtraction or mutation wraps the cumulative accounting.
        work.issued = CALCULATOR_COMPILER_WORK_LIMIT - 1;
        work.remaining = 0;
        assert!(matches!(
            compiler.signature(&alloc::vec![Ty::Bool; 64], &mut work),
            Err(crate::Diagnostic::Exhausted)
        ));
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT - 1);
        assert_eq!(work.remaining, 0);
        assert_eq!(compiler.signatures.len(), 64);
        assert!(matches!(work.charge(u64::MAX), Err(crate::Diagnostic::Exhausted)));
        assert_eq!(work.issued, CALCULATOR_COMPILER_WORK_LIMIT - 1);
        assert_eq!(work.remaining, 0);
    }

    #[test]
    fn calculator_module_metadata_does_not_grow_with_retained_signature_history() {
        let session = noble_contracts::source::ModuleSession::new_calculator()
            .unwrap_or_else(|_| panic!("calculator session refused"));
        let limits = noble_contracts::Limits {
            bytes: 262_144,
            nodes: 524_288,
            depth: 64,
            work: 512_000_000,
        };
        let input = session.prepare(b"", &[], limits)
            .unwrap_or_else(|_| panic!("empty calculator source refused"));
        let submission = input.submission()
            .unwrap_or_else(|| panic!("missing checked source submission"));
        let step = |compiler: &mut Compiler| {
            let prepared = compiler.prepare(submission)
                .unwrap_or_else(|_| panic!("calculator prepare refused"));
            let wat = prepared.wat().to_vec();
            compiler.commit(prepared).unwrap_or_else(|_| panic!("calculator commit refused"));
            wat
        };
        let published = |wat: &[u8]| wat.split(|byte| *byte == b'\n')
            .filter(|line| line.starts_with(b"(data $sig")).count();

        let mut fresh = Compiler::new_calculator();
        step(&mut fresh);
        let fresh_steady = step(&mut fresh);

        // About 1.4 MB of unrelated retained keys, comparable to a long
        // calculator session, are published once by the next module.
        let mut retained = Compiler::new_calculator();
        for index in 0..3_000u32 {
            let stack = alloc::vec![Ty::Resource(ResourceKind(index)); 64];
            assert_eq!(retained.signatures.intern(&stack).ok(), Some(index));
        }
        let publishing = step(&mut retained);
        assert!(published(&publishing) > 3_000);
        assert!(publishing.len() > 1_000_000);
        let retained_steady = step(&mut retained);
        assert_eq!(retained.signatures.len(), fresh.signatures.len() + 3_000);

        // Later modules carry no retained key, branch or segment: they differ
        // from a fresh session's only in the decimal published-range bounds.
        assert_eq!(published(&fresh_steady), 0);
        assert_eq!(published(&retained_steady), 0);
        assert!(retained_steady.len() <= fresh_steady.len() + 16,
            "steady module grew from {} to {} bytes with retained history",
            fresh_steady.len(), retained_steady.len());
    }
}
