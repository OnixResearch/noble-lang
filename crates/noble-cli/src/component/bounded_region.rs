//! Host-owned region adapter for the versioned `bounded-region/bounded` world.
//! The guest receives an opaque component resource, never a native pointer or
//! an offset into its own Wasm memory. Only the host invoker selects the bytes.

use anyhow::{bail, Context as _, Result};
use noble_kernel::{resources as ownership, types::ResourceKind};
use std::ffi::OsString;
use wasmtime::{
    component::{
        types::ComponentItem, Component, Linker, Resource, ResourceAny, ResourceType, Val,
    },
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Trap,
};

use crate::workflow::encoding::{self, Json};

const INTERFACE: &str = "noble-test:bounded-region/regions@1.0.0";
const MAX_REGION_BYTES: usize = 4096;
const KIND: ResourceKind = ResourceKind(1);
const READ: ownership::Rights = ownership::Rights(1);

/// How a bounded-region invocation ended, as far as the host can actually observe. The abnormal
/// classes stay distinct instead of collapsing into one `adapter-trap` label: an exhausted fuel
/// budget is a host-imposed boundary, a cancellation is the host's own interruption, a guest trap
/// is the guest's own abnormal exit, and an adapter fault (missing export, malformed payload,
/// usage) is none of them. Nothing here infers a finer guest outcome from a report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExitClass {
    Normal,
    Rejected,
    Trap,
    QuotaExhausted,
    Cancelled,
    AdapterInvalid,
}

impl ExitClass {
    const fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Rejected => "rejected",
            Self::Trap => "trap",
            Self::QuotaExhausted => "quota-exhausted",
            Self::Cancelled => "cancelled",
            Self::AdapterInvalid => "adapter-invalid",
        }
    }

    /// Classify an invocation error by what it actually is. Anything that is not a Wasmtime trap is
    /// an adapter fault, so an adapter bug can never be reported as a guest trap. An interrupt is a
    /// cancellation: epoch interruption is enabled only with `--cancel-after-reads`, and the epoch
    /// deadline elapses only when the host cancels.
    fn of_error(error: &anyhow::Error) -> Self {
        match error.downcast_ref::<Trap>() {
            Some(Trap::OutOfFuel) => Self::QuotaExhausted,
            Some(Trap::Interrupt) => Self::Cancelled,
            Some(_) => Self::Trap,
            None => Self::AdapterInvalid,
        }
    }

    /// Classify an invocation that returned normally.
    const fn of_return(rejection: bool) -> Self {
        if rejection {
            Self::Rejected
        } else {
            Self::Normal
        }
    }

    /// Why the host's cleanup retired the owner after this exit: a normal return or a rejection
    /// that left the owner unreleased omitted its release; any other exit is abnormal.
    const fn retirement_reason(self) -> &'static str {
        match self {
            Self::Normal | Self::Rejected => "omitted-release",
            Self::Trap => "trap",
            Self::QuotaExhausted => "quota-exhausted",
            Self::Cancelled => "cancelled",
            Self::AdapterInvalid => "adapter-invalid",
        }
    }
}

/// The v2 exit classes: the v1 classes, and `outside-model` for a returned result outside
/// RegionProtocol-v1's result schema.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExitV2 {
    Class(ExitClass),
    OutsideModel,
}

impl ExitV2 {
    const fn label(self) -> &'static str {
        match self {
            Self::Class(class) => class.label(),
            Self::OutsideModel => "outside-model",
        }
    }
}

/// Which report a run emits. `V1` stays the `noble-bounded-region/v1` report, byte for byte; the
/// opt-in `V2` adds what EPROTO-OBS-01 asks of a protocol-evidence report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Schema {
    V1,
    V2,
}

/// A request's operation. The v1 report spells a destructor drop like a guest release; the v2
/// report keeps them apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Operation {
    Read,
    Release,
    Drop,
    HostRetire,
}

impl Operation {
    const fn label(self, schema: Schema) -> &'static str {
        match (self, schema) {
            (Self::Read, Schema::V1 | Schema::V2) => "region.read",
            (Self::Release, Schema::V1 | Schema::V2) | (Self::Drop, Schema::V1) => "region.release",
            (Self::Drop, Schema::V2) => "region.drop",
            (Self::HostRetire, Schema::V1 | Schema::V2) => "host.retire",
        }
    }
}

/// Who committed a release. The v1 report counts the three together (F1); the v2 report apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Release {
    Guest,
    Destructor,
    HostRetirement,
}

/// Why the host refused a read, beyond the reason the guest sees (F8). The guest keeps seeing
/// `invalid-handle` or `bounds-reject`; only the v2 report shows the class.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RefusalClass {
    ForeignRepresentation,
    NoOwner,
    Identity,
    Bounds,
    ScopeExhausted,
    Admission,
    NativeAccess,
    Completion,
}

impl RefusalClass {
    const ALL: [Self; 8] = [
        Self::ForeignRepresentation,
        Self::NoOwner,
        Self::Identity,
        Self::Bounds,
        Self::ScopeExhausted,
        Self::Admission,
        Self::NativeAccess,
        Self::Completion,
    ];

    const fn label(self) -> &'static str {
        match self {
            Self::ForeignRepresentation => "foreign-representation",
            Self::NoOwner => "no-owner",
            Self::Identity => "identity",
            Self::Bounds => "bounds",
            Self::ScopeExhausted => "scope-exhausted",
            Self::Admission => "admission",
            Self::NativeAccess => "native-access",
            Self::Completion => "completion",
        }
    }

    const fn slot(self) -> usize {
        match self {
            Self::ForeignRepresentation => 0,
            Self::NoOwner => 1,
            Self::Identity => 2,
            Self::Bounds => 3,
            Self::ScopeExhausted => 4,
            Self::Admission => 5,
            Self::NativeAccess => 6,
            Self::Completion => 7,
        }
    }
}

/// A refused read: the reason the guest sees, and the host's class for it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Refusal {
    reason: &'static str,
    class: RefusalClass,
}

impl Refusal {
    const BOUNDS: Self = Self {
        reason: "bounds-reject",
        class: RefusalClass::Bounds,
    };

    const fn invalid(class: RefusalClass) -> Self {
        Self {
            reason: "invalid-handle",
            class,
        }
    }
}

/// What the host itself observed about post-exit cleanup. `Absent` is recorded where no observation
/// exists rather than being read as a successful retirement.
#[derive(Clone, PartialEq, Eq, Debug)]
enum CleanupObservation {
    Retired,
    Failed(String),
    /// The retained owner was already gone, so no cleanup call of ours can be attributed.
    AlreadyRetired,
}

impl CleanupObservation {
    const fn label(&self) -> &'static str {
        match self {
            Self::Retired => "retired",
            Self::Failed(_) => "failed",
            Self::AlreadyRetired => "already-retired",
        }
    }

    fn reason(&self) -> Option<&str> {
        match self {
            Self::Retired | Self::AlreadyRetired => None,
            Self::Failed(reason) => Some(reason.as_str()),
        }
    }
}

/// One ordered host event. The trace preserves what RegionProtocol's relation must not erase:
/// request/response pairing by exact resource identity, protected commits, guest releases and host
/// retirements as distinct kinds. It is an observation, never a synthesized protocol outcome.
#[derive(Clone, PartialEq, Eq, Debug)]
enum HostEvent {
    /// A guest-initiated request, with the exact operation, the exact resource identity and
    /// the arguments as they arrived.
    Request {
        operation: Operation,
        identity: u64,
        offset: Option<Offset>,
        length: Option<Offset>,
    },
    /// The host's answer to the request it follows: a value length or a refusal reason.
    Response {
        result: ResponseKind,
        bytes: Option<usize>,
        reason: Option<String>,
        /// The host's class for a refused read, which only the v2 report shows.
        class: Option<RefusalClass>,
    },
    /// A protected read commit: it exists only when the host actually accessed the region, and it
    /// carries the exact identity the host authorised.
    Commit { identity: u64 },
}

