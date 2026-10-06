pub(crate) mod assembly;
mod configuration;
mod process;
mod protocol;

const NODE: &str = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
const HOST: &str = concat!(
    include_str!("runtime/preamble.mjs"),
    include_str!("runtime/values.mjs"),
    include_str!("runtime/engine-host.mjs"),
    include_str!("runtime/engine.mjs"),
    include_str!("runtime/protocol.mjs"),
);
const SLOT_HOST: &str = concat!(
    include_str!("runtime/preamble.mjs"),
    include_str!("runtime/slot-policy.mjs"),
    include_str!("runtime/slot-registry.mjs"),
    include_str!("runtime/slot-engine.mjs"),
    include_str!("runtime/protocol.mjs"),
);
const SELECTION: &str = include_str!("runtime/config.json");
const ABI: &str = include_str!("runtime/abi.json");
const DECLARED_ABI: &str = include_str!("runtime/declared-abi.json");

pub(super) struct Engine {
    child: std::process::Child,
    replies: std::sync::mpsc::Receiver<Result<super::output::Report, super::output::Failure>>,
    reader: Option<std::thread::JoinHandle<()>>,
    control: Option<std::os::unix::net::UnixDatagram>,
    has_failed: bool,
}

impl Engine {
    /// A fresh engine for exactly one artifact decision; no persistent table
    /// entries or effect authority can survive a previous submission.
    pub fn start_admission(optimized: bool) -> Result<Self, super::output::Failure> {
        let config = configuration::admission(optimized);
        let (send, replies) = std::sync::mpsc::sync_channel(1);
        let mut engine = Self {
            child: attempt!(launch(&config, HOST, None)),
            replies,
            reader: None,
            control: None,
            has_failed: false,
        };
        let stream = attempt!(engine.child.stdout.take().ok_or_else(protocol::error));
        engine.reader = Some(attempt!(std::thread::Builder::new()
            .name("noble-artifact-output".into())
            .spawn(move || protocol::receive(stream, send))
            .map_err(super::framing::io_error)));
        let ready = attempt!(engine.reply());
        if ready.outcome != "ready" {
            engine.has_failed = true;
            return Err(super::output::Failure::new(
                super::output::ErrorContext { stage: "wasm", outcome: "unsupported" },
                ready.json,
            ));
        }
        Ok(engine)
    }

    pub fn admit(
        &mut self,
        artifact: &[u8],
        wat: &[u8],
        source: &[u8],
        claimed: &[&str],
        allowed: &[&str],
    ) -> Result<super::output::Report, super::output::Failure> {
        let claims = crate::workflow::encoding::strings(claimed).encode();
        let policy = crate::workflow::encoding::strings(allowed).encode();
        let header = std::format!(
            "admit {} {} {} {} {}\n",
            artifact.len(), wat.len(), source.len(), claims.len(), policy.len()
        );
        attempt!(self.write(header.as_bytes()));
        for bytes in [artifact, wat, source, claims.as_bytes(), policy.as_bytes()] {
            attempt!(self.write(bytes));
        }
        self.reply()
    }

    pub fn start(options: &super::arguments::Options) -> Result<Self, super::output::Failure> {
        let config = attempt!(configuration::build(options));
        Self::start_with_config(&config)
    }

    pub fn start_live() -> Result<Self, super::output::Failure> {
        Self::start_with_config(&configuration::live())
    }

    pub fn start_slot() -> Result<Self, super::output::Failure> {
        Self::start_with_host(&configuration::slot(), SLOT_HOST, true)
    }

    /// A separately framed, private host-operator datagram channel. Only the
    /// selected slot profile owns it; normal engine modes keep stderr instead.
    pub fn slot_control_sender(
        &self,
    ) -> Result<std::os::unix::net::UnixDatagram, super::output::Failure> {
        attempt!(self.control.as_ref().ok_or_else(protocol::error))
            .try_clone().map_err(super::framing::io_error)
    }

    /// Slot mode must report worker shutdown failures through JSONL and its
    /// exit status, rather than leaving them to Drop's stderr diagnostic.
    pub fn finish_slot(&mut self) -> Result<(), super::output::Failure> {
        self.finish()
    }

    /// The slot host has parsed a source-bound nested terminal guest report.
    /// No shutdown frame can be trusted after that terminal protocol state.
    pub fn mark_slot_terminal(&mut self) {
        if self.control.is_some() {
            self.has_failed = true;
        }
    }

