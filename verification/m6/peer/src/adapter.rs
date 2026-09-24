//! Reference shell storage. The driver retains `Arc<Mutex<Host>>` independently
//! of its invocation Store; dropping a Store is never a native-stop notification.
mod protection;
mod resources;

use anyhow::{Context, Result, anyhow, bail, ensure};
use noble_kernel::{async_tasks as task, authority as auth, resources as owner};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

use protection::Protection;
use resources::ResourcePayload;
pub use task::Callback;

pub(crate) const CONTEXT: owner::Context = owner::Context(1);
const SLOT_LIMIT: usize = 8;
const TRACE_LIMIT: usize = 4096;
const LIVE_EXTERNAL_BYTES: usize = 128;
static NEXT_NAMESPACE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn checked<T, E: std::fmt::Debug>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|error| anyhow!("{error:?}"))
}

pub(crate) fn default_limits() -> task::Limits {
    task::Limits {
        tasks: SLOT_LIMIT,
        terminal_results: SLOT_LIMIT,
        bytes: 4096,
        parked_payloads: SLOT_LIMIT,
        pins: SLOT_LIMIT,
        wakeups: 64,
        retirement_work: 128,
        generations: 128,
    }
}

struct Payload {
    input: Vec<u8>,
    result: Vec<u8>,
    parked: Vec<u8>,
    resource: Option<ResourcePayload>,
    produced: Option<ResourcePayload>,
    return_owner: bool,
    argument: i64,
    protected: Option<auth::Started>,
    // This is local native-operation evidence, not another task lifecycle.
    // Only the actual producer callback sets it; cancellation never does.
    native_finished: bool,
}

struct Slot {
    callback: Callback,
    token: Option<task::Task>,
    label: String,
    payload: Payload,
}

#[derive(Default)]
struct Accounting {
    admissions: usize,
    completions: usize,
    deliveries: usize,
    cancellations: usize,
    duplicates: usize,
    pins_acquired: usize,
    pins_released: usize,
    acquired_inputs: usize,
    acquired_results: usize,
    acquired_buffers: usize,
    consumed_inputs: usize,
    retirement_withdrawn_inputs: usize,
    retired_inputs: usize,
    retired_results: usize,
    retired_buffers: usize,
    cleaned_inputs: usize,
    cleaned_results: usize,
    cleaned_buffers: usize,
    delivered_inputs: usize,
    delivered_results: usize,
    delivered_buffers: usize,
    rejected_result_bytes: usize,
    reservations_released: usize,
    wake_queued: usize,
    wake_removed: usize,
}

pub struct Host {
    case: String,
    pub(crate) tasks: task::Table,
    slots: Vec<Option<Slot>>,
    owners: owner::Table,
    guest_resources: Vec<Option<(u32, ResourcePayload)>>,
    protection: Protection,
    next_native: u64,
    next_rep: u32,
    accounting: Accounting,
    events: Vec<Value>,
    trace_dropped: usize,
    native_releases: usize,
    native_created: usize,
    released_values: Vec<i64>,
    returned_values: Vec<i64>,
    resource_transfers: usize,
    resource_local_releases: usize,
}

impl Host {
    pub fn new(case: &str) -> Result<Self> {
        Self::with_limits(case, default_limits())
    }

