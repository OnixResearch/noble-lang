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
//! Phase C reuse: W1 = [`Cli::run_vectors`] on I1, W2 =
//! [`Cli::run_transition`] on I4. The CLI writes one report line per
//! submission as it finishes, so each line's arrival time (from the session
//! process spawn) times that submission ([`Timing`]); the CLI reports no
//! timing itself.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Submissions per session process. A Core session retains each prepared
/// module; ~430 two-value submissions exhaust its allocation quota with the
/// pinned build, so chunks stay well below that.
pub const MAX_SUBMISSIONS: usize = 240;

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

/// One `noble-core-report/v1` line.
#[derive(Clone, Debug)]
pub struct Report {
    /// `outcome` (`normal`, `defined`, `reject`, `unsupported`, …).
    pub outcome: String,
    /// `stage` (`wasm`, `check`, `resolve`, …).
    pub stage: String,
    /// `diagnostic`, if any.
    pub diagnostic: Option<String>,
    /// Bottom-to-top stack (empty when absent).
    pub stack: Vec<Value>,
    /// `request_trace` entries (host requests such as `test.emit:hi`).
    pub trace: Vec<String>,
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

/// Harness failure.
#[derive(Debug)]
pub enum Error {
    /// Spawning or talking to the CLI failed.
    Io(std::io::Error),
    /// A report line did not parse or a report is missing.
    Protocol(String),
    /// A submission did not run normally.
    Refused(Box<Report>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Protocol(m) => write!(f, "protocol: {m}"),
            Self::Refused(r) => write!(f, "refused: {}", r.raw),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
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
    /// returns one report per submission. A session that ends early (a trap
    /// or an exhausted execution limit ends it) returns fewer reports.
    ///
    /// # Errors
    /// Spawn/IO failure or an unparsable report line.
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
    /// As [`Cli::session`].
    pub fn session_timed(&self, submissions: &[String]) -> Result<Vec<(Report, Duration)>, Error> {
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
        let writer = std::thread::spawn(move || {
            // A session that ends early closes its stdin; that is reported
            // by the missing reports, not as a write error.
            let _ = stdin.write_all(&input);
        });
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Protocol("no stdout".into()))?;
        let mut reports = Vec::with_capacity(submissions.len());
        for line in BufReader::new(stdout).lines() {
            let line = line?;
            let at = spawned.elapsed();
            reports.push((parse(&line)?, at));
        }
        child.wait()?;
        writer
            .join()
            .map_err(|_| Error::Protocol("stdin writer panicked".into()))?;
        Ok(reports)
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
            for (i, v) in chunk.iter().enumerate() {
                subs.push(push_line(if i == 0 { 0 } else { outputs }, v));
                subs.push(root.to_owned());
            }
            let (reports, times) = split(self.session_timed(&subs)?);
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
            for (i, inputs) in chunk.iter().enumerate() {
                let line = if i == 0 {
                    format!("{} {}", literals(&state), literals(inputs))
                } else {
                    push_line(observable, inputs)
                };
                subs.push(line);
                subs.push(root.to_owned());
            }
            let (reports, times) = split(self.session_timed(&subs)?);
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

/// Parses one report line.
///
/// # Errors
/// A line that is not a `noble-core-report/v1` object.
pub fn parse(line: &str) -> Result<Report, Error> {
    let v: serde_json::Value =
        serde_json::from_str(line).map_err(|e| Error::Protocol(format!("{e}: {line}")))?;
    let field = |k: &str| {
        v.get(k)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let outcome = field("outcome").ok_or_else(|| Error::Protocol(format!("no outcome: {line}")))?;
    let stage = field("stage").unwrap_or_default();
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
    Ok(Report {
        outcome,
        stage,
        diagnostic: field("diagnostic"),
        stack,
        trace,
        raw: line.to_owned(),
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
