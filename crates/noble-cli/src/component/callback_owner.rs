//! A host-selected callback result is checked against retained ownership before
//! it can become a Wasmtime resource or a trusted guest value.
use anyhow::{bail, ensure, Context as _, Result};
use noble_kernel::{resources as ownership, types::ResourceKind};
use std::ffi::OsString;
use wasmtime::{
    component::{types::ComponentItem, Component, Linker, Resource, ResourceAny, ResourceType, Val},
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder,
};

use crate::workflow::encoding::{self, Json};

const INTERFACE: &str = "noble-test:callback-owner/tokens@1.0.0";
const TOKEN: ResourceKind = ResourceKind(1);
const OTHER: ResourceKind = ResourceKind(2);
const RIGHTS: ownership::Rights = ownership::Rights(1);
struct Token;

#[derive(Clone, Copy)]
enum Mode { Authentic, StaleGeneration, WrongKind, WrongContext }
impl Mode {
    fn parse(value: &OsString) -> Result<Self> {
        match value.to_str() {
            Some("authentic") => Ok(Self::Authentic),
            Some("stale-generation") => Ok(Self::StaleGeneration),
            Some("wrong-kind") => Ok(Self::WrongKind),
            Some("wrong-context") => Ok(Self::WrongContext),
            _ => bail!("MODE must be authentic, stale-generation, wrong-kind or wrong-context"),
        }
    }
}

struct Host {
    limits: StoreLimits,
    table: ownership::Table,
    required: ownership::Requirement,
    owner: Option<ownership::Owner>,
    wrong_kind: Option<ownership::Owner>,
    wrong_context: Option<ownership::Owner>,
    stale: Option<ownership::Handle>,
    mode: Mode,
    callback_refused: bool,
    callback_rejection: Option<&'static str>,
    published: bool,
    trusted_values_created: u64,
    guest_requests: u64,
    protected_operations: u64,
    releases: u64,
    fixture_releases: u64,
}

impl Host {
    fn new(mode: Mode) -> Result<Self> {
        let identity = super::identity::reserve(if matches!(mode, Mode::WrongContext) { 2 } else { 1 })?;
        let required = ownership::Requirement { context: ownership::Context(identity), kind: TOKEN, rights: RIGHTS };
        let slots = if matches!(mode, Mode::WrongKind | Mode::WrongContext) { 2 } else { 1 };
        let mut table = ownership::Table::new(ownership::TableId(identity), ownership::Limits {
            slots, pins: 0, owners_per_context: slots,
            generations: if matches!(mode, Mode::Authentic) { 1 } else { 2 }, scopes: 1,
        }).map_err(|error| anyhow::anyhow!("resource table: {error:?}"))?;
        // Allocate only the selected hostile fixture. A stale generation is
        // truly retired before the pending owner's registration reuses it.
        let stale = if matches!(mode, Mode::StaleGeneration) {
            let retired = table.register(required).map_err(|error| anyhow::anyhow!("stale fixture: {error:?}"))?;
            let claim = retired.handle();
            table.release(retired, required)
                .map_err(|error| anyhow::anyhow!("retire stale fixture: {:?}", error.error))?;
            Some(claim)
        } else { None };
        let owner = table.register(required).map_err(|error| anyhow::anyhow!("token registration: {error:?}"))?;
        let wrong_kind = if matches!(mode, Mode::WrongKind) {
            Some(table.register(ownership::Requirement { kind: OTHER, ..required })
                .map_err(|error| anyhow::anyhow!("wrong-kind fixture: {error:?}"))?)
        } else { None };
        let wrong_context = if matches!(mode, Mode::WrongContext) {
            Some(table.register(ownership::Requirement {
                context: ownership::Context(identity.checked_add(1)
                    .context("component invocation identity exhausted")?), ..required
            }).map_err(|error| anyhow::anyhow!("foreign-context fixture: {error:?}"))?)
        } else { None };
        if let Some(stale) = stale {
            ensure!(stale.slot == owner.handle().slot && stale.generation != owner.handle().generation,
                "stale fixture did not reuse the retired owner slot");
        }
        Ok(Self {
            limits: StoreLimitsBuilder::new().memory_size(4_194_304).instances(16).tables(16).build(),
            table, required, owner: Some(owner), wrong_kind, wrong_context,
            stale, mode, callback_refused: false, callback_rejection: None, published: false,
            trusted_values_created: 0, guest_requests: 0, protected_operations: 0,
            releases: 0, fixture_releases: u64::from(stale.is_some()),
        })
    }

