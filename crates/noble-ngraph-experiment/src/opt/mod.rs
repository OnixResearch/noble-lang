//! Restricted typed optimizer-kernel experiment (`ngraph-opt/1`).
//!
//! This crate interprets only the closed candidate envelope; it does not load
//! foreign code or grant candidates host capabilities. Finite comparisons are
//! evidence about these rows, not a proof of Rust/Lean implementation
//! correspondence.

mod candidate;
mod evaluate;
mod hash;
mod isolation;
mod snapshot;
mod trial;
mod types;

pub use candidate::Candidate;
pub use isolation::{
    evaluate_frozen, Evaluation, EvaluationRow, EvaluationRowResult, FrozenCandidate,
    FrozenDataset, FrozenEvaluation, FrozenRow,
};
pub use snapshot::{OptSnapshot, SnapshotContext, SnapshotProvenance};
pub use trial::{
    run_trial, AttemptEvent, AttemptLog, AttemptRecord, Checkpoint, Counters, FaultInjection,
    RngCheckpoint, TrialFailure, TrialResult, TrialStatus,
};
pub use types::{
    classify_bits, Digest, NonFinite, Refusal, RefusalClass, StepDescriptor, TransitionInput,
    TransitionOutput, NUMERIC_SEMANTICS,
};