    pub(crate) fn with_limits(case: &str, limits: task::Limits) -> Result<Self> {
        let namespace = NEXT_NAMESPACE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current < u64::MAX / 2).then(|| current + 1)
            })
            .map_err(|_| anyhow::anyhow!("host namespace exhausted"))?;
        let tasks = checked(task::Table::new(task::TableId(namespace * 2), limits))?;
        let owners = checked(owner::Table::new(
            owner::TableId(namespace * 2 + 1),
            owner::Limits {
                slots: SLOT_LIMIT,
                pins: SLOT_LIMIT,
                owners_per_context: SLOT_LIMIT,
                generations: 4096,
                scopes: 4096,
            },
        ))?;
        Ok(Self {
            case: case.to_owned(),
            tasks,
            slots: (0..limits.tasks).map(|_| None).collect(),
            owners,
            guest_resources: (0..SLOT_LIMIT).map(|_| None).collect(),
            protection: Protection::new(case, namespace)?,
            next_native: 1,
            next_rep: 1,
            accounting: Accounting::default(),
            events: Vec::with_capacity(TRACE_LIMIT),
            trace_dropped: 0,
            native_releases: 0,
            native_created: 0,
            released_values: Vec::with_capacity(SLOT_LIMIT),
            returned_values: Vec::with_capacity(SLOT_LIMIT),
            resource_transfers: 0,
            resource_local_releases: 0,
        })
    }

    fn request(&self, input_bytes: usize, result_bytes: usize) -> task::Request {
        task::Request {
            context: CONTEXT,
            native: task::NativeId(self.next_native),
            inputs: 0,
            results: 0,
            input_bytes,
            result_bytes,
            parked_bytes: 0,
            pins: 1,
            wakeups: 4,
        }
    }

    pub fn admit(
        &mut self,
        label: &str,
        input_bytes: usize,
        result_bytes: usize,
        protected_argument: Option<i64>,
    ) -> Result<Callback> {
        let mut request = self.request(input_bytes, result_bytes);
        if matches!(label, "make-future" | "make-stream") {
            request.result_bytes = request.result_bytes.max(128);
            // Includes both retained host payload and the independently retained
            // native worker/channel buffers in this fixed adapter profile.
            request.parked_bytes = 256;
        }
        self.admit_request(label, request, protected_argument)
    }

    pub(crate) fn admit_request(
        &mut self,
        label: &str,
        request: task::Request,
        protected_argument: Option<i64>,
    ) -> Result<Callback> {
        ensure!(
            request.inputs == 0 && request.results == 0,
            "scalar request contains owners"
        );
        let preparation = checked(self.tasks.prepare(request))?;
        // Reserve the task and all actual storage before touching the witness.
        let input = buffer(request.input_bytes)?;
        let result = reserved_buffer(request.result_bytes)?;
        let local_parked_bytes = if matches!(label, "make-future" | "make-stream") {
            request
                .parked_bytes
                .checked_sub(LIVE_EXTERNAL_BYTES)
                .context("missing native live-value buffer reservation")?
        } else {
            request.parked_bytes
        };
        let parked = buffer(local_parked_bytes)?;
        let execution = match protected_argument {
            Some(argument) => Some(self.protection.admit(argument, None)?),
            None => None,
        };
        let admitted = preparation.commit(Payload {
            input,
            result,
            parked,
            resource: None,
            produced: None,
            return_owner: false,
            argument: protected_argument.unwrap_or(0),
            protected: None,
            native_finished: false,
        });
        let callback = admitted.callback;
        self.next_native = self
            .next_native
            .checked_add(1)
            .context("native identity exhausted")?;
        self.slots[callback.task.slot] = Some(Slot {
            callback,
            token: Some(admitted.task),
            label: label.to_owned(),
            payload: admitted.inputs,
        });
        self.record(label, admitted.decision);
        if let Some(execution) = execution {
            let started = checked(self.protection.authority.start_execution(execution))?;
            let payload = &mut self.slots[callback.task.slot].as_mut().unwrap().payload;
            payload.protected = Some(started);
        }
        Ok(callback)
    }

    pub fn complete(&mut self, callback: Callback, domain_error: bool, bytes: usize) -> Result<()> {
        let snapshot = self.authenticated(callback)?;
        let completion = if let Some(slot) = self.matching_slot(callback) {
            task::Completion {
                inputs: task::Disposition {
                    returned: u64::from(
                        slot.payload.resource.is_some() && slot.payload.return_owner,
                    ),
                    consumed: 0,
                    retired: u64::from(
                        slot.payload.resource.is_some() && !slot.payload.return_owner,
                    ),
                },
                produced: u64::from(slot.payload.produced.is_some()),
                bytes,
            }
        } else {
            // A finalized callback is still checked by the production table.
            snapshot.completion.unwrap_or(task::Completion {
                inputs: task::Disposition {
                    returned: 0,
                    consumed: 0,
                    retired: 0,
                },
                produced: 0,
                bytes,
            })
        };
        let outcome = if domain_error {
            task::Outcome::DomainError
        } else {
            task::Outcome::Success
        };
        let decision = match self.tasks.complete(callback, outcome, completion) {
            Ok(decision) => decision,
            Err(error) => {
                self.reject("complete", error);
                return checked(Err(error));
            }
        };
        let accepted = decision.accounting.completion_accepted;
        self.record("complete", decision);
        if !accepted {
            return Ok(());
        }
        let slot = self.slots[callback.task.slot]
            .as_mut()
            .context("missing native task storage")?;
        // A local worker executes its one operation exactly once. These are the
        // observed payload writes, not claimed external effects or cancellation.
        if let Some(resource) = slot.payload.resource.as_mut() {
            if !domain_error {
                resource.native.value = resource
                    .native
                    .value
                    .checked_add(slot.payload.argument)
                    .context("resource arithmetic overflow")?;
            }
        }
        if bytes <= snapshot.reservation.result_bytes {
            slot.payload
                .result
                .resize(bytes, if domain_error { 0xff } else { 0x2a });
        }
        if let Some(started) = slot.payload.protected.take() {
            self.protection
                .observe(started, slot.payload.argument, domain_error)?;
        }
        slot.payload.native_finished = true;
        Ok(())
    }

    pub fn cancel(&mut self, callback: Callback) -> Result<()> {
        self.cancel_with_scope(callback, true)
    }

    pub fn cancel_task(&mut self, callback: Callback) -> Result<()> {
        self.cancel_with_scope(callback, false)
    }

    fn cancel_with_scope(&mut self, callback: Callback, invocation: bool) -> Result<()> {
        self.authenticated(callback)?;
        let token = self
            .slots
            .get_mut(callback.task.slot)
            .and_then(Option::as_mut)
            .filter(|slot| slot.callback == callback)
            .and_then(|slot| slot.token.take());
        let decision = if let Some(token) = token {
            match self.tasks.cancel(token, CONTEXT) {
                Ok(decision) => decision,
                Err(rejected) => {
                    self.slots[callback.task.slot].as_mut().unwrap().token = Some(rejected.input);
                    self.reject("cancel", rejected.error);
                    return checked(Err(rejected.error));
                }
            }
        } else {
            checked(self.tasks.retire(callback.task, task::Failure::Cancelled))?
        };
        if invocation && decision.action == task::Action::CancellationAcknowledged {
            self.protection.cancel()?;
        }
        self.record("cancel", decision);
        // Ready cancellation may arrive after the producer settled its pins.
        self.cleanup_if_stopped(callback)
    }

    pub fn fail_all(&mut self, failure: task::Failure) -> Result<()> {
        if failure == task::Failure::Cancelled {
            self.protection.cancel()?;
        } else {
            self.protection.fail()?;
        }
        let callbacks: Vec<_> = self
            .slots
            .iter()
            .flatten()
            .map(|slot| slot.callback)
            .collect();
        for callback in callbacks {
            let decision = checked(self.tasks.retire(callback.task, failure))?;
            if matches!(
                decision.record.state,
                task::State::Retiring | task::State::Retired
            ) {
                self.slots[callback.task.slot]
                    .as_mut()
                    .unwrap()
                    .token
                    .take();
            }
            self.record("fail-all", decision);
            self.cleanup_if_stopped(callback)?;
        }
        Ok(())
    }

    pub fn deliver(&mut self, callback: Callback) -> Result<()> {
        let snapshot = self.authenticated(callback)?;
        ensure!(
            snapshot.returned_inputs == 0 && snapshot.results == 0,
            "owner-bearing result requires deliver_resource"
        );
        self.deliver_token(callback)?;
        self.cleanup_if_stopped(callback)
    }

    fn deliver_token(&mut self, callback: Callback) -> Result<task::Decision> {
        let snapshot = self.authenticated(callback)?;
        let slot = self.slots[callback.task.slot]
            .as_mut()
            .context("missing task storage")?;
        let token = match slot.token.take() {
            Some(token) => token,
            None => bail!("{:?}", snapshot.state),
        };
        let decision = match self.tasks.deliver(token, CONTEXT) {
            Ok(decision) => decision,
            Err(rejected) => {
                self.slots[callback.task.slot].as_mut().unwrap().token = Some(rejected.input);
                self.reject("deliver", rejected.error);
                return checked(Err(rejected.error));
            }
        };
        // The result vector leaves task custody at this delivery boundary.
        self.slots[callback.task.slot]
            .as_mut()
            .unwrap()
            .payload
            .result = Vec::new();
        self.record("deliver", decision);
        Ok(decision)
    }

    /// Trusted native producer notification. The driver calls this only after
    /// the actual worker ended its accesses. A completed callback alone leaves
    /// the production native pin charged until this separate operation.
    pub fn settle(&mut self, callback: Callback) -> Result<()> {
        let snapshot = self.authenticated(callback)?;
        if snapshot.finalized {
            return Ok(());
        }
        let slot = self
            .matching_slot(callback)
            .context("missing native task storage")?;
        ensure!(slot.payload.native_finished, "NativeStillRunning");
        if !snapshot.native_stopped {
            let decision = checked(self.tasks.native_stopped(callback))?;
            self.record("native-stopped", decision);
        }
        let snapshot = self.authenticated(callback)?;
        if snapshot.pins != 0 {
            let decision = checked(self.tasks.settle_pins(callback, snapshot.pins))?;
            self.record("settle-pins", decision);
        }
        self.cleanup_if_stopped(callback)
    }

    fn cleanup_if_stopped(&mut self, callback: Callback) -> Result<()> {
        let snapshot = self.authenticated(callback)?;
        if !snapshot.native_stopped || snapshot.pins != 0 || snapshot.finalized {
            return Ok(());
        }
        let cleanup = match snapshot.state {
            task::State::Ready => task::Obligations {
                // Producer/consumer frames can remain parked after the blocking
                // worker stops. Retain their reservation until terminal delivery.
                inputs: snapshot.retiring_inputs,
                results: 0,
                buffers: snapshot.buffers & 1,
            },
            task::State::Delivered | task::State::Retiring => task::Obligations {
                inputs: snapshot.retiring_inputs,
                results: snapshot.results,
                buffers: snapshot.buffers,
            },
            task::State::Pending | task::State::Retired => return Ok(()),
        };
        // Real local storage is released before acknowledging completed cleanup.
        if cleanup.inputs != 0 {
            if let Some(resource) = self.slots[callback.task.slot]
                .as_mut()
                .unwrap()
                .payload
                .resource
                .take()
            {
                if let Err((resource, error)) = self.release_payload(resource) {
                    self.slots[callback.task.slot]
                        .as_mut()
                        .unwrap()
                        .payload
                        .resource = Some(resource);
                    bail!("{error:?}");
                }
            }
        }
        if cleanup.results != 0 {
            if let Some(resource) = self.slots[callback.task.slot]
                .as_mut()
                .unwrap()
                .payload
                .produced
                .take()
            {
                if let Err((resource, error)) = self.release_payload(resource) {
                    self.slots[callback.task.slot]
                        .as_mut()
                        .unwrap()
                        .payload
                        .produced = Some(resource);
                    bail!("{error:?}");
                }
            }
        }
        let payload = &mut self.slots[callback.task.slot].as_mut().unwrap().payload;
        if cleanup.buffers & 1 != 0 {
            payload.input = Vec::new();
        }
        if cleanup.buffers & 2 != 0 {
            payload.result = Vec::new();
        }
        if cleanup.buffers & 4 != 0 {
            payload.parked = Vec::new();
        }
        if cleanup != task::Obligations::empty() {
            let decision = checked(self.tasks.cleanup(callback.task, cleanup))?;
            self.record("cleanup", decision);
        }
        if matches!(
            snapshot.state,
            task::State::Delivered | task::State::Retiring
        ) {
            // A refused or empty result never acquires the result-buffer bit,
            // but admission still reserved its backing. Release it before the
            // final reservation acknowledgement, after native access has ended.
            let payload = &mut self.slots[callback.task.slot].as_mut().unwrap().payload;
            payload.input = Vec::new();
            payload.result = Vec::new();
            payload.parked = Vec::new();
            let decision = checked(self.tasks.finish(callback.task))?;
            self.record("finish", decision);
        }
        Ok(())
    }

    fn matching_slot(&self, callback: Callback) -> Option<&Slot> {
        self.slots
            .get(callback.task.slot)
            .and_then(Option::as_ref)
            .filter(|slot| slot.callback == callback)
    }

    pub(crate) fn authenticated(&self, callback: Callback) -> Result<task::Snapshot> {
        let snapshot = checked(self.tasks.inspect(callback.task, CONTEXT))?;
        ensure!(snapshot.native == callback.native, "WrongNative");
        Ok(snapshot)
    }

    pub fn snapshot(&self, callback: Callback) -> Result<Value> {
        Ok(snapshot_json(self.authenticated(callback)?))
    }

    pub fn state(&self, callback: Callback) -> Result<task::State> {
        Ok(self.authenticated(callback)?.state)
    }

    pub fn terminal_outcome(&self, callback: Callback) -> Result<task::Outcome> {
        self.authenticated(callback)?
            .outcome
            .context("native operation has no terminal outcome")
    }

    pub fn native_pins(&self) -> usize {
        self.tasks.observation().outstanding_pins
    }

    /// Successful component return is distinct from the protected import's
    /// delivery. Without a protected operation no operation receipt is invented.
    pub fn finish_invocation(&mut self) -> Result<()> {
        self.protection.finish_invocation()
    }

    /// Called only after the driver joined native stream workers and observed
    /// reader closure. The shell consumes its terminal producer result; this
    /// is not a delivery of bytes to a closed guest reader or invocation cancel.
    pub fn close_streams(&mut self) -> Result<()> {
        let callbacks: Vec<_> = self
            .slots
            .iter()
            .flatten()
            .filter(|slot| slot.label == "make-stream")
            .map(|slot| slot.callback)
            .collect();
        for callback in callbacks {
            let snapshot = self.authenticated(callback)?;
            ensure!(
                snapshot.native_stopped && snapshot.pins == 0,
                "stream native worker still retained"
            );
            if snapshot.state == task::State::Ready {
                self.event(
                    json!({"operation":"reader-closed","native":callback.native.0,
                    "guest_bytes_delivered":0,"terminal_consumer":"host"}),
                );
                self.deliver(callback)?;
            }
        }
        Ok(())
    }

    pub(crate) fn set_authority_quota(&mut self, quota: u64) {
        self.protection.set_quota(quota);
    }
    pub(crate) fn set_authority_revoked(&mut self, revoked: bool) {
        self.protection.set_revoked(revoked);
    }

    pub(crate) fn record(&mut self, operation: &str, decision: task::Decision) {
        let delta = decision.accounting;
        let a = &mut self.accounting;
        a.admissions += usize::from(decision.action == task::Action::Admitted);
        a.completions += usize::from(delta.completion_accepted);
        a.deliveries += usize::from(decision.action == task::Action::ResultDelivered);
        a.cancellations += usize::from(decision.action == task::Action::CancellationAcknowledged);
        a.duplicates += usize::from(decision.action == task::Action::Duplicate);
        a.pins_acquired += delta.pins_acquired.count_ones() as usize;
        a.pins_released += delta.pins_released.count_ones() as usize;
        a.acquired_inputs += delta.acquired.inputs.count_ones() as usize;
        a.acquired_results += delta.acquired.results.count_ones() as usize;
        a.acquired_buffers += delta.acquired.buffers.count_ones() as usize;
        a.consumed_inputs += delta.consumed_inputs.count_ones() as usize;
        a.retirement_withdrawn_inputs += delta.retirement_withdrawn_inputs.count_ones() as usize;
        a.retired_inputs += delta.retired.inputs.count_ones() as usize;
        a.retired_results += delta.retired.results.count_ones() as usize;
        a.retired_buffers += delta.retired.buffers.count_ones() as usize;
        a.cleaned_inputs += delta.cleaned.inputs.count_ones() as usize;
        a.cleaned_results += delta.cleaned.results.count_ones() as usize;
        a.cleaned_buffers += delta.cleaned.buffers.count_ones() as usize;
        a.delivered_inputs += delta.delivered.inputs.count_ones() as usize;
        a.delivered_results += delta.delivered.results.count_ones() as usize;
        a.delivered_buffers += delta.delivered.buffers.count_ones() as usize;
        a.rejected_result_bytes += delta.rejected_result_bytes;
        a.reservations_released += usize::from(delta.reservation_released);
        a.wake_queued += usize::from(delta.wake_queued);
        a.wake_removed += usize::from(delta.wake_removed);
        self.event(
            json!({"operation":operation,"action":format!("{:?}",decision.action),
            "snapshot":snapshot_json(decision.record), "accounting":accounting_json(delta)}),
        );
    }

    pub(crate) fn reject(&mut self, operation: &str, error: task::Error) {
        self.event(json!({"operation":operation,"error":format!("{error:?}")}));
    }

    fn event(&mut self, event: Value) {
        if self.events.len() < TRACE_LIMIT {
            self.events.push(event);
        } else {
            self.trace_dropped += 1;
        }
    }

    pub fn report(&self) -> Value {
        let observation = self.tasks.observation();
        let reserved = observation.reserved;
        let resources = self.owners.observation();
        let a = &self.accounting;
        let task_owners = self
            .slots
            .iter()
            .flatten()
            .map(|slot| {
                usize::from(slot.payload.resource.is_some())
                    + usize::from(slot.payload.produced.is_some())
            })
            .sum::<usize>();
        let guest_owners = self.guest_resources.iter().flatten().count();
        let storage_bytes = self
            .slots
            .iter()
            .flatten()
            .map(|slot| {
                slot.payload.input.len() + slot.payload.result.len() + slot.payload.parked.len()
            })
            .sum::<usize>();
        let storage_capacity_bytes = self
            .slots
            .iter()
            .flatten()
            .map(|slot| {
                slot.payload.input.capacity()
                    + slot.payload.result.capacity()
                    + slot.payload.parked.capacity()
            })
            .sum::<usize>();
        json!({
            "case":self.case,
            "tasks":{"pending":observation.pending,"ready":observation.ready,"delivered":observation.delivered,
                "retiring":observation.retiring,"retired":observation.retired,
                "outstanding_pins":observation.outstanding_pins,"queued_wakeups":observation.queued_wakeups,
                "reserved":{"tasks":reserved.tasks,"terminal_results":reserved.terminal_results,"bytes":reserved.bytes,
                    "parked_payloads":reserved.parked_payloads,"pins":reserved.pins,"wakeups":reserved.wakeups,
                    "retirement_work":reserved.retirement_work}},
            "resources":{"live":resources.live,"busy":resources.busy,"retiring":resources.retiring,
                "retired":resources.retired,"native_pins":resources.native_pins,"guest_owners":guest_owners,
                "task_owners":task_owners,"native_payloads":guest_owners+task_owners,
                "native_created":self.native_created,"native_releases":self.native_releases,
                "released_values":self.released_values,"returned_values":self.returned_values,
                "transfers":self.resource_transfers,"local_releases":self.resource_local_releases},
            "authority":self.protection.report(), "storage_bytes":storage_bytes,
            "storage_capacity_bytes":storage_capacity_bytes,
            "accounting":{"admissions":a.admissions,"completions":a.completions,"deliveries":a.deliveries,
                "cancellations":a.cancellations,"duplicates":a.duplicates,"pins_acquired":a.pins_acquired,
                "pins_released":a.pins_released,"acquired_inputs":a.acquired_inputs,
                "acquired_results":a.acquired_results,"acquired_buffers":a.acquired_buffers,
                "consumed_inputs":a.consumed_inputs,"retirement_withdrawn_inputs":a.retirement_withdrawn_inputs,
                "retired_inputs":a.retired_inputs,"retired_results":a.retired_results,"retired_buffers":a.retired_buffers,
                "cleaned_inputs":a.cleaned_inputs,"cleaned_results":a.cleaned_results,"cleaned_buffers":a.cleaned_buffers,
                "delivered_inputs":a.delivered_inputs,"delivered_results":a.delivered_results,
                "delivered_buffers":a.delivered_buffers,"rejected_result_bytes":a.rejected_result_bytes,
                "reservations_released":a.reservations_released,"wake_queued":a.wake_queued,"wake_removed":a.wake_removed},
            "slots":self.slots.iter().flatten().map(|slot| json!({"label":slot.label,
                "snapshot":self.tasks.snapshot(slot.callback.task.slot).map(snapshot_json),
                "native_finished":slot.payload.native_finished,"task_token":slot.token.is_some()})).collect::<Vec<_>>(),
            "events":self.events,"trace_dropped":self.trace_dropped,
        })
    }
}

