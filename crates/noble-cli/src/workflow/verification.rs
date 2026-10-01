mod compilation;
mod evidence;
pub(super) mod named_v2;

pub(crate) struct Acceptance {
    pub(crate) axioms: std::vec::Vec<std::string::String>,
    pub(crate) wire: std::string::String,
    pub(crate) tools: super::encoding::Json,
}

struct Workspace {
    temporary: super::artifacts::Temporary,
    sources: std::path::PathBuf,
    library: std::path::PathBuf,
    obligation: std::path::PathBuf,
    submission: std::path::PathBuf,
    producer: std::path::PathBuf,
    consumer: std::path::PathBuf,
    checker: std::path::PathBuf,
}

impl Workspace {
    fn create(library: &super::rules::Library, install_v1_replay: bool) -> Result<Self, super::output::Failure> {
        let temporary = attempt!(super::artifacts::Temporary::create());
        let workspace = Self {
            sources: temporary.path.join("sources"),
            library: temporary.path.join("library"),
            obligation: temporary.path.join("obligation"),
            submission: temporary.path.join("submission"),
            producer: temporary.path.join("producer"),
            consumer: temporary.path.join("consumer"),
            checker: temporary.path.join("checker"),
            temporary,
        };
        attempt!([
            &workspace.sources,
            &workspace.library,
            &workspace.obligation,
            &workspace.submission,
            &workspace.producer,
            &workspace.consumer,
            &workspace.checker,
        ]
        .iter()
        .try_for_each(|path| super::artifacts::create_directory(path)));
        attempt!(super::artifacts::write_source(
            &workspace.checker.join("NobleCompile.lean"),
            include_bytes!("../compiler.lean"),
        ));
        if install_v1_replay {
            // Concatenation preserves every byte of the trusted v1 replay.
            attempt!(super::artifacts::write_source(
                &workspace.checker.join("NobleConsumer.lean"),
                concat!(
                    include_str!("../consumer/decoding.lean"),
                    include_str!("../consumer/replay.lean"),
                )
                .as_bytes()
            ));
        }
        attempt!(library.modules.iter().try_for_each(|module| {
            super::artifacts::write_source(
                &workspace.sources.join(&module.relative),
                module.source.as_bytes(),
            )
        }));
        Ok(workspace)
    }
}

struct Session {
    sandbox: crate::sandbox::Environment,
    workspace: Workspace,
    deadline: std::time::Instant,
}

#[expect(
    tigerstyle::contradictory_time,
    reason = "Owner: noble-maintainers. timeout_ms is a duration budget, not an injected timestamp; this process boundary samples the monotonic clock once to establish the deadline shared by tool discovery and every worker."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; verify propagates checked deadline, sandbox discovery, compilation, export and replay failures through Failure. Evidence acceptance is a subprocess result, never an assertion precondition."
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-maintainers; the sole verify caller supplies generated obligation then submitted proof as Lean source strings, each staged in its dedicated workspace file. Preserve this private protocol order; reassess if additional callers require named source roles."
)]
pub(super) fn verify(
    obligation: &str,
    proof: &str,
    library: &super::rules::Library,
    is_refutation: bool,
    timeout_ms: u64,
) -> Result<Acceptance, super::output::Failure> {
    let deadline = attempt!(crate::sandbox::now()
        .checked_add(std::time::Duration::from_millis(timeout_ms))
        .ok_or_else(|| super::output::Failure::error(
            "timeout-overflow",
            "invalid consumer deadline".into()
        )));
    let sandbox = attempt!(crate::sandbox::Environment::discover(deadline));
    let probe = attempt!(probe_toolchain(&sandbox, deadline));
    let session = Session {
        sandbox,
        workspace: attempt!(Workspace::create(library, true)),
        deadline,
    };
    attempt!(session.compile_library(library));
    attempt!(session.compile_obligation(obligation));
    attempt!(session.compile_submission(proof));
    // The exporter may load untrusted producer objects. Only bounded textual
    // declarations, never those objects, cross to the fresh kernel consumer.
    let wire = attempt!(session.export_proof(is_refutation));
    let axioms = attempt!(session.replay(&wire, is_refutation));
    Ok(Acceptance {
        axioms,
        wire,
        tools: tools(&session.sandbox, &probe.stdout),
    })
}

