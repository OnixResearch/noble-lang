//! Real-process checks for the opt-in live REPL. These are finite runtime cases,
//! not a source-bound milestone receipt or a refinement proof.

use std::fs::{self, DirBuilder, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{symlink, DirBuilderExt};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::Value;

const DEADLINE: Duration = Duration::from_secs(20);
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    directory: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new(initial: &str) -> Result<Self, Box<dyn std::error::Error>> {
        for _ in 0..32 {
            let suffix = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "noble-live-repl-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_nanos(),
                suffix,
            ));
            match DirBuilder::new().mode(0o700).create(&directory) {
                Ok(()) => {
                    let source = directory.join("math.noble");
                    fs::write(&source, initial)?;
                    return Ok(Self { directory, source });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err("could not allocate private live test directory".into())
    }

    fn replace(&self, replacement: &str) -> Result<(), Box<dyn std::error::Error>> {
        let temporary = self.directory.join("math.noble.tmp");
        let mut file = File::create_new(&temporary)?;
        file.write_all(replacement.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fs::rename(temporary, &self.source)?;
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

struct LiveChild {
    child: Child,
    stdin: Option<ChildStdin>,
    reports: Receiver<Result<String, String>>,
}

impl LiveChild {
    fn start(source: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        Self::start_with_options(source, None)
    }

    fn start_verified(source: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noble"));
        command
            .args(["live", "repl", "--source"])
            .arg(source)
            .args(["--engine", "v8", "--verify-source-before-execute"]);
        Self::spawn(command)
    }

    #[cfg(target_os = "linux")]
    fn start_watch(source: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        Self::start_watch_with_limit(source, None)
    }

    #[cfg(target_os = "linux")]
    fn start_watch_with_limit(
        source: &Path,
        retained_limit: Option<u64>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noble"));
        command.args(["live", "watch", "--source"]).arg(source).args(["--engine", "v8"]);
        if let Some(limit) = retained_limit {
            command.args(["--max-retained-generations", &limit.to_string()]);
        }
        Self::spawn(command)
    }

    #[cfg(target_os = "linux")]
    fn start_watch_declared(
        source: &Path,
        bindings: &Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noble"));
        command.args(["live", "watch", "--source"]).arg(source)
            .args(["--engine", "v8", "--declared-modules", "--bindings"])
            .arg(bindings);
        Self::spawn(command)
    }

    fn start_declared(
        source: &Path,
        bindings: &Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noble"));
        command
            .args(["live", "repl", "--source"])
            .arg(source)
            .args(["--engine", "v8", "--declared-modules", "--bindings"])
            .arg(bindings)
            .arg("--verify-source-before-execute");
        Self::spawn(command)
    }

    fn start_with_grant(
        source: &Path,
        name: &str,
        expected_generation: u64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::start_with_options(source, Some((name, expected_generation)))
    }

    fn start_with_options(
        source: &Path,
        grant: Option<(&str, u64)>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noble"));
        command
            .args(["live", "repl", "--source"])
            .arg(source)
            .args(["--engine", "v8"]);
        if let Some((name, generation)) = grant {
            command.args([
                "--self-edit",
                name,
                "--expect-generation",
                &generation.to_string(),
            ]);
        }
        Self::spawn(command)
    }

    fn start_with_guest_step_budget(
        source: &Path,
        name: &str,
        steps: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut command = Command::new(env!("CARGO_BIN_EXE_noble"));
        command.args(["live", "repl", "--source"]).arg(source)
            .args(["--engine", "v8", "--self-edit", name, "--expect-generation", "1",
                "--max-guest-steps", &steps.to_string()]);
        Self::spawn(command)
    }

    fn spawn(mut command: Command) -> Result<Self, Box<dyn std::error::Error>> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdin = child.stdin.take().ok_or("missing live stdin")?;
        let stdout = child.stdout.take().ok_or("missing live stdout")?;
        let (sender, reports) = mpsc::channel();
        std::thread::spawn(move || {
            let mut output = BufReader::new(stdout);
            loop {
                let mut bytes = Vec::new();
                match (&mut output).take(65_537).read_until(b'\n', &mut bytes) {
                    Ok(0) => {
                        let _ = sender.send(Err("live child closed stdout".into()));
                        break;
                    }
                    Ok(_) => {
                        if bytes.len() > 65_536 || !bytes.ends_with(b"\n") {
                            let _ = sender.send(Err(
                                "live report exceeds 65536 bytes or is unterminated".into(),
                            ));
                            break;
                        }
                        let line = String::from_utf8(bytes)
                            .map_err(|error| format!("live report is not UTF-8: {error}"));
                        if sender.send(line).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(format!("live stdout read failed: {error}")));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            stdin: Some(stdin),
            reports,
        })
    }

    fn report(&self) -> Result<Value, Box<dyn std::error::Error>> {
        let line = self.reports.recv_timeout(DEADLINE)??;
        assert!(
            line.ends_with('\n'),
            "incomplete JSON-line report: {line:?}"
        );
        Ok(serde_json::from_str(&line)?)
    }

    fn submit(&mut self, command: &str) -> Result<Value, Box<dyn std::error::Error>> {
        let stdin = self.stdin.as_mut().ok_or("closed live stdin")?;
        stdin.write_all(command.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        self.report()
    }

    fn finish(mut self) -> Result<(), Box<dyn std::error::Error>> {
        drop(self.stdin.take());
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Some(status) = self.child.try_wait()? {
                assert!(status.success(), "live child failed on clean EOF: {status}");
                return Ok(());
            }
            assert!(Instant::now() < deadline, "live child did not exit on EOF");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for LiveChild {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn live_ack(report: &Value, expected_stack: &[&str]) -> (u64, String) {
    assert_eq!(text_field(report, "schema"), "noble-live-report/v1");
    assert_eq!(text_field(report, "stage"), "wasm-live", "{report}");
    assert_eq!(text_field(report, "outcome"), "reload-committed");
    assert_eq!(number_field(report, "guest_requests"), 0);
    assert_eq!(number_field(report, "protected_operations"), 0);
    assert_stack(report, expected_stack);
    let generation = number_field(report, "generation");
    assert!(
        generation > 0,
        "committed generation must be positive: {report}"
    );
    let digest = text_field(report, "source_sha256");
    assert_eq!(digest.len(), 64, "source digest must be SHA-256: {report}");
    assert!(
        digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{report}"
    );
    (generation, digest.to_owned())
}

fn assert_generation_inspection(report: &Value, generation: u64, stack: &Value) {
    assert_eq!(text_field(report, "schema"), "noble-live-report/v1");
    assert_eq!(text_field(report, "stage"), "inspection");
    assert_eq!(text_field(report, "outcome"), "generation-observed");
    assert_eq!(text_field(report, "engine"), "v8");
    assert_eq!(number_field(report, "generation"), generation);
    assert_eq!(&report["stack"], stack);
    assert_eq!(number_field(report, "guest_requests"), 0);
    assert_eq!(number_field(report, "protected_operations"), 0);
}

fn number_field(report: &Value, key: &str) -> u64 {
    report
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing unsigned {key}: {report}"))
}

#[cfg(all(target_os = "linux", feature = "live-test-barrier"))]
struct LiveBarrier {
    directory: PathBuf,
    point: &'static str,
    expected: String,
    reached: Receiver<Result<String, String>>,
    reached_channel: File,
    release_channel: File,
    reader: Option<std::thread::JoinHandle<()>>,
}

#[cfg(all(target_os = "linux", feature = "live-test-barrier"))]
impl LiveBarrier {
    fn new(
        directory: &Path,
        point: &'static str,
        expected: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let parent = File::open(directory)?;
        for name in ["reached", "release"] {
            rustix::fs::mkfifoat(
                &parent, name, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            )?;
        }
        // Linux FIFO O_RDWR opens without a rendezvous race. Both ends stay
        // owned by this test until a real guest sends its exact event.
        let reached_channel = std::fs::OpenOptions::new().read(true).write(true)
            .open(directory.join("reached"))?;
        let release_channel = std::fs::OpenOptions::new().read(true).write(true)
            .open(directory.join("release"))?;
        let reader_file = reached_channel.try_clone()?;
        let (sender, reached) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let mut event = String::new();
            let result = BufReader::new(reader_file).read_line(&mut event)
                .map(|_| event).map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
        Ok(Self {
            directory: directory.to_path_buf(), point, expected: expected.to_owned(),
            reached, reached_channel, release_channel, reader: Some(reader),
        })
    }

    fn configure(&self, command: &mut Command) {
        command.env("NOBLE_LIVE_TEST_BARRIER_DIR", &self.directory)
            .env("NOBLE_LIVE_TEST_BARRIER_POINT", self.point)
            .env("NOBLE_LIVE_TEST_BARRIER_EXPECTED", &self.expected);
    }

    fn wait(&self) -> Result<Value, Box<dyn std::error::Error>> {
        let line = self.reached.recv_timeout(DEADLINE)??;
        let event: Value = serde_json::from_str(&line)?;
        assert_eq!(text_field(&event, "point"), self.point);
        assert_eq!(text_field(&event, "matched"), self.expected);
        Ok(event)
    }

    fn release(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.release_channel.write_all(b"R")?;
        self.release_channel.flush()?;
        Ok(())
    }
}

#[cfg(all(target_os = "linux", feature = "live-test-barrier"))]
impl Drop for LiveBarrier {
    fn drop(&mut self) {
        // A failing assertion may precede guest entry. Wake the bounded
        // reader so the suite never leaves a private test thread blocked.
        let _ = (&self.reached_channel).write_all(b"{}\n");
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn text_field<'a>(report: &'a Value, key: &str) -> &'a str {
    report
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing text {key}: {report}"))
}

fn assert_stack(report: &Value, values: &[&str]) {
    let stack = report
        .get("stack")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing stack array: {report}"));
    assert_eq!(stack.len(), values.len(), "wrong ordered stack: {report}");
    for (item, value) in stack.iter().zip(values) {
        assert_eq!(
            item.get("type").and_then(Value::as_str),
            Some("I64"),
            "{report}"
        );
        assert_eq!(
            item.get("value").and_then(Value::as_str),
            Some(*value),
            "{report}"
        );
    }
}

fn normal(report: &Value, stack: &[&str]) {
    assert_eq!(text_field(report, "outcome"), "normal", "{report}");
    assert_stack(report, stack);
}

fn refused_reload(report: &Value, stack: &[&str]) -> u64 {
    assert_eq!(text_field(report, "schema"), "noble-live-report/v1");
    assert_eq!(text_field(report, "stage"), "admission");
    assert_eq!(text_field(report, "outcome"), "reload-refused");
    assert!(!text_field(report, "diagnostic").is_empty());
    assert_eq!(number_field(report, "guest_requests"), 0);
    assert_eq!(number_field(report, "protected_operations"), 0);
    assert_stack(report, stack);
    number_field(report, "generation")
}

fn assert_no_candidate_span(report: &Value) {
    assert!(
        report.get("source_span_basis").is_none() && report.get("source_span").is_none(),
        "refusal without a selected-source diagnostic claimed candidate provenance: {report}"
    );
}

fn current_generation(
    live: &mut LiveChild,
    fixture: &Fixture,
    stack: &[&str],
) -> Result<u64, Box<dyn std::error::Error>> {
    // A different selected path must fail before reading any candidate bytes.
    let other = fixture.directory.join("unselected.noble");
    let report = live.submit(&format!(":reload {}", other.display()))?;
    assert_no_candidate_span(&report);
    Ok(refused_reload(&report, stack))
}

#[test]
fn live_01_reload_ack_precedes_fresh_next_top_level_call() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    let initial = live.report()?;
    let (before, old_hash) = live_ack(&initial, &[]);
    normal(&live.submit("20 addone")?, &["21"]);

    fixture.replace("def addone [ 2 + ]")?;
    let (after, new_hash) = live_ack(
        &live.submit(&format!(":reload {}", fixture.source.display()))?,
        &["21"],
    );
    assert!(after > before, "committed generation must advance");
    assert_ne!(old_hash, new_hash, "changed source must have a new digest");
    normal(&live.submit("20 addone")?, &["21", "22"]);
    live.finish()
}

fn assert_source_change_refused(report: &Value, generation: u64, stack: &[&str], old_hash: &str) {
    assert_eq!(text_field(report, "schema"), "noble-live-report/v1");
    assert_eq!(text_field(report, "stage"), "admission");
    assert_eq!(text_field(report, "outcome"), "source-change-refused", "{report}");
    assert_eq!(number_field(report, "generation"), generation);
    assert_eq!(text_field(report, "selected_file_sha256"), old_hash);
    assert_eq!(number_field(report, "guest_requests"), 0);
    assert_eq!(number_field(report, "protected_operations"), 0);
    assert_eq!(number_field(report, "candidate_prepare_requests"), 0);
    assert_eq!(text_field(report, "prior_namespace"), "unchanged");
    assert_stack(report, stack);
}

#[test]
fn source_verification_requires_explicit_selected_file() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["live", "repl", "--verify-source-before-execute"])
        .stdin(Stdio::null())
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    let reports = String::from_utf8(output.stdout)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(reports.len(), 1);
    assert_eq!(text_field(&reports[0], "outcome"), "reload-refused");
    assert_eq!(number_field(&reports[0], "guest_requests"), 0);
    assert_eq!(number_field(&reports[0], "protected_operations"), 0);
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn watched_complete_rename_acks_only_checked_bytes_and_never_runs_partial_write(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_watch(&fixture.source)?;
    let (old_generation, old_hash) = live_ack(&live.report()?, &[]);
    fs::write(&fixture.source, "def addone [ 2 +")?;
    assert!(matches!(
        live.reports.recv_timeout(Duration::from_millis(250)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ), "an in-place partial write must not produce a save ACK");
    assert_source_change_refused(&live.submit("20 addone")?, old_generation, &[], &old_hash);

    fixture.replace("def addone [ 2 + ]")?;
    let ack = live.report()?;
    let (fresh_generation, fresh_hash) = live_ack(&ack, &[]);
    assert_eq!(fresh_generation, old_generation + 1);
    assert_ne!(fresh_hash, old_hash);
    assert_eq!(text_field(&ack, "watch_save_protocol"), "fsync-temp-then-atomic-rename");
    assert_eq!(text_field(&ack, "source_freshness"), "matched-at-watch-ack-check");
    assert!(number_field(&ack, "watch_sequence") >= 1);
    normal(&live.submit("20 addone")?, &["22"]);
    live.finish()
}

#[cfg(target_os = "linux")]
#[test]
fn watched_rename_preserves_saved_old_program_and_uses_new_word_after_ack(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_watch(&fixture.source)?;
    let (initial, _) = live_ack(&live.report()?, &[]);
    let saved = live.submit("20 [ addone ]")?;
    assert_eq!(saved["stack"][1]["type"], "Program");
    fixture.replace("def addone [ 2 + ]")?;
    let ack = live.report()?;
    assert_eq!(text_field(&ack, "outcome"), "reload-committed", "{ack}");
    assert_eq!(number_field(&ack, "generation"), initial + 1);
    assert_eq!(ack["stack"], saved["stack"]);
    normal(&live.submit("run")?, &["21"]);
    normal(&live.submit("20 addone")?, &["21", "22"]);
    live.finish()
}

#[cfg(target_os = "linux")]
#[test]
fn watch_flag_requires_explicit_selected_source() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["live", "repl", "--watch"])
        .stdin(Stdio::null()).output()?;
    assert_eq!(output.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(text_field(&report, "outcome"), "reload-refused");
    assert_eq!(number_field(&report, "guest_requests"), 0);
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn watched_symlink_escape_refuses_and_next_complete_save_recovers(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_watch(&fixture.source)?;
    let (old_generation, old_hash) = live_ack(&live.report()?, &[]);
    let unselected = fixture.directory.parent().ok_or("no fixture parent")?
        .join("unselected-watch-escape.noble");
    let temporary = fixture.directory.join("math.noble.tmp");
    symlink(&unselected, &temporary)?;
    fs::rename(&temporary, &fixture.source)?;
    let refused = live.report()?;
    assert_eq!(refused_reload(&refused, &[]), old_generation);
    assert_eq!(number_field(&refused, "guest_requests"), 0);
    assert_source_change_refused(&live.submit("20 addone")?, old_generation, &[], &old_hash);
    fixture.replace("def addone [ 3 + ]")?;
    let ack = live.report()?;
    assert_eq!(live_ack(&ack, &[]).0, old_generation + 1);
    normal(&live.submit("20 addone")?, &["23"]);
    live.finish()
}

#[cfg(target_os = "linux")]
#[test]
fn watched_unpaired_rename_cannot_ack_unselected_writer(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_watch(&fixture.source)?;
    let (initial, hash) = live_ack(&live.report()?, &[]);
    let unselected = fixture.directory.join("different-temp.noble");
    let mut file = File::create_new(&unselected)?;
    file.write_all(b"def addone [ 7 + ]")?;
    file.sync_all()?;
    drop(file);
    fs::rename(&unselected, &fixture.source)?;
    let refused = live.report()?;
    assert_eq!(refused_reload(&refused, &[]), initial);
    assert!(text_field(&refused, "diagnostic").contains("paired atomic rename"));
    assert_source_change_refused(&live.submit("20 addone")?, initial, &[], &hash);
    fixture.replace("def addone [ 2 + ]")?;
    assert_eq!(live_ack(&live.report()?, &[]).0, initial + 1);
    normal(&live.submit("20 addone")?, &["22"]);
    live.finish()
}

#[cfg(target_os = "linux")]
#[test]
fn watched_hardlinked_temp_cannot_claim_immutable_complete_save(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let external = Fixture::new("unused")?;
    let mut live = LiveChild::start_watch(&fixture.source)?;
    let (generation, hash) = live_ack(&live.report()?, &[]);
    let temporary = fixture.directory.join("math.noble.tmp");
    let mut file = File::create_new(&temporary)?;
    file.write_all(b"def addone [ 9 + ]")?;
    file.sync_all()?;
    drop(file);
    fs::remove_file(&external.source)?;
    fs::hard_link(&temporary, &external.source)?;
    fs::rename(&temporary, &fixture.source)?;
    let refused = live.report()?;
    assert_eq!(refused_reload(&refused, &[]), generation);
    assert!(text_field(&refused, "diagnostic").contains("hardlink alias"));
    assert_source_change_refused(&live.submit("20 addone")?, generation, &[], &hash);
    fixture.replace("def addone [ 2 + ]")?;
    assert_eq!(live_ack(&live.report()?, &[]).0, generation + 1);
    normal(&live.submit("20 addone")?, &["22"]);
    live.finish()
}

#[cfg(target_os = "linux")]
#[test]
fn watched_superseded_renames_and_buffered_input_report_accepted_generation(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_watch(&fixture.source)?;
    let initial = live_ack(&live.report()?, &[]).0;
    fixture.replace("def addone [ 2 + ]")?;
    fixture.replace("def addone [ 3 + ]")?;
    // The writer completes both renames before these lines enter stdin. The
    // watcher must drain the complete-save queue before consuming its buffered
    // second top-level line, even if a first rename was staged already.
    let input = live.stdin.as_mut().ok_or("missing child stdin")?;
    input.write_all(b":generation\n20 addone\n")?;
    input.flush()?;
    let mut saw_final_ack = false;
    let mut buffered_call = None;
    let mut saw_inspection = false;
    for _ in 0..8 {
        let report = live.report()?;
        match text_field(&report, "outcome") {
            "reload-committed" if report["watch_save_protocol"].is_string() => {
                assert_eq!(text_field(&report, "source_freshness"),
                    "matched-at-watch-ack-check", "{report}");
                if number_field(&report, "watch_sequence") >= 2 {
                    saw_final_ack = true;
                }
            }
            "watch-superseded" | "reload-refused" => {}
            "generation-observed" => saw_inspection = true,
            "normal" => {
                assert!(saw_final_ack, "top-level ran before the final save ACK");
                assert_stack(&report, &["23"]);
                assert!(number_field(&report, "source_generation") > initial);
                buffered_call = Some(true);
            }
            "source-change-refused" => {
                assert_eq!(number_field(&report, "guest_requests"), 0);
                buffered_call = Some(false);
            }
            other => panic!("unexpected watched report {other}: {report}"),
        }
        if saw_final_ack && saw_inspection && buffered_call.is_some() {
            break;
        }
    }
    assert!(saw_final_ack && saw_inspection, "last complete save was not acknowledged");
    match buffered_call {
        Some(true) => normal(&live.submit("20 addone")?, &["23", "23"]),
        Some(false) => normal(&live.submit("20 addone")?, &["23"]),
        None => panic!("buffered top-level command did not finish"),
    }
    live.finish()
}

#[cfg(target_os = "linux")]
#[test]
fn watched_retained_generation_budget_refuses_third_save_without_losing_old_program(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_watch_with_limit(&fixture.source, Some(2))?;
    let (initial, _) = live_ack(&live.report()?, &[]);
    let saved = live.submit("20 [ addone ]")?;
    assert_eq!(saved["stack"][1]["type"], "Program");
    fixture.replace("def addone [ 2 + ]")?;
    let accepted = live.report()?;
    assert_eq!(text_field(&accepted, "outcome"), "reload-committed");
    assert_eq!(accepted["stack"], saved["stack"]);
    let second = number_field(&accepted, "generation");
    assert_eq!(second, initial + 1);
    fixture.replace("def addone [ 3 + ]")?;
    let refused = live.report()?;
    assert_eq!(text_field(&refused, "outcome"), "reload-refused");
    assert_eq!(number_field(&refused, "generation"), second);
    assert_eq!(refused["stack"], saved["stack"]);
    assert!(text_field(&refused, "diagnostic").contains("retained generation budget"));
    assert_eq!(number_field(&refused, "guest_requests"), 0);
    // No third generation was accepted. Restore exactly the last ACKed file
    // bytes without a new rename so the source gate permits using the saved
    // old Program and current second-generation definition again.
    fs::write(&fixture.source, "def addone [ 2 + ]")?;
    normal(&live.submit("run")?, &["21"]);
    normal(&live.submit("20 addone")?, &["21", "22"]);
    live.finish()
}

#[test]
fn verified_submission_refuses_atomic_rename_until_explicit_reload() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_verified(&fixture.source)?;
    let (generation, old_hash) = live_ack(&live.report()?, &[]);
    normal(&live.submit("20 addone")?, &["21"]);
    fixture.replace("def addone [ 2 + ]")?;
    let refused = live.submit("20 addone")?;
    assert_source_change_refused(&refused, generation, &["21"], &old_hash);
    assert_ne!(text_field(&refused, "observed_source_sha256"), old_hash);
    assert_generation_inspection(&live.submit(":generation")?, generation, &refused["stack"]);
    let (after, next_hash) = live_ack(
        &live.submit(&format!(":reload {}", fixture.source.display()))?, &["21"],
    );
    assert_eq!(after, generation + 1);
    assert_eq!(text_field(&refused, "observed_source_sha256"), next_hash);
    normal(&live.submit("20 addone")?, &["21", "22"]);
    live.finish()
}

#[test]
fn verified_definition_cannot_extend_namespace_while_selected_file_changed() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_verified(&fixture.source)?;
    let (generation, old_hash) = live_ack(&live.report()?, &[]);
    fixture.replace("def addone [ 2 + ]")?;
    let refused = live.submit("def another [ 3 + ]")?;
    assert_source_change_refused(&refused, generation, &[], &old_hash);
    let (next, _) = live_ack(
        &live.submit(&format!(":reload {}", fixture.source.display()))?, &[],
    );
    assert_eq!(next, generation + 1);
    let unknown = live.submit("20 another")?;
    assert_eq!(text_field(&unknown, "outcome"), "unbound-word", "{unknown}");
    assert_eq!(text_field(&unknown, "stage"), "resolve");
    assert_eq!(number_field(&unknown, "guest_requests"), 0);
    normal(&live.submit("20 addone")?, &["22"]);
    live.finish()
}

#[test]
fn verified_reload_retains_old_program_capture_after_source_change_refusal() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_verified(&fixture.source)?;
    let (generation, old_hash) = live_ack(&live.report()?, &[]);
    let saved = live.submit("20 [ addone ]")?;
    assert_eq!(saved["stack"][1]["type"], "Program");
    fixture.replace("def addone [ 2 + ]")?;
    let refused = live.submit("run")?;
    assert_eq!(text_field(&refused, "outcome"), "source-change-refused", "{refused}");
    assert_eq!(refused["stack"], saved["stack"]);
    assert_eq!(number_field(&refused, "generation"), generation);
    assert_eq!(text_field(&refused, "selected_file_sha256"), old_hash);
    assert_eq!(number_field(&refused, "guest_requests"), 0);
    let ack = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&ack, "outcome"), "reload-committed", "{ack}");
    assert_eq!(number_field(&ack, "generation"), generation + 1);
    assert_eq!(ack["stack"], saved["stack"]);
    normal(&live.submit("run")?, &["21"]);
    normal(&live.submit("20 addone")?, &["21", "22"]);
    live.finish()
}

#[test]
fn verified_submission_detects_external_hardlink_mutation() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let external = Fixture::new("unused")?;
    let mut live = LiveChild::start_verified(&fixture.source)?;
    let (generation, old_hash) = live_ack(&live.report()?, &[]);
    fs::remove_file(&external.source)?;
    fs::hard_link(&fixture.source, &external.source)?;
    fs::write(&external.source, "def addone [ 9 + ]")?;
    let refused = live.submit("20 addone")?;
    assert_source_change_refused(&refused, generation, &[], &old_hash);
    assert_ne!(text_field(&refused, "observed_source_sha256"), old_hash);
    fixture.replace("def addone [ 1 + ]")?;
    normal(&live.submit("20 addone")?, &["21"]);
    assert_eq!(number_field(&live.submit(":generation")?, "generation"), generation);
    live.finish()
}

#[test]
fn verified_submission_refuses_unverifiable_selected_path_without_guest() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_verified(&fixture.source)?;
    let (generation, old_hash) = live_ack(&live.report()?, &[]);
    let displaced = fixture.directory.join("displaced.noble");
    fs::rename(&fixture.source, &displaced)?;
    let refused = live.submit("20 addone")?;
    assert_source_change_refused(&refused, generation, &[], &old_hash);
    assert_eq!(refused["observed_source_sha256"], Value::Null);
    assert_eq!(number_field(&refused, "guest_requests"), 0);
    fs::rename(displaced, &fixture.source)?;
    normal(&live.submit("20 addone")?, &["21"]);
    live.finish()
}

#[test]
fn verified_submission_refuses_partial_in_place_bytes_before_guest() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start_verified(&fixture.source)?;
    let (generation, old_hash) = live_ack(&live.report()?, &[]);
    fs::write(&fixture.source, "def addone [ 2 +")?;
    let refused = live.submit("20 addone")?;
    assert_source_change_refused(&refused, generation, &[], &old_hash);
    let stale = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&stale, "outcome"), "reload-refused", "{stale}");
    assert_eq!(number_field(&stale, "generation"), generation);
    assert_eq!(number_field(&stale, "guest_requests"), 0);
    fixture.replace("def addone [ 2 + ]")?;
    let (next, _) = live_ack(
        &live.submit(&format!(":reload {}", fixture.source.display()))?, &[],
    );
    assert_eq!(next, generation + 1);
    normal(&live.submit("20 addone")?, &["22"]);
    live.finish()
}