    fn start_with_config(config: &str) -> Result<Self, super::output::Failure> {
        Self::start_with_host(config, HOST, false)
    }

    fn start_with_host(config: &str, host: &str, slot: bool) -> Result<Self, super::output::Failure> {
        let (control, child_control) = if slot {
            let (parent, child) = attempt!(std::os::unix::net::UnixDatagram::pair()
                .map_err(super::framing::io_error));
            attempt!(parent.set_nonblocking(true).map_err(super::framing::io_error));
            attempt!(child.set_read_timeout(Some(std::time::Duration::from_secs(10)))
                .map_err(super::framing::io_error));
            (Some(parent), Some(child))
        } else {
            (None, None)
        };
        let (send, replies) = std::sync::mpsc::sync_channel(1);
        let mut engine = Self {
            child: attempt!(launch(config, host, child_control)),
            replies,
            reader: None,
            control,
            has_failed: false,
        };
        let stream = attempt!(engine.child.stdout.take().ok_or_else(protocol::error));
        engine.reader = Some(attempt!(std::thread::Builder::new()
            .name("noble-wasm-output".into())
            .spawn(move || protocol::receive(stream, send))
            .map_err(super::framing::io_error)));
        let ready = attempt!(engine.reply());
        if ready.outcome != "ready" {
            engine.has_failed = true;
            return Err(super::output::Failure::new(
                super::output::ErrorContext {
                    stage: "wasm",
                    outcome: "unsupported",
                },
                ready.json,
            ));
        }
        Ok(engine)
    }

    pub fn prepare(
        &mut self,
        wat: &[u8],
        source: &[u8],
        submission: u64,
    ) -> Result<super::output::Report, super::output::Failure> {
        let header = std::format!("compile {} {} {submission}\n", wat.len(), source.len());
        attempt!(self.write(header.as_bytes()));
        attempt!(self.write(wat));
        attempt!(self.write(source));
        self.reply()
    }

    /// One bounded host-selected JSON command; only an install carries the
    /// host-assembled Wasm (source-classified origins are also structurally
    /// checked). Source is never worker protocol input.
    pub fn slot(
        &mut self,
        request: &serde_json::Value,
        binary: &[u8],
    ) -> Result<super::output::Report, super::output::Failure> {
        const MAX_REQUEST: usize = 524_288;
        const MAX_BINARY: usize = 4_194_304;
        let json = attempt!(serde_json::to_vec(request).map_err(|error| {
            super::output::Failure::new(
                super::output::ErrorContext { stage: "shell", outcome: "invalid-input" },
                std::format!("cannot encode slot request: {error}"),
            )
        }));
        if json.len() > MAX_REQUEST || binary.len() > MAX_BINARY
            || (request.get("operation").and_then(serde_json::Value::as_str) != Some("install")
                && !binary.is_empty())
        {
            return Err(super::output::Failure::new(
                super::output::ErrorContext { stage: "shell", outcome: "exhausted" },
                "slot request exceeds bounded worker protocol",
            ));
        }
        let header = std::format!("slot {} {}\n", json.len(), binary.len());
        attempt!(self.write(header.as_bytes()));
        attempt!(self.write(&json));
        attempt!(self.write(binary));
        self.reply()
    }

    /// The checked Rust CLI owns source acceptance and in-process assembly;
    /// the resident worker accepts only bounded standard Wasm bytes.
    pub fn prepare_binary(
        &mut self,
        binary: &[u8],
        source: &[u8],
        submission: u64,
    ) -> Result<super::output::Report, super::output::Failure> {
        self.send_binary("compile-bin", binary, source, submission)
    }

    pub fn stage_binary(
        &mut self,
        binary: &[u8],
        source: &[u8],
        submission: u64,
    ) -> Result<super::output::Report, super::output::Failure> {
        self.send_binary("stage-bin", binary, source, submission)
    }

    fn send_binary(
        &mut self,
        kind: &str,
        binary: &[u8],
        source: &[u8],
        submission: u64,
    ) -> Result<super::output::Report, super::output::Failure> {
        let header = std::format!("{kind} {} {} {submission}\n", binary.len(), source.len());
        attempt!(self.write(header.as_bytes()));
        attempt!(self.write(binary));
        attempt!(self.write(source));
        self.reply()
    }

    pub fn publish_binary(&mut self) -> Result<super::output::Report, super::output::Failure> {
        attempt!(self.write(b"publish-bin\n"));
        self.reply()
    }

