use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    pub(super) step: u32,
    pub(super) round: u32,
    pub(super) role: Role,
    pub(super) export: Export,
    pub(super) ready: Option<bool>,
    pub(super) result: Expected,
}

impl Action {
    pub const fn step(&self) -> u32 {
        self.step
    }
    pub const fn round(&self) -> u32 {
        self.round
    }
    pub const fn role(&self) -> Role {
        self.role
    }
    pub const fn export(&self) -> Export {
        self.export
    }
    pub const fn ready(&self) -> Option<bool> {
        self.ready
    }
    pub const fn result(&self) -> Expected {
        self.result
    }
    const EMPTY: Self = Self {
        step: 0,
        round: 0,
        role: Role::Subscriber,
        export: Export::Observer,
        ready: None,
        result: Expected::Returned(false),
    };
}

#[derive(Debug)]
pub struct Plan {
    rounds: [Option<Round>; 2],
    round_count: u32,
    actions: [Action; 10],
    len: usize,
    publisher: [u32; 4],
    publishers: usize,
    subscriber: [u32; 6],
    subscribers: usize,
}

impl Plan {
    pub fn global(&self) -> &[Action] {
        &self.actions[..self.len]
    }
    pub fn publisher_steps(&self) -> &[u32] {
        &self.publisher[..self.publishers]
    }
    pub fn subscriber_steps(&self) -> &[u32] {
        &self.subscriber[..self.subscribers]
    }
    pub fn round_count(&self) -> u32 {
        self.round_count
    }
    pub fn round(&self, ordinal: u32) -> Option<&Round> {
        let Ok(at) = usize::try_from(ordinal) else {
            return None;
        };
        self.rounds.get(at).and_then(Option::as_ref)
    }
    pub fn name(&self, step: u32) -> Option<&str> {
        let Ok(at) = usize::try_from(step) else {
            return None;
        };
        self.global()
            .get(at)
            .and_then(|action| self.round(action.round))
            .and_then(|round| round.name.as_str())
    }
    const fn push(
        &mut self,
        round: u32,
        role: Role,
        export: Export,
        ready: Option<bool>,
        result: Expected,
    ) -> Result<(), Error> {
        let step = self.len as u32;
        self.actions[self.len] = Action {
            step,
            round,
            role,
            export,
            ready,
            result,
        };
        self.len += 1;
        if let Role::Publisher = role {
            self.publisher[self.publishers] = step;
            self.publishers += 1;
        } else if let Role::Subscriber = role {
            self.subscriber[self.subscribers] = step;
            self.subscribers += 1;
        } else {
            return Err(Error::Descriptor);
        }
        Ok(())
    }

    fn checked_round(&self, ordinal: u32) -> Result<(bool, Exit), Error> {
        let Some(round) = self.round(ordinal) else {
            return Err(Error::Descriptor);
        };
        if matches!(ordinal, 1)
            && self
                .round(0)
                .is_some_and(|first| first.name.eq(&round.name))
        {
            return Err(Error::Descriptor);
        }
        if matches!(round.exit, Exit::Trap) && ordinal.saturating_add(1).ne(&self.round_count) {
            return Err(Error::Descriptor);
        }
        Ok((round.ready, round.exit))
    }

    fn append_withdraw(&mut self, ordinal: u32, is_ready: bool) -> Result<(), Error> {
        attempt!(self.push(
            ordinal,
            Role::Publisher,
            Export::Publisher,
            Some(is_ready),
            Expected::Returned(true)
        ));
        attempt!(self.push(
            ordinal,
            Role::Subscriber,
            Export::Observer,
            Some(is_ready),
            Expected::Returned(true)
        ));
        self.push(
            ordinal,
            Role::Publisher,
            Export::Withdraw,
            None,
            Expected::Returned(true),
        )
    }

    fn append_round(&mut self, ordinal: u32, is_ready: bool, exit: Exit) -> Result<(), Error> {
        attempt!(self.push(
            ordinal,
            Role::Subscriber,
            Export::Observer,
            Some(is_ready),
            Expected::Returned(false)
        ));
        if matches!(exit, Exit::Withdraw) {
            attempt!(self.append_withdraw(ordinal, is_ready));
        } else if matches!(exit, Exit::Trap) {
            attempt!(self.push(
                ordinal,
                Role::Publisher,
                Export::PublishAndTrap,
                Some(is_ready),
                Expected::TrapAfterPublication
            ));
        } else {
            return Err(Error::Descriptor);
        }
        self.push(
            ordinal,
            Role::Subscriber,
            Export::Observer,
            Some(is_ready),
            Expected::Returned(false),
        )
    }

    const fn from_descriptor(descriptor: json::Parsed) -> Self {
        Self {
            rounds: descriptor.rounds,
            round_count: descriptor.len as u32,
            actions: [Action::EMPTY; 10],
            len: 0,
            publisher: [0; 4],
            publishers: 0,
            subscriber: [0; 6],
            subscribers: 0,
        }
    }
}

/// The sole untrusted descriptor ingress. The fixed-capacity decoder validates
/// UTF-8, JSON syntax, decoded exact keys and complete input before projection.
pub fn project(bytes: &[u8]) -> Result<Plan, Error> {
    let descriptor = attempt!(json::parse(bytes));
    let mut plan = Plan::from_descriptor(descriptor);
    let mut ordinal = 0;
    while ordinal < plan.round_count {
        let (is_ready, exit) = attempt!(plan.checked_round(ordinal));
        attempt!(plan.append_round(ordinal, is_ready, exit));
        ordinal += 1;
    }
    Ok(plan)
}
