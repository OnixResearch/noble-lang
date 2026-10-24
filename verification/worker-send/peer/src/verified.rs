use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::os::{fd::AsRawFd, unix::{fs::{DirBuilderExt, OpenOptionsExt}, process::CommandExt}};
use std::{fmt, fs, io::{self, Read, Write}, path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering}, time::{Duration, Instant}};

static NEXT_FILE: AtomicU64 = AtomicU64::new(1);
const REPORT_LIMIT: usize = 131_072;
const STDERR_LIMIT: usize = 8_192;
const SELECTION_DEADLINE: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq, Eq)]
enum SelectionFailure { StdoutLimit, StderrLimit, Deadline }
impl fmt::Display for SelectionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::StdoutLimit => "checked worker stdout exceeded peer bound",
            Self::StderrLimit => "checked worker stderr exceeded peer bound",
            Self::Deadline => "selected Noble typed worker exceeded deadline",
        })
    }
}
impl std::error::Error for SelectionFailure {}

/// The host-selected Submit transition. The Cancelled transition is a
/// separate step so the shell can commit cancellation before supplying it.
pub struct Submitted {
    pub attempt: i64,
    pub text: String,
    pub source_sha256: String,
    pub first: Value,
    files: SelectedFiles,
    cli: PathBuf,
    package: PathBuf,
    schema: PathBuf,
    returned_state: PathBuf,
    cancel: PathBuf,
    package_sha256: String,
    schema_sha256: String,
}

// The private directory pins the bytes handed to both checked CLI runs.
// Keeping caller-controlled paths here would allow substitution between runs.
struct SelectedFiles(PathBuf);
impl SelectedFiles {
    fn new() -> Result<Self> {
        let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("noble-worker-send-selected-{}-{id}",
            std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&path)
            .context("creating private selected-input directory")?;
        Ok(Self(path))
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<PathBuf> {
        let path = self.0.join(name);
        let mut file = fs::OpenOptions::new().write(true).create_new(true).mode(0o600)
            .open(&path).context("creating private selected input")?;
        file.write_all(bytes).context("writing private selected input")?;
        Ok(path)
    }

    fn snapshot(&self, name: &str, path: &Path, limit: u64) -> Result<PathBuf> {
        let bytes = read_bounded(path, limit).context("reading selected input")?;
        self.write(name, &bytes)
    }

    fn snapshot_hashed(&self, name: &str, path: &Path, limit: u64)
        -> Result<(PathBuf, String)> {
        let bytes = read_bounded(path, limit).context("reading selected input")?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        Ok((self.write(name, &bytes)?, sha256))
    }
}
impl Drop for SelectedFiles {
    fn drop(&mut self) {
        for name in ["package", "schema", "initial-state", "submit", "cancel", "returned-state"] {
            let _ = fs::remove_file(self.0.join(name));
        }
        let _ = fs::remove_dir(&self.0);
    }
}

// An owned process group is killed before its leader is reaped; this also
// stops descendants retaining either pipe after the direct child exits.
struct SelectedChild {
    child: Option<Child>,
    group: libc::pid_t,
}
impl SelectedChild {
    fn spawn(command: &mut Command) -> Result<Self> {
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .process_group(0);
        let mut child = command.spawn().context("selected Noble typed worker failed to start")?;
        let group = match libc::pid_t::try_from(child.id()) {
            Ok(group) if group > 0 => group,
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                bail!("selected worker process identifier is out of range");
            }
        };
        Ok(Self { child: Some(child), group })
    }

    // WNOWAIT keeps the process-group identifier reserved until kill/reap.
    fn exited(&self) -> io::Result<bool> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe { libc::waitid(libc::P_PID, self.group as libc::id_t,
            &mut info, libc::WEXITED | libc::WNOHANG | libc::WNOWAIT) };
        if result == -1 { return Err(io::Error::last_os_error()); }
        Ok(unsafe { info.si_pid() } != 0)
    }

    fn stop_and_wait(mut self) -> io::Result<ExitStatus> {
        // The leader has exited, so killing its group cannot turn a
        // successful checked worker into an unsuccessful one.
        self.stop_group();
        let status = self.child.as_mut().expect("owned selected child").wait();
        if status.is_ok() { self.child.take(); }
        status
    }

    fn stop_group(&self) {
        unsafe { libc::kill(-self.group, libc::SIGKILL); }
    }
}
impl Drop for SelectedChild {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else { return };
        self.stop_group();
        let _ = child.kill();
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline =>
                    std::thread::sleep(Duration::from_millis(10)),
                _ => break,
            }
        }
        // A kernel-stuck process cannot be synchronously reaped within a
        // bounded deadline. Move its handle to a background reaper. If even
        // thread creation fails, dropping the killed child handle lets the
        // peer exit rather than blocking forever; the OS reaps at parent exit.
        let _=std::thread::Builder::new().name("selected-worker-reaper".into())
            .spawn(move || {let _=child.wait();});
    }
}

