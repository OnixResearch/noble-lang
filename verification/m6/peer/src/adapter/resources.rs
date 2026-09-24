use super::{CONTEXT, Callback, Host, Payload, SLOT_LIMIT, Slot, buffer, checked, reserved_buffer};
use anyhow::{Context, Result, bail, ensure};
use noble_kernel::{async_tasks as task, resources as owner, types::ResourceKind};
use serde_json::Value;

pub(super) struct NativePayload {
    pub(super) value: i64,
}

// Neither owning token nor native payload is Copy/Clone. The only move back to
// guest storage is a successful production task delivery and owner transfer.
pub(super) struct ResourcePayload {
    pub(super) owner: owner::Owner,
    pub(super) native: Box<NativePayload>,
}

fn required(context: owner::Context) -> owner::Requirement {
    owner::Requirement {
        context,
        kind: ResourceKind(1),
        rights: owner::Rights(1),
    }
}

impl Host {
    pub fn create_resource(&mut self, value: i64) -> Result<u32> {
        let index = self
            .guest_resources
            .iter()
            .position(Option::is_none)
            .context("resource slot capacity")?;
        let rep = self.next_rep;
        let next = rep
            .checked_add(1)
            .context("resource representation exhausted")?;
        let owner = checked(self.owners.register(required(CONTEXT)))?;
        let handle = owner.handle();
        self.guest_resources[index] = Some((
            rep,
            ResourcePayload {
                owner,
                native: Box::new(NativePayload { value }),
            },
        ));
        self.next_rep = next;
        self.native_created += 1;
        self.event(serde_json::json!({"operation":"resource-created","rep":rep,"value":value,
            "table":handle.table.0,"slot":handle.slot,"generation":handle.generation,"context":handle.context.0}));
        Ok(rep)
    }

    pub fn resource_value(&self, rep: u32) -> Result<i64> {
        let resource = &self
            .guest_resources
            .iter()
            .flatten()
            .find(|(key, _)| *key == rep)
            .context("unknown resource representation")?
            .1;
        checked(
            self.owners
                .validate(resource.owner.handle(), required(CONTEXT)),
        )?;
        Ok(resource.native.value)
    }

    pub(crate) fn resource_handle(&self, rep: u32) -> Result<owner::Handle> {
        Ok(self
            .guest_resources
            .iter()
            .flatten()
            .find(|(key, _)| *key == rep)
            .context("unknown resource representation")?
            .1
            .owner
            .handle())
    }

    pub(crate) fn validate_resource(
        &self,
        handle: owner::Handle,
        context: owner::Context,
    ) -> Result<()> {
        checked(self.owners.validate(handle, required(context)))?;
        Ok(())
    }

    pub(crate) fn admit_result_owner(&mut self, value: i64) -> Result<Callback> {
        let mut request = self.request(0, 8);
        request.results = 1;
        let preparation = checked(self.tasks.prepare(request))?;
        let result = reserved_buffer(request.result_bytes)?;
        let context = owner::Context(
            request
                .native
                .0
                .checked_add(100)
                .context("resource context exhausted")?,
        );
        // The native producer owns its acquired object while running. It enters
        // task result custody only when its authenticated completion is accepted.
        let owner = checked(self.owners.register(required(context)))?;
        let produced = ResourcePayload {
            owner,
            native: Box::new(NativePayload { value }),
        };
        let admitted = preparation.commit(Payload {
            input: Vec::new(),
            result,
            parked: Vec::new(),
            resource: None,
            produced: Some(produced),
            return_owner: false,
            argument: 0,
            protected: None,
            native_finished: false,
        });
        let callback = admitted.callback;
        self.next_native = self
            .next_native
            .checked_add(1)
            .context("native identity exhausted")?;
        self.native_created += 1;
        self.slots[callback.task.slot] = Some(Slot {
            callback,
            token: Some(admitted.task),
            label: "result-owner".into(),
            payload: admitted.inputs,
        });
        self.record("result-owner-admit", admitted.decision);
        Ok(callback)
    }

