use super::*;
use alloc::vec::Vec;
use core::cmp::Ordering;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    pub exact_interface: Option<(Vec<Ty>, Vec<Ty>)>,
    /// Concrete full stacks immediately before and after the hole. The common
    /// bottom tail is solved exactly; no suffix-only signature guess is used.
    pub composition_context: Option<(Vec<Ty>, Vec<Ty>)>,
    /// Return only records whose transitive closure contains this identity.
    pub dependency_root: Option<ProcedureId>,
    pub status: Option<Status>,
}

#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeProducer {
    Proposer,
    Teacher,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueryBudget {
    queries: u32,
    work: u32,
    proposer_attempts: u32,
    teacher_attempts: u32,
}

impl QueryBudget {
    pub const fn new() -> Self {
        Self {
            queries: 0,
            work: 0,
            proposer_attempts: 0,
            teacher_attempts: 0,
        }
    }

    pub const fn queries(&self) -> u32 {
        self.queries
    }

    pub const fn work(&self) -> u32 {
        self.work
    }

    pub const fn proposer_attempts(&self) -> u32 {
        self.proposer_attempts
    }

    pub const fn teacher_attempts(&self) -> u32 {
        self.teacher_attempts
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryResult {
    /// Candidate identities only. These are not programs or authority.
    pub candidates: Vec<ProcedureId>,
    pub examined_entries: u32,
    pub index_bytes: u32,
}

pub(super) fn signature(record: &ProcedureRecord) -> RetrievalSignature {
    RetrievalSignature {
        stack_in: record.chunk.interface.stack_in.clone(),
        stack_out: record.chunk.interface.stack_out.clone(),
        effects: record.chunk.interface.effects.clone(),
        status_rank: record.status.rank(),
        cost_key: record.cost.key(),
    }
}

pub(super) fn compare_records(left: &ProcedureRecord, right: &ProcedureRecord) -> Ordering {
    compare_stack(
        &left.chunk.interface.stack_in,
        &right.chunk.interface.stack_in,
    )
    .then_with(|| {
        compare_stack(
            &left.chunk.interface.stack_out,
            &right.chunk.interface.stack_out,
        )
    })
    .then_with(|| {
        left.chunk
            .interface
            .effects
            .is_empty()
            .cmp(&right.chunk.interface.effects.is_empty())
    })
    .then_with(|| left.status.rank().cmp(&right.status.rank()))
    .then_with(|| left.cost.key().cmp(&right.cost.key()))
    .then_with(|| left.procedure_id.cmp(&right.procedure_id))
}

fn compare_stack(left: &[Ty], right: &[Ty]) -> Ordering {
    let mut at = 0usize;
    let mut order = Ordering::Equal;
    while order == Ordering::Equal && at < left.len() && at < right.len() {
        order = type_rank(&left[at]).cmp(&type_rank(&right[at]));
        at = at.saturating_add(1);
    }
    if order != Ordering::Equal {
        return order;
    }
    left.len().cmp(&right.len())
}

const fn type_rank(ty: &Ty) -> u8 {
    match ty {
        Ty::I64 => 0,
        Ty::Bool => 1,
        _ => 2,
    }
}

impl ProcedureLibrary {
    /// A retrieval miss must not invoke a host proposer or teacher. The
    /// caller's callback is held only to demonstrate that it is never reached;
    /// the attempt is counted separately from actual producer calls.
    pub fn request_runtime_producer(
        &self,
        producer: RuntimeProducer,
        budget: &mut QueryBudget,
        _invoke: impl FnOnce() -> ProcedureRecord,
    ) -> Result<(), Refusal> {
        let attempts = match producer {
            RuntimeProducer::Proposer => &mut budget.proposer_attempts,
            RuntimeProducer::Teacher => &mut budget.teacher_attempts,
        };
        *attempts = attempts.checked_add(1).ok_or_else(|| {
            Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.runtime_producer",
                    message: "runtime producer attempt counter overflow",
                },
            )
        })?;
        Err(Refusal::new(
            RefusalKind::Unsupported,
            super::Diagnostic {
                field: "retrieval.runtime_producer",
                message: "runtime proposer and teacher fallback is disabled",
            },
        ))
    }

