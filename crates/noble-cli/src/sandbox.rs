//! Linux process boundary. Source elaboration never has an unsandboxed fallback.

mod launch;
mod process;
mod runtime;

pub const PIN: &str = "leanprover/lean4:v4.31.0";
pub const COMMIT: &str = "68218e876d2a38b1985b8590fff244a83c321783";
pub const OUTPUT_LIMIT: usize = 131_072;
pub const ARTIFACT_LIMIT: u64 = 33_554_432;
pub const MEMORY_LIMIT: u64 = 2_147_483_648;

pub struct Mount {
    pub host: std::path::PathBuf,
    pub guest: std::string::String,
    pub writable: bool,
}

pub struct Environment {
    pub lean: std::path::PathBuf,
    pub bwrap: std::path::PathBuf,
    pub prlimit: std::path::PathBuf,
    pub systemd_run: std::path::PathBuf,
    lean_root: std::path::PathBuf,
    runtime_directory: std::path::PathBuf,
    runtime_roots: std::vec::Vec<std::path::PathBuf>,
}

pub struct Transcript {
    pub status: std::process::ExitStatus,
    pub stdout: std::string::String,
    pub stderr: std::string::String,
}

#[allow(
    tigerstyle::ambient_env,
    reason = "Owner: noble-maintainers. Consumer-selected tool paths are captured only at the CLI boundary."
)]
pub fn configured_path(key: &str, default: &std::path::Path) -> std::path::PathBuf {
    match std::env::var_os(key) {
        Some(path) => std::path::PathBuf::from(path),
        None => default.to_path_buf(),
    }
}

#[allow(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers. The process boundary must observe monotonic time to enforce the shared wall-clock deadline against real child processes."
)]
pub(super) fn now() -> std::time::Instant {
    std::time::Instant::now()
}

impl Environment {
    pub fn discover(
        deadline: std::time::Instant,
    ) -> Result<Self, crate::workflow::output::Failure> {
        if !cfg!(target_os = "linux") {
            return Err(crate::workflow::output::Failure::unsupported(
                "sandbox-unavailable",
                "MC1 source verification requires the Linux bubblewrap sandbox".into(),
            ));
        }
        // Fail before touching proof code if isolation cannot even be selected.
        let bwrap = attempt!(runtime::executable(
            "NOBLE_BWRAP",
            &[
                "/run/current-system/sw/bin/bwrap",
                "/usr/bin/bwrap",
                "/bin/bwrap",
            ]
        ));
        let prlimit = attempt!(runtime::executable(
            "NOBLE_PRLIMIT",
            &[
                "/run/current-system/sw/bin/prlimit",
                "/usr/bin/prlimit",
                "/bin/prlimit",
            ]
        ));
        let systemd_run = attempt!(runtime::executable(
            "NOBLE_SYSTEMD_RUN",
            &[
                "/run/current-system/sw/bin/systemd-run",
                "/usr/bin/systemd-run",
                "/bin/systemd-run",
            ]
        ));
        let runtime_directory = configured_path(
            "XDG_RUNTIME_DIR",
            std::path::Path::new("/nonexistent/noble-user-runtime"),
        );
        if !runtime_directory.join("bus").exists() {
            return Err(crate::workflow::output::Failure::unsupported(
                "sandbox-unavailable",
                "an active user systemd manager is required for aggregate proof-worker limits"
                    .into(),
            ));
        }
        let (lean, lean_root) = attempt!(runtime::installation());
        let roots = attempt!(runtime::roots(&lean_root, deadline));
        Ok(Self {
            lean,
            bwrap,
            prlimit,
            systemd_run,
            lean_root,
            runtime_directory,
            runtime_roots: roots,
        })
    }
}
