#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::ambient_clock,
    tigerstyle::ambient_env,
    tigerstyle::assertion_density,
    tigerstyle::compound_condition,
    tigerstyle::ignored_result,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
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

/// The candidate exists only after the coordinator binary was built. The host
/// preparation service publishes it once as a new package; `worker typed`
/// re-admits that untrusted package in a separate invocation.
#[test]
fn candidate_prepared_after_build_is_published_once_and_evaluated_separately(
) -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::{json, Value};
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let path = std::env::temp_dir().join(format!(
        "noble-typed-prepare-{}-{stamp}",
        std::process::id()
    ));
    std::fs::DirBuilder::new().mode(0o700).create(&path)?;
    let private = PrivateDirectory(path);
    let source = private.0.join("transition.noble");
    std::fs::write(&source, include_str!("fixtures/worker-transition.noble"))?;
    let schema = private.0.join("schema.noble");
    std::fs::write(&schema, include_str!("fixtures/worker-data.noble"))?;
    let prepare = |candidate: &std::path::Path,
                   out: &std::path::Path|
     -> Result<(Option<i32>, Value), Box<dyn std::error::Error>> {
        let output = Command::new(env!("CARGO_BIN_EXE_noble"))
            .args(["worker", "prepare"])
            .arg(candidate)
            .arg("--schema")
            .arg(&schema)
            .arg("--out")
            .arg(out)
            .stdin(Stdio::null())
            .output()?;
        Ok((
            output.status.code(),
            serde_json::from_slice(&output.stdout)?,
        ))
    };

    let package = private.0.join("package.json");
    let (status, trace) = prepare(&source, &package)?;
    assert_eq!(status, Some(0), "{trace}");
    assert_eq!(trace["outcome"], "ready");
    assert_eq!(trace["stage"], "publish");
    assert_eq!(
        trace["service_effects"],
        json!({"input_files_read":2,"source_checks":3,"closure_walks":1,
            "backend_compilations":1,"packages_written":1})
    );
    assert!(trace.get("typed_output").is_none() && trace.get("guest_requests").is_none());
    let published = std::fs::read(&package)?;
    assert_eq!(trace["package_bytes"], published.len());
    let document: Value = serde_json::from_slice(&published)?;
    assert_eq!(document["format"], "noble-worker-local-package/v1");
    assert_eq!(document["definitions"].as_array().map(Vec::len), Some(3));

    // A repeated preparation never overwrites the published package.
    let (status, repeated) = prepare(&source, &package)?;
    assert_eq!(status, Some(2), "{repeated}");
    assert_eq!(repeated["outcome"], "invalid-input");
    assert_eq!(repeated["stage"], "publish");
    assert_eq!(repeated["service_effects"]["packages_written"], 0);
    assert_eq!(std::fs::read(&package)?, published);

    // A refused candidate is never compiled and publishes nothing.
    let reversed = private.0.join("reversed.noble");
    std::fs::write(
        &reversed,
        "module worker@1 [ export transition \
         def transition [ swap drop unit WorkerData@1.Action.Noop nil cons ] ]",
    )?;
    let refused_out = private.0.join("refused.json");
    let (status, refused) = prepare(&reversed, &refused_out)?;
    assert_eq!(status, Some(2), "{refused}");
    assert_eq!(refused["outcome"], "interface-reject");
    assert_eq!(refused["stage"], "transition-check");
    assert_eq!(refused["service_effects"]["backend_compilations"], 0);
    assert_eq!(refused["service_effects"]["packages_written"], 0);
    assert!(refused["package_sha256"].is_null());
    assert!(!refused_out.exists());

    // The coordinator builds its value frames from its own schema rows.
    let state_wire = document["interface"]["inputs"][0].clone();
    let event_wire = document["interface"]["inputs"][1].clone();
    let action_wire = document["interface"]["outputs"][1]["list"].clone();
    let attempt_wire = document["schemas"]
        .as_array()
        .ok_or("schema rows")?
        .iter()
        .map(|row| {
            let mut wire = row.clone();
            wire.as_object_mut()
                .map(|fields| fields.remove("source_sha256"));
            wire
        })
        .find(|wire| ![&state_wire, &event_wire, &action_wire].contains(&wire))
        .ok_or("AttemptId schema row")?;
    let state = private.0.join("state.json");
    std::fs::write(
        &state,
        serde_json::to_vec(&json!({"format":"noble-worker-value/v1",
            "expected_schema":state_wire,"value":{"tag":"opaque","schema":state_wire,
                "value":{"tag":"pair","left":{"tag":"i64","value":"0"},
                    "right":{"tag":"sum","arm":"left","value":{"tag":"unit","value":null}}}}}))?,
    )?;
    let event = private.0.join("event.json");
    std::fs::write(
        &event,
        serde_json::to_vec(&json!({"format":"noble-worker-value/v1",
            "expected_schema":event_wire,"value":{"tag":"variant","schema":event_wire,
                "arm":"left","value":{"tag":"pair",
                    "left":{"tag":"opaque","schema":attempt_wire,
                        "value":{"tag":"i64","value":"1"}},
                    "right":{"tag":"text","value":"ping"}}}}))?,
    )?;
    let output = Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["worker", "typed"])
        .arg(&package)
        .arg("--schema")
        .arg(&schema)
        .arg("--state")
        .arg(&state)
        .arg("--event")
        .arg(&event)
        .stdin(Stdio::null())
        .output()?;
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(output.status.code(), Some(0), "{report}");
    assert_eq!(report["outcome"], "normal");
    assert_eq!(report["guest_requests"], 0);
    assert_eq!(report["typed_output"]["actions"]["items"][0]["arm"], "left");
    assert_eq!(report["module"]["wat_sha256"], trace["wat_sha256"]);
    assert!(report.get("service_effects").is_none());
    Ok(())
}
