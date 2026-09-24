#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; capture reports spawn, pipe, deadline, output-budget, reader and wait failures through Failure. Child termination/reaping and reader joins must complete rather than be interrupted by assertions."
)]
pub(crate) fn capture(
    mut command: std::process::Command,
    deadline: std::time::Instant,
) -> Result<super::Transcript, crate::workflow::output::Failure> {
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = attempt!(command.spawn().map_err(|error| {
        crate::workflow::output::Failure::unsupported(
            "sandbox-unavailable",
            std::format!("cannot launch sandbox: {error}"),
        )
    }));
    let output_exceeded = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stdout = match child.stdout.take() {
        Some(stream) => stream,
        None => return Err(missing_pipe(&mut child, "missing stdout pipe")),
    };
    let stderr = match child.stderr.take() {
        Some(stream) => stream,
        None => return Err(missing_pipe(&mut child, "missing stderr pipe")),
    };
    let out_flag = std::sync::Arc::clone(&output_exceeded);
    let err_flag = std::sync::Arc::clone(&output_exceeded);
    let out_reader = std::thread::spawn(move || read_output(stdout, &out_flag));
    let err_reader = std::thread::spawn(move || read_output(stderr, &err_flag));
    let (status, terminal_failure) = monitor(&mut child, &output_exceeded, deadline);
    let stdout = out_reader.join();
    let stderr = err_reader.join();
    if let Some(error) = terminal_failure {
        return Err(error);
    }
    if output_exceeded.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(output_limit());
    }
    let status = attempt!(
        status.map_err(|error| crate::workflow::output::Failure::error(
            "process-wait",
            error.to_string()
        ))
    );
    Ok(super::Transcript {
        status,
        stdout: attempt!(joined_output(stdout)),
        stderr: attempt!(joined_output(stderr)),
    })
}

fn monitor(
    child: &mut std::process::Child,
    output_exceeded: &std::sync::atomic::AtomicBool,
    deadline: std::time::Instant,
) -> (
    std::io::Result<std::process::ExitStatus>,
    Option<crate::workflow::output::Failure>,
) {
    loop {
        if output_exceeded.load(std::sync::atomic::Ordering::Relaxed) {
            return (stop(child), Some(output_limit()));
        }
        if super::now() >= deadline {
            return (
                stop(child),
                Some(crate::workflow::output::Failure::timeout(
                    "sandbox exceeded the consumer's total wall-clock budget",
                )),
            );
        }
        match child.try_wait() {
            Ok(Some(status)) => return (Ok(status), None),
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(10)),
            Err(error) => {
                if let Err(cleanup_error) = stop(child) {
                    // Reaping was attempted; retain the original wait failure.
                    drop(cleanup_error);
                }
                return (Err(error), None);
            }
        }
    }
}

fn stop(child: &mut std::process::Child) -> std::io::Result<std::process::ExitStatus> {
    if let Err(error) = child.kill() {
        // A child can exit between observation and kill. Even a failed kill
        // must never prevent wait/reaping; wait supplies the terminal result.
        drop(error);
    }
    child.wait()
}

fn missing_pipe(
    child: &mut std::process::Child,
    message: &str,
) -> crate::workflow::output::Failure {
    if let Err(error) = stop(child) {
        // Preserve the primary pipe error after attempting both kill and wait.
        drop(error);
    }
    crate::workflow::output::Failure::error("process-io", message.into())
}

fn output_limit() -> crate::workflow::output::Failure {
    crate::workflow::output::Failure::error(
        "proof-output-limit",
        "sandbox output exceeded the 128 KiB per-stream limit".into(),
    )
}

fn read_output(
    mut stream: impl std::io::Read,
    exceeded: &std::sync::atomic::AtomicBool,
) -> std::io::Result<std::vec::Vec<u8>> {
    let mut bytes = std::vec::Vec::with_capacity(super::OUTPUT_LIMIT);
    let mut buffer = [0_u8; 4096];
    loop {
        let count = attempt!(stream.read(&mut buffer));
        if count == 0 {
            return Ok(bytes);
        }
        if bytes.len().saturating_add(count) > super::OUTPUT_LIMIT {
            exceeded.store(true, std::sync::atomic::Ordering::Relaxed);
            return Ok(bytes);
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
}

fn joined_output(
    result: std::thread::Result<std::io::Result<std::vec::Vec<u8>>>,
) -> Result<std::string::String, crate::workflow::output::Failure> {
    match result {
        Ok(Ok(bytes)) => Ok(std::string::String::from_utf8_lossy(&bytes).into_owned()),
        Ok(Err(error)) => Err(crate::workflow::output::Failure::error(
            "process-output",
            error.to_string(),
        )),
        Err(_) => Err(crate::workflow::output::Failure::error(
            "process-output",
            "output reader failed".into(),
        )),
    }
}
