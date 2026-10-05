//! Separately selected, source-checked live-slot host. JSONL is control data;
//! the `source` string in an install request is only compiler input.

use noble_kernel::types::{EffSet, NominalShape, NominalTypeId, ResourceKind, Ty};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::os::unix::{fs::{FileTypeExt, MetadataExt, OpenOptionsExt}, io::AsRawFd};
use std::sync::{Arc, Condvar, Mutex, atomic::{AtomicBool, Ordering}};

const COMMAND_BYTES: u32 = 131_072;
const AUTHORITY_BYTES: u32 = 65_536;
const MAX_INSTALLS: usize = 128;
const MAX_TYPES: usize = 128;
const MAX_SAVED_PROGRAMS: usize = 4096;

struct Resource {
    name: String,
    id: NominalTypeId,
    kind: ResourceKind,
    descriptor: String,
    shape_fingerprint: String,
    schema_fingerprint: String,
}

#[derive(Clone)]
struct Slot {
    input: Vec<String>,
    output: Vec<String>,
    ceiling: Vec<String>,
    proof_required: bool,
}

struct Authority {
    owner: String,
    fingerprint: String,
    source_digests: BTreeMap<String, String>,
    quota: u64,
    effects: Vec<String>,
    grants: Vec<Value>,
    resources: Vec<Resource>,
    slots: BTreeMap<String, Slot>,
}

struct Candidate {
    input: Vec<Ty>,
    output: Vec<String>,
    artifact_sha256: String,
    handle: u32,
    span: noble_wasm::source::CodeSpan,
    root_index: usize,
    programs: Vec<ProgramContract>,
}

#[derive(Clone)]
struct ProgramContract {
    input: Vec<String>,
    output: Vec<String>,
    effects: Vec<String>,
}

impl ProgramContract {
    fn descriptor(&self) -> String {
        format!("Program([{}]->[{}]!{{{}}})",
            self.input.join(","), self.output.join(","), self.effects.join(","))
    }
}

struct ProgramOwner {
    source_id: Option<String>,
    program_index: Option<usize>,
    descriptor: String,
}

struct ReplayOwner {
    caller_id: String,
    frozen_identity: Value,
}

#[derive(Default)]
struct ControlState {
    installed: BTreeMap<(String, u32), ProgramContract>,
    staged: BTreeSet<(String, u32)>,
}

struct ControlPolicy {
    slots: BTreeMap<String, Slot>,
    grants: BTreeSet<(String, Option<String>, Option<String>)>,
    sources: BTreeSet<String>,
    state: Mutex<ControlState>,
}

enum Input {
    Command(Value),
    Queued { control_id: String, scope: String },
    HoldSelected { import: String, occurrence: u32 },
    Refused(String),
    End,
    Failed(String),
}

enum RouterAction {
    Control(Value),
    Hold { import: String, occurrence: u32,
        armed: std::sync::mpsc::SyncSender<()> },
    CancelHold,
    Resume,
}

struct Host {
    frontend: noble_contracts::source::Session,
    compiler: noble_wasm::source::Compiler,
    worker: super::worker::Engine,
    authority: Authority,
    candidates: BTreeMap<String, Candidate>,
    definitions: BTreeMap<String, String>,
    replays: BTreeMap<String, ReplayOwner>,
    programs: BTreeMap<String, ProgramOwner>,
    control: Arc<ControlPolicy>,
    active: Arc<AtomicBool>,
    started: Arc<(Mutex<u64>, Condvar)>,
    control_events: BTreeMap<String, Value>,
    worker_invocation_started: bool,
    poisoned: bool,
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let authority_path = match arguments {
        [live, slot, engine, selected, authority, path]
            if live == "live" && slot == "slot" && engine == "--engine"
                && selected == "v8" && authority == "--authority" => path,
        _ => {
            emit(&refusal("selection", "usage: noble live slot --engine v8 --authority ABS_PATH"));
            return std::process::ExitCode::from(2);
        }
    };
    let path = std::path::Path::new(authority_path);
    if !path.is_absolute() {
        emit(&refusal("selection", "slot authority must be an absolute file path"));
        return std::process::ExitCode::from(2);
    }
    if let Err(problem) = operator_pipe() {
        emit(&refusal("selection", &problem));
        return std::process::ExitCode::from(2);
    }
    let mut authority = match load_authority(path) {
        Ok(authority) => authority,
        Err(problem) => {
            emit(&refusal("authority", &problem));
            return std::process::ExitCode::from(2);
        }
    };
    let mut environment = match noble_kernel::contracts::environment() {
        Ok(environment) => environment.enable_live_slots(),
        Err(problem) => {
            emit(&refusal("authority", &format!("cannot initialize source environment: {problem:?}")));
            return std::process::ExitCode::from(2);
        }
    };
    for resource in &authority.resources {
        let declaration = noble_kernel::contracts::NominalDecl {
            id: resource.id,
            shape: NominalShape::Opaque(Box::new(Ty::Resource(resource.kind))),
            exported: true,
            public: [false, false],
        };
        environment = match environment.register_live_resource(declaration) {
            Ok(environment) => environment,
            Err(problem) => {
                emit(&refusal("authority", &format!("invalid registered nominal: {problem:?}")));
                return std::process::ExitCode::from(2);
            }
        };
    }
    let test_emit_selected = authority.effects.iter().any(|effect| effect == "test.emit")
        && authority.grants.iter().any(|grant| grant["operation"] == "effect"
            && grant["allowed"] == true
            && grant.get("slotId").and_then(Value::as_str).is_some_and(|slot|
                authority.slots.get(slot).is_some_and(|contract|
                    contract.ceiling.iter().any(|effect| effect == "test.emit"))));
    let frontend = if test_emit_selected {
        noble_contracts::source::Session::new_live_slots_with_test_hosts(environment)
    } else {
        noble_contracts::source::Session::new_live_slots(environment)
    };
    let mut frontend = match frontend {
        Ok(frontend) => frontend,
        Err(error) => {
            emit(&refusal("authority", &error.diagnostic().message));
            return std::process::ExitCode::from(2);
        }
    };
    for resource in &authority.resources {
        if let Err(error) = frontend.register_live_resource(&resource.name, resource.id) {
            emit(&refusal("authority", &error.diagnostic().message));
            return std::process::ExitCode::from(2);
        }
    }
    // A host slot descriptor is independently parsed against the same registered
    // type universe; it never comes from a guest submission or a publish request.
    if let Err(problem) = check_slots(&mut authority.slots, &frontend) {
        emit(&refusal("authority", &problem));
        return std::process::ExitCode::from(2);
    }
    let control = Arc::new(ControlPolicy::from_authority(&authority));
    let active = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(AtomicBool::new(false));
    let started = Arc::new((Mutex::new(0), Condvar::new()));
    let worker = match super::worker::Engine::start_slot() {
        Ok(worker) => worker,
        Err(problem) => {
            emit(&refusal("worker", &problem.message));
            return std::process::ExitCode::from(2);
        }
    };
    let mut host = Host {
        frontend,
        compiler: noble_wasm::source::Compiler::new_live_slots(),
        worker,
        authority,
        candidates: BTreeMap::new(),
        definitions: BTreeMap::new(),
        replays: BTreeMap::new(),
        programs: BTreeMap::new(),
        control,
        active,
        started,
        control_events: BTreeMap::new(),
        worker_invocation_started: false,
        poisoned: false,
    };
    let selected_source_hashes = host.authority.source_digests.values().collect::<BTreeSet<_>>();
    let configure = json!({"operation":"configure", "authority": {
        "quota":host.authority.quota,
        "effects":host.authority.effects,
        "grants":host.authority.grants,
        "sources":host.authority.source_digests.iter().map(|(id, sha256)|
            json!({"id":id,"sha256":sha256})).collect::<Vec<_>>(),
        "resources":host.authority.resources.iter().map(|resource| json!({
            "module":resource.id.module.to_string(), "ordinal":resource.id.ordinal,
            "kind":resource.kind.0, "shapeFingerprint":resource.shape_fingerprint,
            "schemaFingerprint":resource.schema_fingerprint,
            "owner":host.authority.owner,
            "sourceArtifactSha256s":selected_source_hashes,
            "interfaceDescriptor":resource.descriptor,
        })).collect::<Vec<_>>()
    }});
    match host.worker.slot(&configure, &[]) {
        Ok(report) if report.outcome == "configured" => emit_worker(&report),
        Ok(report) => {
            emit_worker(&report);
            return std::process::ExitCode::from(2);
        }
        Err(error) => {
            emit(&refusal("worker", &error.message));
            return std::process::ExitCode::from(2);
        }
    }
    let operator = match host.worker.slot_control_sender() {
        Ok(operator) => operator,
        Err(error) => {
            emit(&refusal("worker", &error.message));
            return std::process::ExitCode::from(2);
        }
    };
    let (sender, receiver) = std::sync::mpsc::sync_channel(128);
    let (router_sender, router_receiver) = std::sync::mpsc::sync_channel(256);
    let (writer, reports) = std::sync::mpsc::sync_channel::<Value>(256);
    let write_failed = sender.clone();
    let writer_handle = std::thread::Builder::new().name("noble-slot-writer".into())
        .spawn(move || {
            use std::io::Write;
            let stdout = std::io::stdout();
            let mut output = std::io::BufWriter::new(stdout.lock());
            for report in reports {
                if report.is_null() { return; }
                if writeln!(output, "{report}").and_then(|()| output.flush()).is_err() {
                    let _ = write_failed.send(Input::Failed(reject("operator report pipe closed")));
                    return;
                }
            }
        });
    let writer_handle = match writer_handle {
        Ok(handle) => handle,
        Err(error) => {
            emit(&refusal("worker", &format!("cannot start output writer: {error}")));
            return std::process::ExitCode::from(2);
        }
    };
    let policy = Arc::clone(&host.control);
    let active = Arc::clone(&host.active);
    let started = Arc::clone(&host.started);
    let router_reports = sender.clone();
    let router_active = Arc::clone(&active);
    let router_paused = Arc::clone(&paused);
    let router_writer = writer.clone();
    if let Err(error) = std::thread::Builder::new().name("noble-slot-checkpoints".into())
        .spawn(move || route_checkpoints(operator, router_receiver, router_reports,
            router_active, router_paused, router_writer)) {
        let _ = writer.send(refusal("worker", &format!("cannot start checkpoint router: {error}")));
        return std::process::ExitCode::from(2);
    }
    let main_router = router_sender.clone();
    let reader_writer = writer.clone();
    if let Err(error) = std::thread::Builder::new().name("noble-slot-operator".into())
        .spawn(move || read_operator(sender, router_sender, policy, active, paused, started,
            reader_writer)) {
        let _ = writer.send(refusal("input", &format!("cannot start operator reader: {error}")));
        return std::process::ExitCode::from(2);
    }
    loop {
        match receiver.recv() {
            Ok(Input::Command(request)) => {
                let invocation = matches!(request["operation"].as_str(), Some("invoke" | "replay"));
                if invocation {
                    host.active.store(true, Ordering::Release);
                    let (started, available) = &*host.started;
                    match started.lock() {
                        Ok(mut epoch) => {
                            let Some(next) = epoch.checked_add(1) else {
                                let _ = writer.send(refusal("input", "operator invocation counter exhausted"));
                                return std::process::ExitCode::from(2);
                            };
                            *epoch = next;
                            available.notify_all();
                        }
                        Err(_) => {
                            let _ = writer.send(refusal("input", "operator invocation gate failed"));
                            return std::process::ExitCode::from(2);
                        }
                    }
                }
                host.worker_invocation_started = false;
                let result = host.command(&request);
                if invocation {
                    host.active.store(false, Ordering::Release);
                    if host.worker_invocation_started {
                        let _ = main_router.send(RouterAction::CancelHold);
                    }
                    let (gate, available) = &*host.started;
                    match gate.lock() {
                        Ok(_guard) => available.notify_all(),
                        Err(_) => {
                            let _ = writer.send(refusal("input", "operator completion gate failed"));
                            return std::process::ExitCode::from(2);
                        }
                    }
                }
                let report = result.unwrap_or_else(|problem| {
                    let mut report = refusal("command", &problem);
                    if !host.worker_invocation_started {
                        report["guest_requests"] = json!(0);
                        report["protected_operations"] = json!(0);
                    }
                    report
                });
                if writer.send(report).is_err() { return std::process::ExitCode::from(2); }
            }
            Ok(Input::Queued {control_id,scope}) => match host.control_receipt(&control_id, &scope) {
                Ok(_) => {}
                Err(problem) => {
                    let _ = writer.send(refusal("control", &problem));
                    return std::process::ExitCode::from(2);
                }
            },
            Ok(Input::HoldSelected {import,occurrence}) => if writer.send(json!({
                "schema":"noble-live-slot-report/v1","stage":"operator-control",
                "outcome":"checkpoint-hold-selected","import":import,"occurrence":occurrence
            })).is_err() { return std::process::ExitCode::from(2); },
            Ok(Input::Refused(problem)) => if writer.send(refusal("input", &problem)).is_err() {
                return std::process::ExitCode::from(2);
            },
            Ok(Input::End) => {
                if let Err(error) = host.worker.finish_slot() {
                    let _ = writer.send(refusal("worker", &format!(
                        "selected slot worker did not close cleanly: {}", error.message)));
                    return finish_reports(&writer, writer_handle, std::process::ExitCode::from(2));
                }
                return finish_reports(&writer, writer_handle, std::process::ExitCode::SUCCESS);
            }
            Ok(Input::Failed(problem)) => {
                let _ = writer.send(refusal("input", &problem));
                return finish_reports(&writer, writer_handle, std::process::ExitCode::from(2));
            }
            Err(_) => {
                let _ = writer.send(refusal("input", "operator reader ended unexpectedly"));
                return finish_reports(&writer, writer_handle, std::process::ExitCode::from(2));
            }
        }
        if host.poisoned {
            return finish_reports(&writer, writer_handle, std::process::ExitCode::from(2));
        }
    }
}