/// Recompile the restricted closed Type0 witness in an isolated kernel process.
/// The final consumer receives only a bounded Lean term produced from the
/// source AST, never a producer object or any source-selected import/tactic.
pub(super) fn verify_intrinsic_pure(
    claim: &str,
    term: &str,
    library: &super::rules::Library,
    timeout_ms: u64,
) -> Result<Acceptance, super::output::Failure> {
    let deadline = attempt!(crate::sandbox::now()
        .checked_add(std::time::Duration::from_millis(timeout_ms))
        .ok_or_else(|| super::output::Failure::error("timeout-overflow","invalid proof deadline".into())));
    let sandbox = attempt!(crate::sandbox::Environment::discover(deadline));
    let probe = attempt!(probe_toolchain(&sandbox,deadline));
    let session = Session {sandbox,workspace:attempt!(Workspace::create(library, false)),deadline};
    attempt!(session.compile_library(library));
    let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../proofs/mc1/IntrinsicTypeWitness.lean"));
    attempt!(super::artifacts::write_source(&session.workspace.sources.join("IntrinsicTypeWitness.lean"),source.as_bytes()));
    let object = std::path::Path::new("IntrinsicTypeWitness.olean");
    let writable = attempt!(super::artifacts::output_slots(&session.workspace.library,object));
    let mounts = attempt!(super::artifacts::output_mounts(
        std::vec![
            super::artifacts::read_mount(&session.workspace.sources,"/inputs"),
            super::artifacts::read_mount(&session.workspace.library,"/out"),
        ],
        &session.workspace.library,&writable,"/out"));
    let transcript = attempt!(session.compile_module(mounts,"/out",
        compilation::Job {source:"/inputs/IntrinsicTypeWitness.lean",root:"/inputs",output:"/out/IntrinsicTypeWitness.olean"}));
    attempt!(require_success(transcript,"intrinsic-library-rejected","trusted PureTyCode witness failed Lean"));
    attempt!(super::artifacts::finish_outputs(&session.workspace.library,object,&writable));
    let independent = std::format!(r#"import Lean
import IntrinsicTypeWitness
open NobleContracts.Intrinsic
theorem intrinsic_proof : {claim} := {term}
open Lean Elab Command
elab "check_intrinsic_proof_axioms" : command => do
  let env ← getEnv
  let name := ``intrinsic_proof
  match env.find? name with
  | some (.thmInfo _) => pure ()
  | _ => throwError "intrinsic theorem is not kernel checked"
  let axioms ← liftCoreM (collectAxioms name)
  for ax in axioms do
    unless [``propext, ``Classical.choice, ``Quot.sound].contains ax do
      throwError "disallowed intrinsic axiom {{ax}}"
  logInfo "NOBLE-INTRINSIC-ACCEPT"
  for ax in axioms do
    logInfo m!"NOBLE-INTRINSIC-AXIOM:{{ax}}"
check_intrinsic_proof_axioms
"#);
    if independent.len() > super::PROOF_LIMIT {
        return Err(super::output::Failure::error("intrinsic-limit","lowered proof exceeded bounded source size".into()));
    }
    attempt!(super::artifacts::write_source(&session.workspace.consumer.join("IntrinsicConsumer.lean"),independent.as_bytes()));
    let mounts = std::vec![
        super::artifacts::read_mount(&session.workspace.library,"/library"),
        super::artifacts::read_mount(&session.workspace.consumer,"/consumer"),
    ];
    let transcript = attempt!(session.sandbox.run(&mounts,"/library",
        &std::vec!["-R".into(),"/consumer".into(),"-j".into(),"2".into(),"/consumer/IntrinsicConsumer.lean".into()],
        deadline));
    let transcript = attempt!(require_intrinsic_consumer_success(transcript));
    let axioms = attempt!(intrinsic_axioms(&transcript.stdout, "NOBLE-INTRINSIC"));
    Ok(Acceptance {axioms,wire:independent,tools:tools(&session.sandbox,&probe.stdout)})
}

fn intrinsic_axioms(stdout: &str, protocol: &str) -> Result<std::vec::Vec<std::string::String>,super::output::Failure> {
    let mut accepted = false;
    let mut axioms = std::vec::Vec::new();
    let accepted_marker = std::format!("{protocol}-ACCEPT");
    let axiom_marker = std::format!("{protocol}-AXIOM:");
    for line in stdout.lines() {
        if line.ends_with(&accepted_marker) {
            if accepted {
                return Err(super::output::Failure::error("intrinsic-consumer-protocol","duplicate kernel acceptance marker".into()));
            }
            accepted = true;
        } else if let Some((_,name)) = line.split_once(&axiom_marker) {
            let name = name.trim();
            if !accepted || !["propext","Classical.choice","Quot.sound"].contains(&name)
                || axioms.iter().any(|old|old==name) || axioms.len()>=3 {
                return Err(super::output::Failure::error("intrinsic-consumer-protocol","invalid transitive axiom report".into()));
            }
            axioms.push(name.into());
        }
    }
    if !accepted {
        return Err(super::output::Failure::error("intrinsic-consumer-protocol","independent kernel did not report accepted theorem".into()));
    }
    Ok(axioms)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; probe_toolchain checks real sandbox exit status and the pinned Lean version and commit before any proof runs. Launch failures or mismatches return Unsupported with the transcript rather than panic."
)]
fn probe_toolchain(
    sandbox: &crate::sandbox::Environment,
    deadline: std::time::Instant,
) -> Result<crate::sandbox::Transcript, super::output::Failure> {
    let probe = attempt!(sandbox.run(
        &[],
        "",
        &std::vec!["-j".into(), "2".into(), "--version".into()],
        deadline,
    ));
    if !probe.status.success() {
        let mut failure = super::output::Failure::unsupported(
            "sandbox-unavailable",
            "sandbox/toolchain launch failed; no proof code ran".into(),
        );
        failure.details = transcript_json(&probe);
        return Err(failure);
    }
    if !probe.stdout.contains("version 4.31.0,") || !probe.stdout.contains(crate::sandbox::COMMIT) {
        let mut failure = super::output::Failure::unsupported(
            "toolchain-mismatch",
            "selected Lean executable does not report the pinned release and commit".into(),
        );
        failure.details = transcript_json(&probe);
        return Err(failure);
    }
    Ok(probe)
}

