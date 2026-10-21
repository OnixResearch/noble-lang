//! Selected pure Worker Program host. A WIT owner is a Wasmtime resource-table
//! entry pointing at a checked, *installed* source-Wasm Program cell. Neither
//! its component index nor the source-Wasm address is accepted as authority.

use anyhow::{bail, ensure, Context, Result};
use std::sync::atomic::{AtomicU64, Ordering};
use wasmtime::component::{Component, Linker, Resource, ResourceTable, ResourceType};
use wasmtime::{
    Caller, Config, Engine, ExternType, Global, Linker as CoreLinker, Memory, Module, Ref, Store,
    StoreLimits, StoreLimitsBuilder, Table, Val,
};

use super::worker_program::{Handle, Refusal, Store as ProgramStore};

const INTERFACE: &str = "noble-test:worker-program/programs@2.0.0";
pub(super) const FUEL: u64 = 10_000_000;
const EFFECT_REQUEST_LIMIT: u32 = 256;
const EFFECT_TEXT_LIMIT: usize = 65_536;
const LEDGER_BYTES_LIMIT: usize = 262_144;
const BATCH_ITERATION_LIMIT: u16 = 512;
static NEXT_INSTALLATION: AtomicU64 = AtomicU64::new(1);

/// Host-selected synthetic `test.emit` ledger. Selection cannot grant: the
/// exact installed Program cell must be issued after checked installation.
#[derive(Clone)]
pub(crate) struct WorkerHostPolicy {
    inner: std::sync::Arc<PolicyInner>,
}
struct PolicyInner {
    source_sha256: String,
    owner_identity: u64,
    source_generation: u64,
    destination: String,
    epoch: u64,
    binding: noble_contracts::source::BoundOperation,
    state: std::sync::Mutex<PolicyState>,
}
struct PolicyState {
    issued: Option<ProgramCell>,
    revoked: bool,
    current_epoch: u64,
    attempted: u32,
    protected: u32,
    denied: u32,
    ledger_bytes: usize,
    ledger: Vec<String>,
    #[cfg(test)]
    pre_commit: Option<(
        std::sync::Arc<std::sync::Barrier>,
        std::sync::Arc<std::sync::Barrier>,
    )>,
}
enum EffectOutcome {
    Denied,
    Committed,
}
impl WorkerHostPolicy {
    /// Bind a host-selected adapter to the exact independently checked named
    /// definition. Source bytes are an untrusted candidate, never a policy row.
    fn select_checked(
        source: &[u8],
        name: &str,
        destination: &str,
        epoch: u64,
        binding: noble_contracts::source::BoundOperation,
    ) -> Result<Self> {
        let retained =
            noble_contracts::source::RetainedSourceLimit::new(super::SOURCE_LIMITS.bytes)
                .context("invalid selected Worker retained source limit")?;
        let session =
            noble_contracts::source::ModuleSession::new(std::slice::from_ref(&binding), retained)
                .map_err(|_| anyhow::anyhow!("selected Worker binding refused"))?;
        let module = session
            .prepare(source, &[], super::SOURCE_LIMITS)
            .map_err(|_| anyhow::anyhow!("selected Worker candidate refused"))?;
        let owner = module
            .checked_program_owner_for_binding(name, &binding)
            .context("selected Worker definition absent")?;
        let (session, committed) = session.commit(module);
        committed.map_err(|_| anyhow::anyhow!("selected Worker commit refused"))?;
        Self::select(
            crate::workflow::intrinsic::sha256(source),
            owner.definition_identity(),
            session.generation(),
            destination.to_owned(),
            epoch,
            binding,
        )
    }

    pub(crate) fn select(
        source_sha256: String,
        owner_identity: u64,
        source_generation: u64,
        destination: String,
        epoch: u64,
        binding: noble_contracts::source::BoundOperation,
    ) -> Result<Self> {
        ensure!(
            source_sha256.len() == 64
                && source_sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
                && owner_identity != 0
                && source_generation != 0
                && !destination.is_empty()
                && destination.len() <= 128
                && !destination.as_bytes().contains(&0)
                && epoch != 0,
            "invalid selected Worker host subject"
        );
        ensure!(
            binding.module_version != 0
                && binding.adapter_slot != 0
                && binding.output.is_empty()
                && ((binding.operation == "test.emit"
                    && binding.adapter_identity == "worker-test-emit@1.0.0"
                    && binding.input == [noble_kernel::types::Ty::Text]
                    && binding.effects == [noble_kernel::contracts::TEST_EMIT])
                    || (binding.operation == "test.code-export"
                        && binding.adapter_identity
                            == noble_kernel::contracts::TEST_CODE_EXPORT_ADAPTER_IDENTITY
                        && binding.input
                            == [noble_kernel::types::Ty::Text, noble_kernel::types::Ty::Text]
                        && binding.effects == [noble_kernel::contracts::TEST_CODE_EXPORT])),
            "unsupported selected Worker bound adapter"
        );
        Ok(Self {
            inner: std::sync::Arc::new(PolicyInner {
                source_sha256,
                owner_identity,
                source_generation,
                destination,
                epoch,
                binding,
                state: std::sync::Mutex::new(PolicyState {
                    issued: None,
                    revoked: false,
                    current_epoch: epoch,
                    attempted: 0,
                    protected: 0,
                    denied: 0,
                    ledger_bytes: 0,
                    ledger: Vec::new(),
                    #[cfg(test)]
                    pre_commit: None,
                }),
            }),
        })
    }

    pub(crate) fn binding(&self) -> &noble_contracts::source::BoundOperation {
        &self.inner.binding
    }

    pub(crate) fn matches_source(
        &self,
        source: &[u8],
        owner_identity: u64,
        source_generation: u64,
    ) -> bool {
        self.inner.source_sha256 == crate::workflow::intrinsic::sha256(source)
            && self.inner.owner_identity == owner_identity
            && self.inner.source_generation == source_generation
    }

    fn issue_installed(
        &self,
        source: &ProgramStore,
        root: Handle,
        cell: ProgramCell,
        task: &TaskOwner,
    ) -> Result<()> {
        ensure!(
            source
                .host_policy()
                .is_some_and(|selected| std::sync::Arc::ptr_eq(&selected.inner, &self.inner))
                && self.inner.binding.operation == "test.emit"
                && source.node_index(root).is_some()
                && source.root_owner_identity() == self.inner.owner_identity
                && cell.scope == source.instance_id()
                && cell.address != 0
                && cell.installation != 0
                && cell.definition_identity == self.inner.owner_identity,
            "Worker issuer subject differs from checked installed Program"
        );
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Worker policy poisoned"))?;
        ensure!(
            task.running()?
                && !state.revoked
                && state.current_epoch == self.inner.epoch
                && state.issued.is_none(),
            "Worker host issue is stale or revoked"
        );
        state.issued = Some(cell);
        Ok(())
    }

    fn continue_issued(&self, cell: ProgramCell, task: &TaskOwner) -> Result<()> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Worker policy poisoned"))?;
        ensure!(
            state.issued == Some(cell)
                && !state.revoked
                && state.current_epoch == self.inner.epoch
                && task.running()?,
            "Worker batch grant or task retired between iterations"
        );
        Ok(())
    }

    pub(crate) fn revoke(&self) -> Result<()> {
        self.inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Worker policy poisoned"))?
            .revoked = true;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn advance_epoch(&self, next: u64) -> Result<()> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Worker policy poisoned"))?;
        ensure!(next > state.current_epoch, "Worker host epoch must advance");
        state.current_epoch = next;
        Ok(())
    }

    fn emit(
        &self,
        cell: ProgramCell,
        slot: u32,
        text: &str,
        task: &TaskOwner,
    ) -> Result<EffectOutcome> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Worker policy poisoned"))?;
        state.attempted = state
            .attempted
            .checked_add(1)
            .context("Worker effect request count overflow")?;
        let next_bytes = state.ledger_bytes.checked_add(text.len());
        if state.attempted > EFFECT_REQUEST_LIMIT
            || state.issued != Some(cell)
            || state.revoked
            || state.current_epoch != self.inner.epoch
            || !task.running().unwrap_or(false)
            || slot != self.inner.binding.adapter_slot
            || text.len() > EFFECT_TEXT_LIMIT
            || next_bytes.is_none_or(|bytes| bytes > LEDGER_BYTES_LIMIT)
        {
            state.denied = state
                .denied
                .checked_add(1)
                .context("Worker effect denial count overflow")?;
            return Ok(EffectOutcome::Denied);
        }
        let completed = state
            .protected
            .checked_add(1)
            .context("Worker protected completion count overflow")?;
        #[cfg(test)]
        if let Some((entered, release)) = &state.pre_commit {
            entered.wait();
            release.wait();
        }
        state.ledger.push(text.to_owned());
        state.ledger_bytes = next_bytes.context("Worker ledger quota overflow")?;
        state.protected = completed;
        Ok(EffectOutcome::Committed)
    }

    fn deny_malformed_request(&self) -> Result<()> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Worker policy poisoned"))?;
        state.attempted = state
            .attempted
            .checked_add(1)
            .context("Worker malformed request count overflow")?;
        state.denied = state
            .denied
            .checked_add(1)
            .context("Worker malformed denial count overflow")?;
        Ok(())
    }

    #[cfg(test)]
    fn counts(&self) -> Result<(u32, u32)> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Worker policy poisoned"))?;
        Ok((state.attempted, state.protected))
    }

    pub(crate) fn receipt(&self) -> Result<serde_json::Value> {
        ensure!(
            self.inner.binding.operation == "test.emit",
            "only the synthetic ledger publishes an effect receipt"
        );
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Worker policy poisoned"))?;
        Ok(serde_json::json!({
            "schema":"noble-worker-host-effect/v1",
            "effect":"test.emit","policy_epoch":self.inner.epoch,
            "source_sha256":self.inner.source_sha256,
            "definition_identity":self.inner.owner_identity.to_string(),
            "source_generation":self.inner.source_generation,
            "destination":self.inner.destination,
            "adapter_identity":self.inner.binding.adapter_identity,
            "adapter_slot":self.inner.binding.adapter_slot,
            "effect_requests":state.attempted,"protected_operations":state.protected,
            "denied_requests":state.denied,"succeeded":state.protected,
            "unknown_after_attempt":0,
            "ledger_entries":state.ledger.len(),"ledger_bytes":state.ledger_bytes,
        }))
    }

    #[cfg(test)]
    fn ledger(&self) -> Result<Vec<(String, String)>> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Worker policy poisoned"))?;
        Ok(state
            .ledger
            .iter()
            .map(|text| (self.inner.destination.clone(), text.clone()))
            .collect())
    }
}

/// Explicit host capability for the finite synthetic ledger. Construction
/// checks the independently selected source binding; it does not issue it.
/// Only `invoke` can issue, after the actual checked cell is installed.
pub(crate) struct HostWorkerIssuer {
    policy: WorkerHostPolicy,
}
impl HostWorkerIssuer {
    pub(crate) fn select_checked(
        source: &[u8],
        name: &str,
        destination: &str,
        epoch: u64,
        binding: noble_contracts::source::BoundOperation,
    ) -> Result<Self> {
        ensure!(
            binding.operation == "test.emit",
            "Worker host issuer is restricted to the synthetic emit ledger"
        );
        Ok(Self {
            policy: WorkerHostPolicy::select_checked(source, name, destination, epoch, binding)?,
        })
    }

    pub(crate) fn prepare(
        &self,
        source: &[u8],
        name: &str,
        work: usize,
    ) -> std::result::Result<(ProgramStore, Handle), super::worker_program::Refusal> {
        ProgramStore::prepare_with_policy(
            source,
            name,
            &[noble_kernel::types::Ty::I64],
            self.policy.clone(),
            work,
        )
    }

    pub(crate) fn invoke(
        &self,
        source: &mut ProgramStore,
        root: Handle,
        operand: i64,
        iterations: i64,
    ) -> std::result::Result<Observation, GuestFailure> {
        execute_with_host_grant(source, root, operand, iterations, &self.policy)
    }

