#![allow(
    tigerstyle::ambient_clock,
    tigerstyle::ambient_env,
    tigerstyle::assertion_density,
    tigerstyle::ambiguous_params,
    tigerstyle::foreign_enum_catch_all,
    tigerstyle::function_length,
    tigerstyle::ignored_result,
    tigerstyle::path_segment_repetition,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts-maintainers; this test-only lane discovers the pinned Lean and library paths once at its process boundary and enforces one wall-clock deadline against the child checker, so the ambient observation is the operation. Test-only lane: matches over the observable/fragment enums are intentional regression tables, panics and ignored cleanup results are fixture assertions, and the bounded checker's digest casts index a 16-entry nibble table."
)]
#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only Lean driver imports its sibling claim types directly so the bounded lane stays readable"
)]
//! Isolated pinned Lean check for the test-only `SMERGE/1` lane.
//!
//! The producer's submission is **data, never Lean source**.  The consumer
//! regenerates the claim, escapes the submitted proof bytes into a string
//! literal of a consumer-written module, and lets the module's own elaborator
//! parse exactly one closed term (`Lean.Parser.runParserCategory` requires end
//! of input) and elaborate it against the regenerated claim type.  It then
//! adds `merge_claim` (synchronously kernel-checked), walks the declaration's
//! axioms against the mc1 allowlist and fails the command on any other axiom.
//! Only that consumer post-check, keyed by a digest-derived acceptance marker
//! that appears exactly once in the transcript, yields acceptance.
//!
//! This is process-level isolation with a pinned binary, a cleared child
//! environment and recorded source/object digests, not the repository's
//! bubblewrap production consumer; the production integration stays unselected
//! (recorded no-go) and this lane must not be read as one.

use super::claim::{CheckFailure, Submission, LEAN_PIN};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// One check's bounds.
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

/// Command, directive, IO and protocol tokens no closed proof term needs.  The
/// submission is parsed as one `term`, so these cannot smuggle commands; the
/// guard is defence in depth against elaborator-time IO (`run_tac` writes
/// files), trace output that mimics the protocol and kernel-proof cheats.
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

