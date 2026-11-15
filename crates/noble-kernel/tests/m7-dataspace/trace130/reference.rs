//! Test-only, set-based M7 reference. No production Table calls or decisions; only the witness harness imports it.
use std::collections::{BTreeMap, BTreeSet};

#[path = "reference/steps.rs"]
mod steps;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rights {
    Publish,
    Observe,
}

impl Rights {
    fn publish(self) -> bool {
        matches!(self, Self::Publish)
    }
    fn observe(self) -> bool {
        matches!(self, Self::Observe)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub facets: usize,
    pub assertions: usize,
    pub interests: usize,
    pub events: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open {
        parent: Option<u8>,
        rights: Rights,
    },
    Publish {
        who: u8,
        name: &'static str,
        ready: bool,
    },
    Observe {
        who: u8,
        name: &'static str,
        ready: bool,
    },
    Retract {
        who: u8,
        name: &'static str,
    },
    Retire {
        who: u8,
    },
    Drain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Bool(bool),
    Facet(u8),
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    InvalidFacet,
    InvalidName,
    Denied,
    Capacity,
}

pub type Result = std::result::Result<Decision, Refusal>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub observer: u8,
    pub name: &'static str,
    pub ready: bool,
    pub change: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub counts: (usize, usize, usize),
    pub slots: usize,
    pub pairs: Vec<(&'static str, bool)>,
    pub queue: Vec<Event>,
}

#[derive(Clone)]
struct Scope {
    parent: Option<u8>,
    rights: Rights,
    live: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    None,
    PrematureRemoval,
    DuplicateAddition,
    FalseAsAbsence,
    MissingDescendants,
    UnderReservedRemoval,
}

#[derive(Clone)]
pub struct Model {
    limits: Limits,
    scopes: Vec<Scope>,
    // Ownership and visibility are represented independently of the table's
    // parallel assertion vectors. Names are from the frozen finite alphabet.
    owned: BTreeMap<(u8, &'static str), bool>,
    // Only the retirement notification schedule is Noble-specific: a
    // replacement/withdrawal relocates the final occupied physical slot.
    order: Vec<(u8, &'static str)>,
    interests: Vec<(u8, &'static str, bool)>,
    events: Vec<Event>,
    fault: Fault,
    fault_fired: bool,
}

impl Model {
    pub fn new(limits: Limits, fault: Fault) -> Self {
        Self {
            limits,
            scopes: Vec::new(),
            owned: BTreeMap::new(),
            order: Vec::new(),
            interests: Vec::new(),
            events: Vec::new(),
            fault,
            fault_fired: false,
        }
    }

    pub fn fault_fired(&self) -> bool {
        self.fault_fired
    }

    pub fn observation(&self) -> Observation {
        let visible = self.visible();
        Observation {
            counts: (
                self.scopes.iter().filter(|s| s.live).count(),
                self.owned.len(),
                self.interests.len(),
            ),
            slots: self.limits.facets.saturating_sub(self.scopes.len()),
            pairs: visible.into_iter().collect(),
            queue: self.events.clone(),
        }
    }

    fn visible(&self) -> BTreeSet<(&'static str, bool)> {
        self.owned
            .iter()
            .map(|((_, name), ready)| (*name, *ready))
            .collect()
    }

    fn valid_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 32
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    }

    fn scope(&self, who: u8, needs_publish: bool) -> std::result::Result<(), Refusal> {
        let Some(scope) = self.scopes.get(usize::from(who).saturating_sub(1)) else {
            return Err(Refusal::InvalidFacet);
        };
        if who == 0 || !scope.live {
            return Err(Refusal::InvalidFacet);
        }
        let is_authorized = if needs_publish {
            scope.rights.publish()
        } else {
            scope.rights.observe()
        };
        if !is_authorized {
            return Err(Refusal::Denied);
        }
        Ok(())
    }

    fn descendants(&self, ancestor: u8) -> BTreeSet<u8> {
        let mut result = BTreeSet::new();
        result.insert(ancestor);
        for index in 0..self.scopes.len() {
            let child = u8::try_from(index.saturating_add(1)).expect("bounded facet IDs fit in u8");
            if self.scopes[index]
                .parent
                .is_some_and(|parent| result.contains(&parent))
            {
                result.insert(child);
            }
        }
        result
    }

    fn notify(&mut self, name: &'static str, ready: bool, change: &'static str) {
        for &(observer, interest_name, interest_ready) in &self.interests {
            if interest_name == name && interest_ready == ready {
                self.events.push(Event {
                    observer,
                    name,
                    ready,
                    change,
                });
            }
        }
    }

    fn owed_removals(&self) -> usize {
        let visible = self.visible();
        self.interests
            .iter()
            .filter(|(_, name, ready)| visible.contains(&(*name, *ready)))
            .count()
    }

    fn admitted(&mut self, candidate: Self) -> Result {
        let mut is_fired = candidate.fault_fired;
        let owed = if self.fault == Fault::UnderReservedRemoval {
            if candidate
                .events
                .len()
                .saturating_add(candidate.owed_removals())
                > self.limits.events
                && candidate.events.len() <= self.limits.events
            {
                is_fired = true;
            }
            0
        } else {
            candidate.owed_removals()
        };
        if candidate.events.len().saturating_add(owed) > self.limits.events {
            return Err(Refusal::Capacity);
        }
        *self = candidate;
        self.fault_fired |= is_fired;
        Ok(Decision::Done)
    }

    pub fn step(&mut self, action: Action) -> Result {
        self.step_inner(action)
    }
}