    pub(crate) fn revoke(&self) -> Result<()> {
        self.policy.revoke()
    }
    pub(crate) fn receipt(&self) -> Result<serde_json::Value> {
        self.policy.receipt()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ready,
    Running,
    Completed,
    Cancelled,
    Delivered,
}
struct TaskState {
    phase: Phase,
    result: Option<i64>,
    engine: Option<Engine>,
}
#[derive(Clone)]
pub(super) struct TaskOwner {
    scope: u64,
    state: std::sync::Arc<std::sync::Mutex<TaskState>>,
    policy: Option<WorkerHostPolicy>,
    #[cfg(test)]
    between_steps: Option<StepBarrier>,
}

/// Test-only handshake that pauses a task between bounded guest steps.
#[cfg(test)]
type StepBarrier = (
    std::sync::mpsc::Sender<()>,
    std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Receiver<()>>>,
);
impl TaskOwner {
    pub(super) fn new(scope: u64) -> Self {
        Self {
            scope,
            state: std::sync::Arc::new(std::sync::Mutex::new(TaskState {
                phase: Phase::Ready,
                result: None,
                engine: None,
            })),
            policy: None,
            #[cfg(test)]
            between_steps: None,
        }
    }
    fn for_source(source: &ProgramStore) -> Self {
        let mut task = Self::new(source.instance_id());
        task.policy = source.host_policy().cloned();
        task
    }
    fn start(&self, engine: &Engine) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("task owner poisoned"))?;
        ensure!(
            state.phase == Phase::Ready,
            "retired or reused Program task"
        );
        state.phase = Phase::Running;
        state.engine = Some(engine.clone());
        Ok(())
    }
    fn cancelled(&self) -> Result<bool> {
        Ok(self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("task owner poisoned"))?
            .phase
            == Phase::Cancelled)
    }
    fn running(&self) -> Result<bool> {
        Ok(self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("task owner poisoned"))?
            .phase
            == Phase::Running)
    }
    fn complete(&self, result: i64) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("task owner poisoned"))?;
        ensure!(
            state.phase == Phase::Running,
            "late Program task completion retired"
        );
        state.phase = Phase::Completed;
        state.engine = None;
        state.result = Some(result);
        Ok(())
    }
    pub(super) fn recall(&self, scope: u64) -> Result<Option<i64>> {
        ensure!(scope == self.scope, "foreign Program task recall");
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("task owner poisoned"))?;
        if state.phase != Phase::Completed {
            return Ok(None);
        }
        state.phase = Phase::Delivered;
        Ok(state.result.take())
    }
    pub(super) fn cancel(&self, scope: u64) -> Result<bool> {
        ensure!(scope == self.scope, "foreign Program task cancellation");
        // Same lock order as emit: policy before task. The acknowledgement
        // cannot overtake an admitted ledger append still holding policy.
        let _policy = self
            .policy
            .as_ref()
            .map(|policy| {
                policy
                    .inner
                    .state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Worker policy poisoned"))
            })
            .transpose()?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("task owner poisoned"))?;
        match state.phase {
            Phase::Ready | Phase::Running | Phase::Completed => {
                if let Some(engine) = state.engine.take() {
                    engine.increment_epoch();
                }
                state.phase = Phase::Cancelled;
                state.result = None;
                Ok(true)
            }
            Phase::Cancelled | Phase::Delivered => Ok(false),
        }
    }
    fn retire(&self) {
        if let Ok(mut state) = self.state.lock() {
            if matches!(
                state.phase,
                Phase::Ready | Phase::Running | Phase::Completed
            ) {
                state.phase = Phase::Cancelled;
                state.engine = None;
                state.result = None;
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ProgramCell {
    scope: u64,
    address: u32,
    definition_identity: u64,
    capture_hash: [u8; 32],
    // An address/source can recur in a fresh Wasmtime Store. This token cannot.
    installation: u64,
}
/// Created before starting a task or installing a cell. The exact issue is
/// recorded here before publication, so even an unwind during issuance
/// retires it. Policy and task locks are never held across guest execution.
struct ExecutionScope<'a> {
    task: &'a TaskOwner,
    policy: Option<&'a WorkerHostPolicy>,
    issued: Option<ProgramCell>,
}
impl<'a> ExecutionScope<'a> {
    fn new(task: &'a TaskOwner, policy: Option<&'a WorkerHostPolicy>) -> Self {
        Self {
            task,
            policy,
            issued: None,
        }
    }
    fn issue(&mut self, source: &ProgramStore, root: Handle, cell: ProgramCell) -> Result<()> {
        ensure!(
            self.issued.is_none(),
            "Worker invocation already attempted an issue"
        );
        let policy = self.policy.context("no selected Worker host grant")?;
        self.issued = Some(cell);
        policy.issue_installed(source, root, cell, self.task)
    }
}
impl Drop for ExecutionScope<'_> {
    fn drop(&mut self) {
        if let (Some(policy), Some(cell)) = (self.policy, self.issued) {
            if let Ok(mut state) = policy.inner.state.lock() {
                if state.issued == Some(cell) {
                    state.issued = None;
                }
            }
        }
        self.task.retire();
    }
}
#[derive(Clone, Copy)]
struct Installed {
    instance: wasmtime::Instance,
    cell: ProgramCell,
}
struct Host {
    table: ResourceTable,
    installed: Option<Installed>,
    export_capture: Option<ProgramCell>,
    policy: Option<WorkerHostPolicy>,
    scope: u64,
    task: TaskOwner,
    limits: StoreLimits,
    admits: u32,
    invokes: u32,
    live_owners: u32,
    effect_requests: u32,
    protected_operations: u32,
    code_export_requests: u32,
    code_export_denials: u32,
    capture_published: bool,
}
#[expect(
    tigerstyle::type_accumulation,
    reason = "Owner: noble-maintainers; Host is the single wasmtime store state for the worker guest; its generated WIT host traits and this ledger impl must all target the same store type."
)]
impl Host {
    fn release(&mut self, owner: Resource<ProgramCell>) -> Result<ProgramCell> {
        let cell = self.table.delete(owner)?;
        self.live_owners = self
            .live_owners
            .checked_sub(1)
            .context("Program owner count underflow")?;
        Ok(cell)
    }
}

fn core_module(engine: &Engine, source_wat: &[u8]) -> Result<Module> {
    let bytes = wat::parse_bytes(source_wat).context("checked source WAT parse")?;
    Module::new(engine, bytes.as_ref()).context("checked source Wasm validation")
}

/// Source calls the checked bound adapter with byte offset, length, then slot.
/// The import only exists in a host-selected bound Store. Denial returns a
/// nonzero status, which the source runtime turns into a failed invocation.
fn bound_emit(mut caller: Caller<'_, Host>, offset: i32, length: i32, slot: i32) -> i32 {
    let request = (|| -> Result<EffectOutcome> {
        let host = caller.data_mut();
        host.effect_requests = host
            .effect_requests
            .checked_add(1)
            .context("Worker effect request count overflow")?;
        let installed = host.installed.context("uninstalled bound Program")?;
        let policy = host.policy.clone().context("unissued bound host policy")?;
        ensure!(
            installed.cell.scope == host.scope
                && installed.cell.definition_identity == policy.inner.owner_identity
                && policy.inner.source_generation != 0
                && !policy.inner.destination.is_empty(),
            "bound host policy differs from installed Program owner"
        );
        let task = host.task.clone();
        let start = usize::try_from(offset)?;
        let end = start
            .checked_add(usize::try_from(length)?)
            .context("Worker effect Text bound overflow")?;
        ensure!(
            end - start <= EFFECT_TEXT_LIMIT,
            "Worker effect Text exceeds bound"
        );
        let memory = caller
            .get_export("memory")
            .and_then(wasmtime::Extern::into_memory)
            .context("checked source memory missing")?;
        let text = std::str::from_utf8(
            memory
                .data(&caller)
                .get(start..end)
                .context("Worker effect Text out of memory bounds")?,
        )
        .context("Worker effect Text is not UTF-8")?;
        policy.emit(installed.cell, u32::try_from(slot)?, text, &task)
    })();
    match request {
        Ok(EffectOutcome::Committed) => {
            let host = caller.data_mut();
            let Some(next) = host.protected_operations.checked_add(1) else {
                return 1;
            };
            host.protected_operations = next;
            0
        }
        Ok(EffectOutcome::Denied) => 1,
        Err(_) => {
            if let Some(policy) = &caller.data().policy {
                let _ = policy.deny_malformed_request();
            }
            1
        }
    }
}

/// There is no authorized source-side code exporter. Even a valid checked
/// adapter receives a typed denial before reading either Text argument.
fn denied_code_export_bound(mut caller: Caller<'_, Host>, _slot: i32) -> i32 {
    let host = caller.data_mut();
    let (Some(requests), Some(denials)) = (
        host.code_export_requests.checked_add(1),
        host.code_export_denials.checked_add(1),
    ) else {
        return 1;
    };
    host.code_export_requests = requests;
    host.code_export_denials = denials;
    1
}

/// Fresh memory/table/globals are kept in this one Store for this one source
/// compiler artifact. A forged extra import cannot be satisfied or guessed.
fn install_core(store: &mut Store<Host>, module: &Module) -> Result<wasmtime::Instance> {
    let mut linker = CoreLinker::new(store.engine());
    for import in module.imports() {
        ensure!(import.module() == "noble", "foreign source runtime import");
        let name = import.name();
        match import.ty() {
            ExternType::Memory(ty) if name == "memory" => {
                ensure!(
                    ty.minimum() == 16 && ty.maximum() == Some(16),
                    "source runtime memory profile mismatch"
                );
                let memory = Memory::new(&mut *store, ty)?;
                linker.define(&*store, "noble", name, memory)?;
            }
            ExternType::Table(ty) if name == "table" => {
                ensure!(
                    ty.minimum() == 16_384 && ty.maximum() == Some(16_384),
                    "source runtime table profile mismatch"
                );
                let table = Table::new(&mut *store, ty, Ref::Func(None))?;
                linker.define(&*store, "noble", name, table)?;
            }
            ExternType::Global(ty) if name != "memory" && name != "table" => {
                let value = match ty.content() {
                    wasmtime::ValType::I32 => Val::I32(0),
                    wasmtime::ValType::I64 => Val::I64(0),
                    _ => bail!("unsupported source runtime global"),
                };
                let global = Global::new(&mut *store, ty, value)?;
                linker.define(&*store, "noble", name, global)?;
            }
            ExternType::Func(_)
                if name == "test_emit_bound"
                    && store
                        .data()
                        .policy
                        .as_ref()
                        .is_some_and(|policy| policy.binding().operation == "test.emit") =>
            {
                linker.func_wrap("noble", name, bound_emit)?;
            }
            ExternType::Func(_)
                if name == "test_code_export_bound"
                    && store
                        .data()
                        .policy
                        .as_ref()
                        .is_some_and(|policy| policy.binding().operation == "test.code-export") =>
            {
                linker.func_wrap("noble", name, denied_code_export_bound)?;
            }
            _ => bail!("unapproved Worker source Wasm import {name}"),
        }
    }
    linker.instantiate(&mut *store, module)
}

fn cell_field(
    store: &mut Store<Host>,
    instance: wasmtime::Instance,
    field: &str,
    address: u32,
) -> Result<u32> {
    let accessor = instance.get_typed_func::<i32, i32>(&mut *store, field)?;
    let value = accessor
        .call(&mut *store, i32::try_from(address)?)
        .with_context(|| format!("source cell {field} read"))?;
    Ok(u32::try_from(value)?)
}

