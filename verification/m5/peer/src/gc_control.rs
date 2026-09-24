//! A bounded, single-threaded GC/retirement control, not an async profile.
//!
//! The engine owns only a descriptive guest handle. Native storage and its
//! captured callback scope live outside the GC heap and the Wasmtime Store.

use anyhow::{bail, ensure, Context, Result};
use noble_kernel::{resources as owner, types::ResourceKind};
use serde_json::{json, Value};
use std::{
    cell::Cell,
    future::{poll_fn, Future},
    pin::Pin,
    rc::{Rc, Weak},
    sync::{
        atomic::{AtomicUsize, Ordering::SeqCst},
        Arc,
    },
    task::{Context as TaskContext, Poll, Waker},
};
use wasmtime::{
    Collector, Config, Engine, ExternRef, Global, Instance, Module, RootScope, Rooted, Store,
    StoreLimits, StoreLimitsBuilder, Val,
};

const GUEST: &str = r#"(module
    (global $description (export "description") (mut externref) (ref.null extern))
    (func (export "hold") (param externref)
        local.get 0
        global.set $description)
    (func (export "visible") (result i32)
        global.get $description
        ref.is_null
        i32.eqz)
)"#;

// No Owner, Borrow, Scope, native allocation, or authority witness is in this
// payload. The counter observes engine finalization; it cannot release storage.
struct GuestDescription {
    handle: owner::Handle,
    finalizers: Arc<AtomicUsize>,
}

impl Drop for GuestDescription {
    fn drop(&mut self) {
        self.finalizers.fetch_add(1, SeqCst);
    }
}

struct NativeStorage {
    releases: Rc<Cell<usize>>,
    completions: Rc<Cell<usize>>,
    protected_value: Cell<usize>,
}

impl Drop for NativeStorage {
    fn drop(&mut self) {
        self.releases.set(self.releases.get() + 1);
    }
}

// This is the local native operation's retained pin, not a GC reference or a
// guest callback. Only begin's authentic, captured scope is delivered.
struct NativeCallback {
    scope: owner::Scope,
    pin: Option<Rc<NativeStorage>>,
    deliveries: usize,
}

impl NativeCallback {
    fn deliver(&mut self, host: &mut Host) -> Result<owner::Decision> {
        self.deliveries += 1;
        if let Some(storage) = self.pin.take() {
            storage.completions.set(storage.completions.get() + 1);
            // The last in-flight native access ends before notifying the table.
            drop(storage);
        }
        let completed = checked(host.table.complete(self.scope, owner::Completion::Success))?;
        host.returned_owners += usize::from(completed.owner.is_some());
        host.apply(completed.decision)?;
        ensure!(
            completed.owner.is_none(),
            "retired callback returned guest ownership"
        );
        Ok(completed.decision)
    }
}

struct Host {
    table: owner::Table,
    required: owner::Requirement,
    handle: owner::Handle,
    storage: Option<Rc<NativeStorage>>,
    storage_weak: Weak<NativeStorage>,
    native_releases: Rc<Cell<usize>>,
    native_completions: Rc<Cell<usize>>,
    accounting: owner::Accounting,
    returned_owners: usize,
    protected_attempts: usize,
    protected_operations: usize,
}

fn checked<T, E: std::fmt::Debug>(result: Result<T, E>) -> Result<T> {
    result.map_err(|error| anyhow::anyhow!("{error:?}"))
}

impl Host {
    fn register(id: u64) -> Result<(Self, owner::Owner)> {
        let required = owner::Requirement {
            context: owner::Context(1),
            kind: ResourceKind(1),
            rights: owner::Rights(1),
        };
        let mut table = checked(owner::Table::new(
            owner::TableId(id),
            owner::Limits {
                slots: 1,
                pins: 1,
                owners_per_context: 1,
                generations: 1,
                scopes: 1,
            },
        ))?;
        let native_releases = Rc::new(Cell::new(0));
        let native_completions = Rc::new(Cell::new(0));
        let storage = Rc::new(NativeStorage {
            releases: Rc::clone(&native_releases),
            completions: Rc::clone(&native_completions),
            protected_value: Cell::new(0),
        });
        let owned = checked(table.register(required))?;
        Ok((
            Self {
                table,
                required,
                handle: owned.handle(),
                storage_weak: Rc::downgrade(&storage),
                storage: Some(storage),
                native_releases,
                native_completions,
                accounting: owner::Accounting {
                    pins_acquired: 0,
                    pins_released: 0,
                    owners_returned: 0,
                    owners_transferred: 0,
                    local_releases: 0,
                },
                returned_owners: 0,
                protected_attempts: 0,
                protected_operations: 0,
            },
            owned,
        ))
    }