    pub fn admit_resource(
        &mut self,
        rep: u32,
        argument: i64,
        return_owner: bool,
    ) -> Result<Callback> {
        let index = self
            .guest_resources
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|(key, _)| *key == rep))
            .context("unknown resource representation")?;
        let resource = &self.guest_resources[index].as_ref().unwrap().1;
        checked(
            self.owners
                .validate(resource.owner.handle(), required(CONTEXT)),
        )?;
        // Compute the native result before authority consumption without changing
        // it. Overflow is a preflight error, never a post-commit owner restore.
        if return_owner {
            resource
                .native
                .value
                .checked_add(argument)
                .context("resource arithmetic overflow")?;
        }
        let mut request = self.request(8, if return_owner { 8 } else { 64 });
        request.inputs = 1;
        let preparation = checked(self.tasks.prepare(request))?;
        let input = buffer(request.input_bytes)?;
        let result = reserved_buffer(request.result_bytes)?;
        let execution = self.protection.admit(argument, Some(return_owner))?;
        let (_, resource) = self.guest_resources[index].take().unwrap();
        let admitted = preparation.commit(Payload {
            input,
            result,
            parked: Vec::new(),
            resource: Some(resource),
            produced: None,
            return_owner,
            argument,
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
            label: if return_owner {
                "resource-transfer"
            } else {
                "resource-error"
            }
            .into(),
            payload: admitted.inputs,
        });
        self.record("resource-admit", admitted.decision);
        // Task commit already consumed custody. A failure from this point keeps
        // the original token with that task for retirement; it is never restored.
        let receiver = owner::Context(
            callback
                .native
                .0
                .checked_add(100)
                .context("resource context exhausted")?,
        );
        let resource = self.slots[callback.task.slot]
            .as_mut()
            .unwrap()
            .payload
            .resource
            .take()
            .unwrap();
        let native = resource.native;
        let transferred =
            match self
                .owners
                .transfer(vec![resource.owner], &[required(CONTEXT)], receiver)
            {
                Ok(transferred) => transferred,
                Err(rejected) => {
                    let owner = rejected
                        .input
                        .into_iter()
                        .next()
                        .context("lost rejected owner")?;
                    self.slots[callback.task.slot]
                        .as_mut()
                        .unwrap()
                        .payload
                        .resource = Some(ResourcePayload { owner, native });
                    let decision =
                        checked(self.tasks.retire(callback.task, task::Failure::Internal))?;
                    self.record("resource-transfer-failed", decision);
                    self.protection.fail()?;
                    // No native operation was started. The retained pin can now be
                    // settled authentically without restoring caller ownership.
                    self.slots[callback.task.slot]
                        .as_mut()
                        .unwrap()
                        .payload
                        .native_finished = true;
                    self.settle(callback)?;
                    bail!(
                        "resource transfer after task admission: {:?}",
                        rejected.error
                    );
                }
            };
        self.resource_transfers += transferred
            .decisions
            .iter()
            .map(|decision| decision.accounting().owners_transferred)
            .sum::<usize>();
        let owner = transferred
            .owners
            .into_iter()
            .next()
            .context("missing transferred owner")?;
        let started = checked(self.protection.authority.start_execution(execution))?;
        let payload = &mut self.slots[callback.task.slot].as_mut().unwrap().payload;
        payload.resource = Some(ResourcePayload { owner, native });
        payload.protected = Some(started);
        Ok(callback)
    }

    pub fn deliver_resource(&mut self, callback: Callback) -> Result<u32> {
        let snapshot = self.authenticated(callback)?;
        ensure!(snapshot.state == task::State::Ready, "{:?}", snapshot.state);
        ensure!(
            snapshot.returned_inputs == 1 || snapshot.results == 1,
            "result contains no resource owner"
        );
        let guest_index = self
            .guest_resources
            .iter()
            .position(Option::is_none)
            .context("resource slot capacity")?;
        let rep = self.next_rep;
        let next = rep
            .checked_add(1)
            .context("resource representation exhausted")?;
        let payload = &mut self.slots[callback.task.slot].as_mut().unwrap().payload;
        let resource = if snapshot.returned_inputs == 1 {
            payload.resource.take()
        } else {
            payload.produced.take()
        }
        .context("resource result storage missing")?;
        let required = required(resource.owner.handle().context);
        let native = resource.native;
        let transferred = match self
            .owners
            .transfer(vec![resource.owner], &[required], CONTEXT)
        {
            Ok(transferred) => transferred,
            Err(rejected) => {
                // A failed return transfer leaves task delivery uncommitted.
                // The exact opaque owner/native payload remain in task custody.
                let owner = rejected
                    .input
                    .into_iter()
                    .next()
                    .context("lost rejected result owner")?;
                let payload = &mut self.slots[callback.task.slot].as_mut().unwrap().payload;
                if snapshot.returned_inputs == 1 {
                    payload.resource = Some(ResourcePayload { owner, native });
                } else {
                    payload.produced = Some(ResourcePayload { owner, native });
                }
                bail!("resource return transfer failed: {:?}", rejected.error);
            }
        };
        self.resource_transfers += transferred
            .decisions
            .iter()
            .map(|decision| decision.accounting().owners_transferred)
            .sum::<usize>();
        let owner = transferred
            .owners
            .into_iter()
            .next()
            .context("missing returned owner")?;
        // Hold the receiver-bound owner privately until task delivery commits.
        // No caller representation can access it between these local steps.
        let payload = &mut self.slots[callback.task.slot].as_mut().unwrap().payload;
        if snapshot.returned_inputs == 1 {
            payload.resource = Some(ResourcePayload { owner, native });
        } else {
            payload.produced = Some(ResourcePayload { owner, native });
        }
        self.deliver_token(callback)?;
        let payload = &mut self.slots[callback.task.slot].as_mut().unwrap().payload;
        let resource = if snapshot.returned_inputs == 1 {
            payload.resource.take()
        } else {
            payload.produced.take()
        }
        .context("committed resource result missing")?;
        let ResourcePayload { owner, native } = resource;
        self.returned_values.push(native.value);
        self.guest_resources[guest_index] = Some((rep, ResourcePayload { owner, native }));
        self.next_rep = next;
        self.cleanup_if_stopped(callback)?;
        Ok(rep)
    }

    pub fn drop_resource(&mut self, rep: u32) -> Result<()> {
        let index = self
            .guest_resources
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|(key, _)| *key == rep))
            .context("unknown resource destructor")?;
        let (_, resource) = self.guest_resources[index].take().unwrap();
        if let Err((resource, error)) = self.release_payload(resource) {
            self.guest_resources[index] = Some((rep, resource));
            bail!("{error:?}");
        }
        Ok(())
    }

    /// Store teardown does not invoke imported resource destructors. Once guest
    /// access has ended, retire its remaining owners through the same kernel
    /// boundary; task-owned and native-pinned resources retain their own path.
    pub fn retire_guest_resources(&mut self) -> Result<()> {
        for index in 0..self.guest_resources.len() {
            if let Some((rep, _)) = &self.guest_resources[index] {
                self.drop_resource(*rep)?;
            }
        }
        Ok(())
    }

    pub(super) fn release_payload(
        &mut self,
        resource: ResourcePayload,
    ) -> std::result::Result<(), (ResourcePayload, owner::Error)> {
        let required = required(resource.owner.handle().context);
        let decision = match self.owners.release(resource.owner, required) {
            Ok(decision) => decision,
            Err(rejected) => {
                return Err((
                    ResourcePayload {
                        owner: rejected.input,
                        native: resource.native,
                    },
                    rejected.error,
                ));
            }
        };
        self.resource_local_releases += decision.accounting().local_releases;
        let value = resource.native.value;
        drop(resource.native);
        self.native_releases += 1;
        if self.released_values.len() < SLOT_LIMIT * 16 {
            self.released_values.push(value);
        }
        self.event(
            serde_json::json!({"operation":"resource-native-release","value":value,
            "action":format!("{:?}",decision.action),"table":decision.record.handle.table.0,
            "slot":decision.record.handle.slot,"generation":decision.record.handle.generation,
            "local_releases":decision.accounting().local_releases}),
        );
        Ok(())
    }

    pub(crate) fn resource_snapshot(&self, rep: u32) -> Result<Value> {
        let handle = self.resource_handle(rep)?;
        let snapshot = checked(self.owners.validate(handle, required(CONTEXT)))?;
        Ok(
            serde_json::json!({"table":handle.table.0,"slot":handle.slot,"generation":handle.generation,
            "context":handle.context.0,"state":format!("{:?}",snapshot.state),"value":self.resource_value(rep)?}),
        )
    }
}