/// Compare every installed c-recipe atom against the *same checked layout*
/// that emitted it. Text bytes alone cannot distinguish lexical aliases:
/// the matching atom carries the checked source node before its runtime
/// address, length and bytes are accepted.
#[expect(
    tigerstyle::allocation_in_loop,
    reason = "Owner: noble-maintainers; each installed Program recipe is checked with a fresh worklist seeded by its recipe address; the walk is bounded by the checked layout's atom count."
)]
fn text_alias_hash(
    store: &mut Store<Host>,
    instance: wasmtime::Instance,
    source: &ProgramStore,
) -> Result<[u8; 32]> {
    let mut hash = blake3::Hasher::new();
    let mut total = 0usize;
    for node in 0..source.recipe_nodes() {
        let Some(handle) = source.checked_root(node) else {
            continue;
        };
        let named = source
            .named(handle)
            .context("checked named Program missing")?;
        let owner_identity = source
            .source_owner_identity(handle)
            .context("checked source owner missing")?;
        let index = source
            .compiled()
            .named_targets()
            .iter()
            .position(|item| {
                item.program_index == named.program_index
                    && item.definition_identity == named.definition_identity
            })
            .context("ambiguous source definition")?;
        let handle = instance
            .get_typed_func::<i32, i32>(&mut *store, "worker_named_handle")?
            .call(&mut *store, i32::try_from(index)?)?;
        ensure!(handle > 0, "missing installed named Program for Text alias");
        let root = u32::try_from(handle)?;
        let mut programs = vec![(named.program_index, root)];
        let mut seen_programs = std::collections::HashSet::new();
        let aliases = source.text_aliases();
        let start = aliases
            .iter()
            .position(|alias| alias.owner_identity == owner_identity)
            .unwrap_or(aliases.len());
        let count = aliases[start..]
            .iter()
            .take_while(|alias| alias.owner_identity == owner_identity)
            .count();
        let expected = &aliases[start..start + count];
        ensure!(
            aliases[start + count..]
                .iter()
                .all(|alias| alias.owner_identity != owner_identity),
            "checked Text aliases are not grouped by source owner"
        );
        let mut matched = vec![false; expected.len()];
        while let Some((program_index, program_address)) = programs.pop() {
            ensure!(
                seen_programs.insert(program_index) && seen_programs.len() <= 4096,
                "installed quotation Program graph repeats or exceeds bound"
            );
            let metadata = source
                .compiled()
                .program_metadata()
                .get(program_index)
                .context("checked source recipe metadata absent")?;
            let installed = instance
                .get_typed_func::<i32, i32>(&mut *store, "worker_program_handle")?
                .call(&mut *store, i32::try_from(program_index)?)?;
            ensure!(
                installed > 0 && program_address == u32::try_from(installed)?,
                "installed quotation Program does not match checked global"
            );
            let payload = instance
                .get_typed_func::<i32, i64>(&mut *store, "cell_payload")?
                .call(&mut *store, i32::try_from(program_address)?)?;
            ensure!(
                cell_field(store, instance, "cell_kind", program_address)? == 4
                    && payload == i64::from(metadata.effect_mask)
                    && cell_field(store, instance, "cell_x", program_address)? == metadata.entry
                    && cell_field(store, instance, "cell_y", program_address)?
                        == metadata.input_signature
                    && cell_field(store, instance, "cell_z", program_address)?
                        == metadata.output_signature
                    && cell_field(store, instance, "cell_a", program_address)?
                        == metadata.capture_left
                    && cell_field(store, instance, "cell_b", program_address)?
                        == metadata.capture_right
                    && cell_field(store, instance, "cell_w", program_address)?
                        == metadata.recipe_depth
                    && cell_field(store, instance, "cell_n", program_address)?
                        == metadata.recipe_leaves,
                "installed quotation Program cell differs from checked source"
            );
            let recipe = cell_field(store, instance, "cell_c", program_address)?;
            ensure!(
                recipe != 0 || metadata.runtime_atoms.is_empty(),
                "installed Program recipe absent"
            );
            let mut pending = if recipe == 0 {
                Vec::new()
            } else {
                vec![recipe]
            };
            let mut visited = std::collections::HashSet::new();
            let mut at = 0usize;
            while let Some(address) = pending.pop() {
                ensure!(
                    address != 0 && visited.insert(address),
                    "installed Program recipe has a null or repeated edge"
                );
                ensure!(
                    visited.len() <= 4096 && pending.len() <= 4096,
                    "installed Text/recipe graph exceeds bound"
                );
                match cell_field(store, instance, "cell_kind", address)? {
                    9 => {
                        let left = cell_field(store, instance, "cell_a", address)?;
                        let right = cell_field(store, instance, "cell_b", address)?;
                        ensure!(
                            left != 0
                                && right != 0
                                && cell_field(store, instance, "cell_c", address)? == 0,
                            "installed concat recipe edges differ from source layout"
                        );
                        pending.push(right);
                        pending.push(left);
                    }
                    8 => {
                        let atom = metadata
                            .runtime_atoms
                            .get(at)
                            .context("installed Program has extra recipe atom")?;
                        at += 1;
                        let payload = instance
                            .get_typed_func::<i32, i64>(&mut *store, "cell_payload")?
                            .call(&mut *store, i32::try_from(address)?)?;
                        ensure!(
                            payload == atom.payload
                                && cell_field(store, instance, "cell_x", address)? == atom.tag
                                && cell_field(store, instance, "cell_y", address)? == atom.input
                                && cell_field(store, instance, "cell_z", address)? == atom.output
                                && cell_field(store, instance, "cell_w", address)? == atom.effects
                                && cell_field(store, instance, "cell_n", address)? == atom.ordinal
                                && cell_field(store, instance, "cell_b", address)? == 0
                                && cell_field(store, instance, "cell_c", address)? == 0,
                            "installed recipe atom differs from checked source node"
                        );
                        let child = cell_field(store, instance, "cell_a", address)?;
                        match (atom.text, atom.child_program) {
                            (Some((start, length)), None) => {
                                ensure!(
                                    child != 0
                                        && cell_field(store, instance, "cell_kind", child)? == 11
                                        && cell_field(store, instance, "cell_x", child)? == start
                                        && cell_field(store, instance, "cell_y", child)? == length,
                                    "installed Text atom differs from checked literal span"
                                );
                                total = total
                                    .checked_add(usize::try_from(length)?)
                                    .context("Text capture size overflow")?;
                                ensure!(total <= 65_536, "Text capture bytes exceed source limit");
                                let end = start
                                    .checked_add(length)
                                    .context("Text capture address overflow")?;
                                ensure!(
                                    end <= source
                                        .compiled()
                                        .worker_literal_end()
                                        .context("not a Worker source literal")?,
                                    "Text capture points outside checked literal arena"
                                );
                                let memory = instance
                                    .get_memory(&mut *store, "memory")
                                    .context("source memory export absent")?;
                                let bytes = memory
                                    .data(&*store)
                                    .get(usize::try_from(start)?..usize::try_from(end)?)
                                    .context("Text capture memory out of bounds")?;
                                let index = expected
                                    .iter()
                                    .enumerate()
                                    .position(|(index, alias)| {
                                        !matched[index]
                                            && alias.node == atom.source_node
                                            && alias.bytes == bytes
                                    })
                                    .context(
                                        "lexically bound Text differs from installed literal",
                                    )?;
                                matched[index] = true;
                            }
                            (None, Some(program)) => {
                                let expected_child = instance
                                    .get_typed_func::<i32, i32>(
                                        &mut *store,
                                        "worker_program_handle",
                                    )?
                                    .call(&mut *store, i32::try_from(program)?)?;
                                ensure!(
                                    expected_child > 0 && child == u32::try_from(expected_child)?,
                                    "installed quotation Program child differs from checked source"
                                );
                                programs.push((program, child));
                            }
                            (None, None) => ensure!(
                                child == 0,
                                "installed Program atom contains an unapproved child"
                            ),
                            (Some(_), Some(_)) => bail!("ambiguous checked recipe atom"),
                        }
                    }
                    _ => bail!("installed Program recipe contains an unapproved cell"),
                }
            }
            ensure!(
                at == metadata.runtime_atoms.len(),
                "installed Program omits checked recipe atoms"
            );
        }
        ensure!(
            matched.iter().all(|matched| *matched),
            "checked Text alias is absent from installed recipe"
        );
        for alias in expected {
            hash.update(&alias.owner_identity.to_le_bytes());
            hash.update(&alias.node.to_le_bytes());
            hash.update(&(alias.bytes.len() as u64).to_le_bytes());
            hash.update(&alias.bytes);
        }
    }
    Ok(*hash.finalize().as_bytes())
}

fn bind_named(
    store: &mut Store<Host>,
    instance: wasmtime::Instance,
    source: &ProgramStore,
    root: Handle,
    capture_hash: [u8; 32],
) -> Result<ProgramCell> {
    let named = source.named(root).context("missing checked source owner")?;
    let owner_identity = source
        .source_owner_identity(root)
        .context("missing checked source identity")?;
    let ceiling = match source.host_policy() {
        Some(policy) => noble_kernel::types::EffSet::from_ids(&policy.binding().effects),
        None => noble_kernel::types::EffSet::empty(),
    };
    ensure!(
        named.effects.is_subset_of(&ceiling),
        "named Program effect exceeds selected host contract"
    );
    let index = source
        .compiled()
        .named_targets()
        .iter()
        .position(|candidate| {
            candidate.program_index == named.program_index
                && candidate.definition_identity == named.definition_identity
        })
        .context("selected Program is not in checked module metadata")?;
    let count = instance
        .get_typed_func::<(), i32>(&mut *store, "worker_named_count")?
        .call(&mut *store, ())?;
    ensure!(
        usize::try_from(count)? == source.compiled().named_targets().len(),
        "installed named Program count differs from checked source"
    );
    let address = instance
        .get_typed_func::<i32, i32>(&mut *store, "worker_named_handle")?
        .call(&mut *store, i32::try_from(index)?)?;
    ensure!(address > 0, "selected named Program cell is not installed");
    let address = u32::try_from(address)?;
    let metadata = source
        .compiled()
        .program_metadata()
        .get(named.program_index)
        .context("selected source Program metadata absent")?;
    let payload = instance
        .get_typed_func::<i32, i64>(&mut *store, "cell_payload")?
        .call(&mut *store, i32::try_from(address)?)?;
    ensure!(
        cell_field(store, instance, "cell_kind", address)? == 4
            && payload == i64::from(named.effect_mask)
            && metadata.effect_mask == named.effect_mask
            && cell_field(store, instance, "cell_x", address)? == named.entry
            && cell_field(store, instance, "cell_y", address)? == named.input_signature
            && cell_field(store, instance, "cell_z", address)? == named.output_signature
            && cell_field(store, instance, "cell_a", address)? == metadata.capture_left
            && cell_field(store, instance, "cell_b", address)? == metadata.capture_right
            && cell_field(store, instance, "cell_w", address)? == metadata.recipe_depth
            && cell_field(store, instance, "cell_n", address)? == metadata.recipe_leaves,
        "installed source Program cell differs from independently checked metadata"
    );
    ensure!(
        named.checked_recipe.len() <= 65_536,
        "source Program recipe exceeds bound"
    );
    Ok(ProgramCell {
        scope: source.instance_id(),
        address,
        definition_identity: owner_identity,
        capture_hash,
        installation: 0,
    })
}

fn bind_installed(
    store: &mut Store<Host>,
    instance: wasmtime::Instance,
    source: &ProgramStore,
    root: Handle,
) -> Result<ProgramCell> {
    let index = source.node_index(root).ok_or(ForeignOrStaleProgramNode)?;
    let (input, output, effects) = source
        .interface(root)
        .context("missing selected Program witness")?;
    let selected_effects = match source.host_policy() {
        Some(policy) => noble_kernel::types::EffSet::from_ids(&policy.binding().effects),
        None => noble_kernel::types::EffSet::empty(),
    };
    ensure!(
        input == [noble_kernel::types::Ty::I64]
            && output == [noble_kernel::types::Ty::I64]
            && effects == &selected_effects,
        "guest requires the exact host-selected I64 Program witness"
    );
    let capture_hash = text_alias_hash(store, instance, source)?;
    let mut cells = Vec::with_capacity(source.recipe_nodes());
    for node in 0..source.recipe_nodes() {
        let handle = source
            .node_handle(node)
            .context("missing checked Program recipe node")?;
        if source.named(handle).is_some() {
            cells.push(bind_named(store, instance, source, handle, capture_hash)?);
            continue;
        }
        let (left, right) = source
            .recipe_edges(handle)
            .context("unsupported Worker Program recipe node")?;
        let first = *cells.get(left).context("unbound first Program recipe")?;
        let second = *cells.get(right).context("unbound second Program recipe")?;
        ensure!(left < node && right < node, "cyclic Program recipe");
        let compose = instance.get_typed_func::<(i32, i32), i32>(&mut *store, "worker_compose")?;
        let address = compose.call(
            &mut *store,
            (
                i32::try_from(first.address)?,
                i32::try_from(second.address)?,
            ),
        )?;
        ensure!(address > 0, "installed Program composition refused");
        let address = u32::try_from(address)?;
        let depth = cell_field(store, instance, "cell_w", first.address)?
            .max(cell_field(store, instance, "cell_w", second.address)?)
            .checked_add(1)
            .context("Program recipe depth overflow")?;
        let leaves = cell_field(store, instance, "cell_n", first.address)?
            .checked_add(cell_field(store, instance, "cell_n", second.address)?)
            .context("Program recipe leaves overflow")?;
        let payload = instance
            .get_typed_func::<i32, i64>(&mut *store, "cell_payload")?
            .call(&mut *store, i32::try_from(address)?)?;
        let left_effects = instance
            .get_typed_func::<i32, i64>(&mut *store, "cell_payload")?
            .call(&mut *store, i32::try_from(first.address)?)?;
        let right_effects = instance
            .get_typed_func::<i32, i64>(&mut *store, "cell_payload")?
            .call(&mut *store, i32::try_from(second.address)?)?;
        ensure!(
            cell_field(store, instance, "cell_kind", address)? == 4
                && cell_field(store, instance, "cell_x", address)? == 2
                && cell_field(store, instance, "cell_y", address)?
                    == cell_field(store, instance, "cell_y", first.address)?
                && cell_field(store, instance, "cell_z", address)?
                    == cell_field(store, instance, "cell_z", second.address)?
                && cell_field(store, instance, "cell_a", address)? == first.address
                && cell_field(store, instance, "cell_b", address)? == second.address
                && cell_field(store, instance, "cell_w", address)? == depth
                && cell_field(store, instance, "cell_n", address)? == leaves
                && payload == (left_effects | right_effects),
            "installed composition differs from checked graph"
        );
        let owner_identity = source.root_owner_identity();
        let definition_identity = if first.definition_identity == owner_identity
            || second.definition_identity == owner_identity
        {
            owner_identity
        } else {
            0
        };
        cells.push(ProgramCell {
            scope: source.instance_id(),
            address,
            definition_identity,
            capture_hash,
            installation: 0,
        });
    }
    cells
        .get(index)
        .copied()
        .context("selected Program cell not materialized")
}

