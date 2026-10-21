#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::ambient_env,
    tigerstyle::ambiguous_params,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]

use serde_json::Value;

fn run(
    source: &std::path::Path,
    state: &str,
    delta: &str,
) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["worker", "scalar"])
        .arg(source)
        .args(["--state", state, "--delta", delta])
        .output()?;
    let status = output
        .status
        .code()
        .ok_or("worker terminated without an exit code")?;
    let report = serde_json::from_slice(&output.stdout)?;
    assert!(
        output.stderr.is_empty(),
        "unexpected worker stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok((status, report))
}

#[test]
fn scalar_guest_is_source_bound_and_rejects_hostile_inputs(
) -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!("noble-worker-scalar-{}", std::process::id()));
    std::fs::create_dir(&root)?;
    let source = root.join("candidate.noble");
    std::fs::write(&source, b"+")?;
    let (code, wrapped) = run(&source, "9223372036854775807", "1")?;
    assert_eq!(code, 0);
    assert_eq!(wrapped["outcome"], "normal");
    assert_eq!(
        wrapped["typed_output"],
        serde_json::json!({"type":"I64","value":"-9223372036854775808"})
    );
    assert_eq!(
        wrapped["source_sha256"],
        "a318c24216defe206feeb73ef5be00033fa9c4a74d0b967f6532a26ca5906d3b"
    );
    assert_eq!(wrapped["module"]["source_sha256"], wrapped["source_sha256"]);
    assert_eq!(wrapped["guest_requests"], 0);
    assert_eq!(wrapped["protected_operations"], 0);

    std::fs::write(&source, b"-")?;
    let (code, selected) = run(&source, "13", "5")?;
    assert_eq!(code, 0);
    assert_eq!(selected["typed_output"]["value"], "8");
    assert_ne!(selected["source_sha256"], wrapped["source_sha256"]);

    for (candidate, expected) in [
        (b"drop drop".as_slice(), "interface-reject"),
        (b"test.abort".as_slice(), "interface-reject"),
    ] {
        std::fs::write(&source, candidate)?;
        let (code, refused) = run(&source, "2", "3")?;
        assert_ne!(code, 0);
        assert_eq!(refused["outcome"], expected);
        assert!(refused["typed_output"].is_null());
        assert!(refused["module"].is_null());
        assert_eq!(refused["guest_requests"], 0);
        assert_eq!(refused["protected_operations"], 0);
    }
    std::fs::write(&source, vec![b' '; 65_537])?;
    let (code, oversized) = run(&source, "2", "3")?;
    assert_ne!(code, 0);
    assert!(oversized["typed_output"].is_null());
    assert!(oversized["module"].is_null());

    std::fs::write(&source, b"+")?;
    let (code, invalid_input) = run(&source, "+2", "3")?;
    assert_ne!(code, 0);
    assert_eq!(invalid_input["outcome"], "invalid-input");
    assert!(invalid_input["module"].is_null());
    std::fs::remove_file(&source)?;
    std::fs::remove_dir(&root)?;
    Ok(())
}
