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

/// How a bounded-region invocation ended, as far as the host can actually observe. The three
/// abnormal classes stay distinct instead of collapsing into one `adapter-trap` label: an exhausted
/// fuel budget is a host-imposed boundary, a guest trap is the guest's own abnormal exit, and an
/// adapter fault (missing export, malformed payload, usage) is neither. Nothing here infers a finer
/// guest outcome from a report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExitClass {
    Normal,
    Rejected,
    Trap,
    QuotaExhausted,
    AdapterInvalid,
}

impl ExitClass {
    const fn label(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Rejected => "rejected",
            Self::Trap => "trap",
            Self::QuotaExhausted => "quota-exhausted",
            Self::AdapterInvalid => "adapter-invalid",
        }
    }

    /// Classify an invocation error by what it actually is. Anything that is not a Wasmtime trap is
    /// an adapter fault, so an adapter bug can never be reported as a guest trap.
    fn of_error(error: &anyhow::Error) -> Self {
        match error.downcast_ref::<Trap>() {
            Some(Trap::OutOfFuel) => Self::QuotaExhausted,
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
        operation: &'static str,
        identity: u64,
        offset: Option<Offset>,
        length: Option<Offset>,
    },
    /// The host's answer to the request it follows: a value length or a refusal reason.
    Response {
        result: ResponseKind,
        bytes: Option<usize>,
        reason: Option<String>,
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
    fn encode(&self) -> Json {
        match self {
            Self::Request {
                operation,
                identity,
                offset,
                length,
            } => encoding::object([
                ("kind", encoding::string("request")),
                ("operation", encoding::string(operation)),
                ("identity", Json::Number(*identity)),
                ("offset", offset.map_or(Json::Null, Offset::encode)),
                ("length", length.map_or(Json::Null, Offset::encode)),
            ]),
            Self::Response {
                result,
                bytes,
                reason,
            } => encoding::object([
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
            ]),
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
    events: Vec<HostEvent>,
    events_truncated: bool,
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
            events: Vec::new(),
            events_truncated: false,
        })
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
            operation: "region.read",
            identity: claim.table.0,
            offset: Some(Offset(offset)),
            length: Some(Offset(length)),
        });
        let answered = self.read_region(rep, claim, offset, length);
        self.record(match &answered {
            Ok(bytes) => HostEvent::Response {
                result: ResponseKind::Value,
                bytes: Some(bytes.len()),
                reason: None,
            },
            Err(reason) => HostEvent::Response {
                result: ResponseKind::Refused,
                bytes: None,
                reason: Some((*reason).to_owned()),
            },
        });
        answered
    }

    fn read_region(
        &mut self,
        rep: u32,
        claim: ownership::Handle,
        offset: i64,
        length: i64,
    ) -> Result<Vec<u8>, &'static str> {
        if rep != 1 {
            return Err("invalid-handle");
        }
        let actual = self.owner.as_ref().ok_or("invalid-handle")?.handle();
        let required = self.required();
        self.table
            .validate(claim, required)
            .map_err(|_| "invalid-handle")?;
        if claim != actual {
            return Err("invalid-handle");
        }
        let end = offset.checked_add(length).ok_or("bounds-reject")?;
        let start = usize::try_from(offset).map_err(|_| "bounds-reject")?;
        let count = usize::try_from(length).map_err(|_| "bounds-reject")?;
        let finish = usize::try_from(end).map_err(|_| "bounds-reject")?;
        if start > self.bytes.len() || finish > self.bytes.len() || count > MAX_REGION_BYTES {
            return Err("bounds-reject");
        }
        let owner = self.owner.take().ok_or("invalid-handle")?;
        let admitted = match self.table.begin(owner, required) {
            Ok(admitted) => admitted,
            Err(rejection) => {
                self.owner = Some(rejection.input);
                return Err("invalid-handle");
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
        let completed = self
            .table
            .complete(
                admitted.borrow.scope(),
                if authorized {
                    ownership::Completion::Success
                } else {
                    ownership::Completion::DomainError
                },
            )
            .map_err(|_| "invalid-handle")?;
        self.owner = completed.owner;
        match output {
            Some(bytes) => {
                self.protected_operations += 1;
                self.record(HostEvent::Commit {
                    identity: claim.table.0,
                });
                Ok(bytes)
            }
            None => Err("invalid-handle"),
        }
    }

    fn release(&mut self, rep: u32) -> Result<()> {
        if rep != 1 {
            bail!("invalid region representation");
        }
        let required = self.required();
        let owner = self.owner.take().context("region already released")?;
        match self.table.release(owner, required) {
            Ok(_) => {
                self.releases += 1;
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
    fn abnormal_exit_cleanup(&mut self) -> CleanupObservation {
        if self.owner.is_none() {
            return CleanupObservation::AlreadyRetired;
        }
        self.record(HostEvent::Request {
            operation: "host.retire",
            identity: self.context.0,
            offset: None,
            length: None,
        });
        let retired = self.release(1);
        self.record(match &retired {
            Ok(()) => HostEvent::Response {
                result: ResponseKind::Value,
                bytes: None,
                reason: None,
            },
            Err(error) => HostEvent::Response {
                result: ResponseKind::Refused,
                bytes: None,
                reason: Some(format!("{error:#}")),
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
                Json::Array(self.events.iter().map(HostEvent::encode).collect()),
            ),
            ("events_truncated", Json::Bool(self.events_truncated)),
            ("released_owners", Json::Number(self.releases)),
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
                                    let identity = store.data().context.0;
                                    store.data_mut().record(HostEvent::Request {
                                        operation: "region.release",
                                        identity,
                                        offset: None,
                                        length: None,
                                    });
                                    let released = if store.data().owner.is_some() {
                                        store.data_mut().release(rep)
                                    } else {
                                        Ok(())
                                    };
                                    store.data_mut().record(match &released {
                                        Ok(()) => HostEvent::Response {
                                            result: ResponseKind::Value,
                                            bytes: None,
                                            reason: None,
                                        },
                                        Err(error) => HostEvent::Response {
                                            result: ResponseKind::Refused,
                                            bytes: None,
                                            reason: Some(format!("{error:#}")),
                                        },
                                    });
                                    released
                                },
                            )?;
                        }
                        ComponentItem::ComponentFunc(_) if function == "[method]region.read" => {
                            instance.func_new(function, |mut store, _, params, results| {
                                crate::safety_observe::event("guest-request");
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
                                    operation: "region.release",
                                    identity,
                                    offset: None,
                                    length: None,
                                });
                                let released = store.data_mut().release(region.rep());
                                store.data_mut().record(match &released {
                                    Ok(()) => HostEvent::Response {
                                        result: ResponseKind::Value,
                                        bytes: None,
                                        reason: None,
                                    },
                                    Err(error) => HostEvent::Response {
                                        result: ResponseKind::Refused,
                                        bytes: None,
                                        reason: Some(format!("{error:#}")),
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
                            operation: "region.release",
                            identity,
                            offset: None,
                            length: None,
                        });
                        let released = if store.data().owner.is_some() {
                            store.data_mut().release(rep)
                        } else {
                            Ok(())
                        };
                        store.data_mut().record(match &released {
                            Ok(()) => HostEvent::Response {
                                result: ResponseKind::Value,
                                bytes: None,
                                reason: None,
                            },
                            Err(error) => HostEvent::Response {
                                result: ResponseKind::Refused,
                                bytes: None,
                                reason: Some(format!("{error:#}")),
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

fn execute(arguments: &[OsString]) -> Result<Json> {
    let [_, _, path, hex, offset, length] = arguments else {
        bail!("usage: noble component read-region COMPONENT HOST_BUFFER_HEX OFFSET LENGTH")
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
    config.wasm_component_model(true).consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, component_bytes)?;
    let mut linker = Linker::new(&engine);
    link(&engine, &component, &mut linker)?;
    let host = Host::new(bytes)?;
    let mut store = Store::new(&engine, host);
    store.limiter(|host| &mut host.limits);
    store.set_fuel(10_000_000)?;
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
            Some(error.to_string()),
        ),
    };
    let cleanup = store.data_mut().abnormal_exit_cleanup();
    Ok(store.data().report(
        &outcome,
        exit_class,
        &cleanup,
        fuel_remaining,
        returned.as_deref(),
        error.as_deref(),
    ))
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
        host.release(1).unwrap();
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
        host.release(1).unwrap();
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
        second.release(1).unwrap();
    }

    #[test]
    fn double_release_does_not_retire_the_owner_twice() {
        let mut host = Host::new(vec![0, 1, 2]).unwrap();
        host.release(1).unwrap();
        assert_eq!(host.releases, 1);
        assert_eq!(host.table.observation().live, 0);
        assert!(host.release(1).is_err());
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
        assert_eq!(host.abnormal_exit_cleanup(), CleanupObservation::Retired);
        assert_eq!(host.releases, 1);
        assert_eq!(host.table.observation().live, 0);
        // A second pass has no retained owner left, so nothing is attributed to us.
        assert_eq!(
            host.abnormal_exit_cleanup(),
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
        match host.abnormal_exit_cleanup() {
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
        assert_eq!(host.abnormal_exit_cleanup(), CleanupObservation::Retired);
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
        assert!(host.release(1).is_err());
        assert_eq!(host.releases, 0);
        assert_eq!(host.table.observation().live, 1);
        assert_eq!(host.owner.as_ref().unwrap().handle(), original);

        host.context = context;
        host.release(1).unwrap();
        assert_eq!(host.releases, 1);
        assert_eq!(host.table.observation().live, 0);
    }
}

#[cfg(test)]
mod ordered_trace_tests {
    use super::{Host, HostEvent, ResponseKind, MAX_EVENTS};

    fn host(bytes: &[u8]) -> Host {
        Host::new(bytes.to_vec()).expect("host registration")
    }

    fn kinds(host: &Host) -> Vec<&'static str> {
        host.events
            .iter()
            .map(|event| match event {
                HostEvent::Request { operation, .. } => operation,
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
                operation: "region.read",
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
            }) => {
                assert_eq!(*result, ResponseKind::Refused);
                assert_eq!(*bytes, None);
                assert_eq!(reason.as_deref(), Some("bounds-reject"));
            }
            other => panic!("expected a refused response: {other:?}"),
        }
    }

    #[test]
    fn host_retirement_is_its_own_request_and_never_a_guest_release() {
        let mut host = host(b"abc");
        assert_eq!(
            host.abnormal_exit_cleanup(),
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
                .encode()
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