    pub fn retrieve(
        &self,
        query: &Query,
        budget: &mut QueryBudget,
    ) -> Result<QueryResult, Refusal> {
        if budget.queries >= 32 {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.queries",
                    message: "retrieval query limit reached",
                },
            ));
        }
        budget.queries = budget.queries.checked_add(1).ok_or_else(|| {
            Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.queries",
                    message: "query counter overflow",
                },
            )
        })?;
        let index_bytes = self.records().len().checked_mul(32).ok_or_else(|| {
            Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.index",
                    message: "index size overflow",
                },
            )
        })?;
        if self.records().len() > MAX_RECORDS || index_bytes > MAX_INDEX_BYTES {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.index",
                    message: "canonical retrieval index exceeds its declared bound",
                },
            ));
        }
        let mut examined_entries = 0u32;
        let mut candidates = Vec::with_capacity(self.records().len());
        let dependency_matches = if let Some(root) = query.dependency_root {
            let (matches, closure_work) = dependent_closure(self.records(), root)?;
            let work = budget.work.checked_add(closure_work).ok_or_else(|| {
                Refusal::new(
                    RefusalKind::Exhausted,
                    super::Diagnostic {
                        field: "retrieval.work",
                        message: "work counter overflow",
                    },
                )
            })?;
            budget.work = work;
            if work > 4_096 {
                return Err(Refusal::new(
                    RefusalKind::Exhausted,
                    super::Diagnostic {
                        field: "retrieval.work",
                        message: "dependency closure work limit reached",
                    },
                ));
            }
            Some(matches)
        } else {
            None
        };
        let records = self.records();
        let mut failure = None;
        let mut index = 0;
        while failure.is_none() && index < records.len() {
            let record = &records[index];
            match Self::examine(
                query,
                budget,
                &mut examined_entries,
                dependency_matches.as_ref(),
                record,
            ) {
                Ok(true) => candidates.push(record.procedure_id),
                Ok(false) => {}
                Err(refusal) => failure = Some(refusal),
            }
            index += 1;
        }
        if let Some(refusal) = failure {
            return Err(refusal);
        }
        Ok(QueryResult {
            candidates,
            examined_entries,
            index_bytes: u32::try_from(index_bytes).map_err(|_| {
                Refusal::new(
                    RefusalKind::Exhausted,
                    super::Diagnostic {
                        field: "retrieval.index",
                        message: "index byte count exceeds the result wire domain",
                    },
                )
            })?,
        })
    }

    /// One retained record of [`Self::retrieve`]: meter it, then report
    /// whether it passes every filter of `query`.
    fn examine(
        query: &Query,
        budget: &mut QueryBudget,
        examined_entries: &mut u32,
        dependency_matches: Option<&Vec<ProcedureId>>,
        record: &ProcedureRecord,
    ) -> Result<bool, Refusal> {
        *examined_entries = examined_entries.checked_add(1).ok_or_else(|| {
            Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.examined",
                    message: "entry count overflow",
                },
            )
        })?;
        if *examined_entries > 128 {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.examined",
                    message: "candidate examination limit reached",
                },
            ));
        }
        let next_work = budget.work.checked_add(1).ok_or_else(|| {
            Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.work",
                    message: "work counter overflow",
                },
            )
        })?;
        budget.work = next_work;
        if next_work > 4_096 {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.work",
                    message: "retrieval work limit reached",
                },
            ));
        }
        if query.status.is_some_and(|status| status != record.status) {
            return Ok(false);
        }
        if query
            .exact_interface
            .as_ref()
            .is_some_and(|(input, output)| {
                input != &record.chunk.interface.stack_in
                    || output != &record.chunk.interface.stack_out
            })
        {
            return Ok(false);
        }
        if query
            .composition_context
            .as_ref()
            .is_some_and(|(before, after)| {
                !compatible_context(
                    &record.chunk.interface.stack_in,
                    &record.chunk.interface.stack_out,
                    before,
                    after,
                )
            })
        {
            return Ok(false);
        }
        if dependency_matches.is_some_and(|matches| !matches.contains(&record.procedure_id)) {
            return Ok(false);
        }
        Ok(true)
    }
}

fn compatible_context(stack_in: &[Ty], stack_out: &[Ty], before: &[Ty], after: &[Ty]) -> bool {
    let Some(input_tail) = before.len().checked_sub(stack_in.len()) else {
        return false;
    };
    let Some(output_tail) = after.len().checked_sub(stack_out.len()) else {
        return false;
    };
    input_tail == output_tail
        && before.get(input_tail..) == Some(stack_in)
        && after.get(output_tail..) == Some(stack_out)
        && before.get(..input_tail) == after.get(..output_tail)
}

