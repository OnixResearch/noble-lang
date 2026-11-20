//! Signed-Declared-Text-v1 source-epoch retirement and observation release
//! through the production framed session.
//!
//! The change's L10 prose claimed 65 same-session signed folds with checked
//! post-report retirement, a pinned Program denying the 14th fold at the
//! unchanged 4,096-cell cap, and forged/early/stale/foreign observation-release
//! denials with no in-repository receipt. These tests pin the behaviour the
//! binary actually exhibits; every number below is measured, not copied.

use std::io::{BufRead, Write};

const RESULT: &[u8] = include_bytes!("../../../noble-contracts/src/fixtures/result.noble");
const DIGIT: &[u8] =
    include_bytes!("../../../../verification/calculator-precursor/decimal-digit.noble");
const FOLD: &[u8] =
    include_bytes!("../../../../verification/calculator-precursor/decimal-fold.noble");
/// The framed session already selects `--framed --declared-modules
/// --text-byte-cursor`.
const FLAGS: [&str; 2] = ["--signed-self-recursion", "--retained-source-bytes"];
/// A three-digit fold allocates and releases exactly this many cells.
const FOLD_CELLS: u64 = 306;
const FOLD_LINE: &[u8] = b"\"123\" 0 0 decimal_fold@1.fold drop";

struct SignedSession {
    child: std::process::Child,
    reports: std::io::BufReader<std::process::ChildStdout>,
}

impl SignedSession {
    fn start(opt: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
            .args([
                "session",
                "--framed",
                "--declared-modules",
                "--text-byte-cursor",
            ])
            .args(flags(opt))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        let stdout = child.stdout.take().ok_or("missing session stdout")?;
        Ok(Self {
            child,
            reports: std::io::BufReader::new(stdout),
        })
    }

    fn report(&mut self) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let mut line = String::new();
        if self.reports.read_line(&mut line)? == 0 {
            return Err("session ended before its report".into());
        }
        Ok(serde_json::from_str(&line)?)
    }

    fn submit(&mut self, source: &[u8]) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let stdin = self.child.stdin.as_mut().ok_or("missing session stdin")?;
        writeln!(stdin, "{}", source.len())?;
        stdin.write_all(source)?;
        stdin.flush()?;
        self.report()
    }

    fn control(&mut self, line: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let stdin = self.child.stdin.as_mut().ok_or("missing session stdin")?;
        stdin.write_all(line.as_bytes())?;
        stdin.flush()?;
        self.report()
    }

    fn finish(mut self) -> Result<std::process::ExitStatus, Box<dyn std::error::Error>> {
        drop(self.child.stdin.take());
        Ok(self.child.wait()?)
    }
}

fn link_modules(session: &mut SignedSession) -> Result<(), Box<dyn std::error::Error>> {
    for source in [RESULT, DIGIT, FOLD] {
        let report = session.submit(source)?;
        assert_eq!(report["outcome"], "linked");
        assert_eq!(report["profile"], "Signed-Declared-Text-v1");
    }
    Ok(())
}

fn observe_first_program(
    session: &mut SignedSession,
) -> Result<(String, serde_json::Value), Box<dyn std::error::Error>> {
    let report = session.submit(b"[ 1 ]")?;
    assert_eq!(report["outcome"], "normal");
    assert_eq!(report["stack"][0]["type"], "Program");
    let store = report["source_store_id"]
        .as_str()
        .ok_or("missing source store")?
        .to_owned();
    Ok((store, report))
}

fn release_header(store: &str, id: u64) -> String {
    format!(":release-observed-roots {store} {id}\n")
}

