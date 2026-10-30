//! L1: the ordinary Noble Wasm/V8 path through the real `noble` CLI.
//!
//! Every observation comes from a `noble session --framed --opt off|on`
//! child process. Each submission is exact source text and each
//! `noble-core-report/v1` line's bottom-to-top `stack` is the ordinary
//! observation. Nothing here links the compiler; the binary is selected by
//! absolute path (receipts record its SHA-256).
//!
//! Session use mirrors a host: definitions are committed first; then, per
//! vector, one submission clears the previous outputs and pushes the inputs
//! as literals (`drop … 5 4`) and one submission is the root body itself, so
//! the root is checked against exactly the input stack types that extraction
//! uses. A session retains every prepared module, so long runs are chunked
//! into fresh sessions (`MAX_SUBMISSIONS`), each of which re-commits the
//! definitions; transitions carry the state across chunks on the host side.
//!
//! Every report is attributed before it counts (issue #75):
//! - **Protocol.** A report carries the [`SCHEMA`] schema, the [`PROFILE`]
//!   profile and the [`BACKEND`] backend. Reports arrive one per submission,
//!   with the session-local ordinal `submission` equal to the position plus
//!   one; a fresh process starts again at one.
//! - **Runtime reports.** A runtime (`wasm`) report carries its status,
//!   stack, session state and module record. The module record names the
//!   same ordinal, the SHA-256 of the exact submitted text and the `--opt`
//!   leg.
//! - **Roles.** A submission's [`Role`] refuses a report of the wrong kind: a
//!   definition never reports `normal`, and an input or root never reports
//!   `defined`.
//! - **Completion.** The process exit status must be the maximum of the
//!   CLI's per-outcome statuses ([`exit_status`]). A session may end before
//!   its last submission only after a terminal outcome ([`terminal`]), which
//!   is also the only case where an undelivered submission is legitimate.
//!
//! Anything else is a [`SessionFailure`]. It keeps the validated reports and
//! the child's status, but never yields a completed session. The CLI and its
//! engine stay trusted: a matching self-reported digest does not attest a
//! malicious child or the semantics of the artifact.
//!
//! Phase C reuse: W1 = [`Cli::run_vectors`] on I1, W2 =
//! [`Cli::run_transition`] on I4. The CLI writes one report line per
//! submission as it finishes, so each line's arrival time (from the session
//! process spawn) times that submission ([`Timing`]); the CLI reports no
//! timing itself.

use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::os::unix::process::ExitStatusExt as _;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Submissions per session process. A Core session retains each prepared
/// module; ~430 two-value submissions exhaust its allocation quota with the
/// pinned build, so chunks stay well below that.
pub const MAX_SUBMISSIONS: usize = 240;

/// The report schema of the ordinary path.
pub const SCHEMA: &str = "noble-core-report/v1";

/// The profile `noble session --framed` reports.
pub const PROFILE: &str = "Core-Bootstrap";

/// The backend the ordinary path reports.
pub const BACKEND: &str = "managed-linear-memory";

/// Upper bound on one report line, in bytes.
pub const MAX_REPORT_BYTES: usize = 1 << 20;

/// Reports a [`SessionFailure`] retains, the most recent first dropped last.
pub const MAX_FAILURE_REPORTS: usize = 32;

/// Bytes of a report line quoted in a diagnostic.
const EXCERPT_BYTES: usize = 240;

/// `--opt` selection of the ordinary path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opt {
    /// `--opt off`.
    Off,
    /// `--opt on` (wasm-opt `-O2`).
    On,
}

impl Opt {
    /// CLI spelling.
    #[must_use]
    pub const fn flag(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
        }
    }

    /// The module record's `optimized` field for this leg.
    #[must_use]
    pub const fn optimized(self) -> bool {
        matches!(self, Self::On)
    }
}

/// What a submission is for, and so which reports may answer it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A committed definition (`def …`): `defined` or a refusal, never `normal`.
    Definition,
    /// Host input setup (`drop … 5 4`): never `defined`.
    Input,
    /// The root body under comparison: never `defined`.
    Root,
    /// A caller-chosen submission with no role expectation.
    Any,
}

