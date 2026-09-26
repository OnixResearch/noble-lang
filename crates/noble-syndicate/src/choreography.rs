//! Bounded M8 projection and serialized host-owned component admission.
//!
//! A descriptor is inert data. The embedding host authenticates component
//! identity/signatures and each Store's token, calls `begin` before invoking a
//! guest export, routes every import through `import`, then calls `finish` after
//! call and post-return. The monitor mutex is never held across guest execution.
//! This is one local synchronous session, not transport, durability, fairness,
//! a general protocol interpreter, or a proof of compiler/engine correctness.

mod json;
mod monitor;
mod projection;

pub use monitor::{
    EventRecord, Import, ImportResult, InvocationOutcome, ParticipantToken, Session, Snapshot,
    WitBinding,
};
pub use projection::{project, Action, Plan};

pub const PROTOCOL: &str = "noble:choreography/service@1.0.0";
pub const WORLD: &str = "noble:syndicate/service@1.0.0";
pub const MAX_DESCRIPTOR_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    ByteLimit,
    Utf8,
    DepthLimit,
    Descriptor,
    ParserEnd,
    Protocol,
    Capacity,
    Nonempty,
    Binding,
    Denied,
    InFlight,
    ImportRefused,
    GuestFail,
    Invocation,
    Host(crate::Error),
}

impl From<crate::Error> for Error {
    fn from(error: crate::Error) -> Self {
        Self::Host(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Name {
    bytes: [u8; noble_kernel::dataspace::MAX_NAME_BYTES],
    len: u8,
}

impl Name {
    fn checked(text: &str) -> Option<Self> {
        if !noble_kernel::dataspace::valid_name(text) {
            return None;
        }
        let mut bytes = [0; noble_kernel::dataspace::MAX_NAME_BYTES];
        bytes[..text.len()].copy_from_slice(text.as_bytes());
        Some(Self {
            bytes,
            len: text.len() as u8,
        })
    }

    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.bytes[..usize::from(self.len)]).ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    Withdraw,
    Trap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Round {
    pub name: Name,
    pub ready: bool,
    pub exit: Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Publisher,
    Subscriber,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Export {
    Publisher,
    Observer,
    Withdraw,
    PublishAndTrap,
}

impl Export {
    pub fn wit_name(self) -> Option<&'static str> {
        if self == Self::Publisher {
            Some("publisher")
        } else if self == Self::Observer {
            Some("observer")
        } else if self == Self::Withdraw {
            Some("withdraw")
        } else if self == Self::PublishAndTrap {
            Some("publish-and-trap")
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expected {
    Returned(bool),
    TrapAfterPublication,
}