fn finish_reports(
    writer: &std::sync::mpsc::SyncSender<Value>,
    handle: std::thread::JoinHandle<()>,
    exit: std::process::ExitCode,
) -> std::process::ExitCode {
    if writer.send(Value::Null).is_err() || handle.join().is_err() {
        std::process::ExitCode::from(2)
    } else {
        exit
    }
}

fn emit(value: &Value) {
    println!("{value}");
}

fn emit_worker(report: &super::output::Report) {
    match serde_json::from_str::<Value>(&report.json) {
        Ok(report) => emit(&report),
        Err(_) => emit(&refusal("worker", "worker emitted malformed JSON report")),
    }
}

fn refusal(stage: &str, diagnostic: &str) -> Value {
    json!({"schema":"noble-live-slot-report/v1", "stage":stage,
        "outcome":"refused", "diagnostic":diagnostic})
}

fn operator_pipe() -> Result<(), String> {
    let metadata = std::fs::metadata("/proc/self/fd/0")
        .map_err(|error| format!("cannot identify host operator ingress: {error}"))?;
    let selected = std::fs::read_link("/proc/self/fd/0")
        .map_err(|error| format!("cannot pin host operator pipe: {error}"))?;
    let name = selected.to_string_lossy();
    let owner = std::fs::metadata("/proc/self")
        .map_err(|error| format!("cannot identify operator process: {error}"))?.uid();
    if !metadata.file_type().is_fifo() || !name.starts_with("pipe:[") || !name.ends_with(']')
        || (metadata.uid() != owner && metadata.uid() != 0)
        || metadata.mode() & 0o022 != 0 {
        return Err(reject("live-slot operator input requires an anonymous OS pipe; launcher must exclusively retain its write end, never delegate it to a guest"));
    }
    Ok(())
}

fn reject(message: &str) -> String { message.to_owned() }

fn object<'a>(value: &'a Value, allowed: &[&str]) -> Result<&'a serde_json::Map<String, Value>, String> {
    let map = value.as_object().ok_or_else(|| reject("expected JSON object"))?;
    if map.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(reject("unknown slot command or authority field"));
    }
    Ok(map)
}

fn text<'a>(value: &'a Value, key: &str, limit: usize) -> Result<&'a str, String> {
    let text = value.get(key).and_then(Value::as_str).ok_or_else(|| format!("missing {key}"))?;
    if text.is_empty() || text.len() > limit || text.as_bytes().contains(&0) {
        return Err(format!("invalid {key}"));
    }
    Ok(text)
}

fn id(value: &Value, key: &str) -> Result<String, String> {
    let name = text(value, key, 128)?;
    if !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-@".contains(&byte)) {
        return Err(format!("invalid {key}"));
    }
    Ok(name.to_owned())
}

fn numbers(value: &Value, key: &str) -> Result<u64, String> {
    let raw = text(value, key, 20)?;
    if raw != "0" && raw.starts_with('0') || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("invalid {key}"));
    }
    raw.parse().map_err(|_| format!("invalid {key}"))
}

fn uint(value: &Value, key: &str) -> Result<u32, String> {
    value.get(key).and_then(Value::as_u64).and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| format!("invalid {key}"))
}

fn strings(value: &Value, key: &str, limit: usize) -> Result<Vec<String>, String> {
    let items = value.get(key).and_then(Value::as_array).ok_or_else(|| format!("missing {key}"))?;
    if items.len() > limit { return Err(format!("oversized {key}")); }
    items.iter().map(|item| item.as_str().filter(|item| !item.is_empty() && item.len() <= 1024)
        .map(str::to_owned).ok_or_else(|| format!("invalid {key}"))).collect()
}

fn effect(id: noble_kernel::types::EffId) -> Result<&'static str, String> {
    match id.0 {
        0 => Ok("test.emit"), 1 => Ok("test.abort"), 2 => Ok("test.clock"),
        5 => Ok("live.dispatch"), _ => Err(reject("unknown checked effect identity")),
    }
}

fn effects(set: &EffSet) -> Result<Vec<String>, String> {
    set.as_slice().iter().map(|id| effect(*id).map(str::to_owned)).collect()
}

fn canonical(ty: &Ty, depth: usize) -> Result<String, String> {
    if depth > 32 { return Err(reject("type descriptor nesting exhausted")); }
    Ok(match ty {
        Ty::I64 => "I64".into(), Ty::Bool => "Bool".into(), Ty::Unit => "Unit".into(),
        Ty::Text => "Text".into(), Ty::Syntax => "Syntax".into(),
        Ty::Contract => "Contract".into(), Ty::Evidence => "Evidence".into(),
        Ty::Certified => "Certified".into(),
        Ty::List(inner) => format!("List({})", canonical(inner, depth + 1)?),
        Ty::Pair(left, right) => format!("Pair({},{})", canonical(left, depth + 1)?, canonical(right, depth + 1)?),
        Ty::Sum(left, right) => format!("Sum({},{})", canonical(left, depth + 1)?, canonical(right, depth + 1)?),
        Ty::Resource(kind) => format!("Resource({})", kind.0),
        Ty::Program(input, output, allowed) => format!("Program([{}]->[{}]!{{{}}})",
            canonical_stack(input, depth + 1)?.join(","), canonical_stack(output, depth + 1)?.join(","), effects(allowed)?.join(",")),
        Ty::LiveRef(input, output, allowed) => format!("LiveRef([{}]->[{}]!{{{}}})",
            canonical_stack(input, depth + 1)?.join(","), canonical_stack(output, depth + 1)?.join(","), effects(allowed)?.join(",")),
        Ty::Nominal(identity, shape) => format!("Nominal({}:{},{})", identity.module, identity.ordinal,
            shape_descriptor(shape, depth + 1)?),
        Ty::GenericNominal(identity, args, shape) => format!("GenericNominal({}:{},[{},{}],{})",
            identity.module, identity.ordinal, canonical(&args[0], depth + 1)?,
            canonical(&args[1], depth + 1)?, shape_descriptor(shape, depth + 1)?),
    })
}

fn shape_descriptor(shape: &NominalShape, depth: usize) -> Result<String, String> {
    if depth > 32 { return Err(reject("nominal descriptor nesting exhausted")); }
    match shape {
        NominalShape::Opaque(ty) => Ok(format!("Opaque({})", canonical(ty, depth + 1)?)),
        NominalShape::Variant(left, right) => Ok(format!("Variant({},{})",
            canonical(left, depth + 1)?, canonical(right, depth + 1)?)),
    }
}

fn canonical_stack(types: &[Ty], depth: usize) -> Result<Vec<String>, String> {
    if types.len() > MAX_TYPES { return Err(reject("ordered type stack exhausted")); }
    types.iter().map(|ty| canonical(ty, depth)).collect()
}