/// One observed stack value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// `I64`.
    I64(i64),
    /// `Bool`.
    Bool(bool),
    /// Any other value, by its reported type name.
    Other(String),
}

impl Value {
    /// Source literal for an input value.
    ///
    /// # Panics
    /// For [`Value::Other`], which has no literal.
    #[must_use]
    pub fn literal(&self) -> String {
        match self {
            Self::I64(v) => v.to_string(),
            Self::Bool(b) => b.to_string(),
            Self::Other(t) => panic!("no source literal for {t}"),
        }
    }
}

/// The runtime fields of a `wasm`-stage report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Runtime {
    /// `status`: zero for a normal run.
    pub status: i64,
    /// `session_state`: `retained` after a normal run.
    pub session_state: String,
    /// The module record's `submission`.
    pub module_submission: u64,
    /// The module record's `source_sha256`.
    pub source_sha256: String,
    /// The module record's `optimized`.
    pub optimized: bool,
}

/// One `noble-core-report/v1` line.
#[derive(Clone, Debug)]
pub struct Report {
    /// `schema`, if present.
    pub schema: Option<String>,
    /// `profile`, if present.
    pub profile: Option<String>,
    /// `backend`, if present.
    pub backend: Option<String>,
    /// `submission`, the session-local ordinal, if present.
    pub submission: Option<u64>,
    /// `outcome` (`normal`, `defined`, `reject`, `unsupported`, …).
    pub outcome: String,
    /// `stage` (`wasm`, `check`, `resolve`, …).
    pub stage: String,
    /// `diagnostic`, if any.
    pub diagnostic: Option<String>,
    /// Bottom-to-top stack. Every runtime report carries one; a static report
    /// has none, so it is empty there.
    pub stack: Vec<Value>,
    /// `request_trace` entries (host requests such as `test.emit:hi`).
    pub trace: Vec<String>,
    /// The runtime fields, for a `wasm`-stage report.
    pub runtime: Option<Runtime>,
    /// The raw report line.
    pub raw: String,
}

impl Report {
    /// Whether the submission ran normally (or installed a definition).
    #[must_use]
    pub fn ok(&self) -> bool {
        self.outcome == "normal" || self.outcome == "defined"
    }
}

/// The CLI's process status for one outcome (`noble-cli` `output::exit`).
#[must_use]
pub fn exit_status(outcome: &str) -> i32 {
    match outcome {
        "normal" | "defined" | "linked" | "ready" | "reported" | "source-observation-released" => 0,
        "unsupported" => 4,
        "exhausted" | "runtime-exhausted" => 5,
        "trap" => 1,
        _ => 2,
    }
}

/// Whether an outcome ends the session (`noble-cli` `Report::is_terminal`).
#[must_use]
pub fn terminal(outcome: &str) -> bool {
    matches!(outcome, "trap" | "runtime-exhausted" | "internal-failure")
}

/// Failure class: the report stream (schema, attribution, cardinality, malformed fields).
pub const PROTOCOL: &str = "protocol";

/// Failure class: an exit status the reports do not explain, or a signal.
pub const PROCESS: &str = "process";

/// Failure class: submissions left undelivered by a session that had not ended.
pub const TRANSPORT: &str = "transport";

/// A session that does not count as a complete, attributed observation.
#[derive(Clone, Debug)]
pub struct SessionFailure {
    /// [`PROTOCOL`], [`PROCESS`] or [`TRANSPORT`].
    pub class: &'static str,
    /// What failed.
    pub reason: String,
    /// Reports validated before the failure.
    pub validated: usize,
    /// The last [`MAX_FAILURE_REPORTS`] validated reports, in submission order.
    pub reports: Vec<Report>,
    /// The child's exit code, if it exited.
    pub exit_code: Option<i32>,
    /// The signal that ended the child, if any.
    pub signal: Option<i32>,
    /// The stdin error, if the submissions were not all delivered.
    pub write_error: Option<String>,
}

