//! One-shot, owner-bound typed transition from a closed local package.
//! Package rows and value frames are untrusted claims; the selected schema
//! module and retained exported Program owner are source-checked separately.

use super::output::{ErrorContext, Failure, Report};
use super::worker_package::{InterfaceReject, Package};
use super::worker_value::{self, Refusal, Schema};
use noble_kernel::types::Ty;
use serde_json::{json, Value};

const USAGE: &str =
    "usage: noble worker typed PACKAGE --schema SCHEMA --state STATE_JSON --event EVENT_JSON";
const PREPARE_USAGE: &str = "usage: noble worker prepare SOURCE --schema SCHEMA --out PACKAGE";
const PACKAGE_BYTES: usize = 262_144;
/// The typed engine refuses a worker literal arena that ends beyond this.
const LITERAL_ARENA_END: u32 = 983_040;
const BUILTIN_COUNT: u32 = 22;
const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 4_096,
    depth: 32,
    work: 3_000_000,
};
// The selected schema and root are distinct retained modules in one session.
// Each preparation still has the independent LIMITS.bytes ceiling.
const RETAINED_SCHEMA_ROOT_BYTES: u32 = LIMITS
    .bytes
    .checked_mul(2)
    .expect("two bounded Worker module sources");

fn failure(outcome: &'static str, message: &'static str) -> Failure {
    Failure::new(
        ErrorContext {
            stage: "admission",
            outcome,
        },
        message,
    )
}

fn value_failure(problem: Refusal) -> Failure {
    match problem {
        Refusal::Invalid => failure(
            "invalid-input",
            "worker value or package contradicts checked source",
        ),
        Refusal::Unsupported => failure(
            "unsupported",
            "worker value or package uses an unsupported feature",
        ),
        Refusal::Exhausted => failure(
            "exhausted",
            "worker value or package exceeded the shared budget",
        ),
        Refusal::Ineligible => failure(
            "eligibility-reject",
            "worker capture hides a resource; resources are never capturable worker data",
        ),
    }
}

/// WORKER-02 interface rejections. All are decided at admission, before any
/// worker engine starts, so the reported guest requests and protected
/// operations are zero.
fn interface_failure(problem: InterfaceReject) -> Failure {
    match problem {
        InterfaceReject::Interface => failure(
            "interface-reject",
            "declared worker interface differs from State Event -- State List<Action>",
        ),
        InterfaceReject::Schema => failure(
            "schema-reject",
            "declared worker interface names a schema identity outside the checked host catalog",
        ),
        InterfaceReject::Effect => failure(
            "effect-reject",
            "declared worker effects exceed the empty dispatch ceiling",
        ),
        InterfaceReject::Malformed => failure("invalid-input", "malformed worker interface claim"),
    }
}

pub(super) fn bounded_diagnostic(message: &str) -> &str {
    // Bound the serialized JSON string, including its quotes. Escaping a
    // control byte can cost six bytes even when the input is shorter than 512.
    let mut encoded = 2;
    let mut end = 0;
    for (index, character) in message.char_indices() {
        let width = match character {
            '\u{0008}' | '\t' | '\n' | '\u{000c}' | '\r' | '"' | '\\' => 2,
            '\u{0000}'..='\u{001f}' => 6,
            _ => character.len_utf8(),
        };
        if encoded + width > 512 {
            break;
        }
        encoded += width;
        end = index + character.len_utf8();
    }
    &message[..end]
}

pub(super) fn read_bounded_regular_file(
    path: &std::path::Path,
    limit: usize,
) -> Result<Vec<u8>, Failure> {
    // A FIFO with no writer blocks in open(2) before an fstat regular-file
    // check could run. Preserve symlink-to-regular-file behavior, but open the
    // selected target nonblocking and inspect that same opened descriptor.
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = attempt!(std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(super::framing::io_error));
    let metadata = attempt!(file.metadata().map_err(super::framing::io_error));
    if !metadata.is_file() {
        return Err(failure(
            "invalid-input",
            "worker input must be a regular bounded file",
        ));
    }
    if metadata.len() > limit as u64 {
        return Err(failure(
            "exhausted",
            "worker input byte limit exceeded before allocation",
        ));
    }
    let count = attempt!(usize::try_from(metadata.len())
        .map_err(|_| failure("exhausted", "worker input file length overflow")));
    let mut bytes = vec![0_u8; count];
    attempt!(std::io::Read::read_exact(&mut file, &mut bytes).map_err(super::framing::io_error));
    let mut extra = [0_u8; 1];
    if attempt!(std::io::Read::read(&mut file, &mut extra).map_err(super::framing::io_error)) != 0 {
        return Err(failure(
            "exhausted",
            "worker file changed beyond its bounded length",
        ));
    }
    Ok(bytes)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-maintainers; the report constructor maps each optional observation to one fixed JSON field."
)]
fn report(
    outcome: &str,
    stage: &str,
    package: Option<&[u8]>,
    schema: Option<&[u8]>,
    source: Option<&[u8]>,
    module: Option<&Value>,
    owner: Option<&Value>,
    output: Option<&Value>,
    guest_requests: u64,
    protected_operations: u64,
    diagnostic: Option<&str>,
) -> Report {
    let value = json!({
        "schema":"noble-worker-typed/v1", "profile":"Worker-Typed-Local",
        "stage":stage, "outcome":outcome, "interface":"State Event -- State List<Action> ! {}",
        "accounting_revision":"core-managed-linear-memory/step-limit-4096",
        "step_ceiling":4096, "invocation":if stage == "execution" {Some("typed-1")} else {None},
        "ownership_disposition":if stage == "execution" {"local-no-guest-resource"} else {"not-admitted"},
        "package_sha256":package.map(crate::workflow::intrinsic::sha256),
        "schema_source_sha256":schema.map(crate::workflow::intrinsic::sha256),
        "source_sha256":source.map(crate::workflow::intrinsic::sha256),
        "module":module, "checked_program_owner":owner, "typed_output":output,
        "guest_requests":guest_requests,"protected_operations":protected_operations,
        "diagnostic":diagnostic.map(bounded_diagnostic),
    });
    Report {
        outcome: outcome.to_owned(),
        json: value.to_string(),
    }
}

/// The host schema must be one ordinary checked module.
fn schema_module(
    session: &noble_contracts::source::ModuleSession,
    schema_source: &[u8],
) -> Result<noble_contracts::source::ModulePrepared, Failure> {
    let checked = attempt!(session
        .prepare(schema_source, &[], LIMITS)
        .map_err(Failure::source));
    if checked.kind() != noble_contracts::source::ModuleKind::Module
        || checked.proof_obligations().is_some()
        || !checked.bindings().is_empty()
    {
        return Err(failure(
            "unsupported",
            "host schema requires an ordinary checked module",
        ));
    }
    Ok(checked)
}

/// The checked catalog of exactly the four local WorkerData@1 nominal types,
/// with State, Event, Action and AttemptId resolved from the module's exports.
fn schema_catalog(
    checked: &noble_contracts::source::ModulePrepared,
) -> Result<(Vec<Schema>, [Ty; 4]), Failure> {
    let (module_id, _, _) = attempt!(checked
        .resolved_module()
        .filter(|(_, name, version)| name == "WorkerData" && *version == 1)
        .ok_or_else(|| failure("unsupported", "host schema must be checked WorkerData@1")));
    let declarations = attempt!(checked
        .checked_nominals()
        .ok_or_else(|| failure("invalid-input", "missing checked schema catalog")));
    let mut catalog = Vec::with_capacity(4);
    for declaration in declarations {
        if catalog.len() == 4 {
            return Err(failure(
                "unsupported",
                "worker schema has extra nominal declarations",
            ));
        }
        catalog.push(attempt!(
            Schema::checked(&declaration).map_err(value_failure)
        ));
    }
    catalog.sort_by_key(|entry| (entry.id().module, entry.id().ordinal));
    if catalog.len() != 4 || catalog.iter().any(|entry| entry.id().module != module_id) {
        return Err(failure(
            "unsupported",
            "worker schema must declare exactly four local nominal types",
        ));
    }
    let state = attempt!(checked
        .resolve_type("WorkerData@1.State")
        .map_err(Failure::source));
    let event = attempt!(checked
        .resolve_type("WorkerData@1.Event")
        .map_err(Failure::source));
    let action = attempt!(checked
        .resolve_type("WorkerData@1.Action")
        .map_err(Failure::source));
    let attempt_id = attempt!(checked
        .resolve_type("WorkerData@1.AttemptId")
        .map_err(Failure::source));
    if ![&state, &event, &action, &attempt_id]
        .iter()
        .all(|ty| matches!(ty, Ty::Nominal(_, _)))
    {
        return Err(failure(
            "invalid-input",
            "worker schema lacks required exported nominal types",
        ));
    }
    for expected in [&state, &event, &action, &attempt_id] {
        if !catalog.iter().any(|schema| &schema.ty == expected) {
            return Err(failure(
                "invalid-input",
                "worker schema identity does not match checked exports",
            ));
        }
    }
    Ok((catalog, [state, event, action, attempt_id]))
}

/// The catalog rows of the selected interface's State, Event and Action.
fn interface_rows<'rows>(
    catalog: &'rows [Schema],
    types: &[Ty; 4],
) -> Result<[&'rows Schema; 3], Failure> {
    let row = |ty: &Ty, missing: &'static str| {
        catalog
            .iter()
            .find(|row| &row.ty == ty)
            .ok_or_else(|| failure("invalid-input", missing))
    };
    Ok([
        attempt!(row(&types[0], "missing State schema")),
        attempt!(row(&types[1], "missing Event schema")),
        attempt!(row(&types[2], "missing Action schema")),
    ])
}

