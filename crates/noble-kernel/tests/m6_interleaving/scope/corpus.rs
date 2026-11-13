const COMMITTED: &str =
    include_str!("../../../../../proofs/foundation/NobleFoundation/TaskScopeCorpus.lean");

#[path = "corpus/completion.rs"]
mod ordering;

#[path = "corpus/emission.rs"]
mod emission;

#[path = "corpus/cancel.rs"]
mod cancel;

pub(super) struct CompletionWitness {
    pub(super) model_trace: String,
    pub(super) out_of_order_refused: bool,
    pub(super) results: Vec<super::manager::ScopeResult>,
    pub(super) accounting: [noble_kernel::async_tasks::Obligations; 2],
    pub(super) initial: noble_kernel::async_tasks::Observation,
    pub(super) after_delivery: noble_kernel::async_tasks::Observation,
    pub(super) settled: noble_kernel::async_tasks::Observation,
}

pub(super) fn completion_trace(reverse: bool) -> Result<CompletionWitness, String> {
    ordering::completion_trace(reverse)
}

pub(super) struct RefusalWitness {
    pub(super) members: Vec<super::manager::MemberRef>,
    pub(super) error: noble_kernel::async_tasks::Error,
    pub(super) input_returned: bool,
    pub(super) before: noble_kernel::async_tasks::Observation,
    pub(super) after: noble_kernel::async_tasks::Observation,
}

pub(super) struct CancellationWitness {
    pub(super) model_trace: String,
    pub(super) results: Vec<super::manager::ScopeResult>,
    pub(super) primary_failure: Option<noble_kernel::async_tasks::Failure>,
    pub(super) cancelled_record: noble_kernel::async_tasks::Snapshot,
    pub(super) delivered: noble_kernel::async_tasks::Obligations,
    pub(super) after_late_completion: noble_kernel::async_tasks::Observation,
    pub(super) settled: noble_kernel::async_tasks::Observation,
}

pub(super) fn refusal_trace() -> Result<RefusalWitness, String> {
    let mut table = super::support::table(1)?;
    let registry = super::registry::ScopeRegistry::shared();
    let mut scope = super::manager::Scope::new(
        super::manager::ScopeId(115),
        noble_kernel::async_tasks::TableId(3),
        registry,
    );
    super::scope_support::admit(&mut scope, &mut table, vec![1, 2, 3])?;
    let before = table.observation();
    let inputs = vec![7, 8, 9];
    let (error, input_returned) =
        match scope.admit(&mut table, super::support::request(), inputs.clone()) {
            Err(super::manager::AdmissionError::Table(rejected)) => {
                (rejected.error, rejected.input == inputs)
            }
            Err(failure) => return Err(format!("wrong scope admission refusal: {failure:?}")),
            Ok(_) => return Err("second task admitted beyond table capacity".to_owned()),
        };
    let after = table.observation();
    assert_eq!(error, noble_kernel::async_tasks::Error::TaskCapacity);
    assert!(input_returned, "the caller retains rejected inputs");
    assert_eq!(before, after, "refusal preserves M6 accounting");
    assert_eq!(scope.members().len(), 1, "the admitted first child remains");
    Ok(RefusalWitness {
        members: scope.members(),
        error,
        input_returned,
        before,
        after,
    })
}

#[test]
fn the_lean_file_is_the_real_adapter_emission() -> Result<(), String> {
    let emitted = emission::lean_file()?;
    assert_eq!(
        emitted, COMMITTED,
        "adapter traces changed: regenerate TaskScopeCorpus.lean with `cargo test -p noble-kernel --test m6-interleaving -- --ignored write_task_scope_artifact` and rerun the foundation lane"
    );
    Ok(())
}

#[test]
#[ignore = "writes proofs/foundation/NobleFoundation/TaskScopeCorpus.lean; run after a deliberate adapter-trace change"]
fn write_task_scope_artifact() -> Result<(), String> {
    let emitted = emission::lean_file()?;
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proofs/foundation/NobleFoundation/TaskScopeCorpus.lean"
        ),
        emitted,
    )
    .map_err(|error| format!("could not write generated corpus: {error}"))?;
    Ok(())
}
