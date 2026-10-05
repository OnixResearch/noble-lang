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

fn effect_policy_refusal(report: &Value, stack: &[&str], generation: u64) {
    assert_eq!(text_field(report, "schema"), "noble-live-report/v1");
    assert_eq!(text_field(report, "stage"), "admission");
    assert_eq!(text_field(report, "outcome"), "effect-refused");
    assert!(!text_field(report, "diagnostic").is_empty());
    assert_eq!(number_field(report, "generation"), generation);
    assert_stack(report, stack);
    assert_eq!(number_field(report, "guest_requests"), 0);
    assert_eq!(number_field(report, "protected_operations"), 0);
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
    assert!(text_field(&refused, "diagnostic").contains("usage: noble live repl"));
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
    normal(&live.submit("7")?, &["7"]);
    let generation = current_generation(&mut live, &fixture, &["7"])?;

    // Ordinary Core sessions permit test.emit; live's unbound, resource-free
    // profile must refuse it before candidate execution instead.
    effect_policy_refusal(&live.submit("\"x\" test.emit")?, &["7"], generation);
    effect_policy_refusal(
        &live.submit("def emit [ \"x\" test.emit ]")?,
        &["7"],
        generation,
    );
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
    assert_eq!(denied["session_state"], "terminated-after-runtime-failure");
    assert_eq!(
        fs::read_to_string(&fixture.source)?,
        EVOLVE.replace("self.generation", "0")
    );
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