/// The host catalog, bound to a package's schema rows and declared
/// interface claim (WORKER-02).
fn selected_schema(
    checked: &noble_contracts::source::ModulePrepared,
    package: &Package,
    schema_source: &[u8],
) -> Result<(Vec<Schema>, [Ty; 4]), Failure> {
    let (catalog, types) = attempt!(schema_catalog(checked));
    attempt!(package.bind_schemas(&catalog, schema_source).map_err(|_| {
        failure(
            "schema-reject",
            "package schema rows differ from the checked host schema catalog",
        )
    }));
    let [state, event, action] = attempt!(interface_rows(&catalog, &types));
    attempt!(package
        .check_interface(&catalog, state, event, action)
        .map_err(interface_failure));
    Ok((catalog, types))
}

/// The root must be one ordinary pure module that declares no nominal types.
fn root_module(
    session: &noble_contracts::source::ModuleSession,
    source: &[u8],
) -> Result<noble_contracts::source::ModulePrepared, Failure> {
    let declared = attempt!(session
        .prepare(source, &[], LIMITS)
        .map_err(Failure::source));
    if declared.kind() != noble_contracts::source::ModuleKind::Module
        || declared.proof_obligations().is_some()
        || !declared.bindings().is_empty()
        || declared
            .checked_nominals()
            .is_none_or(|mut values| values.next().is_some())
    {
        return Err(failure(
            "unsupported",
            "worker package must contain one ordinary pure module",
        ));
    }
    Ok(declared)
}

/// The checked owner of the exported `transition` Program.
fn root_owner(
    declared: &noble_contracts::source::ModulePrepared,
) -> Result<noble_contracts::source::CheckedProgramOwner, Failure> {
    declared.checked_program_owner("transition").ok_or_else(|| {
        failure(
            "invalid-input",
            "package has no checked exported transition Program",
        )
    })
}

/// The checked, acyclic local closure of the exported `transition`.
fn root_closure(
    declared: &noble_contracts::source::ModulePrepared,
) -> Result<Vec<noble_contracts::source::CheckedProgramOwner>, Failure> {
    declared.checked_program_closure("transition").map_err(|_| {
        failure(
            "invalid-input",
            "package has a cyclic, foreign, or unreachable definition",
        )
    })
}

/// Commit the checked root and check its exact invocation against the
/// selected `State Event -- State List<Action> ! {}` interface.
fn checked_transition(
    session: noble_contracts::source::ModuleSession,
    declared: noble_contracts::source::ModulePrepared,
    owner: noble_contracts::source::CheckedProgramOwner,
    input: &[Ty; 4],
) -> Result<
    (
        noble_contracts::source::ModulePrepared,
        noble_contracts::source::CheckedProgramOwner,
        noble_contracts::source::ModuleSession,
    ),
    Failure,
> {
    let (module_id, module_name, version) = attempt!(declared
        .resolved_module()
        .ok_or_else(|| failure("invalid-input", "missing checked root module")));
    if module_id != owner.module_identity() || version != owner.module_version() {
        return Err(failure(
            "invalid-input",
            "root module differs from checked owner",
        ));
    }
    let (session, linked) = session.commit(declared);
    attempt!(linked.map_err(Failure::source));
    let invocation = format!("{module_name}@{version}.transition");
    let prepared = attempt!(session
        .prepare(
            invocation.as_bytes(),
            &[input[0].clone(), input[1].clone()],
            LIMITS
        )
        .map_err(Failure::source));
    if prepared.kind() != noble_contracts::source::ModuleKind::Expression
        || prepared.output() != [input[0].clone(), Ty::List(Box::new(input[2].clone()))]
    {
        return Err(failure(
            "interface-reject",
            "transition differs from State Event -- State List<Action>",
        ));
    }
    let submission = attempt!(prepared
        .submission()
        .ok_or_else(|| failure("invalid-input", "no checked transition body")));
    let output = [input[0].clone(), Ty::List(Box::new(input[2].clone()))];
    if submission.request.expected.stack_in != [input[0].clone(), input[1].clone()]
        || submission.request.expected.stack_out != output
        || !submission.request.expected.allowed_effects.is_empty()
        || submission
            .definitions
            .iter()
            .filter(|definition| {
                definition.identity == owner.definition_identity()
                    && submission
                        .environment
                        .definition_owners
                        .get(definition.definition.0 as usize)
                        == Some(&Some(module_id))
            })
            .count()
            != 1
        || submission
            .body
            .candidate
            .nodes
            .iter()
            .filter(|node| {
                matches!(node,
            noble_kernel::untrusted::Node::Invocation { def, .. }
                if submission.definitions.iter().any(|definition| definition.definition == *def
                    && definition.identity == owner.definition_identity()))
            })
            .count()
            != 1
    {
        return Err(failure(
            "interface-reject",
            "root invocation has no unique checked Program owner or pure interface",
        ));
    }
    Ok((prepared, owner, session))
}

fn checked_program(
    session: noble_contracts::source::ModuleSession,
    package: &Package,
    input: &[Ty; 4],
) -> Result<
    (
        noble_contracts::source::ModulePrepared,
        noble_contracts::source::CheckedProgramOwner,
        noble_contracts::source::ModuleSession,
    ),
    Failure,
> {
    let declared = attempt!(root_module(&session, package.source().as_bytes()));
    let owner = attempt!(root_owner(&declared));
    let closure = attempt!(root_closure(&declared));
    attempt!(package.bind_owner(&owner, &closure).map_err(value_failure));
    checked_transition(session, declared, owner, input)
}

fn owner_record(owner: &noble_contracts::source::CheckedProgramOwner) -> Value {
    json!({"module_id":owner.module_identity().to_string(),
        "version":owner.module_version(),"definition_ordinal":owner.definition_ordinal(),
        "definition_identity":owner.definition_identity().to_string(),
        "checked_named_slot":owner.definition_slot()})
}

/// The local v1 package for one checked root. Every field derives from the
/// checked owner and closure, the host catalog and the selected interface;
/// admission re-derives and compares each of them.
fn package_manifest(
    source: &str,
    schema_source: &[u8],
    catalog: &[Schema],
    types: &[Ty; 4],
    owner: &noble_contracts::source::CheckedProgramOwner,
    closure: &[noble_contracts::source::CheckedProgramOwner],
) -> Result<Value, Failure> {
    let [state, event, action] = attempt!(interface_rows(catalog, types));
    let schema_sha = crate::workflow::intrinsic::sha256(schema_source);
    let source_sha = crate::workflow::intrinsic::sha256(source.as_bytes());
    let definitions: Vec<Value> = closure
        .iter()
        .map(|definition| {
            json!({
                "module_id":definition.module_identity().to_string(),
                "version":definition.module_version(),
                "definition_ordinal":definition.definition_ordinal(),
                "definition_identity":definition.definition_identity().to_string(),
                "source_utf8":source,"source_sha256":source_sha
            })
        })
        .collect();
    let mut builtins = Vec::with_capacity(BUILTIN_COUNT as usize);
    for ordinal in 0..BUILTIN_COUNT {
        let name = attempt!(noble_contracts::source::pure_builtin_spelling(ordinal)
            .ok_or_else(|| failure("internal-failure", "pure builtin table is incomplete")));
        builtins.push(json!({"ordinal":ordinal,"name":name,
            "semantic_revision":super::worker_package::REVISION}));
    }
    Ok(json!({
        "format":"noble-worker-local-package/v1",
        "semantic_revision":super::worker_package::REVISION,
        "root":{"source_utf8":source,"module_id":owner.module_identity().to_string(),
            "version":owner.module_version(),"definition_ordinal":owner.definition_ordinal(),
            "definition_identity":owner.definition_identity().to_string()},
        "definitions":definitions,
        "schemas":catalog.iter().map(|row| row.package_row(&schema_sha)).collect::<Vec<_>>(),
        "builtins":builtins,
        "host_contracts":[],"captures":[],"artifact":null,
        "interface":{"inputs":[state.wire(),event.wire()],
            "outputs":[state.wire(),{"list":action.wire()}],"effects":[]}
    }))
}

/// Service effects of one explicit preparation, each counted where the
/// service does that work. They are the host compiler service's effects,
/// never the candidate's latent effects: the service starts no engine.
#[derive(Default)]
pub(super) struct PreparationTrace {
    stage: &'static str,
    input_files_read: u32,
    source_checks: u32,
    closure_walks: u32,
    backend_compilations: u32,
    packages_written: u32,
    owner: Option<Value>,
    wat_sha256: Option<String>,
}

impl PreparationTrace {
    fn enter(&mut self, stage: &'static str) {
        self.stage = stage;
    }
    fn count(counter: &mut u32) {
        *counter = counter.saturating_add(1);
    }
}

