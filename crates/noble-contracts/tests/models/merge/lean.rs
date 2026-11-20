#![allow(
    tigerstyle::ambient_clock,
    tigerstyle::ambient_env,
    tigerstyle::assertion_density,
    tigerstyle::ambiguous_params,
    tigerstyle::foreign_enum_catch_all,
    tigerstyle::fragile_exhaustive_enum_match,
    tigerstyle::function_length,
    tigerstyle::ignored_result,
    tigerstyle::path_segment_repetition,
    tigerstyle::too_many_parameters,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts-maintainers; this test-only lane discovers the pinned Lean, library and optional bubblewrap paths once at its process boundary and enforces one wall-clock deadline against each child, so the ambient observation is the operation. The private run_step helper names the binary, source, directory, output, mounts, sandbox and deadline together because they are one child-process description. Test-only lane: matches over the observable/fragment enums are intentional regression tables, panics and ignored cleanup results are fixture assertions, and the bounded checker's digest casts index a 16-entry nibble table."
)]
#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only Lean driver imports its sibling claim types directly so the bounded lane stays readable"
)]
//! Isolated pinned Lean check for the test-only `SMERGE/1` lane.
//!
//! The submission is compiled **once** in a producer process that has the full
//! meta-programming API in scope, so its elaborator code runs there — in a
//! fresh producer directory, with a cleared environment, and under
//! bubblewrap when a sandbox binary is available.  Nothing that process prints
//! or writes is trusted.
//!
//! A second, fresh **checker** process never elaborates producer text.  It
//! reads the producer's `.olean` as data (`readModuleData`, no import and no
//! initializers), replays the declarations `merge_claim` reaches through the
//! kernel with every check enabled into an environment built only from the
//! pinned library, requires the replayed theorem's type to be the consumer's
//! own elaboration of the regenerated claim, and computes the axiom closure
//! there against the mc1 allowlist.  Only that process's structured result is
//! acceptance.
//!
//! This is process-level isolation with a pinned binary and recorded
//! source/object digests, not the repository's production consumer; the
//! production integration stays unselected (recorded no-go) and this lane must
//! not be read as one.

use super::claim::{CheckFailure, Submission, LEAN_PIN};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// One process's bounds.  The deadline applies to each of the two processes.
#[derive(Clone, Copy, Debug)]
pub(super) struct Bounds {
    pub(super) timeout_ms: u64,
    pub(super) proof_bytes: usize,
}

impl Bounds {
    pub(super) const fn lane() -> Self {
        Self {
            timeout_ms: 60_000,
            proof_bytes: 16_384,
        }
    }
}

/// Which boundary produced the admitted `.olean`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Isolation {
    Bubblewrap,
    Process,
}

impl Isolation {
    pub(super) fn name(self) -> &'static str {
        match self {
            Isolation::Bubblewrap => "bubblewrap",
            Isolation::Process => "process",
        }
    }
}

const MAX_CLAIM_BYTES: usize = 8_192;
const POLL: Duration = Duration::from_millis(5);
const ACCEPT: &str = "NOBLE-MERGE-ACCEPT:";
const AXIOM: &str = "NOBLE-MERGE-AXIOM:";
const TRANSCRIPT_BYTES: u64 = 262_144;
const DIAGNOSTIC_CHARS: usize = 1_024;
const MARKER_CHARS: usize = 16;
const ALLOWED_AXIOMS: [&str; 3] = ["propext", "Classical.choice", "Quot.sound"];
const MODULES: [&str; 3] = [
    "NobleContracts/Merge",
    "NobleContracts/Rules",
    "NobleContracts/Model",
];
const BWRAP_CANDIDATES: [&str; 3] = [
    "/run/current-system/sw/bin/bwrap",
    "/usr/bin/bwrap",
    "/bin/bwrap",
];