impl SessionFailure {
    /// A bounded receipt record: the class, the reason, the child's status and
    /// the retained outcomes. None of it counts as agreement.
    #[must_use]
    pub fn record(&self) -> serde_json::Value {
        serde_json::json!({
            "class": self.class,
            "reason": self.reason,
            "validated_reports": self.validated,
            "retained_outcomes": self
                .reports
                .iter()
                .map(|r| serde_json::json!({ "submission": r.submission, "stage": r.stage, "outcome": r.outcome }))
                .collect::<Vec<_>>(),
            "exit_code": self.exit_code,
            "signal": self.signal,
            "write_error": self.write_error,
        })
    }
}

/// Harness failure.
#[derive(Debug)]
pub enum Error {
    /// Spawning or talking to the CLI failed.
    Io(std::io::Error),
    /// A report line did not parse or a report is missing.
    Protocol(String),
    /// A submission did not run normally.
    Refused(Box<Report>),
    /// A session was incomplete, misattributed or ended unexplained.
    Session(Box<SessionFailure>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Protocol(m) => write!(f, "protocol: {m}"),
            Self::Refused(r) => write!(f, "refused: {}", r.raw),
            Self::Session(failure) => write!(
                f,
                "session {} failure: {} ({} reports validated, exit {:?}, signal {:?}, stdin {})",
                failure.class,
                failure.reason,
                failure.validated,
                failure.exit_code,
                failure.signal,
                failure.write_error.as_deref().unwrap_or("delivered"),
            ),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// What one submission must be answered by.
struct Expected {
    ordinal: u64,
    role: Role,
    source_sha256: String,
}

/// How the child ended.
struct Finish {
    exit_code: Option<i32>,
    signal: Option<i32>,
    write_error: Option<String>,
}

/// A pinned CLI and optimization mode.
#[derive(Clone, Debug)]
pub struct Cli {
    /// Absolute `noble` binary path.
    pub bin: PathBuf,
    /// `--opt` mode.
    pub opt: Opt,
}

impl Cli {
    /// A CLI handle.
    #[must_use]
    pub const fn new(bin: PathBuf, opt: Opt) -> Self {
        Self { bin, opt }
    }

    /// Runs one `noble session --framed` process over `submissions` and
    /// returns one validated report per submission. A session that ends at a
    /// terminal outcome (a trap or an exhausted execution limit) returns fewer.
    ///
    /// # Errors
    /// As [`Cli::attributed`].
    pub fn session(&self, submissions: &[String]) -> Result<Vec<Report>, Error> {
        Ok(self
            .session_timed(submissions)?
            .into_iter()
            .map(|(r, _)| r)
            .collect())
    }

    /// [`Cli::session`] with each report's arrival time measured from the
    /// process spawn.
    ///
    /// # Errors
    /// As [`Cli::attributed`].
    pub fn session_timed(&self, submissions: &[String]) -> Result<Vec<(Report, Duration)>, Error> {
        let roles = vec![Role::Any; submissions.len()];
        self.attributed(submissions, &roles)
    }

    /// Runs one session whose submissions have declared roles and returns one
    /// validated report per submission, with its arrival time from the process
    /// spawn. A session that ends at a terminal outcome returns fewer.
    ///
    /// # Errors
    /// [`Error::Session`] for an incomplete, misattributed or unexplained
    /// session; [`Error::Io`] when the CLI cannot be spawned or awaited;
    /// [`Error::Protocol`] when `roles` does not match `submissions` or a
    /// submission cannot be hashed.
    pub fn attributed(
        &self,
        submissions: &[String],
        roles: &[Role],
    ) -> Result<Vec<(Report, Duration)>, Error> {
        if roles.len() != submissions.len() {
            return Err(Error::Protocol(format!(
                "{} roles for {} submissions",
                roles.len(),
                submissions.len()
            )));
        }
        let expected = expectations(submissions, roles)?;
        let spawned = Instant::now();
        let mut child = Command::new(&self.bin)
            .args(["session", "--framed", "--opt", self.opt.flag()])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut input = Vec::new();
        for s in submissions {
            input.extend_from_slice(format!("{}\n", s.len()).as_bytes());
            input.extend_from_slice(s.as_bytes());
        }
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::Protocol("no stdin".into()))?;
        // The result is kept: an undelivered submission is legitimate only
        // after a terminal outcome ended the session.
        let writer = std::thread::spawn(move || stdin.write_all(&input).map_err(|e| e.to_string()));
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Protocol("no stdout".into()))?;
        let mut reader = BufReader::new(stdout);
        let mut reports: Vec<(Report, Duration)> = Vec::with_capacity(submissions.len());
        let mut violation = None;
        // One line per submission, plus one to notice an extra line.
        for _ in 0..=submissions.len() {
            let line = match read_line(&mut reader) {
                Ok(Some(line)) => line,
                Ok(None) => break,
                Err(reason) => {
                    violation = Some(reason);
                    break;
                }
            };
            let at = spawned.elapsed();
            match attribute(&line, &reports, &expected, self.opt) {
                Ok(report) => reports.push((report, at)),
                Err(reason) => {
                    violation = Some(reason);
                    break;
                }
            }
        }
        if let Some(reason) = violation.take() {
            // A child whose stream is already invalid is not left running.
            violation = Some(match child.kill() {
                Ok(()) => reason,
                Err(e) => format!("{reason}; stopping the child: {e}"),
            });
        }
        let status = child.wait()?;
        let written = writer
            .join()
            .map_err(|_| Error::Protocol("stdin writer panicked".into()))?;
        let finish = Finish {
            exit_code: status.code(),
            signal: status.signal(),
            write_error: written.err(),
        };
        complete(reports, violation, &finish, submissions.len())
    }

