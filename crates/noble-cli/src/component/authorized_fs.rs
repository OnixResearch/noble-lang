//! An invocation-scoped, exact-file capability for the `authorized-fs` world.
//! The trusted invoker opens the file before guest execution. Guest path text
//! is only an exact key; it is never joined to a native filesystem path.
use anyhow::{bail, ensure, Context as _, Result};
use noble_kernel::{resources as ownership, types::ResourceKind};
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Seek};
use wasmtime::{
    component::{types::ComponentItem, Component, Linker, Val},
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder,
};

use crate::workflow::encoding::{self, Json};

const INTERFACE: &str = "noble-test:authorized-fs/fs@1.0.0";
const KIND: ResourceKind = ResourceKind(1);
const READ: ownership::Rights = ownership::Rights(1);
const NO_RIGHTS: ownership::Rights = ownership::Rights(0);
const MAX_BYTES: usize = 4096;
const MAX_REQUESTS: usize = 4096;

/// An exact one-entry Directory namespace, NOT an OS directory or a general
/// filesystem preopen. The invoker independently chooses the already-opened
/// regular file; guest strings can select only the fixed `main.rs` entry.
struct Directory {
    main_rs: File,
}

impl Directory {
    fn new(main_rs: File) -> Result<Self> {
        if !main_rs.metadata()?.is_file() { bail!("preopened directory entry must be regular"); }
        Ok(Self { main_rs })
    }

    fn read(&mut self, path: &str) -> Result<Vec<u8>, &'static str> {
        if path != "main.rs" { return Err("invalid-path"); }
        // A bounded physical read, including if the host's file changes after
        // preopen. No guest path is ever converted to a native path.
        let mut bytes = Vec::new();
        match self.main_rs.rewind().and_then(|()|
            self.main_rs.by_ref().take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)) {
            Ok(_) if bytes.len() <= MAX_BYTES => Ok(bytes),
            Ok(_) => Err("bounds-reject"),
            Err(_) => Err("read-error"),
        }
    }
}

struct Host {
    limits: StoreLimits,
    context: ownership::Context,
    table: ownership::Table,
    owner: Option<ownership::Owner>,
    directory: Directory,
    request_trace: Vec<&'static str>,
    authorization_denied: bool,
    protected_operations: u64,
    releases: u64,
}

impl Host {
    fn new(directory: Directory, read_right: bool) -> Result<Self> {
        let identity = super::identity::reserve(1)?;
        let context = ownership::Context(identity);
        let mut table = ownership::Table::new(ownership::TableId(identity), ownership::Limits {
            slots: 1, pins: 1, owners_per_context: 1, generations: 1, scopes: 2,
        }).map_err(|error| anyhow::anyhow!("resource table: {error:?}"))?;
        let owner = table.register(ownership::Requirement {
            context, kind: KIND, rights: if read_right { READ } else { NO_RIGHTS },
        }).map_err(|error| anyhow::anyhow!("directory registration: {error:?}"))?;
        Ok(Self {
            limits: StoreLimitsBuilder::new().memory_size(4_194_304).instances(16).tables(16).build(),
            context, table, owner: Some(owner), directory,
            request_trace: Vec::new(), authorization_denied: false,
            protected_operations: 0, releases: 0,
        })
    }