/// Command, directive, IO and protocol tokens no closed proof term needs.  This
/// is defence in depth only: the replay checker, not this list, decides
/// acceptance, and a submission may use elaborator code (the producer process
/// is where that code runs).
const FORBIDDEN_TOKENS: [&str; 47] = [
    "sorry",
    "admit",
    "sorryAx",
    "axiom",
    "unsafe",
    "unsafeCast",
    "partial",
    "opaque",
    "extern",
    "implemented_by",
    "native_decide",
    "run_tac",
    "run_cmd",
    "run_elab",
    "elab",
    "macro",
    "syntax",
    "notation",
    "attribute",
    "set_option",
    "import",
    "export",
    "open",
    "namespace",
    "section",
    "variable",
    "universe",
    "mutual",
    "deriving",
    "termination_by",
    "decreasing_by",
    "dbg_trace",
    "dbgTrace",
    "logInfo",
    "logWarning",
    "logError",
    "trace",
    "trace_state",
    "IO",
    "System",
    "panic",
    "debug",
    "Lean",
    "Parser",
    "Environment",
    "Declaration",
    "CoreM",
];

static WORKSPACES: AtomicU64 = AtomicU64::new(0);

/// What the isolated check observed.  Acceptance is the replay checker's own
/// result; every other state names its refusal, bound or tool failure.
#[derive(Clone, Debug)]
pub(super) struct Acceptance {
    pub(super) submission: Submission,
    pub(super) diagnostic: String,
    pub(super) producer: String,
    pub(super) lean: String,
    pub(super) library: String,
    pub(super) source_digest: String,
    pub(super) object_digest: String,
    pub(super) isolation: Isolation,
}

fn library_root() -> PathBuf {
    match std::env::var_os("NOBLE_CONTRACT_LIBRARY") {
        Some(path) => PathBuf::from(path),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proofs/mc1"),
    }
}

fn lean_binary() -> PathBuf {
    match std::env::var_os("NOBLE_LEAN") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from("lean"),
    }
}

/// The sandbox binary, if one is selected.  Without it the producer still runs
/// in a fresh directory with a cleared environment, but the receipt records
/// `process` isolation rather than claiming a filesystem boundary.
fn sandbox() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("NOBLE_BWRAP") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    for candidate in BWRAP_CANDIDATES {
        let path = Path::new(candidate);
        if path.is_file() {
            return Some(path.to_path_buf());
        }
    }
    None
}

fn read_pin(root: &Path) -> Option<String> {
    std::fs::read_to_string(root.join("lean-toolchain"))
        .ok()
        .map(|text| text.trim().to_owned())
}

fn version(binary: &Path) -> String {
    match Command::new(binary).arg("--version").output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        Err(error) => format!("lean-unavailable: {error}"),
    }
}

/// The pinned toolchain text selects a release; the binary's own banner is
/// checked against it so a different `lean` on the path is a tool failure, not
/// a silently accepted check.
fn version_matches(lean: &str, pin: &str) -> bool {
    let Some(release) = pin.rsplit(':').next() else {
        return false;
    };
    let release = release.trim_start_matches('v');
    !release.is_empty() && lean.contains(&format!("version {release}"))
}

fn module_digest(root: &Path, modules: &[&str], suffix: &str) -> String {
    let mut bytes = Vec::new();
    for module in modules {
        bytes.extend_from_slice(module.as_bytes());
        let path = root.join(format!("{module}{suffix}"));
        match std::fs::read(path) {
            Ok(source) => {
                bytes.extend_from_slice(&(source.len() as u64).to_le_bytes());
                bytes.extend_from_slice(&source);
            }
            Err(_) => bytes.extend_from_slice(b"missing"),
        }
    }
    super::claim::sha256(&bytes)
}

fn source_digest(root: &Path) -> String {
    module_digest(root, &MODULES, ".lean")
}

/// The object files Lean actually loads.  Recorded (not compared with a pin):
/// the runner rebuilds before the check, so this binds the bytes that ran.
fn object_digest(root: &Path) -> String {
    let modules: [&str; 3] = [
        "NobleContracts/Merge",
        "NobleContracts/Rules",
        "NobleContracts/Model",
    ];
    let mut bytes = Vec::new();
    for module in modules {
        bytes.extend_from_slice(module.as_bytes());
        let path = root
            .join(".lake/build/lib/lean")
            .join(format!("{module}.olean"));
        match std::fs::read(path) {
            Ok(object) => {
                bytes.extend_from_slice(&(object.len() as u64).to_le_bytes());
                bytes.extend_from_slice(&object);
            }
            Err(_) => bytes.extend_from_slice(b"missing"),
        }
    }
    super::claim::sha256(&bytes)
}

