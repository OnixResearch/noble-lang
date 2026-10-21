use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct PrivateDirectory(PathBuf);

impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn unwritten_fifo_is_refused_before_blocking_or_guest_execution(
) -> Result<(), Box<dyn std::error::Error>> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path =
        std::env::temp_dir().join(format!("noble-typed-fifo-{}-{stamp}", std::process::id()));
    std::fs::DirBuilder::new().mode(0o700).create(&path)?;
    let private = PrivateDirectory(path);
    let fifo = private.0.join("unwritten");
    if !Command::new("mkfifo").arg(&fifo).status()?.success() {
        return Err("could not create private FIFO".into());
    }
    let link = private.0.join("fifo-link");
    std::os::unix::fs::symlink(&fifo, &link)?;
    let other = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    for selected in [&fifo, &link] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_noble"))
            .args(["worker", "typed"])
            .arg(selected)
            .arg("--schema")
            .arg(other)
            .arg("--state")
            .arg(other)
            .arg("--event")
            .arg(other)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if child.try_wait()?.is_some() {
                break;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("typed worker blocked opening {}", selected.display()).into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let output = child.wait_with_output()?;
        let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        if output.status.code() != Some(2)
            || report["stage"] != "admission"
            || report["outcome"] != "invalid-input"
            || report["guest_requests"] != 0
            || report["protected_operations"] != 0
            || !report["typed_output"].is_null()
        {
            return Err(format!(
                "unexpected FIFO admission: {}",
                String::from_utf8_lossy(&output.stdout)
            )
            .into());
        }
    }
    Ok(())
}