#[test]
fn declared_module_snapshots_preserve_old_program_and_fresh_new_alias() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("module Definitions@5 [ export step def step [ 1 + ] ]")?;
    let bindings = fixture.directory.join("bindings.txt");
    fs::write(&bindings, "")?;
    let mut live = LiveChild::start_declared(&fixture.source, &bindings)?;
    let initial = live.report()?;
    let (initial_generation, old_hash) = live_ack(&initial, &[]);
    assert_eq!(text_field(&initial, "declared_kind"), "module");
    let linked = live.submit("import Definitions@5 as d")?;
    assert_eq!(text_field(&linked, "outcome"), "namespace-committed");
    assert_eq!(text_field(&linked, "declared_kind"), "import");
    assert_eq!(number_field(&linked, "generation"), initial_generation + 1);
    let saved = live.submit("20 [ d.step ]")?;
    assert_eq!(saved["stack"][1]["type"], "Program", "{saved}");
    fixture.replace("module Definitions@6 [ export step def step [ 2 + ] ]")?;
    let refused = live.submit("run")?;
    assert_eq!(text_field(&refused, "outcome"), "source-change-refused");
    assert_eq!(refused["stack"], saved["stack"]);
    assert_eq!(number_field(&refused, "guest_requests"), 0);
    assert_eq!(text_field(&refused, "selected_file_sha256"), old_hash);
    let updated = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&updated, "outcome"), "reload-committed", "{updated}");
    assert_eq!(text_field(&updated, "declared_kind"), "module");
    assert_eq!(number_field(&updated, "generation"), initial_generation + 2);
    assert_ne!(text_field(&updated, "source_sha256"), old_hash);
    assert_eq!(updated["stack"], saved["stack"]);
    let next = live.submit("import Definitions@6 as e")?;
    assert_eq!(text_field(&next, "outcome"), "namespace-committed", "{next}");
    assert_eq!(number_field(&next, "generation"), initial_generation + 3);
    normal(&live.submit("run")?, &["21"]);
    normal(&live.submit("20 e.step")?, &["21", "22"]);
    live.finish()
}

