//! Host-owned trial checkpoints, deterministic data selection and append-only attempt records.

use noble_ngraph::prng::SplitMix64;

use super::candidate::Candidate;
use super::isolation::FrozenDataset;
use super::snapshot::OptSnapshot;
use super::types::Digest;
use super::types::{Refusal, RefusalClass, StepDescriptor, TransitionInput};

/// Complete host RNG state as a seed plus consumed `splitmix64/0` draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RngCheckpoint {
    /// Seed, also bound by snapshot provenance.
    pub seed: u64,
    /// Number of consumed outputs.
    pub draws: u64,
}

/// Counters that are part of trial state and therefore restored on rollback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counters {
    /// Successfully staged updates.
    pub updates: u64,
    /// Measured scalar mul/sub lane invocations.
    pub work: u64,
}

/// Durable host checkpoint for an entire experiment trial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    /// Last immutable candidate snapshot.
    pub snapshot: OptSnapshot,
    /// RNG seed and draw cursor.
    pub rng: RngCheckpoint,
    /// Number of data rows consumed.
    pub data_cursor: u64,
    /// Update/work accounting.
    pub counters: Counters,
}

impl Checkpoint {
    /// Creates a checkpoint only when its RNG seed matches the bound context.
    pub fn new(
        snapshot: OptSnapshot,
        rng: RngCheckpoint,
        data_cursor: u64,
        counters: Counters,
    ) -> Result<Self, Refusal> {
        if rng.seed != snapshot.context().seed {
            return Err(Refusal::new(
                RefusalClass::Unsupported,
                "$.checkpoint.rng.seed",
                format!(
                    "{} differs from bound seed {}",
                    rng.seed,
                    snapshot.context().seed
                ),
            ));
        }
        if counters.updates > u64::from(u32::MAX) {
            return Err(Refusal::new(
                RefusalClass::Exhausted,
                "$.checkpoint.counters.updates",
                counters.updates.to_string(),
            ));
        }
        Ok(Self {
            snapshot,
            rng,
            data_cursor,
            counters,
        })
    }
}

/// Fault injected after the selected number of updates have been staged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultInjection {
    /// No injected fault.
    None,
    /// Fail after exactly this many staged updates (zero fails before the first).
    AfterSteps(usize),
}

/// Event retained in the append-only host attempt log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttemptEvent {
    /// A step produced a temporary child snapshot that has not yet been committed to the trial.
    StepStaged {
        /// Candidate child identity.
        snapshot: Digest,
        /// Measured work added by this update.
        work: u64,
    },
    /// Candidate admission or evaluation refused.
    Refused(Refusal),
    /// The requested deterministic fault fired.
    FaultInjected {
        /// Number of staged steps before the fault.
        after_steps: usize,
    },
    /// All trial state was restored to the original checkpoint.
    RolledBack {
        /// Snapshot identity restored.
        snapshot: Digest,
    },
    /// Every requested step completed and the final state is returned.
    Completed {
        /// Final snapshot identity.
        snapshot: Digest,
    },
    /// No checkpoint was available; no candidate step was evaluated.
    CheckpointUnavailable,
}

/// One immutable append-only attempt record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptRecord {
    /// Monotonic log position.
    pub sequence: usize,
    /// Descriptor for a concrete step, when one was attempted.
    pub step: Option<StepDescriptor>,
    /// Result retained even if the trial state rolls back.
    pub event: AttemptEvent,
}

/// Append-only per-trial observations; it is deliberately outside `Checkpoint`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AttemptLog {
    records: Vec<AttemptRecord>,
}

impl AttemptLog {
    /// Immutable attempt history.
    #[must_use]
    pub fn records(&self) -> &[AttemptRecord] {
        &self.records
    }

    fn append(&mut self, step: Option<StepDescriptor>, event: AttemptEvent) {
        self.records.push(AttemptRecord {
            sequence: self.records.len(),
            step,
            event,
        });
    }
}

/// Why staged trial state was restored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrialFailure {
    /// A deterministic fault was injected.
    InjectedFault { after_steps: usize },
    /// Candidate input, result or budget refused.
    Refusal(Refusal),
}