fn load_authority(path: &std::path::Path) -> Result<Authority, String> {
    let bytes = read_pinned_authority(path)?;
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|_| reject("invalid selected authority JSON"))?;
    object(&manifest, &["owner", "quota", "effects", "grants", "resources", "slots", "sources"])?;
    let owner = text(&manifest, "owner", 256)?.to_owned();
    let quota = manifest.get("quota").and_then(Value::as_u64).filter(|quota| (1..=16384).contains(quota))
        .ok_or_else(|| reject("authority requires a bounded retention quota"))?;
    let effects = strings(&manifest, "effects", 4)?;
    let mut seen = BTreeSet::new();
    for name in &effects {
        if !matches!(name.as_str(), "test.emit" | "test.abort" | "live.dispatch") || !seen.insert(name.clone()) {
            return Err(reject("unsupported or duplicate authority effect"));
        }
    }
    let grants = manifest.get("grants").and_then(Value::as_array)
        .filter(|grants| grants.len() <= 512).ok_or_else(|| reject("invalid authority grants"))?;
    let mut checked_grants = Vec::with_capacity(grants.len());
    let mut grant_keys = BTreeSet::new();
    for grant in grants {
        object(grant, &["operation", "slotId", "nominalId", "allowed"])?;
        let op = text(grant, "operation", 32)?;
        if !matches!(op, "publish" | "delete" | "rollback" | "dispatch" | "effect"
            | "register-resource" | "issue-resource" | "discard")
            || grant.get("allowed").and_then(Value::as_bool).is_none() {
            return Err(reject("unsupported authority grant"));
        }
        let slot = grant.get("slotId").map(|_| id(grant, "slotId")).transpose()?;
        let nominal = grant.get("nominalId").map(|_| text(grant, "nominalId", 64).map(str::to_owned)).transpose()?;
        if slot.is_some() == nominal.is_some() {
            return Err(reject("authority grant requires exactly one slot or nominal scope"));
        }
        if !grant_keys.insert((op.to_owned(), slot.clone(), nominal.clone())) {
            return Err(reject("duplicate authority grant"));
        }
        checked_grants.push(grant.clone());
    }
    let selected_hash = crate::workflow::intrinsic::sha256(&bytes);
    let selected_sources = manifest.get("sources").and_then(Value::as_array)
        .filter(|rows| !rows.is_empty() && rows.len() <= MAX_INSTALLS)
        .ok_or_else(|| reject("host authority must select exact source digests"))?;
    let mut source_digests = BTreeMap::new();
    for row in selected_sources {
        object(row, &["id", "sha256"])?;
        let name = id(row, "id")?;
        let digest = text(row, "sha256", 64)?;
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()
                && !byte.is_ascii_uppercase())
            || source_digests.insert(name, digest.to_owned()).is_some() {
            return Err(reject("invalid or duplicate host-selected source identity"));
        }
    }
    let rows = manifest.get("resources").and_then(Value::as_array).filter(|rows| rows.len() <= 128)
        .ok_or_else(|| reject("invalid host resource catalog"))?;
    let mut resources = Vec::with_capacity(rows.len());
    let mut names = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for row in rows {
        object(row, &["name", "module", "ordinal", "kind", "owner"])?;
        let name = id(row, "name")?;
        let module = numbers(row, "module")?;
        let ordinal = uint(row, "ordinal")?;
        let kind = uint(row, "kind")?;
        if kind == 0 || text(row, "owner", 256)? != owner
            || !names.insert(name.clone()) || !identities.insert((module, ordinal)) {
            return Err(reject("unowned, duplicate or invalid nominal resource"));
        }
        let shape = format!("Opaque(Resource({kind}))");
        let descriptor = format!("Nominal({module}:{ordinal},{shape})");
        let shape_fingerprint = crate::workflow::intrinsic::sha256(shape.as_bytes());
        let schema_fingerprint = crate::workflow::intrinsic::sha256(descriptor.as_bytes());
        resources.push(Resource {name, id:NominalTypeId {module,ordinal}, kind:ResourceKind(kind),
            descriptor, shape_fingerprint, schema_fingerprint});
    }
    let rows = manifest.get("slots").and_then(Value::as_array).filter(|rows| rows.len() <= 128)
        .ok_or_else(|| reject("invalid authority slot contracts"))?;
    let mut slots = BTreeMap::new();
    for row in rows {
        object(row, &["slotId", "input", "output", "effectCeiling", "proofRequired"])?;
        let name = id(row, "slotId")?;
        let input = strings(row, "input", MAX_TYPES)?;
        let output = strings(row, "output", MAX_TYPES)?;
        let ceiling = strings(row, "effectCeiling", 4)?;
        let proof_required = row.get("proofRequired").and_then(Value::as_bool)
            .ok_or_else(|| reject("slot proofRequired must be explicit"))?;
        if slots.insert(name, Slot {input, output, ceiling, proof_required}).is_some() {
            return Err(reject("duplicate selected slot identity"));
        }
    }
    Ok(Authority {owner, fingerprint:selected_hash, source_digests, quota, effects,
        grants:checked_grants, resources, slots})
}

/// Resolve each absolute component against a pinned directory descriptor.
/// A symlink, group/world-writable parent, mismatched owner or mutable file
/// cannot turn the selected authority pathname into another inode mid-read.
fn read_pinned_authority(path: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut directory = std::fs::File::open("/")
        .map_err(|error| format!("cannot open authority root: {error}"))?;
    let components = path.components().collect::<Vec<_>>();
    if components.first() != Some(&std::path::Component::RootDir) || components.len() < 2 {
        return Err(reject("authority path must name an absolute regular file"));
    }
    let owner = std::fs::metadata("/proc/self")
        .map_err(|error| format!("cannot identify authority reader: {error}"))?.uid();
    for (index, component) in components.iter().enumerate().skip(1) {
        let std::path::Component::Normal(name) = component else {
            return Err(reject("authority path may not contain dot or parent components"));
        };
        let last = index == components.len() - 1;
        let flags = libc::O_NOFOLLOW | if last { 0 } else { libc::O_DIRECTORY };
        let parent = format!("/proc/self/fd/{}", directory.as_raw_fd());
        let next = std::fs::OpenOptions::new().read(true)
            .custom_flags(flags).open(std::path::Path::new(&parent).join(name))
            .map_err(|error| format!("cannot pin selected authority path: {error}"))?;
        let metadata = next.metadata().map_err(|error| format!("cannot stat pinned authority: {error}"))?;
        if (metadata.uid() != owner && metadata.uid() != 0)
            || metadata.mode() & 0o022 != 0
            || (last && !metadata.is_file())
            || (!last && !metadata.is_dir()) {
            return Err(reject("authority file or ancestor is writable by another principal"));
        }
        directory = next;
    }
    let before = directory.metadata().map_err(|error| format!("cannot stat authority file: {error}"))?;
    let mut bytes = Vec::new();
    (&mut directory).take(u64::from(AUTHORITY_BYTES) + 1).read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read pinned authority: {error}"))?;
    if bytes.len() > AUTHORITY_BYTES as usize {
        return Err(reject("selected authority exceeds byte limit"));
    }
    let after = directory.metadata().map_err(|error| format!("cannot recheck authority file: {error}"))?;
    if (before.dev(), before.ino(), before.uid(), before.mode(), before.len(),
        before.mtime(), before.mtime_nsec(), before.ctime(), before.ctime_nsec())
        != (after.dev(), after.ino(), after.uid(), after.mode(), after.len(),
            after.mtime(), after.mtime_nsec(), after.ctime(), after.ctime_nsec()) {
        return Err(reject("selected authority changed during pinned read"));
    }
    Ok(bytes)
}

fn check_slots(slots: &mut BTreeMap<String, Slot>, frontend: &noble_contracts::source::Session) -> Result<(), String> {
    for slot in slots.values_mut() {
        for stack in [&mut slot.input, &mut slot.output] {
            for descriptor in stack {
                let ty = frontend.parse_live_type(descriptor).map_err(|error| error.diagnostic().message.clone())?;
                *descriptor = canonical(&ty, 0)?;
            }
        }
        let mut seen = BTreeSet::new();
        for name in &slot.ceiling {
            if !matches!(name.as_str(), "test.emit" | "test.abort" | "live.dispatch") || !seen.insert(name.clone()) {
                return Err(reject("unavailable or duplicate slot effect ceiling"));
            }
        }
        slot.ceiling.sort_by_key(|name| match name.as_str() {
            "test.emit" => 0, "test.abort" => 1, "test.clock" => 2, _ => 5,
        });
    }
    Ok(())
}

impl ControlPolicy {
    fn from_authority(authority: &Authority) -> Self {
        let grants = authority.grants.iter().filter(|grant| grant["allowed"] == true).map(|grant| (
            grant["operation"].as_str().unwrap_or_default().to_owned(),
            grant["slotId"].as_str().map(str::to_owned),
            grant["nominalId"].as_str().map(str::to_owned),
        )).collect();
        Self {
            slots: authority.slots.clone(),
            grants,
            sources: authority.source_digests.keys().cloned().collect(),
            state: Mutex::new(ControlState::default()),
        }
    }

    fn allowed(&self, op: &str, slot: Option<&str>, nominal: Option<&str>) -> bool {
        self.grants.contains(&(op.to_owned(), slot.map(str::to_owned), nominal.map(str::to_owned)))
    }

