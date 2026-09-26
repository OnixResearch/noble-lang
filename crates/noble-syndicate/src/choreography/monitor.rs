use super::*;

static NEXT_SESSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Authenticated host-provided component binding. These are exact signatures,
/// not descriptor claims; the embedding linker must validate actual components.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WitBinding<'a> {
    pub world: &'a str,
    pub imports: [&'a str; 4],
    pub exports: [&'a str; 4],
}

impl WitBinding<'static> {
    pub const SERVICE: Self = Self {
        world: WORLD,
        imports: [
            "publish(string,bool)->bool",
            "observe(string,bool)->bool",
            "retract(string)->bool",
            "fail(bool)->bool",
        ],
        exports: [
            "publisher(string,bool)->bool",
            "observer(string,bool)->bool",
            "withdraw(string)->bool",
            "publish-and-trap(string,bool)->bool",
        ],
    };
}

#[derive(Clone)]
pub struct ParticipantToken {
    authority: u64,
    role: Role,
    facet: noble_kernel::dataspace::Facet,
}

impl ParticipantToken {
    pub const fn role(&self) -> Role {
        self.role
    }
    pub const fn facet(&self) -> noble_kernel::dataspace::Facet {
        self.facet
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Import<'a> {
    Publish { name: &'a str, ready: bool },
    Observe { name: &'a str, ready: bool },
    Retract { name: &'a str },
    Fail { value: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportResult {
    Boolean(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvocationOutcome {
    /// Guest returned, post_return completed, and yielded this typed result.
    Returned(bool),
    /// Actual component invocation failed, including post_return failure.
    Trapped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    First,
    AwaitFail,
    Complete(bool),
    FailObserved,
    Refused,
}

struct Pending {
    step: usize,
    phase: Phase,
}

struct Inner {
    profile: crate::Profile,
    publisher: crate::Participant,
    subscriber: crate::Participant,
    plan: Plan,
    authority: u64,
    cursor: usize,
    pending: Option<Pending>,
    guest_requests: usize,
    refused_requests: usize,
    protected_operations: usize,
}

impl Inner {
    fn participant(&self, role: Role) -> Result<&crate::Participant, Error> {
        if let Role::Publisher = role {
            Ok(&self.publisher)
        } else if let Role::Subscriber = role {
            Ok(&self.subscriber)
        } else {
            Err(Error::Denied)
        }
    }
    fn valid(&self, token: &ParticipantToken) -> bool {
        let Ok(participant) = self.participant(token.role) else {
            return false;
        };
        participant.live
            && self.authority.eq(&token.authority)
            && participant.facet.eq(&token.facet)
    }
    const fn set_phase(&mut self, phase: Phase) -> Result<(), Error> {
        let Some(pending) = self.pending.as_mut() else {
            return Err(Error::ImportRefused);
        };
        pending.phase = phase;
        Ok(())
    }
    fn retire(&mut self, role: Role) -> Result<(), Error> {
        let participant = if let Role::Publisher = role {
            &mut self.publisher
        } else if let Role::Subscriber = role {
            &mut self.subscriber
        } else {
            return Err(Error::Denied);
        };
        if participant.live {
            let mut table = attempt!(self.profile.table_unchecked());
            attempt!(table.retire(participant.facet).map_err(crate::Error::from));
            participant.live = false;
        }
        Ok(())
    }
    fn cleanup(&mut self) {
        // Cleanup, unlike a guest operation, must still retire after lock poisoning.
        let mut table = self
            .profile
            .table
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.publisher.live && table.retire(self.publisher.facet).is_ok() {
            self.publisher.live = false;
        }
        if self.subscriber.live && table.retire(self.subscriber.facet).is_ok() {
            self.subscriber.live = false;
        }
        table.choreography_active = false;
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Exactly one exclusive, host-serialized session per empty M7 dataspace.
pub struct Session {
    inner: std::sync::Mutex<Inner>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub cursor: usize,
    pub pending: bool,
    pub guest_requests: usize,
    pub refused_requests: usize,
    pub protected_operations: usize,
    pub counts: (u32, u32, u32),
    pub events_len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventRecord {
    pub observer: noble_kernel::dataspace::Facet,
    pub name: Name,
    pub ready: bool,
    pub change: noble_kernel::dataspace::Change,
}

mod admit;
mod drive;