    pub fn discard_binary(&mut self) -> Result<super::output::Report, super::output::Failure> {
        attempt!(self.write(b"discard-bin\n"));
        self.reply()
    }

    /// Only the trusted CLI chooses the exact checked owner and selected-source
    /// generation. Guest input is never a grant-setting protocol command.
    pub fn set_live_grant(
        &mut self,
        name: &str,
        owner: u64,
        source_generation: u64,
    ) -> Result<super::output::Report, super::output::Failure> {
        let header = std::format!(
            "live-grant {} {owner} {source_generation}\n",
            name.len()
        );
        attempt!(self.write(header.as_bytes()));
        attempt!(self.write(name.as_bytes()));
        self.reply()
    }

    /// Retrieve a bounded immutable candidate only after the guest returns.
    /// Reading this receipt cannot consume the selected owner grant.
    pub fn take_live_proposal(&mut self) -> Result<super::output::Report, super::output::Failure> {
        attempt!(self.write(b"take-live-proposal\n"));
        self.reply()
    }

    pub fn execute(&mut self) -> Result<super::output::Report, super::output::Failure> {
        attempt!(self.write(b"execute\n"));
        self.reply()
    }

    /// Execute one prepared module with explicitly typed host injections.
    pub fn execute_inputs(
        &mut self,
        inputs: &str,
    ) -> Result<super::output::Report, super::output::Failure> {
        let header = std::format!("execute {}\n", inputs.len());
        attempt!(self.write(header.as_bytes()));
        attempt!(self.write(inputs.as_bytes()));
        self.reply()
    }

    /// Push typed companion or program values onto the persistent session stack
    /// without executing a candidate. One refused push is reported, never fatal.
    pub fn push(&mut self, inputs: &str) -> Result<super::output::Report, super::output::Failure> {
        let header = std::format!("push {}\n", inputs.len());
        attempt!(self.write(header.as_bytes()));
        attempt!(self.write(inputs.as_bytes()));
        self.reply()
    }

    /// Suspend only existing live operand slots in one engine-owned frame.
    pub fn park(&mut self) -> Result<super::output::Report, super::output::Failure> {
        attempt!(self.write(b"park\n"));
        self.reply()
    }

    /// Restore that frame without reconstructing cells or clearing failures.
    pub fn restore(&mut self) -> Result<super::output::Report, super::output::Failure> {
        attempt!(self.write(b"restore\n"));
        self.reply()
    }

    /// Observe one stack slot from the last executed instance.
    pub fn observe(&mut self, index: u32) -> Result<super::output::Report, super::output::Failure> {
        let header = std::format!("observe {index}\n");
        attempt!(self.write(header.as_bytes()));
        self.reply()
    }

