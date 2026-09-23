//! Companion session operations.
//!
//! One persistent guest session holds first-class `Contract`, `Evidence` and
//! `Certified` values. Guest cells are inert descriptions; every acceptance
//! decision is made by the deterministic Rust core in `noble-contracts`, from
//! complete observations the guest reports. Only explicit contract admission
//! invokes the independent Lean consumer; runtime operations never fetch or
//! recheck evidence implicitly.
//!
//! The script grammar is one operation per line, `#` starts a comment, and
//! every operation prints exactly one `noble-companions-report/v1` JSON line.
//! An operation that refuses (an unresolved composition, a failed guard, a
//! malformed argument) prints a refusal line and keeps the session usable;
//! only unreadable input stops the script.

#[cfg(test)]
mod controls;

pub const USAGE: &str = "usage:
  noble companions [--timeout-ms N] [--opt on|off] < SCRIPT
Script operations (one per line; '#' starts a comment):
  contract PATH [PROOF_PATH]      prepare and admit one contract (+ optional proof)
  submit PATH                     execute one source submission in the session
  plain PATH                      execute source with no contract profile at all
  stack                           report the current session stack
  project INDEX                   project a Certified companion to its subject
  inspect INDEX                   read retained companion metadata only
  certify INDEX [CONTRACT_ID]      bind a Program slot to an admitted contract
  compose LEFT RIGHT ARGUMENT     certified composition of two companions
  instantiate INDEX CAPTURE [ARG]  instantiate a builder; optionally run its result
  invoke INDEX ARGUMENT           guard-checked certified invocation
  policy REVISION                 select a policy; invalidate old evidence
  derive RULE CONTRACT_ID SUBJECT_INDEX EVIDENCE_ID...
                                  submit an explicit bounded replay offer
Exit status is 2 for malformed input or I/O failure; every accepted script
prints its per-operation outcomes and exits 0. The Lean kernel runs only for
'contract PATH PROOF_PATH'.";

/// Whole-script budget; larger scripts are refused rather than truncated.
const SCRIPT_LIMIT: u32 = 1_048_576;
/// Contract source and proof source budgets.
const SOURCE_LIMIT: usize = 65_536;
const PROOF_LIMIT: usize = 524_288;
/// Default consumer proof budget, matching `noble verify`.
const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
/// The claim kind these values carry: partial correctness.
const CLAIM_PARTIAL_CORRECTNESS: u32 = 1;
/// The prechecked rule set version these values carry.
const RULESET: u32 = noble_contracts::companion::RULESET_V1;

struct Published {
    contract_handle: u64,
    evidence_handle: u64,
    certified_handle: u64,
    subject_identity: u64,
}

/// One retained admission or derivation, indexed by the core's identifiers.
/// Nothing here is authority: the core retains the statement digest itself.
#[derive(Clone, Copy)]
struct Record {
    contract: noble_contracts::companion::ContractId,
    statement: u64,
    evidence: Option<noble_contracts::companion::EvidenceId>,
    class: Option<noble_contracts::companion::EvidenceClass>,
}

/// The session cells a metered companion operation parked.
struct Parked {
    types: std::vec::Vec<noble_kernel::types::Ty>,
}

/// A refusal that ends one operation but keeps the session usable.
struct Refused {
    outcome: &'static str,
    code: std::string::String,
    message: std::string::String,
}

type Attempt<T> = Result<T, crate::core::companions::Refused>;

impl crate::core::companions::Refused {
    fn new(
        outcome: &'static str,
        code: impl Into<std::string::String>,
        message: std::string::String,
    ) -> Self {
        Self {
            outcome,
            code: code.into(),
            message,
        }
    }

    /// A core refusal: complete data, declined decision.
    fn core(refusal: noble_contracts::companion::Refusal) -> Self {
        Self::new(
            crate::core::companions::refusal_outcome(&refusal),
            refusal.code(),
            std::string::String::from("the deterministic core declined"),
        )
    }

    fn script(message: impl Into<std::string::String>) -> Self {
        Self::new("malformed", "script", message.into())
    }

    fn engine(failure: crate::core::output::Failure) -> Self {
        Self::new(failure.context.outcome, "engine", failure.message)
    }
}

/// Outcome spelling for a core refusal. Refusals that the specification names
/// keep their own name; every other refusal is `refused`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; classification calls the core's non-const refusal code API and compares runtime str values, which are not const-compatible on the pinned compiler."
)]
fn refusal_outcome(refusal: &noble_contracts::companion::Refusal) -> &'static str {
    match refusal.code() {
        "unresolved-implication" => "unresolved-implication",
        "stale-context" => "stale-context",
        "mismatched-subject" => "mismatched-subject",
        "mismatched-claim" => "mismatched-claim",
        "mismatched-context" => "mismatched-context",
        "forged-status" => "forged-status",
        "wrong-instantiation" => "wrong-instantiation",
        "exhausted-replay" => "exhausted-replay",
        "exhausted-registry" => "exhausted-registry",
        "unsupported-guard-template" => "unsupported-guard-template",
        "unsupported-evidence-class" => "unsupported-evidence-class",
        "ineligible-ghost" => "ineligible-ghost",
        "live-capability-in-evidence" => "live-capability-in-evidence",
        "correspondence-mismatch" => "correspondence-mismatch",
        "unknown-contract" => "unknown-contract",
        "unknown-evidence" => "unknown-evidence",
        _ => "refused",
    }
}

/// A subject program handle together with its exact stack type.
struct ProgramSlot {
    handle: u64,
    ty: noble_kernel::types::Ty,
}

struct CertifiedSlot {
    subject: u64,
    contract: noble_contracts::companion::ContractId,
    evidence: noble_contracts::companion::EvidenceId,
    identity: std::string::String,
}

/// The persistent session.
struct Driver {
    frontend: noble_contracts::source::Session,
    compiler: noble_wasm::source::Compiler,
    stack: std::vec::Vec<noble_kernel::types::Ty>,
    programs: std::vec::Vec<crate::core::companions::ProgramSlot>,
    worker: Option<crate::core::worker::Engine>,
    submissions: u64,
    core: noble_contracts::companion::Core,
    records: std::vec::Vec<crate::core::companions::Record>,
    /// Consumer-selected policy revision these values were published under.
    policy: u32,
    timeout_ms: u64,
    optimized: bool,
}

mod entry;
mod operations;
mod reporting;
mod session;

pub fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    entry::run(arguments)
}
