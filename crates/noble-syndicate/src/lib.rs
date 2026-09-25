//! Selected local M7 service host policy, not a general Syndicate runtime.
//!
//! The WIT component engine is a separately pinned trusted peer. The only
//! shared state here is host-owned, serialized dataspace policy; guests have
//! separate memories. A Preserves service value is inert data, never a facet,
//! ownership token, resource, or grant. All imports use this same adapter.

#![feature(register_tool)]
#![register_tool(tigerstyle)]

// Fallible forwarding is an owned compiler-resolved expansion, unlike the
// unsupported QuestionMark desugaring in the architecture collector.
macro_rules! attempt {
    ($step:expr) => {
        match $step {
            Ok(value) => value,
            Err(error) => return Err(error.into()),
        }
    };
}

type Shared = std::sync::Arc<std::sync::Mutex<noble_kernel::dataspace::Table>>;

pub const SELECTED_LIMITS: noble_kernel::dataspace::Limits = noble_kernel::dataspace::Limits {
    facets: 8,
    assertions: 8,
    interests: 8,
    events: 32,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Kernel(noble_kernel::dataspace::Error),
    Poisoned,
    WrongProfile,
    Retired,
    UnsupportedSharedGuestMemory,
}

impl From<noble_kernel::dataspace::Error> for Error {
    fn from(error: noble_kernel::dataspace::Error) -> Self {
        Self::Kernel(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdmissionRequest {
    /// There is no shared mutable Noble guest memory in this WIT profile.
    pub shared_mutable_guest_memory: bool,
    /// Independent host policy; a decoded service never supplies these bits.
    pub rights: noble_kernel::dataspace::Rights,
}

pub struct Participant {
    table: Shared,
    facet: noble_kernel::dataspace::Facet,
    live: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublishReceipt<'a> {
    pub accepted: bool,
    pub service: noble_kernel::dataspace::ServiceRef<'a>,
}

impl Participant {
    /// Only host-issued participants carry a facet; no raw-id constructor.
    pub const fn facet(&self) -> noble_kernel::dataspace::Facet {
        self.facet
    }
}

impl Drop for Participant {
    fn drop(&mut self) {
        if self.live {
            // Trap/abandoned-Store cleanup must not rely on a caller remembering
            // close. A poisoned lock is still exclusively held for retirement.
            let mut table = match self.table.lock() {
                Ok(table) => table,
                Err(poisoned) => poisoned.into_inner(),
            };
            let _ = table.retire(self.facet);
            self.live = false;
        }
    }
}

#[derive(Clone)]
pub struct Profile {
    table: Shared,
}

impl Profile {
    pub fn new(limits: noble_kernel::dataspace::Limits) -> Result<Self, Error> {
        Ok(Self {
            table: std::sync::Arc::new(std::sync::Mutex::new(attempt!(
                noble_kernel::dataspace::Table::new(limits)
            ))),
        })
    }

    fn table(&self) -> Result<std::sync::MutexGuard<'_, noble_kernel::dataspace::Table>, Error> {
        self.table.lock().map_err(|_| Error::Poisoned)
    }

    fn member(&self, participant: &Participant) -> Result<noble_kernel::dataspace::Facet, Error> {
        if !std::sync::Arc::ptr_eq(&self.table, &participant.table) {
            return Err(Error::WrongProfile);
        }
        if !participant.live {
            return Err(Error::Retired);
        }
        Ok(participant.facet)
    }

    /// Admission refuses the requested memory bridge before any facet grant.
    pub fn admit_participant(&self, request: AdmissionRequest) -> Result<Participant, Error> {
        if noble_kernel::dataspace::admit_shared_memory(request.shared_mutable_guest_memory)
            .is_err()
        {
            return Err(Error::UnsupportedSharedGuestMemory);
        }
        let facet = attempt!(attempt!(self.table()).open(None, request.rights));
        Ok(Participant {
            table: self.table.clone(),
            facet,
            live: true,
        })
    }

    /// Host-issued child; retirement of the root recursively retires it.
    pub fn child(
        &self,
        parent: &Participant,
        rights: noble_kernel::dataspace::Rights,
    ) -> Result<Participant, Error> {
        let parent = attempt!(self.member(parent));
        let facet = attempt!(attempt!(self.table()).open(Some(parent), rights));
        Ok(Participant {
            table: self.table.clone(),
            facet,
            live: true,
        })
    }

    pub fn publish_wire(&self, owner: &Participant, wire: &[u8]) -> Result<bool, Error> {
        Ok(attempt!(self.publish_wire_receipt(owner, wire)).accepted)
    }

    pub fn publish_wire_receipt<'a>(
        &self,
        owner: &Participant,
        wire: &'a [u8],
    ) -> Result<PublishReceipt<'a>, Error> {
        let facet = attempt!(self.member(owner));
        let (accepted, service) =
            attempt!(attempt!(self.table()).publish_wire_receipt(facet, wire));
        Ok(PublishReceipt { accepted, service })
    }

    pub fn observe_wire(&self, owner: &Participant, wire: &[u8]) -> Result<bool, Error> {
        let facet = attempt!(self.member(owner));
        Ok(attempt!(attempt!(self.table()).observe_wire(facet, wire)))
    }

    pub fn retract_wire(&self, owner: &Participant, wire: &[u8]) -> Result<bool, Error> {
        let facet = attempt!(self.member(owner));
        Ok(attempt!(attempt!(self.table()).retract_wire(facet, wire)))
    }

    /// The typed WIT input must make a Preserves round trip through the exact
    /// same bounded schema ingress as process/network bytes.
    pub fn publish(&self, owner: &Participant, name: &str, ready: bool) -> Result<bool, Error> {
        let wire = attempt!(noble_kernel::dataspace::encode_service(name, ready));
        self.publish_wire(owner, wire.as_bytes())
    }

    pub fn observe(&self, owner: &Participant, name: &str, ready: bool) -> Result<bool, Error> {
        let wire = attempt!(noble_kernel::dataspace::encode_service(name, ready));
        self.observe_wire(owner, wire.as_bytes())
    }

    pub fn retract(&self, owner: &Participant, name: &str) -> Result<bool, Error> {
        let wire = attempt!(noble_kernel::dataspace::encode_service(name, false));
        self.retract_wire(owner, wire.as_bytes())
    }

    /// Idempotent normal/trap close. Failure cleanup cannot be blocked by a
    /// filled event buffer: kernel admission reserved future removal slots.
    pub fn close(&self, owner: &mut Participant) -> Result<(), Error> {
        if owner.live {
            let facet = attempt!(self.member(owner));
            match attempt!(self.table()).retire(facet) {
                Ok(()) | Err(noble_kernel::dataspace::Error::InvalidFacet) => owner.live = false,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    pub fn events(&self) -> Result<Vec<noble_kernel::dataspace::Event>, Error> {
        Ok(attempt!(self.table()).events().to_vec())
    }

    pub fn clear_events(&self) -> Result<(), Error> {
        attempt!(self.table()).clear_events();
        Ok(())
    }

    pub fn counts(&self) -> Result<(u32, u32, u32), Error> {
        Ok(attempt!(self.table()).counts())
    }
}