/// Render `text` as the body of one Lean string literal.  The closed-term guard
/// permits only bytes the encoding handles: printable ASCII plus tab/newline.
fn escape(text: &str) -> String {
    let mut rendered = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => rendered.push_str("\\\\"),
            '"' => rendered.push_str("\\\""),
            '\n' => rendered.push_str("\\n"),
            '\t' => rendered.push_str("\\t"),
            other => rendered.push(other),
        }
    }
    rendered
}

/// The acceptance marker binds the exact claim and proof bytes.  The producer
/// source does not contain it (only the checker source does), so the producer
/// process cannot learn it from its own input.
fn nonce(claim: &str, proof: &str) -> String {
    let mut material =
        String::with_capacity(claim.len().saturating_add(proof.len()).saturating_add(1));
    material.push_str(claim);
    material.push('\u{0}');
    material.push_str(proof);
    super::claim::sha256(material.as_bytes())
        .chars()
        .take(MARKER_CHARS)
        .collect()
}

/// A submission is one closed Lean term: printable ASCII plus tab/newline, no
/// `#` directives, no protocol marker and no obvious command/IO token.  This
/// guard is not the decision boundary; it only keeps obviously malformed
/// submissions out of the elaborator.
fn proof_is_closed_term(proof: &str) -> Result<(), String> {
    for character in proof.chars() {
        if character == '\n' || character == '\t' {
            continue;
        }
        if character == '\r' {
            return Err("carriage return".into());
        }
        if character.is_ascii_control() || !character.is_ascii() {
            return Err(format!("character U+{:04X}", character as u32));
        }
    }
    if proof.contains('#') {
        return Err("Lean directive `#`".into());
    }
    if proof.contains(ACCEPT) {
        return Err("checker protocol marker".into());
    }
    let mut token = String::new();
    for character in proof.chars().chain(std::iter::once(' ')) {
        if character.is_ascii_alphanumeric() || character == '_' {
            token.push(character);
        } else if !token.is_empty() {
            if FORBIDDEN_TOKENS.contains(&token.as_str()) {
                return Err(format!("forbidden Lean token `{token}`"));
            }
            token.clear();
        }
    }
    Ok(())
}

/// The producer module: it elaborates the claim and the submission and adds
/// `merge_claim`.  Its only output that crosses to the checker is the `.olean`.
const PRODUCER_HEAD: &str = r#"import Lean
import NobleContracts.Merge
open Lean Elab Command
open NobleContracts
set_option Elab.async false
set_option autoImplicit false

elab "produce_merge_claim" : command => do
"#;

