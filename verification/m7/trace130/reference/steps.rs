use super::{Action, Decision, Fault, Model, Refusal, Result, Scope};

impl Model {
    pub(super) fn step_inner(&mut self, action: Action) -> Result {
        match action {
            Action::Open { parent, rights } => {
                if self.scopes.len() >= self.limits.facets {
                    return Err(Refusal::Capacity);
                }
                if parent.is_some_and(|id| {
                    id == 0
                        || !self
                            .scopes
                            .get(usize::from(id - 1))
                            .is_some_and(|scope| scope.live)
                }) {
                    return Err(Refusal::InvalidFacet);
                }
                let id = u8::try_from(self.scopes.len() + 1).expect("selected facet limit fits u8");
                self.scopes.push(Scope {
                    parent,
                    rights,
                    live: true,
                });
                Ok(Decision::Facet(id))
            }
            Action::Publish { who, name, ready } => {
                self.scope(who, true)?;
                if !Self::valid_name(name) {
                    return Err(Refusal::InvalidName);
                }
                let prior = self.owned.get(&(who, name)).copied();
                if prior == Some(ready) {
                    return Ok(Decision::Bool(true));
                }
                if prior.is_none() && self.owned.len() == self.limits.assertions {
                    return Err(Refusal::Capacity);
                }
                let was_visible = self.visible();
                let mut next = self.clone();
                if prior.is_some() {
                    let index = next
                        .order
                        .iter()
                        .position(|key| *key == (who, name))
                        .expect("owned key has order");
                    next.order.swap_remove(index);
                }
                next.order.push((who, name));
                next.owned.insert((who, name), ready);
                let now_visible = next.visible();
                if let Some(previous) = prior {
                    if !now_visible.contains(&(name, previous)) {
                        next.notify(name, previous, "Removed");
                    } else if self.fault == Fault::PrematureRemoval {
                        next.notify(name, previous, "Removed");
                        next.fault_fired = true;
                    }
                }
                if !was_visible.contains(&(name, ready)) {
                    next.notify(name, ready, "Added");
                } else if self.fault == Fault::DuplicateAddition && prior != Some(ready) {
                    next.notify(name, ready, "Added");
                    next.fault_fired = true;
                }
                self.admitted(next)?;
                Ok(Decision::Bool(true))
            }
            Action::Observe { who, name, ready } => {
                self.scope(who, false)?;
                if !Self::valid_name(name) {
                    return Err(Refusal::InvalidName);
                }
                let is_present = self.visible().contains(&(name, ready));
                let is_false_fault = self.fault == Fault::FalseAsAbsence && !ready && is_present;
                if self.interests.contains(&(who, name, ready)) {
                    self.fault_fired |= is_false_fault;
                    return Ok(Decision::Bool(is_present && !is_false_fault));
                }
                if self.interests.len() == self.limits.interests {
                    return Err(Refusal::Capacity);
                }
                let mut next = self.clone();
                next.interests.push((who, name, ready));
                if is_present {
                    next.notify_one(who, name, ready);
                }
                next.fault_fired |= is_false_fault;
                self.admitted(next)?;
                Ok(Decision::Bool(is_present && !is_false_fault))
            }
            Action::Retract { who, name } => {
                self.scope(who, true)?;
                if !Self::valid_name(name) {
                    return Err(Refusal::InvalidName);
                }
                let Some(prior) = self.owned.get(&(who, name)).copied() else {
                    return Ok(Decision::Bool(false));
                };
                let mut next = self.clone();
                next.owned.remove(&(who, name));
                let index = next
                    .order
                    .iter()
                    .position(|key| *key == (who, name))
                    .expect("owned key has order");
                next.order.swap_remove(index);
                if !next.visible().contains(&(name, prior)) {
                    next.notify(name, prior, "Removed");
                } else if self.fault == Fault::PrematureRemoval {
                    next.notify(name, prior, "Removed");
                    next.fault_fired = true;
                }
                self.admitted(next)?;
                Ok(Decision::Bool(true))
            }
            Action::Retire { who } => {
                if who == 0
                    || !self
                        .scopes
                        .get(usize::from(who - 1))
                        .is_some_and(|s| s.live)
                {
                    return Err(Refusal::InvalidFacet);
                }
                let mut retiring = self.descendants(who);
                let mut next = self.clone();
                if self.fault == Fault::MissingDescendants && retiring.len() > 1 {
                    retiring.retain(|id| *id == who);
                    next.fault_fired = true;
                }
                next.interests.retain(|(id, _, _)| !retiring.contains(id));
                let mut index = 0;
                while index < next.order.len() {
                    let (owner, name) = next.order[index];
                    if retiring.contains(&owner) {
                        next.order.swap_remove(index);
                        let ready = next
                            .owned
                            .remove(&(owner, name))
                            .expect("ordered owner has assertion");
                        if !next.visible().contains(&(name, ready)) {
                            next.notify(name, ready, "Removed");
                        }
                    } else {
                        index += 1;
                    }
                }
                next.events
                    .retain(|event| !retiring.contains(&event.observer));
                for id in retiring {
                    next.scopes[usize::from(id - 1)].live = false;
                }
                // Cleanup cannot be refused: successful admission reserved its debt.
                *self = next;
                Ok(Decision::Done)
            }
            Action::Drain => {
                self.events.clear();
                Ok(Decision::Done)
            }
        }
    }

    fn notify_one(&mut self, observer: u8, name: &'static str, ready: bool) {
        self.events.push(super::Event {
            observer,
            name,
            ready,
            change: "Added",
        });
    }
}