/// A signed guest argument exactly as it arrived, without narrowing it to usize: a rejected
/// negative offset is still an observed request, and it is reported with its sign.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Offset(i64);

impl Offset {
    const fn encode(self) -> Json {
        Json::Signed(self.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ResponseKind {
    Value,
    Refused,
}

impl ResponseKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Refused => "refused",
        }
    }
}

impl HostEvent {
    fn encode(&self, schema: Schema) -> Json {
        match self {
            Self::Request {
                operation,
                identity,
                offset,
                length,
            } => encoding::object([
                ("kind", encoding::string("request")),
                ("operation", encoding::string(operation.label(schema))),
                ("identity", Json::Number(*identity)),
                ("offset", offset.map_or(Json::Null, Offset::encode)),
                ("length", length.map_or(Json::Null, Offset::encode)),
            ]),
            Self::Response {
                result,
                bytes,
                reason,
                class,
            } => {
                let mut response = encoding::object([
                    ("kind", encoding::string("response")),
                    ("result", encoding::string(result.label())),
                    (
                        "bytes",
                        bytes.map_or(Json::Null, |count| Json::Number(count as u64)),
                    ),
                    (
                        "reason",
                        reason.as_deref().map_or(Json::Null, encoding::string),
                    ),
                ]);
                if let (Schema::V2, Json::Object(entries)) = (schema, &mut response) {
                    entries.push((
                        "host_class",
                        class.map_or(Json::Null, |class| encoding::string(class.label())),
                    ));
                }
                response
            }
            Self::Commit { identity } => encoding::object([
                ("kind", encoding::string("commit")),
                ("identity", Json::Number(*identity)),
            ]),
        }
    }
}

/// Bounded: a trace never silently drops an event, and a trace that hit its bound says so.
const MAX_EVENTS: usize = 64;

/// What one bounded-region invocation returned, as the adapter observed it.
struct Invocation {
    label: String,
    bytes: Option<Vec<u8>>,
    reason: Option<String>,
    rejected: bool,
}

struct Region;

struct Host {
    limits: StoreLimits,
    context: ownership::Context,
    table: ownership::Table,
    owner: Option<ownership::Owner>,
    bytes: Box<[u8]>,
    protected_operations: u64,
    region_reads: u64,
    releases: u64,
    guest_releases: u64,
    destructor_releases: u64,
    host_retirements: u64,
    /// Reads whose completion failed or returned no owner, so the host lost the owner (F7).
    lost_owners: u64,
    /// Refused reads, by `RefusalClass::slot`. Counted beside the bounded trace, so a class survives
    /// truncation.
    refusals: [u64; 8],
    retirement_reason: Option<&'static str>,
    events: Vec<HostEvent>,
    events_truncated: bool,
    /// The number of answered reads after which the host cancels the invocation, when requested.
    cancel_after_reads: Option<u64>,
    answered_reads: u64,
    /// Set when the host has cancelled the invocation. From then on it admits no guest call.
    cancelled: bool,
}

impl Host {
    fn new(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() > MAX_REGION_BYTES {
            bail!("region exceeds host limit");
        }
        let identity = super::identity::reserve(1)?;
        let context = ownership::Context(identity);
        let required = ownership::Requirement {
            context,
            kind: KIND,
            rights: READ,
        };
        let mut table = ownership::Table::new(
            ownership::TableId(identity),
            ownership::Limits {
                slots: 1,
                pins: 1,
                owners_per_context: 1,
                generations: 1,
                scopes: 64,
            },
        )
        .map_err(|error| anyhow::anyhow!("resource table: {error:?}"))?;
        let owner = table
            .register(required)
            .map_err(|error| anyhow::anyhow!("host registration: {error:?}"))?;
        Ok(Self {
            limits: StoreLimitsBuilder::new()
                .memory_size(4_194_304)
                .instances(16)
                .tables(16)
                .build(),
            context,
            table,
            owner: Some(owner),
            bytes: bytes.into_boxed_slice(),
            protected_operations: 0,
            region_reads: 0,
            releases: 0,
            guest_releases: 0,
            destructor_releases: 0,
            host_retirements: 0,
            lost_owners: 0,
            refusals: [0; 8],
            retirement_reason: None,
            events: Vec::new(),
            events_truncated: false,
            cancel_after_reads: None,
            answered_reads: 0,
            cancelled: false,
        })
    }

    /// Admit a guest call, unless the host has cancelled the invocation. Cancellation revokes
    /// access immediately (RA-CLEAN-02): a call after it traps before reaching the protocol.
    fn admit(&self) -> Result<()> {
        if self.cancelled {
            return Err(Trap::Interrupt.into());
        }
        Ok(())
    }

    /// Count an answered read, and cancel the invocation once the requested number of reads is
    /// answered. Returns whether this answer cancelled it.
    fn answered_read(&mut self) -> bool {
        self.answered_reads = self.answered_reads.saturating_add(1);
        let cancel = self.cancel_after_reads == Some(self.answered_reads);
        if cancel {
            self.cancelled = true;
        }
        cancel
    }

    fn record(&mut self, event: HostEvent) {
        if self.events.len() >= MAX_EVENTS {
            self.events_truncated = true;
            return;
        }
        self.events.push(event);
    }

    const fn required(&self) -> ownership::Requirement {
        ownership::Requirement {
            context: self.context,
            kind: KIND,
            rights: READ,
        }
    }