    /// The independent reader cannot borrow the main Host during a blocked
    /// invoke. It selects only already checked, idle-staged candidates and
    /// authority-owned contracts, never accepting source or metadata fields.
    fn normalize(&self, request: &Value, control_id: &str) -> Result<Option<(Value, String)>, String> {
        let Some(op) = request["operation"].as_str() else {
            return Err(reject("operator command lacks operation"));
        };
        if !matches!(op, "policy" | "publish" | "rollback" | "delete") {
            return Ok(None);
        }
        if op == "policy" {
            object(request, &["operation", "grant"])?;
            let grant = request.get("grant").ok_or_else(|| reject("missing operator grant"))?;
            object(grant, &["operation", "slotId", "nominalId", "allowed"])?;
            let target = text(grant, "operation", 32)?;
            let slot = grant.get("slotId").map(|_| id(grant, "slotId")).transpose()?;
            let nominal = grant.get("nominalId")
                .map(|_| text(grant, "nominalId", 64).map(str::to_owned)).transpose()?;
            if slot.is_some() == nominal.is_some() || grant["allowed"] != false
                || !self.allowed(target, slot.as_deref(), nominal.as_deref()) {
                return Err(reject("in-flight policy may only revoke a selected authority scope"));
            }
            let scope = format!("{target}:{}", slot.as_deref().or(nominal.as_deref()).unwrap_or_default());
            return Ok(Some((json!({"operation":"policy","grant":grant,"control_id":control_id}), scope)));
        }
        let slot = id(request, "slot")?;
        let contract = self.slots.get(&slot).ok_or_else(|| reject("unregistered operator slot"))?;
        if !self.allowed(op, Some(&slot), None) {
            return Err(reject("selected host authority denies operator CAS"));
        }
        if op == "delete" {
            object(request, &["operation", "slot", "expected_epoch",
                "expected_incarnation", "expected_generation"])?;
            return Ok(Some((json!({"operation":"delete","slot":slot,
                "expected_epoch":numbers(request, "expected_epoch")?.to_string(),
                "expected_incarnation":numbers(request, "expected_incarnation")?.to_string(),
                "expected_generation":numbers(request, "expected_generation")?.to_string(),
                "control_id":control_id}), slot)));
        }
        object(request, &["operation", "slot", "id", "program_index", "expected_epoch",
            "expected_incarnation", "expected_generation"])?;
        if contract.proof_required {
            return Err(reject("proof-required selected slot cannot publish in-flight"));
        }
        let candidate_id = id(request, "id")?;
        let index = uint(request, "program_index")?;
        if !self.sources.contains(&candidate_id) {
            return Err(reject("operator candidate lacks selected source identity"));
        }
        let state = self.state.lock().map_err(|_| reject("operator catalog lock failed"))?;
        let candidate = state.installed.get(&(candidate_id.clone(), index))
            .ok_or_else(|| reject("operator candidate lacks independently checked Program metadata"))?;
        if !state.staged.contains(&(candidate_id.clone(), index))
            || candidate.input != contract.input || candidate.output != contract.output
            || candidate.effects.iter().any(|effect| !contract.ceiling.contains(effect)) {
            return Err(reject("operator candidate is not idle-staged under exact slot contract"));
        }
        let incarnation = request.get("expected_incarnation").map(|value| {
            if value.is_null() { Ok(Value::Null) } else {
                numbers(request, "expected_incarnation").map(|number| json!(number.to_string()))
            }
        }).transpose()?.unwrap_or(Value::Null);
        let generation = request.get("expected_generation").map(|value| {
            if value.is_null() { Ok(Value::Null) } else {
                numbers(request, "expected_generation").map(|number| json!(number.to_string()))
            }
        }).transpose()?.unwrap_or(Value::Null);
        Ok(Some((json!({"operation":op,"slot":slot,"id":candidate_id,"program_index":index,
            "expected_epoch":numbers(request, "expected_epoch")?.to_string(),
            "expected_incarnation":incarnation,"expected_generation":generation,
            "proof_required":false,
            "interface":{"input":contract.input,"output":contract.output,
                "effectCeiling":contract.ceiling,"proofRequired":false},
            "control_id":control_id}), slot)))
    }
}

fn read_operator(
    sender: std::sync::mpsc::SyncSender<Input>,
    router: std::sync::mpsc::SyncSender<RouterAction>,
    policy: Arc<ControlPolicy>,
    active: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    started: Arc<(Mutex<u64>, Condvar)>,
    writer: std::sync::mpsc::SyncSender<Value>,
) {
    let mut stdin = std::io::BufReader::new(std::io::stdin());
    let mut next_control = 0u64;
    loop {
        let line = match super::framing::read_line(&mut stdin, COMMAND_BYTES) {
            Ok(Some(line)) => line,
            Ok(None) => { let _ = sender.send(Input::End); return; }
            Err(error) => { let _ = sender.send(Input::Failed(error.message)); return; }
        };
        let request: Value = match serde_json::from_slice(&line) {
            Ok(request) => request,
            Err(error) => {
                if sender.send(Input::Refused(format!("invalid JSONL slot command: {error}"))).is_err() {
                    return;
                }
                continue;
            }
        };
        if request["operation"] == "hold-checkpoint" {
            let selected = (|| {
                object(&request, &["operation", "import", "occurrence"])?;
                let import = text(&request, "import", 16)?.to_owned();
                let occurrence = uint(&request, "occurrence")?;
                if !matches!(import.as_str(), "live_select" | "test_emit" | "test_abort")
                    || occurrence == 0 || occurrence > 4096 {
                    return Err(reject("invalid selected checkpoint import or occurrence"));
                }
                Ok((import, occurrence))
            })();
            match selected {
                Ok((import, occurrence)) => {
                    let (armed, receipt) = std::sync::mpsc::sync_channel(0);
                    if router.send(RouterAction::Hold {
                        import:import.clone(), occurrence, armed,
                    }).is_err()
                        || receipt.recv_timeout(std::time::Duration::from_secs(2)).is_err()
                        || sender.send(Input::HoldSelected { import, occurrence }).is_err() {
                        let _ = sender.send(Input::Failed(reject("checkpoint hold was not armed before invocation")));
                        return;
                    }
                }
                Err(problem) => { if sender.send(Input::Refused(problem)).is_err() { return; } }
            }
            continue;
        }
        if request["operation"] == "resume-checkpoint" {
            if object(&request, &["operation"]).is_err() || !active.load(Ordering::Acquire)
                || !paused.swap(false, Ordering::AcqRel) {
                if writer.send(refusal("input", "checkpoint has not entered; resume refused")).is_err() {
                    return;
                }
                continue;
            }
            if router.send(RouterAction::Resume).is_err() {
                let _ = sender.send(Input::Failed(reject("checkpoint resume delivery failed")));
                return;
            }
            continue;
        }
        if active.load(Ordering::Acquire)
            && (request["operation"] != "publish" && request["operation"] != "rollback"
                || request.get("program_index").is_some()) {
            let Some(next) = next_control.checked_add(1) else {
                let _ = sender.send(Input::Failed(reject("operator control counter exhausted")));
                return;
            };
            let control_id = next.to_string();
            match policy.normalize(&request, &control_id) {
                Ok(Some((message,scope))) => {
                    next_control = next;
                    if router.send(RouterAction::Control(message)).is_err() {
                        let _ = sender.send(Input::Failed(reject("operator router is unavailable")));
                        return;
                    }
                    if sender.send(Input::Queued { control_id, scope }).is_err() { return; }
                    continue;
                }
                Ok(None) => {}
                Err(problem) => {
                    if sender.send(Input::Refused(problem)).is_err() { return; }
                    continue;
                }
            }
        }
        let invocation = matches!(request["operation"].as_str(), Some("invoke" | "replay"));
        let before = if invocation {
            match started.0.lock() {
                Ok(epoch) => *epoch,
                Err(_) => { let _ = sender.send(Input::Failed(reject("operator invocation gate failed"))); return; }
            }
        } else { 0 };
        if sender.send(Input::Command(request)).is_err() { return; }
        if invocation {
            let (started, available) = &*started;
            let Ok(mut epoch) = started.lock() else { return };
            while *epoch == before {
                epoch = match available.wait(epoch) {
                    Ok(epoch) => epoch,
                    Err(_) => return,
                };
            }
        } else if active.load(Ordering::Acquire) {
            let (gate, available) = &*started;
            let Ok(mut guard) = gate.lock() else { return };
            while active.load(Ordering::Acquire) {
                guard = match available.wait(guard) {
                    Ok(guard) => guard,
                    Err(_) => return,
                };
            }
        }
    }
}

fn route_checkpoints(
    socket: std::os::unix::net::UnixDatagram,
    commands: std::sync::mpsc::Receiver<RouterAction>,
    sender: std::sync::mpsc::SyncSender<Input>,
    active: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    writer: std::sync::mpsc::SyncSender<Value>,
) {
    let failure = route_checkpoints_inner(&socket, &commands, &active, &paused, &writer);
    if let Err(problem) = failure {
        paused.store(false, Ordering::Release);
        let _ = socket.shutdown(std::net::Shutdown::Both);
        let _ = writer.send(refusal("control", &problem));
        let _ = sender.send(Input::Failed(problem));
    }
}

fn route_checkpoints_inner(
    socket: &std::os::unix::net::UnixDatagram,
    commands: &std::sync::mpsc::Receiver<RouterAction>,
    active: &AtomicBool,
    paused: &AtomicBool,
    writer: &std::sync::mpsc::SyncSender<Value>,
) -> Result<(), String> {
    socket.set_nonblocking(false).map_err(|error| format!("checkpoint socket: {error}"))?;
    socket.set_read_timeout(Some(std::time::Duration::from_millis(100)))
        .map_err(|error| format!("checkpoint socket deadline: {error}"))?;
    let mut pending = VecDeque::new();
    let mut hold = None::<(String, u32)>;
    let mut root = String::new();
    let mut last_root = 0u64;
    let mut next_index = 1u32;
    let mut occurrences = BTreeMap::<String, u32>::new();
    loop {
        drain_router_commands(commands, &mut pending, &mut hold)?;
        let Some(checkpoint) = read_control_packet(socket)? else { continue };
        object(&checkpoint, &["kind", "root", "index", "operation", "site", "ordinal"])?;
        if checkpoint["kind"] != "checkpoint" || !active.load(Ordering::Acquire) {
            return Err(reject("unsolicited or malformed guest checkpoint"));
        }
        let selected_root = text(&checkpoint, "root", 20)?;
        let root_number = selected_root.parse::<u64>()
            .map_err(|_| reject("invalid checkpoint root identity"))?;
        let index = uint(&checkpoint, "index")?;
        let import = text(&checkpoint, "operation", 16)?;
        if !matches!(import, "live_select" | "test_emit" | "test_abort") {
            return Err(reject("unknown protected import checkpoint"));
        }
        if let Some(_) = checkpoint.get("site") { uint(&checkpoint, "site")?; }
        if let Some(_) = checkpoint.get("ordinal") { uint(&checkpoint, "ordinal")?; }
        if root != selected_root {
            if root_number <= last_root {
                return Err(reject("guest checkpoint root did not advance"));
            }
            last_root = root_number;
            root = selected_root.to_owned();
            next_index = 1;
            occurrences.clear();
        }
        if index != next_index || index == 0 || index > 4096 {
            return Err(reject("out-of-order protected import checkpoint"));
        }
        drain_router_commands(commands, &mut pending, &mut hold)?;
        next_index += 1;
        let seen = occurrences.entry(import.to_owned()).or_default();
        *seen += 1;
        let selected_hold = hold.as_ref().is_some_and(|(selected, occurrence)|
            selected == import && *occurrence == *seen);
        if selected_hold {
            hold = None;
            paused.store(true, Ordering::Release);
            writer.send(json!({"schema":"noble-live-slot-report/v1","stage":"operator-control",
                "outcome":"checkpoint-entered","root":root,"checkpoint":index,
                "import":import,"site":checkpoint.get("site"),
                "ordinal":checkpoint.get("ordinal")}))
                .map_err(|_| reject("checkpoint entry cannot reach operator report writer"))?;
            let until = std::time::Instant::now() + std::time::Duration::from_secs(8);
            loop {
                let remaining = until.saturating_duration_since(std::time::Instant::now());
                if remaining.is_zero() {
                    return Err(reject("selected checkpoint expired before operator resume"));
                }
                match commands.recv_timeout(remaining.min(std::time::Duration::from_millis(100))) {
                    Ok(RouterAction::Control(control)) => {
                        pending.push_back(control);
                        if pending.len() > 8 {
                            return Err(reject("checkpoint exceeds eight ordered operator controls"));
                        }
                        apply_checkpoint_controls(socket, &root, index, &mut pending, writer)?;
                    }
                    Ok(RouterAction::Resume) => break,
                    Ok(RouterAction::Hold { .. }) => return Err(reject("nested checkpoint hold refused")),
                    Ok(RouterAction::CancelHold) => return Err(reject("root ended during held checkpoint")),
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) =>
                        return Err(reject("operator lane closed during held checkpoint")),
                }
            }
        }
        drain_router_commands(commands, &mut pending, &mut hold)?;
        if pending.len() > 8 {
            return Err(reject("checkpoint exceeds eight ordered operator controls"));
        }
        apply_checkpoint_controls(socket, &root, index, &mut pending, writer)?;
        send_control_packet(socket, &json!({"kind":"resume","root":root,"index":index}))?;
    }
}