fn accounting_json(delta: task::Accounting) -> Value {
    let obligations = |value: task::Obligations| {
        json!({
            "inputs":value.inputs,"results":value.results,"buffers":value.buffers,
        })
    };
    json!({"acquired":obligations(delta.acquired),"consumed_inputs":delta.consumed_inputs,
        "retirement_withdrawn_inputs":delta.retirement_withdrawn_inputs,
        "retired":obligations(delta.retired),"delivered":obligations(delta.delivered),
        "cleaned":obligations(delta.cleaned),"pins_acquired":delta.pins_acquired,
        "pins_released":delta.pins_released,"rejected_result_bytes":delta.rejected_result_bytes,
        "completion_accepted":delta.completion_accepted,"wake_queued":delta.wake_queued,
        "wake_removed":delta.wake_removed,"reservation_released":delta.reservation_released})
}

pub(crate) fn snapshot_json(snapshot: task::Snapshot) -> Value {
    json!({"table":snapshot.handle.table.0,"slot":snapshot.handle.slot,"generation":snapshot.handle.generation,
        "context":snapshot.handle.context.0,"native":snapshot.native.0,"state":format!("{:?}",snapshot.state),
        "outcome":snapshot.outcome.map(|value|format!("{value:?}")),
        "failure":snapshot.failure.map(|value|format!("{value:?}")),
        "completion_closed":snapshot.completion_closed,"native_stopped":snapshot.native_stopped,
        "pins":snapshot.pins,"returned_inputs":snapshot.returned_inputs,"retiring_inputs":snapshot.retiring_inputs,
        "results":snapshot.results,"buffers":snapshot.buffers,"wake_pending":snapshot.wake_pending,
        "wakeups_remaining":snapshot.wakeups_remaining,"finalized":snapshot.finalized})
}

fn reserved_buffer(bytes: usize) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(bytes)
        .context("native payload reservation failed")?;
    Ok(buffer)
}

fn buffer(bytes: usize) -> Result<Vec<u8>> {
    let mut buffer = reserved_buffer(bytes)?;
    buffer.resize(bytes, 0);
    Ok(buffer)
}