    // Claims are untrusted even when Wasmtime has already checked the resource
    // representation. Every read still checks the independently retained table.
    fn read(
        &mut self,
        rep: u32,
        claim: ownership::Handle,
        offset: i64,
        length: i64,
    ) -> Result<Vec<u8>, &'static str> {
        self.record(HostEvent::Request {
            operation: Operation::Read,
            identity: claim.table.0,
            offset: Some(Offset(offset)),
            length: Some(Offset(length)),
        });
        let answered = self.read_region(rep, claim, offset, length);
        if let Err(refusal) = &answered {
            let count = &mut self.refusals[refusal.class.slot()];
            *count = count.saturating_add(1);
        }
        self.record(match &answered {
            Ok(bytes) => HostEvent::Response {
                result: ResponseKind::Value,
                bytes: Some(bytes.len()),
                reason: None,
                class: None,
            },
            Err(refusal) => HostEvent::Response {
                result: ResponseKind::Refused,
                bytes: None,
                reason: Some(refusal.reason.to_owned()),
                class: Some(refusal.class),
            },
        });
        answered.map_err(|refusal| refusal.reason)
    }

    fn read_region(
        &mut self,
        rep: u32,
        claim: ownership::Handle,
        offset: i64,
        length: i64,
    ) -> Result<Vec<u8>, Refusal> {
        if rep != 1 {
            return Err(Refusal::invalid(RefusalClass::ForeignRepresentation));
        }
        let actual = self
            .owner
            .as_ref()
            .ok_or(Refusal::invalid(RefusalClass::NoOwner))?
            .handle();
        let required = self.required();
        self.table
            .validate(claim, required)
            .map_err(|_| Refusal::invalid(RefusalClass::Identity))?;
        if claim != actual {
            return Err(Refusal::invalid(RefusalClass::Identity));
        }
        let end = offset.checked_add(length).ok_or(Refusal::BOUNDS)?;
        let start = usize::try_from(offset).map_err(|_| Refusal::BOUNDS)?;
        let count = usize::try_from(length).map_err(|_| Refusal::BOUNDS)?;
        let finish = usize::try_from(end).map_err(|_| Refusal::BOUNDS)?;
        if start > self.bytes.len() || finish > self.bytes.len() || count > MAX_REGION_BYTES {
            return Err(Refusal::BOUNDS);
        }
        let owner = self
            .owner
            .take()
            .ok_or(Refusal::invalid(RefusalClass::NoOwner))?;
        let admitted = match self.table.begin(owner, required) {
            Ok(admitted) => admitted,
            Err(rejection) => {
                let class = if matches!(rejection.error, ownership::Error::ScopeExhausted) {
                    RefusalClass::ScopeExhausted
                } else {
                    RefusalClass::Admission
                };
                self.owner = Some(rejection.input);
                return Err(Refusal::invalid(class));
            }
        };
        let authorized = self.table.native_access(&admitted.borrow).is_ok();
        let output = if authorized {
            crate::safety_observe::event("protected-operation");
            self.region_reads += 1;
            Some(self.bytes[start..finish].to_vec())
        } else {
            None
        };
        let Ok(completed) = self.table.complete(
            admitted.borrow.scope(),
            if authorized {
                ownership::Completion::Success
            } else {
                ownership::Completion::DomainError
            },
        ) else {
            self.lost_owners = self.lost_owners.saturating_add(1);
            return Err(Refusal::invalid(RefusalClass::Completion));
        };
        if completed.owner.is_none() {
            self.lost_owners = self.lost_owners.saturating_add(1);
        }
        self.owner = completed.owner;
        match output {
            Some(bytes) => {
                self.protected_operations += 1;
                self.record(HostEvent::Commit {
                    identity: claim.table.0,
                });
                Ok(bytes)
            }
            None => Err(Refusal::invalid(RefusalClass::NativeAccess)),
        }
    }

    fn release(&mut self, rep: u32, kind: Release) -> Result<()> {
        if rep != 1 {
            bail!("invalid region representation");
        }
        let required = self.required();
        let owner = self.owner.take().context("region already released")?;
        match self.table.release(owner, required) {
            Ok(_) => {
                self.releases += 1;
                let count = match kind {
                    Release::Guest => &mut self.guest_releases,
                    Release::Destructor => &mut self.destructor_releases,
                    Release::HostRetirement => &mut self.host_retirements,
                };
                *count = count.saturating_add(1);
                Ok(())
            }
            Err(rejected) => {
                self.owner = Some(rejected.input);
                bail!("region release refused: {:?}", rejected.error)
            }
        }
    }

    /// Abnormal guest exit must retire its owner even if no destructor ran. A refusal is retained as
    /// an observation rather than turning the whole report into an error: the failed cleanup is the
    /// fact the caller needs, and the owner stays available for recovery.
    fn abnormal_exit_cleanup(&mut self, exit: ExitClass) -> CleanupObservation {
        if self.owner.is_none() {
            return CleanupObservation::AlreadyRetired;
        }
        self.record(HostEvent::Request {
            operation: Operation::HostRetire,
            identity: self.context.0,
            offset: None,
            length: None,
        });
        let retired = self.release(1, Release::HostRetirement);
        if retired.is_ok() {
            self.retirement_reason = Some(exit.retirement_reason());
        }
        self.record(match &retired {
            Ok(()) => HostEvent::Response {
                result: ResponseKind::Value,
                bytes: None,
                reason: None,
                class: None,
            },
            Err(error) => HostEvent::Response {
                result: ResponseKind::Refused,
                bytes: None,
                reason: Some(format!("{error:#}")),
                class: None,
            },
        });
        match retired {
            Ok(()) => CleanupObservation::Retired,
            Err(error) => CleanupObservation::Failed(format!("{error:#}")),
        }
    }

    fn report(
        &self,
        outcome: &str,
        exit_class: ExitClass,
        cleanup: &CleanupObservation,
        fuel_remaining: Option<u64>,
        bytes: Option<&[u8]>,
        error: Option<&str>,
    ) -> Json {
        let observation = self.table.observation();
        encoding::object([
            ("schema", encoding::string("noble-bounded-region/v1")),
            ("stage", encoding::string("adapter")),
            ("outcome", encoding::string(outcome)),
            ("exit_class", encoding::string(exit_class.label())),
            (
                "cleanup",
                encoding::object([
                    ("state", encoding::string(cleanup.label())),
                    (
                        "reason",
                        cleanup.reason().map_or(Json::Null, encoding::string),
                    ),
                ]),
            ),
            (
                "fuel_remaining",
                fuel_remaining.map_or(Json::Null, Json::Number),
            ),
            (
                "bytes_hex",
                bytes.map_or(Json::Null, |value| encoding::string(encoding::hex(value))),
            ),
            ("error", error.map_or(Json::Null, encoding::string)),
            (
                "protected_operations",
                Json::Number(self.protected_operations),
            ),
            ("region_reads", Json::Number(self.region_reads)),
            ("native_memory_access", Json::Bool(false)),
            (
                "events",
                Json::Array(
                    self.events
                        .iter()
                        .map(|event| event.encode(Schema::V1))
                        .collect(),
                ),
            ),
            ("events_truncated", Json::Bool(self.events_truncated)),
            ("released_owners", Json::Number(self.releases)),
            ("live_owners", Json::Number(observation.live as u64)),
            ("native_pins", Json::Number(observation.native_pins as u64)),
        ])
    }

    /// The opt-in `noble-bounded-region/v2` report (EPROTO-OBS-01). Beside the v1 observations it
    /// counts guest releases, destructor releases and host retirements apart, records the
    /// cleanup's retirement reason, classifies each refused read, counts lost owners and records
    /// whether the host was asked to cancel and did.
    fn report_v2(
        &self,
        outcome: &str,
        exit: ExitV2,
        cleanup: &CleanupObservation,
        fuel_remaining: Option<u64>,
        bytes: Option<&[u8]>,
        error: Option<&str>,
    ) -> Json {
        let observation = self.table.observation();
        encoding::object([
            ("schema", encoding::string("noble-bounded-region/v2")),
            ("stage", encoding::string("adapter")),
            ("outcome", encoding::string(outcome)),
            ("exit_class", encoding::string(exit.label())),
            (
                "cancellation",
                encoding::object([
                    (
                        "after_reads",
                        self.cancel_after_reads.map_or(Json::Null, Json::Number),
                    ),
                    ("requested", Json::Bool(self.cancelled)),
                ]),
            ),
            (
                "cleanup",
                encoding::object([
                    ("state", encoding::string(cleanup.label())),
                    (
                        "reason",
                        cleanup.reason().map_or(Json::Null, encoding::string),
                    ),
                    (
                        "retirement_reason",
                        self.retirement_reason.map_or(Json::Null, encoding::string),
                    ),
                ]),
            ),
            (
                "fuel_remaining",
                fuel_remaining.map_or(Json::Null, Json::Number),
            ),
            (
                "bytes_hex",
                bytes.map_or(Json::Null, |value| encoding::string(encoding::hex(value))),
            ),
            ("error", error.map_or(Json::Null, encoding::string)),
            (
                "protected_operations",
                Json::Number(self.protected_operations),
            ),
            ("region_reads", Json::Number(self.region_reads)),
            ("native_memory_access", Json::Bool(false)),
            // The owner the host registered before guest start (A1). A host retirement is
            // authorised by this registration, not by any guest request, so a trace in which the
            // guest made no request still names who may be retired (#50).
            (
                "registration",
                encoding::object([("identity", Json::Number(self.context.0))]),
            ),
            (
                "events",
                Json::Array(
                    self.events
                        .iter()
                        .map(|event| event.encode(Schema::V2))
                        .collect(),
                ),
            ),
            ("events_truncated", Json::Bool(self.events_truncated)),
            (
                "releases",
                encoding::object([
                    ("guest", Json::Number(self.guest_releases)),
                    ("destructor", Json::Number(self.destructor_releases)),
                    ("host_retirement", Json::Number(self.host_retirements)),
                ]),
            ),
            (
                "refusals",
                Json::Object(
                    RefusalClass::ALL
                        .iter()
                        .map(|class| (class.label(), Json::Number(self.refusals[class.slot()])))
                        .collect(),
                ),
            ),
            ("lost_owners", Json::Number(self.lost_owners)),
            ("live_owners", Json::Number(observation.live as u64)),
            ("native_pins", Json::Number(observation.native_pins as u64)),
        ])
    }
}