#[cfg(target_os = "linux")]
#[test]
fn watched_declared_module_save_preserves_old_program_and_imports_new_version(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("module m@1 [ export step def step [ 1 + ] ]")?;
    let bindings = fixture.directory.join("bindings.txt");
    fs::write(&bindings, "")?;
    let mut live = LiveChild::start_watch_declared(&fixture.source, &bindings)?;
    let initial = live.report()?;
    live_ack(&initial, &[]);
    assert_eq!(text_field(&initial, "declared_kind"), "module");
    assert_eq!(text_field(&live.submit("import m@1 as m")?, "outcome"), "namespace-committed");
    let saved = live.submit("20 [ m.step ]")?;
    assert_eq!(saved["stack"][1]["type"], "Program", "{saved}");
    fixture.replace("module m@2 [ export step def step [ 2 + ] ]")?;
    let ack = live.report()?;
    assert_eq!(text_field(&ack, "outcome"), "reload-committed", "{ack}");
    assert_eq!(text_field(&ack, "source_freshness"), "matched-at-watch-ack-check", "{ack}");
    assert_eq!(ack["stack"], saved["stack"]);
    let imported = live.submit("import m@2 as next")?;
    assert_eq!(text_field(&imported, "outcome"), "namespace-committed", "{imported}");
    normal(&live.submit("run")?, &["21"]);
    normal(&live.submit("20 next.step")?, &["21", "22"]);
    live.finish()
}

#[test]
fn declared_checked_rebind_replays_transitive_dependents_without_rewriting_saved_program(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("module m@1 [ export addone def addone [ 1 + ] ]")?;
    let bindings = fixture.directory.join("bindings.txt");
    fs::write(&bindings, "")?;
    let mut live = LiveChild::start_declared(&fixture.source, &bindings)?;
    let (initial, _) = live_ack(&live.report()?, &[]);
    for source in ["import m@1 as m", "import m@1 as other",
        "def once [ m.addone ]", "def twice [ once once ]"] {
        let receipt = live.submit(source)?;
        assert_eq!(text_field(&receipt, "outcome"), "namespace-committed", "{receipt}");
    }
    let saved = live.submit("20 [ m.addone ]")?;
    assert_eq!(saved["stack"][1]["type"], "Program", "{saved}");
    fixture.replace("module m@2 [ export addone def addone [ 2 + ] ]")?;
    let updated = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&updated, "outcome"), "reload-committed", "{updated}");
    assert_eq!(updated["stack"], saved["stack"]);
    let rebound = live.submit("import m@2 as m")?;
    assert_eq!(text_field(&rebound, "outcome"), "namespace-committed", "{rebound}");
    assert_eq!(number_field(&rebound, "generation"), initial + 6);
    let rebuilt = rebound["rebuilt_import_definitions"].as_array()
        .ok_or("checked transitive rebind omitted replay receipts")?;
    let rebuilt_names: std::collections::BTreeSet<_> =
        rebuilt.iter().map(|entry| text_field(entry, "name")).collect();
    assert_eq!(rebuilt_names, ["once", "twice"].into_iter().collect());
    for entry in rebuilt {
        assert_ne!(entry["previous_identity"], entry["new_identity"]);
    }
    assert_eq!(rebound["stack"], saved["stack"]);
    normal(&live.submit("run")?, &["21"]);
    normal(&live.submit("20 twice")?, &["21", "24"]);
    normal(&live.submit("20 other.addone")?, &["21", "24", "21"]);
    normal(&live.submit("20 m.addone")?, &["21", "24", "21", "22"]);
    fixture.replace("module m@3 [ export addone def addone [ true ] ]")?;
    let incompatible = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&incompatible, "outcome"), "reload-committed", "{incompatible}");
    let rejected = live.submit("import m@3 as m")?;
    assert_ne!(text_field(&rejected, "outcome"), "namespace-committed", "{rejected}");
    assert_eq!(number_field(&live.submit(":generation")?, "generation"),
        number_field(&incompatible, "generation"));
    normal(&live.submit("20 twice")?, &["21", "24", "21", "22", "24"]);
    live.finish()
}

#[test]
fn declared_same_version_changed_bytes_refuse_without_losing_old_owner() -> Result<(), Box<dyn std::error::Error>> {
    let original = "module Definitions@5 [ export step def step [ 1 + ] ]";
    let fixture = Fixture::new(original)?;
    let bindings = fixture.directory.join("bindings.txt");
    fs::write(&bindings, "")?;
    let mut live = LiveChild::start_declared(&fixture.source, &bindings)?;
    let (generation, old_hash) = live_ack(&live.report()?, &[]);
    assert_eq!(text_field(&live.submit("import Definitions@5 as d")?, "outcome"), "namespace-committed");
    fixture.replace("module Definitions@5 [ export step def step [ 2 + ] ]")?;
    let refused = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&refused, "outcome"), "reload-refused", "{refused}");
    assert_eq!(number_field(&refused, "generation"), generation + 1);
    assert_eq!(number_field(&refused, "guest_requests"), 0);
    let blocked = live.submit("20 d.step")?;
    assert_eq!(text_field(&blocked, "outcome"), "source-change-refused");
    assert_eq!(text_field(&blocked, "selected_file_sha256"), old_hash);
    fixture.replace(original)?;
    normal(&live.submit("20 d.step")?, &["21"]);
    assert_eq!(number_field(&live.submit(":generation")?, "generation"), generation + 1);
    live.finish()
}

