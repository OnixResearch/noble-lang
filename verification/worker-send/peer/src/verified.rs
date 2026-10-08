use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::os::unix::fs::OpenOptionsExt;
use std::{fs, io::Read, path::Path, process::Command,
    sync::atomic::{AtomicU64, Ordering}};

static NEXT_FILE: AtomicU64 = AtomicU64::new(1);

pub struct Verified {
    pub attempt: i64,
    pub text: String,
    pub source_sha256: String,
    pub first: Value,
    pub second: Value,
}

struct StateFile(std::path::PathBuf);
impl Drop for StateFile {
    fn drop(&mut self) { let _ = fs::remove_file(&self.0); }
}

fn selected(cli: &Path, package: &Path, schema: &Path, state: &Path, event: &Path)
    -> Result<Value> {
    let run = Command::new(cli).args(["worker", "typed"]).arg(package)
        .arg("--schema").arg(schema).arg("--state").arg(state)
        .arg("--event").arg(event).output().context("selected Noble typed worker failed to start")?;
    ensure!(run.status.success(), "checked worker refused selected State/Event");
    ensure!(run.stdout.len() <= 131_072, "checked worker observation exceeded peer bound");
    let report: Value = serde_json::from_slice(&run.stdout).context("checked worker report is not JSON")?;
    ensure!(report["schema"] == "noble-worker-typed/v1" && report["outcome"] == "normal"
        && report["stage"] == "execution" && report["guest_requests"] == 0
        && report["protected_operations"] == 0
        && report["module"]["wasm_sha256"].as_str().is_some(),
        "selected guest did not produce a pure compiled worker transition");
    Ok(report)
}

fn positive(value: &Value) -> Result<i64> {
    let spelling = value.as_str().context("missing decimal attempt identity")?;
    let attempt = spelling.parse::<i64>().context("attempt is not I64")?;
    ensure!(attempt > 0 && attempt.to_string() == spelling, "noncanonical attempt identity");
    Ok(attempt)
}

fn read_bounded(path:&Path,limit:u64)->Result<Vec<u8>> {
    let mut file=fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path)?;
    let metadata=file.metadata()?;
    ensure!(metadata.is_file() && metadata.len()<=limit,"selected frame must be a bounded regular file");
    let mut bytes=vec![0;usize::try_from(metadata.len())?];
    file.read_exact(&mut bytes)?;
    let mut extra=[0];
    ensure!(file.read(&mut extra)?==0,"selected frame grew beyond its byte bound");
    Ok(bytes)
}

pub fn run(cli: &Path, package: &Path, schema: &Path, state: &Path,
           submit: &Path, cancel: &Path) -> Result<Verified> {
    let first = selected(cli, package, schema, state, submit)?;
    let actions = first["typed_output"]["actions"]["items"].as_array()
        .filter(|values| values.len() == 1).context("worker did not return exactly one Send action")?;
    let send = &actions[0];
    ensure!(send["tag"] == "variant" && send["arm"] == "left"
        && send["value"]["tag"] == "pair", "worker action is not checked Send");
    let attempt = positive(&send["value"]["left"]["value"]["value"])?;
    let text = send["value"]["right"]["value"].as_str()
        .context("checked Send has no Text")?.to_owned();
    let output_state = &first["typed_output"]["state"];
    ensure!(output_state["value"]["left"]["value"] == "1"
        && output_state["value"]["right"]["arm"] == "right"
        && positive(&output_state["value"]["right"]["value"]["value"]["value"])? == attempt,
        "checked worker did not retain the exact Send identity");
    let cancellation: Value = serde_json::from_slice(
        &read_bounded(cancel,65_536).context("reading selected Cancelled frame")?)?;
    ensure!(cancellation["value"]["tag"] == "variant"
        && cancellation["value"]["arm"] == "right"
        && positive(&cancellation["value"]["value"]["value"]["value"])? == attempt,
        "host-selected cancellation differs from admitted Send");
    let next_state = serde_json::to_vec(&json!({"format":"noble-worker-value/v1",
        "expected_schema":output_state["schema"],"value":output_state}))?;
    let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
    let name = std::env::temp_dir().join(format!("noble-worker-send-state-{}-{id}",std::process::id()));
    let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&name)
        .context("creating private checked returned State")?;
    let private = StateFile(name);
    std::io::Write::write_all(&mut file, &next_state)?;
    drop(file);
    let second = selected(cli, package, schema, &private.0, cancel)?;
    ensure!(second["source_sha256"] == first["source_sha256"]
        && second["typed_output"]["state"]["value"]["left"]["value"] == "1"
        && second["typed_output"]["state"]["value"]["right"]["arm"] == "left"
        && second["typed_output"]["actions"]["items"].as_array().is_some_and(Vec::is_empty),
        "checked cancellation did not clear the exact pending attempt");
    let source_sha256 = first["source_sha256"].as_str()
        .filter(|value| value.len() == 64).context("missing checked source digest")?.to_owned();
    if text.len() > noble_kernel::authority::MAX_ARGUMENT_BYTES - 8 {
        bail!("checked Send exceeds the host operation argument bound");
    }
    Ok(Verified { attempt, text, source_sha256, first, second })
}