    /// Evaluates `root` on each input vector (bottom-first values) after
    /// committing `definitions`. `outputs` is the root's checked output
    /// stack length (the values a host clears before the next vector).
    /// Returns each root submission's report, normal or not.
    ///
    /// A Core session retains compiled modules and runtime allocations
    /// across submissions. When a later root submission hits an
    /// ordinary-path quota ([`quota`]: static `exhausted` or
    /// `runtime-exhausted`, BE-LIMIT-01 classes, not observations of the
    /// root) the vector is retried in a fresh session and the restart is
    /// counted; a quota outcome on the first vector of a fresh session is
    /// returned as that vector's report.
    ///
    /// # Errors
    /// A definition or input submission that is not accepted, a session that
    /// ends early, or a protocol/IO failure.
    pub fn run_vectors(
        &self,
        definitions: &[&str],
        root: &str,
        outputs: usize,
        vectors: &[Vec<Value>],
    ) -> Result<Run<Report>, Error> {
        let per_chunk = (MAX_SUBMISSIONS.saturating_sub(definitions.len()) / 2).max(1);
        let mut run = Run {
            results: Vec::with_capacity(vectors.len()),
            timing: Vec::with_capacity(vectors.len()),
            restarts: 0,
        };
        let mut at = 0;
        while at < vectors.len() {
            let chunk = &vectors[at..(at + per_chunk).min(vectors.len())];
            let mut subs: Vec<String> = definitions.iter().map(|d| (*d).to_owned()).collect();
            let mut roles = vec![Role::Definition; definitions.len()];
            for (i, v) in chunk.iter().enumerate() {
                subs.push(push_line(if i == 0 { 0 } else { outputs }, v));
                roles.push(Role::Input);
                subs.push(root.to_owned());
                roles.push(Role::Root);
            }
            let (reports, times) = split(self.attributed(&subs, &roles)?);
            for i in 0..chunk.len() {
                let r = root_report(&reports, definitions.len(), i)?;
                if !r.ok() && i > 0 && quota(r) {
                    run.restarts += 1;
                    break;
                }
                run.results.push(r.clone());
                run.timing.push(timing(&times, definitions.len(), i));
                at += 1;
                if !r.ok() {
                    // Later input lines no longer match the stack.
                    break;
                }
            }
        }
        Ok(run)
    }