#[test]
fn declared_saved_adapter_keeps_old_identity_while_host_policy_allows_it() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(
        "module ledger@1 [ require emit Text -- ! test.emit export send def send [ \"A\" emit ] ]",
    )?;
    let bindings = fixture.directory.join("bindings.txt");
    fs::write(
        &bindings,
        "bind ledger@1 test.emit emit-a Text -- ! test.emit allow\n\
         bind ledger@2 test.emit emit-b Text -- ! test.emit allow\n",
    )?;
    let mut live = LiveChild::start_declared(&fixture.source, &bindings)?;
    let initial = live.report()?;
    let (before, _) = live_ack(&initial, &[]);
    assert_eq!(number_field(&initial, "guest_requests"), 0);
    let imported = live.submit("import ledger@1 as l")?;
    assert_eq!(text_field(&imported, "outcome"), "namespace-committed", "{imported}");
    let saved = live.submit("[ l.send ]")?;
    assert_eq!(saved["stack"][0]["type"], "Program", "{saved}");
    assert_eq!(number_field(&saved, "guest_requests"), 0);
    fixture.replace(
        "module ledger@2 [ require emit Text -- ! test.emit export send def send [ \"B\" emit ] ]",
    )?;
    let updated = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&updated, "outcome"), "reload-committed", "{updated}");
    assert_eq!(number_field(&updated, "generation"), before + 2);
    assert_eq!(updated["stack"], saved["stack"]);
    let imported = live.submit("import ledger@2 as n")?;
    assert_eq!(text_field(&imported, "outcome"), "namespace-committed", "{imported}");
    let old = live.submit("run")?;
    normal(&old, &[]);
    assert_eq!(number_field(&old, "protected_operations"), 1);
    assert_eq!(old["request_trace"][0]["adapter_identity"], "emit-a");
    assert_eq!(old["request_trace"][0]["version"], 1);
    assert_eq!(old["request_trace"][0]["text"], "A");
    assert_eq!(old["request_trace"][0]["decision"], "allow");
    let fresh = live.submit("n.send")?;
    normal(&fresh, &[]);
    assert_eq!(number_field(&fresh, "protected_operations"), 1);
    assert_eq!(fresh["request_trace"][0]["adapter_identity"], "emit-b");
    assert_eq!(fresh["request_trace"][0]["version"], 2);
    assert_eq!(fresh["request_trace"][0]["text"], "B");
    live.finish()
}

#[test]
fn declared_rebound_alias_revokes_old_adapter_even_for_saved_program(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(
        "module ledger@1 [ require emit Text -- ! test.emit export send def send [ \"A\" emit ] ]",
    )?;
    let bindings = fixture.directory.join("bindings.txt");
    fs::write(
        &bindings,
        "bind ledger@1 test.emit emit-a Text -- ! test.emit allow\n\
         bind ledger@2 test.emit emit-b Text -- ! test.emit allow\n",
    )?;
    let mut live = LiveChild::start_declared(&fixture.source, &bindings)?;
    let (initial, _) = live_ack(&live.report()?, &[]);
    let linked = live.submit("import ledger@1 as l")?;
    assert_eq!(text_field(&linked, "outcome"), "namespace-committed", "{linked}");
    let saved = live.submit("[ l.send ]")?;
    assert_eq!(saved["stack"][0]["type"], "Program", "{saved}");
    fixture.replace(
        "module ledger@2 [ require emit Text -- ! test.emit export send def send [ \"B\" emit ] ]",
    )?;
    let updated = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&updated, "outcome"), "reload-committed", "{updated}");
    let rebound = live.submit("import ledger@2 as l")?;
    assert_eq!(text_field(&rebound, "outcome"), "namespace-committed", "{rebound}");
    assert_eq!(number_field(&rebound, "generation"), initial + 3);
    assert_eq!(rebound["rebuilt_import_definitions"], serde_json::json!([]));
    assert_eq!(rebound["stack"], saved["stack"]);
    let refused = live.submit("run")?;
    assert_eq!(text_field(&refused, "outcome"), "trap", "{refused}");
    assert_eq!(number_field(&refused, "protected_operations"), 0);
    assert_eq!(refused["request_trace"][0]["adapter_identity"], "emit-a");
    assert_eq!(refused["request_trace"][0]["decision"], "deny");
    assert_eq!(text_field(&refused, "session_state"), "terminated-after-runtime-failure");
    Ok(())
}

#[cfg(feature = "live-test-barrier")]
#[test]
fn live06_host_revokes_old_effect_before_actual_retained_wasm_request(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(
        "module ledger@1 [ require emit Text -- ! test.emit export send def send [ \"A\" emit ] ]",
    )?;
    let bindings = fixture.directory.join("bindings.txt");
    fs::write(
        &bindings,
        "bind ledger@1 test.emit emit-a Text -- ! test.emit allow\n\
         bind ledger@2 test.emit emit-b Text -- ! test.emit allow\n",
    )?;
    let mut live = LiveChild::start_declared(&fixture.source, &bindings)?;
    let (initial, _) = live_ack(&live.report()?, &[]);
    let imported = live.submit("import ledger@1 as l")?;
    assert_eq!(text_field(&imported, "outcome"), "namespace-committed", "{imported}");
    let saved = live.submit("[ l.send ]")?;
    assert_eq!(saved["stack"][0]["type"], "Program", "{saved}");
    assert_eq!(saved["stack"][0]["interface"]["effects"], serde_json::json!(["test.emit"]));
    assert_eq!(number_field(&saved, "guest_requests"), 0);

    // An explicit, test-feature-only host event revokes A while its old alias
    // is still active. The later alias switch must not masquerade as revocation.
    let revoked = live.submit(":test-host-revoke-ledger1-emit")?;
    assert_eq!(text_field(&revoked, "stage"), "host-policy");
    assert_eq!(text_field(&revoked, "outcome"), "host-policy-revoked", "{revoked}");
    assert_eq!(text_field(&revoked, "module"), "ledger");
    assert_eq!(number_field(&revoked, "version"), 1);
    assert_eq!(text_field(&revoked, "effect"), "test.emit");
    assert_eq!(text_field(&revoked, "adapter_identity"), "emit-a");
    assert_eq!(revoked["previously_authorized"], true);
    assert_eq!(revoked["currently_authorized"], false);
    assert_eq!(revoked["stack"], saved["stack"]);
    assert_eq!(number_field(&revoked, "generation"), initial + 1);
    assert_eq!(number_field(&revoked, "guest_requests"), 0);
    assert_eq!(number_field(&revoked, "protected_operations"), 0);
    assert_generation_inspection(&live.submit(":generation")?, initial + 1, &saved["stack"]);

    fixture.replace(
        "module ledger@2 [ require emit Text -- ! test.emit export send def send [ \"B\" emit ] ]",
    )?;
    let updated = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&updated, "outcome"), "reload-committed", "{updated}");
    assert_eq!(updated["stack"], saved["stack"]);
    assert_eq!(number_field(&updated, "guest_requests"), 0);
    assert_eq!(number_field(&updated, "protected_operations"), 0);
    let imported = live.submit("import ledger@2 as l")?;
    assert_eq!(text_field(&imported, "outcome"), "namespace-committed", "{imported}");
    assert_eq!(imported["stack"], saved["stack"]);
    assert_eq!(number_field(&imported, "guest_requests"), 0);
    assert_eq!(number_field(&imported, "protected_operations"), 0);

    let refused = live.submit("run")?;
    assert_eq!(text_field(&refused, "outcome"), "trap", "{refused}");
    assert_eq!(refused["stack"], serde_json::json!([]), "{refused}");
    assert_eq!(text_field(&refused, "session_state"), "terminated-after-runtime-failure");
    assert_eq!(number_field(&refused, "guest_requests"), 1);
    assert_eq!(number_field(&refused, "host_requests"), 1);
    assert_eq!(number_field(&refused, "protected_operations"), 0);
    assert_eq!(number_field(&refused, "candidate_prepare_requests"), 0);
    assert_eq!(number_field(&refused, "ambient_fallback_calls"), 0);
    assert_eq!(refused["effect_requests"], serde_json::json!(["test.emit"]));
    assert_eq!(refused["request_trace_complete"], true);
    assert_eq!(refused["declared_effect_preserved"], true);
    assert_eq!(refused["request_trace"].as_array().map(Vec::len), Some(1));
    let request = &refused["request_trace"][0];
    assert_eq!(text_field(request, "module"), "ledger");
    assert_eq!(number_field(request, "version"), 1);
    assert_eq!(text_field(request, "effect"), "test.emit");
    assert_eq!(text_field(request, "adapter_identity"), "emit-a");
    assert_eq!(text_field(request, "decision"), "deny");
    assert_eq!(text_field(request, "reason"), "policy-denied");
    assert_ne!(text_field(request, "adapter_identity"), "emit-b");
    assert_eq!(refused["request_trace"][0]["text"], serde_json::Value::Null);
    let request_identity = format!(
        "{}@{}:{}",
        text_field(request, "module"),
        number_field(request, "version"),
        text_field(request, "effect"),
    );
    assert_eq!(request_identity, "ledger@1:test.emit");
    println!("LIVE06 scoped old-code/current-host observation: {}", serde_json::json!({
        "request_identity": request_identity,
        "outcome": refused["outcome"],
        "session_state": refused["session_state"],
        "guest_requests": refused["guest_requests"],
        "protected_operations": refused["protected_operations"],
        "staging_guest_requests": updated["guest_requests"],
        "policy_authorized_before_reload": revoked["currently_authorized"],
    }));
    Ok(())
}

#[test]
fn generation_inspection_without_selected_source_is_exact_and_inert(
) -> Result<(), Box<dyn std::error::Error>> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_noble"));
    command.args(["live", "repl", "--engine", "v8"]);
    let mut live = LiveChild::spawn(command)?;
    let empty = serde_json::json!([]);
    assert_generation_inspection(&live.submit(":generation")?, 0, &empty);
    for malformed in [":generation ", ":generation anything", ":generationX"] {
        assert_eq!(refused_reload(&live.submit(malformed)?, &[]), 0);
    }
    assert_generation_inspection(&live.submit(":generation")?, 0, &empty);
    live.finish()
}

#[test]
fn generation_inspection_preserves_typed_program_and_tracks_only_publication(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    let (initial_generation, _) = live_ack(&live.report()?, &[]);
    assert_eq!(initial_generation, 1);
    assert_generation_inspection(&live.submit(":generation")?, 1, &serde_json::json!([]));

    let defined = live.submit("def twice [ addone addone ]")?;
    assert_eq!(text_field(&defined, "outcome"), "defined");
    assert_generation_inspection(&live.submit(":generation")?, 2, &serde_json::json!([]));

    let saved = live.submit("20 \"keep\" [ addone ]")?;
    assert_eq!(text_field(&saved, "outcome"), "normal");
    assert_eq!(saved["stack"][0]["value"], "20");
    assert_eq!(saved["stack"][1]["type"], "Text");
    assert_eq!(saved["stack"][1]["value"], "keep");
    assert_eq!(saved["stack"][2]["type"], "Program");
    assert_eq!(saved["stack"][2]["interface"]["stack_in"], "[I64]");
    assert_eq!(saved["stack"][2]["interface"]["stack_out"], "[I64]");
    assert_generation_inspection(&live.submit(":generation")?, 2, &saved["stack"]);

    let next = live.submit("7")?;
    assert_eq!(text_field(&next, "outcome"), "normal");
    assert_eq!(next["stack"][3]["value"], "7");
    assert_generation_inspection(&live.submit(":generation")?, 2, &next["stack"]);

    fixture.replace("def addone [ 2 +")?;
    let refused = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&refused, "outcome"), "reload-refused");
    assert_eq!(number_field(&refused, "generation"), 2);
    assert_eq!(refused["stack"], next["stack"]);
    assert_generation_inspection(&live.submit(":generation")?, 2, &next["stack"]);

    fixture.replace("def addone [ 2 + ]")?;
    let committed = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&committed, "outcome"), "reload-committed");
    assert_eq!(number_field(&committed, "generation"), 3);
    assert_eq!(committed["stack"], next["stack"]);
    assert_generation_inspection(&live.submit(":generation")?, 3, &next["stack"]);

    let old = live.submit("drop dip")?;
    assert_eq!(text_field(&old, "outcome"), "normal");
    assert_eq!(old["stack"][0]["value"], "21");
    assert_eq!(old["stack"][1]["value"], "keep");
    assert_generation_inspection(&live.submit(":generation")?, 3, &old["stack"]);

    let fresh = live.submit("20 twice")?;
    assert_eq!(text_field(&fresh, "outcome"), "normal");
    assert_eq!(fresh["stack"][2]["value"], "24");
    assert_generation_inspection(&live.submit(":generation")?, 3, &fresh["stack"]);
    live.finish()
}