fn link(engine: &Engine, component: &Component, linker: &mut Linker<Host>) -> Result<()> {
    for (name, item) in component.component_type().imports(engine) {
        match item {
            wasmtime::component::types::ComponentItem::ComponentInstance(interface)
                if name == INTERFACE =>
            {
                let mut imports = linker.instance(name)?;
                imports.resource(
                    "program",
                    ResourceType::host::<ProgramCell>(),
                    |mut store, rep| {
                        store
                            .data_mut()
                            .release(Resource::<ProgramCell>::new_own(rep))?;
                        Ok(())
                    },
                )?;
                for (function, item) in interface.exports(engine) {
                    match item {
                        wasmtime::component::types::ComponentItem::ComponentFunc(_)
                            if function == "admit" =>
                        {
                            imports.func_wrap("admit", |mut cx, (): ()| {
                                ensure!(!cx.data().task.cancelled()?, "Program task cancelled");
                                let installed =
                                    cx.data().installed.context("uninstalled source")?;
                                ensure!(
                                    installed.cell.scope == cx.data().scope,
                                    "foreign Program source scope"
                                );
                                let host = cx.data_mut();
                                let cell = host.table.push(installed.cell)?;
                                host.admits = host
                                    .admits
                                    .checked_add(1)
                                    .context("Program admission count overflow")?;
                                host.live_owners = host
                                    .live_owners
                                    .checked_add(1)
                                    .context("Program owner count overflow")?;
                                Ok((cell,))
                            })?;
                        }
                        wasmtime::component::types::ComponentItem::ComponentFunc(_)
                            if function == "invoke" =>
                        {
                            imports.func_wrap(
                                "invoke",
                                |mut cx, (owner, operand): (Resource<ProgramCell>, i64)| {
                                    let cell = cx.data_mut().release(owner)?;
                                    ensure!(!cx.data().task.cancelled()?, "Program task cancelled");
                                    let installed =
                                        cx.data().installed.context("uninstalled source")?;
                                    ensure!(
                                        cell.scope == cx.data().scope && cell == installed.cell,
                                        "foreign or stale Program owner"
                                    );
                                    let begin = installed
                                        .instance
                                        .get_typed_func::<(i32, i32, i32, i32, i32, i32), i32>(
                                            &mut cx, "begin",
                                        )?;
                                    let budget = (32_768, 256, 32, 2048, 4096, 4096);
                                    ensure!(
                                        begin.call(&mut cx, budget)? == 0,
                                        "source task budget admission refused"
                                    );
                                    let invoke =
                                        installed.instance.get_typed_func::<(i32, i64), i64>(
                                            &mut cx,
                                            "worker_invoke",
                                        )?;
                                    let result = invoke
                                        .call(&mut cx, (i32::try_from(cell.address)?, operand))?;
                                    let metric = installed
                                        .instance
                                        .get_typed_func::<i32, i64>(&mut cx, "metric")?;
                                    ensure!(
                                        metric.call(&mut cx, 0)? == 0,
                                        "source Program task failed"
                                    );
                                    let host = cx.data_mut();
                                    host.invokes = host
                                        .invokes
                                        .checked_add(1)
                                        .context("Program invocation count overflow")?;
                                    Ok((result,))
                                },
                            )?;
                        }
                        wasmtime::component::types::ComponentItem::ComponentFunc(_)
                            if function == "request-code-export" =>
                        {
                            imports.func_wrap(
                                "request-code-export",
                                |mut cx, (owner, destination): (Resource<ProgramCell>, String)| {
                                    let host = cx.data_mut();
                                    host.code_export_requests = host
                                        .code_export_requests
                                        .checked_add(1)
                                        .context("Worker export request count overflow")?;
                                    // Record the default denial before even resolving the
                                    // supplied owner; invalid owners cannot cause publication.
                                    host.code_export_denials = host
                                        .code_export_denials
                                        .checked_add(1)
                                        .context("Worker export denial count overflow")?;
                                    let cell = host.release(owner)?;
                                    let installed =
                                        host.installed.context("uninstalled export subject")?;
                                    ensure!(
                                        cell == installed.cell
                                            && cell.scope == host.scope
                                            && host.export_capture == Some(cell)
                                            && host
                                                .policy
                                                .as_ref()
                                                .is_some_and(|policy| policy.binding().operation
                                                    == "test.code-export"),
                                        "foreign or unbound private capture export subject"
                                    );
                                    // The owner refers to the actual checked, installed Text
                                    // capture. The typed destination is received, but this
                                    // deny-only host cannot authorize either destination or
                                    // capture and never serializes their content.
                                    let _ = destination;
                                    Ok((false,))
                                },
                            )?;
                        }
                        wasmtime::component::types::ComponentItem::Resource(_)
                            if function == "program" => {}
                        _ => bail!("unapproved Worker Program import {function}"),
                    }
                }
            }
            wasmtime::component::types::ComponentItem::Type(_) => {}
            _ => bail!("unapproved Worker component import {name}"),
        }
    }
    Ok(())
}

pub(super) struct Observation {
    pub(super) value: i64,
    pub(super) admits: u32,
    pub(super) invokes: u32,
    pub(super) live_owners: u32,
    pub(super) effect_requests: u32,
    pub(super) protected_operations: u32,
    pub(super) code_export_requests: u32,
    pub(super) code_export_denials: u32,
    pub(super) capture_published: bool,
}

#[derive(Debug)]
pub(super) enum GuestFailure {
    Exhausted,
    Invalid(anyhow::Error),
}

#[derive(Debug)]
struct ForeignOrStaleProgramNode;
impl std::fmt::Display for ForeignOrStaleProgramNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("foreign or stale Program node")
    }
}
impl std::error::Error for ForeignOrStaleProgramNode {}

#[derive(Debug)]
struct UnselectedProgramNode;
impl std::fmt::Display for UnselectedProgramNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Program node is not rooted in the selected source export")
    }
}
impl std::error::Error for UnselectedProgramNode {}

#[derive(Clone, Copy)]
struct BatchIterations(u16);
impl BatchIterations {
    const ONE: Self = Self(1);

    fn checked(value: i64) -> std::result::Result<Self, GuestFailure> {
        u16::try_from(value)
            .ok()
            .filter(|count| (1..=BATCH_ITERATION_LIMIT).contains(count))
            .map(Self)
            .ok_or_else(|| {
                GuestFailure::Invalid(anyhow::anyhow!(
                    "Worker batch iteration count must be 1..=512"
                ))
            })
    }
}

#[derive(Clone, Copy)]
enum GuestAction<'a> {
    Invoke,
    CodeExport(&'a str),
    CoreCodeExport,
}

fn install_selected(
    engine: &Engine,
    source: &ProgramStore,
    root: Handle,
    task: TaskOwner,
) -> Result<Store<Host>> {
    source
        .require_selected_root(root)
        .map_err(|refusal| match refusal {
            Refusal::Invalid => anyhow::Error::new(ForeignOrStaleProgramNode),
            Refusal::Unsupported => anyhow::Error::new(UnselectedProgramNode),
            Refusal::Exhausted => anyhow::Error::new(UnselectedProgramNode),
        })?;
    let installation = NEXT_INSTALLATION
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
            next.checked_add(1)
        })
        .map_err(|_| anyhow::anyhow!("Worker installation IDs exhausted"))?;
    let core = core_module(engine, source.wat())?;
    let mut store = Store::new(
        engine,
        Host {
            table: ResourceTable::new(),
            installed: None,
            export_capture: None,
            policy: source.host_policy().cloned(),
            scope: source.instance_id(),
            task,
            limits: StoreLimitsBuilder::new()
                .memory_size(4_194_304)
                .table_elements(16_384)
                .instances(8)
                .tables(8)
                .build(),
            admits: 0,
            invokes: 0,
            live_owners: 0,
            effect_requests: 0,
            protected_operations: 0,
            code_export_requests: 0,
            code_export_denials: 0,
            capture_published: false,
        },
    );
    store.limiter(|host| &mut host.limits);
    store.epoch_deadline_trap();
    store.set_epoch_deadline(1);
    store.set_fuel(FUEL)?;
    let instance = install_core(&mut store, &core)?;
    let begin =
        instance.get_typed_func::<(i32, i32, i32, i32, i32, i32), i32>(&mut store, "begin")?;
    ensure!(
        begin.call(&mut store, (32_768, 256, 32, 2048, 4096, 4096))? == 0,
        "checked source installation budget refused"
    );
    let install = instance.get_typed_func::<(), i32>(&mut store, "worker_install")?;
    ensure!(
        install.call(&mut store, ())? == 0,
        "source Program install failed"
    );
    let mut cell = bind_installed(&mut store, instance, source, root)?;
    cell.installation = installation;
    let host = store.data_mut();
    if source
        .host_policy()
        .is_some_and(|policy| policy.binding().operation == "test.code-export")
        && !source.text_aliases().is_empty()
    {
        host.export_capture = Some(cell);
    }
    host.installed = Some(Installed { instance, cell });
    Ok(store)
}

/// Inspect actual runtime Text cells before accepting a portable capture
/// package. Neither a source literal nor a package row can attest itself.
pub(super) fn attest(source: &ProgramStore, root: Handle) -> Result<[u8; 32]> {
    let mut config = Config::new();
    config
        .wasm_component_model(true)
        .consume_fuel(true)
        .epoch_interruption(true);
    let engine = Engine::new(&config)?;
    let store = install_selected(&engine, source, root, TaskOwner::for_source(source))?;
    Ok(store
        .data()
        .installed
        .context("missing installed Program cell")?
        .cell
        .capture_hash)
}

/// The selected guest is always rebuilt from the WIT and Noble body retained
/// here; no package or caller-supplied component binary is installed.
pub(super) fn execute(
    source: &mut ProgramStore,
    root: Handle,
    operand: i64,
    iterations: i64,
) -> std::result::Result<Observation, GuestFailure> {
    let iterations = BatchIterations::checked(iterations)?;
    source
        .reserve_guest()
        .map_err(|_| GuestFailure::Exhausted)?;
    let task = TaskOwner::for_source(source);
    execute_reserved_with_task(
        source,
        root,
        operand,
        iterations,
        &task,
        GuestAction::Invoke,
        false,
        false,
    )
    .map_err(GuestFailure::Invalid)
}

/// Only an embedder holding the selected policy may explicitly issue a fresh
/// installed source cell; CLI entrypoints always use `execute` instead.
pub(crate) fn execute_with_host_grant(
    source: &mut ProgramStore,
    root: Handle,
    operand: i64,
    iterations: i64,
    policy: &WorkerHostPolicy,
) -> std::result::Result<Observation, GuestFailure> {
    let iterations = BatchIterations::checked(iterations)?;
    if !source
        .host_policy()
        .is_some_and(|selected| std::sync::Arc::ptr_eq(&selected.inner, &policy.inner))
    {
        return Err(GuestFailure::Invalid(anyhow::anyhow!(
            "foreign Worker host policy"
        )));
    }
    source
        .reserve_guest()
        .map_err(|_| GuestFailure::Exhausted)?;
    let task = TaskOwner::for_source(source);
    execute_reserved_with_task(
        source,
        root,
        operand,
        iterations,
        &task,
        GuestAction::Invoke,
        true,
        false,
    )
    .map_err(GuestFailure::Invalid)
}

pub(super) fn execute_with_task(
    source: &mut ProgramStore,
    root: Handle,
    operand: i64,
    iterations: i64,
    task: &TaskOwner,
) -> std::result::Result<Observation, GuestFailure> {
    let iterations = BatchIterations::checked(iterations)?;
    if source.host_policy().is_some_and(|policy| {
        !task
            .policy
            .as_ref()
            .is_some_and(|selected| std::sync::Arc::ptr_eq(&selected.inner, &policy.inner))
    }) {
        return Err(GuestFailure::Invalid(anyhow::anyhow!(
            "unbound Worker policy task"
        )));
    }
    source
        .reserve_guest()
        .map_err(|_| GuestFailure::Exhausted)?;
    execute_reserved_with_task(
        source,
        root,
        operand,
        iterations,
        task,
        GuestAction::Invoke,
        false,
        false,
    )
    .map_err(GuestFailure::Invalid)
}

