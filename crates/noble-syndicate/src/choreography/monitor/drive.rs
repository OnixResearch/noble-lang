use super::*;

impl Session {
    /// Read one fixed-size typed notification without cloning an M7 String or
    /// holding either lock across caller code.
    pub fn event(&self, index: u32) -> Result<Option<EventRecord>, Error> {
        let inner = attempt!(self.lock());
        let table = attempt!(inner.profile.table_unchecked());
        let Some(event) = usize::try_from(index)
            .ok()
            .and_then(|at| table.events().get(at))
        else {
            return Ok(None);
        };
        let name = attempt!(Name::checked(&event.service.name).ok_or(Error::Descriptor));
        Ok(Some(EventRecord {
            observer: event.observer,
            name,
            ready: event.service.ready,
            change: event.change,
        }))
    }

    pub fn complete(&self) -> Result<bool, Error> {
        let inner = attempt!(self.lock());
        Ok(inner.cursor == inner.plan.global().len() && inner.pending.is_none())
    }

    /// Must be called by the trusted host before invoking any guest export.
    /// Denials do not install a pending call or touch guest/M7 counters.
    pub fn begin(
        &self,
        token: &ParticipantToken,
        export: Export,
        name: &str,
        ready: Option<bool>,
    ) -> Result<(), Error> {
        let mut inner = attempt!(self.lock());
        if inner.pending.is_some() {
            return Err(Error::InFlight);
        }
        if !inner.valid(token) {
            return Err(Error::Denied);
        }
        let Some(action) = inner.plan.global().get(inner.cursor) else {
            return Err(Error::Denied);
        };
        if token.role != action.role || export != action.export {
            return Err(Error::Denied);
        }
        if Some(name) != inner.plan.name(action.step) || ready != action.ready {
            return Err(Error::Denied);
        }
        let Some(round) = inner.plan.round(action.round) else {
            return Err(Error::Denied);
        };
        let is_present = {
            let table = attempt!(inner.profile.table_unchecked());
            table.contains_pair(name, round.ready)
        };
        let is_expected_present = if action.export == Export::Observer {
            matches!(action.result, Expected::Returned(true))
        } else if action.export == Export::Publisher || action.export == Export::PublishAndTrap {
            false
        } else if action.export == Export::Withdraw {
            true
        } else {
            return Err(Error::Denied);
        };
        if is_present != is_expected_present {
            return Err(Error::Denied);
        }
        inner.pending = Some(Pending {
            step: inner.cursor,
            phase: Phase::First,
        });
        Ok(())
    }

    /// The only guest-import path in an M8 session. Lock order is monitor,
    /// then dataspace, and check/commit/phase update are one critical section.
    pub fn import(
        &self,
        token: &ParticipantToken,
        operation: Import<'_>,
    ) -> Result<ImportResult, Error> {
        let mut inner = attempt!(self.lock());
        let Some(pending) = inner.pending.as_ref() else {
            return Err(Error::ImportRefused);
        };
        let action = inner.plan.global()[pending.step];
        let Some(name) = inner.plan.name(action.step) else {
            return Err(Error::ImportRefused);
        };
        let is_expected = match (action.export, pending.phase) {
            (Export::Observer, Phase::First) => {
                matches!(operation, Import::Observe { name: n, ready } if n == name && Some(ready) == action.ready)
            }
            (Export::Publisher | Export::PublishAndTrap, Phase::First) => {
                matches!(operation, Import::Publish { name: n, ready } if n == name && Some(ready) == action.ready)
            }
            (Export::Withdraw, Phase::First) => {
                matches!(operation, Import::Retract { name: n } if n == name)
            }
            (Export::PublishAndTrap, Phase::AwaitFail) => {
                matches!(operation, Import::Fail { value: true })
            }
            _ => false,
        };
        if !inner.valid(token) || token.role != action.role || !is_expected {
            inner.refused_requests += 1;
            attempt!(inner.set_phase(Phase::Refused));
            return Err(Error::ImportRefused);
        }
        if matches!(operation, Import::Fail { .. }) {
            inner.guest_requests += 1;
            attempt!(inner.set_phase(Phase::FailObserved));
            return Err(Error::GuestFail);
        }
        let participant = attempt!(inner.participant(token.role));
        let facet = participant.facet;
        let result: Result<bool, crate::Error> = {
            let mut table = attempt!(inner.profile.table_unchecked());
            if let Import::Publish { name, ready } = operation {
                noble_kernel::dataspace::encode_service(name, ready)
                    .map_err(crate::Error::from)
                    .and_then(|wire| {
                        table
                            .publish_wire(facet, wire.as_bytes())
                            .map_err(crate::Error::from)
                    })
            } else if let Import::Observe { name, ready } = operation {
                noble_kernel::dataspace::encode_service(name, ready)
                    .map_err(crate::Error::from)
                    .and_then(|wire| {
                        table
                            .observe_wire(facet, wire.as_bytes())
                            .map_err(crate::Error::from)
                    })
            } else if let Import::Retract { name } = operation {
                noble_kernel::dataspace::encode_service(name, false)
                    .map_err(crate::Error::from)
                    .and_then(|wire| {
                        table
                            .retract_wire(facet, wire.as_bytes())
                            .map_err(crate::Error::from)
                    })
            } else {
                return Err(Error::ImportRefused);
            }
        };
        inner.guest_requests += 1;
        match result {
            Ok(value) => {
                inner.protected_operations += 1;
                let phase = if action.export == Export::PublishAndTrap && value {
                    Phase::AwaitFail
                } else {
                    Phase::Complete(value)
                };
                attempt!(inner.set_phase(phase));
                Ok(ImportResult::Boolean(value))
            }
            Err(error) => {
                attempt!(inner.set_phase(Phase::Refused));
                Err(Error::Host(error))
            }
        }
    }

    /// Called after the actual component call and post_return, or its trap.
    /// Any mismatch retires the failing facet; a trap step advances only after
    /// successful publish, observed fail, failed invocation and owed removal.
    pub fn finish(
        &self,
        token: &ParticipantToken,
        outcome: InvocationOutcome,
    ) -> Result<(), Error> {
        let mut inner = attempt!(self.lock());
        let Some(pending) = inner.pending.take() else {
            return Err(Error::Invocation);
        };
        let action = inner.plan.global()[pending.step];
        let is_authorized = inner.valid(token) && token.role == action.role;
        let is_successful = is_authorized
            && match (action.result, pending.phase, outcome) {
                (
                    Expected::Returned(expected),
                    Phase::Complete(actual),
                    InvocationOutcome::Returned(result),
                ) => expected == actual && actual == result,
                (
                    Expected::TrapAfterPublication,
                    Phase::FailObserved,
                    InvocationOutcome::Trapped,
                ) => true,
                _ => false,
            };
        if matches!(outcome, InvocationOutcome::Trapped) || !is_successful {
            attempt!(inner.retire(action.role));
        }
        if is_successful {
            if action.export == Export::PublishAndTrap {
                let Some(trap_ready) = action.ready else {
                    return Err(Error::Invocation);
                };
                let table = attempt!(inner.profile.table_unchecked());
                let events = table.events();
                let is_removed = events.iter().any(|event| {
                    event.change == noble_kernel::dataspace::Change::Removed
                        && Some(event.service.name.as_str()) == inner.plan.name(action.step)
                        && event.service.ready == trap_ready
                });
                if !is_removed {
                    return Err(Error::Invocation);
                }
            }
            inner.cursor += 1;
            Ok(())
        } else {
            Err(Error::Invocation)
        }
    }
}