/// Final disposition of a trial call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrialStatus {
    /// Requested step count completed.
    Completed,
    /// The complete mutable trial state was restored.
    RolledBack(TrialFailure),
}

/// Returned checkpoint and status; rollback returns the exact original checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrialResult {
    /// Updated checkpoint on success, or byte-stable original checkpoint on rollback.
    pub checkpoint: Checkpoint,
    /// Whether the trial completed or was restored.
    pub status: TrialStatus,
}

/// Runs a stateless optimizer candidate against a frozen ordered data set.
///
/// The full checkpoint (parameters/state, RNG cursor, data cursor and counters) is copied before
/// any step. Every refusal or injected failure discards staged state and appends its observations to
/// `attempts`; the log itself is never rolled back. An unavailable checkpoint refuses before any
/// candidate is evaluated.
pub fn run_trial(
    candidate: &Candidate,
    checkpoint: Option<&Checkpoint>,
    dataset: &FrozenDataset,
    steps: usize,
    fault: FaultInjection,
    attempts: &mut AttemptLog,
) -> Result<TrialResult, Refusal> {
    let Some(original) = checkpoint else {
        attempts.append(None, AttemptEvent::CheckpointUnavailable);
        return Err(Refusal::new(
            RefusalClass::Internal,
            "$.checkpoint",
            "checkpoint unavailable; trial refused before evaluation",
        ));
    };
    let origin = original.clone();
    if dataset.id() != origin.snapshot.context().data_order {
        return rollback(
            origin,
            attempts,
            None,
            Refusal::new(
                RefusalClass::Unsupported,
                "$.data_order",
                "frozen data identity differs from snapshot provenance",
            ),
        );
    }
    if let Err(refusal) = dataset.verify() {
        return rollback(origin, attempts, None, refusal);
    }
    if candidate.id() != origin.snapshot.provenance().optimizer_code {
        return rollback(
            origin,
            attempts,
            None,
            Refusal::new(
                RefusalClass::Unsupported,
                "$.optimizer_code",
                "candidate identity differs from snapshot provenance",
            ),
        );
    }
    if origin.rng.seed != origin.snapshot.context().seed {
        return rollback(
            origin,
            attempts,
            None,
            Refusal::new(
                RefusalClass::Unsupported,
                "$.checkpoint.rng.seed",
                "checkpoint seed differs from snapshot provenance",
            ),
        );
    }
    if dataset.is_empty() {
        return rollback(
            origin,
            attempts,
            None,
            Refusal::new(
                RefusalClass::Malformed,
                "$.data_order",
                "empty frozen data set",
            ),
        );
    }
    if let FaultInjection::AfterSteps(0) = fault {
        attempts.append(None, AttemptEvent::FaultInjected { after_steps: 0 });
        attempts.append(
            None,
            AttemptEvent::RolledBack {
                snapshot: origin.snapshot.id(),
            },
        );
        return Ok(TrialResult {
            checkpoint: origin.clone(),
            status: TrialStatus::RolledBack(TrialFailure::InjectedFault { after_steps: 0 }),
        });
    }

    let mut staged = origin.clone();
    let mut rng = splitmix_at(staged.rng.seed, staged.rng.draws);
    for step_offset in 0..steps {
        let step_index = match u32::try_from(staged.counters.updates) {
            Ok(index) => index,
            Err(_) => {
                return rollback(
                    origin,
                    attempts,
                    None,
                    Refusal::new(
                        RefusalClass::Exhausted,
                        "$.inputs.step.step_index",
                        staged.counters.updates.to_string(),
                    ),
                );
            }
        };
        let batch_index = rng.below(dataset.len_u64());
        let batch_index_u32 = match u32::try_from(batch_index) {
            Ok(index) => index,
            Err(_) => {
                return rollback(
                    origin,
                    attempts,
                    None,
                    Refusal::new(
                        RefusalClass::Exhausted,
                        "$.inputs.step.batch_index",
                        batch_index.to_string(),
                    ),
                );
            }
        };
        let descriptor = StepDescriptor {
            version: 1,
            step_index,
            batch_index: batch_index_u32,
        };
        let Some(row) = dataset.row(batch_index) else {
            return rollback(
                origin,
                attempts,
                Some(descriptor),
                Refusal::new(
                    RefusalClass::Internal,
                    "$.envelope",
                    "frozen data row disappeared",
                ),
            );
        };
        let parameters = staged.snapshot.parameters();
        let input = TransitionInput {
            parameters,
            gradients: &row.gradients,
            eta: row.eta,
            step: descriptor,
            state: staged.snapshot.state_bytes(),
        };
        let output = match candidate.evaluate(&input) {
            Ok(output) => output,
            Err(refusal) => return rollback(origin, attempts, Some(descriptor), refusal),
        };
        let child = match OptSnapshot::publish(candidate, &staged.snapshot, &output, descriptor) {
            Ok(snapshot) => snapshot,
            Err(refusal) => return rollback(origin, attempts, Some(descriptor), refusal),
        };
        let Some(updates) = staged.counters.updates.checked_add(1) else {
            return rollback(
                origin,
                attempts,
                Some(descriptor),
                Refusal::new(
                    RefusalClass::Exhausted,
                    "$.checkpoint.counters.updates",
                    "overflow",
                ),
            );
        };
        let Some(work) = staged.counters.work.checked_add(output.work) else {
            return rollback(
                origin,
                attempts,
                Some(descriptor),
                Refusal::new(
                    RefusalClass::Exhausted,
                    "$.checkpoint.counters.work",
                    "overflow",
                ),
            );
        };
        let Some(data_cursor) = staged.data_cursor.checked_add(1) else {
            return rollback(
                origin,
                attempts,
                Some(descriptor),
                Refusal::new(
                    RefusalClass::Exhausted,
                    "$.checkpoint.data_cursor",
                    "overflow",
                ),
            );
        };
        let Some(draws) = staged.rng.draws.checked_add(1) else {
            return rollback(
                origin,
                attempts,
                Some(descriptor),
                Refusal::new(
                    RefusalClass::Exhausted,
                    "$.checkpoint.rng.draws",
                    "overflow",
                ),
            );
        };
        staged.snapshot = child;
        staged.counters = Counters { updates, work };
        staged.data_cursor = data_cursor;
        staged.rng.draws = draws;
        attempts.append(
            Some(descriptor),
            AttemptEvent::StepStaged {
                snapshot: staged.snapshot.id(),
                work: output.work,
            },
        );
        if matches!(fault, FaultInjection::AfterSteps(count) if count == step_offset + 1) {
            let after_steps = step_offset + 1;
            attempts.append(None, AttemptEvent::FaultInjected { after_steps });
            attempts.append(
                None,
                AttemptEvent::RolledBack {
                    snapshot: origin.snapshot.id(),
                },
            );
            return Ok(TrialResult {
                checkpoint: origin,
                status: TrialStatus::RolledBack(TrialFailure::InjectedFault { after_steps }),
            });
        }
    }
    attempts.append(
        None,
        AttemptEvent::Completed {
            snapshot: staged.snapshot.id(),
        },
    );
    Ok(TrialResult {
        checkpoint: staged,
        status: TrialStatus::Completed,
    })
}

fn rollback(
    original: Checkpoint,
    attempts: &mut AttemptLog,
    step: Option<StepDescriptor>,
    refusal: Refusal,
) -> Result<TrialResult, Refusal> {
    attempts.append(step, AttemptEvent::Refused(refusal.clone()));
    attempts.append(
        None,
        AttemptEvent::RolledBack {
            snapshot: original.snapshot.id(),
        },
    );
    Ok(TrialResult {
        checkpoint: original,
        status: TrialStatus::RolledBack(TrialFailure::Refusal(refusal)),
    })
}

fn splitmix_at(seed: u64, draws: u64) -> SplitMix64 {
    const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
    SplitMix64::new(seed.wrapping_add(draws.wrapping_mul(GAMMA)))
}