    fn read(&mut self, claim: ownership::Handle, path: &str) -> Result<Vec<u8>, &'static str> {
        if self.request_trace.len() == MAX_REQUESTS { return Err("request-limit"); }
        self.request_trace.push("fs.read");
        // This host-selected grant is independent of the guest's declared effect.
        // Refuse missing READ at authorization, before any native file read.
        let actual = self.owner.as_ref().ok_or("invalid-handle")?.handle();
        if claim != actual { return Err("invalid-handle"); }
        let required = ownership::Requirement { context: self.context, kind: KIND, rights: READ };
        match self.table.validate(claim, required) {
            Ok(_) => {}
            Err(ownership::Error::WrongRights) => {
                self.authorization_denied = true;
                return Err("denied");
            }
            Err(_) => return Err("invalid-handle"),
        }
        if path != "main.rs" { return Err("invalid-path"); }
        let owner = self.owner.take().ok_or("invalid-handle")?;
        let admitted = match self.table.begin(owner, required) {
            Ok(admitted) => admitted,
            Err(rejected) => { self.owner = Some(rejected.input); return Err("invalid-handle"); }
        };
        let accessible = self.table.native_access(&admitted.borrow).is_ok();
        let result = if accessible {
            let read = self.directory.read(path);
            self.protected_operations += 1;
            read
        } else {
            Err("invalid-handle")
        };
        let completed = self.table.complete(admitted.borrow.scope(),
            if result.is_ok() { ownership::Completion::Success } else { ownership::Completion::DomainError })
            .map_err(|_| "invalid-handle")?;
        self.owner = completed.owner;
        result
    }

    fn release(&mut self) -> Result<()> {
        let owner = self.owner.take().context("directory already released")?;
        let required = ownership::Requirement { context: self.context, kind: KIND, rights: NO_RIGHTS };
        match self.table.release(owner, required) {
            Ok(_) => { self.releases += 1; Ok(()) }
            Err(rejected) => {
                self.owner = Some(rejected.input);
                bail!("directory release refused: {:?}", rejected.error)
            }
        }
    }

    fn report(&self, outcome: &str, bytes: Option<&[u8]>) -> Json {
        let observation = self.table.observation();
        // Guest Result payloads are untrusted data. Only the host's retained
        // Table READ decision can label an authorization-stage denial.
        let classified = if self.authorization_denied {
            "denied"
        } else if outcome == "denied" {
            "guest-returned-denied"
        } else {
            outcome
        };
        encoding::object([
            ("schema", encoding::string("noble-authorized-fs/v1")),
            ("stage", encoding::string(if self.authorization_denied { "authorization" } else { "adapter" })),
            ("outcome", encoding::string(classified)),
            ("context", encoding::string(format!("invocation-{}", self.context.0))),
            ("guest_requests", Json::Number(self.request_trace.len() as u64)),
            ("request_trace", Json::Array(self.request_trace.iter().map(encoding::string).collect())),
            ("protected_operations", Json::Number(self.protected_operations)),
            ("bytes_hex", if self.authorization_denied { Json::Null } else {
                bytes.map_or(Json::Null, |value| encoding::string(encoding::hex(value)))
            }),
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
                        ComponentItem::ComponentFunc(_) if function == "read" => {
                            instance.func_new(function, |mut store, _, params, results| {
                                let [Val::String(path)] = params else { bail!("invalid fs.read arguments") };
                                let claim = store.data().owner.as_ref().context("missing directory")?.handle();
                                let value = store.data_mut().read(claim, path);
                                let [slot] = results else { bail!("invalid fs.read result"); };
                                *slot = match value {
                                    Ok(bytes) => Val::Result(Ok(Some(Box::new(Val::List(
                                        bytes.into_iter().map(Val::U8).collect()
                                    ))))),
                                    Err(reason) => Val::Result(Err(Some(Box::new(Val::String(reason.into()))))),
                                };
                                Ok(())
                            })?;
                        }
                        ComponentItem::Type(_) => {}
                        _ => bail!("unapproved filesystem import {name}#{function}"),
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
    let [_, _, component_path, file_path, grant, wit_path, world_name, sources @ ..] = arguments else {
        bail!("usage: noble component read-fs COMPONENT HOST_PREOPENED_FILE READ_RIGHT WIT WORLD EXPORT=SOURCE ...")
    };
    ensure!(!sources.is_empty(), "independent Noble source recipe required");
    let grant = match grant.to_str() {
        Some("read") => true,
        Some("none") => false,
        _ => bail!("READ_RIGHT must be read or none"),
    };
    let component_bytes = crate::workflow::read_bounded(
        std::path::Path::new(component_path), 4_194_304, "authorized-fs-component")
        .map_err(|error| anyhow::anyhow!("component input: {}", error.json().encode()))?;
    let wit = crate::workflow::read_bounded(std::path::Path::new(wit_path), 65_536,
        "authorized-fs-wit").map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    let world_name = world_name.to_str().context("world name must be UTF-8")?;
    let world = noble_contracts::component::World::parse(
        &wit, world_name, crate::core::SOURCE_LIMITS)
        .map_err(super::report::diagnostic)
        .map_err(|error| anyhow::anyhow!("source recipe: {}", error.json().encode()))?;
    ensure!(world.identity() == "noble-test:authorized-fs/bounded@1.0.0",
        "unapproved filesystem world");
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
    // No file is opened or right granted until exact component correspondence.
    let file = File::open(file_path).context("host preopen failed")?;
    let mut config = Config::new();
    config.wasm_component_model(true).consume_fuel(true);
    let engine = Engine::new(&config)?;
    let component = Component::new(&engine, component_bytes)?;
    let mut linker = Linker::new(&engine);
    link(&engine, &component, &mut linker)?;
    let mut store = Store::new(&engine, Host::new(Directory::new(file)?, grant)?);
    store.limiter(|host| &mut host.limits);
    store.set_fuel(10_000_000)?;
    let instance = linker.instantiate(&mut store, &component)?;
    let function = instance.get_func(&mut store, "read-file").context("missing read-file export")?;
    let mut results = [Val::Bool(false)];
    let invoked = (|| -> Result<(String, Option<Vec<u8>>)> {
        function.call(&mut store, &[], &mut results)?;
        function.post_return(&mut store)?;
        let [Val::Result(result)] = &results else { bail!("invalid read-file result"); };
        match result {
            Ok(Some(value)) => {
                let Val::List(list) = value.as_ref() else { bail!("invalid byte payload"); };
                let bytes = list.iter().map(|value| match value {
                    Val::U8(byte) => Ok(*byte), _ => bail!("invalid byte result")
                }).collect::<Result<Vec<_>>>()?;
                Ok(("read".into(), Some(bytes)))
            }
            Err(Some(value)) => {
                let Val::String(reason) = value.as_ref() else { bail!("invalid refusal payload"); };
                Ok((reason.clone(), None))
            }
            _ => bail!("invalid read-file variant"),
        }
    })();
    let (outcome, bytes) = match invoked {
        Ok(result) => result,
        Err(_) => ("adapter-trap".into(), None),
    };
    store.data_mut().release()?;
    Ok(store.data().report(&outcome, bytes.as_deref()))
}

pub(super) fn run(arguments: &[OsString]) -> std::process::ExitCode {
    match execute(arguments) {
        Ok(report) => { println!("{}", report.encode()); std::process::ExitCode::SUCCESS }
        Err(error) => {
            eprintln!("authorized filesystem adapter: {error:#}");
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek, Write};

    #[test]
    fn authorization_and_hostile_claims_never_read() {
        let path = std::env::temp_dir().join(format!("noble-fs-{}-{}", std::process::id(),
            super::super::identity::reserve(1).unwrap()));
        let mut temporary = std::fs::OpenOptions::new().read(true).write(true)
            .create_new(true).open(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        temporary.write_all(b"safe\n").unwrap();
        temporary.rewind().unwrap();
        let file = temporary.try_clone().unwrap();
        let mut denied = Host::new(Directory::new(file).unwrap(), false).unwrap();
        let claim = denied.owner.as_ref().unwrap().handle();
        assert_eq!(denied.read(claim, "main.rs"), Err("denied"));
        assert!(denied.authorization_denied);
        assert_eq!(denied.protected_operations, 0);
        let host_denial = denied.report("read", Some(b"guest-forgery"));
        let Json::Object(fields) = host_denial else { panic!("report must be an object") };
        assert!(fields.iter().any(|(name, value)| *name == "stage"
            && matches!(value, Json::String(stage) if stage == "authorization")));
        assert!(fields.iter().any(|(name, value)| *name == "outcome"
            && matches!(value, Json::String(outcome) if outcome == "denied")));
        assert!(fields.iter().any(|(name, value)| *name == "bytes_hex"
            && matches!(value, Json::Null)));
        denied.release().unwrap();
        let mut granted = Host::new(Directory::new(temporary).unwrap(), true).unwrap();
        let spoof = granted.report("denied", None);
        let Json::Object(fields) = spoof else { panic!("report must be an object") };
        assert!(fields.iter().any(|(name, value)| *name == "stage"
            && matches!(value, Json::String(stage) if stage == "adapter")));
        assert!(fields.iter().any(|(name, value)| *name == "outcome"
            && matches!(value, Json::String(outcome) if outcome == "guest-returned-denied")));
        let actual = granted.owner.as_ref().unwrap().handle();
        let mut wrong = actual;
        wrong.context = ownership::Context(actual.context.0.checked_add(1).unwrap());
        assert_eq!(granted.read(wrong, "main.rs"), Err("invalid-handle"));
        wrong = actual;
        wrong.generation = wrong.generation.checked_add(1).unwrap();
        assert_eq!(granted.read(wrong, "main.rs"), Err("invalid-handle"));
        wrong = actual;
        wrong.kind = ResourceKind(actual.kind.0.checked_add(1).unwrap());
        assert_eq!(granted.read(wrong, "main.rs"), Err("invalid-handle"));
        assert_eq!(granted.read(actual, "../main.rs"), Err("invalid-path"));
        assert_eq!(granted.protected_operations, 0);
        assert_eq!(granted.read(actual, "main.rs"), Ok(b"safe\n".to_vec()));
        assert_eq!(granted.request_trace, vec!["fs.read"; 5]);
        let mut foreign = Host::new(
            Directory::new(granted.directory.main_rs.try_clone().unwrap()).unwrap(), true).unwrap();
        assert_eq!(foreign.read(actual, "main.rs"), Err("invalid-handle"));
        assert_eq!(foreign.protected_operations, 0);
        foreign.release().unwrap();
        granted.directory.main_rs.set_len((MAX_BYTES + 1) as u64).unwrap();
        assert_eq!(granted.read(actual, "main.rs"), Err("bounds-reject"));
        assert_eq!(granted.protected_operations, 2);
        assert_eq!(granted.request_trace, vec!["fs.read"; 6]);
        granted.release().unwrap();
        assert_eq!(granted.table.observation().live, 0);
    }
}