fn nonblocking(fd: &impl AsRawFd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    if flags == -1 { return Err(io::Error::last_os_error()); }
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn drain(pipe: &mut impl Read, data: &mut Vec<u8>, limit: usize) -> Result<bool> {
    let mut buffer = [0; 8192];
    match pipe.read(&mut buffer) {
        Ok(0) => Ok(false),
        Ok(n) => {
            if n > limit.saturating_sub(data.len()) {
                return Err(SelectionFailure::StdoutLimit.into());
            }
            data.extend_from_slice(&buffer[..n]);
            Ok(true)
        }
        Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) =>
            Ok(true),
        Err(error) => Err(error).context("reading checked worker stdout"),
    }
}

fn drain_stderr(pipe: &mut impl Read, count: &mut usize) -> Result<bool> {
    // Count but do not retain untrusted diagnostics in peer error reports.
    let mut buffer = [0; 8192];
    match pipe.read(&mut buffer) {
        Ok(0) => Ok(false),
        Ok(n) => {
            if n > STDERR_LIMIT.saturating_sub(*count) {
                return Err(SelectionFailure::StderrLimit.into());
            }
            *count += n;
            Ok(true)
        }
        Err(error) if matches!(error.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted) =>
            Ok(true),
        Err(error) => Err(error).context("reading checked worker stderr"),
    }
}

fn collect(command: &mut Command, deadline: Instant) -> Result<Vec<u8>> {
    collect_spawned(SelectedChild::spawn(command)?, deadline)
}

fn collect_spawned(mut process: SelectedChild, deadline: Instant) -> Result<Vec<u8>> {
    let child = process.child.as_mut().expect("owned selected child");
    let mut stdout = child.stdout.take().context("selected worker stdout absent")?;
    let mut stderr = child.stderr.take().context("selected worker stderr absent")?;
    nonblocking(&stdout).context("configuring selected worker stdout")?;
    nonblocking(&stderr).context("configuring selected worker stderr")?;
    let mut out = Vec::new();
    let mut stderr_bytes = 0;
    let mut out_open = true;
    let mut err_open = true;
    loop {
        if Instant::now() >= deadline {
            return Err(SelectionFailure::Deadline.into());
        }
        let mut fds = [libc::pollfd { fd: -1, events: libc::POLLIN, revents: 0 }; 2];
        let mut count = 0;
        if out_open {
            fds[count].fd = stdout.as_raw_fd();
            count += 1;
        }
        if err_open {
            fds[count].fd = stderr.as_raw_fd();
            count += 1;
        }
        let stderr_index = usize::from(out_open);
        let timeout = deadline.saturating_duration_since(Instant::now())
            .min(Duration::from_millis(50)).as_millis().max(1) as libc::c_int;
        let polled = unsafe { libc::poll(fds.as_mut_ptr(), count as libc::nfds_t, timeout) };
        if polled == -1 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted { continue; }
            return Err(error).context("polling checked worker output");
        }
        if out_open && fds[0].revents != 0 {
            out_open = drain(&mut stdout, &mut out, REPORT_LIMIT)?;
        }
        if err_open && fds[stderr_index].revents != 0 {
            err_open = drain_stderr(&mut stderr, &mut stderr_bytes)?;
        }
        if !out_open && !err_open && process.exited()
            .context("observing selected worker exit")? {
            let status = process.stop_and_wait().context("reaping selected worker")?;
            ensure!(status.success(), "checked worker refused selected State/Event");
            return Ok(out);
        }
    }
}

fn selected(cli: &Path, package: &Path, schema: &Path, state: &Path, event: &Path,
            package_sha256: &str, schema_sha256: &str, deadline: Instant) -> Result<Value> {
    let mut command = Command::new(cli);
    command.args(["worker", "typed"]).arg(package)
        .arg("--schema").arg(schema).arg("--state").arg(state)
        .arg("--event").arg(event);
    let out = collect(&mut command, deadline)?;
    let report: Value = serde_json::from_slice(&out).context("checked worker report is not JSON")?;
    ensure!(report["schema"] == "noble-worker-typed/v1" && report["outcome"] == "normal"
        && report["stage"] == "execution" && report["guest_requests"] == 0
        && report["protected_operations"] == 0,
        "selected guest did not produce a pure compiled worker transition");
    checked_identity(&report, package_sha256, schema_sha256)?;
    Ok(report)
}