#[test]
fn unsupported_live_subcommand_refuses_before_publication(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let output = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["live", "watch"])
        .arg(&fixture.source)
        .stdin(Stdio::null())
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    let reports = String::from_utf8(output.stdout)?;
    let [refused] = reports.lines().map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "expected one unsupported-command refusal")?;
    assert_eq!(text_field(&refused, "outcome"), "reload-refused");
    assert_eq!(number_field(&refused, "generation"), 0);
    assert_eq!(number_field(&refused, "guest_requests"), 0);
    Ok(())
}

#[test]
fn malformed_reload_preserves_the_prior_namespace_and_typed_stack()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    normal(&live.submit("7")?, &["7"]);
    let generation = current_generation(&mut live, &fixture, &["7"])?;

    fixture.replace("def addone [ 2 +")?;
    let refused = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(refused_reload(&refused, &["7"]), generation);
    normal(&live.submit("addone")?, &["8"]);
    live.finish()
}

#[test]
fn wrong_type_reload_reports_candidate_origin_and_keeps_old_definition()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    let prior = live.submit("7")?;
    normal(&prior, &["7"]);
    let generation = current_generation(&mut live, &fixture, &["7"])?;

    fixture.replace("def addone [ true + ]")?;
    let refused = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(refused_reload(&refused, &["7"]), generation);
    assert_eq!(refused["stack"], prior["stack"]);
    assert_eq!(refused["source_span_basis"], "refused-candidate-file-byte-offsets");
    assert_eq!(refused["source_span"], serde_json::json!({"start": 18, "end": 19}));
    assert_eq!(refused["word_or_join"], "+");
    assert_eq!(refused["required_stack"], "?stack I64 I64");
    assert_eq!(refused["actual_stack"], "?stack Bool");
    assert_eq!(refused["constraint"], "stack-type");
    assert_eq!(
        refused["value_origin_or_unavailable"],
        serde_json::json!({"start": 13, "end": 17})
    );
    assert_generation_inspection(&live.submit(":generation")?, generation, &prior["stack"]);
    normal(&live.submit("addone")?, &["8"]);
    live.finish()
}

#[test]
fn symlink_selected_leaf_refuses_without_changing_the_session()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    normal(&live.submit("7")?, &["7"]);
    let generation = current_generation(&mut live, &fixture, &["7"])?;

    let unselected = fixture.directory.join("unselected.noble");
    fs::write(&unselected, "def addone [ 100 + ]")?;
    let temporary = fixture.directory.join("math.noble.tmp");
    symlink(&unselected, &temporary)?;
    fs::rename(&temporary, &fixture.source)?;

    let refused = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(refused_reload(&refused, &["7"]), generation);
    fixture.replace("def addone [ 1 + ]")?;
    normal(&live.submit("addone")?, &["8"]);
    live.finish()
}

#[test]
fn reload_preserves_text_and_old_program_while_new_calls_use_new_definition()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    let (before, _) = live_ack(&live.report()?, &[]);

    let prior = live.submit("20 \"keep\" [ addone ]")?;
    assert_eq!(text_field(&prior, "outcome"), "normal");
    let retained = prior["stack"]
        .as_array()
        .ok_or("missing retained stack")?
        .clone();
    assert_eq!(retained.len(), 3);
    assert_eq!(retained[0]["type"], "I64");
    assert_eq!(retained[0]["value"], "20");
    assert_eq!(retained[1]["type"], "Text");
    assert_eq!(retained[1]["value"], "keep");
    assert_eq!(retained[2]["type"], "Program");

    fixture.replace("def addone [ 2 + ]")?;
    let ack = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&ack, "schema"), "noble-live-report/v1");
    assert_eq!(text_field(&ack, "stage"), "wasm-live");
    assert_eq!(text_field(&ack, "outcome"), "reload-committed");
    assert!(number_field(&ack, "generation") > before);
    assert_eq!(number_field(&ack, "guest_requests"), 0);
    assert_eq!(number_field(&ack, "protected_operations"), 0);
    assert_eq!(ack["stack"], serde_json::Value::Array(retained));

    let old_call = live.submit("dip")?;
    assert_eq!(text_field(&old_call, "outcome"), "normal");
    assert_eq!(old_call["stack"][0]["type"], "I64");
    assert_eq!(old_call["stack"][0]["value"], "21");
    assert_eq!(old_call["stack"][1]["type"], "Text");
    assert_eq!(old_call["stack"][1]["value"], "keep");

    let new_call = live.submit("20 addone")?;
    assert_eq!(text_field(&new_call, "outcome"), "normal");
    assert_eq!(new_call["stack"][0]["type"], "I64");
    assert_eq!(new_call["stack"][0]["value"], "21");
    assert_eq!(new_call["stack"][1]["type"], "Text");
    assert_eq!(new_call["stack"][1]["value"], "keep");
    assert_eq!(new_call["stack"][2]["type"], "I64");
    assert_eq!(new_call["stack"][2]["value"], "22");
    live.finish()
}

fn rejected_replacement_keeps_old_addone(
    replacement: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    normal(&live.submit("7")?, &["7"]);
    let generation = current_generation(&mut live, &fixture, &["7"])?;

    fixture.replace(replacement)?;
    let refused = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(refused_reload(&refused, &["7"]), generation);
    normal(&live.submit("addone")?, &["8"]);
    live.finish()?;
    Ok(refused)
}

#[test]
fn incompatible_definition_interface_refuses_without_changing_session()
-> Result<(), Box<dyn std::error::Error>> {
    let refused = rejected_replacement_keeps_old_addone("def addone [ dup ]")?;
    assert_no_candidate_span(&refused);
    Ok(())
}

#[test]
fn effectful_definition_without_host_grant_refuses_without_guest_effects()
-> Result<(), Box<dyn std::error::Error>> {
    rejected_replacement_keeps_old_addone("def addone [ \"x\" test.emit 2 + ]")?;
    Ok(())
}

#[test]
fn direct_effectful_submission_without_binding_never_enters_guest()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    let pushed = live.submit("7")?;
    normal(&pushed, &["7"]);
    let generation = current_generation(&mut live, &fixture, &["7"])?;

    // No-gift Core-live frontend cannot even resolve a test host effect. The
    // selected worker must receive zero requests and preserve the old stack.
    for source in ["\"x\" test.emit", "def emit [ \"x\" test.emit ]"] {
        let refused = live.submit(source)?;
        assert_eq!(text_field(&refused, "stage"), "resolve", "{refused}");
        assert_eq!(text_field(&refused, "outcome"), "unbound-word", "{refused}");
        assert_eq!(number_field(&refused, "guest_requests"), 0);
        assert_eq!(number_field(&refused, "protected_operations"), 0);
        assert_eq!(text_field(&refused, "prior_stack"), "unchanged");
        assert_eq!(text_field(&refused, "prior_namespace"), "unchanged");
        assert_generation_inspection(&live.submit(":generation")?, generation, &pushed["stack"]);
    }
    let unbound = live.submit("emit")?;
    assert_eq!(text_field(&unbound, "stage"), "resolve");
    assert_eq!(text_field(&unbound, "outcome"), "unbound-word");
    assert_eq!(number_field(&unbound, "guest_requests"), 0);
    assert_eq!(text_field(&unbound, "prior_stack"), "unchanged");
    assert_eq!(text_field(&unbound, "prior_namespace"), "unchanged");
    normal(&live.submit("addone")?, &["8"]);
    live.finish()
}

#[test]
fn ten_real_reloads_preserve_checked_budget_and_reuse_cleared_code(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    let initial = live.report()?;
    live_ack(&initial, &[]);
    let first_call = live.submit("20 addone")?;
    normal(&first_call, &["21"]);
    let first_span = &first_call["installed_code_span"];
    assert!(number_field(first_span, "length") > 0, "{first_call}");
    let mut generation = number_field(&initial, "generation");
    let mut expected = vec!["21".to_string()];
    for increment in 2..=11 {
        fixture.replace(&format!("def addone [ {increment} + ]"))?;
        let ack = live.submit(&format!(":reload {}", fixture.source.display()))?;
        assert_eq!(text_field(&ack, "outcome"), "reload-committed", "{ack}");
        assert_eq!(ack["stack"].as_array().map(Vec::len), Some(expected.len()), "{ack}");
        generation += 1;
        assert_eq!(number_field(&ack, "generation"), generation, "{ack}");
        if increment == 2 {
            let retired = ack["retired_code_spans"].as_array()
                .ok_or("checked live code retirement omitted worker receipts")?;
            assert!(retired.iter().any(|row| row["code_span"] == *first_span
                && row["code_owners"] == 0 && row["table_cleared"] == true), "{ack}");
        }
        let value = live.submit("20 addone")?;
        assert_eq!(text_field(&value, "outcome"), "normal", "{value}");
        expected.push((increment + 20).to_string());
        let stack = value["stack"].as_array().ok_or("normal execution omitted guest stack")?;
        assert_eq!(stack.len(), expected.len(), "{value}");
        assert_eq!(stack.last().and_then(|top| top["value"].as_str()),
            expected.last().map(String::as_str), "{value}");
        if increment == 2 {
            assert_eq!(value["installed_code_span"]["start"], first_span["start"], "{value}");
        }
    }
    live.finish()
}

#[test]
fn dependent_rebuild_is_transitive_and_preserves_saved_old_program()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    assert_eq!(text_field(&live.submit("def twice [ addone addone ]")?, "outcome"), "defined");
    assert_eq!(text_field(&live.submit("def four [ twice twice ]")?, "outcome"), "defined");
    assert_eq!(text_field(&live.submit("def unrelated [ 10 + ]")?, "outcome"), "defined");
    let saved = live.submit("[ addone ]")?;
    assert_eq!(saved["stack"][0]["type"], "Program");
    let saved_span = &saved["installed_code_span"];
    assert!(number_field(saved_span, "length") > 0, "{saved}");
    let other = fixture.directory.join("unselected.noble");
    let probe = live.submit(&format!(":reload {}", other.display()))?;
    assert_eq!(text_field(&probe, "outcome"), "reload-refused");
    assert_eq!(probe["stack"], saved["stack"]);
    let generation = number_field(&probe, "generation");

    fixture.replace("def addone [ 2 + ]")?;
    let ack = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&ack, "outcome"), "reload-committed", "{ack}");
    assert_eq!(number_field(&ack, "generation"), generation + 1);
    assert_eq!(ack["stack"], saved["stack"]);
    assert_eq!(number_field(&ack, "guest_requests"), 0);
    assert!(!ack["retired_code_spans"].as_array()
        .ok_or("checked retirement receipts missing")?
        .iter().any(|row| row["code_span"] == *saved_span),
        "a saved old Program must pin its old code: {ack}");
    normal(&live.submit("20 swap run")?, &["21"]);
    normal(&live.submit("20 four")?, &["21", "28"]);
    normal(&live.submit("20 unrelated")?, &["21", "28", "30"]);
    live.finish()
}