fn dependent_closure(
    records: &[ProcedureRecord],
    root: ProcedureId,
) -> Result<(Vec<ProcedureId>, u32), Refusal> {
    if crate::scan::find_first(records, |record| record.procedure_id == root).is_none() {
        return Ok((Vec::new(), 0));
    }
    let bounded_record_count = records.len().min(MAX_RECORDS);
    let mut pending = Vec::with_capacity(bounded_record_count);
    let mut visited = Vec::with_capacity(bounded_record_count);
    pending.push(root);
    let mut work = 0u32;
    let mut failure = None;
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        if visited.len() >= MAX_RECORDS {
            failure = Some(Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.dependency_root",
                    message: "dependency closure exceeds its record bound",
                },
            ));
            break;
        }
        visited.push(current);
        debug_assert!(visited.len() <= MAX_RECORDS);
        if let Err(refusal) = push_dependents(records, current, &visited, &mut pending, &mut work) {
            failure = Some(refusal);
            break;
        }
    }
    if let Some(refusal) = failure {
        return Err(refusal);
    }
    Ok((visited, work))
}

/// Queue every record that directly depends on `current`, in record order.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; dependents are pushed onto the caller's growing worklist, which needs the Vec, not a fixed slice"
)]
fn push_dependents(
    records: &[ProcedureRecord],
    current: ProcedureId,
    visited: &[ProcedureId],
    pending: &mut Vec<ProcedureId>,
    work: &mut u32,
) -> Result<(), Refusal> {
    let mut outcome = Ok(());
    let mut index = 0;
    while outcome.is_ok() && index < records.len() {
        outcome = push_if_dependent(&records[index], current, visited, pending, work);
        index += 1;
    }
    outcome
}

/// One record of [`push_dependents`]: queue it if it depends on `current`.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; a dependent record is pushed onto the caller's growing worklist, which needs the Vec, not a fixed slice"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; work-counter overflow and the closure record bound are refused as typed `exhausted` refusals, and `dependent_closure` asserts the visited bound"
)]
fn push_if_dependent(
    record: &ProcedureRecord,
    current: ProcedureId,
    visited: &[ProcedureId],
    pending: &mut Vec<ProcedureId>,
    work: &mut u32,
) -> Result<(), Refusal> {
    *work = work.checked_add(1).ok_or_else(|| {
        Refusal::new(
            RefusalKind::Exhausted,
            super::Diagnostic {
                field: "retrieval.work",
                message: "dependency work overflow",
            },
        )
    })?;
    let is_dependent_on_current = depends_on(record, current, work)?;
    if is_dependent_on_current
        && !visited.contains(&record.procedure_id)
        && !pending.contains(&record.procedure_id)
    {
        if pending.len() >= MAX_RECORDS {
            return Err(Refusal::new(
                RefusalKind::Exhausted,
                super::Diagnostic {
                    field: "retrieval.dependency_root",
                    message: "dependency closure exceeds its record bound",
                },
            ));
        }
        pending.push(record.procedure_id);
    }
    Ok(())
}

/// Whether `record` names `current` as a library dependency, metering one
/// unit of `work` per dependency examined up to the first match.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the scan only meters and compares identities, and counter overflow is refused as a typed `exhausted` refusal"
)]
fn depends_on(
    record: &ProcedureRecord,
    current: ProcedureId,
    work: &mut u32,
) -> Result<bool, Refusal> {
    let mut is_dependent = false;
    let mut is_overflowed = false;
    let mut index = 0;
    while !is_dependent && !is_overflowed && index < record.library_dependencies.len() {
        match work.checked_add(1) {
            Some(next) => {
                *work = next;
                is_dependent = is_dependency_on(&record.library_dependencies[index], &current);
            }
            None => is_overflowed = true,
        }
        index += 1;
    }
    if is_overflowed {
        return Err(Refusal::new(
            RefusalKind::Exhausted,
            super::Diagnostic {
                field: "retrieval.work",
                message: "dependency work overflow",
            },
        ));
    }
    Ok(is_dependent)
}

/// Whether `dependency` names the procedure `current`.
fn is_dependency_on(dependency: &LibraryDependency, current: &ProcedureId) -> bool {
    dependency.procedure_id == *current
}