    fn apply(&mut self, decision: owner::Decision) -> Result<()> {
        let delta = decision.accounting();
        if delta.local_releases != 0 {
            ensure!(delta.local_releases == 1, "invalid local release delta");
            drop(
                self.storage
                    .take()
                    .context("duplicate native storage release")?,
            );
        }
        self.accounting.pins_acquired += delta.pins_acquired;
        self.accounting.pins_released += delta.pins_released;
        self.accounting.owners_returned += delta.owners_returned;
        self.accounting.owners_transferred += delta.owners_transferred;
        self.accounting.local_releases += delta.local_releases;
        Ok(())
    }

    fn attempt_protected_access(&mut self, borrow: &owner::Borrow) -> Result<()> {
        self.protected_attempts += 1;
        match self.table.native_access(borrow) {
            Err(owner::Error::Retiring | owner::Error::Retired) => Ok(()),
            Err(error) => bail!("unexpected access refusal: {error:?}"),
            Ok(_) => {
                // This body really is behind the production table check. A
                // failed revocation cannot produce a zero-work passing report.
                self.protected_operations += 1;
                let storage = self.storage.as_ref().context("released native storage")?;
                storage
                    .protected_value
                    .set(storage.protected_value.get() + 1);
                bail!("revoked native access admitted protected work")
            }
        }
    }

    fn observe(
        &self,
        step: &str,
        decision: Option<owner::Decision>,
        borrow: &owner::Borrow,
        callback: &NativeCallback,
        guest_reference_present: bool,
        gc_collections: usize,
        guest_finalizers: usize,
    ) -> Result<Value> {
        let record = self
            .table
            .snapshot(self.handle.slot)
            .context("missing retained resource")?;
        let table = self.table.observation();
        let guest = self.table.validate(self.handle, self.required);
        let native = self.table.native_access(borrow);
        let callback_pin = callback.pin.is_some();
        let storage_alive = self.storage_weak.strong_count() != 0;
        ensure!(
            table.native_pins == usize::from(callback_pin),
            "native pin accounting mismatch"
        );
        ensure!(
            self.accounting.local_releases == self.native_releases.get(),
            "local decision and native destructor disagree"
        );
        ensure!(
            self.accounting.owners_returned == self.returned_owners,
            "returned-owner accounting mismatch"
        );
        Ok(json!({
            "step": step, "action": decision.map(|decision| format!("{:?}", decision.action)),
            "final_state": state_name(record.state), "scope": record.last_scope,
            "retirement": record.retirement.map(|reason| format!("{reason:?}")),
            "guest_access": guest.is_ok(),
            "guest_access_error": guest.err().map(|error| format!("{error:?}")),
            "native_access": native.is_ok(),
            "native_access_error": native.err().map(|error| format!("{error:?}")),
            "guest_reference_present": guest_reference_present,
            "gc_collections": gc_collections, "guest_finalizers": guest_finalizers,
            "native_pins": table.native_pins, "callback_pin_retained": callback_pin,
            "callback_deliveries": callback.deliveries,
            "callback_storage_accesses": self.native_completions.get(),
            "native_storage_alive": storage_alive,
            "native_strong_references": self.storage_weak.strong_count(),
            "local_release_count": self.accounting.local_releases,
            "native_release_count": self.native_releases.get(),
            "returned_owner_count": self.returned_owners,
            "fully_cleaned_up": table.live == 0 && table.busy == 0 && table.retiring == 0
                && table.native_pins == 0 && !callback_pin && !storage_alive,
            "protected_access_attempts": self.protected_attempts,
            "protected_operations": self.protected_operations,
            "table": {"live": table.live, "busy": table.busy, "retiring": table.retiring,
                "retired": table.retired, "native_pins": table.native_pins},
            "accounting": {
                "pins_acquired": self.accounting.pins_acquired,
                "pins_released": self.accounting.pins_released,
                "owners_returned": self.accounting.owners_returned,
                "owners_transferred": self.accounting.owners_transferred,
                "local_releases": self.accounting.local_releases,
            },
        }))
    }
}

fn state_name(state: owner::State) -> String {
    match state {
        owner::State::Live => "Live".into(),
        owner::State::Busy(serial) => format!("Busy(call-{serial})"),
        owner::State::Retiring(serial) => format!("Retiring(call-{serial})"),
        owner::State::Retired => "Retired".into(),
    }
}

fn reference_present(store: &mut Store<StoreLimits>, description: Global) -> Result<bool> {
    let mut roots = RootScope::new(store);
    match description.get(&mut roots) {
        Val::ExternRef(reference) => Ok(reference.is_some()),
        _ => bail!("guest description is not an externref"),
    }
}