fn drain_router_commands(
    receiver: &std::sync::mpsc::Receiver<RouterAction>,
    pending: &mut VecDeque<Value>,
    hold: &mut Option<(String, u32)>,
) -> Result<(), String> {
    while let Ok(action) = receiver.try_recv() {
        match action {
            RouterAction::Control(request) => pending.push_back(request),
            RouterAction::Hold { import, occurrence, armed } => {
                if hold.replace((import, occurrence)).is_some() {
                    return Err(reject("multiple outstanding checkpoint holds"));
                }
                armed.send(()).map_err(|_| reject("operator closed selected checkpoint barrier"))?;
            }
            RouterAction::CancelHold => *hold = None,
            RouterAction::Resume => return Err(reject("checkpoint resume has no held import")),
        }
        if pending.len() > 8 {
            return Err(reject("operator control backlog exceeds protected checkpoint capacity"));
        }
    }
    Ok(())
}

fn read_control_packet(socket: &std::os::unix::net::UnixDatagram) -> Result<Option<Value>, String> {
    let mut packet = [0u8; 4097];
    let size = match socket.recv(&mut packet) {
        Ok(size) => size,
        Err(error) if matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) =>
            return Ok(None),
        Err(error) => return Err(format!("checkpoint receive failed: {error}")),
    };
    if size == 0 || size > 4096 {
        return Err(reject("checkpoint datagram is empty or oversized"));
    }
    serde_json::from_slice(&packet[..size]).map(Some)
        .map_err(|_| reject("checkpoint datagram is not JSON"))
}

fn send_control_packet(socket: &std::os::unix::net::UnixDatagram, packet: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(packet).map_err(|_| reject("cannot encode checkpoint packet"))?;
    if bytes.len() > 4096 {
        return Err(reject("checkpoint packet exceeds byte limit"));
    }
    match socket.send(&bytes) {
        Ok(written) if written == bytes.len() => Ok(()),
        _ => Err(reject("checkpoint datagram was not delivered atomically")),
    }
}

fn apply_checkpoint_controls(
    socket: &std::os::unix::net::UnixDatagram,
    root: &str,
    index: u32,
    pending: &mut VecDeque<Value>,
    writer: &std::sync::mpsc::SyncSender<Value>,
) -> Result<(), String> {
    while let Some(request) = pending.pop_front() {
        let control_id = text(&request, "control_id", 20)?;
        send_control_packet(socket, &json!({"kind":"update","root":root,"index":index,
            "request":request}))?;
        socket.set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .map_err(|error| format!("control acknowledgement deadline: {error}"))?;
        let acknowledged = read_control_packet(socket)?;
        socket.set_read_timeout(Some(std::time::Duration::from_millis(100)))
            .map_err(|error| format!("checkpoint deadline reset: {error}"))?;
        let acknowledged = acknowledged.ok_or_else(|| reject("operator CAS acknowledgement timed out"))?;
        object(&acknowledged, &["kind", "root", "index", "control_id", "outcome", "epoch"])?;
        if acknowledged["kind"] != "commit" || acknowledged["root"] != root
            || acknowledged["index"] != index || acknowledged["control_id"] != control_id {
            return Err(reject("operator CAS acknowledgement lacks exact checkpoint identity"));
        }
        let outcome = text(&acknowledged, "outcome", 64)?;
        writer.send(json!({"schema":"noble-live-slot-report/v1","stage":"operator-control",
            "outcome":outcome,"control_id":control_id,"checkpoint":index,
            "root":root,"epoch":acknowledged["epoch"]}))
            .map_err(|_| reject("operator report writer closed before committed acknowledgement"))?;
    }
    Ok(())
}

impl Host {
    fn command(&mut self, request: &Value) -> Result<Value, String> {
        let op = text(request, "operation", 32)?;
        let report = match op {
            "define" => return self.define(request),
            "install" => return self.install(request),
            "candidate" => return self.stage_candidate(request),
            "publish" | "rollback" => {
                object(request, &["operation", "slot", "id", "owner", "program_index",
                    "expected_epoch", "expected_incarnation", "expected_generation"])?;
                self.publication(request, op)?
            }
            "delete" => {
                object(request, &["operation", "slot", "expected_epoch", "expected_incarnation", "expected_generation"])?;
                let slot = id(request, "slot")?;
                self.require_grant("delete", Some(&slot), None)?;
                numbers(request, "expected_incarnation")?;
                numbers(request, "expected_generation")?;
                self.cas(request, json!({"operation":"delete","slot":slot}))?
            }
            "invoke" => return self.invoke(request),
            "replay" => return self.replay(request),
            "release-replay" => return self.release_replay(request),
            "release-program" => return self.release_program(request),
            "discard" => return self.discard(request),
            "reflect" | "trace" | "policy" => self.inspect(request, op)?,
            _ => return Err(reject("unsupported live-slot operation")),
        };
        self.send(&report, &[])
    }

    fn send(&mut self, request: &Value, wat: &[u8]) -> Result<Value, String> {
        let report = self.worker.slot(request, wat).map_err(|error| {
            self.poisoned = true;
            format!("worker: {}", error.message)
        })?;
        let value: Value = serde_json::from_str(&report.json).map_err(|_| {
            self.poisoned = true;
            reject("worker emitted malformed JSON")
        })?;
        if value.get("outcome").and_then(Value::as_str) != Some(report.outcome.as_str()) {
            self.poisoned = true;
            return Err(reject("worker report outcome differs from frame"));
        }
        if let Some(events) = value.get("control_events") {
            let rows = events.as_array().filter(|rows| rows.len() <= 256).ok_or_else(|| {
                self.poisoned = true;
                reject("worker returned unbounded operator receipts")
            })?;
            for row in rows {
                let control_id = text(row, "control_id", 20).map_err(|problem| {
                    self.poisoned = true;
                    problem
                })?;
                if self.control_events.insert(control_id.to_owned(), row.clone()).is_some() {
                    self.poisoned = true;
                    return Err(reject("worker returned duplicate operator receipt"));
                }
            }
        }
        if request["operation"] != "install" {
            if let Err(problem) = self.retire_reported_code(&value) {
                self.poisoned = true;
                return Err(problem);
            }
        }
        if value["session_state"] == "terminated-after-guest-failure"
            || value["actual"]["session_state"] == "terminated-after-guest-failure" {
            self.poisoned = true;
            self.worker.mark_slot_terminal();
        }
        Ok(value)
    }

    fn retire_reported_code(&mut self, report: &Value) -> Result<(), String> {
        let rows = report.get("retire_code_spans").and_then(Value::as_array)
            .filter(|rows| rows.len() <= MAX_INSTALLS)
            .ok_or_else(|| {
                self.poisoned = true;
                reject("worker omitted bounded installed-code retirement ledger")
            })?;
        for row in rows {
            let span = noble_wasm::source::CodeSpan {
                start: uint(row, "start")?,
                length: uint(row, "length")?,
                generation: uint(row, "generation")?,
            };
            let mut owner = None;
            for (id, candidate) in &self.candidates {
                if candidate.span == span {
                    if owner.is_some() {
                        self.poisoned = true;
                        return Err(reject("worker requested duplicate installed-code retirement"));
                    }
                    owner = Some(id.clone());
                }
            }
            if span.length == 0 || owner.is_none() {
                self.poisoned = true;
                return Err(reject("worker requested unknown or duplicate code-span retirement"));
            }
            let mut problem = None;
            let worker = &mut self.worker;
            let result = self.compiler.retire_code(span, |exact| {
                let message = json!({"operation":"retire-code","code_span":{
                    "start":exact.start,"length":exact.length,"generation":exact.generation}});
                let reply = match worker.slot(&message, &[]) {
                    Ok(reply) => reply,
                    Err(error) => {
                        problem = Some(format!("worker retirement failed: {}", error.message));
                        return Err(noble_wasm::Diagnostic::Invalid);
                    }
                };
                let acknowledged = serde_json::from_str::<Value>(&reply.json).ok();
                let valid = reply.outcome == "code-retired"
                    && acknowledged.as_ref().is_some_and(|value|
                        value["outcome"] == "code-retired"
                        && value["code_span"] == message["code_span"]
                        && value["code_owners"] == 0
                        && value["table_cleared"] == true);
                if !valid {
                    problem = Some(reject("worker did not prove exact zero-owned table-span clearance"));
                    return Err(noble_wasm::Diagnostic::Invalid);
                }
                Ok(())
            });
            if result.is_err() {
                self.poisoned = true;
                return Err(problem.unwrap_or_else(|| reject("compiler refused exact installed-code retirement")));
            }
            if let Some(owner) = owner {
                self.candidates.remove(&owner);
                let mut control = self.control.state.lock().map_err(|_| {
                    self.poisoned = true;
                    reject("operator candidate catalog failed during retirement")
                })?;
                control.installed.retain(|(id, _), _| id != &owner);
                control.staged.retain(|(id, _)| id != &owner);
            }
        }
        Ok(())
    }

