//! Revision-3 `Named-Contract-Calls-v3` CLI integration and differential tests.

#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::function_length,
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; CLI test harness uses tempdir fixtures and JSON assertions."
)]

use std::path::PathBuf;
use std::process::Command;

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str, files: &[(&str, &str)]) -> std::io::Result<Self> {
        let base =
            std::env::temp_dir().join(format!("noble-named-v3-{label}-{}", std::process::id()));
        if base.exists() {
            std::fs::remove_dir_all(&base)?;
        }
        std::fs::create_dir_all(&base)?;
        for (name, body) in files {
            std::fs::write(base.join(name), body)?;
        }
        Ok(Self(base))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const GUARD_CONTRACT_ONLY: &str = "module guard@1 [ export dec def dec [ 1 - ] contract 3 dec-law [ subject dec input [ x I64 ] output [ y I64 ] requires [ (lt 0 (in x)) ] ensures [ (eq (out y) (sub (in x) 1)) ] ] ]";
const INC_MOD: &str = "module incmod@1 [ export inc export inc-ok def inc [ 1 + ] contract 1 inc-law [ subject inc input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 1 inc-ok for inc-law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]";
const GUARD_MOD: &str = "module guard@1 [ export dec export dec-ok def dec [ 1 - ] contract 3 dec-law [ subject dec input [ x I64 ] output [ y I64 ] requires [ (lt 0 (in x)) ] ensures [ (eq (out y) (sub (in x) 1)) ] ] proof 3 dec-ok for dec-law [ (export-named-claim (join (refl (sub (in x) 1)))) ] ]";
const DIFF_MOD: &str = "module diff@1 [ export sub2 export sub2-ok def sub2 [ - ] contract 3 sub2-law [ subject sub2 input [ a I64 b I64 ] output [ d I64 ] requires [ true ] ensures [ (eq (out d) (sub (in a) (in b))) ] ] proof 3 sub2-ok for sub2-law [ (export-named-claim (join (refl (sub (in a) (in b))))) ] ]";

#[test]
fn rev3_obligations_and_goal_view_report_v3_profile() -> Result<(), Box<dyn std::error::Error>> {
    let caller = "module app@1 [ export f def f [ g.dec ] contract 3 f-law [ subject f input [ x I64 ] output [ y I64 ] requires [ (lt 0 (in x)) ] ensures [ (eq (out y) (sub (in x) 1)) ] ] ]";
    let fix = Fixture::new(
        "obl-v3",
        &[
            ("guard.noble", GUARD_CONTRACT_ONLY),
            ("imp_g.noble", "import guard@1 as g"),
            ("app.noble", caller),
        ],
    )?;

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_noble"));
    cmd.arg("obligations")
        .arg(fix.0.join("app.noble"))
        .arg("--module")
        .arg(fix.0.join("guard.noble"))
        .arg("--module")
        .arg(fix.0.join("imp_g.noble"));
    for key in [
        "NOBLE_LEAN",
        "NOBLE_BWRAP",
        "NOBLE_PRLIMIT",
        "NOBLE_SYSTEMD_RUN",
        "NOBLE_NIX_STORE",
    ] {
        cmd.env_remove(key);
    }
    let output = cmd.output()?;
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        output.status.success(),
        "obligations failed: {stdout} / {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(json["profile"], "Named-Contract-Calls-v3");
    assert_eq!(json["obligations"][0]["revision"], 3);
    assert_eq!(
        json["obligations"][0]["statement_source"],
        "prepare_named_v3_contract"
    );
    assert_eq!(
        json["obligations"][0]["statement_ref"],
        "NamedV3Obligation.claim"
    );
    let statement = json["obligations"][0]["statement"]
        .as_str()
        .expect("obligations statement string");
    assert!(statement.contains("namespace NamedV3Obligation"));
    assert!(statement.contains("exportedLiftedClaim subject"));
    Ok(())
}

#[test]
#[ignore = "requires NOBLE_LEAN sandbox toolchain (run after M4_CLEAR)"]
fn rev3_strict_lean_consumer_accepts_preuse_s01_and_s04() -> Result<(), Box<dyn std::error::Error>>
{
    let s01_caller = "module app_s01@1 [ export round export round-ok def round [ m.inc g.dec ] contract 3 round-law [ subject round input [ x I64 ] output [ y I64 ] requires [ (lt 0 (add (in x) 1)) ] ensures [ (eq (out y) (sub (add (in x) 1) 1)) ] ] proof 3 round-ok for round-law [ (export-named-claim (pc-call 0 m.inc-ok (by-true)) (pc-call 1 g.dec-ok (use f0)) (join (refl (sub (add (in x) 1) 1)))) ] ]";
    let fix = Fixture::new(
        "lean-s01",
        &[
            ("incmod.noble", INC_MOD),
            ("imp_m.noble", "import incmod@1 as m"),
            ("guard.noble", GUARD_MOD),
            ("imp_g.noble", "import guard@1 as g"),
            ("app_s01.noble", s01_caller),
        ],
    )?;
    let out = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify-module")
        .arg(fix.0.join("app_s01.noble"))
        .arg("--module")
        .arg(fix.0.join("incmod.noble"))
        .arg("--module")
        .arg(fix.0.join("imp_m.noble"))
        .arg("--module")
        .arg(fix.0.join("guard.noble"))
        .arg("--module")
        .arg(fix.0.join("imp_g.noble"))
        .args(["--timeout-ms", "600000"])
        .output()?;
    let stdout = String::from_utf8(out.stdout)?;
    assert!(out.status.success(), "verify-module S01 failed: {stdout}");
    let json: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(json["outcome"], "proved");
    assert_eq!(json["profile"], "Named-Contract-Calls-v3");

    let s04_caller = "module app_s04@1 [ export revsub def revsub [ swap d.sub2 ] contract 3 revsub-law [ subject revsub input [ x I64 y I64 ] output [ z I64 ] requires [ true ] ensures [ (eq (out z) (sub (in y) (in x))) ] ] proof 3 revsub-ok for revsub-law [ (export-named-claim (pc-call 0 d.sub2-ok (by-true)) (join (refl (sub (in y) (in x))))) ] ]";
    let fix4 = Fixture::new(
        "lean-s04",
        &[
            ("diff.noble", DIFF_MOD),
            ("imp_d.noble", "import diff@1 as d"),
            ("app_s04.noble", s04_caller),
        ],
    )?;
    let out4 = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify-module")
        .arg(fix4.0.join("app_s04.noble"))
        .arg("--module")
        .arg(fix4.0.join("diff.noble"))
        .arg("--module")
        .arg(fix4.0.join("imp_d.noble"))
        .args(["--timeout-ms", "600000"])
        .output()?;
    let stdout4 = String::from_utf8(out4.stdout)?;
    assert!(out4.status.success(), "verify-module S04 failed: {stdout4}");
    let json4: serde_json::Value = serde_json::from_str(&stdout4)?;
    assert_eq!(json4["outcome"], "proved");
    Ok(())
}