/// The explicit host preparation service of the local Worker-Typed profile.
/// Only `source` comes from the candidate. The host selects the schema
/// module, the exact `State Event -- State List<Action> ! {}` interface, the
/// `transition` root, the limits and the encoding. Each stage runs only after
/// the previous one succeeded, so a refusal enters no later stage, and the
/// returned package is exactly what admission accepts.
pub(super) fn prepare(
    source: &[u8],
    schema: &[u8],
    trace: &mut PreparationTrace,
) -> Result<Vec<u8>, Failure> {
    trace.enter("inputs");
    let text = attempt!(std::str::from_utf8(source)
        .map_err(|_| failure("invalid-input", "worker source must be UTF-8 text")));
    let retained = attempt!(noble_contracts::source::RetainedSourceLimit::new(
        RETAINED_SCHEMA_ROOT_BYTES
    )
    .ok_or_else(|| failure("internal-failure", "Worker retained source limit")));
    let session = attempt!(
        noble_contracts::source::ModuleSession::new(&[], retained).map_err(Failure::source)
    );
    trace.enter("schema-check");
    PreparationTrace::count(&mut trace.source_checks);
    let schema_checked = attempt!(schema_module(&session, schema));
    let (catalog, types) = attempt!(schema_catalog(&schema_checked));
    let (session, linked) = session.commit(schema_checked);
    attempt!(linked.map_err(Failure::source));
    trace.enter("root-check");
    PreparationTrace::count(&mut trace.source_checks);
    let declared = attempt!(root_module(&session, source));
    let owner = attempt!(root_owner(&declared));
    trace.owner = Some(owner_record(&owner));
    PreparationTrace::count(&mut trace.closure_walks);
    let closure = attempt!(root_closure(&declared));
    trace.enter("transition-check");
    PreparationTrace::count(&mut trace.source_checks);
    let (checked, owner, _session) = attempt!(checked_transition(session, declared, owner, &types));
    let submission = attempt!(checked
        .submission()
        .ok_or_else(|| failure("invalid-input", "checked Program body absent")));
    trace.enter("backend");
    PreparationTrace::count(&mut trace.backend_compilations);
    let compiled = attempt!(noble_wasm::source::Compiler::new_worker()
        .prepare(submission)
        .map_err(Failure::backend));
    if compiled
        .worker_literal_end()
        .is_none_or(|end| end > LITERAL_ARENA_END)
    {
        return Err(failure(
            "exhausted",
            "worker literal arena exceeds the typed engine bound",
        ));
    }
    trace.wat_sha256 = Some(crate::workflow::intrinsic::sha256(compiled.wat()));
    trace.enter("encode");
    let manifest = attempt!(package_manifest(
        text, schema, &catalog, &types, &owner, &closure
    ));
    let bytes = attempt!(serde_json::to_vec(&manifest)
        .map_err(|_| failure("internal-failure", "worker package encoding failed")));
    if bytes.len() > PACKAGE_BYTES {
        return Err(failure(
            "exhausted",
            "worker package exceeds its byte limit",
        ));
    }
    attempt!(Package::parse(&bytes).map_err(value_failure));
    Ok(bytes)
}

fn preparation_report(
    outcome: &str,
    trace: &PreparationTrace,
    source: Option<&[u8]>,
    schema: Option<&[u8]>,
    package: Option<&[u8]>,
    diagnostic: Option<&str>,
) -> Report {
    let value = json!({
        "schema":"noble-worker-preparation/v1", "profile":"Worker-Typed-Local",
        "service":"worker-prepare", "stage":trace.stage, "outcome":outcome,
        "interface":"State Event -- State List<Action> ! {}", "root_export":"transition",
        "limits":{"bytes":LIMITS.bytes,"nodes":LIMITS.nodes,"depth":LIMITS.depth,
            "work":LIMITS.work,"retained_source_bytes":RETAINED_SCHEMA_ROOT_BYTES,
            "package_bytes":PACKAGE_BYTES},
        "candidate_source_sha256":source.map(crate::workflow::intrinsic::sha256),
        "schema_source_sha256":schema.map(crate::workflow::intrinsic::sha256),
        "service_effects":{"input_files_read":trace.input_files_read,
            "source_checks":trace.source_checks,"closure_walks":trace.closure_walks,
            "backend_compilations":trace.backend_compilations,
            "packages_written":trace.packages_written},
        "checked_program_owner":trace.owner, "wat_sha256":trace.wat_sha256,
        "package_sha256":package.map(crate::workflow::intrinsic::sha256),
        "package_bytes":package.map(<[u8]>::len),
        "diagnostic":diagnostic.map(bounded_diagnostic),
    });
    Report {
        outcome: outcome.to_owned(),
        json: value.to_string(),
    }
}

/// Publish only a new file. An existing destination is never overwritten.
fn publish(destination: &std::path::Path, bytes: &[u8]) -> Result<(), Failure> {
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(failure(
                "invalid-input",
                "package destination already exists",
            ))
        }
        Err(error) => return Err(super::framing::io_error(error)),
    };
    std::io::Write::write_all(&mut file, bytes).map_err(super::framing::io_error)
}

fn prepare_command(arguments: &[std::ffi::OsString], trace: &mut PreparationTrace) -> Report {
    trace.enter("inputs");
    if arguments.len() != 7 || arguments[3] != "--schema" || arguments[5] != "--out" {
        return preparation_report(
            "invalid-input",
            trace,
            None,
            None,
            None,
            Some(PREPARE_USAGE),
        );
    }
    let mut inputs = Vec::with_capacity(2);
    for index in [2, 4] {
        match read_bounded_regular_file(std::path::Path::new(&arguments[index]), 65_536) {
            Ok(bytes) => {
                PreparationTrace::count(&mut trace.input_files_read);
                inputs.push(bytes);
            }
            Err(problem) => {
                return preparation_report(
                    problem.context.outcome,
                    trace,
                    None,
                    None,
                    None,
                    Some(&problem.message),
                )
            }
        }
    }
    let (source, schema) = (inputs[0].as_slice(), inputs[1].as_slice());
    let package = match prepare(source, schema, trace) {
        Ok(package) => package,
        Err(problem) => {
            return preparation_report(
                problem.context.outcome,
                trace,
                Some(source),
                Some(schema),
                None,
                Some(&problem.message),
            )
        }
    };
    trace.enter("publish");
    if let Err(problem) = publish(std::path::Path::new(&arguments[6]), &package) {
        return preparation_report(
            problem.context.outcome,
            trace,
            Some(source),
            Some(schema),
            None,
            Some(&problem.message),
        );
    }
    PreparationTrace::count(&mut trace.packages_written);
    preparation_report(
        "ready",
        trace,
        Some(source),
        Some(schema),
        Some(&package),
        None,
    )
}

/// `noble worker prepare`: one explicit host preparation, reported as its
/// own service trace. It never evaluates; `noble worker typed` re-admits the
/// published package in a separate invocation.
pub(crate) fn run_prepare(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let result = prepare_command(arguments, &mut PreparationTrace::default());
    if let Err(error) = super::output::print(&result) {
        eprintln!("noble: {}", error.message);
        return std::process::ExitCode::from(error.exit());
    }
    std::process::ExitCode::from(result.exit())
}

