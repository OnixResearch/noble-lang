use std::io::Write as _;

static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Clone, Copy)]
pub struct Backend(&'static str);

impl Backend {
    pub const V8: Self = Self("v8");
    pub const WASMTIME_CORE: Self = Self("wasmtime-core");

    pub const fn as_flag(self) -> &'static str {
        self.0
    }
}

#[derive(Clone, Copy)]
pub struct OptMode {
    flag: &'static str,
    wasm_name: &'static str,
}

impl OptMode {
    pub const OFF: Self = Self {
        flag: "off",
        wasm_name: "module-1.wasm",
    };
    pub const ON: Self = Self {
        flag: "on",
        wasm_name: "module-1-optimized.wasm",
    };

    pub const fn as_flag(self) -> &'static str {
        self.flag
    }

    pub const fn wasm_filename(self) -> &'static str {
        self.wasm_name
    }
}

pub struct TempDir(std::path::PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Result<Self, String> {
        assert!(!label.is_empty(), "temp dir label must not be empty");
        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
        let path = base.join(format!(
            "noble-wasmtime-plan-{label}-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).map_err(|error| format!("create temp dir: {error}"))?;
        assert!(path.is_dir(), "created temp path must be a directory");
        Ok(Self(path))
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        match std::fs::remove_dir_all(&self.0) {
            Ok(()) => {}
            Err(error) => eprintln!("cleanup {}: {error}", self.0.display()),
        }
    }
}

pub fn run_noble(
    args: &[&str],
    stdin_data: Option<&[u8]>,
) -> Result<(i32, Vec<serde_json::Value>, String), String> {
    assert!(!args.is_empty(), "noble CLI invocation requires arguments");
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_noble"));
    command
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if stdin_data.is_some() {
        command.stdin(std::process::Stdio::piped());
    } else {
        command.stdin(std::process::Stdio::null());
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("spawn noble: {error}"))?;
    if let Some(bytes) = stdin_data {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| String::from("missing child stdin"))?;
        stdin
            .write_all(bytes)
            .map_err(|error| format!("write child stdin: {error}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait noble: {error}"))?;
    assert!(
        output.status.code().is_some(),
        "noble child terminated by signal"
    );
    let code = output.status.code().unwrap_or(-1);
    let stdout =
        String::from_utf8(output.stdout).map_err(|error| format!("utf8 stdout: {error}"))?;
    let stderr =
        String::from_utf8(output.stderr).map_err(|error| format!("utf8 stderr: {error}"))?;
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let mut reports = Vec::with_capacity(lines.len());
    for line in lines {
        assert!(
            reports.len() < 4096,
            "report count exceeded harness capacity"
        );
        let parsed: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| format!("invalid report JSON ({error}): {line}"))?;
        reports.push(parsed);
    }
    Ok((code, reports, stderr))
}

pub struct SourceRun<'a> {
    pub temp: &'a TempDir,
    pub name: &'a str,
    pub source: &'a str,
    pub extra_args: &'a [&'a str],
}

pub fn run_source_file(request: SourceRun<'_>) -> Result<(i32, serde_json::Value, String), String> {
    assert!(!request.name.is_empty(), "source filename required");
    assert!(!request.source.is_empty(), "source program required");
    let source_path = request.temp.path().join(request.name);
    std::fs::write(&source_path, request.source)
        .map_err(|error| format!("write source: {error}"))?;
    let source_str = source_path
        .to_str()
        .ok_or_else(|| String::from("non-utf8 source path"))?;
    let mut args = Vec::with_capacity(request.extra_args.len().saturating_add(2));
    args.push("run");
    args.push(source_str);
    args.extend_from_slice(request.extra_args);
    let (code, reports, stderr) = run_noble(&args, None)?;
    if reports.len() != 1 {
        return Err(format!(
            "expected 1 report for {args:?}, got {} (stderr: {stderr})",
            reports.len()
        ));
    }
    Ok((code, reports[0].clone(), stderr))
}

pub fn framed_bytes(submissions: &[&str]) -> Vec<u8> {
    assert!(!submissions.is_empty(), "framed session needs submissions");
    let mut buf = Vec::with_capacity(submissions.len().saturating_mul(16));
    for item in submissions {
        let header = format!("{}\n", item.len());
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(item.as_bytes());
    }
    assert!(!buf.is_empty(), "framed payload must not be empty");
    buf
}

pub fn run_session(
    backend: Backend,
    opt: OptMode,
    submissions: &[&str],
) -> Result<(i32, Vec<serde_json::Value>, String), String> {
    assert!(!submissions.is_empty(), "session requires submissions");
    let stdin = framed_bytes(submissions);
    let (code, reports, stderr) = run_noble(
        &[
            "session",
            "--framed",
            "--engine",
            backend.as_flag(),
            "--opt",
            opt.as_flag(),
        ],
        Some(&stdin),
    )?;
    assert!(!reports.is_empty(), "session produced zero reports");
    Ok((code, reports, stderr))
}
