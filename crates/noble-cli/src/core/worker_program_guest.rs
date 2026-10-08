//! Selected pure Worker Program host. A WIT owner is a Wasmtime resource-table
//! entry pointing at a checked, *installed* source-Wasm Program cell. Neither
//! its component index nor the source-Wasm address is accepted as authority.

use anyhow::{Context, Result, bail, ensure};
use wasmtime::{Config, Engine, ExternType, Global, Linker as CoreLinker, Memory, Module,
    Ref, Store, StoreLimits, StoreLimitsBuilder, Table, Val};
use wasmtime::component::{Component, Linker, Resource, ResourceTable, ResourceType};

use super::worker_program::{Handle, Store as ProgramStore};

const INTERFACE: &str = "noble-test:worker-program/programs@2.0.0";
pub(super) const FUEL: u64 = 10_000_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase { Ready, Running, Completed, Cancelled, Delivered }
struct TaskState {
    phase: Phase,
    result: Option<i64>,
    engine: Option<Engine>,
}
#[derive(Clone)]
pub(super) struct TaskOwner {
    scope: u64,
    state: std::sync::Arc<std::sync::Mutex<TaskState>>,
}
impl TaskOwner {
    pub(super) fn new(scope: u64) -> Self {
        Self { scope, state: std::sync::Arc::new(std::sync::Mutex::new(
            TaskState { phase: Phase::Ready, result: None, engine: None })) }
    }
    fn start(&self, engine: &Engine) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| anyhow::anyhow!("task owner poisoned"))?;
        ensure!(state.phase == Phase::Ready, "retired or reused Program task");
        state.phase = Phase::Running;
        state.engine = Some(engine.clone());
        Ok(())
    }
    fn cancelled(&self) -> Result<bool> {
        Ok(self.state.lock().map_err(|_| anyhow::anyhow!("task owner poisoned"))?
            .phase == Phase::Cancelled)
    }
    fn complete(&self, result: i64) -> Result<()> {
        let mut state = self.state.lock().map_err(|_| anyhow::anyhow!("task owner poisoned"))?;
        ensure!(state.phase == Phase::Running, "late Program task completion retired");
        state.phase = Phase::Completed;
        state.engine = None;
        state.result = Some(result);
        Ok(())
    }
    pub(super) fn recall(&self, scope: u64) -> Result<Option<i64>> {
        ensure!(scope == self.scope, "foreign Program task recall");
        let mut state = self.state.lock().map_err(|_| anyhow::anyhow!("task owner poisoned"))?;
        if state.phase != Phase::Completed { return Ok(None) }
        state.phase = Phase::Delivered;
        Ok(state.result.take())
    }
    pub(super) fn cancel(&self, scope: u64) -> Result<bool> {
        ensure!(scope == self.scope, "foreign Program task cancellation");
        let mut state = self.state.lock().map_err(|_| anyhow::anyhow!("task owner poisoned"))?;
        match state.phase {
            Phase::Ready | Phase::Running | Phase::Completed => {
                if let Some(engine) = state.engine.take() { engine.increment_epoch(); }
                state.phase = Phase::Cancelled;
                state.result = None;
                Ok(true)
            }
            Phase::Cancelled | Phase::Delivered => Ok(false),
        }
    }
    fn retire(&self) {
        if let Ok(mut state) = self.state.lock() {
            if matches!(state.phase, Phase::Ready | Phase::Running | Phase::Completed) {
                state.phase = Phase::Cancelled;
                state.engine = None;
                state.result = None;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ProgramCell {
    scope: u64,
    address: u32,
    definition_identity: u64,
    capture_hash: [u8; 32],
}
#[derive(Clone, Copy)]
struct Installed {
    instance: wasmtime::Instance,
    cell: ProgramCell,
}
struct Host {
    table: ResourceTable,
    installed: Option<Installed>,
    scope: u64,
    task: TaskOwner,
    limits: StoreLimits,
    admits: u32,
    invokes: u32,
    live_owners: u32,
}
impl Host {
    fn release(&mut self, owner: Resource<ProgramCell>) -> Result<ProgramCell> {
        let cell = self.table.delete(owner)?;
        self.live_owners = self.live_owners.checked_sub(1)
            .context("Program owner count underflow")?;
        Ok(cell)
    }
}

fn core_module(engine: &Engine, source_wat: &[u8]) -> Result<Module> {
    let bytes = wat::parse_bytes(source_wat).context("checked source WAT parse")?;
    Module::new(engine, bytes.as_ref()).context("checked source Wasm validation")
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
                ensure!(ty.minimum() == 16 && ty.maximum() == Some(16),
                    "source runtime memory profile mismatch");
                let memory = Memory::new(&mut *store, ty)?;
                linker.define(&*store, "noble", name, memory)?;
            }
            ExternType::Table(ty) if name == "table" => {
                ensure!(ty.minimum() == 16_384 && ty.maximum() == Some(16_384),
                    "source runtime table profile mismatch");
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
            _ => bail!("unapproved Worker source Wasm import {name}"),
        }
    }
    linker.instantiate(&mut *store, module)
}

fn cell_field(store: &mut Store<Host>, instance: wasmtime::Instance,
              field: &str, address: u32) -> Result<u32> {
    let accessor = instance.get_typed_func::<i32, i32>(&mut *store, field)?;
    let value = accessor.call(&mut *store, i32::try_from(address)?)
        .with_context(|| format!("source cell {field} read"))?;
    Ok(u32::try_from(value)?)
}

/// Compare every installed c-recipe atom against the *same checked layout*
/// that emitted it. Text bytes alone cannot distinguish lexical aliases:
/// the matching atom carries the checked source node before its runtime
/// address, length and bytes are accepted.
fn text_alias_hash(store: &mut Store<Host>, instance: wasmtime::Instance,
                   source: &ProgramStore) -> Result<[u8;32]> {
    let mut hash = blake3::Hasher::new();
    let mut total = 0usize;
    for node in 0..source.recipe_nodes() {
        let Some(named) = source.checked_root(node).and_then(|handle| source.named(handle))
            else { continue };
        let index = source.compiled().named_targets().iter()
            .position(|item| item.program_index == named.program_index
                && item.definition_identity == named.definition_identity)
            .context("ambiguous source definition")?;
        let handle = instance.get_typed_func::<i32,i32>(&mut *store,"worker_named_handle")?
            .call(&mut *store, i32::try_from(index)?)?;
        ensure!(handle > 0, "missing installed named Program for Text alias");
        let root = u32::try_from(handle)?;
        let mut programs = vec![(named.program_index, root)];
        let mut seen_programs = std::collections::HashSet::new();
        let aliases = source.text_aliases();
        let start = aliases.iter().position(|alias|
            alias.owner_identity == named.definition_identity).unwrap_or(aliases.len());
        let count = aliases[start..].iter().take_while(|alias|
            alias.owner_identity == named.definition_identity).count();
        let expected = &aliases[start..start + count];
        ensure!(aliases[start + count..].iter().all(|alias|
            alias.owner_identity != named.definition_identity),
            "checked Text aliases are not grouped by source owner");
        let mut matched = vec![false; expected.len()];
        while let Some((program_index, program_address)) = programs.pop() {
            ensure!(seen_programs.insert(program_index) && seen_programs.len() <= 4096,
                "installed quotation Program graph repeats or exceeds bound");
            let metadata = source.compiled().program_metadata().get(program_index)
                .context("checked source recipe metadata absent")?;
            let installed = instance.get_typed_func::<i32,i32>(
                &mut *store,"worker_program_handle")?
                .call(&mut *store,i32::try_from(program_index)?)?;
            ensure!(installed > 0 && program_address == u32::try_from(installed)?,
                "installed quotation Program does not match checked global");
            let payload = instance.get_typed_func::<i32,i64>(&mut *store,"cell_payload")?
                .call(&mut *store,i32::try_from(program_address)?)?;
            ensure!(cell_field(store,instance,"cell_kind",program_address)? == 4
                && payload == i64::from(metadata.effect_mask)
                && cell_field(store,instance,"cell_x",program_address)? == metadata.entry
                && cell_field(store,instance,"cell_y",program_address)? == metadata.input_signature
                && cell_field(store,instance,"cell_z",program_address)? == metadata.output_signature
                && cell_field(store,instance,"cell_a",program_address)? == metadata.capture_left
                && cell_field(store,instance,"cell_b",program_address)? == metadata.capture_right
                && cell_field(store,instance,"cell_w",program_address)? == metadata.recipe_depth
                && cell_field(store,instance,"cell_n",program_address)? == metadata.recipe_leaves,
                "installed quotation Program cell differs from checked source");
            let recipe = cell_field(store,instance,"cell_c",program_address)?;
            ensure!(recipe != 0 || metadata.runtime_atoms.is_empty(),
                "installed Program recipe absent");
            let mut pending = if recipe == 0 { Vec::new() } else { vec![recipe] };
            let mut visited = std::collections::HashSet::new();
            let mut at = 0usize;
            while let Some(address) = pending.pop() {
                ensure!(address != 0 && visited.insert(address),
                    "installed Program recipe has a null or repeated edge");
                ensure!(visited.len() <= 4096 && pending.len() <= 4096,
                    "installed Text/recipe graph exceeds bound");
                match cell_field(store,instance,"cell_kind",address)? {
                    9 => {
                        let left = cell_field(store,instance,"cell_a",address)?;
                        let right = cell_field(store,instance,"cell_b",address)?;
                        ensure!(left != 0 && right != 0
                            && cell_field(store,instance,"cell_c",address)? == 0,
                            "installed concat recipe edges differ from source layout");
                        pending.push(right);
                        pending.push(left);
                    }
                    8 => {
                        let atom = metadata.runtime_atoms.get(at)
                            .context("installed Program has extra recipe atom")?;
                        at += 1;
                        let payload = instance.get_typed_func::<i32,i64>(&mut *store,"cell_payload")?
                            .call(&mut *store,i32::try_from(address)?)?;
                        ensure!(payload == atom.payload
                            && cell_field(store,instance,"cell_x",address)? == atom.tag
                            && cell_field(store,instance,"cell_y",address)? == atom.input
                            && cell_field(store,instance,"cell_z",address)? == atom.output
                            && cell_field(store,instance,"cell_w",address)? == atom.effects
                            && cell_field(store,instance,"cell_n",address)? == atom.ordinal
                            && cell_field(store,instance,"cell_b",address)? == 0
                            && cell_field(store,instance,"cell_c",address)? == 0,
                            "installed recipe atom differs from checked source node");
                        let child = cell_field(store,instance,"cell_a",address)?;
                        match (atom.text, atom.child_program) {
                            (Some((start, length)), None) => {
                                ensure!(child != 0
                                    && cell_field(store,instance,"cell_kind",child)? == 11
                                    && cell_field(store,instance,"cell_x",child)? == start
                                    && cell_field(store,instance,"cell_y",child)? == length,
                                    "installed Text atom differs from checked literal span");
                                total = total.checked_add(usize::try_from(length)?)
                                    .context("Text capture size overflow")?;
                                ensure!(total <= 65_536, "Text capture bytes exceed source limit");
                                let end = start.checked_add(length)
                                    .context("Text capture address overflow")?;
                                ensure!(end <= source.compiled().worker_literal_end()
                                    .context("not a Worker source literal")?,
                                    "Text capture points outside checked literal arena");
                                let memory = instance.get_memory(&mut *store,"memory")
                                    .context("source memory export absent")?;
                                let bytes = memory.data(&*store)
                                    .get(usize::try_from(start)?..usize::try_from(end)?)
                                    .context("Text capture memory out of bounds")?;
                                let index = expected.iter().enumerate().position(|(index, alias)|
                                    !matched[index] && alias.node == atom.source_node
                                        && alias.bytes == bytes)
                                    .context("lexically bound Text differs from installed literal")?;
                                matched[index] = true;
                            }
                            (None, Some(program)) => {
                                let expected_child = instance.get_typed_func::<i32,i32>(
                                    &mut *store,"worker_program_handle")?
                                    .call(&mut *store,i32::try_from(program)?)?;
                                ensure!(expected_child > 0 && child == u32::try_from(expected_child)?,
                                    "installed quotation Program child differs from checked source");
                                programs.push((program, child));
                            }
                            (None, None) => ensure!(child == 0,
                                "installed Program atom contains an unapproved child"),
                            (Some(_), Some(_)) => bail!("ambiguous checked recipe atom"),
                        }
                    }
                    _ => bail!("installed Program recipe contains an unapproved cell"),
                }
            }
            ensure!(at == metadata.runtime_atoms.len(),
                "installed Program omits checked recipe atoms");
        }
        ensure!(matched.iter().all(|matched| *matched),
            "checked Text alias is absent from installed recipe");
        for alias in expected {
            hash.update(&alias.owner_identity.to_le_bytes());
            hash.update(&alias.node.to_le_bytes());
            hash.update(&(alias.bytes.len() as u64).to_le_bytes());
            hash.update(&alias.bytes);
        }
    }
    Ok(*hash.finalize().as_bytes())
}

fn bind_named(store: &mut Store<Host>, instance: wasmtime::Instance,
              source: &ProgramStore, root: Handle, capture_hash: [u8;32]) -> Result<ProgramCell> {
    let named = source.named(root).context("missing checked source owner")?;
    ensure!(named.effects.is_empty(), "named Program contract is not pure");
    let index = source.compiled().named_targets().iter().position(|candidate|
        candidate.program_index == named.program_index
            && candidate.definition_identity == named.definition_identity)
        .context("selected Program is not in checked module metadata")?;
    let count = instance.get_typed_func::<(),i32>(&mut *store, "worker_named_count")?
        .call(&mut *store, ())?;
    ensure!(usize::try_from(count)? == source.compiled().named_targets().len(),
        "installed named Program count differs from checked source");
    let address = instance.get_typed_func::<i32,i32>(&mut *store, "worker_named_handle")?
        .call(&mut *store, i32::try_from(index)?)?;
    ensure!(address > 0, "selected named Program cell is not installed");
    let address = u32::try_from(address)?;
    let metadata = source.compiled().program_metadata().get(named.program_index)
        .context("selected source Program metadata absent")?;
    let payload = instance.get_typed_func::<i32,i64>(&mut *store,"cell_payload")?
        .call(&mut *store, i32::try_from(address)?)?;
    ensure!(cell_field(store, instance, "cell_kind", address)? == 4
        && payload == i64::from(named.effect_mask)
        && metadata.effect_mask == named.effect_mask
        && cell_field(store, instance, "cell_x", address)? == named.entry
        && cell_field(store, instance, "cell_y", address)? == named.input_signature
        && cell_field(store, instance, "cell_z", address)? == named.output_signature
        && cell_field(store, instance, "cell_a", address)? == metadata.capture_left
        && cell_field(store, instance, "cell_b", address)? == metadata.capture_right
        && cell_field(store, instance, "cell_w", address)? == metadata.recipe_depth
        && cell_field(store, instance, "cell_n", address)? == metadata.recipe_leaves,
        "installed source Program cell differs from independently checked metadata");
    ensure!(named.checked_recipe.len() <= 65_536,
        "source Program recipe exceeds bound");
    Ok(ProgramCell { scope: source.instance_id(), address,
        definition_identity: named.definition_identity, capture_hash })
}

fn bind_installed(store: &mut Store<Host>, instance: wasmtime::Instance,
                  source: &ProgramStore, root: Handle) -> Result<ProgramCell> {
    let index = source.node_index(root).context("foreign or stale Program node")?;
    let (input, output, effects) = source.interface(root)
        .context("missing selected Program witness")?;
    ensure!(input == [noble_kernel::types::Ty::I64]
        && output == [noble_kernel::types::Ty::I64]
        && effects.is_empty(), "guest requires an exact pure I64 Program witness");
    let capture_hash = text_alias_hash(store, instance, source)?;
    let mut cells = Vec::with_capacity(source.recipe_nodes());
    for node in 0..source.recipe_nodes() {
        let handle = source.node_handle(node).context("missing checked Program recipe node")?;
        if source.named(handle).is_some() {
            cells.push(bind_named(store, instance, source, handle, capture_hash)?);
            continue;
        }
        let (left, right) = source.recipe_edges(handle)
            .context("unsupported Worker Program recipe node")?;
        let first = *cells.get(left).context("unbound first Program recipe")?;
        let second = *cells.get(right).context("unbound second Program recipe")?;
        ensure!(left < node && right < node, "cyclic Program recipe");
        let compose = instance.get_typed_func::<(i32,i32),i32>(&mut *store,
            "worker_compose")?;
        let address = compose.call(&mut *store,
            (i32::try_from(first.address)?,i32::try_from(second.address)?))?;
        ensure!(address > 0, "installed Program composition refused");
        let address = u32::try_from(address)?;
        let depth = cell_field(store,instance,"cell_w",first.address)?
            .max(cell_field(store,instance,"cell_w",second.address)?)
            .checked_add(1).context("Program recipe depth overflow")?;
        let leaves = cell_field(store,instance,"cell_n",first.address)?
            .checked_add(cell_field(store,instance,"cell_n",second.address)?)
            .context("Program recipe leaves overflow")?;
        let payload = instance.get_typed_func::<i32,i64>(&mut *store,"cell_payload")?
            .call(&mut *store,i32::try_from(address)?)?;
        ensure!(cell_field(store,instance,"cell_kind",address)? == 4
            && cell_field(store,instance,"cell_x",address)? == 2
            && cell_field(store,instance,"cell_y",address)?
                == cell_field(store,instance,"cell_y",first.address)?
            && cell_field(store,instance,"cell_z",address)?
                == cell_field(store,instance,"cell_z",second.address)?
            && cell_field(store,instance,"cell_a",address)? == first.address
            && cell_field(store,instance,"cell_b",address)? == second.address
            && cell_field(store,instance,"cell_w",address)? == depth
            && cell_field(store,instance,"cell_n",address)? == leaves
            && payload == 0, "installed composition differs from checked graph");
        cells.push(ProgramCell { scope: source.instance_id(), address,
            definition_identity: 0, capture_hash });
    }
    cells.get(index).copied().context("selected Program cell not materialized")
}

fn link(engine: &Engine, component: &Component, linker: &mut Linker<Host>) -> Result<()> {
    for (name, item) in component.component_type().imports(engine) {
        match item {
            wasmtime::component::types::ComponentItem::ComponentInstance(interface)
                if name == INTERFACE => {
                let mut imports = linker.instance(name)?;
                imports.resource("program", ResourceType::host::<ProgramCell>(),
                    |mut store, rep| {
                        store.data_mut().release(Resource::<ProgramCell>::new_own(rep))?;
                        Ok(())
                    })?;
                for (function, item) in interface.exports(engine) {
                    match item {
                        wasmtime::component::types::ComponentItem::ComponentFunc(_)
                            if function == "admit" => {
                            imports.func_wrap("admit", |mut cx, (): ()| {
                                ensure!(!cx.data().task.cancelled()?, "Program task cancelled");
                                let installed = cx.data().installed.context("uninstalled source")?;
                                ensure!(installed.cell.scope == cx.data().scope,
                                    "foreign Program source scope");
                                let host = cx.data_mut();
                                let cell = host.table.push(installed.cell)?;
                                host.admits = host.admits.checked_add(1)
                                    .context("Program admission count overflow")?;
                                host.live_owners = host.live_owners.checked_add(1)
                                    .context("Program owner count overflow")?;
                                Ok((cell,))
                            })?;
                        }
                        wasmtime::component::types::ComponentItem::ComponentFunc(_)
                            if function == "invoke" => {
                            imports.func_wrap("invoke", |mut cx,
                                (owner, operand): (Resource<ProgramCell>, i64)| {
                                let cell = cx.data_mut().release(owner)?;
                                ensure!(!cx.data().task.cancelled()?, "Program task cancelled");
                                let installed = cx.data().installed.context("uninstalled source")?;
                                ensure!(cell.scope == cx.data().scope
                                    && cell.scope == installed.cell.scope
                                    && cell.address == installed.cell.address
                                    && cell.definition_identity == installed.cell.definition_identity
                                    && cell.capture_hash == installed.cell.capture_hash,
                                    "foreign or stale Program owner");
                                let begin = installed.instance.get_typed_func::<
                                    (i32,i32,i32,i32,i32,i32),i32>(&mut cx,"begin")?;
                                let budget = (32_768, 256, 32, 2048, 4096, 4096);
                                ensure!(begin.call(&mut cx, budget)? == 0,
                                    "source task budget admission refused");
                                let invoke = installed.instance.get_typed_func::<(i32,i64),i64>(
                                    &mut cx,"worker_invoke")?;
                                let result = invoke.call(&mut cx,
                                    (i32::try_from(cell.address)?,operand))?;
                                let metric = installed.instance.get_typed_func::<i32,i64>(
                                    &mut cx,"metric")?;
                                ensure!(metric.call(&mut cx,0)? == 0,
                                    "source Program task failed");
                                let host = cx.data_mut();
                                host.invokes = host.invokes.checked_add(1)
                                    .context("Program invocation count overflow")?;
                                Ok((result,))
                            })?;
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
}

#[derive(Debug)]
pub(super) enum GuestFailure {
    Exhausted,
    Invalid(anyhow::Error),
}

fn install_selected(engine: &Engine, source: &ProgramStore, root: Handle, task: TaskOwner)
    -> Result<Store<Host>> {
    let core = core_module(&engine, source.wat())?;
    let mut store = Store::new(&engine, Host {
        table: ResourceTable::new(), installed: None, scope: source.instance_id(), task,
        limits: StoreLimitsBuilder::new().memory_size(4_194_304)
            .table_elements(16_384).instances(8).tables(8).build(),
        admits: 0, invokes: 0, live_owners: 0,
    });
    store.limiter(|host| &mut host.limits);
    store.epoch_deadline_trap();
    store.set_epoch_deadline(1);
    store.set_fuel(FUEL)?;
    let instance = install_core(&mut store, &core)?;
    let begin = instance.get_typed_func::<(i32,i32,i32,i32,i32,i32),i32>(
        &mut store,"begin")?;
    ensure!(begin.call(&mut store,(32_768,256,32,2048,4096,4096))? == 0,
        "checked source installation budget refused");
    let install = instance.get_typed_func::<(),i32>(&mut store,"worker_install")?;
    ensure!(install.call(&mut store,())? == 0,"source Program install failed");
    let cell = bind_installed(&mut store, instance, source, root)?;
    store.data_mut().installed = Some(Installed { instance, cell });
    Ok(store)
}

/// Inspect actual runtime Text cells before accepting a portable capture
/// package. Neither a source literal nor a package row can attest itself.
pub(super) fn attest(source: &ProgramStore, root: Handle) -> Result<[u8;32]> {
    let mut config = Config::new();
    config.wasm_component_model(true).consume_fuel(true).epoch_interruption(true);
    let engine = Engine::new(&config)?;
    let store = install_selected(&engine, source, root, TaskOwner::new(source.instance_id()))?;
    Ok(store.data().installed.context("missing installed Program cell")?
        .cell.capture_hash)
}

/// The selected guest is always rebuilt from the WIT and Noble body retained
/// here; no package or caller-supplied component binary is installed.
pub(super) fn execute(source: &mut ProgramStore, root: Handle, operand: i64)
    -> std::result::Result<Observation, GuestFailure> {
    let task = TaskOwner::new(source.instance_id());
    execute_with_task(source, root, operand, &task)
}

pub(super) fn execute_with_task(source: &mut ProgramStore, root: Handle,
                                operand: i64, task: &TaskOwner)
    -> std::result::Result<Observation, GuestFailure> {
    source.reserve_guest().map_err(|_| GuestFailure::Exhausted)?;
    execute_reserved_with_task(source, root, operand, task).map_err(GuestFailure::Invalid)
}

fn execute_reserved_with_task(source: &ProgramStore, root: Handle,
                              operand: i64, task: &TaskOwner) -> Result<Observation> {
    ensure!(task.scope == source.instance_id(), "foreign Program task context");
    ensure!(!task.cancelled()?, "Program task cancelled before guest admission");
    let world = source.selected_world()
        .map_err(|_| anyhow::anyhow!("source Program WIT binding refused"))?;
    let checked = world.prepare_export("compute",
        b"programs.admit swap programs.invoke", super::worker_program::SOURCE_LIMITS)
        .map_err(|_| anyhow::anyhow!("selected guest source check refused"))?;
    let guest = noble_wasm::component::compile(&world, &[checked])
        .map_err(|_| anyhow::anyhow!("selected guest compile refused"))?;
    let component = crate::component::assemble_checked_world(&world, guest.wat())
        .map_err(|_| anyhow::anyhow!("selected guest assembly refused"))?;
    let mut config = Config::new();
    config.wasm_component_model(true).consume_fuel(true).epoch_interruption(true);
    let engine = Engine::new(&config)?;
    task.start(&engine)?;
    let result = (|| {
    let guest = Component::new(&engine, component)?;
    let mut linker = Linker::new(&engine);
    link(&engine, &guest, &mut linker)?;
    let mut store = install_selected(&engine, source, root, task.clone())?;
    store.set_fuel(FUEL)?;
    let guest = linker.instantiate(&mut store, &guest)?;
    let compute = guest.get_typed_func::<(i64,),(i64,)>(&mut store,"compute")?;
    let (value,) = compute.call(&mut store,(operand,))?;
    compute.post_return(&mut store)?;
    let host = store.data();
    ensure!(host.admits == 1 && host.invokes == 1 && host.live_owners == 0,
        "guest did not consume exactly its checked Program owner");
    task.complete(value)?;
    let value = task.recall(source.instance_id())?
        .context("Program completion retired before recall")?;
    Ok(Observation { value, admits: host.admits, invokes: host.invokes,
        live_owners: host.live_owners })
    })();
    if result.is_err() { task.retire(); }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use noble_kernel::types::Ty;

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
        assert!(!delivered.cancel(79).unwrap(),
            "late cancellation cannot revoke an already delivered owner");
    }

    #[test]
    fn selected_guest_executes_checked_program_and_consumes_own_resource() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment", &[Ty::I64], 70_000_000).unwrap();
        let observed = execute(&mut source, root, 41).unwrap();
        assert_eq!(observed.value, 42);
        assert_eq!((observed.admits, observed.invokes, observed.live_owners), (1,1,0));
        let observed = execute(&mut source, root, -2).unwrap();
        assert_eq!(observed.value, -1);
    }

    #[test]
    fn selected_guest_invokes_actual_bounded_composed_program_cell() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def double [ 2 * ] export plus def plus [ double 1 + ] ]",
            "plus", &[Ty::I64], 70_000_000).unwrap();
        let double = source.checked_root(0).unwrap();
        let adapted = source.adapt(root, &noble_kernel::types::EffSet::empty()).unwrap();
        let chosen = source.select(&[adapted, double], 1).unwrap();
        assert_eq!(execute(&mut source, chosen, 3).unwrap().value, 6);
        let composed = source.compose(root, double).unwrap();
        assert!(source.recipe_edges(composed).is_some());
        let observed = execute(&mut source, composed, 3).unwrap();
        assert_eq!(observed.value, 14);
        assert_eq!(observed.live_owners, 0);
    }

    #[test]
    fn type_level_widening_does_not_issue_a_host_effect_grant() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment", &[Ty::I64], 50_000_000).unwrap();
        let widened = source.adapt(root, &noble_kernel::types::EffSet::from_ids(
            &[noble_kernel::contracts::TEST_EMIT])).unwrap();
        assert!(matches!(execute(&mut source, widened, 41), Err(GuestFailure::Invalid(_))),
            "an adapted effect witness alone cannot enter the pure Worker guest");
    }

    #[test]
    fn equal_text_bytes_rebind_as_distinct_checked_lexical_atoms() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ \"same\" drop \"same\" drop 1 + ] export increment ]",
            "increment", &[Ty::I64], 50_000_000).unwrap();
        assert_eq!(source.text_aliases().len(), 2);
        assert_ne!(source.text_aliases()[0].node, source.text_aliases()[1].node);
        assert_eq!(execute(&mut source, root, 41).unwrap().value, 42);
    }

    #[test]
    fn checked_foreign_instance_handle_cannot_be_installed_in_guest() {
        let (first, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment", &[Ty::I64], 1_000_000).unwrap();
        let (mut other, _) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def decrement [ 1 - ] export decrement ]",
            "decrement", &[Ty::I64], 50_000_000).unwrap();
        assert!(first.named(root).is_some());
        assert!(execute(&mut other, root, 3).is_err(),
            "another checked module cannot receive a foreign Program owner");
    }

    #[test]
    fn cancelled_and_delivered_tasks_cannot_reenter_guest_admission() {
        let (mut source, root) = ProgramStore::prepare(
            b"module Arithmetic@1 [ def increment [ 1 + ] export increment ]",
            "increment", &[Ty::I64], 80_000_000).unwrap();
        let cancelled = TaskOwner::new(source.instance_id());
        assert!(cancelled.cancel(source.instance_id()).unwrap());
        assert!(execute_with_task(&mut source, root, 41, &cancelled).is_err());
        let delivered = TaskOwner::new(source.instance_id());
        assert_eq!(execute_with_task(&mut source, root, 41, &delivered).unwrap().value, 42);
        assert!(execute_with_task(&mut source, root, 7, &delivered).is_err(),
            "one delivered task must not mint a second Program owner");
    }
}