fn link(engine: &Engine, component: &Component, linker: &mut Linker<Host>) -> Result<()> {
    for (name, item) in component.component_type().imports(engine) {
        match item {
            ComponentItem::ComponentInstance(interface) if name == INTERFACE => {
                let mut instance = linker.instance(name)?;
                for (function, item) in interface.exports(engine) {
                    match item {
                        ComponentItem::Resource(_) if function == "region" => {
                            instance.resource(
                                function,
                                ResourceType::host::<Region>(),
                                |mut store, rep| {
                                    crate::safety_observe::event("guest-request");
                                    store.data().admit()?;
                                    let identity = store.data().context.0;
                                    store.data_mut().record(HostEvent::Request {
                                        operation: Operation::Drop,
                                        identity,
                                        offset: None,
                                        length: None,
                                    });
                                    let released = if store.data().owner.is_some() {
                                        store.data_mut().release(rep, Release::Destructor)
                                    } else {
                                        Ok(())
                                    };
                                    store.data_mut().record(match &released {
                                        Ok(()) => HostEvent::Response {
                                            result: ResponseKind::Value,
                                            bytes: None,
                                            reason: None,
                                            class: None,
                                        },
                                        Err(error) => HostEvent::Response {
                                            result: ResponseKind::Refused,
                                            bytes: None,
                                            reason: Some(format!("{error:#}")),
                                            class: None,
                                        },
                                    });
                                    released
                                },
                            )?;
                        }
                        ComponentItem::ComponentFunc(_) if function == "[method]region.read" => {
                            instance.func_new(function, |mut store, _, params, results| {
                                crate::safety_observe::event("guest-request");
                                store.data().admit()?;
                                let [Val::Resource(resource), Val::S64(offset), Val::S64(length)] =
                                    params
                                else {
                                    bail!("invalid region read arguments")
                                };
                                let region = resource.try_into_resource::<Region>(&mut store)?;
                                if region.owned() {
                                    bail!("read receiver must be borrowed");
                                }
                                let claim = store
                                    .data()
                                    .owner
                                    .as_ref()
                                    .context("missing region")?
                                    .handle();
                                let value =
                                    store.data_mut().read(region.rep(), claim, *offset, *length);
                                if store.data_mut().answered_read() {
                                    // The deadline elapses at once, so the guest is interrupted at
                                    // its next function entry or loop header.
                                    store.set_epoch_deadline(0);
                                }
                                let [slot] = results else {
                                    bail!("invalid region read result");
                                };
                                *slot = match value {
                                    Ok(bytes) => Val::Result(Ok(Some(Box::new(Val::List(
                                        bytes.into_iter().map(Val::U8).collect(),
                                    ))))),
                                    Err(reason) => {
                                        Val::Result(Err(Some(Box::new(Val::String(reason.into())))))
                                    }
                                };
                                Ok(())
                            })?;
                        }
                        ComponentItem::ComponentFunc(_) if function == "release" => {
                            instance.func_new(function, |mut store, _, params, results| {
                                crate::safety_observe::event("guest-request");
                                store.data().admit()?;
                                let [Val::Resource(resource)] = params else {
                                    bail!("invalid release arguments");
                                };
                                if !results.is_empty() {
                                    bail!("invalid release result");
                                }
                                let region = resource.try_into_resource::<Region>(&mut store)?;
                                if !region.owned() {
                                    bail!("release requires an owner");
                                }
                                let identity = store.data().context.0;
                                store.data_mut().record(HostEvent::Request {
                                    operation: Operation::Release,
                                    identity,
                                    offset: None,
                                    length: None,
                                });
                                let released =
                                    store.data_mut().release(region.rep(), Release::Guest);
                                store.data_mut().record(match &released {
                                    Ok(()) => HostEvent::Response {
                                        result: ResponseKind::Value,
                                        bytes: None,
                                        reason: None,
                                        class: None,
                                    },
                                    Err(error) => HostEvent::Response {
                                        result: ResponseKind::Refused,
                                        bytes: None,
                                        reason: Some(format!("{error:#}")),
                                        class: None,
                                    },
                                });
                                released
                            })?;
                        }
                        ComponentItem::Type(_) => {}
                        _ => bail!("unapproved region import {name}#{function}"),
                    }
                }
            }
            ComponentItem::Resource(_) if name == "region" => {
                linker.root().resource(
                    name,
                    ResourceType::host::<Region>(),
                    |mut store, rep| {
                        crate::safety_observe::event("guest-request");
                        let identity = store.data().context.0;
                        store.data_mut().record(HostEvent::Request {
                            operation: Operation::Drop,
                            identity,
                            offset: None,
                            length: None,
                        });
                        let released = if store.data().owner.is_some() {
                            store.data_mut().release(rep, Release::Destructor)
                        } else {
                            Ok(())
                        };
                        store.data_mut().record(match &released {
                            Ok(()) => HostEvent::Response {
                                result: ResponseKind::Value,
                                bytes: None,
                                reason: None,
                                class: None,
                            },
                            Err(error) => HostEvent::Response {
                                result: ResponseKind::Refused,
                                bytes: None,
                                reason: Some(format!("{error:#}")),
                                class: None,
                            },
                        });
                        released
                    },
                )?;
            }
            ComponentItem::Type(_) => {}
            _ => bail!("unapproved component import {name}"),
        }
    }
    Ok(())
}

fn parse_hex(text: &str) -> Result<Vec<u8>> {
    if !text.len().is_multiple_of(2) || text.len() > MAX_REGION_BYTES * 2 {
        bail!("invalid bounded region hex length");
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let ascii = std::str::from_utf8(pair)?;
            Ok(u8::from_str_radix(ascii, 16)?)
        })
        .collect()
}

/// Whether a returned result lies outside RegionProtocol-v1's result schema: an error other than
/// `bounds-reject` and `invalid-handle`, or an `ok` whose length is not the requested one.
fn outside_model(invocation: &Invocation, length: i64) -> bool {
    if let Some(bytes) = &invocation.bytes {
        return i64::try_from(bytes.len()).ok() != Some(length);
    }
    invocation
        .reason
        .as_deref()
        .is_none_or(|reason| !matches!(reason, "bounds-reject" | "invalid-handle"))
}

const USAGE: &str = "usage: noble component read-region [--report v1|v2] [--fuel N] [--cancel-after-reads N] COMPONENT HOST_BUFFER_HEX OFFSET LENGTH";

/// The fuel budget of an invocation by default, and the largest one `--fuel` accepts.
const FUEL: u64 = 10_000_000;

