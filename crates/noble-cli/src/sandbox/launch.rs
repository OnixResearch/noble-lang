const ISOLATION_ARGUMENTS: &[&str] = &[
    "--unshare-all",
    "--die-with-parent",
    "--new-session",
    "--cap-drop",
    "ALL",
    "--clearenv",
    "--proc",
    "/proc",
    "--dev",
    "/dev",
    "--size",
    "1048576",
    "--tmpfs",
    "/dev/shm",
    "--remount-ro",
    "/dev",
    "--size",
    "67108864",
    "--tmpfs",
    "/tmp",
    "--dir",
    "/inputs",
    "--dir",
    "/out",
    "--dir",
    "/library",
    "--dir",
    "/obligation",
    "--dir",
    "/producer",
    "--dir",
    "/checker",
    "--chdir",
    "/tmp",
    "--setenv",
    "HOME",
    "/tmp",
    "--setenv",
    "TMPDIR",
    "/tmp",
    "--setenv",
    "LANG",
    "C.UTF-8",
    "--setenv",
    "LEAN_PATH",
];

impl super::Environment {
    pub fn run(
        &self,
        mounts: &[super::Mount],
        search_path: &str,
        arguments: &[std::string::String],
        deadline: std::time::Instant,
    ) -> Result<super::Transcript, crate::workflow::output::Failure> {
        let remaining = deadline.saturating_duration_since(super::now());
        if remaining.is_zero() {
            return Err(crate::workflow::output::Failure::timeout(
                "consumer wall-clock budget exhausted",
            ));
        }
        let seconds = remaining.as_secs().saturating_add(1);
        let mut command = self.scoped_command(seconds);
        command
            .args(ISOLATION_ARGUMENTS)
            .arg(search_path)
            .args(["--setenv", "LEAN_SYSROOT"])
            .arg(&self.lean_root)
            .args(["--setenv", "NOBLE_LEAN_EXECUTABLE"])
            .arg(&self.lean);
        self.bind_inputs(&mut command, mounts);
        // TasksMax covers the complete worker cgroup; RLIMIT_NPROC would also
        // count unrelated host-user tasks across nested user namespaces.
        command
            .args(["--remount-ro", "/", "--"])
            .arg(&self.lean)
            .args(arguments);
        super::process::capture(command, deadline)
    }

    fn scoped_command(&self, seconds: u64) -> std::process::Command {
        let mut command = std::process::Command::new(&self.systemd_run);
        command
            .env_clear()
            .env("XDG_RUNTIME_DIR", &self.runtime_directory)
            .args(["--user", "--scope", "--quiet", "--collect", "-p"])
            .arg(std::format!("MemoryMax={}", super::MEMORY_LIMIT))
            .args(["-p", "MemorySwapMax=0", "-p", "TasksMax=64", "--"])
            .arg(&self.prlimit)
            .arg(std::format!("--fsize={0}:{0}", super::ARTIFACT_LIMIT))
            .arg("--nofile=128:128")
            .arg("--core=0:0")
            .arg(std::format!(
                "--cpu={seconds}:{}",
                seconds.saturating_add(1)
            ))
            .arg("--")
            .arg(&self.bwrap);
        command
    }

    fn bind_inputs(&self, command: &mut std::process::Command, mounts: &[super::Mount]) {
        // Expose only the selected immutable toolchain closure, never the whole
        // Nix store or project. Conventional installations use library roots.
        self.runtime_roots.iter().for_each(|root| {
            command.arg("--ro-bind").arg(root).arg(root);
        });
        mounts.iter().for_each(|mount| {
            command
                .arg(if mount.writable {
                    "--bind"
                } else {
                    "--ro-bind"
                })
                .arg(&mount.host)
                .arg(&mount.guest);
        });
    }
}