/// Real typed WIT host request, but no positive production exporter exists:
/// the bound private source cell is checked and installed before the one
/// default-denied request; no capture/source/package bytes are serialized.
pub(crate) fn request_code_export(
    source: &mut ProgramStore,
    root: Handle,
    operand: i64,
    destination: &str,
) -> std::result::Result<serde_json::Value, GuestFailure> {
    if source.text_aliases().is_empty()
        || source
            .host_policy()
            .is_none_or(|policy| policy.binding().operation != "test.code-export")
        || destination.is_empty()
        || destination.len() > 128
        || !destination.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-' | b':')
        })
    {
        return Err(GuestFailure::Invalid(anyhow::anyhow!(
            "unbound Worker private Text export request"
        )));
    }
    source
        .reserve_guest()
        .map_err(|_| GuestFailure::Exhausted)?;
    let task = TaskOwner::for_source(source);
    let observation = execute_reserved_with_task(
        source,
        root,
        operand,
        BatchIterations::ONE,
        &task,
        GuestAction::CodeExport(destination),
        false,
        false,
    )
    .map_err(GuestFailure::Invalid)?;
    Ok(serde_json::json!({
        "schema":"noble-worker-program-code-export/v2",
        "stage":"export-authorization","outcome":"denied",
        "guest_requests":observation.code_export_requests,
        "protected_operations":observation.protected_operations,
        "capture_published":observation.capture_published,
        "capturability_grants_export_authority":observation.protected_operations != 0,
        "denial_recorded":observation.code_export_denials == 1,
        "candidate_body_requests":observation.invokes
    }))
}

/// Unlike WIT's owner request, this executes the checked source body and
/// inspects the separate, always-denied Core adapter's actual callback count.
pub(crate) fn request_core_code_export(
    source: &mut ProgramStore,
    root: Handle,
    operand: i64,
) -> Result<serde_json::Value> {
    ensure!(
        source
            .host_policy()
            .is_some_and(|policy| policy.binding().operation == "test.code-export"),
        "unselected source export effect"
    );
    source
        .reserve_guest()
        .map_err(|_| anyhow::anyhow!("Core export work exhausted"))?;
    let task = TaskOwner::for_source(source);
    let observed = execute_reserved_with_task(
        source,
        root,
        operand,
        BatchIterations::ONE,
        &task,
        GuestAction::CoreCodeExport,
        false,
        false,
    )?;
    Ok(serde_json::json!({
        "schema":"noble-worker-program-core-code-export/v1",
        "outcome":"denied","core_requests":observed.code_export_requests,
        "core_denials":observed.code_export_denials,
        "protected_operations":observed.protected_operations,
        "capture_published":observed.capture_published,
        "completed_invocations":observed.invokes,
    }))
}

/// Dedicated test embedder: source is the only untrusted input. Modes select
/// fixed host code paths, never an authority document, destination or grant.
/// This is not the ordinary Worker CLI/package issuer.
const HOST_EMIT_SOURCE: &[u8] = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"private-ledger-marker\" emit 1 + ] ]";
const HOST_WRONG_RESULT_SOURCE: &[u8] = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"private-ledger-marker\" emit 2 + ] ]";
const HOST_EXPORT_SOURCE: &[u8] = b"module ledger@1 [ require code_export Text Text -- ! test.code-export export capture def capture [ \"private-export-marker\" \"hidden-capture-destination\" code_export ] ]";

fn host_ledger_denial_source() -> Vec<u8> {
    // One checked Text atom, not 257 recipe leaves: four committed copies
    // precede the fifth callback's actual ledger-byte quota refusal.
    let mut source = String::with_capacity(52_554);
    source.push_str("module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"private-ledger-marker");
    source.extend(std::iter::repeat_n('x', 52_408));
    source.push_str("\" dup emit dup emit dup emit dup emit emit 1 + ] ]");
    source.into_bytes()
}

pub(crate) fn run_test_host(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let mut selected_policy = None;
    let mut guest_attempted = false;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {
        eprintln!("noble: Worker host panic (redacted)")
    }));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<serde_json::Value> {
            use std::io::Read;
            ensure!(
                arguments.len() == 4
                    && [
                        "emit",
                        "emit-wrong",
                        "emit-ledger-denied",
                        "emit-request-denied",
                        "emit-panic",
                        "emit-revoked",
                        "wit-export",
                        "core-export"
                    ]
                    .iter()
                    .any(|mode| arguments[3] == *mode),
                "unsupported Worker test host mode"
            );
            let mode = arguments[3]
                .to_str()
                .context("invalid Worker test host mode")?;
            let emit = mode.starts_with("emit");
            // The fixture source, unlike stdin, is chosen before evaluating any
            // candidate bytes. No arbitrary checked candidate can select a grant.
            let trusted_source = match mode {
                "emit" | "emit-request-denied" => HOST_EMIT_SOURCE.to_vec(),
                "emit-wrong" => HOST_WRONG_RESULT_SOURCE.to_vec(),
                "emit-ledger-denied" => host_ledger_denial_source(),
                "emit-panic" | "emit-revoked" => HOST_EMIT_SOURCE.to_vec(),
                _ => HOST_EXPORT_SOURCE.to_vec(),
            };
            let (name, binding) = if emit {
                (
                    "plus",
                    noble_contracts::source::BoundOperation {
                        module_name: "Ledger".into(),
                        module_version: 1,
                        operation: "test.emit".into(),
                        adapter_identity: "worker-test-emit@1.0.0".into(),
                        adapter_slot: 4,
                        input: vec![noble_kernel::types::Ty::Text],
                        output: vec![],
                        effects: vec![noble_kernel::contracts::TEST_EMIT],
                    },
                )
            } else {
                (
                    "capture",
                    noble_contracts::source::BoundOperation {
                        module_name: "ledger".into(),
                        module_version: 1,
                        operation: "test.code-export".into(),
                        adapter_identity:
                            noble_kernel::contracts::TEST_CODE_EXPORT_ADAPTER_IDENTITY.into(),
                        adapter_slot: 4,
                        input: vec![noble_kernel::types::Ty::Text, noble_kernel::types::Ty::Text],
                        output: vec![],
                        effects: vec![noble_kernel::contracts::TEST_CODE_EXPORT],
                    },
                )
            };
            let destination = "synthetic-ledger";
            let epoch = 1;
            let policy = if emit {
                HostWorkerIssuer::select_checked(
                    &trusted_source,
                    name,
                    destination,
                    epoch,
                    binding.clone(),
                )?
                .policy
            } else {
                WorkerHostPolicy::select_checked(
                    &trusted_source,
                    name,
                    destination,
                    epoch,
                    binding.clone(),
                )?
            };
            selected_policy = Some(policy.clone());
            let mut source = Vec::new();
            std::io::stdin()
                .lock()
                .take(u64::from(super::SOURCE_LIMITS.bytes) + 1)
                .read_to_end(&mut source)?;
            ensure!(
                !source.is_empty() && source.len() <= super::SOURCE_LIMITS.bytes as usize,
                "Worker candidate source byte bound"
            );
            ensure!(
                source == trusted_source,
                "Worker candidate differs from the preselected fixture source"
            );
            let (mut selected, root) = ProgramStore::prepare_with_policy(
                &source,
                name,
                &[noble_kernel::types::Ty::I64],
                policy.clone(),
                160_000_000,
            )
            .map_err(|_| anyhow::anyhow!("Worker candidate Program refused"))?;
            let subject = serde_json::json!({
                "source_sha256":crate::workflow::intrinsic::sha256(&source),
                "definition_identity":policy.inner.owner_identity.to_string(),
                "source_generation":policy.inner.source_generation,
                "destination":destination,"policy_epoch":epoch,
                "adapter_identity":binding.adapter_identity,
                "adapter_slot":binding.adapter_slot,
            });
            if emit {
                let issuer = HostWorkerIssuer { policy };
                if mode == "emit-ledger-denied" || mode == "emit-request-denied" {
                    // These controls have distinct unchanged quotas. One source
                    // invocation reaches the fifth ledger-byte refusal; 257
                    // checked invocations reach the effect-request refusal.
                    let request_limit = mode == "emit-request-denied";
                    guest_attempted = true;
                    let invocation =
                        issuer.invoke(&mut selected, root, 41, if request_limit { 257 } else { 1 });
                    let receipt = issuer.receipt()?;
                    let exact_denial = if request_limit {
                        receipt["effect_requests"] == 257
                            && receipt["protected_operations"] == 256
                            && receipt["denied_requests"] == 1
                            && receipt["ledger_entries"] == 256
                            && receipt["ledger_bytes"] == 5_376
                    } else {
                        receipt["effect_requests"] == 5
                            && receipt["protected_operations"] == 4
                            && receipt["denied_requests"] == 1
                            && receipt["ledger_entries"] == 4
                            && receipt["ledger_bytes"] == 209_716
                    };
                    return Ok(match invocation {
                        Err(GuestFailure::Invalid(_)) if exact_denial => serde_json::json!({
                            "schema":"noble-worker-program-test-host/v1","outcome":"guest-trapped",
                            "operation":"test.emit","subject":subject,"guest_trapped":true,
                            "guest_failure_kind":"source-effect-denied","receipt":receipt,
                        }),
                        Ok(observed) => serde_json::json!({
                            "schema":"noble-worker-program-test-host/v1",
                            "outcome":"unexpected-guest-success","operation":"test.emit",
                            "subject":subject,"guest_trapped":false,
                            "issued_value":observed.value,"receipt":receipt,
                        }),
                        Err(_) => serde_json::json!({
                            "schema":"noble-worker-program-test-host/v1",
                            "outcome":"unexpected-guest-failure","operation":"test.emit",
                            "subject":subject,"guest_trapped":false,"receipt":receipt,
                        }),
                    });
                }
                if mode == "emit-panic" {
                    guest_attempted = true;
                    let observed = issuer
                        .invoke(&mut selected, root, 41, 1)
                        .map_err(|_| anyhow::anyhow!("pre-panic Worker invocation refused"))?;
                    ensure!(
                        observed.protected_operations == 1,
                        "host panic fixture never committed its protected effect"
                    );
                    std::panic::panic_any("host-controlled Worker failure after protected append");
                }
                if mode == "emit-revoked" {
                    selected
                        .reserve_guest()
                        .map_err(|_| anyhow::anyhow!("revoked Worker work exhausted"))?;
                    let task = TaskOwner::for_source(&selected);
                    guest_attempted = true;
                    let denied = execute_reserved_with_task(
                        &selected,
                        root,
                        41,
                        BatchIterations::ONE,
                        &task,
                        GuestAction::Invoke,
                        true,
                        true,
                    )
                    .is_err();
                    let receipt = issuer.receipt()?;
                    ensure!(
                        denied
                            && receipt["effect_requests"] == 1
                            && receipt["protected_operations"] == 0
                            && receipt["denied_requests"] == 1,
                        "issued revoked cell did not deny its real callback"
                    );
                    return Ok(serde_json::json!({
                        "schema":"noble-worker-program-test-host/v1","outcome":"denied",
                        "operation":"test.emit","subject":subject,
                        "issued_then_revoked_denied":denied,"receipt":receipt,
                    }));
                }
                let cancelled_task = TaskOwner::for_source(&selected);
                cancelled_task.cancel(selected.instance_id())?;
                let cancelled =
                    execute_with_task(&mut selected, root, 41, 1, &cancelled_task).is_err();
                guest_attempted = true;
                let before = execute(&mut selected, root, 41, 1).is_err();
                let issued = issuer
                    .invoke(&mut selected, root, 41, 1)
                    .map_err(|_| anyhow::anyhow!("issued Worker invocation refused"))?;
                let stale_install = execute(&mut selected, root, 41, 1).is_err();
                let (mut fresh, fresh_root) = issuer
                    .prepare(&source, name, 70_000_000)
                    .map_err(|_| anyhow::anyhow!("fresh Worker session refused"))?;
                let cross_install = execute(&mut fresh, fresh_root, 41, 1).is_err();
                issuer.revoke()?;
                let revoked = execute(&mut selected, root, 41, 1).is_err();
                let receipt = issuer.receipt()?;
                ensure!(
                    cancelled
                        && before
                        && stale_install
                        && cross_install
                        && revoked
                        && issued.effect_requests == 1
                        && issued.protected_operations == 1
                        && receipt["effect_requests"] == 5
                        && receipt["protected_operations"] == 1
                        && receipt["denied_requests"] == 4,
                    "Worker host effect transition differed from selected ledger"
                );
                Ok(serde_json::json!({
                    "schema":"noble-worker-program-test-host/v1","outcome":"normal",
                    "operation":"test.emit","subject":subject,
                    "cancelled_denied":cancelled,"unissued_denied":before,
                    "issued_value":issued.value,
                    "stale_install_denied":stale_install,"cross_session_denied":cross_install,
                    "revoked_denied":revoked,"receipt":receipt,
                }))
            } else if mode == "wit-export" {
                let denial = request_code_export(&mut selected, root, 41, "blocked-destination")
                    .map_err(|_| anyhow::anyhow!("Worker WIT export denial failed"))?;
                Ok(serde_json::json!({
                    "schema":"noble-worker-program-test-host/v1","outcome":"denied",
                    "operation":"wit.request-code-export","subject":subject,
                    "denial":denial,
                }))
            } else {
                let denial = request_core_code_export(&mut selected, root, 41)
                    .map_err(|_| anyhow::anyhow!("Worker Core export denial failed"))?;
                Ok(serde_json::json!({
                    "schema":"noble-worker-program-test-host/v1","outcome":"denied",
                    "operation":"core.test.code-export","subject":subject,
                    "denial":denial,
                }))
            }
        },
    ));
    std::panic::set_hook(previous_hook);
    let panicked = caught.is_err();
    let result = caught.unwrap_or_else(|_| Err(anyhow::anyhow!("Worker host panic")));
    let (outcome, value) = match result {
        Ok(value) => (
            value["outcome"]
                .as_str()
                .unwrap_or("invalid-input")
                .to_owned(),
            value,
        ),
        Err(_) => {
            let outcome = if panicked {
                "execution-panicked"
            } else if guest_attempted {
                "execution-refused"
            } else {
                "invalid-input"
            };
            let receipt = selected_policy
                .as_ref()
                .filter(|policy| policy.binding().operation == "test.emit")
                .map(WorkerHostPolicy::receipt);
            match receipt {
                Some(Ok(receipt)) => (
                    outcome.to_owned(),
                    serde_json::json!({
                        "schema":"noble-worker-program-test-host/v1","outcome":outcome,
                        "operation":"test.emit","diagnostic":"Worker candidate or host execution refused",
                        "receipt":receipt,
                    }),
                ),
                Some(Err(_)) => (
                    "receipt-unavailable".to_owned(),
                    serde_json::json!({
                        "schema":"noble-worker-program-test-host/v1",
                        "outcome":"receipt-unavailable","operation":"test.emit",
                        "diagnostic":"Worker policy state unavailable; protected effects are unknown",
                    }),
                ),
                None => (
                    outcome.to_owned(),
                    serde_json::json!({
                        "schema":"noble-worker-program-test-host/v1","outcome":outcome,
                        "diagnostic":"Worker candidate or host execution refused",
                        "effect_requests":0,"protected_operations":0,
                    }),
                ),
            }
        }
    };
    let report = super::output::Report {
        outcome,
        json: value.to_string(),
    };
    if let Err(error) = super::output::print(&report) {
        eprintln!("noble: {}", error.message);
        return std::process::ExitCode::from(error.exit());
    }
    std::process::ExitCode::from(report.exit())
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-maintainers; reserved execution names its store, root, operand, iteration bound, task, action and grant controls separately so each authority check reads one explicit input."
)]
fn execute_reserved_with_task(
    source: &ProgramStore,
    root: Handle,
    operand: i64,
    iterations: BatchIterations,
    task: &TaskOwner,
    action: GuestAction<'_>,
    grant: bool,
    revoke_after_issue: bool,
) -> Result<Observation> {
    ensure!(
        !revoke_after_issue || grant,
        "cannot revoke an unissued Program"
    );
    ensure!(
        matches!(action, GuestAction::Invoke) || iterations.0 == 1,
        "only checked Program invocation supports a Worker batch"
    );
    ensure!(
        task.scope == source.instance_id(),
        "foreign Program task context"
    );
    let mut scope = ExecutionScope::new(task, source.host_policy());
    ensure!(
        !task.cancelled()?,
        "Program task cancelled before guest admission"
    );
    let world = source
        .selected_world()
        .map_err(|_| anyhow::anyhow!("source Program WIT binding refused"))?;
    let guest_source = match action {
        GuestAction::Invoke | GuestAction::CoreCodeExport => {
            "programs.admit swap programs.invoke".to_owned()
        }
        GuestAction::CodeExport(destination) => {
            format!("programs.admit \"{destination}\" programs.request-code-export drop")
        }
    };
    let checked = world
        .prepare_export(
            "compute",
            guest_source.as_bytes(),
            super::worker_program::SOURCE_LIMITS,
        )
        .map_err(|_| anyhow::anyhow!("selected guest source check refused"))?;
    let guest = noble_wasm::component::compile(&world, &[checked])
        .map_err(|_| anyhow::anyhow!("selected guest compile refused"))?;
    let component = crate::component::assemble_checked_world(&world, guest.wat())
        .map_err(|_| anyhow::anyhow!("selected guest assembly refused"))?;
    let mut config = Config::new();
    config
        .wasm_component_model(true)
        .consume_fuel(true)
        .epoch_interruption(true);
    let engine = Engine::new(&config)?;
    task.start(&engine)?;
    let guest = Component::new(&engine, component)?;
    let mut linker = Linker::new(&engine);
    link(&engine, &guest, &mut linker)?;
    let mut store = install_selected(&engine, source, root, task.clone())?;
    if grant {
        scope.issue(
            source,
            root,
            store
                .data()
                .installed
                .context("missing installed Worker cell")?
                .cell,
        )?;
        if revoke_after_issue {
            scope
                .policy
                .context("missing issued Worker policy")?
                .revoke()?;
        }
    }
    store.set_fuel(FUEL)?;
    let guest = linker.instantiate(&mut store, &guest)?;
    let compute = guest.get_typed_func::<(i64,), (i64,)>(&mut store, "compute")?;
    let mut value = operand;
    for step in 0..u32::from(iterations.0) {
        #[cfg(test)]
        if step == 1 {
            if let Some((ready, resume)) = &task.between_steps {
                ready
                    .send(())
                    .context("batch step test synchronization failed")?;
                resume
                    .lock()
                    .map_err(|_| anyhow::anyhow!("batch step test lock poisoned"))?
                    .recv_timeout(std::time::Duration::from_secs(30))
                    .context("batch step test continuation timed out")?;
            }
        }
        if step != 0 {
            if grant {
                scope
                    .policy
                    .context("missing Worker batch issuer")?
                    .continue_issued(
                        scope.issued.context("missing issued Worker batch cell")?,
                        task,
                    )?;
            } else {
                ensure!(
                    task.running()?,
                    "Worker batch task retired between iterations"
                );
            }
        }
        let (next,) = match compute.call(&mut store, (value,)) {
            Ok(next) if !matches!(action, GuestAction::CoreCodeExport) => next,
            Ok(_) => bail!("source code-export effect unexpectedly returned"),
            Err(_) if matches!(action, GuestAction::CoreCodeExport) => {
                let host = store.data();
                ensure!(
                    host.admits == 1
                        && host.code_export_requests == 1
                        && host.code_export_denials == 1
                        && host.protected_operations == 0
                        && !host.capture_published
                        && host.invokes == 0,
                    "source effect failed without an exact denied Core export request"
                );
                return Ok(Observation {
                    value: operand,
                    admits: host.admits,
                    invokes: host.invokes,
                    live_owners: host.live_owners,
                    effect_requests: host.effect_requests,
                    protected_operations: host.protected_operations,
                    code_export_requests: host.code_export_requests,
                    code_export_denials: host.code_export_denials,
                    capture_published: host.capture_published,
                });
            }
            Err(problem) => return Err(problem),
        };
        compute.post_return(&mut store)?;
        let host = store.data();
        if matches!(action, GuestAction::Invoke) {
            ensure!(
                host.admits == step + 1
                    && host.invokes == step + 1
                    && host.live_owners == 0
                    && host.code_export_requests == 0,
                "guest did not consume exactly one checked Program owner per iteration"
            );
        } else {
            ensure!(
                host.admits == 1
                    && host.invokes == 0
                    && host.live_owners == 0
                    && host.code_export_requests == 1
                    && host.code_export_denials == 1
                    && host.protected_operations == 0
                    && !host.capture_published,
                "code-export request did not remain a single protected-work-free denial"
            );
        }
        value = next;
    }
    let host = store.data();
    task.complete(value)?;
    let value = task
        .recall(source.instance_id())?
        .context("Program completion retired before recall")?;
    Ok(Observation {
        value,
        admits: host.admits,
        invokes: host.invokes,
        live_owners: host.live_owners,
        effect_requests: host.effect_requests,
        protected_operations: host.protected_operations,
        code_export_requests: host.code_export_requests,
        code_export_denials: host.code_export_denials,
        capture_published: host.capture_published,
    })
}