/// The flags of one invocation.
#[derive(Debug, PartialEq, Eq)]
struct Options {
    schema: Schema,
    fuel: u64,
    /// Cancel the invocation once this many reads are answered (EPROTO-CASE-07).
    cancel_after_reads: Option<u64>,
}

/// Read the leading flags, in any order, and return them with the remaining arguments. Only the
/// v2 report can show a cancellation, so `--cancel-after-reads` requires `--report v2`.
fn options(mut arguments: &[OsString]) -> Result<(Options, &[OsString])> {
    let mut options = Options {
        schema: Schema::V1,
        fuel: FUEL,
        cancel_after_reads: None,
    };
    while let [flag, value, rest @ ..] = arguments {
        match flag.to_str() {
            Some("--report") => options.schema = report_schema(value)?,
            Some("--fuel") => options.fuel = fuel_budget(value)?,
            Some("--cancel-after-reads") => options.cancel_after_reads = Some(read_count(value)?),
            Some(_) | None => break,
        }
        arguments = rest;
    }
    if options.cancel_after_reads.is_some() && options.schema == Schema::V1 {
        bail!("--cancel-after-reads requires --report v2; {USAGE}");
    }
    Ok((options, arguments))
}

fn report_schema(value: &OsString) -> Result<Schema> {
    match value.to_str() {
        Some("v1") => Ok(Schema::V1),
        Some("v2") => Ok(Schema::V2),
        _ => bail!("{USAGE}"),
    }
}

/// An explicit fuel budget, a test knob for placing quota exhaustion (EPROTO-CASE-06).
fn fuel_budget(value: &OsString) -> Result<u64> {
    value
        .to_str()
        .and_then(|text| text.parse().ok())
        .filter(|fuel| (1..=FUEL).contains(fuel))
        .context(USAGE)
}

/// A positive number of answered reads.
fn read_count(value: &OsString) -> Result<u64> {
    value
        .to_str()
        .and_then(|text| text.parse::<std::num::NonZeroU64>().ok())
        .map(std::num::NonZeroU64::get)
        .context(USAGE)
}