/// What the isolated check observed.  Acceptance is the consumer's own
/// post-check; every other state names its refusal, bound or tool failure.
#[derive(Clone, Debug)]
pub(super) struct Acceptance {
    pub(super) submission: Submission,
    pub(super) diagnostic: String,
    pub(super) lean: String,
    pub(super) library: String,
    pub(super) source_digest: String,
    pub(super) object_digest: String,
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

/// A length-delimited digest over the named modules under `root`; a missing
/// module is recorded as missing instead of being skipped.
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

/// Render `text` as the body of one Lean string literal.  Only bytes the
/// closed-term guard permits reach here: printable ASCII plus tab/newline.
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

/// The acceptance marker is derived from the exact claim and proof bytes, so a
/// term cannot print it without the submission's own digest (and any duplicate
/// marker is a protocol failure).
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

/// A submission is one closed Lean term: printable ASCII plus tab/newline,
/// no `#` directives, no protocol marker and no command, IO or trace token.
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

/// The consumer-written module.  The claim and the submission appear only as
/// escaped string literals; the module's own elaborator parses the claim as a
/// type and the submission as exactly one `term`, elaborates it against that
/// type, adds the declaration with the kernel check enabled (`Elab.async
/// false`) and emits the nonced acceptance marker only after the axiom walk.
fn consumer_source(claim: &str, proof: &str, marker: &str) -> String {
    format!(
        r#"import Lean
import NobleContracts.Merge
open Lean Elab Command
open NobleContracts
set_option Elab.async false
set_option autoImplicit false

elab "check_merge_claim_axioms" : command => do
  let claimText : String := "{claim}"
  let proofText : String := "{proof}"
  let marker : String := "{marker}"
  let env ← getEnv
  let claimSyntax ← match Lean.Parser.runParserCategory env `term claimText with
    | .ok parsed => pure parsed
    | .error error => throwError "cannot parse the regenerated claim: {{error}}"
  let proofSyntax ← match Lean.Parser.runParserCategory env `term proofText with
    | .ok parsed => pure parsed
    | .error error => throwError "the submitted proof is not one closed term: {{error}}"
  let (type, value) ← liftTermElabM do
    let type ← Term.elabType claimSyntax
    let value ← Term.elabTermEnsuringType proofSyntax (some type)
    Term.synthesizeSyntheticMVarsNoPostponing
    let type ← instantiateMVars type
    let value ← instantiateMVars value
    return (type, value)
  liftCoreM <| addDecl (.thmDecl {{ name := `merge_claim, levelParams := [], type := type, value := value }})
  let axioms ← liftCoreM (collectAxioms `merge_claim)
  for ax in axioms do
    unless [``propext, ``Classical.choice, ``Quot.sound].contains ax do
      throwError "disallowed merge axiom {{ax}}"
  logInfo marker
  for ax in axioms do
    logInfo m!"NOBLE-MERGE-AXIOM:{{ax}}"

check_merge_claim_axioms
"#,
        claim = escape(claim),
        proof = escape(proof),
        marker = marker,
    )
}

/// Parse the consumer post-check strictly: exactly one nonced acceptance
/// marker, axiom reports only after it, every axiom allowlisted and distinct.
/// Any other appearance of the protocol marker is a failure, so a trace line
/// that mimics the marker cannot be counted (and a duplicate cannot hide).
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
            continue;
        }
        if trimmed.contains(AXIOM) {
            return Err("unexpected axiom report in the checker transcript".into());
        }
    }
    if !is_accepted {
        return Err("checker did not report kernel acceptance".into());
    }
    Ok(axioms)
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-maintainers; the observation, its diagnostic and the tool/library/digest identities are one recorded value for this test-only checker, and the private helpers are called only by check_at with named fields"
)]
fn observed(
    submission: Submission,
    diagnostic: String,
    lean: String,
    library: String,
    source: String,
    object: String,
) -> Acceptance {
    Acceptance {
        submission,
        diagnostic,
        lean,
        library,
        source_digest: source,
        object_digest: object,
    }
}