    fn callback_claim(&mut self) -> Result<ownership::Handle> {
        self.guest_requests += 1;
        let authentic = self.owner.as_ref().context("missing retained token")?.handle();
        let claim = match self.mode {
            Mode::Authentic => authentic,
            Mode::StaleGeneration => self.stale.context("missing retired fixture")?,
            Mode::WrongKind => self.wrong_kind.as_ref().context("missing wrong-kind fixture")?.handle(),
            Mode::WrongContext => self.wrong_context.as_ref().context("missing foreign fixture")?.handle(),
        };
        // Table::validate uses retained records, not Wasmtime's resource type.
        // Even another otherwise valid owner is never the pending callback's
        // selected obligation. No resource conversion or transfer precedes this.
        let rejection = match self.table.validate(claim, self.required) {
            Ok(_) if claim == authentic && !self.published => None,
            Err(ownership::Error::WrongGeneration) => Some("wrong-generation"),
            Err(ownership::Error::WrongKind) => Some("wrong-kind"),
            Err(ownership::Error::WrongContext) => Some("wrong-context"),
            Ok(_) | Err(_) => Some("unbound-owner"),
        };
        if let Some(reason) = rejection {
            self.callback_refused = true;
            self.callback_rejection = Some(reason);
            bail!("invalid-result: callback returned an unbound owner");
        }
        Ok(claim)
    }

    fn consume(&mut self, rep: u32) -> Result<i64> {
        self.guest_requests += 1;
        ensure!(rep == 1 && self.published, "invalid token representation or custody");
        self.release_pending(rep)?;
        self.protected_operations += 1;
        Ok(42)
    }

    fn release_pending(&mut self, rep: u32) -> Result<()> {
        ensure!(rep == 1, "invalid token representation");
        let owner = self.owner.take().context("token already released")?;
        match self.table.release(owner, self.required) {
            Ok(_) => { self.releases += 1; Ok(()) }
            Err(rejected) => {
                self.owner = Some(rejected.input);
                bail!("token release refused: {:?}", rejected.error)
            }
        }
    }

    fn cleanup(&mut self) -> Result<()> {
        if self.owner.is_some() { self.release_pending(1)?; }
        for (owner, required) in [
            (self.wrong_kind.take(), ownership::Requirement { kind: OTHER, ..self.required }),
            (self.wrong_context.take(), ownership::Requirement {
                context: ownership::Context(self.required.context.0.checked_add(1)
                    .context("component invocation identity exhausted")?), ..self.required
            }),
        ] {
            if let Some(owner) = owner {
                self.table.release(owner, required)
                    .map_err(|rejected| anyhow::anyhow!("fixture cleanup refused: {:?}", rejected.error))?;
                self.fixture_releases += 1;
            }
        }
        Ok(())
    }