fn tools(sandbox: &crate::sandbox::Environment, version: &str) -> super::encoding::Json {
    super::encoding::object([
        (
            "lean",
            super::encoding::string(sandbox.lean.to_string_lossy()),
        ),
        (
            "bubblewrap",
            super::encoding::string(sandbox.bwrap.to_string_lossy()),
        ),
        (
            "prlimit",
            super::encoding::string(sandbox.prlimit.to_string_lossy()),
        ),
        (
            "systemd_run",
            super::encoding::string(sandbox.systemd_run.to_string_lossy()),
        ),
        (
            "aggregate_memory_bytes",
            super::encoding::Json::Number(crate::sandbox::MEMORY_LIMIT),
        ),
        ("aggregate_tasks", super::encoding::Json::Number(64)),
        ("lean_version", super::encoding::string(version.trim())),
    ])
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-maintainers; require_success pairs a static diagnostic code with its stage-specific human message at each call. Both intentionally remain strings and allocation occurs only on failure; reassess if this private reporting interface gains unrelated callers."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; require_success inspects platform ExitStatus and formats an owned transcript diagnostic on failure. Exit-status queries and String/JSON construction are not const on the pinned compiler; reassess if their const support changes."
)]
fn require_success(
    transcript: crate::sandbox::Transcript,
    code: &'static str,
    message: &str,
) -> Result<crate::sandbox::Transcript, super::output::Failure> {
    if transcript.status.success() {
        return Ok(transcript);
    }
    #[cfg(unix)]
    if std::os::unix::process::ExitStatusExt::signal(&transcript.status) == Some(24) {
        let mut error = super::output::Failure::timeout("sandbox exhausted its CPU-time limit");
        error.details = transcript_json(&transcript);
        return Err(error);
    }
    let mut error = super::output::Failure::error(code, message.into());
    error.details = transcript_json(&transcript);
    Err(error)
}

fn require_intrinsic_consumer_success(
    transcript: crate::sandbox::Transcript,
) -> Result<crate::sandbox::Transcript, super::output::Failure> {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let status = &transcript.status;
        // systemd-run/bwrap can relay a child's signal as 128 + signal
        // instead of reporting the signal on their own ExitStatus.
        if status.code() == Some(128 + 24) {
            let mut error = super::output::Failure::timeout("sandbox exhausted its CPU-time limit");
            error.details = transcript_json(&transcript);
            return Err(error);
        }
        let abnormal = status.signal().is_some_and(|signal| signal != 24)
            || status.code().is_some_and(|code| (129..=192).contains(&code));
        if abnormal {
            let mut error = super::output::Failure::error(
                "intrinsic-checker-process-failure",
                "fresh intrinsic proof consumer process terminated abnormally".into(),
            );
            error.details = transcript_json(&transcript);
            return Err(error);
        }
    }
    require_success(transcript,"intrinsic-proof-rejected","fresh kernel rejected exact typed intrinsic proof")
}

fn transcript_json(transcript: &crate::sandbox::Transcript) -> super::encoding::Json {
    super::encoding::object([
        (
            "exit_status",
            super::encoding::string(transcript.status.to_string()),
        ),
        ("stdout", super::encoding::string(&transcript.stdout)),
        ("stderr", super::encoding::string(&transcript.stderr)),
    ])
}