    /// Runs a host-driven transition. The host installs `initial_state` on
    /// the stack; each step pushes the step's host inputs and submits
    /// `root`, whose `outputs` results are the next state (the first
    /// `initial_state.len()` values) followed by the step's observable
    /// outputs, which the host drops before the next step. Returns each
    /// step's full output stack (bottom-first). Chunks — and restarts after
    /// a session's own compile-budget exhaustion, as in
    /// [`Cli::run_vectors`] — start fresh sessions from the host-held state.
    ///
    /// # Errors
    /// A step that is not normal, a session that ends early, or a
    /// protocol/IO failure.
    pub fn run_transition(
        &self,
        definitions: &[&str],
        root: &str,
        outputs: usize,
        initial_state: &[Value],
        steps: &[Vec<Value>],
    ) -> Result<Run<Vec<Value>>, Error> {
        let per_chunk = (MAX_SUBMISSIONS.saturating_sub(definitions.len()) / 2).max(1);
        let observable = outputs.saturating_sub(initial_state.len());
        let mut state = initial_state.to_vec();
        let mut run = Run {
            results: Vec::with_capacity(steps.len()),
            timing: Vec::with_capacity(steps.len()),
            restarts: 0,
        };
        let mut at = 0;
        while at < steps.len() {
            let chunk = &steps[at..(at + per_chunk).min(steps.len())];
            let mut subs: Vec<String> = definitions.iter().map(|d| (*d).to_owned()).collect();
            let mut roles = vec![Role::Definition; definitions.len()];
            for (i, inputs) in chunk.iter().enumerate() {
                let line = if i == 0 {
                    format!("{} {}", literals(&state), literals(inputs))
                } else {
                    push_line(observable, inputs)
                };
                subs.push(line);
                roles.push(Role::Input);
                subs.push(root.to_owned());
                roles.push(Role::Root);
            }
            let (reports, times) = split(self.attributed(&subs, &roles)?);
            for i in 0..chunk.len() {
                let r = root_report(&reports, definitions.len(), i)?;
                if !r.ok() && i > 0 && quota(r) {
                    run.restarts += 1;
                    break;
                }
                if !r.ok() || r.stack.len() != outputs {
                    return Err(Error::Refused(Box::new(r.clone())));
                }
                state = r.stack[..initial_state.len()].to_vec();
                run.results.push(r.stack.clone());
                run.timing.push(timing(&times, definitions.len(), i));
                at += 1;
            }
        }
        Ok(run)
    }
}

/// The ordinal, role and source digest every submission must be answered by.
/// Each distinct text is hashed once, before the session starts, so hashing
/// never shifts report arrival times.
fn expectations(submissions: &[String], roles: &[Role]) -> Result<Vec<Expected>, Error> {
    let mut digests: HashMap<&str, String> = HashMap::new();
    let mut expected = Vec::with_capacity(submissions.len());
    for (index, (source, role)) in submissions.iter().zip(roles).enumerate() {
        let digest = if let Some(known) = digests.get(source.as_str()) {
            known.clone()
        } else {
            let fresh = crate::bench::sha256_text(source).map_err(Error::Protocol)?;
            digests.insert(source.as_str(), fresh.clone());
            fresh
        };
        let ordinal = u64::try_from(index)
            .map_err(|e| Error::Protocol(e.to_string()))?
            .saturating_add(1);
        expected.push(Expected {
            ordinal,
            role: *role,
            source_sha256: digest,
        });
    }
    Ok(expected)
}

/// One report line, at most [`MAX_REPORT_BYTES`] long, without its newline.
fn read_line(reader: &mut impl std::io::BufRead) -> Result<Option<String>, String> {
    let limit = u64::try_from(MAX_REPORT_BYTES)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut line = Vec::new();
    let read = reader
        .by_ref()
        .take(limit)
        .read_until(b'\n', &mut line)
        .map_err(|e| format!("reading reports: {e}"))?;
    if read == 0 {
        return Ok(None);
    }
    if line.pop() != Some(b'\n') {
        return Err(if read > MAX_REPORT_BYTES {
            format!("a report line exceeds {MAX_REPORT_BYTES} bytes")
        } else {
            "an unterminated report line ends the stream".to_owned()
        });
    }
    String::from_utf8(line)
        .map(Some)
        .map_err(|_| "a report line is not UTF-8".to_owned())
}