fn execute(arguments: &[OsString]) -> Result<Json> {
    let (options, rest) = options(arguments.get(2..).context(USAGE)?)?;
    let [path, hex, offset, length] = rest else {
        bail!("{USAGE}")
    };
    let path = std::path::Path::new(path);
    let hex = hex.to_str().context("buffer hex must be UTF-8")?;
    let offset: i64 = offset.to_str().context("offset must be UTF-8")?.parse()?;
    let length: i64 = length.to_str().context("length must be UTF-8")?.parse()?;
    let bytes = parse_hex(hex)?;
    let component_bytes =
        crate::workflow::read_bounded(path, 4_194_304, "bounded-region-component")
            .map_err(|error| anyhow::anyhow!("component input: {}", error.json().encode()))?;
    let mut config = Config::new();
    config
        .wasm_component_model(true)
        .consume_fuel(true)
        .epoch_interruption(options.cancel_after_reads.is_some());
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, component_bytes)?;
    let mut linker = Linker::new(&engine);
    link(&engine, &component, &mut linker)?;
    let mut host = Host::new(bytes)?;
    host.cancel_after_reads = options.cancel_after_reads;
    let mut store = Store::new(&engine, host);
    store.limiter(|host| &mut host.limits);
    store.set_fuel(options.fuel)?;
    if options.cancel_after_reads.is_some() {
        // The engine's epoch is never incremented, so this deadline elapses only when the host
        // cancels by setting it to zero.
        store.set_epoch_deadline(1);
    }
    let instance = linker.instantiate(&mut store, &component)?;
    let function = instance
        .get_func(&mut store, "read-region")
        .context("missing read-region export")?;
    let owner = ResourceAny::try_from_resource(Resource::<Region>::new_own(1), &mut store)?;
    let mut results = [Val::Bool(false)];
    let invoked = (|| -> Result<Invocation> {
        function.call(
            &mut store,
            &[Val::Resource(owner), Val::S64(offset), Val::S64(length)],
            &mut results,
        )?;
        function.post_return(&mut store)?;
        let [Val::Result(result)] = &results else {
            bail!("invalid read-region result");
        };
        match result {
            Ok(Some(value)) => {
                let Val::List(list) = value.as_ref() else {
                    bail!("invalid success payload");
                };
                let bytes = list
                    .iter()
                    .map(|value| match value {
                        Val::U8(byte) => Ok(*byte),
                        _ => bail!("invalid byte result"),
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(Invocation {
                    label: "read".into(),
                    bytes: Some(bytes),
                    reason: None,
                    rejected: false,
                })
            }
            Err(Some(value)) => {
                let Val::String(reason) = value.as_ref() else {
                    bail!("invalid rejection payload");
                };
                Ok(Invocation {
                    label: reason.clone(),
                    bytes: None,
                    reason: Some(reason.clone()),
                    rejected: true,
                })
            }
            _ => bail!("invalid read-region variant"),
        }
    })();
    let fuel_remaining = store.get_fuel().ok();
    let outside = invoked
        .as_ref()
        .is_ok_and(|invocation| outside_model(invocation, length));
    let (outcome, exit_class, returned, error) = match invoked {
        Ok(Invocation {
            label,
            bytes,
            reason,
            rejected,
        }) => (label, ExitClass::of_return(rejected), bytes, reason),
        Err(error) => (
            "adapter-trap".into(),
            ExitClass::of_error(&error),
            None,
            // v1 keeps the outermost context, as before. v2 carries the whole chain, so an untyped
            // runtime refusal (Wasmtime's `unknown handle index` for a consumed handle) keeps its
            // cause beside the `adapter-invalid` class.
            Some(match options.schema {
                Schema::V1 => error.to_string(),
                Schema::V2 => format!("{error:#}"),
            }),
        ),
    };
    let cleanup = store.data_mut().abnormal_exit_cleanup(exit_class);
    Ok(match options.schema {
        Schema::V1 => store.data().report(
            &outcome,
            exit_class,
            &cleanup,
            fuel_remaining,
            returned.as_deref(),
            error.as_deref(),
        ),
        Schema::V2 => store.data().report_v2(
            &outcome,
            if outside {
                ExitV2::OutsideModel
            } else {
                ExitV2::Class(exit_class)
            },
            &cleanup,
            fuel_remaining,
            returned.as_deref(),
            error.as_deref(),
        ),
    })
}

pub(super) fn run(arguments: &[OsString]) -> std::process::ExitCode {
    match execute(arguments) {
        Ok(report) => {
            println!("{}", report.encode());
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("bounded-region adapter: {error:#}");
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_claims_and_boundaries_do_not_read_or_consume_region() {
        let mut host = Host::new(vec![0, 1, 2]).unwrap();
        let original = host.owner.as_ref().unwrap().handle();
        let mut forged = original;
        forged.slot = 42;
        let mut wrong_table = original;
        wrong_table.table = ownership::TableId(original.table.0.checked_add(1).unwrap());
        let mut foreign = original;
        foreign.context = ownership::Context(original.context.0.checked_add(1).unwrap());
        let mut wrong_kind = original;
        wrong_kind.kind = ResourceKind(2);
        let mut stale = original;
        stale.generation += 1;
        let mut wrong_right = original;
        wrong_right.rights = ownership::Rights(0);
        for claim in [forged, wrong_table, foreign, wrong_kind, stale, wrong_right] {
            assert_eq!(host.read(1, claim, 1, 2), Err("invalid-handle"));
        }
        assert_eq!(host.read(2, original, 1, 2), Err("invalid-handle"));
        for (offset, length) in [(3, 1), (2, 2), (4, 0), (-1, 1), (0, -1), (i64::MAX, 1)] {
            assert_eq!(host.read(1, original, offset, length), Err("bounds-reject"));
        }
        assert_eq!(host.region_reads, 0);
        assert_eq!(host.protected_operations, 0);
        assert_eq!(host.table.observation().native_pins, 0);
        assert_eq!(host.owner.as_ref().unwrap().handle(), original);
        assert_eq!(host.read(1, original, 1, 2), Ok(vec![1, 2]));
        assert_eq!(host.owner.as_ref().unwrap().handle(), original);
        assert_eq!(host.region_reads, 1);
        assert_eq!(host.protected_operations, 1);
        host.release(1, Release::Guest).unwrap();
        assert_eq!(host.releases, 1);
        assert_eq!(host.read(1, original, 0, 0), Err("invalid-handle"));
        assert_eq!(host.region_reads, 1);
    }

    #[test]
    fn end_boundary_is_empty_success_and_zero_length_outside_refuses() {
        let mut host = Host::new(vec![0, 1, 2]).unwrap();
        let claim = host.owner.as_ref().unwrap().handle();
        assert_eq!(host.read(1, claim, 3, 0), Ok(Vec::new()));
        assert_eq!(host.read(1, claim, 4, 0), Err("bounds-reject"));
        assert_eq!(host.region_reads, 1);
        host.release(1, Release::Guest).unwrap();
        assert_eq!(host.table.observation().live, 0);
    }

    #[test]
    fn same_slot_and_generation_from_another_live_invocation_cannot_cross() {
        let first = Host::new(vec![0, 1, 2]).unwrap();
        let mut second = Host::new(vec![9, 8, 7]).unwrap();
        let stolen = first.owner.as_ref().unwrap().handle();
        let rightful = second.owner.as_ref().unwrap().handle();
        assert_eq!(stolen.slot, rightful.slot);
        assert_eq!(stolen.generation, rightful.generation);
        assert_ne!(stolen.table, rightful.table);
        assert_ne!(stolen.context, rightful.context);
        assert_eq!(second.read(1, stolen, 0, 1), Err("invalid-handle"));
        assert_eq!(second.region_reads, 0);
        assert_eq!(second.table.observation().native_pins, 0);
        assert_eq!(second.read(1, rightful, 0, 1), Ok(vec![9]));
        second.release(1, Release::Guest).unwrap();
    }

    #[test]
    fn double_release_does_not_retire_the_owner_twice() {
        let mut host = Host::new(vec![0, 1, 2]).unwrap();
        host.release(1, Release::Guest).unwrap();
        assert_eq!(host.releases, 1);
        assert_eq!(host.table.observation().live, 0);
        assert!(host.release(1, Release::Guest).is_err());
        assert_eq!(host.releases, 1);
        assert_eq!(host.table.observation().live, 0);
    }

    #[test]
    fn exit_classes_distinguish_quota_trap_and_adapter_faults() {
        let quota = anyhow::Error::new(Trap::OutOfFuel);
        let guest = anyhow::Error::new(Trap::UnreachableCodeReached);
        let adapter = anyhow::anyhow!("invalid success payload");
        assert_eq!(ExitClass::of_error(&quota), ExitClass::QuotaExhausted);
        assert_eq!(ExitClass::of_error(&guest), ExitClass::Trap);
        assert_eq!(ExitClass::of_error(&adapter), ExitClass::AdapterInvalid);
        assert_eq!(ExitClass::of_return(true), ExitClass::Rejected);
        assert_eq!(ExitClass::of_return(false), ExitClass::Normal);
        assert_eq!(ExitClass::QuotaExhausted.label(), "quota-exhausted");
        assert_eq!(ExitClass::AdapterInvalid.label(), "adapter-invalid");
    }

    #[test]
    fn abnormal_exit_cleanup_is_observed_rather_than_propagated() {
        let mut host = Host::new(vec![0, 1, 2]).unwrap();
        assert_eq!(
            host.abnormal_exit_cleanup(ExitClass::Trap),
            CleanupObservation::Retired
        );
        assert_eq!(host.releases, 1);
        assert_eq!(host.table.observation().live, 0);
        // A second pass has no retained owner left, so nothing is attributed to us.
        assert_eq!(
            host.abnormal_exit_cleanup(ExitClass::Trap),
            CleanupObservation::AlreadyRetired
        );
        assert_eq!(host.releases, 1);
    }

    #[test]
    fn failed_cleanup_is_reported_as_a_failed_observation() {
        // Test-only corrupted Host state: a guest cannot mutate this context.
        let mut host = Host::new(vec![0, 1, 2]).unwrap();
        let context = host.context;
        let original = host.owner.as_ref().unwrap().handle();
        host.context = ownership::Context(context.0.checked_add(1).unwrap());
        match host.abnormal_exit_cleanup(ExitClass::Trap) {
            CleanupObservation::Failed(reason) => {
                assert!(reason.contains("region release refused"), "{reason}");
                assert_eq!(CleanupObservation::Failed(reason).label(), "failed");
            }
            other => panic!("expected a failed cleanup observation, got {other:?}"),
        }
        assert_eq!(host.releases, 0);
        assert_eq!(host.table.observation().live, 1);
        assert_eq!(host.owner.as_ref().unwrap().handle(), original);
        host.context = context;
        assert_eq!(
            host.abnormal_exit_cleanup(ExitClass::Trap),
            CleanupObservation::Retired
        );
        assert_eq!(host.releases, 1);
    }

    #[test]
    fn failed_cleanup_release_retains_owner_for_recovery() {
        // Test-only corrupted Host state: a guest cannot mutate this context.
        // Exercise the production release refusal and returned-owner path.
        let mut host = Host::new(vec![0, 1, 2]).unwrap();
        let context = host.context;
        let original = host.owner.as_ref().unwrap().handle();
        host.context = ownership::Context(context.0.checked_add(1).unwrap());
        assert!(host.release(1, Release::Guest).is_err());
        assert_eq!(host.releases, 0);
        assert_eq!(host.table.observation().live, 1);
        assert_eq!(host.owner.as_ref().unwrap().handle(), original);

        host.context = context;
        host.release(1, Release::Guest).unwrap();
        assert_eq!(host.releases, 1);
        assert_eq!(host.table.observation().live, 0);
    }
}

#[cfg(test)]
mod ordered_trace_tests {
    use super::{Host, HostEvent, Operation, ResponseKind, Schema, MAX_EVENTS};

    fn host(bytes: &[u8]) -> Host {
        Host::new(bytes.to_vec()).expect("host registration")
    }

    fn kinds(host: &Host) -> Vec<&'static str> {
        host.events
            .iter()
            .map(|event| match event {
                HostEvent::Request { operation, .. } => operation.label(Schema::V1),
                HostEvent::Response { .. } => "response",
                HostEvent::Commit { .. } => "commit",
            })
            .collect()
    }

    /// Every read request, its protected commit and every release request carry the exact table
    /// identity the host retained, so a handle class can never stand in for it.
    fn identities(host: &Host) -> Vec<u64> {
        host.events
            .iter()
            .filter_map(|event| match event {
                HostEvent::Request { identity, .. } | HostEvent::Commit { identity } => {
                    Some(*identity)
                }
                HostEvent::Response { .. } => None,
            })
            .collect()
    }

    #[test]
    fn an_admitted_read_records_request_response_and_its_protected_commit_in_order() {
        let mut host = host(b"abc");
        let claim = host.owner.as_ref().expect("owner").handle();
        let identity = claim.table.0;
        assert_eq!(host.read(1, claim, 0, 1).expect("read"), b"a");
        assert_eq!(kinds(&host), ["region.read", "commit", "response"]);
        assert_eq!(
            host.events.first(),
            Some(&HostEvent::Request {
                operation: Operation::Read,
                identity,
                offset: Some(super::Offset(0)),
                length: Some(super::Offset(1)),
            })
        );
        assert_eq!(
            host.events.get(2),
            Some(&HostEvent::Response {
                result: ResponseKind::Value,
                bytes: Some(1),
                reason: None,
                class: None,
            })
        );
        assert_eq!(identities(&host), [identity, identity]);
    }

    #[test]
    fn a_refused_read_records_its_reason_and_no_commit() {
        let mut host = host(b"abc");
        let claim = host.owner.as_ref().expect("owner").handle();
        assert_eq!(host.read(1, claim, 0, 9), Err("bounds-reject"));
        assert_eq!(kinds(&host), ["region.read", "response"]);
        match host.events.get(1) {
            Some(HostEvent::Response {
                result,
                bytes,
                reason,
                class,
            }) => {
                assert_eq!(*result, ResponseKind::Refused);
                assert_eq!(*bytes, None);
                assert_eq!(reason.as_deref(), Some("bounds-reject"));
                assert_eq!(*class, Some(super::RefusalClass::Bounds));
            }
            other => panic!("expected a refused response: {other:?}"),
        }
    }

    #[test]
    fn host_retirement_is_its_own_request_and_never_a_guest_release() {
        let mut host = host(b"abc");
        assert_eq!(
            host.abnormal_exit_cleanup(super::ExitClass::Trap),
            super::CleanupObservation::Retired
        );
        assert_eq!(kinds(&host), ["host.retire", "response"]);
        assert_eq!(identities(&host), [host.context.0]);
        assert_eq!(host.releases, 1);
    }

    /// A recorded request reports the arguments the guest passed, signed. A refused negative or
    /// extreme argument is reported as itself, never as its unsigned reinterpretation (#90), and
    /// an admitted read reports its own arguments the same way.
    #[test]
    fn recorded_requests_report_the_signed_arguments_the_guest_passed() {
        for (offset, length, answer, arguments) in [
            (-1, 1, Err("bounds-reject"), r#""offset":-1,"length":1"#),
            (0, -1, Err("bounds-reject"), r#""offset":0,"length":-1"#),
            (
                i64::MIN,
                1,
                Err("bounds-reject"),
                r#""offset":-9223372036854775808,"length":1"#,
            ),
            (
                1,
                i64::MIN,
                Err("bounds-reject"),
                r#""offset":1,"length":-9223372036854775808"#,
            ),
            (
                i64::MAX,
                1,
                Err("bounds-reject"),
                r#""offset":9223372036854775807,"length":1"#,
            ),
            (1, 2, Ok(b"bc".to_vec()), r#""offset":1,"length":2"#),
        ] {
            let mut host = host(b"abc");
            let claim = host.owner.as_ref().expect("owner").handle();
            let identity = claim.table.0;
            assert_eq!(host.read(1, claim, offset, length), answer);
            let request = host
                .events
                .first()
                .expect("request event")
                .encode(Schema::V1)
                .encode();
            assert_eq!(
                request,
                format!(
                    r#"{{"kind":"request","operation":"region.read","identity":{identity},{arguments}}}"#
                ),
                "({offset}, {length})"
            );
            let admitted = u64::from(answer.is_ok());
            assert_eq!(
                (host.protected_operations, host.region_reads),
                (admitted, admitted)
            );
            assert!(!host.events_truncated);
        }
    }

    #[test]
    fn a_trace_that_hits_its_bound_says_so_instead_of_dropping_silently() {
        let mut host = host(b"abc");
        let claim = host.owner.as_ref().expect("owner").handle();
        for _ in 0..(MAX_EVENTS / 3 + 1) {
            assert_eq!(host.read(1, claim, 0, 1).expect("read"), b"a");
        }
        assert!(host.events_truncated);
        assert_eq!(host.events.len(), MAX_EVENTS);
    }
}

#[cfg(test)]
mod report_v2_tests {
    use super::{
        options, outside_model, CleanupObservation, ExitClass, ExitV2, Host, HostEvent, Invocation,
        Operation, Options, RefusalClass, Release, ResponseKind, Schema, FUEL,
    };
    use crate::workflow::encoding::Json;
    use std::ffi::OsString;
    use wasmtime::Trap;

    fn host(bytes: &[u8]) -> Host {
        Host::new(bytes.to_vec()).expect("host registration")
    }

    fn v1(host: &Host, cleanup: &CleanupObservation) -> String {
        host.report("read", ExitClass::Normal, cleanup, Some(0), None, None)
            .encode()
    }

    fn v2(host: &Host, cleanup: &CleanupObservation) -> String {
        host.report_v2(
            "read",
            ExitV2::Class(ExitClass::Normal),
            cleanup,
            Some(0),
            None,
            None,
        )
        .encode()
    }

    /// F1 and F9: a guest release, a destructor release and a host retirement are different
    /// commits. The v1 report counts them as one released owner; the v2 report keeps them apart and
    /// records why the host retired the owner.
    #[test]
    fn v2_counts_guest_destructor_and_host_releases_apart() {
        let mut guest = host(b"abc");
        guest.release(1, Release::Guest).expect("guest release");
        let mut destructor = host(b"abc");
        destructor
            .release(1, Release::Destructor)
            .expect("destructor release");
        let mut trapped = host(b"abc");
        let trap = trapped.abnormal_exit_cleanup(ExitClass::Trap);
        let mut omitted = host(b"abc");
        let omission = omitted.abnormal_exit_cleanup(ExitClass::Normal);
        let absent = CleanupObservation::AlreadyRetired;
        for (subject, cleanup, counts, reason) in [
            (&guest, &absent, (1, 0, 0), "null"),
            (&destructor, &absent, (0, 1, 0), "null"),
            (&trapped, &trap, (0, 0, 1), r#""trap""#),
            (&omitted, &omission, (0, 0, 1), r#""omitted-release""#),
        ] {
            let report = v2(subject, cleanup);
            let (guest, destructor, host) = counts;
            assert!(
                report.contains(&format!(
                    r#""releases":{{"guest":{guest},"destructor":{destructor},"host_retirement":{host}}}"#
                )),
                "{report}"
            );
            assert!(
                report.contains(&format!(r#""retirement_reason":{reason}"#)),
                "{report}"
            );
            assert!(
                report.contains(r#""cancellation":{"after_reads":null,"requested":false}"#),
                "{report}"
            );
            assert!(v1(subject, cleanup).contains(r#""released_owners":1"#));
        }
    }

    /// F8: the 65th read meets the scope ceiling. The guest sees `invalid-handle`, as it does for a
    /// forged claim, but the v2 report classifies the two refusals apart. The counts sit beside the
    /// bounded trace, so a class survives the trace's truncation.
    #[test]
    fn v2_classifies_scope_exhaustion_apart_from_an_identity_refusal() {
        let mut host = host(b"abc");
        let claim = host.owner.as_ref().expect("owner").handle();
        let mut forged = claim;
        forged.slot = 42;
        assert_eq!(host.read(1, forged, 0, 1), Err("invalid-handle"));
        for _ in 0..64 {
            assert_eq!(host.read(1, claim, 0, 1).expect("admitted read"), b"a");
        }
        assert_eq!(host.read(1, claim, 0, 1), Err("invalid-handle"));
        assert!(host.events_truncated);
        let report = v2(&host, &CleanupObservation::AlreadyRetired);
        assert!(report.contains(r#""identity":1"#), "{report}");
        assert!(report.contains(r#""scope-exhausted":1"#), "{report}");
        assert!(report.contains(r#""region_reads":64"#), "{report}");
        assert!(report.contains(r#""lost_owners":0"#), "{report}");
        assert!(!v1(&host, &CleanupObservation::AlreadyRetired).contains("scope-exhausted"));
    }

    /// #50: a guest that traps before any request leaves a trace whose only event is the host's
    /// retirement. The v2 report names the owner the host registered before guest start, and the
    /// retirement retires exactly that identity, so the registration, not the retirement itself,
    /// says who may be retired. The v1 report is unchanged.
    #[test]
    fn v2_reports_the_registered_owner_a_retirement_retires() {
        let mut trapped = host(b"abc");
        let registered = trapped.context.0;
        let cleanup = trapped.abnormal_exit_cleanup(ExitClass::Trap);
        let report = v2(&trapped, &cleanup);
        assert!(
            report.contains(&format!(r#""registration":{{"identity":{registered}}}"#)),
            "{report}"
        );
        assert!(
            report.contains(&format!(
                r#""kind":"request","operation":"host.retire","identity":{registered}"#
            )),
            "{report}"
        );
        assert!(!v1(&trapped, &cleanup).contains("registration"));
    }

    /// The v1 report keeps exactly its fields, in order, and its event encoding; only the v2 report
    /// adds a refused read's class and spells a destructor drop apart from a guest release.
    #[test]
    fn v1_is_unchanged_and_v2_adds_only_its_own_fields() {
        let refused = HostEvent::Response {
            result: ResponseKind::Refused,
            bytes: None,
            reason: Some("invalid-handle".into()),
            class: Some(RefusalClass::ScopeExhausted),
        };
        assert_eq!(
            refused.encode(Schema::V1).encode(),
            r#"{"kind":"response","result":"refused","bytes":null,"reason":"invalid-handle"}"#
        );
        assert_eq!(
            refused.encode(Schema::V2).encode(),
            r#"{"kind":"response","result":"refused","bytes":null,"reason":"invalid-handle","host_class":"scope-exhausted"}"#
        );
        let drop = HostEvent::Request {
            operation: Operation::Drop,
            identity: 7,
            offset: None,
            length: None,
        };
        assert!(drop
            .encode(Schema::V1)
            .encode()
            .contains(r#""operation":"region.release""#));
        assert!(drop
            .encode(Schema::V2)
            .encode()
            .contains(r#""operation":"region.drop""#));
        let report = host(b"abc").report(
            "read",
            ExitClass::Normal,
            &CleanupObservation::AlreadyRetired,
            Some(0),
            None,
            None,
        );
        let Json::Object(entries) = report else {
            panic!("the v1 report is an object");
        };
        let names = entries.iter().map(|(name, _)| *name).collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "schema",
                "stage",
                "outcome",
                "exit_class",
                "cleanup",
                "fuel_remaining",
                "bytes_hex",
                "error",
                "protected_operations",
                "region_reads",
                "native_memory_access",
                "events",
                "events_truncated",
                "released_owners",
                "live_owners",
                "native_pins",
            ]
        );
    }

    /// RegionProtocol-v1's result schema is `ok` bytes of the requested length, `bounds-reject` or
    /// `invalid-handle`; anything else a guest returns lies outside the model.
    #[test]
    fn results_outside_the_result_schema_are_outside_the_model() {
        let returned = |bytes: Option<Vec<u8>>, reason: Option<&str>| Invocation {
            label: String::new(),
            bytes,
            reason: reason.map(String::from),
            rejected: reason.is_some(),
        };
        assert!(!outside_model(&returned(Some(vec![1, 2]), None), 2));
        assert!(outside_model(&returned(Some(vec![1, 2]), None), 3));
        assert!(!outside_model(&returned(None, Some("bounds-reject")), 2));
        assert!(!outside_model(&returned(None, Some("invalid-handle")), 2));
        assert!(outside_model(&returned(None, Some("timeout")), 2));
    }

    /// RA-CLEAN-02: the host admits every call until the requested number of reads is answered,
    /// and none after. The refusal is an interrupt, so the exit is classified as a cancellation and
    /// the cleanup retires the owner for that reason.
    #[test]
    fn cancellation_revokes_access_once_the_requested_reads_are_answered() {
        let mut cancelled = host(b"abc");
        cancelled.cancel_after_reads = Some(2);
        assert!(cancelled.admit().is_ok());
        assert!(!cancelled.answered_read());
        assert!(cancelled.admit().is_ok(), "one answered read of two");
        assert!(cancelled.answered_read(), "the second answer cancels");
        let refusal = cancelled
            .admit()
            .expect_err("no call is admitted after cancellation");
        assert_eq!(refusal.downcast_ref::<Trap>(), Some(&Trap::Interrupt));
        let exit = ExitClass::of_error(&refusal);
        assert_eq!(exit, ExitClass::Cancelled);
        assert!(matches!(
            cancelled.abnormal_exit_cleanup(exit),
            CleanupObservation::Retired
        ));
        let encoded = cancelled
            .report_v2(
                "adapter-trap",
                ExitV2::Class(exit),
                &CleanupObservation::Retired,
                None,
                None,
                None,
            )
            .encode();
        assert!(encoded.contains(r#""exit_class":"cancelled""#), "{encoded}");
        assert!(
            encoded.contains(r#""cancellation":{"after_reads":2,"requested":true}"#),
            "{encoded}"
        );
        assert!(
            encoded.contains(r#""retirement_reason":"cancelled""#),
            "{encoded}"
        );

        let mut unrequested = host(b"abc");
        assert!(!unrequested.answered_read());
        assert!(unrequested.admit().is_ok(), "no cancellation was requested");
    }

    fn parsed(arguments: &[&str]) -> anyhow::Result<(Options, usize)> {
        let arguments: Vec<OsString> = arguments.iter().map(OsString::from).collect();
        options(&arguments).map(|(options, rest)| (options, rest.len()))
    }

    /// The flags come in any order before the positional arguments. A zero fuel budget or read
    /// count is refused, and a cancellation needs the v2 report, the only one that can show it.
    #[test]
    fn invocation_flags_bound_fuel_and_require_v2_for_cancellation() {
        let positional = ["c.wasm", "00", "0", "0"];
        let with = |flags: &[&str]| parsed(&[flags, &positional[..]].concat());
        assert_eq!(
            with(&[]).expect("defaults"),
            (
                Options {
                    schema: Schema::V1,
                    fuel: FUEL,
                    cancel_after_reads: None
                },
                4
            )
        );
        assert_eq!(
            with(&[
                "--cancel-after-reads",
                "3",
                "--fuel",
                "500",
                "--report",
                "v2"
            ])
            .expect("any order"),
            (
                Options {
                    schema: Schema::V2,
                    fuel: 500,
                    cancel_after_reads: Some(3)
                },
                4
            )
        );
        assert!(
            with(&["--cancel-after-reads", "1"]).is_err(),
            "v1 cannot show it"
        );
        assert!(with(&["--report", "v1", "--cancel-after-reads", "1"]).is_err());
        assert!(with(&["--report", "v2", "--cancel-after-reads", "0"]).is_err());
        assert!(with(&["--fuel", "0"]).is_err());
        assert!(with(&["--fuel", "10000001"]).is_err());
        assert!(with(&["--fuel", "10000000"]).is_ok());
    }
}