#[test]
fn dependent_inside_nested_quotation_resolves_new_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    assert_eq!(text_field(&live.submit("def wrapped [ [ addone ] ]")?, "outcome"), "defined");
    normal(&live.submit("20 wrapped run")?, &["21"]);
    fixture.replace("def addone [ 2 + ]")?;
    live_ack(&live.submit(&format!(":reload {}", fixture.source.display()))?, &["21"]);
    normal(&live.submit("20 wrapped run")?, &["21", "22"]);
    live.finish()
}

#[test]
fn queued_reload_ack_follows_prior_wasm_invocation()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    fixture.replace("def addone [ 2 + ]")?;
    let input = live.stdin.as_mut().ok_or("missing live stdin")?;
    input.write_all(format!("20 addone\n:reload {}\n", fixture.source.display()).as_bytes())?;
    input.flush()?;
    normal(&live.report()?, &["21"]);
    live_ack(&live.report()?, &["21"]);
    normal(&live.submit("20 addone")?, &["21", "22"]);
    live.finish()
}

#[test]
fn incompatible_replacement_with_dependents_never_acknowledges()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    assert_eq!(text_field(&live.submit("def twice [ addone addone ]")?, "outcome"), "defined");
    normal(&live.submit("20 twice")?, &["22"]);
    let generation = current_generation(&mut live, &fixture, &["22"])?;
    fixture.replace("def addone [ dup ]")?;
    let refused = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(refused_reload(&refused, &["22"]), generation);
    assert_no_candidate_span(&refused);
    normal(&live.submit("20 twice")?, &["22", "22"]);
    live.finish()
}

#[test]
fn indirect_self_rebind_via_existing_dependents_never_acknowledges()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def f [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    assert_eq!(text_field(&live.submit("def g [ f ]")?, "outcome"), "defined");
    assert_eq!(text_field(&live.submit("def h [ g ]")?, "outcome"), "defined");
    normal(&live.submit("20 h")?, &["21"]);
    let generation = current_generation(&mut live, &fixture, &["21"])?;
    fixture.replace("def f [ h ]")?;
    let refused = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(refused_reload(&refused, &["21"]), generation);
    assert!(
        text_field(&refused, "diagnostic").contains("previous identity"),
        "{refused}"
    );
    normal(&live.submit("20 f")?, &["21", "21"]);
    normal(&live.submit("20 h")?, &["21", "21", "21"]);
    live.finish()
}

#[test]
fn old_program_keeps_shadowed_target_across_dependent_rebuilds()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def f [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    assert_eq!(text_field(&live.submit("def offset [ 10 + ]")?, "outcome"), "defined");
    assert_eq!(text_field(&live.submit("def use [ f offset ]")?, "outcome"), "defined");
    // Capture before the explicit direct redefinition. That transaction
    // rebuilds the current `use` while keeping this older Program immutable.
    let saved = live.submit("[ use ]")?;
    assert_eq!(saved["stack"][0]["type"], "Program");
    assert_eq!(text_field(&live.submit("def offset [ 100 + ]")?, "outcome"), "defined");
    fixture.replace("def f [ 2 + ]")?;
    let ack = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(text_field(&ack, "outcome"), "reload-committed", "{ack}");
    assert_eq!(ack["stack"], saved["stack"]);
    normal(&live.submit("20 swap run")?, &["31"]);
    normal(&live.submit("20 use")?, &["31", "122"]);
    live.finish()
}

#[test]
fn dependent_rebuild_exceeding_retained_source_budget_refuses_atomically()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut live = LiveChild::start(&fixture.source)?;
    live_ack(&live.report()?, &[]);
    let dependent = format!("def twice [ addone addone \"{}\" drop ]", "x".repeat(33_000));
    assert_eq!(text_field(&live.submit(&dependent)?, "outcome"), "defined");
    normal(&live.submit("20 twice")?, &["22"]);
    let generation = current_generation(&mut live, &fixture, &["22"])?;
    fixture.replace("def addone [ 2 + ]")?;
    let refused = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(refused_reload(&refused, &["22"]), generation);
    assert!(
        text_field(&refused, "diagnostic").contains("retained namespace byte limit"),
        "{refused}"
    );
    normal(&live.submit("20 twice")?, &["22", "22"]);
    live.finish()
}

const EVOLVE: &str =
    "def evolve [ dup 18 - quote [ + ] compose self.generation swap self.propose drop 1 + ]";

#[test]
fn generation_inspection_retains_grant_until_guest_publication(
) -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(EVOLVE)?;
    let mut live = LiveChild::start_with_grant(&fixture.source, "evolve", 1)?;
    live_ack(&live.report()?, &[]);
    assert_grant(&live, 1)?;
    assert_generation_inspection(&live.submit(":generation")?, 1, &serde_json::json!([]));

    fixture.replace("def evolve [ 2 +")?;
    let refused = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(refused_reload(&refused, &[]), 1);
    assert_generation_inspection(&live.submit(":generation")?, 1, &serde_json::json!([]));
    fixture.replace(EVOLVE)?;

    normal(&live.submit("20 evolve")?, &["21"]);
    assert_eq!(text_field(&live.report()?, "outcome"), "proposal-pending");
    let committed = live.report()?;
    assert_eq!(text_field(&committed, "outcome"), "proposal-committed");
    assert_eq!(number_field(&committed, "generation"), 2);
    assert_generation_inspection(&live.submit(":generation")?, 2, &committed["stack"]);
    normal(&live.submit("20 evolve")?, &["21", "22"]);
    live.finish()
}

fn assert_grant(live: &LiveChild, generation: u64) -> Result<(), Box<dyn std::error::Error>> {
    let report = live.report()?;
    assert_eq!(text_field(&report, "outcome"), "grant-selected", "{report}");
    assert_eq!(number_field(&report, "source_generation"), generation);
    assert_eq!(text_field(&report, "name"), "evolve");
    Ok(())
}

#[test]
fn named_guest_captures_i64_then_publishes_pure_self_replacement()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(EVOLVE)?;
    let mut live = LiveChild::start_with_grant(&fixture.source, "evolve", 1)?;
    let (initial_generation, file_hash) = live_ack(&live.report()?, &[]);
    assert_eq!(initial_generation, 1);
    assert_grant(&live, initial_generation)?;

    let executed = live.submit("20 evolve")?;
    normal(&executed, &["21"]);
    assert_eq!(
        executed["proposal_queued"], true,
        "actual compiled guest must queue a proposal"
    );
    assert!(
        number_field(&executed, "guest_requests") >= 2,
        "generation observation and proposal are real effects: {executed}"
    );
    let pending = live.report()?;
    assert_eq!(
        text_field(&pending, "outcome"),
        "proposal-pending",
        "{pending}"
    );
    assert_stack(&pending, &["21"]);
    assert_eq!(text_field(&pending, "selected_name"), "evolve");
    assert_eq!(text_field(&pending, "checked_owner_identity"), "1");
    assert_eq!(text_field(&pending, "grant_expected_generation"), "1");
    assert_eq!(text_field(&pending, "proposal_name"), "evolve");
    assert_eq!(text_field(&pending, "proposal_owner_identity"), "1");
    assert_eq!(text_field(&pending, "proposal_source_generation"), "1");
    assert_eq!(text_field(&pending, "proposal_expected_generation"), "1");
    assert_eq!(number_field(&pending, "source_generation"), 1);
    assert_eq!(pending["candidate_program_interface"]["input"], "[I64]");
    assert_eq!(pending["candidate_program_interface"]["output"], "[I64]");
    assert_eq!(pending["candidate_program_interface"]["effects"], 0);
    assert_eq!(pending["pre_invocation_stack"], serde_json::json!([]));
    assert_eq!(pending["postreturn_snapshot_verified"], true);
    assert_eq!(text_field(&pending, "candidate_snapshot_sha256").len(), 64);
    assert_eq!(text_field(&pending, "proposal_recipe_sha256").len(), 64);
    assert!(
        pending["candidate_recipe_events"]
            .as_array()
            .is_some_and(|events| events.len() >= 4)
    );
    assert_eq!(text_field(&pending, "invocation_source_sha256").len(), 64);
    assert_eq!(text_field(&pending, "invocation_wasm_sha256").len(), 64);
    assert_eq!(pending["source_freshness"], "not-file-backed");
    let committed = live.report()?;
    assert_eq!(
        text_field(&committed, "outcome"),
        "proposal-committed",
        "{committed}"
    );
    assert_eq!(committed["origin"], "guest-program-recipe");
    assert_eq!(committed["source_freshness"], "not-file-backed");
    assert_eq!(committed["selected_file_sha256"], file_hash);
    assert_eq!(
        committed["candidate_snapshot_sha256"],
        pending["candidate_snapshot_sha256"]
    );
    assert_eq!(
        committed["candidate_recipe_events"],
        pending["candidate_recipe_events"]
    );
    assert_eq!(
        committed["proposal_recipe_sha256"],
        pending["proposal_recipe_sha256"]
    );
    assert_eq!(committed["checked_owner_identity"], "1");
    assert_eq!(committed["grant_expected_generation"], "1");
    assert_eq!(committed["postreturn_snapshot_verified"], true);
    assert_eq!(committed["pre_invocation_stack"], serde_json::json!([]));
    assert_ne!(text_field(&committed, "source_sha256"), file_hash);
    assert!(number_field(&committed, "generation") > initial_generation);
    assert_stack(&committed, &["21"]);
    assert_eq!(
        fs::read_to_string(&fixture.source)?,
        EVOLVE,
        "guest publication must not write selected file"
    );

    normal(&live.submit("20 evolve")?, &["21", "22"]);
    live.finish()
}

#[test]
fn named_guest_selects_a_different_captured_i64_from_runtime_input()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(EVOLVE)?;
    let mut live = LiveChild::start_with_grant(&fixture.source, "evolve", 1)?;
    live_ack(&live.report()?, &[]);
    assert_grant(&live, 1)?;
    normal(&live.submit("21 evolve")?, &["22"]);
    assert_eq!(text_field(&live.report()?, "outcome"), "proposal-pending");
    let committed = live.report()?;
    assert_eq!(
        text_field(&committed, "outcome"),
        "proposal-committed",
        "{committed}"
    );
    normal(&live.submit("20 evolve")?, &["22", "23"]);
    assert_eq!(fs::read_to_string(&fixture.source)?, EVOLVE);
    live.finish()
}

#[test]
fn postreturn_guest_dependent_refusal_retains_stack_and_grant()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(EVOLVE)?;
    let mut live = LiveChild::start_with_grant(&fixture.source, "evolve", 1)?;
    let (generation, _) = live_ack(&live.report()?, &[]);
    assert_grant(&live, generation)?;
    assert_eq!(
        text_field(&live.submit("def twice [ evolve evolve ]")?, "outcome"),
        "defined"
    );
    let after_definition = live.submit(":grant-self-edit evolve 2")?;
    assert_eq!(
        text_field(&after_definition, "outcome"),
        "grant-selected",
        "{after_definition}"
    );
    for (input, stack) in [(20, &["21"][..]), (21, &["21", "22"][..])] {
        normal(&live.submit(&format!("{input} evolve"))?, stack);
        assert_eq!(text_field(&live.report()?, "outcome"), "proposal-pending");
        let refused = live.report()?;
        assert_eq!(text_field(&refused, "outcome"), "proposal-refused", "{refused}");
        assert_eq!(number_field(&refused, "generation"), 2);
        assert_stack(&refused, stack);
        assert!(number_field(&refused, "guest_requests") >= 2);
        assert_eq!(number_field(&refused, "candidate_prepare_requests"), 0);
        assert!(
            text_field(&refused, "diagnostic").contains("dependent"),
            "{refused}"
        );
    }
    assert_eq!(fs::read_to_string(&fixture.source)?, EVOLVE);
    live.finish()
}