fn refused(
    diagnostic: String,
    lean: String,
    library: String,
    source: String,
    object: String,
) -> Acceptance {
    observed(
        Submission::refused(),
        diagnostic,
        lean,
        library,
        source,
        object,
    )
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-maintainers; the failure kind, its diagnostic and the tool/library/digest identities are one recorded value for this test-only checker, and the private helper is called only by check_at with named fields"
)]
fn failed(
    failure: CheckFailure,
    diagnostic: String,
    lean: String,
    library: String,
    source: String,
    object: String,
) -> Acceptance {
    observed(
        Submission::failed(failure),
        diagnostic,
        lean,
        library,
        source,
        object,
    )
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

/// Run the isolated check against the library the environment selects.
pub(super) fn check(claim: &str, proof: &str, bounds: Bounds) -> Acceptance {
    check_at(&library_root(), claim, proof, bounds)
}

/// Run the isolated check against `root`; the lane and its controls use the
/// environment-selected root, while the internal-failure control names one.
pub(super) fn check_at(root: &Path, claim: &str, proof: &str, bounds: Bounds) -> Acceptance {
    let library = root.display().to_string();
    let source = source_digest(root);
    let object = object_digest(root);
    if claim.len() > MAX_CLAIM_BYTES || proof.len() > bounds.proof_bytes {
        return failed(
            CheckFailure::Exhausted,
            "claim or proof exceeded its byte bound".into(),
            String::new(),
            library,
            source,
            object,
        );
    }
    if let Err(reason) = proof_is_closed_term(proof) {
        return refused(
            format!("proof submission is not one closed term: {reason}"),
            String::new(),
            library,
            source,
            object,
        );
    }
    let binary = lean_binary();
    let lean = version(&binary);
    let Some(pin) = read_pin(root) else {
        return failed(
            CheckFailure::InternalFailure,
            "library does not select a Lean release".into(),
            lean,
            library,
            source,
            object,
        );
    };
    if pin != LEAN_PIN {
        return failed(
            CheckFailure::InternalFailure,
            format!("library selects `{pin}`, not the pinned `{LEAN_PIN}`"),
            lean,
            library,
            source,
            object,
        );
    }
    if !version_matches(&lean, &pin) {
        return failed(
            CheckFailure::InternalFailure,
            format!("checker binary is not the pinned release: {lean}"),
            lean,
            library,
            source,
            object,
        );
    }
    let build = root.join(".lake/build/lib/lean");
    for module in MODULES {
        if !build.join(format!("{module}.olean")).is_file() {
            return failed(
                CheckFailure::InternalFailure,
                format!("pinned module {module} is not built"),
                lean,
                library,
                source,
                object,
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
    if std::fs::create_dir_all(&workspace).is_err() {
        return failed(
            CheckFailure::InternalFailure,
            "cannot create the checker workspace".into(),
            lean,
            library,
            source,
            object,
        );
    }
    let source_path = workspace.join("MergeConsumer.lean");
    if std::fs::write(
        &source_path,
        consumer_source(claim, proof, &marker).as_bytes(),
    )
    .is_err()
    {
        let _ = std::fs::remove_dir_all(&workspace);
        return failed(
            CheckFailure::InternalFailure,
            "cannot write the consumer module".into(),
            lean,
            library,
            source,
            object,
        );
    }
    let stdout_path = workspace.join("stdout.txt");
    let stderr_path = workspace.join("stderr.txt");
    let (Ok(stdout_file), Ok(stderr_file)) =
        (File::create(&stdout_path), File::create(&stderr_path))
    else {
        let _ = std::fs::remove_dir_all(&workspace);
        return failed(
            CheckFailure::InternalFailure,
            "cannot create the checker transcript".into(),
            lean,
            library,
            source,
            object,
        );
    };
    let spawned = Command::new(&binary)
        .arg("-j")
        .arg("2")
        .arg(&source_path)
        .env_clear()
        .env("LEAN_PATH", &build)
        .current_dir(&workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn();
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&workspace);
            return failed(
                CheckFailure::InternalFailure,
                format!("cannot start the pinned checker: {error}"),
                lean,
                library,
                source,
                object,
            );
        }
    };
    let deadline = Instant::now() + Duration::from_millis(bounds.timeout_ms);
    let mut is_exhausted = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= deadline => {
                is_exhausted = true;
                let _ = child.kill();
                let _ = child.wait();
                break;
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(error) => {
                let _ = std::fs::remove_dir_all(&workspace);
                return failed(
                    CheckFailure::InternalFailure,
                    format!("checker wait failed: {error}"),
                    lean,
                    library,
                    source,
                    object,
                );
            }
        }
    }
    let waited = child.wait();
    let stdout = transcript(&stdout_path);
    let stderr = transcript(&stderr_path);
    let _ = std::fs::remove_dir_all(&workspace);
    if is_exhausted {
        return failed(
            CheckFailure::Exhausted,
            format!("checker deadline {}ms reached", bounds.timeout_ms),
            lean,
            library,
            source,
            object,
        );
    }
    let Ok(status) = waited else {
        return failed(
            CheckFailure::InternalFailure,
            "checker status unavailable".into(),
            lean,
            library,
            source,
            object,
        );
    };
    if !status.success() {
        let mut text = stdout;
        text.push_str(&stderr);
        let bounded: String = text.chars().take(DIAGNOSTIC_CHARS).collect();
        return refused(bounded, lean, library, source, object);
    }
    match parse_transcript(&stdout, &marker) {
        Ok(axioms) => observed(
            Submission::accepted(axioms),
            "the isolated consumer kernel-checked the regenerated claim".into(),
            lean,
            library,
            source,
            object,
        ),
        Err(reason) => refused(reason, lean, library, source, object),
    }
}