const PRODUCER_BODY: &str = r#"  let env ← getEnv
  let claimSyntax ← match Lean.Parser.runParserCategory env `term claimText with
    | .ok parsed => pure parsed
    | .error error => throwError "claim parse: {error}"
  let proofSyntax ← match Lean.Parser.runParserCategory env `term proofText with
    | .ok parsed => pure parsed
    | .error error => throwError "proof parse: {error}"
  let (type, value) ← liftTermElabM do
    let type ← Term.elabType claimSyntax
    let value ← Term.elabTermEnsuringType proofSyntax (some type)
    Term.synthesizeSyntheticMVarsNoPostponing
    let type ← instantiateMVars type
    let value ← instantiateMVars value
    return (type, value)
  liftCoreM <| addDecl (.thmDecl { name := `merge_claim, levelParams := [], type := type, value := value })

produce_merge_claim
"#;

fn producer_source(claim: &str, proof: &str) -> String {
    let mut source = String::from(PRODUCER_HEAD);
    source.push_str(&format!(
        "  let claimText : String := \"{}\"\n",
        escape(claim)
    ));
    source.push_str(&format!(
        "  let proofText : String := \"{}\"\n",
        escape(proof)
    ));
    source.push_str(PRODUCER_BODY);
    source
}

/// The checker module.  It never imports the producer module and never
/// elaborates producer text: it reads the `.olean` as data, replays the target's
/// declaration closure through the kernel with checks on, compares the replayed
/// type with its own elaboration of the claim, and walks the axioms there.
const CHECKER_HEAD: &str = r#"import Lean
import NobleContracts.Merge
open Lean Elab Command
open NobleContracts
set_option Elab.async false
set_option autoImplicit false

elab "check_merge_replay" : command => do
"#;

const CHECKER_BODY: &str = r#"  let (data, _region) ← liftIO (Lean.readModuleData producer)
  let env0 ← getEnv
  let mut infos : Std.HashMap Name ConstantInfo := {}
  for info in data.constants do
    match info with
    | .axiomInfo ax => throwError "producer module declares an axiom: {ax.name}"
    | _ => infos := infos.insert info.name info
  let usedConstants (info : ConstantInfo) : Array Name :=
    match info with
    | .thmInfo thm => thm.type.getUsedConstants ++ thm.value.getUsedConstants
    | .defnInfo defn => defn.type.getUsedConstants ++ defn.value.getUsedConstants
    | _ => info.type.getUsedConstants
  let some _ := infos[`merge_claim]?
    | throwError "producer module has no merge_claim"
  let mut closure : Array Name := #[]
  let mut queue : Array Name := #[`merge_claim]
  let mut queued : Std.HashSet Name := {`merge_claim}
  while queue.size > 0 do
    let name := queue.back!
    queue := queue.pop
    closure := closure.push name
    match infos[name]? with
    | some info =>
      for dep in usedConstants info do
        if infos.contains dep && !queued.contains dep then
          queue := queue.push dep
          queued := queued.insert dep
    | none => pure ()
  let mut order : Array Name := #[]
  let mut done : Std.HashSet Name := {}
  let mut remaining := closure
  for _ in [:closure.size] do
    let mut next : Array Name := #[]
    for name in remaining do
      let ready := match infos[name]? with
        | some info => (usedConstants info).all fun dep => !infos.contains dep || done.contains dep
        | none => true
      if ready then
        order := order.push name
        done := done.insert name
      else
        next := next.push name
    remaining := next
  unless remaining.size == 0 do
    throwError "producer declarations are cyclic or unresolved"
  let mut env := env0
  for name in order do
    let some info := infos[name]?
      | throwError "producer declaration vanished: {name}"
    match info with
    | .thmInfo thm =>
      match env.addDeclCore 0 0 (.thmDecl thm) none with
      | .ok next => env := next
      | .error error => throwKernelException error
    | .defnInfo defn =>
      match env.addDeclCore 0 0 (.defnDecl defn) none with
      | .ok next => env := next
      | .error error => throwKernelException error
    | _ => throwError "unsupported producer declaration: {name}"
  setEnv env
  let some info := (← getEnv).find? `merge_claim
    | throwError "producer module has no merge_claim"
  let actual := info.type
  let claimSyntax ← match Lean.Parser.runParserCategory (← getEnv) `term claimText with
    | .ok parsed => pure parsed
    | .error error => throwError "claim parse: {error}"
  let expected ← liftTermElabM do
    let type ← Term.elabType claimSyntax
    Term.synthesizeSyntheticMVarsNoPostponing
    instantiateMVars type
  unless actual == expected do
    unless (← liftTermElabM (Lean.Meta.isDefEq actual expected)) do
      throwError "replayed theorem type differs from the regenerated claim"
  let axioms ← liftCoreM (collectAxioms `merge_claim)
  for ax in axioms do
    unless [``propext, ``Classical.choice, ``Quot.sound].contains ax do
      throwError "disallowed merge axiom {ax}"
  logInfo marker
  for ax in axioms do
    logInfo m!"NOBLE-MERGE-AXIOM:{ax}"

check_merge_replay
"#;

fn checker_source(claim: &str, producer: &Path, marker: &str) -> String {
    let mut source = String::from(CHECKER_HEAD);
    source.push_str(&format!(
        "  let claimText : String := \"{}\"\n",
        escape(claim)
    ));
    source.push_str(&format!(
        "  let producer : String := \"{}\"\n",
        escape(&producer.display().to_string())
    ));
    source.push_str(&format!("  let marker : String := \"{marker}\"\n"));
    source.push_str(CHECKER_BODY);
    source
}

/// Parse the replay checker's stdout strictly: exactly one digest-derived
/// acceptance marker, axiom reports only after it, every axiom allowlisted and
/// distinct.  Nothing the producer process printed is read here.
pub(super) fn parse_transcript(stdout: &str, marker: &str) -> Result<Vec<String>, String> {
    let mut is_accepted = false;
    let mut axioms: Vec<String> = Vec::with_capacity(3);
    for line in stdout.lines().take(4_096) {
        let trimmed = line.trim();
        if trimmed == marker {
            if is_accepted {
                return Err("duplicate kernel acceptance marker".into());
            }
            is_accepted = true;
            continue;
        }
        if trimmed.contains(ACCEPT) {
            return Err("unexpected acceptance marker in the checker transcript".into());
        }
        if trimmed.starts_with(AXIOM) {
            if !is_accepted {
                return Err("axiom report before kernel acceptance".into());
            }
            let Some((_, name)) = trimmed.split_once(AXIOM) else {
                return Err("invalid axiom report".into());
            };
            let name = name.trim();
            if !ALLOWED_AXIOMS.contains(&name) {
                return Err(format!("disallowed merge axiom {name}"));
            }
            if axioms.iter().any(|old| old == name) {
                return Err("duplicate merge axiom report".into());
            }
            if axioms.len() >= ALLOWED_AXIOMS.len() {
                return Err("invalid transitive axiom report".into());
            }
            axioms.push(name.to_owned());
        } else if trimmed.contains(AXIOM) {
            return Err("unexpected axiom report in the checker transcript".into());
        }
    }
    if !is_accepted {
        return Err("checker did not report kernel acceptance".into());
    }
    Ok(axioms)
}

fn transcript(path: &Path) -> String {
    let Ok(file) = File::open(path) else {
        return String::new();
    };
    let mut limited = file.take(TRANSCRIPT_BYTES);
    let mut bytes = Vec::new();
    let _ = limited.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

/// One child process's observation.
struct Step {
    status: Option<std::process::ExitStatus>,
    is_timed_out: bool,
    start_error: Option<String>,
    stdout: String,
    stderr: String,
}

impl Step {
    fn output(&self) -> String {
        let mut text = self.stdout.clone();
        text.push_str(&self.stderr);
        text.chars().take(DIAGNOSTIC_CHARS).collect()
    }
}

/// Run one pinned Lean process in `cwd`, writing its output files, under
/// bubblewrap when selected.
#[expect(
    tigerstyle::contradictory_time,
    reason = "Owner: noble-maintainers; timeout_ms is a per-child duration budget, not an injected timestamp; the monotonic clock is sampled once to set that child's deadline"
)]
fn run_step(
    binary: &Path,
    source: &Path,
    cwd: &Path,
    output: Option<&Path>,
    read_only: &[&Path],
    sandbox: Option<&Path>,
    timeout_ms: u64,
) -> Step {
    let stdout_path = cwd.join("stdout.txt");
    let stderr_path = cwd.join("stderr.txt");
    let (Ok(stdout_file), Ok(stderr_file)) =
        (File::create(&stdout_path), File::create(&stderr_path))
    else {
        return Step {
            status: None,
            is_timed_out: false,
            start_error: Some("cannot create the process transcript".into()),
            stdout: String::new(),
            stderr: String::new(),
        };
    };
    let mut command = match sandbox {
        Some(bwrap) => {
            let library = read_only
                .first()
                .map(|path| path.display().to_string())
                .unwrap_or_default();
            let mut command = Command::new(bwrap);
            command
                .arg("--unshare-all")
                .arg("--die-with-parent")
                .arg("--new-session")
                .arg("--cap-drop")
                .arg("ALL")
                .arg("--clearenv")
                .arg("--proc")
                .arg("/proc")
                .arg("--dev")
                .arg("/dev")
                .arg("--tmpfs")
                .arg("/tmp")
                .arg("--ro-bind")
                .arg("/nix/store")
                .arg("/nix/store");
            for path in read_only {
                command.arg("--ro-bind").arg(path).arg(path);
            }
            command
                .arg("--bind")
                .arg(cwd)
                .arg(cwd)
                .arg("--chdir")
                .arg(cwd)
                .arg("--setenv")
                .arg("HOME")
                .arg("/tmp")
                .arg("--setenv")
                .arg("TMPDIR")
                .arg("/tmp")
                .arg("--setenv")
                .arg("LEAN_PATH")
                .arg(&library)
                .arg(binary)
                .arg("-j")
                .arg("2");
            command
        }
        None => {
            let mut command = Command::new(binary);
            command
                .current_dir(cwd)
                .env_clear()
                .env(
                    "LEAN_PATH",
                    read_only.first().copied().unwrap_or(Path::new("")),
                )
                .env("HOME", cwd)
                .env("TMPDIR", cwd)
                .arg("-j")
                .arg("2");
            command
        }
    };
    if let Some(output) = output {
        command.arg("-o").arg(output);
    }
    command
        .arg(source)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Step {
                status: None,
                is_timed_out: false,
                start_error: Some(format!("cannot start the pinned process: {error}")),
                stdout: String::new(),
                stderr: String::new(),
            };
        }
    };
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut is_timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() >= deadline => {
                is_timed_out = true;
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    Step {
        status,
        is_timed_out,
        start_error: None,
        stdout: transcript(&stdout_path),
        stderr: transcript(&stderr_path),
    }
}