#[test]
fn unsupported_identity_program_refuses_after_guest_without_rolling_back_stack_or_grant()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def evolve [ [ ] self.generation swap self.propose drop 1 + ]")?;
    let mut live = LiveChild::start_with_grant(&fixture.source, "evolve", 1)?;
    live_ack(&live.report()?, &[]);
    assert_grant(&live, 1)?;
    for stack in [&["21"][..], &["21", "21"][..]] {
        normal(&live.submit("20 evolve")?, stack);
        let pending = live.report()?;
        assert_eq!(
            text_field(&pending, "outcome"),
            "proposal-pending",
            "{pending}"
        );
        let refused = live.report()?;
        assert_eq!(
            text_field(&refused, "outcome"),
            "proposal-refused",
            "{refused}"
        );
        assert_stack(&refused, stack);
        assert_eq!(number_field(&refused, "generation"), 1);
        assert!(number_field(&refused, "guest_requests") >= 2);
        assert!(
            !refused["request_trace"]
                .as_array()
                .ok_or("missing trace")?
                .is_empty()
        );
    }
    live.finish()
}

#[test]
fn old_saved_program_is_denied_after_rearming_new_checked_owner()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(EVOLVE)?;
    let mut live = LiveChild::start_with_grant(&fixture.source, "evolve", 1)?;
    live_ack(&live.report()?, &[]);
    assert_grant(&live, 1)?;
    let captured = live.submit("[ evolve ]")?;
    assert_eq!(text_field(&captured, "outcome"), "normal", "{captured}");
    assert_eq!(captured["stack"][0]["type"], "Program");
    fixture.replace(&EVOLVE.replacen("18 -", "17 -", 1))?;
    let reloaded = live.submit(&format!(":reload {}", fixture.source.display()))?;
    assert_eq!(
        text_field(&reloaded, "outcome"),
        "reload-committed",
        "{reloaded}"
    );
    assert_eq!(reloaded["stack"], captured["stack"]);
    let generation = number_field(&reloaded, "generation");
    assert_eq!(generation, 2);
    assert_eq!(
        text_field(&live.submit(":grant-self-edit evolve 2")?, "outcome"),
        "grant-selected"
    );
    let denied = live.submit("20 swap run")?;
    assert_eq!(text_field(&denied, "outcome"), "trap", "{denied}");
    assert!(
        number_field(&denied, "guest_requests") >= 2,
        "host denial must not erase request prefix: {denied}"
    );
    assert_eq!(denied["session_state"], "terminated-after-runtime-failure");
    Ok(())
}

#[test]
fn new_checked_owner_can_self_edit_after_explicit_rearm() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = Fixture::new(EVOLVE)?;
    let mut live = LiveChild::start_with_grant(&fixture.source, "evolve", 1)?;
    live_ack(&live.report()?, &[]);
    assert_grant(&live, 1)?;
    fixture.replace(&EVOLVE.replacen("18 -", "17 -", 1))?;
    let (generation, _) = live_ack(
        &live.submit(&format!(":reload {}", fixture.source.display()))?,
        &[],
    );
    assert_eq!(generation, 2);
    assert_eq!(
        text_field(&live.submit(":grant-self-edit evolve 2")?, "outcome"),
        "grant-selected"
    );
    normal(&live.submit("20 evolve")?, &["21"]);
    assert_eq!(text_field(&live.report()?, "outcome"), "proposal-pending");
    assert_eq!(text_field(&live.report()?, "outcome"), "proposal-committed");
    normal(&live.submit("20 evolve")?, &["21", "23"]);
    live.finish()
}

#[test]
fn stale_expected_source_generation_denies_without_staging_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(&EVOLVE.replace("self.generation", "0"))?;
    let mut live = LiveChild::start_with_grant(&fixture.source, "evolve", 1)?;
    live_ack(&live.report()?, &[]);
    assert_grant(&live, 1)?;
    let denied = live.submit("20 evolve")?;
    assert_eq!(text_field(&denied, "outcome"), "trap", "{denied}");
    assert!(number_field(&denied, "guest_requests") >= 1);
    assert_ne!(
        denied["proposal_queued"], true,
        "stale callback cannot enqueue a proposal"
    );
    assert_eq!(denied["queued_proposal_roots"], 0,
        "rejected callback before retain must not add a backend owner: {denied}");
    assert_eq!(denied["session_state"], "terminated-after-runtime-failure");
    assert_eq!(
        fs::read_to_string(&fixture.source)?,
        EVOLVE.replace("self.generation", "0")
    );
    Ok(())
}

#[test]
fn guest_denial_after_enqueuing_cancels_exact_backend_proposal_pin(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = "def evolve [ dup 18 - quote [ + ] compose self.generation swap self.propose \
        drop dup 18 - quote [ + ] compose self.generation swap self.propose drop 1 + ]";
    let fixture = Fixture::new(source)?;
    let mut live = LiveChild::start_with_grant(&fixture.source, "evolve", 1)?;
    live_ack(&live.report()?, &[]);
    assert_grant(&live, 1)?;
    let saved = live.submit("[ evolve ]")?;
    assert_eq!(saved["stack"][0]["type"], "Program", "{saved}");
    let denied = live.submit("20 evolve")?;
    assert_eq!(text_field(&denied, "outcome"), "trap", "{denied}");
    assert_eq!(denied["session_state"], "terminated-after-runtime-failure");
    assert_eq!(denied["proposal_queued"], false);
    assert_eq!(denied["cancelled_proposal_roots_before_cancel"], 1,
        "actual backend queued owner must be present before cancellation: {denied}");
    assert_eq!(denied["cancelled_proposal_roots_remaining"], 0,
        "backend must prove the exact queued proposal's pin is gone: {denied}");
    assert_eq!(denied["queued_proposal_roots"], 0);
    assert_eq!(denied["cancelled_proposal_release_status"], 0);
    assert_eq!(denied["cancelled_proposal_repeat_release_status"], 4,
        "terminal cancellation must not release twice: {denied}");
    assert_eq!(denied["cancelled_proposal_root_state_after_repeat"],
        denied["cancelled_proposal_root_state_after"], "{denied}");
    assert_eq!(denied["cancelled_proposal_unrelated_root_release_status"], 4,
        "old saved Program must remain pinned while poisoned: {denied}");
    assert_eq!(denied["cancelled_proposal_unrelated_root_state_before"],
        denied["cancelled_proposal_unrelated_root_state_after"], "{denied}");
    let before = number_field(&denied, "cancelled_proposal_root_state_before");
    let after = number_field(&denied, "cancelled_proposal_root_state_after");
    assert_eq!(before, after + 1,
        "exact backend root count, not a JS flag, must decrease by one: {denied}");
    assert_eq!(after & 127, 0, "failed guest retained a host pin: {denied}");
    assert_eq!(before & 128, after & 128,
        "terminal cleanup must not resurrect or clear an independent module owner: {denied}");
    assert_eq!(number_field(&denied, "protected_operations"), 0);
    assert_eq!(number_field(&denied, "candidate_prepare_requests"), 0);
    let trace = denied["request_trace"].as_array().ok_or("missing guest request trace")?;
    assert!(trace.iter().any(|entry| entry == "live.propose:evolve:queued"), "{denied}");
    assert!(trace.iter().any(|entry| entry == "live.propose:denied:already-queued"), "{denied}");
    assert_eq!(fs::read_to_string(&fixture.source)?, source);
    assert_eq!(live.child.wait()?.code(), Some(2));
    Ok(())
}

#[test]
fn guest_work_quota_after_enqueuing_cancels_exact_backend_proposal_pin(
) -> Result<(), Box<dyn std::error::Error>> {
    let source = format!(
        "def evolve [ dup 18 - quote [ + ] compose self.generation swap self.propose drop{} ]",
        " 1 +".repeat(80),
    );
    let fixture = Fixture::new(&source)?;
    let mut positive = LiveChild::start_with_grant(&fixture.source, "evolve", 1)?;
    live_ack(&positive.report()?, &[]);
    assert_grant(&positive, 1)?;
    normal(&positive.submit("20 evolve")?, &["100"]);
    assert_eq!(text_field(&positive.report()?, "outcome"), "proposal-pending");
    assert_eq!(text_field(&positive.report()?, "outcome"), "proposal-committed");
    positive.finish()?;

    let mut quota = LiveChild::start_with_guest_step_budget(&fixture.source, "evolve", 300)?;
    live_ack(&quota.report()?, &[]);
    assert_grant(&quota, 1)?;
    let exhausted = quota.submit("20 evolve")?;
    assert_eq!(text_field(&exhausted, "outcome"), "runtime-exhausted", "{exhausted}");
    assert_eq!(text_field(&exhausted, "session_state"), "terminated-after-runtime-failure");
    assert_eq!(exhausted["proposal_queued"], false);
    assert_eq!(exhausted["queued_proposal_roots"], 0);
    assert_eq!(exhausted["cancelled_proposal_roots_before_cancel"], 1,
        "work quota must follow actual guest proposal enqueue: {exhausted}");
    assert_eq!(exhausted["cancelled_proposal_roots_remaining"], 0);
    assert_eq!(exhausted["cancelled_proposal_release_status"], 0);
    assert_eq!(exhausted["cancelled_proposal_repeat_release_status"], 4);
    let before = number_field(&exhausted, "cancelled_proposal_root_state_before");
    let after = number_field(&exhausted, "cancelled_proposal_root_state_after");
    assert_eq!(before, after + 1, "{exhausted}");
    assert_eq!(after & 127, 0, "{exhausted}");
    assert_eq!(before & 128, after & 128, "{exhausted}");
    let trace = exhausted["request_trace"].as_array().ok_or("missing quota request prefix")?;
    assert!(trace.iter().any(|entry| entry == "live.propose:evolve:queued"), "{exhausted}");
    assert_eq!(number_field(&exhausted, "protected_operations"), 0);
    assert_eq!(number_field(&exhausted, "candidate_prepare_requests"), 0);
    assert_eq!(fs::read_to_string(&fixture.source)?, source);
    assert_eq!(quota.child.wait()?.code(), Some(5));
    Ok(())
}

#[test]
fn no_host_selected_grant_rejects_guest_edit_definition_before_execution()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new(EVOLVE)?;
    let child = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["live", "repl", "--source"])
        .arg(&fixture.source)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let output = child.wait_with_output()?;
    assert_eq!(output.status.code(), Some(2));
    let reports = String::from_utf8(output.stdout)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        reports.len(),
        1,
        "initial source must fail without grant: {reports:?}"
    );
    let refused = &reports[0];
    assert_eq!(
        text_field(refused, "outcome"),
        "reload-refused",
        "{refused}"
    );
    assert_eq!(number_field(refused, "guest_requests"), 0);
    assert_eq!(number_field(refused, "protected_operations"), 0);
    Ok(())
}

#[test]
fn wrong_program_interface_and_effectful_proposal_refuse_before_guest()
-> Result<(), Box<dyn std::error::Error>> {
    for source in [
        "def evolve [ [ true ] self.generation swap self.propose drop 1 + ]",
        "def evolve [ [ \"x\" test.emit ] self.generation swap self.propose drop 1 + ]",
    ] {
        let fixture = Fixture::new(source)?;
        let output = Command::new(env!("CARGO_BIN_EXE_noble"))
            .args(["live", "repl", "--source"])
            .arg(&fixture.source)
            .args(["--self-edit", "evolve", "--expect-generation", "1"])
            .stdin(Stdio::null())
            .output()?;
        assert_eq!(output.status.code(), Some(2));
        let reports = String::from_utf8(output.stdout)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            reports.len(),
            1,
            "no grant or guest after static rejection: {reports:?}"
        );
        let refused = &reports[0];
        assert_eq!(
            text_field(refused, "outcome"),
            "reload-refused",
            "{refused}"
        );
        assert_eq!(number_field(refused, "guest_requests"), 0);
        assert_eq!(number_field(refused, "protected_operations"), 0);
        assert_eq!(refused["stack"], serde_json::json!([]));
    }
    Ok(())
}

