//! Exact source, tool, and runner identity for the opt-in export.

use std::io::Write;
use std::process::{Command, Stdio};

pub(super) fn command(program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .output()
        .expect("pinned executable");
    assert!(output.status.success(), "{program} {args:?}: {output:?}");
    String::from_utf8(output.stdout)
        .expect("UTF8 tool output")
        .trim()
        .to_owned()
}

pub(super) fn tool_digest(program: &str) -> String {
    let path = command("sh", &["-c", &format!("command -v {program}")]);
    digest(&std::fs::read(path).expect("exact tool bytes"))
}

pub(super) fn digest(bytes: &[u8]) -> String {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(bytes)
        .expect("hash input");
    let result = child.wait_with_output().expect("sha256sum output");
    assert!(result.status.success());
    String::from_utf8(result.stdout).unwrap()[..64].to_owned()
}

pub(super) fn producer_digest() -> String {
    // Length-framed, ordered source paths and bytes bind every module of this
    // producer, not merely its short test entry point.
    const SOURCES: [&str; 5] = [
        "tests/tick_runner/export.rs",
        "tests/tick_runner/export/identity.rs",
        "tests/tick_runner/export/model.rs",
        "tests/tick_runner/export/rows.rs",
        "tests/tick_runner/export/sequence.rs",
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut framed = Vec::new();
    for path in SOURCES {
        let name = path.as_bytes();
        let bytes = std::fs::read(root.join(path)).expect("exact producer source bytes");
        framed.extend_from_slice(&u64::try_from(name.len()).unwrap().to_le_bytes());
        framed.extend_from_slice(name);
        framed.extend_from_slice(&u64::try_from(bytes.len()).unwrap().to_le_bytes());
        framed.extend_from_slice(&bytes);
    }
    digest(&framed)
}