/// Parses the next report line and attributes it to its submission.
fn attribute(
    line: &str,
    earlier: &[(Report, Duration)],
    expected: &[Expected],
    opt: Opt,
) -> Result<Report, String> {
    if let Some((last, _)) = earlier.last() {
        if terminal(&last.outcome) {
            return Err(format!(
                "a report follows the terminal `{}` outcome: {}",
                last.outcome,
                excerpt(line)
            ));
        }
    }
    let expect = expected.get(earlier.len()).ok_or_else(|| {
        format!(
            "an extra report beyond {} submissions: {}",
            expected.len(),
            excerpt(line)
        )
    })?;
    let report = parse(line).map_err(|e| e.to_string())?;
    check(&report, expect, opt)?;
    Ok(report)
}

/// The protocol, identity and role obligations of one report.
fn check(report: &Report, expect: &Expected, opt: Opt) -> Result<(), String> {
    let ordinal = expect.ordinal;
    if report.schema.as_deref() != Some(SCHEMA) {
        return Err(format!(
            "submission {ordinal}: schema {:?} is not {SCHEMA}",
            report.schema
        ));
    }
    if report.profile.as_deref() != Some(PROFILE) {
        return Err(format!(
            "submission {ordinal}: profile {:?} is not {PROFILE}",
            report.profile
        ));
    }
    if report.backend.as_deref() != Some(BACKEND) {
        return Err(format!(
            "submission {ordinal}: backend {:?} is not {BACKEND}",
            report.backend
        ));
    }
    if report.submission != Some(ordinal) {
        return Err(format!(
            "submission {ordinal}: the report names submission {:?}",
            report.submission
        ));
    }
    if let Some(runtime) = &report.runtime {
        if runtime.module_submission != ordinal {
            return Err(format!(
                "submission {ordinal}: the module record names submission {}",
                runtime.module_submission
            ));
        }
        if runtime.source_sha256 != expect.source_sha256 {
            return Err(format!(
                "submission {ordinal}: the module record's source digest is not the submitted text's"
            ));
        }
        if runtime.optimized != opt.optimized() {
            return Err(format!(
                "submission {ordinal}: the module record's optimization does not match --opt {}",
                opt.flag()
            ));
        }
        if report.outcome == "normal"
            && (runtime.status != 0 || runtime.session_state != "retained")
        {
            return Err(format!(
                "submission {ordinal}: a normal report has status {} and session state {}",
                runtime.status, runtime.session_state
            ));
        }
    } else if report.outcome == "normal" {
        return Err(format!(
            "submission {ordinal}: a normal report has no runtime fields"
        ));
    }
    if expect.role == Role::Definition && report.outcome == "normal" {
        return Err(format!(
            "submission {ordinal}: a definition submission reported normal"
        ));
    }
    if (expect.role == Role::Input || expect.role == Role::Root) && report.outcome == "defined" {
        return Err(format!(
            "submission {ordinal}: an input or root submission reported defined"
        ));
    }
    Ok(())
}

/// Accepts a session only when it completed as its reports say, or fails
/// closed with what was validated and how the child ended.
fn complete(
    reports: Vec<(Report, Duration)>,
    violation: Option<String>,
    finish: &Finish,
    submitted: usize,
) -> Result<Vec<(Report, Duration)>, Error> {
    let ended_early = reports.last().is_some_and(|(r, _)| terminal(&r.outcome));
    if let Some(reason) = violation {
        return Err(failure(PROTOCOL, reason, &reports, finish));
    }
    if reports.len() < submitted && !ended_early {
        return Err(failure(
            PROTOCOL,
            format!(
                "{} reports for {submitted} submissions, and the last outcome is not terminal",
                reports.len()
            ),
            &reports,
            finish,
        ));
    }
    if let Some(signal) = finish.signal {
        return Err(failure(
            PROCESS,
            format!("the child was ended by signal {signal}"),
            &reports,
            finish,
        ));
    }
    let explained = reports
        .iter()
        .map(|(r, _)| exit_status(&r.outcome))
        .max()
        .unwrap_or(0);
    if finish.exit_code != Some(explained) {
        return Err(failure(
            PROCESS,
            format!(
                "exit status {:?} where the reported outcomes give {explained}",
                finish.exit_code
            ),
            &reports,
            finish,
        ));
    }
    if finish.write_error.is_some() && !ended_early {
        return Err(failure(
            TRANSPORT,
            "the submissions were not all delivered".to_owned(),
            &reports,
            finish,
        ));
    }
    Ok(reports)
}

