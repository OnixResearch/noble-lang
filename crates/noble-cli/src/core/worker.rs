mod process;
mod protocol;

const NODE: &str = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
const HOST: &str = concat!(
    include_str!("runtime/preamble.mjs"),
    include_str!("runtime/values.mjs"),
    include_str!("runtime/engine.mjs"),
    include_str!("runtime/protocol.mjs"),
);
const SELECTION: &str = include_str!("runtime/config.json");
const ABI: &str = include_str!("runtime/abi.json");

pub(super) struct Engine {
    child: std::process::Child,
    replies: std::sync::mpsc::Receiver<Result<super::output::Report, super::output::Failure>>,
    reader: Option<std::thread::JoinHandle<()>>,
    has_failed: bool,
}

impl Engine {
    pub fn start(options: &super::arguments::Options) -> Result<Self, super::output::Failure> {
        let config = attempt!(configuration(options));
        let (send, replies) = std::sync::mpsc::sync_channel(1);
        let mut engine = Self {
            child: attempt!(launch(&config)),
            replies,
            reader: None,
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

fn launch(config: &str) -> Result<std::process::Child, super::output::Failure> {
    std::process::Command::new(NODE)
        .env_clear()
        .args([
            "--no-liftoff",
            "--no-wasm-lazy-compilation",
            "--no-wasm-tier-up",
            "--stack-size=1024",
            "--max-old-space-size=256",
            "--input-type=module",
            "-e",
            HOST,
            "--",
            "--protocol",
            config,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
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

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; configuration canonicalizes the caller's artifact directory and allocates the worker's JSON configuration. Host filesystem observations and owned JSON encoding cannot be const."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; configuration propagates artifact-directory canonicalization failures before worker launch. Filesystem state is fallible external input, not an assertion precondition."
)]
fn configuration(
    options: &super::arguments::Options,
) -> Result<std::string::String, super::output::Failure> {
    let destination = match &options.emit {
        Some(path) => Some(attempt!(
            std::fs::canonicalize(path).map_err(super::framing::io_error)
        )),
        None => None,
    };
    let destination = destination
        .as_ref()
        .map(|path| path.to_string_lossy().into_owned());
    Ok(crate::workflow::encoding::object([
        ("selection", crate::workflow::encoding::string(SELECTION)),
        ("abi", crate::workflow::encoding::string(ABI)),
        (
            "optimized",
            crate::workflow::encoding::Json::Bool(options.optimized),
        ),
        (
            "artifacts",
            crate::workflow::encoding::optional_string(destination.as_deref()),
        ),
    ])
    .encode())
}