pub(super) fn execute(
    package_bytes: &[u8],
    schema_bytes: &[u8],
    state_bytes: &[u8],
    event_bytes: &[u8],
) -> Result<Report, Failure> {
    let package = attempt!(Package::parse(package_bytes).map_err(value_failure));
    let retained = noble_contracts::source::RetainedSourceLimit::new(RETAINED_SCHEMA_ROOT_BYTES)
        .expect("combined Worker schema and root source limit");
    let session = attempt!(
        noble_contracts::source::ModuleSession::new(&[], retained).map_err(Failure::source)
    );
    let schema_checked = attempt!(schema_module(&session, schema_bytes));
    let (catalog, types) = attempt!(selected_schema(&schema_checked, &package, schema_bytes));
    let (session, linked) = session.commit(schema_checked);
    attempt!(linked.map_err(Failure::source));
    let values = attempt!(worker_value::admit(
        [state_bytes, event_bytes],
        [&types[0], &types[1]],
        &catalog,
        package.memory,
        package.remaining_work
    )
    .map_err(value_failure));
    let (checked, owner, _session) = attempt!(checked_program(session, &package, &types));
    let submission = attempt!(checked
        .submission()
        .ok_or_else(|| failure("invalid-input", "checked Program body absent")));
    let compiled = attempt!(noble_wasm::source::Compiler::new_worker()
        .prepare(submission)
        .map_err(Failure::backend));
    let owner_record = owner_record(&owner);
    let input = attempt!(serde_json::to_string(&values)
        .map_err(|_| failure("internal-failure", "validated input encoding failed")));
    let mut engine = match super::worker::Engine::start_typed_worker() {
        Ok(engine) => engine,
        Err(error) => {
            return Ok(report(
                error.context.outcome,
                "execution",
                Some(package_bytes),
                Some(schema_bytes),
                Some(package.source().as_bytes()),
                None,
                Some(&owner_record),
                None,
                0,
                0,
                Some(&error.message),
            ))
        }
    };
    let prepared = match engine.prepare_typed(
        &compiled,
        package.source().as_bytes(),
        1,
        &catalog,
        [&types[0], &types[1]],
        schema_bytes,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            return Ok(report(
                error.context.outcome,
                "execution",
                Some(package_bytes),
                Some(schema_bytes),
                Some(package.source().as_bytes()),
                None,
                Some(&owner_record),
                None,
                0,
                0,
                Some(&error.message),
            ))
        }
    };
    let preparation: Value = match serde_json::from_str(&prepared.json) {
        Ok(value) => value,
        Err(_) => {
            return Ok(report(
                "internal-failure",
                "execution",
                Some(package_bytes),
                Some(schema_bytes),
                Some(package.source().as_bytes()),
                None,
                Some(&owner_record),
                None,
                0,
                0,
                Some("malformed typed worker preparation"),
            ))
        }
    };
    if prepared.outcome != "ready" {
        return Ok(report(
            &prepared.outcome,
            "execution",
            Some(package_bytes),
            Some(schema_bytes),
            Some(package.source().as_bytes()),
            preparation.get("module"),
            None,
            None,
            0,
            0,
            None,
        ));
    }
    let module = match preparation.get("module") {
        Some(module) => module,
        None => {
            return Ok(report(
                "internal-failure",
                "execution",
                Some(package_bytes),
                Some(schema_bytes),
                Some(package.source().as_bytes()),
                None,
                Some(&owner_record),
                None,
                0,
                0,
                Some("prepared artifact absent"),
            ))
        }
    };
    if module["source_sha256"] != crate::workflow::intrinsic::sha256(package.source().as_bytes())
        || module["wat_sha256"] != crate::workflow::intrinsic::sha256(compiled.wat())
        || module["worker_catalog"]["schema_source_sha256"]
            != crate::workflow::intrinsic::sha256(schema_bytes)
        || module["worker_literal_end"] != compiled.literal_end()
        || module["wasm_sha256"].as_str().is_none()
    {
        return Ok(report(
            "internal-failure",
            "execution",
            Some(package_bytes),
            Some(schema_bytes),
            Some(package.source().as_bytes()),
            None,
            Some(&owner_record),
            None,
            0,
            0,
            Some("compiled artifact is not bound to checked source and literal arena"),
        ));
    }
    let execution = match engine.execute_typed(&input) {
        Ok(execution) => execution,
        Err(error) => {
            return Ok(report(
                error.context.outcome,
                "execution",
                Some(package_bytes),
                Some(schema_bytes),
                Some(package.source().as_bytes()),
                Some(module),
                Some(&owner_record),
                None,
                0,
                0,
                Some(&error.message),
            ))
        }
    };
    let observation: Value = match serde_json::from_str(&execution.json) {
        Ok(value) => value,
        Err(_) => {
            return Ok(report(
                "internal-failure",
                "execution",
                Some(package_bytes),
                Some(schema_bytes),
                Some(package.source().as_bytes()),
                Some(module),
                Some(&owner_record),
                None,
                0,
                0,
                Some("malformed typed worker observation"),
            ))
        }
    };
    let requests = observation["guest_requests"].as_u64();
    let protected = observation["protected_operations"].as_u64();
    if observation["module"]["source_sha256"] != module["source_sha256"]
        || observation["module"]["wasm_sha256"] != module["wasm_sha256"]
        || requests != Some(0)
        || protected != Some(0)
        || observation["host_requests"] != 0
        || observation["effect_requests_unrecorded"] != 0
        || observation["ambient_fallback_calls"] != 0
        || observation["acquired_authority"] != false
    {
        return Ok(report(
            "internal-failure",
            "execution",
            Some(package_bytes),
            Some(schema_bytes),
            Some(package.source().as_bytes()),
            None,
            Some(&owner_record),
            None,
            requests.unwrap_or(0),
            protected.unwrap_or(0),
            Some("worker execution violates source or effect boundary"),
        ));
    }
    if execution.outcome == "normal" {
        let stack = match observation["stack"]
            .as_array()
            .filter(|stack| stack.len() == 2)
        {
            Some(stack) => stack,
            None => {
                return Ok(report(
                    "internal-failure",
                    "execution",
                    Some(package_bytes),
                    Some(schema_bytes),
                    Some(package.source().as_bytes()),
                    Some(module),
                    Some(&owner_record),
                    None,
                    0,
                    0,
                    Some("wrong worker output arity"),
                ))
            }
        };
        let actions_ty = Ty::List(Box::new(types[2].clone()));
        let [next, actions] = match worker_value::returned(
            [&stack[0], &stack[1]],
            [&types[0], &actions_ty],
            &catalog,
        ) {
            Ok(values) => values,
            Err(problem) => {
                return Ok(report(
                    if problem == Refusal::Exhausted {
                        "runtime-exhausted"
                    } else {
                        "internal-failure"
                    },
                    "execution",
                    Some(package_bytes),
                    Some(schema_bytes),
                    Some(package.source().as_bytes()),
                    Some(module),
                    Some(&owner_record),
                    None,
                    0,
                    0,
                    Some("returned value violates selected schema or observation budget"),
                ))
            }
        };
        let typed_output = json!({"state":next,"actions":actions});
        return Ok(report(
            "normal",
            "execution",
            Some(package_bytes),
            Some(schema_bytes),
            Some(package.source().as_bytes()),
            Some(module),
            Some(&owner_record),
            Some(&typed_output),
            0,
            0,
            None,
        ));
    }
    if !matches!(execution.outcome.as_str(), "runtime-exhausted" | "trap") {
        return Ok(report(
            "internal-failure",
            "execution",
            Some(package_bytes),
            Some(schema_bytes),
            Some(package.source().as_bytes()),
            Some(module),
            Some(&owner_record),
            None,
            0,
            0,
            Some("unknown typed worker outcome"),
        ));
    }
    Ok(report(
        &execution.outcome,
        "execution",
        Some(package_bytes),
        Some(schema_bytes),
        Some(package.source().as_bytes()),
        Some(module),
        Some(&owner_record),
        None,
        0,
        0,
        observation["quota_reason"].as_str(),
    ))
}

/// The second event is untrusted too. Admit its exact frame and the returned
/// state under the original host-selected schema before any shell operation.
pub(super) fn validate_secondary(
    package_bytes: &[u8],
    schema_bytes: &[u8],
    state_bytes: &[u8],
    event_bytes: &[u8],
) -> Result<(), Failure> {
    let package = attempt!(Package::parse(package_bytes).map_err(value_failure));
    let retained = noble_contracts::source::RetainedSourceLimit::new(LIMITS.bytes)
        .expect("Worker source limit");
    let session = attempt!(
        noble_contracts::source::ModuleSession::new(&[], retained).map_err(Failure::source)
    );
    let checked = attempt!(session
        .prepare(schema_bytes, &[], LIMITS)
        .map_err(Failure::source));
    let (catalog, types) = attempt!(selected_schema(&checked, &package, schema_bytes));
    attempt!(worker_value::admit(
        [state_bytes, event_bytes],
        [&types[0], &types[1]],
        &catalog,
        package.memory,
        package.remaining_work
    )
    .map_err(value_failure));
    Ok(())
}

pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    if arguments.len() != 9
        || arguments[1] != "typed"
        || arguments[3] != "--schema"
        || arguments[5] != "--state"
        || arguments[7] != "--event"
    {
        let result = report(
            "invalid-input",
            "admission",
            None,
            None,
            None,
            None,
            None,
            None,
            0,
            0,
            Some(USAGE),
        );
        let _ = super::output::print(&result);
        return std::process::ExitCode::from(result.exit());
    }
    let files = [(2, 262_144), (4, 65_536), (6, 65_536), (8, 65_536)].map(|(index, limit)| {
        read_bounded_regular_file(std::path::Path::new(&arguments[index]), limit)
    });
    let result = match files {
        [Ok(package), Ok(schema), Ok(state), Ok(event)] => {
            match execute(&package, &schema, &state, &event) {
                Ok(report) => report,
                Err(problem) => report(
                    problem.context.outcome,
                    "admission",
                    Some(&package),
                    Some(&schema),
                    None,
                    None,
                    None,
                    None,
                    0,
                    0,
                    Some(&problem.message),
                ),
            }
        }
        files => {
            let problem = files
                .into_iter()
                .find_map(Result::err)
                .expect("at least one failed bounded file read");
            report(
                problem.context.outcome,
                "admission",
                None,
                None,
                None,
                None,
                None,
                None,
                0,
                0,
                Some(&problem.message),
            )
        }
    };
    if let Err(error) = super::output::print(&result) {
        eprintln!("noble: {}", error.message);
        return std::process::ExitCode::from(error.exit());
    }
    std::process::ExitCode::from(result.exit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_diagnostics_bound_serialized_bytes_at_utf8_and_escape_boundaries() {
        let ascii = "x".repeat(511);
        let at_limit = bounded_diagnostic(&ascii);
        assert_eq!(at_limit.len(), 510);
        assert_eq!(serde_json::to_string(at_limit).unwrap().len(), 512);

        let control = "\u{001f}".repeat(86);
        let at_limit = bounded_diagnostic(&control);
        assert_eq!(at_limit.chars().count(), 85);
        assert_eq!(serde_json::to_string(at_limit).unwrap().len(), 512);

        let escaped = format!("{}\"tail", "x".repeat(508));
        let at_limit = bounded_diagnostic(&escaped);
        assert!(at_limit.ends_with('"'));
        assert_eq!(serde_json::to_string(at_limit).unwrap().len(), 512);

        let unicode = format!("{}😀tail", "x".repeat(506));
        let at_limit = bounded_diagnostic(&unicode);
        assert!(at_limit.ends_with('😀'));
        assert_eq!(serde_json::to_string(at_limit).unwrap().len(), 512);
    }

    const SCHEMA: &str = "module WorkerData@1 [ \
        opaque AttemptId Text public \
        opaque State Pair<I64,Sum<Unit,AttemptId>> public \
        variant Event Submit Pair<AttemptId,Text> public Cancelled AttemptId public \
        variant Action Send Pair<AttemptId,Text> public Noop Unit public \
        export AttemptId export AttemptId.new export AttemptId.into \
        export State export State.new export State.into \
        export Event export Event.Submit export Event.Cancelled \
        export Action export Action.Send export Action.Noop ]";
    const SOURCE: &str = "module worker@1 [ export transition \
        def transition [ drop unit WorkerData@1.Action.Noop nil cons ] ]";
    const COLLISION_SOURCE: &str = "module worker@1 [ \
        def earlier [ drop unit WorkerData@1.Action.Noop nil cons ] \
        export transition def transition [ drop unit WorkerData@1.Action.Noop nil cons ] ]";

    struct Fixture {
        manifest: Value,
        state: Vec<u8>,
        event: Vec<u8>,
    }

    const WORKER_SCHEMA: &str = include_str!("../../tests/fixtures/worker-data.noble");
    const WORKER_TRANSITION: &str = include_str!("../../tests/fixtures/worker-transition.noble");

    /// A package from the production encoder over a checked root owner and
    /// closure. The transition-interface check is deliberately skipped so a
    /// hostile control can carry a checked owner whose interface is wrong.
    fn fixture_with_source(
        schema_source: &str,
        source: &str,
        attempt_value: Value,
    ) -> Result<Fixture, String> {
        let session = noble_contracts::source::ModuleSession::new(
            &[],
            noble_contracts::source::RetainedSourceLimit::new(RETAINED_SCHEMA_ROOT_BYTES)
                .expect("combined Worker schema and root source limit"),
        )
        .map_err(|error| format!("{error:?}"))?;
        let checked =
            schema_module(&session, schema_source.as_bytes()).map_err(|error| error.message)?;
        let (catalog, types) = schema_catalog(&checked).map_err(|error| error.message)?;
        let (session, linked) = session.commit(checked);
        linked.map_err(|error| format!("{error:?}"))?;
        let declared = root_module(&session, source.as_bytes()).map_err(|error| error.message)?;
        let owner = root_owner(&declared).map_err(|error| error.message)?;
        let closure = root_closure(&declared).map_err(|error| error.message)?;
        let manifest = package_manifest(
            source,
            schema_source.as_bytes(),
            &catalog,
            &types,
            &owner,
            &closure,
        )
        .map_err(|error| error.message)?;
        let [state_schema, event_schema, _] =
            interface_rows(&catalog, &types).map_err(|error| error.message)?;
        let attempt_schema = catalog
            .iter()
            .find(|row| row.ty == types[3])
            .ok_or("missing AttemptId schema")?;
        let state = json!({"format":"noble-worker-value/v1","expected_schema":state_schema.wire(),
            "value":{"tag":"opaque","schema":state_schema.wire(),
                "value":{"tag":"pair","left":{"tag":"i64","value":"0"},
                    "right":{"tag":"sum","arm":"left","value":{"tag":"unit","value":null}}}}});
        let event = json!({"format":"noble-worker-value/v1","expected_schema":event_schema.wire(),
            "value":{"tag":"variant","schema":event_schema.wire(),"arm":"left",
                "value":{"tag":"pair","left":{"tag":"opaque","schema":attempt_schema.wire(),
                    "value":attempt_value},
                    "right":{"tag":"text","value":"ping"}}}});
        Ok(Fixture {
            manifest,
            state: serde_json::to_vec(&state).map_err(|error| error.to_string())?,
            event: serde_json::to_vec(&event).map_err(|error| error.to_string())?,
        })
    }

    fn fixture_with_schema(schema_source: &str) -> Result<Fixture, String> {
        fixture_with_source(
            schema_source,
            SOURCE,
            json!({"tag":"text","value":"attempt-1"}),
        )
    }

    fn fixture() -> Result<Fixture, String> {
        fixture_with_schema(SCHEMA)
    }

    fn invoke(
        fixture: &Fixture,
        schema: &[u8],
        state: &[u8],
        event: &[u8],
    ) -> Result<Value, String> {
        let package = serde_json::to_vec(&fixture.manifest).map_err(|error| error.to_string())?;
        let report = execute(&package, schema, state, event)
            .map_err(|error| format!("{}: {}", error.context.outcome, error.message))?;
        serde_json::from_str(&report.json).map_err(|error| error.to_string())
    }

    #[test]
    fn typed_host_retains_distinct_bounded_schema_and_root_above_single_source_limit(
    ) -> Result<(), String> {
        let schema = SCHEMA.replacen(
            "opaque AttemptId Text public",
            &format!("opaque AttemptId Text public # {}\n", "s".repeat(38_000)),
            1,
        );
        let source = SOURCE.replacen(
            "export transition",
            &format!("# {}\n export transition", "r".repeat(30_000)),
            1,
        );
        let per_source = usize::try_from(LIMITS.bytes).map_err(|error| error.to_string())?;
        if schema.len() > per_source
            || source.len() > per_source
            || schema.len() + source.len() <= per_source
        {
            return Err("Worker schema/root boundary fixture is not independent".into());
        }
        let fixture =
            fixture_with_source(&schema, &source, json!({"tag":"text","value":"attempt-1"}))?;
        let result = invoke(&fixture, schema.as_bytes(), &fixture.state, &fixture.event)?;
        if result["outcome"] != "normal"
            || result["typed_output"]["actions"]["items"][0]["arm"] != "right"
        {
            return Err(format!(
                "separately bounded Worker sources were refused: {result}"
            ));
        }
        Ok(())
    }

    #[test]
    fn typed_host_ingress_executes_checked_nominal_transition_and_refuses_hostile_claims(
    ) -> Result<(), String> {
        let fixture = fixture()?;
        let private_schema = SCHEMA
            .replace(
                "opaque AttemptId Text public",
                "opaque AttemptId Text private",
            )
            .replace("export AttemptId.new export AttemptId.into ", "");
        let private = fixture_with_schema(&private_schema)?;
        if let Some(directory) = std::env::var_os("NOBLE_WORKER_FIXTURE_DIR") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            let package =
                serde_json::to_vec(&fixture.manifest).map_err(|error| error.to_string())?;
            for (name, bytes) in [
                ("package.json", package.as_slice()),
                ("schema.noble", SCHEMA.as_bytes()),
                ("state.json", fixture.state.as_slice()),
                ("event.json", fixture.event.as_slice()),
            ] {
                std::fs::write(directory.join(name), bytes).map_err(|error| error.to_string())?;
            }
            let mut spoof: Value =
                serde_json::from_slice(&fixture.event).map_err(|error| error.to_string())?;
            spoof["value"]["value"]["left"]["schema"]["version"] = json!(2);
            let mut forged = fixture.manifest.clone();
            forged["root"]["definition_identity"] = json!("0");
            let mut semantic = fixture.manifest.clone();
            semantic["root"]["definition_identity"] = json!("0");
            semantic["definitions"][0]["definition_identity"] = json!("0");
            let mut wrong_owner = fixture.manifest.clone();
            wrong_owner["root"]["module_id"] = json!("19");
            wrong_owner["definitions"][0]["module_id"] = json!("19");
            let mut wrong_slot = fixture.manifest.clone();
            wrong_slot["root"]["definition_ordinal"] = json!(1);
            wrong_slot["definitions"][0]["definition_ordinal"] = json!(1);
            let mut duplicate = fixture.manifest.clone();
            let mut second = duplicate["definitions"][0].clone();
            let other_source = "module other@1 [ def transition [ 0 ] ]";
            second["source_utf8"] = json!(other_source);
            second["source_sha256"] =
                json!(crate::workflow::intrinsic::sha256(other_source.as_bytes()));
            duplicate["definitions"]
                .as_array_mut()
                .ok_or("missing definition rows")?
                .push(second);
            let mut version = fixture.manifest.clone();
            version["root"]["version"] = json!(2);
            version["definitions"][0]["version"] = json!(2);
            let mut nested = fixture.manifest.clone();
            nested["artifact"] = json!({"package":fixture.manifest.clone()});
            let mut sorted = fixture.manifest.clone();
            sorted["schemas"]
                .as_array_mut()
                .ok_or("missing schema rows")?
                .swap(0, 1);
            let mut schema_duplicate = fixture.manifest.clone();
            let first_schema = schema_duplicate["schemas"][0].clone();
            schema_duplicate["schemas"][1] = first_schema;
            let mut constructor = fixture.manifest.clone();
            constructor["schemas"][0]["constructors"] = json!({"opaque":true});
            let mut capture = fixture.manifest.clone();
            capture["captures"] = json!([{"tag":"resource","value":"forged"}]);
            let mut literal = fixture.manifest.clone();
            literal["root"]["worker_literal_end"] = json!(262_144);
            let mut crowded_state: Value =
                serde_json::from_slice(&fixture.state).map_err(|error| error.to_string())?;
            let mut crowded_event: Value =
                serde_json::from_slice(&fixture.event).map_err(|error| error.to_string())?;
            let attempt_schema = crowded_event["value"]["value"]["left"]["schema"].clone();
            crowded_state["value"]["value"]["right"] = json!({"tag":"sum","arm":"right",
                "value":{"tag":"opaque","schema":attempt_schema,
                    "value":{"tag":"text","value":"x".repeat(40_000)}}});
            crowded_event["value"]["value"]["left"]["value"]["value"] = json!("x".repeat(40_000));
            for (name, value) in [
                ("event-spoof.json", spoof),
                ("package-forged.json", forged),
                ("package-semantic.json", semantic),
                ("package-owner.json", wrong_owner),
                ("package-slot.json", wrong_slot),
                ("package-duplicate.json", duplicate),
                ("package-version.json", version),
                ("package-nested.json", nested),
                ("package-literal-forged.json", literal),
                ("package-schema-unsorted.json", sorted),
                ("package-schema-duplicate.json", schema_duplicate),
                ("package-constructor-forged.json", constructor),
                ("package-capture.json", capture),
                ("state-overbudget.json", crowded_state),
                ("event-overbudget.json", crowded_event),
            ] {
                std::fs::write(
                    directory.join(name),
                    serde_json::to_vec(&value).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
            }
            let mut collision = fixture.manifest.clone();
            collision["root"]["source_utf8"] = json!(COLLISION_SOURCE);
            collision["definitions"][0]["source_utf8"] = json!(COLLISION_SOURCE);
            collision["definitions"][0]["source_sha256"] = json!(
                crate::workflow::intrinsic::sha256(COLLISION_SOURCE.as_bytes())
            );
            std::fs::write(
                directory.join("package-root-collision.json"),
                serde_json::to_vec(&collision).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            let private_package =
                serde_json::to_vec(&private.manifest).map_err(|error| error.to_string())?;
            for (name, bytes) in [
                ("schema-private.noble", private_schema.as_bytes()),
                ("package-private.json", private_package.as_slice()),
            ] {
                std::fs::write(directory.join(name), bytes).map_err(|error| error.to_string())?;
            }
        }
        let normal = invoke(&fixture, SCHEMA.as_bytes(), &fixture.state, &fixture.event)?;
        if normal["outcome"] != "normal"
            || normal["typed_output"]["state"]["value"]["left"]["value"] != "0"
            || normal["typed_output"]["actions"]["items"][0]["arm"] != "right"
            || normal["guest_requests"] != 0
            || normal["protected_operations"] != 0
            || normal["module"]["worker_literal_end"].as_u64().is_none()
        {
            return Err(format!("typed guest result mismatch: {normal}"));
        }
        let private_package = Package::parse(
            &serde_json::to_vec(&private.manifest).map_err(|error| error.to_string())?,
        )
        .map_err(|error| format!("{error:?}"))?;
        let session = noble_contracts::source::ModuleSession::new(
            &[],
            noble_contracts::source::RetainedSourceLimit::new(LIMITS.bytes)
                .expect("Worker source limit"),
        )
        .map_err(|error| format!("{error:?}"))?;
        let selected = session
            .prepare(private_schema.as_bytes(), &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        let (catalog, types) =
            selected_schema(&selected, &private_package, private_schema.as_bytes())
                .map_err(|error| error.message)?;
        if !matches!(
            worker_value::admit(
                [&private.state, &private.event],
                [&types[0], &types[1]],
                &catalog,
                private_package.memory,
                private_package.remaining_work
            ),
            Err(Refusal::Invalid)
        ) {
            return Err("private AttemptId constructor passed checked typed ingress".into());
        }
        let mut event: Value =
            serde_json::from_slice(&fixture.event).map_err(|error| error.to_string())?;
        event["value"]["value"]["left"]["schema"]["version"] = json!(2);
        let spoof = serde_json::to_vec(&event).map_err(|error| error.to_string())?;
        let package = serde_json::to_vec(&fixture.manifest).map_err(|error| error.to_string())?;
        let rejection = match execute(&package, SCHEMA.as_bytes(), &fixture.state, &spoof) {
            Err(problem) => problem,
            Ok(_) => return Err("spoofed nested nominal version was admitted".into()),
        };
        if rejection.context.outcome != "invalid-input" {
            return Err("wrong spoof refusal".into());
        }

        let mut bad = fixture.manifest.clone();
        bad["root"]["definition_identity"] = json!("0");
        let forged = serde_json::to_vec(&bad).map_err(|error| error.to_string())?;
        if execute(&forged, SCHEMA.as_bytes(), &fixture.state, &fixture.event).is_ok() {
            return Err("forged Program owner was admitted".into());
        }

        let mut cyclic = fixture.manifest.clone();
        let source = "module worker@1 [ export transition def transition [ helper ] def helper [ transition ] ]";
        cyclic["root"]["source_utf8"] = json!(source);
        cyclic["definitions"][0]["source_utf8"] = json!(source);
        cyclic["definitions"][0]["source_sha256"] =
            json!(crate::workflow::intrinsic::sha256(source.as_bytes()));
        let cyclic = serde_json::to_vec(&cyclic).map_err(|error| error.to_string())?;
        if execute(&cyclic, SCHEMA.as_bytes(), &fixture.state, &fixture.event).is_ok() {
            return Err("cyclic Program closure was admitted".into());
        }

        let session = noble_contracts::source::ModuleSession::new(
            &[],
            noble_contracts::source::RetainedSourceLimit::new(LIMITS.bytes)
                .expect("Worker source limit"),
        )
        .map_err(|error| format!("{error:?}"))?;
        let selected = session
            .prepare(SCHEMA.as_bytes(), &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        let (session, linked) = session.commit(selected);
        linked.map_err(|error| format!("{error:?}"))?;
        let same_body = session
            .prepare(COLLISION_SOURCE.as_bytes(), &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        if same_body
            .checked_program_owner("transition")
            .is_none_or(|owner| owner.definition_slot() != 1)
            || same_body.checked_program_closure("transition").is_ok()
        {
            return Err("equal-body Named slots were collapsed or unused decoy admitted".into());
        }
        let mut collision = fixture.manifest.clone();
        collision["root"]["source_utf8"] = json!(COLLISION_SOURCE);
        collision["definitions"][0]["source_utf8"] = json!(COLLISION_SOURCE);
        collision["definitions"][0]["source_sha256"] = json!(crate::workflow::intrinsic::sha256(
            COLLISION_SOURCE.as_bytes()
        ));
        let collision = serde_json::to_vec(&collision).map_err(|error| error.to_string())?;
        if execute(
            &collision,
            SCHEMA.as_bytes(),
            &fixture.state,
            &fixture.event,
        )
        .is_ok()
        {
            return Err("equal-body unused decoy was admitted as root".into());
        }
        Ok(())
    }

    fn worker_state(
        fixture: &Fixture,
        count: i64,
        pending: Option<i64>,
    ) -> Result<Vec<u8>, String> {
        let mut frame: Value =
            serde_json::from_slice(&fixture.state).map_err(|error| error.to_string())?;
        frame["value"]["value"]["left"]["value"] = json!(count.to_string());
        if let Some(attempt) = pending {
            let event: Value =
                serde_json::from_slice(&fixture.event).map_err(|error| error.to_string())?;
            let schema = &event["value"]["value"]["left"]["schema"];
            frame["value"]["value"]["right"] = json!({"tag":"sum","arm":"right",
                "value":{"tag":"opaque","schema":schema,
                    "value":{"tag":"i64","value":attempt.to_string()}}});
        }
        serde_json::to_vec(&frame).map_err(|error| error.to_string())
    }

    fn worker_event(
        fixture: &Fixture,
        attempt: i64,
        submit: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let mut frame: Value =
            serde_json::from_slice(&fixture.event).map_err(|error| error.to_string())?;
        frame["value"]["value"]["left"]["value"]["value"] = json!(attempt.to_string());
        if let Some(text) = submit {
            frame["value"]["value"]["right"]["value"] = json!(text);
        } else {
            let id = frame["value"]["value"]["left"].clone();
            frame["value"]["arm"] = json!("right");
            frame["value"]["value"] = id;
        }
        serde_json::to_vec(&frame).map_err(|error| error.to_string())
    }

    fn worker_output(
        fixture: &Fixture,
        count: i64,
        pending: Option<i64>,
        event_id: i64,
        submit: Option<&str>,
    ) -> Result<Value, String> {
        let state = worker_state(fixture, count, pending)?;
        let event = worker_event(fixture, event_id, submit)?;
        let report = invoke(fixture, WORKER_SCHEMA.as_bytes(), &state, &event)?;
        if report["outcome"] != "normal"
            || report["stage"] != "execution"
            || report["guest_requests"] != 0
            || report["protected_operations"] != 0
            || report["checked_program_owner"].is_null()
            || report["module"]["wasm_sha256"].as_str().is_none()
        {
            return Err(format!("compiled worker transition failed: {report}"));
        }
        Ok(report["typed_output"].clone())
    }

    #[test]
    fn checked_guest_executes_exact_worker_transition_table() -> Result<(), String> {
        let fixture = fixture_with_source(
            WORKER_SCHEMA,
            WORKER_TRANSITION,
            json!({"tag":"i64","value":"1"}),
        )?;
        if let Some(directory) = std::env::var_os("NOBLE_WORKER_SHELL_FIXTURE_DIR") {
            use std::os::unix::fs::DirBuilderExt;
            let directory = std::path::PathBuf::from(directory);
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&directory)
                .map_err(|error| error.to_string())?;
            let package =
                serde_json::to_vec(&fixture.manifest).map_err(|error| error.to_string())?;
            let state = worker_state(&fixture, 0, None)?;
            let submit = worker_event(&fixture, 1, Some("ping"))?;
            let cancel = worker_event(&fixture, 1, None)?;
            let grant = serde_json::to_vec(&json!({
                "format":"noble-worker-shell-grant/v1",
                "operation":"noble-test:worker-shell/send@1.0.0#send",
                "attempt":"1","text":"ping","allow":true,
            }))
            .map_err(|error| error.to_string())?;
            let mut forged = fixture.manifest.clone();
            forged["root"]["definition_identity"] = json!("0");
            let forged = serde_json::to_vec(&forged).map_err(|error| error.to_string())?;
            for (name, bytes) in [
                ("package.json", package.as_slice()),
                ("schema.noble", WORKER_SCHEMA.as_bytes()),
                ("state.json", state.as_slice()),
                ("submit.json", submit.as_slice()),
                ("cancel.json", cancel.as_slice()),
                ("grant.json", grant.as_slice()),
                ("forged-package.json", forged.as_slice()),
            ] {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(directory.join(name))
                    .map_err(|error| error.to_string())?;
                std::io::Write::write_all(&mut file, bytes).map_err(|error| error.to_string())?;
            }
        }
        let submit = worker_output(&fixture, 0, None, 1, Some("ping"))?;
        assert_eq!(submit["state"]["value"]["left"]["value"], "1");
        assert_eq!(submit["state"]["value"]["right"]["arm"], "right");
        assert_eq!(
            submit["state"]["value"]["right"]["value"]["value"]["value"],
            "1"
        );
        assert_eq!(submit["actions"]["items"].as_array().map(Vec::len), Some(1));
        assert_eq!(submit["actions"]["items"][0]["arm"], "left");
        assert_eq!(
            submit["actions"]["items"][0]["value"]["left"]["value"]["value"],
            "1"
        );
        assert_eq!(
            submit["actions"]["items"][0]["value"]["right"]["value"],
            "ping"
        );

        let ignored = worker_output(&fixture, 1, Some(1), 2, Some("ignored"))?;
        assert_eq!(ignored["state"]["value"]["left"]["value"], "1");
        assert_eq!(
            ignored["state"]["value"]["right"]["value"]["value"]["value"],
            "1"
        );
        assert_eq!(
            ignored["actions"]["items"].as_array().map(Vec::len),
            Some(0)
        );

        let other_cancel = worker_output(&fixture, 1, Some(1), 2, None)?;
        assert_eq!(other_cancel["state"]["value"]["left"]["value"], "1");
        assert_eq!(other_cancel["state"]["value"]["right"]["arm"], "right");
        assert_eq!(
            other_cancel["state"]["value"]["right"]["value"]["value"]["value"],
            "1"
        );
        assert_eq!(
            other_cancel["actions"]["items"].as_array().map(Vec::len),
            Some(0)
        );

        let matching_cancel = worker_output(&fixture, 1, Some(1), 1, None)?;
        assert_eq!(matching_cancel["state"]["value"]["left"]["value"], "1");
        assert_eq!(matching_cancel["state"]["value"]["right"]["arm"], "left");
        assert_eq!(
            matching_cancel["actions"]["items"].as_array().map(Vec::len),
            Some(0)
        );

        let no_pending = worker_output(&fixture, 1, None, 1, None)?;
        assert_eq!(no_pending["state"]["value"]["left"]["value"], "1");
        assert_eq!(no_pending["state"]["value"]["right"]["arm"], "left");
        assert_eq!(
            no_pending["actions"]["items"].as_array().map(Vec::len),
            Some(0)
        );

        let wrapped = worker_output(&fixture, i64::MAX, None, 1, Some("ping"))?;
        assert_eq!(
            wrapped["state"]["value"]["left"]["value"],
            i64::MIN.to_string()
        );
        assert_eq!(wrapped["state"]["value"]["right"]["arm"], "right");
        assert_eq!(
            wrapped["actions"]["items"].as_array().map(Vec::len),
            Some(1)
        );
        Ok(())
    }

    #[test]
    fn compiled_worker_actions_require_independent_grant_and_retire_late_native_result(
    ) -> Result<(), String> {
        let fixture = fixture_with_source(
            WORKER_SCHEMA,
            WORKER_TRANSITION,
            json!({"tag":"i64","value":"1"}),
        )?;
        let package = serde_json::to_vec(&fixture.manifest).map_err(|error| error.to_string())?;
        let state = worker_state(&fixture, 0, None)?;
        let submit = worker_event(&fixture, 1, Some("ping"))?;
        let cancel = worker_event(&fixture, 1, None)?;
        let grant = |text: &str, allow: bool| {
            serde_json::to_vec(&json!({
                "format":"noble-worker-shell-grant/v1",
                "operation":"noble-test:worker-shell/send@1.0.0#send",
                "attempt":"1","text":text,"allow":allow,
            }))
            .map_err(|error| error.to_string())
        };
        let approved = super::super::worker_shell::execute(
            &package,
            WORKER_SCHEMA.as_bytes(),
            &state,
            &submit,
            &cancel,
            &grant("ping", true)?,
        )
        .map_err(|error| error.message)?;
        let approved: Value =
            serde_json::from_str(&approved.json).map_err(|error| error.to_string())?;
        assert_eq!(approved["outcome"], "normal");
        assert_eq!(approved["stage"], "retirement");
        assert_eq!(
            approved["first_guest"]["typed_output"]["actions"]["items"][0]["arm"],
            "left"
        );
        assert_eq!(
            approved["cancel_guest"]["typed_output"]["state"]["value"]["right"]["arm"],
            "left"
        );
        assert_eq!(approved["native_send_bytes"], 12);
        assert_eq!(approved["protected_operation_attempts"], 1);
        assert_eq!(approved["successful_result_deliveries"], 0);
        assert_eq!(approved["task_state"], "Retired");
        assert_eq!(approved["outstanding_native_pins"], 0);
        assert_eq!(approved["resource_state"], "Retired");
        assert_eq!(approved["resource_native_pins"], 0);
        assert_eq!(approved["external_send_outcome"], "not-attempted");
        assert_eq!(approved["external_rollback_claimed"], false);

        let wrong_attempt = serde_json::to_vec(&json!({
            "format":"noble-worker-shell-grant/v1",
            "operation":"noble-test:worker-shell/send@1.0.0#send",
            "attempt":"2","text":"ping","allow":true,
        }))
        .map_err(|error| error.to_string())?;
        for (label, grant_bytes) in [
            ("mismatched exact Text", grant("changed", true)?),
            ("mismatched attempt owner", wrong_attempt),
            ("explicitly denied grant", grant("ping", false)?),
        ] {
            let refused = super::super::worker_shell::execute(
                &package,
                WORKER_SCHEMA.as_bytes(),
                &state,
                &submit,
                &cancel,
                &grant_bytes,
            )
            .map_err(|error| error.message)?;
            let refusal: Value =
                serde_json::from_str(&refused.json).map_err(|error| error.to_string())?;
            assert_eq!(refusal["outcome"], "denied", "{label}");
            assert_eq!(refusal["stage"], "authorization", "{label}");
            assert_eq!(refusal["protected_operation_attempts"], 0, "{label}");
            assert!(refusal["native_send_bytes"].is_null(), "{label}");
            assert_eq!(refusal["resource_state"], "Retired", "{label}");
            assert_eq!(refusal["resource_native_pins"], 0, "{label}");
        }

        let wrong_cancel = worker_event(&fixture, 2, None)?;
        let refusal = super::super::worker_shell::execute(
            &package,
            WORKER_SCHEMA.as_bytes(),
            &state,
            &submit,
            &wrong_cancel,
            &grant("ping", true)?,
        )
        .err()
        .ok_or("mismatched cancellation passed shell preflight")?;
        assert_eq!(refusal.context.outcome, "invalid-input");

        let mut forged = fixture.manifest.clone();
        forged["root"]["definition_identity"] = json!("0");
        let forged = serde_json::to_vec(&forged).map_err(|error| error.to_string())?;
        let refusal = super::super::worker_shell::execute(
            &forged,
            WORKER_SCHEMA.as_bytes(),
            &state,
            &submit,
            &cancel,
            &grant("ping", true)?,
        )
        .err()
        .ok_or("forged Program owner passed before shell operation")?;
        assert_eq!(refusal.context.outcome, "invalid-input");

        let noop = self::fixture()?;
        let noop_package = serde_json::to_vec(&noop.manifest).map_err(|error| error.to_string())?;
        let refusal = super::super::worker_shell::execute(
            &noop_package,
            SCHEMA.as_bytes(),
            &noop.state,
            &noop.event,
            &cancel,
            &grant("ping", true)?,
        )
        .err()
        .ok_or("non-Send action crossed protected boundary")?;
        assert_eq!(refusal.context.outcome, "invalid-input");
        Ok(())
    }

    /// WORKER-02 rejection variants, each run through the production admission
    /// path. An Err from `execute` is an admission-stage refusal before any
    /// worker engine starts, which `run` reports with zero guest requests and
    /// zero protected operations.
    #[test]
    fn worker_interface_checker_classifies_each_worker02_variant() -> Result<(), String> {
        let fixture = fixture()?;
        let refused = |manifest: &Value, schema: &str| -> Result<String, String> {
            let package = serde_json::to_vec(manifest).map_err(|error| error.to_string())?;
            match execute(&package, schema.as_bytes(), &fixture.state, &fixture.event) {
                Ok(report) => Err(format!("variant was admitted: {}", report.json)),
                Err(problem) => Ok(problem.context.outcome.to_owned()),
            }
        };
        let state = fixture.manifest["interface"]["inputs"][0].clone();
        let event = fixture.manifest["interface"]["inputs"][1].clone();
        let actions = fixture.manifest["interface"]["outputs"][1].clone();
        let action = actions["list"].clone();

        // The unmodified claim and program are admitted and executed.
        let accepted = serde_json::to_vec(&fixture.manifest).map_err(|error| error.to_string())?;
        let normal = execute(&accepted, SCHEMA.as_bytes(), &fixture.state, &fixture.event)
            .map_err(|problem| problem.message)?;
        if normal.outcome != "normal" {
            return Err(format!("baseline worker was refused: {}", normal.json));
        }

        let mut reversed = fixture.manifest.clone();
        reversed["interface"]["inputs"] = json!([event, state]);
        let mut extra = fixture.manifest.clone();
        extra["interface"]["outputs"] = json!([state, actions, state]);
        let mut foreign_identity = fixture.manifest.clone();
        let mut same_layout = state.clone();
        let module_id = same_layout["module_id"]
            .as_str()
            .ok_or("schema wire lacks module identity")?
            .parse::<u64>()
            .map_err(|error| error.to_string())?;
        same_layout["module_id"] = json!(module_id.wrapping_add(1).to_string());
        foreign_identity["interface"]["inputs"][0] = same_layout;
        let mut effectful = fixture.manifest.clone();
        effectful["interface"]["effects"] = json!(["test.emit"]);
        let mut capture = fixture.manifest.clone();
        capture["captures"] = json!([{"tag":"opaque","schema":state,
            "value":{"tag":"pair","left":{"tag":"i64","value":"0"},
                "right":{"tag":"resource","value":"hidden"}}}]);
        let mut heterogeneous = fixture.manifest.clone();
        heterogeneous["interface"]["outputs"][1] = json!({"list":[action, event]});

        for (name, manifest, expected) in [
            ("reversed-state-event-stack", &reversed, "interface-reject"),
            ("extra-output-value", &extra, "interface-reject"),
            (
                "same-layout-different-schema-identity",
                &foreign_identity,
                "schema-reject",
            ),
            (
                "effect-outside-dispatch-ceiling",
                &effectful,
                "effect-reject",
            ),
            (
                "resource-hidden-in-worker-capture",
                &capture,
                "eligibility-reject",
            ),
            (
                "heterogeneous-list-without-adapter",
                &heterogeneous,
                "interface-reject",
            ),
        ] {
            let outcome = refused(manifest, SCHEMA)?;
            if outcome != expected {
                return Err(format!("{name}: expected {expected}, got {outcome}"));
            }
        }

        // The checked Program itself is held to the same interface: a correct
        // claim cannot admit a transition whose instantiated stack is reversed
        // or carries an extra output.
        for (name, source) in [
            (
                "program-reversed-state-event-stack",
                "module worker@1 [ export transition \
                 def transition [ swap drop unit WorkerData@1.Action.Noop nil cons ] ]",
            ),
            (
                "program-extra-output-value",
                "module worker@1 [ export transition \
                 def transition [ drop unit WorkerData@1.Action.Noop nil cons 0 ] ]",
            ),
        ] {
            let variant =
                fixture_with_source(SCHEMA, source, json!({"tag":"text","value":"attempt-1"}))?;
            let outcome = refused(&variant.manifest, SCHEMA)?;
            if outcome != "interface-reject" {
                return Err(format!("{name}: expected interface-reject, got {outcome}"));
            }
        }
        Ok(())
    }

    /// The accepted WORKER-02 baseline package (sha256 of
    /// `verification/worker02/fixtures/package.json`).
    const ACCEPTED_BASELINE_PACKAGE_SHA256: &str =
        "8a1bb3c6ac496a19246663cfd42211be6aa45d52deafc7284fa318af32d62681";

    #[test]
    fn preparation_service_reproduces_the_accepted_baseline_that_admission_runs(
    ) -> Result<(), String> {
        let mut trace = PreparationTrace::default();
        let package = prepare(SOURCE.as_bytes(), SCHEMA.as_bytes(), &mut trace)
            .map_err(|problem| problem.message)?;
        assert_eq!(
            crate::workflow::intrinsic::sha256(&package),
            ACCEPTED_BASELINE_PACKAGE_SHA256
        );
        assert_eq!(
            (
                trace.source_checks,
                trace.closure_walks,
                trace.backend_compilations
            ),
            (3, 1, 1)
        );
        let repeated = prepare(
            SOURCE.as_bytes(),
            SCHEMA.as_bytes(),
            &mut PreparationTrace::default(),
        )
        .map_err(|problem| problem.message)?;
        assert_eq!(repeated, package, "preparation is deterministic");
        // Evaluation re-admits the untrusted package and runs exactly the
        // module the service compiled.
        let frames = fixture()?;
        let report = execute(&package, SCHEMA.as_bytes(), &frames.state, &frames.event)
            .map_err(|problem| problem.message)?;
        let report: Value =
            serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
        assert_eq!(report["outcome"], "normal");
        assert_eq!(report["guest_requests"], 0);
        assert_eq!(report["module"]["wat_sha256"], json!(trace.wat_sha256));

        // The multi-definition transition table: three closure rows, one
        // compilation, and admission runs the prepared package.
        let mut trace = PreparationTrace::default();
        let table = prepare(
            WORKER_TRANSITION.as_bytes(),
            WORKER_SCHEMA.as_bytes(),
            &mut trace,
        )
        .map_err(|problem| problem.message)?;
        let document: Value = serde_json::from_slice(&table).map_err(|error| error.to_string())?;
        assert_eq!(document["definitions"].as_array().map(Vec::len), Some(3));
        let frames = fixture_with_source(
            WORKER_SCHEMA,
            WORKER_TRANSITION,
            json!({"tag":"i64","value":"1"}),
        )?;
        let state = worker_state(&frames, 0, None)?;
        let event = worker_event(&frames, 1, Some("ping"))?;
        let report = execute(&table, WORKER_SCHEMA.as_bytes(), &state, &event)
            .map_err(|problem| problem.message)?;
        let report: Value =
            serde_json::from_str(&report.json).map_err(|error| error.to_string())?;
        assert_eq!(report["outcome"], "normal");
        assert_eq!(report["typed_output"]["actions"]["items"][0]["arm"], "left");
        assert_eq!(report["module"]["wat_sha256"], json!(trace.wat_sha256));
        Ok(())
    }

    /// A refused candidate returns no package, and its trace shows the stage
    /// that refused it and no work of any later stage.
    #[test]
    fn preparation_refusals_stop_at_their_stage_without_a_package() -> Result<(), String> {
        let other_schema = SCHEMA.replacen("module WorkerData@1", "module OtherData@1", 1);
        let reversed = "module worker@1 [ export transition \
            def transition [ swap drop unit WorkerData@1.Action.Noop nil cons ] ]";
        let extra = "module worker@1 [ export transition \
            def transition [ drop unit WorkerData@1.Action.Noop nil cons 0 ] ]";
        let nominal = "module worker@1 [ opaque Hidden I64 public export transition \
            def transition [ drop unit WorkerData@1.Action.Noop nil cons ] ]";
        let unexported =
            "module worker@1 [ def transition [ drop unit WorkerData@1.Action.Noop nil cons ] ]";
        // Name, candidate source, host schema, refusing stage, outcome, and
        // the checker stages, closure walks and compilations entered.
        type Case<'case> = (
            &'case str,
            &'case [u8],
            &'case str,
            &'case str,
            &'case str,
            (u32, u32, u32),
        );
        let cases: [Case<'_>; 9] = [
            (
                "non-UTF-8 source",
                b"module worker@1 [ \xff ]",
                SCHEMA,
                "inputs",
                "invalid-input",
                (0, 0, 0),
            ),
            (
                "schema outside WorkerData@1",
                SOURCE.as_bytes(),
                &other_schema,
                "schema-check",
                "unsupported",
                (1, 0, 0),
            ),
            (
                "unterminated root",
                b"module worker@1 [ export transition def transition [ drop",
                SCHEMA,
                "root-check",
                "reject",
                (2, 0, 0),
            ),
            (
                "unbound word",
                b"module worker@1 [ export transition def transition [ frobnicate ] ]",
                SCHEMA,
                "root-check",
                "unbound-word",
                (2, 0, 0),
            ),
            (
                "root declares a nominal type",
                nominal.as_bytes(),
                SCHEMA,
                "root-check",
                "unsupported",
                (2, 0, 0),
            ),
            (
                "no exported transition",
                unexported.as_bytes(),
                SCHEMA,
                "root-check",
                "invalid-input",
                (2, 0, 0),
            ),
            (
                "unused equal-body decoy",
                COLLISION_SOURCE.as_bytes(),
                SCHEMA,
                "root-check",
                "invalid-input",
                (2, 1, 0),
            ),
            (
                "reversed State Event stack",
                reversed.as_bytes(),
                SCHEMA,
                "transition-check",
                "interface-reject",
                (3, 1, 0),
            ),
            (
                "extra output value",
                extra.as_bytes(),
                SCHEMA,
                "transition-check",
                "interface-reject",
                (3, 1, 0),
            ),
        ];
        for (name, source, schema, stage, outcome, work) in cases {
            let mut trace = PreparationTrace::default();
            match prepare(source, schema.as_bytes(), &mut trace) {
                Ok(_) => return Err(format!("{name}: a package was produced")),
                Err(problem) => {
                    assert_eq!(problem.context.outcome, outcome, "{name}");
                    assert_eq!(trace.stage, stage, "{name}");
                    assert_eq!(
                        (
                            trace.source_checks,
                            trace.closure_walks,
                            trace.backend_compilations
                        ),
                        work,
                        "{name}"
                    );
                    assert!(trace.wat_sha256.is_none(), "{name}");
                }
            }
        }
        Ok(())
    }
}