fn failure(
    class: &'static str,
    reason: String,
    reports: &[(Report, Duration)],
    finish: &Finish,
) -> Error {
    let kept = reports.len().saturating_sub(MAX_FAILURE_REPORTS);
    Error::Session(Box::new(SessionFailure {
        class,
        reason,
        validated: reports.len(),
        reports: reports.iter().skip(kept).map(|(r, _)| r.clone()).collect(),
        exit_code: finish.exit_code,
        signal: finish.signal,
        write_error: finish.write_error.clone(),
    }))
}

/// The first [`EXCERPT_BYTES`] of a line, on a character boundary.
fn excerpt(line: &str) -> &str {
    let end = (0..=EXCERPT_BYTES.min(line.len()))
        .rev()
        .find(|&i| line.is_char_boundary(i))
        .unwrap_or(0);
    line.get(..end).unwrap_or_default()
}

/// Results of a multi-session run.
#[derive(Clone, Debug)]
pub struct Run<T> {
    /// One result per vector or step, in order.
    pub results: Vec<T>,
    /// One [`Timing`] per result.
    pub timing: Vec<Timing>,
    /// Fresh sessions started because accumulated session state hit an
    /// ordinary-path quota ([`quota`]), reported separately from
    /// observations.
    pub restarts: usize,
}

/// Wall-clock timing of one vector or step, from report arrival times.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// The input submission: previous report (process spawn for a session's
    /// first submission) to its report.
    pub push: Duration,
    /// The root submission: input report to root report. The ordinary path
    /// checks, compiles, instantiates and runs every submission, so this is
    /// its whole per-evaluation cost.
    pub root: Duration,
    /// Process spawn to the root report.
    pub since_spawn: Duration,
    /// Whether this is the first vector or step of its session process (its
    /// `push` includes process start-up and the definitions).
    pub first_in_session: bool,
}

fn split(timed: Vec<(Report, Duration)>) -> (Vec<Report>, Vec<Duration>) {
    timed.into_iter().unzip()
}

/// Timing of vector `vector` from arrival times; `root_report` has already
/// checked that both of its reports exist.
fn timing(times: &[Duration], definitions: usize, vector: usize) -> Timing {
    let push_at = definitions + 2 * vector;
    let before = push_at.checked_sub(1).map_or(Duration::ZERO, |p| times[p]);
    Timing {
        push: times[push_at] - before,
        root: times[push_at + 1] - times[push_at],
        since_spawn: times[push_at + 1],
        first_in_session: vector == 0,
    }
}

/// An ordinary-path quota outcome: a static `exhausted` refusal (the
/// session survives; nothing ran) or `runtime-exhausted` (the session ends).
#[must_use]
pub fn quota(r: &Report) -> bool {
    r.outcome == "exhausted" || r.outcome == "runtime-exhausted"
}

fn root_report(reports: &[Report], definitions: usize, vector: usize) -> Result<&Report, Error> {
    for d in &reports[..definitions.min(reports.len())] {
        if !d.ok() {
            return Err(Error::Refused(Box::new(d.clone())));
        }
    }
    let push = reports
        .get(definitions + 2 * vector)
        .ok_or_else(|| Error::Protocol(format!("missing input report {vector}")))?;
    if !push.ok() {
        return Err(Error::Refused(Box::new(push.clone())));
    }
    reports
        .get(definitions + 2 * vector + 1)
        .ok_or_else(|| Error::Protocol(format!("missing root report {vector}")))
}

