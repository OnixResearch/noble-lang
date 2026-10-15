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
//! [`Cli::run_transition`] on I4.

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::PathBuf;
use std::process::{Command, Stdio};

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
            reports.push(parse(&line?)?);
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
    /// # Errors
    /// A definition or input submission that is not accepted, a session that
    /// ends early, or a protocol/IO failure.
    pub fn run_vectors(
        &self,
        definitions: &[&str],
        root: &str,
        outputs: usize,
        vectors: &[Vec<Value>],
    ) -> Result<Vec<Report>, Error> {
        let per_chunk = (MAX_SUBMISSIONS.saturating_sub(definitions.len()) / 2).max(1);
        let mut out = Vec::with_capacity(vectors.len());
        for chunk in vectors.chunks(per_chunk) {
            let mut subs: Vec<String> = definitions.iter().map(|d| (*d).to_owned()).collect();
            for (i, v) in chunk.iter().enumerate() {
                subs.push(push_line(if i == 0 { 0 } else { outputs }, v));
                subs.push(root.to_owned());
            }
            let reports = self.session(&subs)?;
            for i in 0..chunk.len() {
                out.push(root_report(&reports, definitions.len(), i)?.clone());
            }
        }
        Ok(out)
    }

    /// Runs a host-driven transition. The host installs `initial_state` on
    /// the stack; each step pushes the step's host inputs and submits
    /// `root`, whose `outputs` results are the next state (the first
    /// `initial_state.len()` values) followed by the step's observable
    /// outputs, which the host drops before the next step. Returns each
    /// step's full output stack (bottom-first). Chunks start fresh sessions
    /// from the host-held state.
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
    ) -> Result<Vec<Vec<Value>>, Error> {
        let per_chunk = (MAX_SUBMISSIONS.saturating_sub(definitions.len()) / 2).max(1);
        let observable = outputs.saturating_sub(initial_state.len());
        let mut state = initial_state.to_vec();
        let mut out = Vec::with_capacity(steps.len());
        for chunk in steps.chunks(per_chunk) {
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
            let reports = self.session(&subs)?;
            for i in 0..chunk.len() {
                let r = root_report(&reports, definitions.len(), i)?;
                if !r.ok() || r.stack.len() != outputs {
                    return Err(Error::Refused(Box::new(r.clone())));
                }
                state = r.stack[..initial_state.len()].to_vec();
                out.push(r.stack.clone());
            }
        }
        Ok(out)
    }
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