// Test harness only: mpsc channels observe revoke/cancel acknowledgement order.
#[allow(
    tigerstyle::compound_assertion,
    tigerstyle::negated_predicate,
    tigerstyle::unbounded_channel,
    reason = "Owner: noble-maintainers; these concurrency tests use short-lived mpsc channels to observe revoke/cancel ordering and assert one combined ordering property, so the harness form is the readable one."
)]
#[cfg(test)]
mod tests {
    use super::*;
    use noble_kernel::types::Ty;

    fn host_selected_policy(source: &[u8], export: bool) -> WorkerHostPolicy {
        let binding = if export {
            noble_contracts::source::BoundOperation {
                module_name: "ledger".into(),
                module_version: 1,
                operation: "test.code-export".into(),
                adapter_identity: "worker-test-code-export@1.0.0".into(),
                adapter_slot: 4,
                input: vec![Ty::Text, Ty::Text],
                output: vec![],
                effects: vec![noble_kernel::contracts::TEST_CODE_EXPORT],
            }
        } else {
            noble_contracts::source::BoundOperation {
                module_name: "Ledger".into(),
                module_version: 1,
                operation: "test.emit".into(),
                adapter_identity: "worker-test-emit@1.0.0".into(),
                adapter_slot: 4,
                input: vec![Ty::Text],
                output: vec![],
                effects: vec![noble_kernel::contracts::TEST_EMIT],
            }
        };
        let retained = noble_contracts::source::RetainedSourceLimit::new(
            super::super::worker_program::SOURCE_LIMITS.bytes,
        )
        .expect("selected Worker source limit");
        let session =
            noble_contracts::source::ModuleSession::new(std::slice::from_ref(&binding), retained)
                .unwrap();
        let module = session
            .prepare(source, &[], super::super::worker_program::SOURCE_LIMITS)
            .expect("host independently checks selected source bytes");
        let owner = module
            .checked_program_owner_for_binding(if export { "capture" } else { "plus" }, &binding)
            .expect("host-selected exact checked owner");
        let (session, committed) = session.commit(module);
        committed.unwrap();
        WorkerHostPolicy::select(
            crate::workflow::intrinsic::sha256(source),
            owner.definition_identity(),
            session.generation(),
            "local-test-destination".into(),
            1,
            binding,
        )
        .unwrap()
    }

    #[test]
    fn checked_bound_guest_emits_only_under_current_host_grant() {
        let source = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let policy = host_selected_policy(source, false);
        let (mut selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            160_000_000,
        )
        .unwrap();
        assert!(
            matches!(
                execute(&mut selected, root, 41, 1),
                Err(GuestFailure::Invalid(_))
            ),
            "selection before checked installation is never an effect grant"
        );
        assert_eq!(policy.counts().unwrap(), (1, 0));
        let observed = execute_with_host_grant(&mut selected, root, 41, 1, &policy).unwrap();
        assert_eq!(observed.value, 42);
        assert_eq!(
            (observed.effect_requests, observed.protected_operations),
            (1, 1)
        );
        assert_eq!(policy.counts().unwrap(), (2, 1));
        assert_eq!(
            policy.ledger().unwrap(),
            [("local-test-destination".into(), "audit".into())]
        );
        assert!(
            matches!(
                execute(&mut selected, root, 41, 1),
                Err(GuestFailure::Invalid(_))
            ),
            "a fresh installation with the same source, slot and address has no grant"
        );
        assert_eq!(policy.counts().unwrap(), (3, 1));
        assert_eq!(
            execute_with_host_grant(&mut selected, root, 41, 1, &policy)
                .unwrap()
                .value,
            42
        );
        assert_eq!(policy.counts().unwrap(), (4, 2));
        policy.revoke().unwrap();
        assert!(
            matches!(
                execute(&mut selected, root, 41, 1),
                Err(GuestFailure::Invalid(_))
            ),
            "the same source and WIT owner cannot reuse a revoked host grant"
        );
        assert_eq!(policy.counts().unwrap(), (5, 2));
        assert_eq!(policy.receipt().unwrap()["denied_requests"], 3);
        assert_eq!(policy.ledger().unwrap().len(), 2);

        let denied = host_selected_policy(source, false);
        let (mut selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            denied.clone(),
            70_000_000,
        )
        .unwrap();
        assert!(matches!(
            execute(&mut selected, root, 41, 1),
            Err(GuestFailure::Invalid(_))
        ));
        assert_eq!(denied.counts().unwrap(), (1, 0));
        assert_eq!(denied.receipt().unwrap()["denied_requests"], 1);
        assert!(denied.ledger().unwrap().is_empty());

        let revoked = host_selected_policy(source, false);
        let (mut selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            revoked.clone(),
            70_000_000,
        )
        .unwrap();
        revoked.revoke().unwrap();
        assert!(matches!(
            execute(&mut selected, root, 41, 1),
            Err(GuestFailure::Invalid(_))
        ));
        assert_eq!(revoked.counts().unwrap(), (1, 0));
        assert!(revoked.ledger().unwrap().is_empty());

        let expired = host_selected_policy(source, false);
        let (mut selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            expired.clone(),
            70_000_000,
        )
        .unwrap();
        expired.advance_epoch(2).unwrap();
        assert!(matches!(
            execute(&mut selected, root, 41, 1),
            Err(GuestFailure::Invalid(_))
        ));
        assert_eq!(expired.counts().unwrap(), (1, 0));
        assert!(expired.ledger().unwrap().is_empty());

        let cancelled = host_selected_policy(source, false);
        let (mut selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            cancelled.clone(),
            70_000_000,
        )
        .unwrap();
        let task = TaskOwner::for_source(&selected);
        assert!(task.cancel(selected.instance_id()).unwrap());
        assert!(matches!(
            execute_with_task(&mut selected, root, 41, 1, &task),
            Err(GuestFailure::Invalid(_))
        ));
        assert_eq!(cancelled.counts().unwrap(), (0, 0));
        assert!(cancelled.ledger().unwrap().is_empty());
    }