fn literals(values: &[Value]) -> String {
    values
        .iter()
        .map(Value::literal)
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_line(drops: usize, values: &[Value]) -> String {
    format!("{}{}", "drop ".repeat(drops), literals(values))
}

/// Runs `noble live slot --engine v8 --authority AUTHORITY` over JSONL
/// `requests` and returns every report line. The CLI refuses file-backed
/// stdin, so (as its own tests do) `cat` holds the only writer of an
/// anonymous pipe.
///
/// # Errors
/// Spawn/IO failure or an unparsable report line.
pub fn live_slot(
    bin: &std::path::Path,
    authority: &std::path::Path,
    requests: &[String],
) -> Result<Vec<serde_json::Value>, Error> {
    let mut child = Command::new("/bin/sh")
        .args([
            "-c",
            "cat | exec \"$1\" live slot --engine v8 --authority \"$2\"",
            "sh",
        ])
        .arg(bin)
        .arg(authority)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut input = String::new();
    for r in requests {
        input.push_str(r);
        input.push('\n');
    }
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| Error::Protocol("no stdin".into()))?;
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let out = child.wait_with_output()?;
    writer
        .join()
        .map_err(|_| Error::Protocol("stdin writer panicked".into()))?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| serde_json::from_str(l).map_err(|e| Error::Protocol(format!("{e}: {l}"))))
        .collect()
}

/// Parses one report line. A runtime (`wasm`) report must carry its stack,
/// status, session state and module record; a missing stage or a malformed
/// runtime field is an error, never an empty default.
///
/// # Errors
/// A line that is not a `noble-core-report/v1` object of that shape.
pub fn parse(line: &str) -> Result<Report, Error> {
    let v: serde_json::Value = serde_json::from_str(line)
        .map_err(|e| Error::Protocol(format!("{e}: {}", excerpt(line))))?;
    let field = |k: &str| {
        v.get(k)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let outcome = field("outcome")
        .ok_or_else(|| Error::Protocol(format!("no outcome: {}", excerpt(line))))?;
    let stage =
        field("stage").ok_or_else(|| Error::Protocol(format!("no stage: {}", excerpt(line))))?;
    let stack = v
        .get("stack")
        .and_then(serde_json::Value::as_array)
        .map(|items| items.iter().map(value).collect::<Result<Vec<_>, _>>())
        .transpose()?
        .unwrap_or_default();
    let trace = v
        .get("request_trace")
        .and_then(serde_json::Value::as_array)
        .map(|t| {
            t.iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let runtime = if stage == "wasm" {
        Some(runtime(&v, line)?)
    } else {
        None
    };
    Ok(Report {
        schema: field("schema"),
        profile: field("profile"),
        backend: field("backend"),
        submission: v.get("submission").and_then(serde_json::Value::as_u64),
        outcome,
        stage,
        diagnostic: field("diagnostic"),
        stack,
        trace,
        runtime,
        raw: line.to_owned(),
    })
}

/// The runtime fields of a `wasm`-stage report, every one required.
fn runtime(v: &serde_json::Value, line: &str) -> Result<Runtime, Error> {
    let malformed = |name: &str| {
        Error::Protocol(format!(
            "a runtime report without a valid `{name}`: {}",
            excerpt(line)
        ))
    };
    if !v.get("stack").is_some_and(serde_json::Value::is_array) {
        return Err(malformed("stack"));
    }
    let status = v
        .get("status")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| malformed("status"))?;
    let session_state = v
        .get("session_state")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| malformed("session_state"))?
        .to_owned();
    let module = v
        .get("module")
        .filter(|m| m.is_object())
        .ok_or_else(|| malformed("module"))?;
    let module_submission = module
        .get("submission")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| malformed("module.submission"))?;
    let source_sha256 = module
        .get("source_sha256")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| malformed("module.source_sha256"))?
        .to_owned();
    let optimized = module
        .get("optimized")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| malformed("module.optimized"))?;
    Ok(Runtime {
        status,
        session_state,
        module_submission,
        source_sha256,
        optimized,
    })
}

fn value(item: &serde_json::Value) -> Result<Value, Error> {
    let ty = item
        .get("type")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("?");
    match ty {
        "I64" => item
            .get("value")
            .and_then(serde_json::Value::as_str)
            .and_then(|s| s.parse::<i64>().ok())
            .map(Value::I64)
            .ok_or_else(|| Error::Protocol(format!("bad I64 {item}"))),
        "Bool" => item
            .get("value")
            .and_then(serde_json::Value::as_bool)
            .map(Value::Bool)
            .ok_or_else(|| Error::Protocol(format!("bad Bool {item}"))),
        other => Ok(Value::Other(other.to_owned())),
    }
}
