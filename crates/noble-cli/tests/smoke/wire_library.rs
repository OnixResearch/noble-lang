//! `wire@1` through the production framed declared session, and the
//! session-cumulative preparation budget behind O11-4 and O11-5.
//!
//! Design-time observations O11-4/O11-5 claimed that an effectful `split` and
//! a long session's `split`/`fanout` are refused with
//! `independent source compilation refused` (`exhausted`). MEASURED: both
//! reproduce, and both come from one session-cumulative budget, not from a
//! type error and not from the shape of the program:
//!
//! - The published source compiler interns every *new* named specialization in
//!   one session-wide identity table capped at `DEFINITION_LIMIT` (256); a new
//!   identity beyond it returns `Exhausted` (`noble-wasm`
//!   `source/admission/identity/records.rs`). Its sibling environment-row cap
//!   is the same `DEFINITION_LIMIT` plus the fixed prefix.
//! - Each library word costs the specializations it instantiates at one
//!   concrete type: `project_first` 1; `first` and `second` 2 (each also
//!   instantiates `rot`); `split` 4 (`split`, `first`, `second`, `rot`);
//!   `fanout` 5. An effectful operand adds its own named use, so the O11-5
//!   `split` costs 5.
//! - With 254 identities consumed, `split` (4 more) is refused while
//!   `project_first` (1 more) is admitted and a program that instantiates
//!   nothing still runs. With 252 consumed, the effectful `split` (5 more) is
//!   refused: 257 > 256.
//!
//! The defaults are unchanged; this test pins the measured limit so no budget
//! can move silently.

use std::io::{BufRead, Write};

const WIRE: &[u8] = include_bytes!("../../../noble-contracts/src/fixtures/wire.noble");
const FX: &[u8] =
    b"module fx@1 [ require emit Text -- ! test.emit def say [ \"L\" emit ] export say ]";
const BINDING: &str = "bind fx@1 test.emit emit-a Text -- ! test.emit allow\n";
const SPLIT: &[u8] = b"10 20 pair [ 1 + ] [ 2 * ] wire@1.split unpair";
const EFFECT_SPLIT: &[u8] = b"\"L\" \"R\" pair [ fx@1.say ] [ fx@1.say ] wire@1.split unpair";
const PROJECT: &[u8] = b"10 20 pair wire@1.project_first";
const LITERAL: &[u8] = b"5 [ 1 + ] run";

struct DeclaredSession {
    child: std::process::Child,
    reports: std::io::BufReader<std::process::ChildStdout>,
}

impl DeclaredSession {
    /// A framed declared session bound to the scripted `fx@1` emitter.
    fn start() -> Result<Self, Box<dyn std::error::Error>> {
        let bindings =
            std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("wire-library-bindings.txt");
        std::fs::write(&bindings, BINDING)?;
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
            .args(["session", "--framed", "--declared-modules", "--bindings"])
            .arg(&bindings)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().ok_or("missing session stdout")?;
        Ok(Self {
            child,
            reports: std::io::BufReader::new(stdout),
        })
    }

    fn submit(&mut self, source: &[u8]) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let stdin = self.child.stdin.as_mut().ok_or("missing session stdin")?;
        writeln!(stdin, "{}", source.len())?;
        stdin.write_all(source)?;
        stdin.flush()?;
        let mut line = String::new();
        if self.reports.read_line(&mut line)? == 0 {
            return Err("session ended before its report".into());
        }
        Ok(serde_json::from_str(&line)?)
    }

    fn link(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for source in [WIRE, FX] {
            let report = self.submit(source)?;
            assert_eq!(report["outcome"], "linked", "{report}");
        }
        Ok(())
    }

    /// Consume `count` session-cumulative named identities, one definition
    /// and one use each.
    fn consume_identities(&mut self, count: usize) -> Result<(), Box<dyn std::error::Error>> {
        for index in 0..count {
            let definition = format!("def d{index} [ {index} ]");
            let report = self.submit(definition.as_bytes())?;
            assert_eq!(report["outcome"], "defined", "definition {index}: {report}");
            let used = format!("d{index} drop");
            let report = self.submit(used.as_bytes())?;
            assert_eq!(report["outcome"], "normal", "use {index}: {report}");
        }
        Ok(())
    }

    fn finish(mut self) -> Result<(), Box<dyn std::error::Error>> {
        drop(self.child.stdin.take());
        self.child.wait()?;
        Ok(())
    }
}

#[test]
fn wire_library_runs_in_a_fresh_declared_session() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = DeclaredSession::start()?;
    session.link()?;
    let split = session.submit(SPLIT)?;
    assert_eq!(split["outcome"], "normal", "{split}");
    // O11-5's shape: both operands are effectful and the trace records both
    // host requests in order, so an effectful `split` does execute.
    let effectful = session.submit(EFFECT_SPLIT)?;
    assert_eq!(effectful["outcome"], "normal", "{effectful}");
    let trace = effectful["request_trace"]
        .as_array()
        .ok_or("missing trace")?;
    assert_eq!(trace.len(), 2, "both operands emit: {effectful}");
    for request in trace {
        assert_eq!(request["effect"], "test.emit");
        assert_eq!(request["decision"], "allow");
    }
    // The single effect slot is the documented library limit: operands with
    // different bounds do not unify, and that is a type refusal, never an
    // exhausted preparation budget.
    let mismatched = session.submit(b"\"L\" [ fx@1.say ] [ 1 + ] wire@1.fanout")?;
    assert_eq!(mismatched["outcome"], "type-reject", "{mismatched}");
    session.finish()
}

#[test]
fn wire_split_refusal_is_the_session_cumulative_budget() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = DeclaredSession::start()?;
    session.link()?;
    // 254 of the 256 session identities are consumed before the probes.
    session.consume_identities(254)?;
    let refused = session.submit(SPLIT)?;
    assert_eq!(refused["outcome"], "exhausted", "{refused}");
    assert_eq!(
        refused["diagnostic"], "independent source compilation refused",
        "the refusal is the preparation budget, not a type or shape error: {refused}"
    );
    // O11-5 under the same pressure: `split` plus its two operands' named uses
    // needs five identities, so this is refused for the same reason.
    let effectful = session.submit(EFFECT_SPLIT)?;
    assert_eq!(effectful["outcome"], "exhausted", "{effectful}");
    assert_eq!(
        effectful["stage"], "acceptance",
        "the refusal is the preparation stage: {effectful}"
    );
    assert_eq!(effectful["guest_requests"], 0, "no guest ran: {effectful}");
    assert_eq!(effectful["host_requests"], 0, "no effect ran: {effectful}");
    // One new identity still fits (255 of 256) and a program that instantiates
    // no named definition needs none, so the refusal is about new
    // specializations and not about the session being unusable.
    let projected = session.submit(PROJECT)?;
    assert_eq!(projected["outcome"], "normal", "{projected}");
    let literal = session.submit(LITERAL)?;
    assert_eq!(literal["outcome"], "normal", "{literal}");
    session.finish()
}