    /// Explicitly project one Certified handle to its subject Program handle.
    pub fn project(
        &mut self,
        handle: u32,
    ) -> Result<super::output::Report, super::output::Failure> {
        let header = std::format!("project {handle}\n");
        attempt!(self.write(header.as_bytes()));
        self.reply()
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), super::output::Failure> {
        let result = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(protocol::error)
            .and_then(|input| {
                attempt!(std::io::Write::write_all(input, bytes).map_err(super::framing::io_error));
                std::io::Write::flush(input).map_err(super::framing::io_error)
            });
        if result.is_err() {
            self.has_failed = true;
        }
        result
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; reply waits on an mpsc receiver with a runtime deadline and formats channel failures. Channel synchronization and owned diagnostic formatting cannot be const."
    )]
    fn reply(&mut self) -> Result<super::output::Report, super::output::Failure> {
        let result = match self
            .replies
            .recv_timeout(std::time::Duration::from_secs(120))
        {
            Ok(reply) => reply,
            Err(error) => Err(super::output::Failure::new(
                super::output::ErrorContext {
                    stage: "wasm",
                    outcome: "internal-failure",
                },
                std::format!("engine worker failed or exceeded wall-clock limit: {error}"),
            )),
        };
        // An engine-level internal failure is a protocol event, not a report a
        // consumer interprets: surface the engine's own diagnostic and mark the
        // session dead.
        let result = match result {
            Ok(report) if report.outcome == "internal-failure" => Err(super::output::Failure::new(
                super::output::ErrorContext {
                    stage: "wasm",
                    outcome: "internal-failure",
                },
                report.json,
            )),
            other => other,
        };
        self.has_failed = match &result {
            Ok(report) => report.is_terminal(),
            Err(_) => true,
        };
        result
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; close writes the shutdown command and waits for the worker's acknowledgement over an mpsc channel. Process I/O and timed channel synchronization cannot be const."
    )]
    fn close(&mut self) -> Result<(), super::output::Failure> {
        attempt!(self.write(b"shutdown\n"));
        match self.replies.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(Ok(report)) if report.outcome == "closed" => Ok(()),
            Ok(Ok(_)) => Err(protocol::error()),
            Ok(Err(error)) => Err(error),
            Err(error) => Err(super::output::Failure::new(
                super::output::ErrorContext {
                    stage: "wasm",
                    outcome: "internal-failure",
                },
                std::format!("engine worker did not acknowledge shutdown: {error}"),
            )),
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; finish closes a process pipe, reaps the child and joins its output-reader thread before propagating errors. Resource destruction, process waiting and thread joining cannot be const."
    )]
    fn finish(&mut self) -> Result<(), super::output::Failure> {
        if self.child.stdin.is_none() && self.reader.is_none() {
            return Ok(());
        }
        let mut can_wait = !self.has_failed && self.reader.is_some();
        let request = if can_wait { self.close() } else { Ok(()) };
        if request.is_err() {
            can_wait = false;
        }
        if let Some(stdin) = self.child.stdin.take() {
            drop(stdin);
        }
        let status = process::reap(&mut self.child, can_wait);
        let joined = match self.reader.take() {
            Some(reader) => reader.join().map_err(|_| {
                super::output::Failure::new(
                    super::output::ErrorContext {
                        stage: "wasm",
                        outcome: "internal-failure",
                    },
                    "engine output reader failed",
                )
            }),
            None => Ok(()),
        };
        // Reaping and joining precede error propagation, including startup failures.
        attempt!(request);
        attempt!(status.map_err(super::framing::io_error));
        joined
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            eprintln!("noble: engine worker cleanup failed: {}", error.message);
        }
    }
}

fn launch(
    config: &str,
    host: &str,
    child_control: Option<std::os::unix::net::UnixDatagram>,
) -> Result<std::process::Child, super::output::Failure> {
    // Linux caps each individual exec argument below its total ARG_MAX. Keep
    // the selected, embedded slot worker off the -e argument as it grows;
    // bounded chunks remain private to the freshly spawned, env-cleared Node
    // process and the loader removes them before starting the worker module.
    const SLOT_SOURCE_LOADER: &str = r#"const count=Number(process.env.NOBLE_SLOT_SOURCE_PARTS);
const parts=Array.from({length:count},(_,i)=>{
  const key=`NOBLE_SLOT_SOURCE_${i}`, part=process.env[key];
  delete process.env[key];
  return part;
});
delete process.env.NOBLE_SLOT_SOURCE_PARTS;
await import('data:text/javascript;base64,'+Buffer.from(parts.join('')).toString('base64'));"#;
    let mut command = std::process::Command::new(NODE);
    let slot = child_control.is_some();
    command.env_clear()
        .args([
            "--no-liftoff",
            "--no-wasm-lazy-compilation",
            "--no-wasm-tier-up",
            "--stack-size=1024",
            "--max-old-space-size=256",
            "--input-type=module",
            "-e",
            if slot { SLOT_SOURCE_LOADER } else { host },
            "--",
            "--protocol",
            config,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    if slot {
        let mut start = 0;
        let mut count = 0;
        while start < host.len() {
            let mut end = (start + 60_000).min(host.len());
            while !host.is_char_boundary(end) {
                end -= 1;
            }
            command.env(std::format!("NOBLE_SLOT_SOURCE_{count}"), &host[start..end]);
            start = end;
            count += 1;
        }
        command.env("NOBLE_SLOT_SOURCE_PARTS", count.to_string());
    }
    if let Some(control) = child_control {
        command.env("NOBLE_SLOT_CONTROL_FD", "2");
        command.stderr(std::process::Stdio::from(std::os::fd::OwnedFd::from(control)));
    } else {
        command.stderr(std::process::Stdio::inherit());
    }
    command.spawn()
        .map_err(|error| {
            super::output::Failure::new(
                super::output::ErrorContext {
                    stage: "wasm",
                    outcome: "unsupported",
                },
                std::format!("cannot start selected Node engine: {error}"),
            )
        })
}
