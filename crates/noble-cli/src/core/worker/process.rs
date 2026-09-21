pub(super) fn reap(
    child: &mut std::process::Child,
    can_wait: bool,
) -> std::io::Result<std::process::ExitStatus> {
    let observed = if can_wait { observe(child) } else { Ok(None) };
    match observed {
        Ok(Some(status)) => Ok(status),
        Ok(None) => stop(child),
        Err(error) => {
            if let Err(cleanup_error) = stop(child) {
                // Both kill and wait were attempted; keep the original wait error.
                drop(cleanup_error);
            }
            Err(error)
        }
    }
}

fn observe(child: &mut std::process::Child) -> std::io::Result<Option<std::process::ExitStatus>> {
    let deadline = attempt!(crate::sandbox::now()
        .checked_add(std::time::Duration::from_secs(1))
        .ok_or_else(|| std::io::Error::other("engine shutdown deadline overflow")));
    loop {
        if let Some(status) = attempt!(child.try_wait()) {
            return Ok(Some(status));
        }
        if crate::sandbox::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn stop(child: &mut std::process::Child) -> std::io::Result<std::process::ExitStatus> {
    if let Err(error) = child.kill() {
        // Exit may race with kill. Even a failed kill must be followed by wait;
        // the wait result determines whether the child was successfully reaped.
        drop(error);
    }
    child.wait()
}