    fn report(&self, outcome: &str, guest_value: Option<i64>) -> Json {
        let observation = self.table.observation();
        encoding::object([
            ("schema", encoding::string("noble-callback-owner/v1")),
            ("stage", encoding::string("callback")),
            ("outcome", encoding::string(outcome)),
            ("callback_rejection", self.callback_rejection.map_or(Json::Null, encoding::string)),
            ("guest_value", guest_value.map_or(Json::Null, |value| Json::Number(value as u64))),
            ("guest_requests", Json::Number(self.guest_requests)),
            ("trusted_values_created", Json::Number(self.trusted_values_created)),
            ("protected_operations", Json::Number(self.protected_operations)),
            ("released_owners", Json::Number(self.releases)),
            ("fixture_releases", Json::Number(self.fixture_releases)),
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
                        ComponentItem::Resource(_) if function == "token" => {
                            instance.resource(function, ResourceType::host::<Token>(), |mut store, rep| {
                                if store.data().owner.is_some() { store.data_mut().release_pending(rep)?; }
                                Ok(())
                            })?;
                        }
                        ComponentItem::ComponentFunc(_) if function == "issue" => {
                            instance.func_new(function, |mut store, _, params, results| {
                                ensure!(params.is_empty(), "invalid issue arguments");
                                let [slot] = results else { bail!("invalid issue result"); };
                                store.data_mut().callback_claim()?;
                                // This is the actual imported callback's result slot. A
                                // malformed claim never reaches ResourceAny/Val::Resource.
                                let resource = ResourceAny::try_from_resource(
                                    Resource::<Token>::new_own(1), &mut store)?;
                                *slot = Val::Resource(resource);
                                store.data_mut().published = true;
                                store.data_mut().trusted_values_created += 1;
                                Ok(())
                            })?;
                        }
                        ComponentItem::ComponentFunc(_) if function == "consume" => {
                            instance.func_new(function, |mut store, _, params, results| {
                                let [Val::Resource(resource)] = params else { bail!("invalid consume argument"); };
                                let token = resource.try_into_resource::<Token>(&mut store)?;
                                ensure!(token.owned(), "consume requires owned token");
                                let [slot] = results else { bail!("invalid consume result"); };
                                *slot = Val::S64(store.data_mut().consume(token.rep())?);
                                Ok(())
                            })?;
                        }
                        ComponentItem::Type(_) => {}
                        _ => bail!("unapproved callback import {name}#{function}"),
                    }
                }
            }
            ComponentItem::Type(_) => {}
            _ => bail!("unapproved component import {name}"),
        }
    }
    Ok(())
}

fn execute(arguments: &[OsString]) -> Result<Json> {
    let [_, _, component_path, mode, wit_path, world_name, sources @ ..] = arguments else {
        bail!("usage: noble component callback-owner COMPONENT MODE WIT WORLD EXPORT=SOURCE ...")
    };
    ensure!(!sources.is_empty(), "independent Noble source recipe required");
    let mode = Mode::parse(mode)?;
    let component_bytes = crate::workflow::read_bounded(
        std::path::Path::new(component_path), 4_194_304, "callback-owner-component")
        .map_err(|error| anyhow::anyhow!("component input: {}", error.json().encode()))?;
    let wit = crate::workflow::read_bounded(std::path::Path::new(wit_path), 65_536, "callback-owner-wit")
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    let world = noble_contracts::component::World::parse(
        &wit, world_name.to_str().context("world name must be UTF-8")?, crate::core::SOURCE_LIMITS)
        .map_err(super::report::diagnostic)
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    ensure!(world.identity() == "noble-test:callback-owner/bounded@1.0.0", "unapproved callback world");
    let sources = super::sources(sources)
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    let prepared = sources.iter().map(|source| {
        world.prepare_export(&source.name, &source.bytes, crate::core::SOURCE_LIMITS)
            .map_err(super::report::diagnostic)
    }).collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    let artifact = noble_wasm::component::compile(&world, &prepared)
        .map_err(super::report::backend)
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    let rebuilt = super::artifacts::assemble(&world, artifact.wat())
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    ensure!(component_bytes == rebuilt.binary,
        "component differs from independent host-selected Noble source recipe");
    let mut config = Config::new();
    config.wasm_component_model(true).consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, component_bytes)?;
    let mut linker = Linker::new(&engine);
    link(&engine, &component, &mut linker)?;
    let mut store = Store::new(&engine, Host::new(mode)?);
    store.limiter(|host| &mut host.limits);
    store.set_fuel(10_000_000)?;
    let instance = linker.instantiate(&mut store, &component)?;
    let function = instance.get_func(&mut store, "check").context("missing check export")?;
    let mut results = [Val::Bool(false)];
    let invoked = (|| -> Result<i64> {
        function.call(&mut store, &[], &mut results)?;
        function.post_return(&mut store)?;
        let [Val::S64(value)] = results else { bail!("invalid check result"); };
        Ok(value)
    })();
    let refused = store.data().callback_refused;
    let (outcome, guest_value) = match invoked {
        Ok(42) if !refused && store.data().protected_operations == 1 => ("accepted", Some(42)),
        Err(_) if refused => ("invalid-result", None),
        Ok(_) | Err(_) => ("adapter-trap", None),
    };
    store.data_mut().cleanup()?;
    Ok(store.data().report(outcome, guest_value))
}