fn digest(value: &Value) -> bool {
    value.as_str().is_some_and(|text| text.len() == 64 && text.bytes().all(|byte|
        byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
}

fn decimal_identity(value: &Value) -> bool {
    value.as_str().and_then(|text| text.parse::<u64>().ok()
        .map(|number| number.to_string() == text)).unwrap_or(false)
}

fn checked_identity(report: &Value, package_sha256: &str, schema_sha256: &str) -> Result<()> {
    let owner = report["checked_program_owner"].as_object()
        .context("selected worker did not retain a checked Program owner")?;
    ensure!(owner.len() == 5
        && owner.get("module_id").is_some_and(decimal_identity)
        && owner.get("version").and_then(Value::as_u64)
            .is_some_and(|number| u32::try_from(number).is_ok())
        && owner.get("definition_ordinal").and_then(Value::as_u64)
            .is_some_and(|number| u32::try_from(number).is_ok())
        && owner.get("definition_identity").is_some_and(decimal_identity)
        && owner.get("checked_named_slot").and_then(Value::as_u64)
            .is_some_and(|number| u32::try_from(number).is_ok())
        && digest(&report["package_sha256"])
        && digest(&report["schema_source_sha256"])
        && digest(&report["source_sha256"])
        && digest(&report["module"]["wasm_sha256"])
        && report["package_sha256"] == package_sha256
        && report["schema_source_sha256"] == schema_sha256
        && report["module"]["source_sha256"] == report["source_sha256"]
        && report["module"]["worker_catalog"]["schema_source_sha256"]
            == report["schema_source_sha256"],
        "selected worker package, schema, source or checked owner was not bound");
    Ok(())
}

fn same_checked_identity(first: &Value, second: &Value) -> Result<()> {
    ensure!(first["package_sha256"] == second["package_sha256"]
        && first["schema_source_sha256"] == second["schema_source_sha256"]
        && first["source_sha256"] == second["source_sha256"]
        && first["module"]["wasm_sha256"] == second["module"]["wasm_sha256"]
        && first["checked_program_owner"] == second["checked_program_owner"],
        "checked cancellation used a different package, schema, source or Program owner");
    Ok(())
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

pub fn submit(cli: &Path, package: &Path, schema: &Path, state: &Path,
              submit: &Path, cancel: &Path) -> Result<Submitted> {
    let deadline = Instant::now().checked_add(SELECTION_DEADLINE)
        .context("selected worker deadline overflow")?;
    let files = SelectedFiles::new()?;
    let (package, package_sha256) = files.snapshot_hashed("package", package, 262_144)?;
    let (schema, schema_sha256) = files.snapshot_hashed("schema", schema, 65_536)?;
    let state = files.snapshot("initial-state", state, 65_536)?;
    let submit = files.snapshot("submit", submit, 65_536)?;
    let cancel = files.snapshot("cancel", cancel, 65_536)?;
    let first = selected(cli, &package, &schema, &state, &submit,
        &package_sha256, &schema_sha256, deadline)?;
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
        &read_bounded(&cancel,65_536).context("reading selected Cancelled frame")?)?;
    ensure!(cancellation["value"]["tag"] == "variant"
        && cancellation["value"]["arm"] == "right"
        && positive(&cancellation["value"]["value"]["value"]["value"])? == attempt,
        "host-selected cancellation differs from admitted Send");
    let next_state = serde_json::to_vec(&json!({"format":"noble-worker-value/v1",
        "expected_schema":output_state["schema"],"value":output_state}))?;
    let returned_state = files.write("returned-state", &next_state)?;
    let source_sha256 = first["source_sha256"].as_str()
        .filter(|value| value.len() == 64).context("missing checked source digest")?.to_owned();
    if text.len() > noble_kernel::authority::MAX_ARGUMENT_BYTES - 8 {
        bail!("checked Send exceeds the host operation argument bound");
    }
    Ok(Submitted { attempt, text, source_sha256, first, files, cli: cli.to_owned(),
        package, schema, returned_state, cancel, package_sha256, schema_sha256 })
}

impl Submitted {
    /// Supply the host-selected Cancelled transition for the admitted attempt.
    /// Its own deadline starts when the shell actually requests it.
    pub fn cancelled(&self) -> Result<Value> {
        let deadline = Instant::now().checked_add(SELECTION_DEADLINE)
            .context("selected worker deadline overflow")?;
        let second = selected(&self.cli, &self.package, &self.schema, &self.returned_state,
            &self.cancel, &self.package_sha256, &self.schema_sha256, deadline)?;
        same_checked_identity(&self.first, &second)?;
        ensure!(second["typed_output"]["state"]["value"]["left"]["value"] == "1"
            && second["typed_output"]["state"]["value"]["right"]["arm"] == "left"
            && second["typed_output"]["actions"]["items"].as_array().is_some_and(Vec::is_empty),
            "checked cancellation did not clear the exact pending attempt");
        Ok(second)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child_command(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "verified::tests::selected_child_fixture",
            "--ignored", "--nocapture"])
            .env("NOBLE_WORKER_SEND_SELECTED_CHILD", mode);
        command
    }

    #[test]
    #[ignore = "only invoked by selected child boundary tests"]
    fn selected_child_fixture() {
        let mode = std::env::var("NOBLE_WORKER_SEND_SELECTED_CHILD")
            .expect("selected child fixture requires an explicit mode");
        match mode.as_str() {
            "stdout" => {
                let mut output = io::stdout().lock();
                loop { output.write_all(&[b'x'; 1024]).unwrap(); }
            }
            "stderr" => {
                let mut output = io::stderr().lock();
                loop { output.write_all(&[b'x'; 1024]).unwrap(); }
            }
            "parent" => {
                let mut child = child_command("descendant");
                child.env("NOBLE_WORKER_SEND_READY", std::env::var_os("NOBLE_WORKER_SEND_READY").unwrap())
                    .env("NOBLE_WORKER_SEND_RELEASE", std::env::var_os("NOBLE_WORKER_SEND_RELEASE").unwrap())
                    .env("NOBLE_WORKER_SEND_WITNESS", std::env::var_os("NOBLE_WORKER_SEND_WITNESS").unwrap());
                child.spawn().unwrap().wait().unwrap();
            }
            "descendant" => {
                let ready = PathBuf::from(std::env::var_os("NOBLE_WORKER_SEND_READY").unwrap());
                let release = PathBuf::from(std::env::var_os("NOBLE_WORKER_SEND_RELEASE").unwrap());
                let witness = PathBuf::from(std::env::var_os("NOBLE_WORKER_SEND_WITNESS").unwrap());
                fs::write(ready, b"ready").unwrap();
                while !release.exists() { std::thread::sleep(Duration::from_millis(5)); }
                fs::write(witness, b"escaped").unwrap();
            }
            _ => panic!("unknown selected child fixture"),
        }
    }

    #[test]
    fn stdout_cap_stops_a_live_child_without_waiting_for_its_deadline() {
        let error = collect(&mut child_command("stdout"),
            Instant::now() + Duration::from_secs(5)).unwrap_err();
        assert_eq!(error.downcast::<SelectionFailure>().unwrap(), SelectionFailure::StdoutLimit);
    }

    #[test]
    fn stderr_cap_stops_a_live_child_without_waiting_for_its_deadline() {
        let error = collect(&mut child_command("stderr"),
            Instant::now() + Duration::from_secs(5)).unwrap_err();
        assert_eq!(error.downcast::<SelectionFailure>().unwrap(), SelectionFailure::StderrLimit);
    }

    #[test]
    fn deadline_stops_a_child_and_descendant_holding_the_output_pipes() {
        let files = SelectedFiles::new().unwrap();
        let ready = files.0.join("child-ready");
        let release = files.0.join("child-release");
        let witness = files.0.join("escaped-child");
        let mut command = child_command("parent");
        command.env("NOBLE_WORKER_SEND_READY", &ready)
            .env("NOBLE_WORKER_SEND_RELEASE", &release)
            .env("NOBLE_WORKER_SEND_WITNESS", &witness);
        let process = SelectedChild::spawn(&mut command).unwrap();
        let startup_deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() {
            assert!(Instant::now() < startup_deadline, "selected descendant did not start");
            std::thread::sleep(Duration::from_millis(5));
        }
        fs::remove_file(&ready).unwrap();
        let result = collect_spawned(process, Instant::now() + Duration::from_millis(100));
        fs::write(&release, b"go").unwrap();
        std::thread::sleep(Duration::from_millis(500));
        let escaped = witness.exists();
        fs::remove_file(&release).unwrap();
        if escaped { fs::remove_file(&witness).unwrap(); }
        let error = result.unwrap_err();
        assert_eq!(error.downcast::<SelectionFailure>().unwrap(), SelectionFailure::Deadline);
        assert!(!escaped);
    }

    #[test]
    fn original_package_path_replacement_does_not_change_selected_bytes() {
        let originals = SelectedFiles::new().unwrap();
        let package = originals.write("package", b"checked original package").unwrap();
        let replacement = originals.write("schema", b"swapped package").unwrap();
        let selected = SelectedFiles::new().unwrap();
        let (pinned, selected_sha256) = selected.snapshot_hashed("package", &package, 262_144)
            .unwrap();
        fs::rename(&replacement, &package).unwrap();
        assert_eq!(read_bounded(&package, 262_144).unwrap(), b"swapped package");
        assert_eq!(read_bounded(&pinned, 262_144).unwrap(), b"checked original package");
        assert_eq!(selected_sha256, format!("{:x}", Sha256::digest(b"checked original package")));
        assert_ne!(selected_sha256, format!("{:x}", Sha256::digest(b"swapped package")));
    }
}