/// Build the observation value; the identity fields travel together.
struct Identity {
    lean: String,
    library: String,
    source_digest: String,
    object_digest: String,
    producer: String,
    isolation: Isolation,
}

fn observed(submission: Submission, diagnostic: String, identity: Identity) -> Acceptance {
    Acceptance {
        submission,
        diagnostic,
        producer: identity.producer,
        lean: identity.lean,
        library: identity.library,
        source_digest: identity.source_digest,
        object_digest: identity.object_digest,
        isolation: identity.isolation,
    }
}

fn refused(diagnostic: String, identity: Identity) -> Acceptance {
    observed(Submission::refused(), diagnostic, identity)
}

fn failed(failure: CheckFailure, diagnostic: String, identity: Identity) -> Acceptance {
    observed(Submission::failed(failure), diagnostic, identity)
}

/// Run the isolated check against the library the environment selects.
pub(super) fn check(claim: &str, proof: &str, bounds: Bounds) -> Acceptance {
    check_at(&library_root(), claim, proof, bounds)
}

/// Run the isolated check against `root`; the lane and its controls use the
/// environment-selected root, while the internal-failure control names one.
pub(super) fn check_at(root: &Path, claim: &str, proof: &str, bounds: Bounds) -> Acceptance {
    let isolation = match sandbox() {
        Some(_) => Isolation::Bubblewrap,
        None => Isolation::Process,
    };
    let mut identity = Identity {
        lean: String::new(),
        library: root.display().to_string(),
        source_digest: source_digest(root),
        object_digest: object_digest(root),
        producer: String::new(),
        isolation,
    };
    if claim.len() > MAX_CLAIM_BYTES || proof.len() > bounds.proof_bytes {
        return failed(
            CheckFailure::Exhausted,
            "claim or proof exceeded its byte bound".into(),
            identity,
        );
    }
    if let Err(reason) = proof_is_closed_term(proof) {
        return refused(
            format!("proof submission is not one closed term: {reason}"),
            identity,
        );
    }
    let binary = lean_binary();
    identity.lean = version(&binary);
    let Some(pin) = read_pin(root) else {
        return failed(
            CheckFailure::InternalFailure,
            "library does not select a Lean release".into(),
            identity,
        );
    };
    if pin != LEAN_PIN {
        return failed(
            CheckFailure::InternalFailure,
            format!("library selects `{pin}`, not the pinned `{LEAN_PIN}`"),
            identity,
        );
    }
    if !version_matches(&identity.lean, &pin) {
        return failed(
            CheckFailure::InternalFailure,
            format!(
                "checker binary is not the pinned release: {}",
                identity.lean
            ),
            identity,
        );
    }
    let build = root.join(".lake/build/lib/lean");
    for module in MODULES {
        if !build.join(format!("{module}.olean")).is_file() {
            return failed(
                CheckFailure::InternalFailure,
                format!("pinned module {module} is not built"),
                identity,
            );
        }
    }
    let digest = nonce(claim, proof);
    let marker = format!("{ACCEPT}{digest}");
    let sequence = WORKSPACES.fetch_add(1, Ordering::Relaxed);
    let workspace = std::env::temp_dir().join(format!(
        "noble-merge-lean-{}-{sequence}-{digest}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&workspace);
    let producer_dir = workspace.join("producer");
    let checker_dir = workspace.join("checker");
    if std::fs::create_dir_all(&producer_dir).is_err() {
        return failed(
            CheckFailure::InternalFailure,
            "cannot create the producer workspace".into(),
            identity,
        );
    }
    let producer_path = producer_dir.join("MergeProducer.lean");
    let olean = producer_dir.join("MergeProducer.olean");
    if std::fs::write(&producer_path, producer_source(claim, proof).as_bytes()).is_err() {
        let _ = std::fs::remove_dir_all(&workspace);
        return failed(
            CheckFailure::InternalFailure,
            "cannot write the producer module".into(),
            identity,
        );
    }
    let producer = run_step(
        &binary,
        &producer_path,
        &producer_dir,
        Some(&olean),
        &[&build],
        sandbox().as_deref(),
        bounds.timeout_ms,
    );
    identity.producer = producer.output();
    if producer.is_timed_out {
        let _ = std::fs::remove_dir_all(&workspace);
        return failed(
            CheckFailure::Exhausted,
            format!("producer deadline {}ms reached", bounds.timeout_ms),
            identity,
        );
    }
    if let Some(error) = producer.start_error {
        let _ = std::fs::remove_dir_all(&workspace);
        return failed(CheckFailure::InternalFailure, error, identity);
    }
    // A hostile producer may have created files in this path; the checker
    // directory is recreated after the producer exits so only the Rust writer
    // owns its contents.
    let _ = std::fs::remove_dir_all(&checker_dir);
    if std::fs::create_dir_all(&checker_dir).is_err() {
        let _ = std::fs::remove_dir_all(&workspace);
        return failed(
            CheckFailure::InternalFailure,
            "cannot create the checker workspace".into(),
            identity,
        );
    }
    let checker_path = checker_dir.join("MergeCheck.lean");
    if std::fs::write(
        &checker_path,
        checker_source(claim, &olean, &marker).as_bytes(),
    )
    .is_err()
    {
        let _ = std::fs::remove_dir_all(&workspace);
        return failed(
            CheckFailure::InternalFailure,
            "cannot write the checker module".into(),
            identity,
        );
    }
    let checker = run_step(
        &binary,
        &checker_path,
        &checker_dir,
        None,
        &[&build, &producer_dir],
        sandbox().as_deref(),
        bounds.timeout_ms,
    );
    let _ = std::fs::remove_dir_all(&workspace);
    if checker.is_timed_out {
        return failed(
            CheckFailure::Exhausted,
            format!("checker deadline {}ms reached", bounds.timeout_ms),
            identity,
        );
    }
    if let Some(error) = checker.start_error {
        return failed(CheckFailure::InternalFailure, error, identity);
    }
    let is_success = matches!(checker.status, Some(status) if status.success());
    if !is_success {
        let mut text = checker.output();
        if text.is_empty() {
            text = "the replay checker failed".into();
        }
        return refused(text, identity);
    }
    match parse_transcript(&checker.stdout, &marker) {
        Ok(axioms) => observed(
            Submission::accepted(axioms),
            "the replay checker kernel-checked the regenerated claim".into(),
            identity,
        ),
        Err(reason) => refused(reason, identity),
    }
}