    fn control_receipt(&mut self, control_id: &str, scope: &str) -> Result<Value, String> {
        let (event, when) = if let Some(event) = self.control_events.remove(control_id) {
            (event, "during-root")
        } else {
            let barrier = self.send(&json!({"operation":"control-barrier"}), &[])?;
            if barrier["outcome"] != "control-barrier" {
                self.poisoned = true;
                return Err(reject("worker refused the operator control barrier"));
            }
            let Some(event) = self.control_events.remove(control_id) else {
                self.poisoned = true;
                return Err(reject("operator command delivery is unacknowledged; host state unknown"));
            };
            (event, "after-root")
        };
        if event["scope"] != scope {
            self.poisoned = true;
            return Err(reject("operator receipt scope differs from selected request"));
        }
        Ok(json!({"schema":"noble-live-slot-report/v1","stage":"operator-control",
            "outcome":event["outcome"],"when":when,"control_id":control_id,
            "scope":scope,"epoch":event["epoch"]}))
    }

    fn require_grant(&self, operation: &str, slot: Option<&str>, nominal: Option<&str>) -> Result<(), String> {
        if self.authority.grants.iter().any(|grant| grant["operation"] == operation
            && grant.get("slotId").and_then(Value::as_str) == slot
            && grant.get("nominalId").and_then(Value::as_str) == nominal
            && grant["allowed"] == true) {
            Ok(())
        } else {
            Err(reject("selected host authority denies operation"))
        }
    }