pub(super) fn run(arguments: &[OsString]) -> std::process::ExitCode {
    match execute(arguments) {
        Ok(report) => { println!("{}", report.encode()); std::process::ExitCode::SUCCESS }
        Err(error) => {
            eprintln!("callback owner adapter: {error:#}");
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_real_owners_are_refused_at_callback_ingress() {
        for mode in [Mode::StaleGeneration, Mode::WrongKind, Mode::WrongContext] {
            let mut host = Host::new(mode).unwrap();
            let authentic = host.owner.as_ref().unwrap().handle();
            match mode {
                Mode::StaleGeneration => {
                    let stale = host.stale.unwrap();
                    assert_eq!(stale.slot, authentic.slot);
                    assert!(stale.generation < authentic.generation);
                }
                Mode::WrongKind => {
                    let other = host.wrong_kind.as_ref().unwrap().handle();
                    assert_eq!(other.kind, OTHER);
                    assert_eq!(other.rights, RIGHTS);
                    assert!(host.table.validate(other, ownership::Requirement {
                        kind: OTHER, ..host.required
                    }).is_ok());
                }
                Mode::WrongContext => {
                    let other = host.wrong_context.as_ref().unwrap().handle();
                    assert_ne!(other.context, authentic.context);
                    assert_eq!(other.rights, RIGHTS);
                    assert!(host.table.validate(other, ownership::Requirement {
                        context: other.context, ..host.required
                    }).is_ok());
                }
                Mode::Authentic => unreachable!(),
            }
            assert!(host.callback_claim().is_err());
            assert!(host.callback_refused);
            assert_eq!(host.callback_rejection, Some(match mode {
                Mode::StaleGeneration => "wrong-generation",
                Mode::WrongKind => "wrong-kind",
                Mode::WrongContext => "wrong-context",
                Mode::Authentic => unreachable!(),
            }));
            assert_eq!(host.owner.as_ref().unwrap().handle(), authentic);
            assert_eq!(host.trusted_values_created, 0);
            host.cleanup().unwrap();
            host.cleanup().unwrap();
            assert_eq!(host.releases, 1);
            assert_eq!(host.table.observation().live, 0);
        }
    }

    #[test]
    fn callback_preflight_does_not_accept_missing_owner_right() {
        let mut host = Host::new(Mode::Authentic).unwrap();
        let mut copied = host.owner.as_ref().unwrap().handle();
        copied.rights = ownership::Rights(0);
        assert!(matches!(host.table.validate(copied, host.required), Err(ownership::Error::WrongRights)));
        host.cleanup().unwrap();
    }

    #[test]
    fn live_stores_with_equal_slots_and_generations_never_share_identity() {
        let mut first = Host::new(Mode::Authentic).unwrap();
        let mut second = Host::new(Mode::Authentic).unwrap();
        let original = first.owner.as_ref().unwrap().handle();
        let other = second.owner.as_ref().unwrap().handle();
        assert_eq!((original.slot, original.generation), (other.slot, other.generation));
        assert_ne!(original.table, other.table);
        assert_ne!(original.context, other.context);
        assert!(second.table.validate(original, second.required).is_err());
        first.cleanup().unwrap();
        second.cleanup().unwrap();
    }
}