    #[test]
    fn revocation_and_cancellation_ack_only_after_atomic_ledger_commit() {
        use std::sync::{mpsc, Barrier};
        use std::time::Duration;

        let source = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let policy = host_selected_policy(source, false);
        let (selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            70_000_000,
        )
        .unwrap();
        let task = TaskOwner::for_source(&selected);
        let mut config = Config::new();
        config.consume_fuel(true).epoch_interruption(true);
        let engine = Engine::new(&config).unwrap();
        let mut execution = ExecutionScope::new(&task, selected.host_policy());
        task.start(&engine).unwrap();
        let installed = install_selected(&engine, &selected, root, task.clone()).unwrap();
        let cell = installed.data().installed.unwrap().cell;
        execution.issue(&selected, root, cell).unwrap();
        let scope = selected.instance_id();
        let entered = std::sync::Arc::new(Barrier::new(2));
        let release = std::sync::Arc::new(Barrier::new(2));
        policy.inner.state.lock().unwrap().pre_commit = Some((entered.clone(), release.clone()));
        std::thread::scope(|threads| {
            let emitter = threads.spawn(|| policy.emit(cell, 4, "audit", &task).unwrap());
            entered.wait();
            let (started, started_rx) = mpsc::channel();
            let (ack, ack_rx) = mpsc::channel();
            let revocation_policy = &policy;
            let started_revoke = started.clone();
            let ack_revoke = ack.clone();
            let revoker = threads.spawn(move || {
                started_revoke.send(()).unwrap();
                revocation_policy.revoke().unwrap();
                ack_revoke.send("revoked").unwrap();
            });
            let cancellation_task = &task;
            let canceler = threads.spawn(move || {
                started.send(()).unwrap();
                cancellation_task.cancel(scope).unwrap();
                ack.send("cancelled").unwrap();
            });
            let both_started = started_rx.recv_timeout(Duration::from_secs(2)).is_ok()
                && started_rx.recv_timeout(Duration::from_secs(2)).is_ok();
            let serialized = policy.inner.state.try_lock().is_err();
            let no_early_ack = ack_rx.recv_timeout(Duration::from_millis(40)).is_err();
            release.wait();
            assert!(matches!(emitter.join().unwrap(), EffectOutcome::Committed));
            revoker.join().unwrap();
            canceler.join().unwrap();
            assert!(
                both_started && serialized && no_early_ack,
                "revoke/cancel must not acknowledge ahead of serialized append"
            );
            let first = ack_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            let second = ack_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_ne!(first, second);
            assert!(matches!(first, "revoked" | "cancelled"));
            assert!(matches!(second, "revoked" | "cancelled"));
        });
        assert_eq!(
            policy.ledger().unwrap(),
            [("local-test-destination".into(), "audit".into())]
        );
        assert_eq!(policy.receipt().unwrap()["succeeded"], 1);
        assert!(matches!(
            policy.emit(cell, 4, "late", &task).unwrap(),
            EffectOutcome::Denied
        ));
        assert_eq!(policy.receipt().unwrap()["denied_requests"], 1);
        assert_eq!(policy.ledger().unwrap().len(), 1);
    }

    #[test]
    fn selected_effectful_composition_rebinds_original_checked_host_owner() {
        let source = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let policy = host_selected_policy(source, false);
        let (mut selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            70_000_000,
        )
        .unwrap();
        let composed = selected.compose(root, root).unwrap();
        let observed = execute_with_host_grant(&mut selected, composed, 41, 1, &policy).unwrap();
        assert_eq!(observed.value, 43);
        assert_eq!(
            (observed.effect_requests, observed.protected_operations),
            (2, 2)
        );
        assert_eq!(
            policy.ledger().unwrap(),
            [
                ("local-test-destination".into(), "audit".into()),
                ("local-test-destination".into(), "audit".into()),
            ]
        );
    }

    #[test]
    fn synthetic_ledger_refuses_the_first_byte_over_its_shared_quota() {
        let source = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let policy = host_selected_policy(source, false);
        let (selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            70_000_000,
        )
        .unwrap();
        let task = TaskOwner::for_source(&selected);
        let mut config = Config::new();
        config.consume_fuel(true).epoch_interruption(true);
        let engine = Engine::new(&config).unwrap();
        let mut execution = ExecutionScope::new(&task, selected.host_policy());
        task.start(&engine).unwrap();
        let installed = install_selected(&engine, &selected, root, task.clone()).unwrap();
        let cell = installed.data().installed.unwrap().cell;
        execution.issue(&selected, root, cell).unwrap();
        let full = "a".repeat(EFFECT_TEXT_LIMIT);
        for _ in 0..(LEDGER_BYTES_LIMIT / EFFECT_TEXT_LIMIT) {
            assert!(matches!(
                policy.emit(cell, 4, &full, &task).unwrap(),
                EffectOutcome::Committed
            ));
        }
        assert!(matches!(
            policy.emit(cell, 4, "z", &task).unwrap(),
            EffectOutcome::Denied
        ));
        assert_eq!(policy.receipt().unwrap()["succeeded"], 4);
        assert_eq!(policy.receipt().unwrap()["denied_requests"], 1);
        assert_eq!(policy.ledger().unwrap().len(), 4);
    }

    #[test]
    fn checked_private_text_code_export_request_is_redacted_and_default_denied() {
        let source = b"module ledger@1 [ require code_export Text Text -- ! test.code-export export capture def capture [ \"synthetic-private-text\" \"local-destination\" code_export ] ]";
        let policy = host_selected_policy(source, true);
        let (mut selected, root) = ProgramStore::prepare_with_policy(
            source,
            "capture",
            &[Ty::I64],
            policy.clone(),
            70_000_000,
        )
        .unwrap();
        let report = request_code_export(&mut selected, root, 41, "denied-destination").unwrap();
        assert_eq!(report["stage"], "export-authorization");
        assert_eq!(report["outcome"], "denied");
        assert_eq!(report["guest_requests"], 1);
        assert_eq!(report["protected_operations"], 0);
        assert_eq!(report["capture_published"], false);
        assert_eq!(report["capturability_grants_export_authority"], false);
        assert_eq!(report["denial_recorded"], true);
        assert!(!report.to_string().contains("synthetic-private-text"));
        assert!(!report.to_string().contains("module ledger"));
        assert_eq!(policy.counts().unwrap(), (0, 0));
        assert!(policy.ledger().unwrap().is_empty());
        let denied_source_invoke = match execute(&mut selected, root, 41, 1) {
            Ok(_) => panic!("unissued code-export source effect unexpectedly succeeded"),
            Err(error) => error,
        };
        assert!(
            !format!("{denied_source_invoke:?}").contains("synthetic-private-text"),
            "a direct checked code-export effect must deny without spilling capture bytes"
        );
        assert!(policy.ledger().unwrap().is_empty());
    }

    #[test]
    fn host_grant_is_bound_to_exact_source_and_generation_before_guest_admission() {
        let selected = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let changed = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"different\" emit 1 + ] ]";
        let policy = host_selected_policy(selected, false);
        assert!(matches!(
            ProgramStore::prepare_with_policy(
                changed,
                "plus",
                &[Ty::I64],
                policy.clone(),
                70_000_000
            ),
            Err(super::super::worker_program::Refusal::Invalid)
        ));
        let stale = WorkerHostPolicy::select(
            crate::workflow::intrinsic::sha256(selected),
            policy.inner.owner_identity,
            policy.inner.source_generation + 1,
            policy.inner.destination.clone(),
            policy.inner.epoch,
            policy.binding().clone(),
        )
        .unwrap();
        assert!(matches!(
            ProgramStore::prepare_with_policy(selected, "plus", &[Ty::I64], stale, 70_000_000),
            Err(super::super::worker_program::Refusal::Invalid)
        ));
        assert!(policy.ledger().unwrap().is_empty());
        assert_eq!(policy.counts().unwrap(), (0, 0));
    }

    #[test]
    fn failed_issued_installation_cannot_grant_an_identical_retry() {
        let source = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let policy = host_selected_policy(source, false);
        let (selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            70_000_000,
        )
        .unwrap();
        let mut config = Config::new();
        config
            .wasm_component_model(true)
            .consume_fuel(true)
            .epoch_interruption(true);
        let engine = Engine::new(&config).unwrap();
        let task = TaskOwner::for_source(&selected);
        let mut execution = ExecutionScope::new(&task, selected.host_policy());
        task.start(&engine).unwrap();
        let mut first = None;
        let failed = (|| -> Result<()> {
            let installed = install_selected(&engine, &selected, root, task.clone())?;
            let cell = installed
                .data()
                .installed
                .context("missing first cell")?
                .cell;
            first = Some(cell);
            execution.issue(&selected, root, cell)?;
            let other = install_selected(&engine, &selected, root, task.clone())?;
            let other = other.data().installed.context("missing second cell")?.cell;
            ensure!(
                cell.scope == other.scope
                    && cell.address == other.address
                    && cell.definition_identity == other.definition_identity
                    && cell.capture_hash == other.capture_hash
                    && cell.installation != other.installation,
                "fresh installed cell must have distinct installation identity"
            );
            ensure!(
                matches!(
                    policy.emit(other, 4, "audit", &task)?,
                    EffectOutcome::Denied
                ),
                "stale issue authorized a fresh installation"
            );
            bail!("guest failed after installation, before invoking the source");
        })();
        assert!(failed.is_err());
        drop(execution);
        let first = first.unwrap();
        assert!(policy.inner.state.lock().unwrap().issued.is_none());
        assert!(!task.running().unwrap(), "failed task was not retired");
        let retry_task = TaskOwner::for_source(&selected);
        let _retry_scope = ExecutionScope::new(&retry_task, selected.host_policy());
        retry_task.start(&engine).unwrap();
        let retry = install_selected(&engine, &selected, root, retry_task.clone()).unwrap();
        let second = retry.data().installed.unwrap().cell;
        assert_eq!(
            (
                first.scope,
                first.address,
                first.definition_identity,
                first.capture_hash
            ),
            (
                second.scope,
                second.address,
                second.definition_identity,
                second.capture_hash
            )
        );
        assert_ne!(first.installation, second.installation);
        assert!(
            matches!(
                policy.emit(second, 4, "audit", &retry_task).unwrap(),
                EffectOutcome::Denied
            ),
            "retry without a new issue must not append"
        );
        assert_eq!(policy.counts().unwrap(), (2, 0));
        assert!(policy.ledger().unwrap().is_empty());
    }