// r[verify K-DEF-03]
/// One licensed epoch per reported fold: 65 folds in a single session retire
/// their checked code and every Program/cell they allocated, keep Core 0..3,
/// and never advance the 4,096-cell session maximum.
#[test]
fn signed_folds_retire_checked_code_across_sixty_five_reports(
) -> Result<(), Box<dyn std::error::Error>> {
    for opt in ["off", "on"] {
        let mut sources: Vec<&[u8]> = vec![RESULT, DIGIT, FOLD];
        sources.extend(std::iter::repeat_n(FOLD_LINE, 65));
        let (status, reports) = super::framed_declared_reports(&flags(opt), &sources)?;
        assert!(status.success(), "{opt}: 65 folds must end normally");
        assert_eq!(reports.len(), 68);
        let store = reports[3]["source_epoch_retirement"]["source_store_id"]
            .as_str()
            .ok_or("missing epoch store")?
            .to_owned();
        let confirm = |report: &serde_json::Value, tick: u64| {
            let retirement = &report["source_epoch_retirement"];
            assert_eq!(retirement["schema"], "noble-source-epoch-retirement/v1");
            assert_eq!(retirement["outcome"], "source-epoch-retired");
            assert_eq!(retirement["released_cells"], FOLD_CELLS);
            assert_eq!(retirement["released_bytes_before"], 0);
            assert_eq!(retirement["released_bytes_after"], FOLD_CELLS * 48);
            assert_eq!(retirement["heap_cells_before"], FOLD_CELLS);
            assert_eq!(retirement["heap_cells_after"], 0);
            assert_eq!(retirement["retained_core_entries"], 4);
            assert_eq!(retirement["cleared_code_entries"], 144);
            assert_eq!(retirement["code_end"], 148);
            // The compiler generation advances once per prepared fold.
            assert_eq!(retirement["generation"], tick + 1);
            assert_eq!(retirement["source_epoch_before"], tick);
            assert_eq!(retirement["source_epoch_after"], tick + 1);
        };
        for (index, report) in reports[3..].iter().enumerate() {
            assert_eq!(report["outcome"], "normal", "{opt}/{index}");
            assert_eq!(report["stack"], serde_json::json!([]));
            assert_eq!(
                report["source_epoch_retirement"]["source_store_id"],
                serde_json::Value::String(store.clone())
            );
            assert_eq!(report["source_epoch_retirement"]["released_programs"], 33);
            confirm(report, u64::try_from(index)?);
        }
    }
    Ok(())
}

fn flags(opt: &str) -> [&str; 5] {
    let [recursion, retained] = FLAGS;
    [recursion, retained, "262144", "--opt", opt]
}

// r[verify K-DEF-03]
/// A reported Program pins its whole reachable group: the checked `drop`
/// empties the stack but the epoch stays owned, 13 folds keep piling cells on
/// the same heap, and the 14th fold is refused as an allocation quota at the
/// unchanged 4,096-cell cap instead of recycling the pinned arena.
#[test]
fn pinned_program_observation_denies_the_fourteenth_fold_at_the_cell_cap(
) -> Result<(), Box<dyn std::error::Error>> {
    for opt in ["off", "on"] {
        let mut sources: Vec<&[u8]> = vec![RESULT, DIGIT, FOLD, b"[ 1 ]", b"drop"];
        sources.extend(std::iter::repeat_n(FOLD_LINE, 14));
        let (status, reports) = super::framed_declared_reports(&flags(opt), &sources)?;
        assert_eq!(status.code(), Some(5), "{opt}: the 14th fold is terminal");
        assert_eq!(reports.len(), 19);
        assert_eq!(reports[3]["stack"][0]["type"], "Program");
        for report in &reports[3..18] {
            assert!(
                report.get("source_epoch_retirement").is_none(),
                "{opt}: a pinned observation must not license retirement"
            );
        }
        assert_eq!(reports[4]["stack"], serde_json::json!([]));
        for report in &reports[5..18] {
            assert_eq!(report["outcome"], "normal");
        }
        let refused = &reports[18];
        assert_eq!(refused["outcome"], "runtime-exhausted");
        assert_eq!(refused["quota_reason"], "allocation");
        // Six cells for the pinned Program and its drop, then 13 folds of 306.
        assert_eq!(refused["metrics"]["retained_before_begin_bytes"], 191_232);
        assert_eq!(refused["metrics"]["allocated_total_bytes"], 5_376);
        assert_eq!((191_232 / 48) + (5_376 / 48), 4_096);
    }
    Ok(())
}

fn assert_refused(report: &serde_json::Value, label: &str) {
    assert_eq!(report["outcome"], "source-observation-refused", "{label}");
    assert_eq!(report["session_state"], "retained", "{label}");
    assert_eq!(report["guest_requests"], 0, "{label}");
}

fn assert_released(report: &serde_json::Value, id: u64, store: &str) {
    assert_eq!(report["outcome"], "source-observation-released");
    assert_eq!(report["source_observation_id"], id);
    assert_eq!(report["source_store_id"], store);
    assert_eq!(report["released_handles"], 1);
    assert_eq!(report["remaining_handles"], 0);
}