fn collect(
    store: &mut Store<StoreLimits>,
    finalizers: &AtomicUsize,
    collections: &mut usize,
    phase: &str,
) -> Value {
    let before = finalizers.load(SeqCst);
    store.gc(None);
    *collections += 1;
    let after = finalizers.load(SeqCst);
    json!({
        "phase": phase, "api": "Store::gc(None)", "collection": *collections,
        "finalizers_before": before, "finalizers_after": after,
        "finalized_during_collection": after - before,
        "observation_boundary": "before-store-drop",
    })
}

fn case_observation(step: &Value, stage: &str, outcome: &str) -> Value {
    json!({
        "stage": stage, "outcome": outcome,
        "final_state": step["final_state"], "guest_access": step["guest_access"],
        "native_pins": step["native_pins"], "fully_cleaned_up": step["fully_cleaned_up"],
        "returned_owner_count": step["returned_owner_count"],
        "local_release_count": step["local_release_count"],
        "native_release_count": step["native_release_count"],
        "guest_finalizers": step["guest_finalizers"],
        "gc_collections": step["gc_collections"],
        "protected_operations": step["protected_operations"],
    })
}

fn scenario(engine: &Engine, module: &Module, id: u64, reason: owner::Retirement) -> Result<Value> {
    let (mut host, owned) = Host::register(id)?;
    let admitted = checked(host.table.begin(owned, host.required))?;
    host.apply(admitted.decision)?;
    let borrow = admitted.borrow;
    let mut callback = NativeCallback {
        scope: borrow.scope(),
        pin: Some(Rc::clone(
            host.storage.as_ref().context("missing native storage")?,
        )),
        deliveries: 0,
    };
    // The guest has no linear memory; Wasmtime's GC heap needs one 64-KiB page.
    let limits = StoreLimitsBuilder::new()
        .memory_size(65_536)
        .instances(1)
        .tables(0)
        .build();
    let mut store = Store::new(engine, limits);
    store.limiter(|limits| limits);
    store.set_fuel(10_000)?;
    let instance = Instance::new(&mut store, module, &[])?;
    let hold = instance.get_typed_func::<Option<Rooted<ExternRef>>, ()>(&mut store, "hold")?;
    let visible = instance.get_typed_func::<(), i32>(&mut store, "visible")?;
    let description = instance
        .get_global(&mut store, "description")
        .context("missing guest global")?;
    let finalizers = Arc::new(AtomicUsize::new(0));
    let rooted = {
        let mut roots = RootScope::new(&mut store);
        let reference = ExternRef::new(
            &mut roots,
            GuestDescription {
                handle: host.handle,
                finalizers: Arc::clone(&finalizers),
            },
        )?;
        let data = reference
            .data(&roots)?
            .context("missing guest description")?;
        ensure!(
            data.downcast_ref::<GuestDescription>()
                .map(|data| data.handle)
                == Some(host.handle),
            "wrong guest handle description"
        );
        hold.call(&mut roots, Some(reference))?;
        // OwnedRooted is only a root, not the payload. Removing it below cannot
        // masquerade as finalization: its payload's counter must still be zero.
        reference.to_owned_rooted(&mut roots)?
    };
    let guest_visible = visible.call(&mut store, ())?;
    ensure!(guest_visible == 1, "guest did not receive the reference");
    let mut gc_collections = 0;
    let mut collections = vec![collect(
        &mut store,
        &finalizers,
        &mut gc_collections,
        "guest-reachable",
    )];
    ensure!(
        finalizers.load(SeqCst) == 0,
        "reachable guest reference was finalized"
    );
    let mut steps = vec![host.observe(
        "busy",
        Some(admitted.decision),
        &borrow,
        &callback,
        reference_present(&mut store, description)?,
        gc_collections,
        finalizers.load(SeqCst),
    )?];

    let mut adapter_polls = 0;
    let (revoked, adapter) = match reason {
        owner::Retirement::Cancelled => {
            // Invocation cleanup is driven by the host, not a guest continuation.
            let decisions = checked(host.table.retire_context(host.required.context, reason))?;
            ensure!(
                decisions.len() == 1,
                "unexpected invocation cleanup cardinality"
            );
            (decisions[0], Value::Null)
        }
        owner::Retirement::UnexpectedSuspension => {
            // A deliberately defective declared-sync native adapter attempts to
            // wait for its held callback. Poll it once, refuse Pending, and do
            // not install an executor or enable Wasmtime async support.
            let poll = {
                let mut native = poll_fn(|_| {
                    adapter_polls += 1;
                    if callback.pin.is_some() {
                        Poll::Pending
                    } else {
                        Poll::Ready(())
                    }
                });
                Pin::new(&mut native).poll(&mut TaskContext::from_waker(Waker::noop()))
            };
            ensure!(
                poll.is_pending(),
                "control did not attempt unexpected suspension"
            );
            let decision = checked(host.table.revoke(callback.scope, reason))?;
            (
                decision,
                json!({
                    "declared": "sync", "native_poll": format!("{poll:?}"),
                    "polls": adapter_polls, "suspension_refused": poll.is_pending(),
                    "boundary": "single-poll-local-native-adapter",
                }),
            )
        }
        _ => bail!("unsupported resource retirement control"),
    };
    host.apply(revoked)?;
    // Neither clearing the guest root nor cleanup runs guest code.
    description.set(&mut store, Val::ExternRef(None))?;
    drop(rooted);
    ensure!(
        finalizers.load(SeqCst) == 0,
        "finalization occurred before the explicit collection"
    );
    host.attempt_protected_access(&borrow)?;
    steps.push(host.observe(
        "revoked",
        Some(revoked),
        &borrow,
        &callback,
        reference_present(&mut store, description)?,
        gc_collections,
        finalizers.load(SeqCst),
    )?);
    ensure!(
        steps[1]["final_state"] == "Retiring(call-1)"
            && steps[1]["native_pins"] == 1
            && steps[1]["local_release_count"] == 0,
        "revocation failed to preserve pending native retirement"
    );

    collections.push(collect(
        &mut store,
        &finalizers,
        &mut gc_collections,
        "after-revocation",
    ));
    ensure!(
        finalizers.load(SeqCst) == 1,
        "explicit engine collection did not finalize the guest reference"
    );
    host.attempt_protected_access(&borrow)?;
    steps.push(host.observe(
        "after-gc",
        None,
        &borrow,
        &callback,
        reference_present(&mut store, description)?,
        gc_collections,
        finalizers.load(SeqCst),
    )?);
    ensure!(
        steps[2]["final_state"] == "Retiring(call-1)"
            && steps[2]["native_pins"] == 1
            && steps[2]["native_storage_alive"] == true
            && steps[2]["local_release_count"] == 0,
        "guest GC discharged a native obligation"
    );

    for step in ["callback", "duplicate-callback"] {
        let completed = callback.deliver(&mut host)?;
        host.attempt_protected_access(&borrow)?;
        steps.push(host.observe(
            step,
            Some(completed),
            &borrow,
            &callback,
            reference_present(&mut store, description)?,
            gc_collections,
            finalizers.load(SeqCst),
        )?);
    }
    let repeated = checked(
        host.table
            .retire(host.handle, owner::Retirement::HostCleanup),
    )?;
    host.apply(repeated)?;
    host.attempt_protected_access(&borrow)?;
    steps.push(host.observe(
        "retire-again",
        Some(repeated),
        &borrow,
        &callback,
        reference_present(&mut store, description)?,
        gc_collections,
        finalizers.load(SeqCst),
    )?);
    ensure!(
        host.table.observation().retired == 1
            && host.table.observation().native_pins == 0
            && host.accounting.local_releases == 1
            && host.native_releases.get() == 1
            && host.returned_owners == 0
            && host.native_completions.get() == 1
            && host.protected_operations == 0,
        "retirement did not complete exactly once"
    );
    let observations = match reason {
        owner::Retirement::Cancelled => json!({
            "RA-CASE-05": case_observation(&steps[2], "cleanup", "retirement-pending"),
            "RA-CASE-06": case_observation(&steps[5], "cleanup", "retired"),
        }),
        owner::Retirement::UnexpectedSuspension => json!({
            "RA-CASE-09": case_observation(&steps[1], "adapter", "binding-defect"),
        }),
        _ => unreachable!(),
    };
    Ok(json!({
        "observations": observations, "adapter": adapter, "steps": steps,
        "collections": collections, "gc_collections": gc_collections,
        "guest_finalizers": finalizers.load(SeqCst),
        "guest_visible_before_revocation": guest_visible,
        "protected_operations": host.protected_operations,
        "remaining_fuel": store.get_fuel()?,
        "observation_boundary": "before-store-drop",
    }))
}

pub fn execute(command: &str) -> Result<Value> {
    ensure!(
        command == "cancellation-gc",
        "unknown resource-control command: {command}"
    );
    let mut config = Config::new();
    config
        .collector(Collector::DeferredReferenceCounting)
        .wasm_reference_types(true)
        .consume_fuel(true);
    let engine = Engine::new(&config)?;
    let module = Module::new(&engine, GUEST)?;
    let cancellation = scenario(&engine, &module, 1, owner::Retirement::Cancelled)?;
    let suspension = scenario(&engine, &module, 2, owner::Retirement::UnexpectedSuspension)?;
    Ok(json!({
        "schema": "noble-m5-resource-control/v1", "control": command,
        "engine": "wasmtime-40.0.2", "collector": "deferred-reference-counting",
        "profile": "Component-Sync-Bootstrap",
        "execution": "bounded-single-threaded-local-control",
        "guest_payload": "nonauthority-handle-description",
        "cancellation": cancellation, "unexpected_suspension": suspension,
    }))
}