    #[test]
    fn unwinding_before_and_after_issue_retires_the_task_and_exact_grant() {
        use std::panic::{catch_unwind, AssertUnwindSafe};
        let source = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let policy = host_selected_policy(source, false);
        let (selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            70_000_000,
        )
        .unwrap();
        let mut config = Config::new();
        config.consume_fuel(true).epoch_interruption(true);
        let engine = Engine::new(&config).unwrap();
        let before = TaskOwner::for_source(&selected);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let _scope = ExecutionScope::new(&before, selected.host_policy());
            before.start(&engine).unwrap();
            panic!("synthetic abort after task start");
        }))
        .is_err());
        assert!(before.cancelled().unwrap());
        assert!(policy.inner.state.lock().unwrap().issued.is_none());

        let after = TaskOwner::for_source(&selected);
        assert!(catch_unwind(AssertUnwindSafe(|| {
            let mut scope = ExecutionScope::new(&after, selected.host_policy());
            after.start(&engine).unwrap();
            let installed = install_selected(&engine, &selected, root, after.clone()).unwrap();
            let cell = installed.data().installed.unwrap().cell;
            scope.issue(&selected, root, cell).unwrap();
            assert!(policy.inner.state.lock().unwrap().issued == Some(cell));
            panic!("synthetic abort after host issue");
        }))
        .is_err());
        assert!(after.cancelled().unwrap());
        assert!(policy.inner.state.lock().unwrap().issued.is_none());
        assert_eq!(policy.counts().unwrap(), (0, 0));
        assert!(policy.ledger().unwrap().is_empty());
    }

    #[test]
    fn fresh_source_session_cannot_reuse_another_installed_scope_grant() {
        let source = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let policy = host_selected_policy(source, false);
        let (mut first, first_root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            70_000_000,
        )
        .unwrap();
        assert_eq!(
            execute_with_host_grant(&mut first, first_root, 41, 1, &policy)
                .unwrap()
                .value,
            42
        );
        let (mut fresh, fresh_root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            70_000_000,
        )
        .unwrap();
        assert_ne!(first.instance_id(), fresh.instance_id());
        assert!(matches!(
            execute(&mut fresh, fresh_root, 41, 1),
            Err(GuestFailure::Invalid(_))
        ));
        assert_eq!(policy.counts().unwrap(), (2, 1));
        assert_eq!(policy.ledger().unwrap().len(), 1);
    }

    #[test]
    fn cancelled_or_foreign_program_task_cannot_recall_a_late_result() {
        let owner = TaskOwner::new(77);
        assert!(owner.cancel(78).is_err());
        assert!(owner.recall(78).is_err());
        let engine = Engine::default();
        owner.start(&engine).unwrap();
        owner.complete(21).unwrap();
        assert!(owner.cancel(77).unwrap());
        assert_eq!(owner.recall(77).unwrap(), None);
        assert!(!owner.cancel(77).unwrap());
        let delivered = TaskOwner::new(79);
        delivered.start(&engine).unwrap();
        delivered.complete(42).unwrap();
        assert_eq!(delivered.recall(79).unwrap(), Some(42));
        assert_eq!(delivered.recall(79).unwrap(), None);
        assert!(
            !delivered.cancel(79).unwrap(),
            "late cancellation cannot revoke an already delivered owner"
        );
    }

    #[test]
    fn selected_guest_executes_checked_program_and_consumes_own_resource() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment",
            &[Ty::I64],
            70_000_000,
        )
        .unwrap();
        let observed = execute(&mut source, root, 41, 1).unwrap();
        assert_eq!(observed.value, 42);
        assert_eq!(
            (observed.admits, observed.invokes, observed.live_owners),
            (1, 1, 0)
        );
        let observed = execute(&mut source, root, -2, 1).unwrap();
        assert_eq!(observed.value, -1);
    }

    #[test]
    fn checked_pure_batch_feeds_each_result_through_a_fresh_linear_owner() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment",
            &[Ty::I64],
            70_000_000,
        )
        .unwrap();
        let two = execute(&mut source, root, 41, 2).unwrap();
        assert_eq!(two.value, 43);
        assert_eq!((two.admits, two.invokes, two.live_owners), (2, 2, 0));
        assert_eq!((two.effect_requests, two.protected_operations), (0, 0));
        let three = execute(&mut source, root, 41, 3).unwrap();
        assert_eq!(three.value, 44);
        assert_eq!((three.admits, three.invokes, three.live_owners), (3, 3, 0));
    }

    #[test]
    fn checked_pure_batch_accepts_512_real_guest_invocations() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment",
            &[Ty::I64],
            70_000_000,
        )
        .unwrap();
        let observed = execute(&mut source, root, 41, 512).unwrap();
        assert_eq!(observed.value, 553);
        assert_eq!(
            (observed.admits, observed.invokes, observed.live_owners),
            (512, 512, 0)
        );
        assert_eq!(
            (observed.effect_requests, observed.protected_operations),
            (0, 0)
        );
    }

    #[test]
    fn invalid_batch_counts_and_foreign_grants_never_reach_effect_callback() {
        let source = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let policy = host_selected_policy(source, false);
        let (mut selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            70_000_000,
        )
        .unwrap();
        for count in [0, -1, 513] {
            assert!(matches!(
                execute_with_host_grant(&mut selected, root, 41, count, &policy),
                Err(GuestFailure::Invalid(_))
            ));
        }
        assert_eq!(policy.counts().unwrap(), (0, 0));
        assert!(policy.ledger().unwrap().is_empty());

        let foreign = host_selected_policy(source, false);
        assert!(matches!(
            execute_with_host_grant(&mut selected, root, 41, 2, &foreign),
            Err(GuestFailure::Invalid(_))
        ));
        assert_eq!(policy.counts().unwrap(), (0, 0));
        assert_eq!(foreign.counts().unwrap(), (0, 0));
        let observed = execute_with_host_grant(&mut selected, root, 41, 2, &policy).unwrap();
        assert_eq!(observed.value, 43);
        assert_eq!(
            (observed.admits, observed.invokes, observed.live_owners),
            (2, 2, 0)
        );
        assert_eq!(
            (observed.effect_requests, observed.protected_operations),
            (2, 2)
        );
        assert_eq!(policy.ledger().unwrap().len(), 2);

        let (mut pure, pure_root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment",
            &[Ty::I64],
            70_000_000,
        )
        .unwrap();
        for count in [0, -1, 513] {
            assert!(matches!(
                execute(&mut pure, pure_root, 41, count),
                Err(GuestFailure::Invalid(_))
            ));
        }
        assert_eq!(execute(&mut pure, pure_root, 41, 2).unwrap().value, 43);
    }

    fn interrupted_batch_cannot_deliver_or_append_after_ack(cancel: bool) {
        use std::sync::{mpsc, Arc, Mutex};
        use std::time::Duration;

        let source = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let policy = host_selected_policy(source, false);
        let (mut selected, root) = ProgramStore::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            70_000_000,
        )
        .unwrap();
        selected.reserve_guest().unwrap();
        let (ready, arrived) = mpsc::channel();
        let (resume, release) = mpsc::channel();
        let mut task = TaskOwner::for_source(&selected);
        task.between_steps = Some((ready, Arc::new(Mutex::new(release))));
        let instance = selected.instance_id();
        let result = std::thread::scope(|threads| {
            let stop_policy = &policy;
            let stop_task = &task;
            let stop = threads.spawn(move || {
                arrived.recv_timeout(Duration::from_secs(30)).unwrap();
                assert_eq!(stop_policy.counts().unwrap(), (1, 1));
                if cancel {
                    assert!(stop_task.cancel(instance).unwrap());
                } else {
                    stop_policy.revoke().unwrap();
                }
                resume.send(()).unwrap();
            });
            let result = execute_reserved_with_task(
                &selected,
                root,
                41,
                BatchIterations(3),
                &task,
                GuestAction::Invoke,
                true,
                false,
            );
            stop.join().unwrap();
            result
        });
        assert!(
            result.is_err(),
            "a stopped batch must not deliver a partial I64"
        );
        assert_eq!(policy.counts().unwrap(), (1, 1));
        assert_eq!(policy.receipt().unwrap()["denied_requests"], 0);
        assert_eq!(policy.ledger().unwrap().len(), 1);
        assert!(task.recall(selected.instance_id()).unwrap().is_none());
        assert!(policy.inner.state.lock().unwrap().issued.is_none());
    }

    #[test]
    fn cancelled_batch_stops_between_committed_steps() {
        interrupted_batch_cannot_deliver_or_append_after_ack(true);
    }

    #[test]
    fn revoked_batch_stops_between_committed_steps() {
        interrupted_batch_cannot_deliver_or_append_after_ack(false);
    }

    #[test]
    fn selected_guest_invokes_actual_bounded_composed_program_cell() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def double [ 2 * ] export plus def plus [ double 1 + ] ]",
            "plus",
            &[Ty::I64],
            70_000_000,
        )
        .unwrap();
        let double = source.checked_root(0).unwrap();
        let adapted = source
            .adapt(root, &noble_kernel::types::EffSet::empty())
            .unwrap();
        let chosen = source.select(&[adapted, double], 1).unwrap();
        assert!(
            matches!(execute(&mut source, chosen, 3, 1),
            Err(GuestFailure::Invalid(problem))
                if problem.downcast_ref::<UnselectedProgramNode>().is_some()),
            "a checked dependency cannot independently become the installed selected root"
        );
        let composed = source.compose(root, double).unwrap();
        assert!(source.recipe_edges(composed).is_some());
        let observed = execute(&mut source, composed, 3, 1).unwrap();
        assert_eq!(observed.value, 14);
        assert_eq!(observed.live_owners, 0);
    }

    #[test]
    fn type_level_widening_does_not_issue_a_host_effect_grant() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment",
            &[Ty::I64],
            50_000_000,
        )
        .unwrap();
        let widened = source
            .adapt(
                root,
                &noble_kernel::types::EffSet::from_ids(&[noble_kernel::contracts::TEST_EMIT]),
            )
            .unwrap();
        assert!(
            matches!(
                execute(&mut source, widened, 41, 1),
                Err(GuestFailure::Invalid(_))
            ),
            "an adapted effect witness alone cannot enter the pure Worker guest"
        );
    }

    #[test]
    fn equal_text_bytes_rebind_as_distinct_checked_lexical_atoms() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ \"same\" drop \"same\" drop 1 + ] export increment ]",
            "increment", &[Ty::I64], 50_000_000).unwrap();
        assert_eq!(source.text_aliases().len(), 2);
        assert_ne!(source.text_aliases()[0].node, source.text_aliases()[1].node);
        assert_eq!(execute(&mut source, root, 41, 1).unwrap().value, 42);
    }

    #[test]
    fn dependent_equal_text_atoms_keep_their_distinct_checked_owners_in_guest() {
        let bytes = b"module Worker@1 [ def helper [ \"same\" drop 2 * ] export plus def plus [ \"same\" drop helper 1 + ] ]";
        let (mut source, root) =
            ProgramStore::prepare(bytes, "plus", &[Ty::I64], 120_000_000).unwrap();
        let helper = source.checked_root(0).unwrap();
        assert_ne!(
            source.source_owner_identity(helper),
            source.source_owner_identity(root)
        );
        assert_eq!(source.text_aliases().len(), 2);
        assert_eq!(
            source.text_aliases()[0].bytes,
            source.text_aliases()[1].bytes
        );
        assert_eq!(execute(&mut source, root, 3, 1).unwrap().value, 7);
        assert!(
            matches!(execute(&mut source, helper, 3, 1),
            Err(GuestFailure::Invalid(problem))
                if problem.downcast_ref::<UnselectedProgramNode>().is_some()),
            "the nested checked Text owner cannot independently obtain a WIT Program"
        );
        let dependency_only = source.compose(helper, helper).unwrap();
        assert!(
            matches!(execute(&mut source, dependency_only, 3, 1),
            Err(GuestFailure::Invalid(problem))
                if problem.downcast_ref::<UnselectedProgramNode>().is_some()),
            "composing only dependency nodes does not recover selected-root authority"
        );
        let composed = source.compose(helper, root).unwrap();
        assert!(
            source.source_owner_identity(composed).is_none(),
            "a composition is not a source-named owner"
        );
        assert_eq!(execute(&mut source, composed, 3, 1).unwrap().value, 13);
        let (mut fresh, _) = ProgramStore::prepare(bytes, "plus", &[Ty::I64], 50_000_000).unwrap();
        assert!(
            matches!(execute(&mut fresh, helper, 3, 1),
            Err(GuestFailure::Invalid(problem))
                if problem.downcast_ref::<ForeignOrStaleProgramNode>().is_some()),
            "a dependency handle cannot cross a fresh source installation"
        );
    }

    #[test]
    fn checked_foreign_instance_handle_cannot_be_installed_in_guest() {
        let (first, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment",
            &[Ty::I64],
            50_000_000,
        )
        .unwrap();
        let (mut other, _) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def decrement [ 1 - ] export decrement ]",
            "decrement",
            &[Ty::I64],
            50_000_000,
        )
        .unwrap();
        assert!(first.named(root).is_some());
        assert_ne!(first.instance_id(), other.instance_id());
        assert!(other.interface(root).is_none());
        assert!(
            matches!(execute(&mut other, root, 3, 1),
            Err(GuestFailure::Invalid(problem))
                if problem.downcast_ref::<ForeignOrStaleProgramNode>().is_some()),
            "another checked module must reject its foreign Program owner before guest admission"
        );
    }

    #[test]
    fn cancelled_and_delivered_tasks_cannot_reenter_guest_admission() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment",
            &[Ty::I64],
            80_000_000,
        )
        .unwrap();
        let cancelled = TaskOwner::new(source.instance_id());
        assert!(cancelled.cancel(source.instance_id()).unwrap());
        assert!(execute_with_task(&mut source, root, 41, 1, &cancelled).is_err());
        let delivered = TaskOwner::new(source.instance_id());
        assert_eq!(
            execute_with_task(&mut source, root, 41, 1, &delivered)
                .unwrap()
                .value,
            42
        );
        assert!(
            execute_with_task(&mut source, root, 7, 1, &delivered).is_err(),
            "one delivered task must not mint a second Program owner"
        );
    }
}