// r[verify DX-MODULE-04]
/// Only the reporting session, after a checked idle empty stack, can
/// relinquish an exact observed aggregate. An early release while the stack
/// still owns it, a forged store, a foreign session's store, a repeated or
/// stale identity, and an unknown identity all refuse; the exact one releases and
/// then licenses the epoch retirement.
#[test]
fn release_observed_roots_refuses_early_forged_foreign_repeated_and_stale_owners(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut session = SignedSession::start("off")?;
    link_modules(&mut session)?;
    let (store, report) = observe_first_program(&mut session)?;
    let id = report["source_observation_id"]
        .as_u64()
        .ok_or("missing observation id")?;
    let early = session.control(&release_header(&store, id))?;
    assert_refused(&early, "early");
    assert_eq!(
        early["diagnostic"],
        "checked source stack still owns observed values"
    );
    // The Program is still on the stack; one checked `drop` empties it and the
    // report observes no new group, so the pinned identity stays exactly one.
    let kept = session.submit(b"drop")?;
    assert_eq!(kept["outcome"], "normal");
    assert_eq!(kept["stack"], serde_json::json!([]));
    let forged = session.control(&release_header("00000000-0000-4000-8000-000000000000", id))?;
    assert_refused(&forged, "forged store");
    let wrong = session.control(&release_header(&store, id + 100))?;
    assert_refused(&wrong, "unknown identity");
    let released = session.control(&release_header(&store, id))?;
    assert_released(&released, id, &store);
    assert_eq!(
        released["source_epoch_retirement"]["outcome"],
        "source-epoch-retired"
    );
    assert_eq!(released["source_epoch_retirement"]["heap_cells_after"], 0);
    let repeated = session.control(&release_header(&store, id))?;
    assert_refused(&repeated, "repeated");
    let (next, next_report) = observe_first_program(&mut session)?;
    assert_eq!(next, store);
    let next_id = next_report["source_observation_id"]
        .as_u64()
        .ok_or("missing second observation id")?;
    assert_eq!(session.submit(b"drop")?["stack"], serde_json::json!([]));
    let stale = session.control(&release_header(&store, id))?;
    assert_refused(&stale, "stale after the retired epoch");
    let uppercase = store.to_uppercase();
    let normalized = session.control(&release_header(&uppercase, next_id))?;
    assert_released(&normalized, next_id, &store);
    // Refusals are visible failures: the session exit keeps their code 2.
    assert_eq!(session.finish()?.code(), Some(2));
    Ok(())
}

// r[verify DX-MODULE-04]
/// A second session's own store identity is foreign to the first: the release
/// refuses and the first session keeps working. Malformed control headers are
/// parse-stage refusals, not source submissions or guest work.
#[test]
fn release_observed_roots_rejects_a_foreign_session_store_and_malformed_headers(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut first = SignedSession::start("on")?;
    link_modules(&mut first)?;
    let (store, report) = observe_first_program(&mut first)?;
    let id = report["source_observation_id"]
        .as_u64()
        .ok_or("missing observation id")?;
    let mut second = SignedSession::start("on")?;
    let (foreign, _) = observe_first_program(&mut second)?;
    second.finish()?;
    assert_ne!(foreign, store);
    assert_refused(&first.control(&release_header(&foreign, id))?, "foreign");
    assert_eq!(first.submit(b"drop")?["stack"], serde_json::json!([]));
    assert_released(&first.control(&release_header(&store, id))?, id, &store);
    // The foreign refusal above keeps the visible process status at 2.
    assert_eq!(first.finish()?.code(), Some(2));
    for header in [
        format!(":release-observed-roots {store} 0\n"),
        format!(":release-observed-roots {store} {id} extra\n"),
        format!(":release-observed-roots {store} 9007199254740992\n"),
        String::from(":release-observed-roots not-a-store 1\n"),
    ] {
        let mut session = SignedSession::start("off")?;
        link_modules(&mut session)?;
        let malformed = session.control(&header)?;
        assert_eq!(malformed["stage"], "parse");
        assert_eq!(malformed["outcome"], "invalid-input");
        assert_eq!(
            malformed["diagnostic"],
            "invalid signed source control header"
        );
        assert_eq!(session.finish()?.code(), Some(2));
    }
    Ok(())
}