#[test]
fn grant_for_other_name_cannot_acquire_selected_definition()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def evolve [ 1 + ]")?;
    let output = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["live", "repl", "--source"])
        .arg(&fixture.source)
        .args(["--self-edit", "other", "--expect-generation", "1"])
        .stdin(Stdio::null())
        .output()?;
    assert_eq!(output.status.code(), Some(2));
    let reports = String::from_utf8(output.stdout)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(reports.len(), 2, "{reports:?}");
    live_ack(&reports[0], &[]);
    assert_eq!(
        text_field(&reports[1], "outcome"),
        "reload-refused",
        "{reports:?}"
    );
    assert_eq!(number_field(&reports[1], "guest_requests"), 0);
    assert_eq!(number_field(&reports[1], "protected_operations"), 0);
    Ok(())
}

#[test]
fn ordinary_run_and_session_leave_self_propose_unbound() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("self.propose")?;
    for command in ["run", "session"] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_noble"));
        child.arg(command);
        if command == "run" {
            child.arg(&fixture.source);
        }
        let mut child = child.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
        if command == "session" {
            child
                .stdin
                .as_mut()
                .ok_or("missing session stdin")?
                .write_all(b"self.propose\n")?;
        }
        drop(child.stdin.take());
        let output = child.wait_with_output()?;
        assert_eq!(output.status.code(), Some(2), "{command}");
        let reports = String::from_utf8(output.stdout)?
            .lines()
            .map(serde_json::from_str::<Value>)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(reports.len(), 1, "{command}: {reports:?}");
        assert_eq!(
            text_field(&reports[0], "outcome"),
            "unbound-word",
            "{command}: {reports:?}"
        );
        assert_eq!(number_field(&reports[0], "guest_requests"), 0);
        assert_eq!(number_field(&reports[0], "protected_operations"), 0);
    }
    Ok(())
}

#[test]
fn interpreter_choice_refuses_at_startup_without_v8_fallback()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["live", "repl", "--source"])
        .arg(&fixture.source)
        .args(["--engine", "interpreter"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Some(status) = child.try_wait()? {
            let output = child.wait_with_output()?;
            assert_eq!(
                status.code(),
                Some(2),
                "interpreter choice unexpectedly ran"
            );
            let stdout = String::from_utf8(output.stdout)?;
            let reports = stdout
                .lines()
                .map(serde_json::from_str::<Value>)
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(
                reports.len(),
                1,
                "expected one engine refusal, not a V8 fallback: {stdout}"
            );
            let refusal = &reports[0];
            assert_eq!(text_field(refusal, "schema"), "noble-live-report/v1");
            assert_eq!(text_field(refusal, "stage"), "engine-selection");
            assert_eq!(text_field(refusal, "outcome"), "interpreter-unavailable");
            assert!(!text_field(refusal, "diagnostic").is_empty());
            assert_eq!(number_field(refusal, "guest_requests"), 0);
            assert_eq!(number_field(refusal, "protected_operations"), 0);
            return Ok(());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("interpreter selection did not fail closed at startup".into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(all(target_os = "linux", feature = "live-test-barrier"))]
#[test]
fn old_active_wasm_frame_finishes_before_queued_reload_ack()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new("def addone [ self.generation drop 1 + ]")?;
    let mut barrier = LiveBarrier::new(&fixture.directory, "in-flight-generation", "1")?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_noble"));
    command.args(["live", "repl", "--source"]).arg(&fixture.source)
        .args(["--engine", "v8", "--self-edit", "addone", "--expect-generation", "1"]);
    barrier.configure(&mut command);
    let mut live = LiveChild::spawn(command)?;
    let (old_generation, old_sha256) = live_ack(&live.report()?, &[]);
    assert_eq!(old_generation, 1);
    let grant = live.report()?;
    assert_eq!(text_field(&grant, "outcome"), "grant-selected", "{grant}");
    assert_eq!(text_field(&grant, "name"), "addone");

    // No source mutation or queued reload before the actual Wasm frame
    // reaches its host import. The child cannot finish this invocation until
    // the test explicitly releases the private FIFO.
    let stdin = live.stdin.as_mut().ok_or("missing live stdin")?;
    stdin.write_all(b"20 addone\n")?;
    stdin.flush()?;
    let frame = barrier.wait()?;
    assert_eq!(text_field(&frame, "source_generation"), "1");
    assert_eq!(frame["guest_frame_active"], true, "{frame}");
    fixture.replace("def addone [ self.generation drop 2 + ]")?;
    stdin.write_all(format!(":reload {}\n", fixture.source.display()).as_bytes())?;
    stdin.flush()?;
    barrier.release()?;

    let old = live.report()?;
    normal(&old, &["21"]);
    assert_eq!(number_field(&old, "source_generation"), old_generation);
    assert!(number_field(&old, "guest_requests") >= 1, "{old}");
    assert!(old["request_trace"].as_array()
        .ok_or("old frame omitted guest request trace")?
        .iter().any(|entry| entry == "live.observe-generation:1"), "{old}");
    let ack = live.report()?;
    let (new_generation, new_sha256) = live_ack(&ack, &["21"]);
    assert_eq!(new_generation, old_generation + 1);
    assert_ne!(new_sha256, old_sha256);
    assert_eq!(number_field(&ack, "guest_requests"), 0);
    let old_guest_wasm = text_field(&old["module"], "wasm_sha256");
    assert_eq!(old_guest_wasm.len(), 64);
    let regrant = live.submit(":grant-self-edit addone 2")?;
    assert_eq!(text_field(&regrant, "outcome"), "grant-selected", "{regrant}");
    assert_eq!(number_field(&regrant, "source_generation"), new_generation);
    let new = live.submit("20 addone")?;
    normal(&new, &["21", "22"]);
    assert_eq!(number_field(&new, "source_generation"), new_generation);
    let new_guest_wasm = text_field(&new["module"], "wasm_sha256");
    assert_eq!(new_guest_wasm.len(), 64);
    println!("{}", serde_json::json!({
        "case": "old-active-frame-queued-reload",
        "barrier_guest_frame_active": frame["guest_frame_active"],
        "old_source_sha256": old_sha256,
        "old_source_generation": old_generation,
        "new_source_sha256": new_sha256,
        "ack_generation": new_generation,
        "old_guest_wasm_sha256": old_guest_wasm,
        "new_guest_wasm_sha256": new_guest_wasm,
        "old_guest_value": old["stack"][0]["value"],
        "new_guest_value": new["stack"][1]["value"],
    }));
    live.finish()
}

#[cfg(all(target_os = "linux", feature = "live-test-barrier"))]
#[test]
fn compiled_candidate_a_is_superseded_by_fsynced_rename_b_without_ack()
-> Result<(), Box<dyn std::error::Error>> {
    // These are SHA-256 digests of precisely the two byte strings below
    // (without trailing newlines), not values produced by the worker hook.
    const A: &str = "def addone [ 2 + ]";
    const A_SHA256: &str = "4e49ee40f4b4a51eff90e9a7820d2bdc1d9b12f8b8c558adeff5ba741075e9ad";
    const B: &str = "def addone [ 3 + ]";
    const B_SHA256: &str = "eb81fb3297b696317d1de013abdf5423a31c3708443b1d1a3d8977e0bf5e92e2";
    let fixture = Fixture::new("def addone [ 1 + ]")?;
    let mut barrier = LiveBarrier::new(&fixture.directory, "candidate-compiled", A_SHA256)?;
    let mut command = Command::new(env!("CARGO_BIN_EXE_noble"));
    command.args(["live", "watch", "--source"]).arg(&fixture.source)
        .args(["--engine", "v8"]);
    barrier.configure(&mut command);
    let mut live = LiveChild::spawn(command)?;
    let (initial_generation, _) = live_ack(&live.report()?, &[]);
    fixture.replace(A)?;
    let staged_a = barrier.wait()?;
    assert_eq!(text_field(&staged_a, "source_generation"),
        initial_generation.to_string());
    assert_eq!(staged_a["guest_frame_active"], false, "{staged_a}");
    assert_eq!(staged_a["checked_wasm_module"], true, "{staged_a}");
    let a_wasm_sha256 = text_field(&staged_a, "checked_wasm_sha256");
    assert_eq!(a_wasm_sha256.len(), 64);
    // A's checked WebAssembly.Module now exists, while stage_binary has not
    // replied. This second complete, fsynced atomic save must defeat A's
    // selected-file/sequence CAS and be the only accepted watch ACK.
    fixture.replace(B)?;
    File::open(&fixture.directory)?.sync_all()?;
    barrier.release()?;
    let mut b_ack = None;
    for _ in 0..4 {
        let report = live.report()?;
        match text_field(&report, "outcome") {
            "reload-refused" => {
                assert_eq!(refused_reload(&report, &[]), initial_generation);
                assert!(text_field(&report, "diagnostic").contains("superseded"),
                    "staged A must be discarded: {report}");
            }
            "watch-superseded" => {
                assert_ne!(text_field(&report, "source_sha256"), A_SHA256,
                    "A was published despite B's completed rename: {report}");
            }
            "reload-committed" => {
                assert_eq!(text_field(&report, "source_sha256"), B_SHA256,
                    "only candidate B can be acknowledged: {report}");
                b_ack = Some(report);
                break;
            }
            other => panic!("unexpected report while A was superseded: {other}: {report}"),
        }
    }
    let ack = b_ack.ok_or("candidate B received no ACK after A was staged")?;
    let generation = number_field(&ack, "generation");
    assert_eq!(generation, initial_generation + 1);
    assert!(number_field(&ack, "watch_sequence") >= 2, "{ack}");
    assert_eq!(text_field(&ack, "source_freshness"), "matched-at-watch-ack-check");
    assert_eq!(number_field(&ack, "guest_requests"), 0);

    // An in-place partial write cannot form a watched complete save; the
    // selected-byte gate refuses it before any guest request. Noncooperative
    // MAP_SHARED drift is independently exercised by the existing external
    // C helper + real-child tools/live-mmap-smoke.mjs, not by unsafe Rust.
    fs::write(&fixture.source, "def addone [ 4 +")?;
    let partial = live.submit("20 addone")?;
    assert_source_change_refused(&partial, generation, &[], B_SHA256);
    let partial_sha256 = text_field(&partial, "observed_source_sha256").to_owned();
    assert_eq!(partial_sha256.len(), 64);
    assert_ne!(partial_sha256, B_SHA256);
    fs::write(&fixture.source, B)?;
    let after_restore = live.submit("20 addone")?;
    normal(&after_restore, &["23"]);
    assert_eq!(number_field(&after_restore, "source_generation"), generation);
    let b_guest_wasm = text_field(&after_restore["module"], "wasm_sha256");
    assert_eq!(b_guest_wasm.len(), 64);
    let inspected = live.submit(":generation")?;
    assert_generation_inspection(&inspected, generation, &after_restore["stack"]);
    println!("{}", serde_json::json!({
        "case": "staged-candidate-superseded-by-complete-rename",
        "compiled_candidate_a_sha256": A_SHA256,
        "compiled_candidate_a_wasm_sha256": a_wasm_sha256,
        "accepted_candidate_b_sha256": B_SHA256,
        "accepted_b_guest_wasm_sha256": b_guest_wasm,
        "watch_sequence": number_field(&ack, "watch_sequence"),
        "ack_generation": generation,
        "partial_in_place_sha256": partial_sha256,
        "restored_source_generation": number_field(&after_restore, "source_generation"),
        "guest_value": after_restore["stack"][0]["value"],
    }));
    live.finish()
}