    fn define(&mut self, request: &Value) -> Result<Value, String> {
        object(request, &["operation", "id", "name", "source", "inputs"])?;
        let source_id = id(request, "id")?;
        let name = id(request, "name")?;
        if self.definitions.contains_key(&name) {
            return Err(reject("selected declaration name is already retained"));
        }
        let source = request.get("source").and_then(Value::as_str)
            .filter(|source| source.len() <= super::SOURCE_LIMITS.bytes as usize)
            .ok_or_else(|| reject("definition must be bounded UTF-8 source text"))?;
        let source_hash = crate::workflow::intrinsic::sha256(source.as_bytes());
        if self.authority.source_digests.get(&source_id) != Some(&source_hash) {
            return Err(reject("definition differs from independently selected host source"));
        }
        if !strings(request, "inputs", MAX_TYPES)?.is_empty() {
            return Err(reject("selected declaration cannot carry an executable input root"));
        }
        let prepared = self.frontend.prepare(source.as_bytes(), &[], super::SOURCE_LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        if !prepared.is_definition() || prepared.submission().is_some()
            || prepared.definition_name() != Some(name.as_str()) {
            return Err(reject("source is not exactly the selected checked declaration"));
        }
        self.frontend.commit(prepared)
            .map_err(|error| error.diagnostic().message.clone())?;
        let generation = self.frontend.generation();
        self.definitions.insert(name.clone(), source_hash.clone());
        Ok(json!({"schema":"noble-live-slot-report/v1","stage":"source",
            "outcome":"definition-retained","id":source_id,"name":name,
            "source_sha256":source_hash,"source_generation":generation}))
    }

    fn install(&mut self, request: &Value) -> Result<Value, String> {
        object(request, &["operation", "id", "source", "inputs", "selected_name"])?;
        let id = id(request, "id")?;
        if self.candidates.len() >= MAX_INSTALLS || self.candidates.contains_key(&id) {
            return Err(reject("duplicate or exhausted installed candidate identity"));
        }
        let source = request.get("source").and_then(Value::as_str)
            .filter(|source| source.len() <= super::SOURCE_LIMITS.bytes as usize)
            .ok_or_else(|| reject("source must be bounded UTF-8 text"))?;
        let source_hash = crate::workflow::intrinsic::sha256(source.as_bytes());
        if self.authority.source_digests.get(&id) != Some(&source_hash) {
            return Err(reject("source bytes differ from independently selected host source identity"));
        }
        let inputs = strings(request, "inputs", MAX_TYPES)?;
        let typed: Vec<Ty> = inputs.iter().map(|name| self.frontend.parse_live_type(name)
            .map_err(|error| error.diagnostic().message.clone())).collect::<Result<_,_>>()?;
        let checked = self.frontend.prepare(source.as_bytes(), &typed, super::SOURCE_LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        if checked.is_definition() {
            return Err(reject("a slot install must contain a checked executable expression"));
        }
        let submission = checked.submission().ok_or_else(|| reject("missing checked source submission"))?;
        let (compiled, selected_target) = if request.get("selected_name").is_some() {
            let name = self::id(request, "selected_name")?;
            let retained_hash = self.definitions.get(&name)
                .ok_or_else(|| reject("selected named target has no independently selected declaration"))?;
            let selected = self.frontend.checked_selected_target(&checked, &name)
                .map_err(|error| format!("source-bound named target refused: {error:?}"))?;
            let actual_hash = crate::workflow::intrinsic::sha256(selected.source());
            if &actual_hash != retained_hash {
                return Err(reject("selected definition differs from retained host source identity"));
            }
            let prepared = self.compiler.prepare_checked_selected(&selected)
                .map_err(|error| super::output::Failure::backend(error).message)?;
            let bound = prepared.selected_target()
                .ok_or_else(|| reject("compiled artifact omitted source-selected target"))?;
            if bound.definition_id != selected.definition_id()
                || bound.source_generation != selected.source_generation()
                || prepared.target_metadata().is_none_or(|target|
                    bound.named_program_index == target.root_program_index)
                || prepared.named_targets().iter().filter(|target|
                    target.program_index == bound.named_program_index
                    && target.definition_identity == bound.definition_id.identity()).count() != 1 {
                return Err(reject("named Program differs from independently checked selected target"));
            }
            let identity = json!({
                "definition_index":bound.definition_id.index(),
                "definition_identity":bound.definition_id.identity().to_string(),
                "source_generation":bound.source_generation.to_string(),
                "named_program_index":bound.named_program_index,
                "selected_source_sha256":actual_hash,
                "source_sha256":source_hash,
                "wat_sha256":crate::workflow::intrinsic::sha256(prepared.wat()),
            });
            (prepared, Some(identity))
        } else {
            (self.compiler.prepare(submission)
                .map_err(|error| super::output::Failure::backend(error).message)?, None)
        };
        if !self.compiler.can_commit(&compiled) {
            return Err(reject("compiler source state changed during preparation"));
        }
        if self.frontend.generation() == u64::MAX {
            return Err(reject("source namespace generation exhausted before install"));
        }
        let target = compiled.target_metadata().ok_or_else(|| reject("missing checked target metadata"))?;
        let root_index = target.root_program_index;
        let input = canonical_stack(&target.stack_in, 0)?;
        let output = canonical_stack(&target.stack_out, 0)?;
        let target_effects = effects(&target.effects)?;
        if target_effects.iter().any(|effect| !self.authority.effects.contains(effect)) {
            return Err(reject("unavailable selected host effect"));
        }
        let resources = compiled.resource_catalog().iter().map(|row| {
            let host = self.authority.resources.iter().find(|registered| registered.id == row.declaration.id)
                .ok_or_else(|| reject("compiler resource lacks independent host registration"))?;
            let descriptor = format!("Nominal({}:{},{})", row.declaration.id.module, row.declaration.id.ordinal,
                shape_descriptor(&row.declaration.shape, 0)?);
            if row.kind != host.kind || descriptor != host.descriptor
                || !row.declaration.exported || row.declaration.public != [false, false] {
                return Err(reject("checked resource catalog differs from host schema"));
            }
            Ok((json!({"module":row.declaration.id.module.to_string(),
                "ordinal":row.declaration.id.ordinal,"kind":row.kind.0,
                "shapeFingerprint":crate::workflow::intrinsic::sha256(shape_descriptor(&row.declaration.shape, 0)?.as_bytes())}),
                json!({"module":row.declaration.id.module.to_string(),
                "ordinal":row.declaration.id.ordinal,"schemaFingerprint":host.schema_fingerprint})))
        }).collect::<Result<Vec<_>, String>>()?;
        if resources.len() != self.authority.resources.len() {
            return Err(reject("incomplete independently checked host resource catalog"));
        }
        let sites = compiled.live_sites().iter().map(|site| Ok(json!({
            "site_id":site.site_id,
            "caller_program_index":site.caller_program_index,
            "source_node":site.source_node.0,
            "selected_ref_logical_position":site.selected_ref_logical_position,
            "forwarded_source_positions":site.forwarded_source_positions,
            "forwarded_target_positions":site.forwarded_target_positions,
            "target_input":canonical_stack(&site.target_input, 0)?,
            "target_output":canonical_stack(&site.target_output, 0)?,
            "effect_ceiling":effects(&site.effect_ceiling)?,
            "input_signature":site.input_signature,
            "output_signature":site.output_signature,
            "effect_mask":site.effect_mask,
        }))).collect::<Result<Vec<Value>, String>>()?;
        let programs = compiled.program_metadata().iter().map(|program| Ok(ProgramContract {
            input: canonical_stack(&program.stack_in, 0)?,
            output: canonical_stack(&program.stack_out, 0)?,
            effects: effects(&program.effects)?,
        })).collect::<Result<Vec<_>, String>>()?;
        let program_metadata = compiled.program_metadata().iter().zip(&programs).map(|(program, contract)| json!({
            "program_index":program.program_index,
            "entry":program.entry,
            "input_signature":program.input_signature,
            "output_signature":program.output_signature,
            "effect_mask":program.effect_mask,
            "stack_in":contract.input,
            "stack_out":contract.output,
            "effects":contract.effects,
            "capture_left":program.capture_left,
            "capture_right":program.capture_right,
            "recipe_depth":program.recipe_depth,
            "recipe_leaves":program.recipe_leaves,
        })).collect::<Vec<Value>>();
        let span = compiled.code_span();
        let mut message = json!({
            "operation":"install", "id":id,
            "source_sha256":source_hash,
            "wat_sha256":crate::workflow::intrinsic::sha256(compiled.wat()),
            "code_span":{"start":span.start,"length":span.length,"generation":span.generation},
            "target_metadata":{"root_program_index":target.root_program_index,
                "input_signature":target.input_signature,
                "output_signature":target.output_signature,
                "effect_mask":target.effect_mask,
                "stack_in":input,"stack_out":output,"effects":target_effects},
            "live_sites":sites,
            "program_metadata":program_metadata,
            "resource_catalog":resources.iter().map(|(row,_)|row).collect::<Vec<_>>(),
            "source_schemas":resources.iter().map(|(_,schema)|schema).collect::<Vec<_>>(),
            "host_owner":self.authority.owner,
            "source_artifact_sha256":source_hash,
            "authority_sha256":self.authority.fingerprint,
        });
        if let Some(selected) = selected_target {
            message["selected_target"] = selected;
        }
        let result = self.send(&message, compiled.wat())?;
        if result["outcome"] != "installed" { return Ok(result); }
        let artifact_sha256 = match text(&result, "artifact_sha256", 64) {
            Ok(digest) if digest.len() == 64
                && digest.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()) =>
                digest.to_owned(),
            _ => {
                self.poisoned = true;
                return Err(reject("worker omitted checked installed artifact identity"));
            }
        };
        let handle = match uint(&result, "handle") {
            Ok(handle) if handle != 0 => handle,
            _ => {
                self.poisoned = true;
                return Err(reject("worker omitted actual installed Program handle"));
            }
        };
        // A single-threaded host holds both preparations exclusively across
        // this worker call. Frontend commit rechecks only the unchanged
        // namespace snapshot and a generation already checked above; compiler
        // commit rechecks can_commit, also established above. Neither local
        // state is touched by the worker. Any unexpected failure is fatal:
        // installed code cannot be unwound by a fake logical discard.
        if let Err(error) = self.compiler.commit(compiled) {
            self.poisoned = true;
            return Err(format!("fatal compiler publication failure after worker install: {}",
                super::output::Failure::backend(error).message));
        }
        if let Err(error) = self.frontend.commit(checked) {
            self.poisoned = true;
            return Err(format!("fatal source publication failure after worker install: {}",
                error.diagnostic().message));
        }
        let installed_id = id.clone();
        self.candidates.insert(id, Candidate {
            input:typed, output,
            artifact_sha256, handle,
            span, root_index, programs,
        });
        let mut state = self.control.state.lock().map_err(|_| {
            self.poisoned = true;
            reject("operator candidate catalog failed after install")
        })?;
        if let Some(candidate) = self.candidates.get(&installed_id) {
            for (index, program) in candidate.programs.iter().enumerate() {
                state.installed.insert((installed_id.clone(), index as u32),
                    program.clone());
            }
        }
        drop(state);
        if let Err(problem) = self.retire_reported_code(&result) {
            self.poisoned = true;
            return Err(problem);
        }
        Ok(result)
    }

    fn stage_candidate(&mut self, request: &Value) -> Result<Value, String> {
        object(request, &["operation", "id", "program_index"])?;
        let selected_id = id(request, "id")?;
        let candidate = self.candidates.get(&selected_id)
            .ok_or_else(|| reject("unknown checked installed candidate"))?;
        let index = match request.get("program_index") {
            Some(_) => uint(request, "program_index")?,
            None => u32::try_from(candidate.root_index)
                .map_err(|_| reject("checked root Program index exceeds protocol range"))?,
        };
        if candidate.programs.get(index as usize).is_none() {
            return Err(reject("candidate Program index is absent from checked metadata"));
        }
        let result = self.send(&json!({"operation":"candidate","id":selected_id,
            "program_index":index}), &[])?;
        if result["outcome"] == "candidate-staged" {
            let mut state = self.control.state.lock()
                .map_err(|_| reject("operator candidate catalog failed during stage"))?;
            state.staged.insert((selected_id, index));
        }
        Ok(result)
    }

    fn cas(&self, request: &Value, mut message: Value) -> Result<Value, String> {
        message["expected_epoch"] = json!(numbers(request, "expected_epoch")?.to_string());
        for key in ["expected_incarnation", "expected_generation"] {
            if let Some(value) = request.get(key) {
                message[key] = if value.is_null() { Value::Null } else {
                    json!(numbers(request, key)?.to_string())
                };
            }
        }
        Ok(message)
    }

    fn publication(&self, request: &Value, op: &str) -> Result<Value, String> {
        let slot = id(request, "slot")?;
        self.require_grant(op, Some(&slot), None)?;
        let contract = self.authority.slots.get(&slot).ok_or_else(|| reject("unregistered selected slot contract"))?;
        if contract.proof_required {
            return Err(reject("proof-required slot lacks independently checked target evidence"));
        }
        let (input, output, effects, selected_field, selected_index) =
            match (request.get("id"), request.get("owner")) {
            (Some(_), None) => {
                let selected_id = id(request, "id")?;
                let candidate = self.candidates.get(&selected_id)
                    .ok_or_else(|| reject("unknown checked installed candidate"))?;
                let index = request.get("program_index").map(|_| uint(request, "program_index"))
                    .transpose()?.map(|value| value as usize).unwrap_or(candidate.root_index);
                let program = candidate.programs.get(index)
                    .ok_or_else(|| reject("candidate Program index lacks checked target metadata"))?;
                (&program.input, &program.output,
                    &program.effects, ("id", selected_id), Some(index))
            }
            (None, Some(_)) => {
                if request.get("program_index").is_some() {
                    return Err(reject("saved owner publication cannot select a different Program index"));
                }
                let owner = text(request, "owner", 256)?.to_owned();
                let saved = self.programs.get(&owner)
                    .ok_or_else(|| reject("unknown saved checked Program owner"))?;
                let (Some(source_id), Some(program_index)) = (&saved.source_id, saved.program_index)
                else {
                    return Err(reject("dynamic saved Program lacks independently checked installed target metadata"));
                };
                let program = self.candidates.get(source_id)
                    .and_then(|candidate| candidate.programs.get(program_index))
                    .ok_or_else(|| reject("saved Program code was retired"))?;
                if program.descriptor() != saved.descriptor {
                    return Err(reject("saved owner differs from checked Program metadata"));
                }
                (&program.input, &program.output, &program.effects, ("owner", owner), None)
            }
            _ => return Err(reject("publish requires exactly one checked candidate or saved Program owner")),
        };
        if *input != contract.input || *output != contract.output
            || effects.iter().any(|name| !contract.ceiling.contains(name)) {
            return Err(reject("checked candidate differs from selected slot interface or effect ceiling"));
        }
        let mut message = json!({"operation":op,"slot":slot,
            "proof_required":contract.proof_required,
            "interface":{"input":contract.input,"output":contract.output,
                "effectCeiling":contract.ceiling,"proofRequired":contract.proof_required}});
        message[selected_field.0] = json!(selected_field.1);
        if let Some(index) = selected_index {
            message["program_index"] = json!(index);
        }
        self.cas(request, message)
    }

    fn invoke(&mut self, request: &Value) -> Result<Value, String> {
        object(request, &["operation", "id", "inputs", "refs", "record"])?;
        let module_id = id(request, "id")?;
        let record = match request.get("record") {
            None => false,
            Some(value) => value.as_bool().ok_or_else(|| reject("record must be boolean"))?,
        };
        if record && self.replays.len() >= MAX_INSTALLS {
            return Err(reject("recorded root retention limit exhausted"));
        }
        let mut invocation = self.invocation(request, &module_id)?;
        invocation["record"] = json!(record);
        self.worker_invocation_started = true;
        let report = self.send(&invocation, &[])?;
        if report["outcome"] == "executed" {
            if let Err(problem) = self.retain_program_outputs(&module_id, &report) {
                self.poisoned = true;
                return Err(problem);
            }
        }
        if record && report["outcome"] == "executed" {
            let token = match text(&report, "replay_token", 256) {
                Ok(token) if token.is_ascii() => token.to_owned(),
                Err(problem) => {
                    self.poisoned = true;
                    return Err(problem);
                }
                Ok(_) => {
                    self.poisoned = true;
                    return Err(reject("worker returned invalid frozen replay token"));
                }
            };
            let frozen_identity = report.get("frozen_identity").ok_or_else(|| {
                self.poisoned = true;
                reject("worker omitted frozen source/artifact identity")
            })?;
            let candidate = self.candidates.get(&module_id).ok_or_else(|| {
                self.poisoned = true;
                reject("frozen caller lacks checked source identity")
            })?;
            if frozen_identity["caller"]["id"] != module_id
                || frozen_identity["caller"]["sourceSha256"] != self.authority.source_digests[&module_id]
                || frozen_identity["caller"]["artifactSha256"] != candidate.artifact_sha256
                || frozen_identity["caller"]["handle"] != candidate.handle
                || !frozen_identity["slots"].as_array().is_some_and(|rows| rows.len() <= MAX_TYPES) {
                self.poisoned = true;
                return Err(reject("frozen receipt differs from independently selected caller identity"));
            }
            let frozen_identity = frozen_identity.clone();
            if self.replays.insert(token, ReplayOwner {caller_id:module_id, frozen_identity}).is_some() {
                self.poisoned = true;
                return Err(reject("worker reused a live frozen replay token"));
            }
        }
        Ok(report)
    }

    fn retain_program_outputs(&mut self, module_id: &str, report: &Value) -> Result<(), String> {
        let candidate = self.candidates.get(module_id).ok_or_else(|| reject("checked caller retired during execution"))?;
        let stack = report.get("stack").and_then(Value::as_array)
            .filter(|stack| stack.len() == candidate.output.len())
            .ok_or_else(|| reject("worker returned a stack incompatible with checked caller"))?;
        for (position, descriptor) in candidate.output.iter().enumerate() {
            let cell = &stack[position];
            if descriptor.starts_with("Program(") {
                if cell["kind"] != 4 || self.programs.len() >= MAX_SAVED_PROGRAMS {
                    self.poisoned = true;
                    return Err(reject("worker did not retain a checked Program output"));
                }
                let owner = text(cell, "owner", 256)?;
                let (source_id, program_index) = match (&cell["source_id"], &cell["program_index"]) {
                    (Value::Null, Value::Null) => (None, None),
                    (Value::String(_), Value::Number(_)) => {
                        let source_id = id(cell, "source_id")?;
                        let program_index = uint(cell, "program_index")? as usize;
                        let selected = self.candidates.get(&source_id)
                            .and_then(|module| module.programs.get(program_index))
                            .ok_or_else(|| reject("worker returned an unregistered checked Program index"))?;
                        if selected.descriptor() != *descriptor {
                            self.poisoned = true;
                            return Err(reject("actual saved Program differs from checked Program metadata"));
                        }
                        (Some(source_id), Some(program_index))
                    }
                    _ => {
                        self.poisoned = true;
                        return Err(reject("worker returned partial checked Program provenance"));
                    }
                };
                if !owner.starts_with("program-")
                    || !owner.is_ascii()
                    || self.programs.insert(owner.to_owned(), ProgramOwner {
                        source_id, program_index, descriptor:descriptor.clone(),
                    }).is_some() {
                    self.poisoned = true;
                    return Err(reject("worker returned duplicate or invalid saved Program owner"));
                }
            } else if cell["kind"] == 4 {
                self.poisoned = true;
                return Err(reject("worker returned an untyped saved Program"));
            }
        }
        Ok(())
    }

    fn release_program(&mut self, request: &Value) -> Result<Value, String> {
        object(request, &["operation", "owner"])?;
        let owner = text(request, "owner", 256)?;
        if !self.programs.contains_key(owner) {
            return Err(reject("unknown saved Program owner"));
        }
        let report = self.send(&json!({"operation":"release-program","owner":owner}), &[])?;
        if report["outcome"] == "program-released" {
            self.programs.remove(owner);
        }
        Ok(report)
    }

    fn discard(&mut self, request: &Value) -> Result<Value, String> {
        object(request, &["operation", "id"])?;
        let selected_id = id(request, "id")?;
        self.require_grant("discard", Some(&selected_id), None)?;
        if !self.candidates.contains_key(&selected_id) {
            return Err(reject("unknown installed candidate to discard"));
        }
        self.send(&json!({"operation":"discard","id":selected_id,
            "host_owner":self.authority.owner}), &[])
    }

    fn replay(&mut self, request: &Value) -> Result<Value, String> {
        object(request, &["operation", "token", "inputs", "refs",
            "expected_trace", "expected_stack", "expected_identity"])?;
        let token = text(request, "token", 256)?;
        let replay = self.replays.get(token).ok_or_else(|| reject("unknown frozen replay owner"))?;
        if request.get("expected_identity").is_some_and(|identity|
            identity != &replay.frozen_identity) {
            return Ok(json!({"schema":"noble-live-slot-report/v1","stage":"slot",
                "outcome":"replay-diverged","guest_requests":0,"protected_operations":0,
                "diagnostic":"frozen replay identity differs from the host-retained receipt"}));
        }
        let mut invocation = self.invocation(request, &replay.caller_id)?;
        invocation["operation"] = json!("replay");
        invocation["token"] = json!(token);
        invocation["expected_identity"] = replay.frozen_identity.clone();
        let trace = request.get("expected_trace").and_then(Value::as_array)
            .filter(|trace| trace.len() <= 4096)
            .ok_or_else(|| reject("replay requires a bounded expected trace"))?;
        let stack = request.get("expected_stack").and_then(Value::as_array)
            .filter(|stack| stack.len() <= MAX_TYPES)
            .ok_or_else(|| reject("replay requires a bounded expected stack"))?;
        for cell in stack {
            object(cell, &["kind", "value"])?;
            if uint(cell, "kind")? == 4
                || text(cell, "value", 64)?.parse::<i64>().is_err() {
                return Err(reject("unavailable replay result cell"));
            }
        }
        invocation["expected_trace"] = json!(trace);
        invocation["expected_stack"] = json!(stack);
        self.worker_invocation_started = true;
        self.send(&invocation, &[])
    }

    fn release_replay(&mut self, request: &Value) -> Result<Value, String> {
        object(request, &["operation", "token"])?;
        let token = text(request, "token", 256)?;
        if !self.replays.contains_key(token) {
            return Err(reject("unknown frozen replay owner"));
        }
        let result = self.send(&json!({"operation":"release-replay","token":token}), &[])?;
        if result["outcome"] == "replay-released" {
            self.replays.remove(token);
        }
        Ok(result)
    }

    fn invocation(&self, request: &Value, module_id: &str) -> Result<Value, String> {
        let candidate = self.candidates.get(module_id).ok_or_else(|| reject("unknown checked caller"))?;
        let values = request.get("inputs").and_then(Value::as_array)
            .filter(|items| items.len() <= MAX_TYPES).ok_or_else(|| reject("invalid bounded root inputs"))?;
        let refs = request.get("refs").and_then(Value::as_array)
            .filter(|items| items.len() <= MAX_TYPES).ok_or_else(|| reject("invalid bounded root reference bindings"))?;
        let mut logical = 0usize;
        let mut expected_refs = 0usize;
        let mut resource_ordinal = 0usize;
        let mut bindings = Vec::new();
        let mut injections = Vec::new();
        for (position, ty) in candidate.input.iter().enumerate() {
            match ty {
                Ty::LiveRef(input, output, ceiling) => {
                    let binding = refs.get(expected_refs).ok_or_else(|| reject("missing ordered root reference"))?;
                    object(binding, &["position", "ordinal", "slot"])?;
                    let declared_position = uint(binding, "position")? as usize;
                    let ordinal = uint(binding, "ordinal")? as usize;
                    let slot = id(binding, "slot")?;
                    let required = self.authority.slots.get(&slot).ok_or_else(|| reject("unregistered bound root slot"))?;
                    if declared_position != position || ordinal != expected_refs
                        || canonical_stack(input, 0)? != required.input
                        || canonical_stack(output, 0)? != required.output
                        || effects(ceiling)? != required.ceiling {
                        return Err(reject("root reference binding disagrees with checked caller or slot"));
                    }
                    self.require_grant("dispatch", Some(&slot), None)?;
                    bindings.push(json!({"position":position,"ordinal":ordinal,"slot":slot}));
                    expected_refs += 1;
                }
                Ty::I64 => {
                    let value = values.get(logical).ok_or_else(|| reject("missing ordered root input"))?;
                    object(value, &["kind", "value"])?;
                    let Some(number) = value["value"].as_str().and_then(|number| number.parse::<i64>().ok()) else {
                        return Err(reject("root input is not a bounded signed i64"));
                    };
                    if value["kind"] != "i64" { return Err(reject("root input is not I64")); }
                    injections.push(json!({"kind":"i64","value":number.to_string()}));
                    logical += 1;
                }
                Ty::Bool => {
                    let value = values.get(logical).ok_or_else(|| reject("missing ordered root input"))?;
                    object(value, &["kind", "value"])?;
                    if value["kind"] != "bool" || !value["value"].is_boolean() {
                        return Err(reject("root input is not Bool"));
                    }
                    injections.push(value.clone());
                    logical += 1;
                }
                Ty::Unit => {
                    let value = values.get(logical).ok_or_else(|| reject("missing ordered root input"))?;
                    object(value, &["kind"])?;
                    if value["kind"] != "unit" {
                        return Err(reject("root input is not Unit"));
                    }
                    injections.push(value.clone());
                    logical += 1;
                }
                Ty::Program(input, output, allowed) => {
                    let value = values.get(logical).ok_or_else(|| reject("missing ordered root Program"))?;
                    object(value, &["kind", "owner"])?;
                    let owner = text(value, "owner", 256)?;
                    let expected = format!("Program([{}]->[{}]!{{{}}})",
                        canonical_stack(input, 1)?.join(","),
                        canonical_stack(output, 1)?.join(","),
                        effects(allowed)?.join(","));
                    if value["kind"] != "program"
                        || self.programs.get(owner).map(|saved| &saved.descriptor) != Some(&expected) {
                        return Err(reject("root Program differs from checked ordered type"));
                    }
                    injections.push(json!({"kind":"program","owner":owner}));
                    logical += 1;
                }
                Ty::Nominal(identity, _) => {
                    let value = values.get(logical).ok_or_else(|| reject("missing ordered root resource"))?;
                    object(value, &["kind", "module", "ordinal", "owner"])?;
                    if !self.authority.resources.iter().any(|resource| resource.id == *identity) {
                        return Err(reject("unregistered root nominal"));
                    }
                    if value["kind"] != "resource" || numbers(value, "module")? != identity.module
                        || uint(value, "ordinal")? != identity.ordinal || text(value, "owner", 256)? != self.authority.owner {
                        return Err(reject("unowned root resource input"));
                    }
                    self.require_grant("issue-resource", None, Some(&format!("{}:{}", identity.module, identity.ordinal)))?;
                    let resource = self.authority.resources.iter()
                        .find(|resource| resource.id == *identity)
                        .ok_or_else(|| reject("missing selected nominal descriptor"))?;
                    injections.push(json!({"kind":"resource","module":identity.module.to_string(),
                        "position":position,"ordinal":resource_ordinal,
                        "nominalOrdinal":identity.ordinal,"resourceKind":resource.kind.0,
                        "owner":self.authority.owner}));
                    resource_ordinal += 1;
                    logical += 1;
                }
                _ => return Err(reject("unsupported root value type for selected slot worker")),
            }
        }
        if values.len() != logical || refs.len() != expected_refs {
            return Err(reject("extra root value or reference input"));
        }
        let message = json!({"operation":"invoke",
            "id":module_id,"inputs":injections,"refs":bindings});
        Ok(message)
    }

    fn inspect(&self, request: &Value, op: &str) -> Result<Value, String> {
        match op {
            "reflect" => {
                object(request, &["operation", "id", "site_id"])?;
                let id = id(request, "id")?;
                if !self.candidates.contains_key(&id) { return Err(reject("unknown checked module")); }
                Ok(json!({"operation":op,"id":id,"site":uint(request, "site_id")?}))
            }
            "trace" => {
                object(request, &["operation"])?;
                Ok(json!({"operation":"trace"}))
            }
            "policy" => {
                object(request, &["operation", "grant"])?;
                let grant = request.get("grant").ok_or_else(|| reject("missing host policy decision"))?;
                object(grant, &["operation", "slotId", "nominalId", "allowed"])?;
                let operation = text(grant, "operation", 32)?;
                let slot = grant.get("slotId").map(|_| id(grant, "slotId")).transpose()?;
                let nominal = grant.get("nominalId")
                    .map(|_| text(grant, "nominalId", 64).map(str::to_owned)).transpose()?;
                if slot.is_some() == nominal.is_some() {
                    return Err(reject("policy change must name one selected scope"));
                }
                let allowed = grant.get("allowed").and_then(Value::as_bool)
                    .ok_or_else(|| reject("policy requires an explicit boolean decision"))?;
                if !self.authority.grants.iter().any(|selected| selected["operation"] == operation
                    && selected.get("slotId").and_then(Value::as_str) == slot.as_deref()
                    && selected.get("nominalId").and_then(Value::as_str) == nominal.as_deref()
                    && (!allowed || selected["allowed"] == true)) {
                    return Err(reject("policy cannot add or widen selected host authority"));
                }
                Ok(json!({"operation":"policy","grant":grant}))
            }
            _ => Err(reject("unknown host inspection command")),
        }
    }
}
