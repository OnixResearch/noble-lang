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
fn rev3_link_reports_revision_3_and_generated_claim() -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_noble"));
    cmd.args([
        "session",
        "--framed",
        "--declared-modules",
        "--text-byte-cursor",
    ])
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped());
    for key in [
        "NOBLE_LEAN",
        "NOBLE_BWRAP",
        "NOBLE_PRLIMIT",
        "NOBLE_SYSTEMD_RUN",
        "NOBLE_NIX_STORE",
    ] {
        cmd.env_remove(key);
    }
    let mut child = cmd.spawn()?;
    let frame = format!("{}\n{}", GUARD_CONTRACT_ONLY.len(), GUARD_CONTRACT_ONLY);
    child
        .stdin
        .as_mut()
        .ok_or("session stdin")?
        .write_all(frame.as_bytes())?;
    let output = child.wait_with_output()?;
    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        output.status.success(),
        "session link failed: {stdout} / {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_str(stdout.trim())?;
    assert_eq!(json["stage"], "link");
    assert_eq!(json["outcome"], "linked");
    assert_eq!(json["contracts"][0]["revision"], 3);
    assert_eq!(
        json["contracts"][0]["generated_claim"],
        "NamedV3Obligation.claim"
    );
    assert!(json["contracts"][0]["original_sources"]
        .as_array()
        .is_some_and(|arr| !arr.is_empty()));
    Ok(())
}

const MID_S01_MOD: &str = "module app_s01@1 [ export round export round-ok def round [ m.inc g.dec ] contract 3 round-law [ subject round input [ x I64 ] output [ y I64 ] requires [ (lt 0 (add (in x) 1)) ] ensures [ (eq (out y) (sub (add (in x) 1) 1)) ] ] proof 3 round-ok for round-law [ (export-named-claim (pc-call 0 m.inc-ok (by-true)) (pc-call 1 g.dec-ok (use f0)) (join (refl (sub (add (in x) 1) 1)))) ] ]";

#[test]
#[ignore = "requires NOBLE_LEAN sandbox toolchain"]
fn rev3_strict_lean_consumer_accepts_preuse_s01() -> Result<(), Box<dyn std::error::Error>> {
    super::require_sandbox_tools()?;
    let fix = Fixture::new(
        "lean-s01",
        &[
            ("incmod.noble", INC_MOD),
            ("imp_m.noble", "import incmod@1 as m"),
            ("guard.noble", GUARD_MOD),
            ("imp_g.noble", "import guard@1 as g"),
            ("app_s01.noble", MID_S01_MOD),
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
        .args(["--report-schema", "v2", "--timeout-ms", "600000"])
        .env("NOBLE_CONTRACT_LIBRARY", super::library_path())
        .output()?;
    let stdout = String::from_utf8(out.stdout)?;
    assert!(out.status.success(), "verify-module S01 failed: {stdout}");
    let json: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(json["outcome"], "proved");
    assert_eq!(json["profile"], "Named-Contract-Calls-v3");
    assert_eq!(
        json["proofs"][0]["subject"]["generated_claim"],
        "NamedV3Obligation.claim"
    );
    Ok(())
}

#[test]
#[ignore = "requires NOBLE_LEAN sandbox toolchain"]
fn rev3_strict_lean_consumer_accepts_preuse_s04() -> Result<(), Box<dyn std::error::Error>> {
    super::require_sandbox_tools()?;
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
        .env("NOBLE_CONTRACT_LIBRARY", super::library_path())
        .output()?;
    let stdout4 = String::from_utf8(out4.stdout)?;
    assert!(out4.status.success(), "verify-module S04 failed: {stdout4}");
    let json4: serde_json::Value = serde_json::from_str(&stdout4)?;
    assert_eq!(json4["outcome"], "proved");
    Ok(())
}

#[test]
#[ignore = "requires NOBLE_LEAN sandbox toolchain"]
fn rev3_strict_lean_consumer_accepts_nested_3hop() -> Result<(), Box<dyn std::error::Error>> {
    super::require_sandbox_tools()?;
    let top_mod = "module app_nested@1 [ export top export top-ok def top [ s01.round m.inc ] contract 3 top-law [ subject top input [ x I64 ] output [ y I64 ] requires [ (lt 0 (add (in x) 1)) ] ensures [ (eq (out y) (add (sub (add (in x) 1) 1) 1)) ] ] proof 3 top-ok for top-law [ (export-named-claim (pc-call 0 s01.round-ok (use f0)) (pc-call 1 m.inc-ok (by-true)) (join (refl (add (sub (add (in x) 1) 1) 1)))) ] ]";
    let top3_mod = "module app_top3@1 [ export top3 def top3 [ n1.top m.inc ] contract 3 top3-law [ subject top3 input [ x I64 ] output [ y I64 ] requires [ (lt 0 (add (in x) 1)) ] ensures [ (eq (out y) (add (add (sub (add (in x) 1) 1) 1) 1)) ] ] proof 3 top3-ok for top3-law [ (export-named-claim (pc-call 0 n1.top-ok (use f0)) (pc-call 1 m.inc-ok (by-true)) (join (refl (add (add (sub (add (in x) 1) 1) 1) 1)))) ] ]";

    let fix = Fixture::new(
        "lean-nested",
        &[
            ("incmod.noble", INC_MOD),
            ("imp_m.noble", "import incmod@1 as m"),
            ("guard.noble", GUARD_MOD),
            ("imp_g.noble", "import guard@1 as g"),
            ("app_s01.noble", MID_S01_MOD),
            ("imp_s01.noble", "import app_s01@1 as s01"),
            ("app_nested.noble", top_mod),
            ("imp_n1.noble", "import app_nested@1 as n1"),
            ("app_top3.noble", top3_mod),
        ],
    )?;
    let out = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify-module")
        .arg(fix.0.join("app_top3.noble"))
        .arg("--module")
        .arg(fix.0.join("incmod.noble"))
        .arg("--module")
        .arg(fix.0.join("imp_m.noble"))
        .arg("--module")
        .arg(fix.0.join("guard.noble"))
        .arg("--module")
        .arg(fix.0.join("imp_g.noble"))
        .arg("--module")
        .arg(fix.0.join("app_s01.noble"))
        .arg("--module")
        .arg(fix.0.join("imp_s01.noble"))
        .arg("--module")
        .arg(fix.0.join("app_nested.noble"))
        .arg("--module")
        .arg(fix.0.join("imp_n1.noble"))
        .args(["--report-schema", "v2", "--timeout-ms", "600000"])
        .env("NOBLE_CONTRACT_LIBRARY", super::library_path())
        .output()?;
    let stdout = String::from_utf8(out.stdout)?;
    assert!(out.status.success(), "verify-module 3-hop failed: {stdout}");
    let json: serde_json::Value = serde_json::from_str(&stdout)?;
    assert_eq!(json["outcome"], "proved");
    assert_eq!(json["profile"], "Named-Contract-Calls-v3");
    assert_eq!(
        json["proofs"][0]["subject"]["generated_claim"],
        "NamedV3Obligation.claim"
    );
    assert!(json["proofs"][0]["subject"]["definition_body_uses"]
        .as_array()
        .is_some_and(|uses| uses.len() >= 5));
    Ok(())
}

#[test]
#[ignore = "requires NOBLE_LEAN sandbox toolchain"]
fn rev3_strict_lean_consumer_accepts_repeated_non_leaf_callee(
) -> Result<(), Box<dyn std::error::Error>> {
    super::require_sandbox_tools()?;
    let twice_mod = "module twice_rev3@1 [ export twice def twice [ s01.round s01.round ] contract 3 twice-law [ subject twice input [ x I64 ] output [ y I64 ] requires [ (and (lt 0 (add (in x) 1)) (lt 0 (add (sub (add (in x) 1) 1) 1))) ] ensures [ (eq (out y) (sub (add (sub (add (in x) 1) 1) 1) 1)) ] ] proof 3 twice-ok for twice-law [ (export-named-claim (pc-call 0 s01.round-ok (use f0)) (pc-call 1 s01.round-ok (use f1)) (join (refl (sub (add (sub (add (in x) 1) 1) 1) 1)))) ] ]";
    let fix_tw = Fixture::new(
        "lean-twice",
        &[
            ("incmod.noble", INC_MOD),
            ("imp_m.noble", "import incmod@1 as m"),
            ("guard.noble", GUARD_MOD),
            ("imp_g.noble", "import guard@1 as g"),
            ("app_s01.noble", MID_S01_MOD),
            ("imp_s01.noble", "import app_s01@1 as s01"),
            ("twice.noble", twice_mod),
        ],
    )?;
    let out_tw = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify-module")
        .arg(fix_tw.0.join("twice.noble"))
        .arg("--module")
        .arg(fix_tw.0.join("incmod.noble"))
        .arg("--module")
        .arg(fix_tw.0.join("imp_m.noble"))
        .arg("--module")
        .arg(fix_tw.0.join("guard.noble"))
        .arg("--module")
        .arg(fix_tw.0.join("imp_g.noble"))
        .arg("--module")
        .arg(fix_tw.0.join("app_s01.noble"))
        .arg("--module")
        .arg(fix_tw.0.join("imp_s01.noble"))
        .args(["--timeout-ms", "600000"])
        .env("NOBLE_CONTRACT_LIBRARY", super::library_path())
        .output()?;
    let stdout_tw = String::from_utf8(out_tw.stdout)?;
    assert!(
        out_tw.status.success(),
        "verify-module twice_rev3 failed: {stdout_tw}"
    );
    let json_tw: serde_json::Value = serde_json::from_str(&stdout_tw)?;
    assert_eq!(json_tw["outcome"], "proved");
    Ok(())
}

#[test]
#[ignore = "requires NOBLE_LEAN sandbox toolchain"]
fn rev3_strict_lean_consumer_accepts_local_multi_hop_chain(
) -> Result<(), Box<dyn std::error::Error>> {
    super::require_sandbox_tools()?;
    let chain_mod = "module localchain@1 [ proof 1 addcomm : [ (Pi (a I64) (Pi (b I64) (Eq I64 (add a b) (add b a)))) ] [ (intro (a I64) (intro (b I64) (bv-add-comm a b))) ] def c1 [ m.inc ] contract 3 c1-law [ subject c1 input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 3 c1-ok for c1-law [ (export-named-claim (pc-call 0 m.inc-ok (by-true)) (join (refl (add (in x) 1)))) ] def c2 [ c1 ] contract 3 c2-law [ subject c2 input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add 1 (in x))) ] ] proof 3 c2-ok for c2-law [ (export-named-claim (pc-call 0 c1-ok (by-true)) (join (subst (motive (v I64) (eq (add (in x) 1) v)) (apply (apply (use addcomm) (in x)) 1) (refl (add (in x) 1))))) ] export c3 export c3-ok def c3 [ c2 ] contract 3 c3-law [ subject c3 input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add 1 (in x))) ] ] proof 3 c3-ok for c3-law [ (export-named-claim (pc-call 0 c2-ok (by-true)) (join (refl (add 1 (in x))))) ] ]";
    let fix_ch = Fixture::new(
        "lean-localchain",
        &[
            ("incmod.noble", INC_MOD),
            ("imp_m.noble", "import incmod@1 as m"),
            ("chain.noble", chain_mod),
        ],
    )?;
    let out_ch = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify-module")
        .arg(fix_ch.0.join("chain.noble"))
        .arg("--module")
        .arg(fix_ch.0.join("incmod.noble"))
        .arg("--module")
        .arg(fix_ch.0.join("imp_m.noble"))
        .args(["--timeout-ms", "600000"])
        .env("NOBLE_CONTRACT_LIBRARY", super::library_path())
        .output()?;
    let stdout_ch = String::from_utf8(out_ch.stdout)?;
    assert!(
        out_ch.status.success(),
        "verify-module localchain failed: {stdout_ch}"
    );
    let json_ch: serde_json::Value = serde_json::from_str(&stdout_ch)?;
    assert_eq!(json_ch["outcome"], "proved");
    Ok(())
}

#[test]
#[ignore = "requires NOBLE_LEAN sandbox toolchain"]
fn rev3_strict_lean_consumer_accepts_preuse_s02() -> Result<(), Box<dyn std::error::Error>> {
    super::require_sandbox_tools()?;
    let arith_mod = "module arith@1 [ export add-assoc-wrap proof 1 add-assoc-wrap : [ (Pi (a I64) (Pi (b I64) (Pi (c I64) (Eq I64 (add (add a b) c) (add a (add b c)))))) ] [ (intro (a I64) (intro (b I64) (intro (c I64) (bv-add-assoc a b c)))) ] ]";
    let s02_caller = "module app_s02@1 [ export plus2 def plus2 [ m.inc m.inc ] contract 3 plus2-law [ subject plus2 input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) (add 1 1))) ] ] proof 3 plus2-ok for plus2-law [ (export-named-claim (pc-call 0 m.inc-ok (by-true)) (pc-call 1 m.inc-ok (by-true)) (join (subst (motive (v I64) (eq (add (add (in x) 1) 1) v)) (apply (apply (apply (use ar.add-assoc-wrap) (in x)) 1) 1) (refl (add (add (in x) 1) 1))))) ] ]";
    let fix_s02 = Fixture::new(
        "lean-s02",
        &[
            ("incmod.noble", INC_MOD),
            ("imp_m.noble", "import incmod@1 as m"),
            ("arith.noble", arith_mod),
            ("imp_ar.noble", "import arith@1 as ar"),
            ("app_s02.noble", s02_caller),
        ],
    )?;
    let out_s02 = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify-module")
        .arg(fix_s02.0.join("app_s02.noble"))
        .arg("--module")
        .arg(fix_s02.0.join("incmod.noble"))
        .arg("--module")
        .arg(fix_s02.0.join("imp_m.noble"))
        .arg("--module")
        .arg(fix_s02.0.join("arith.noble"))
        .arg("--module")
        .arg(fix_s02.0.join("imp_ar.noble"))
        .args(["--timeout-ms", "600000"])
        .env("NOBLE_CONTRACT_LIBRARY", super::library_path())
        .output()?;
    let stdout_s02 = String::from_utf8(out_s02.stdout)?;
    assert!(
        out_s02.status.success(),
        "verify-module S02 failed: {stdout_s02}"
    );
    let json_s02: serde_json::Value = serde_json::from_str(&stdout_s02)?;
    assert_eq!(json_s02["outcome"], "proved");
    Ok(())
}

#[test]
#[ignore = "requires NOBLE_LEAN sandbox toolchain"]
fn rev3_strict_lean_consumer_accepts_left_and_requires() -> Result<(), Box<dyn std::error::Error>> {
    super::require_sandbox_tools()?;
    let left_and_mod = "module left_and@1 [ export f def f [ dup drop ] contract 3 f-law [ subject f input [ x I64 ] output [ y I64 ] requires [ (and (and (eq (in x) (in x)) (eq 1 1)) (eq 2 2)) ] ensures [ (eq (out y) (in x)) ] ] proof 3 f-ok for f-law [ (export-named-claim (join (use f0))) ] ]";
    let fix_la = Fixture::new("lean-left-and", &[("left_and.noble", left_and_mod)])?;
    let out_la = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify-module")
        .arg(fix_la.0.join("left_and.noble"))
        .args(["--report-schema", "v2", "--timeout-ms", "600000"])
        .env("NOBLE_CONTRACT_LIBRARY", super::library_path())
        .output()?;
    let stdout_la = String::from_utf8(out_la.stdout)?;
    assert!(
        out_la.status.success(),
        "verify-module left_and failed: {stdout_la}"
    );
    let json_la: serde_json::Value = serde_json::from_str(&stdout_la)?;
    assert_eq!(json_la["outcome"], "proved");
    assert_eq!(
        json_la["proofs"][0]["subject"]["generated_claim"],
        "NamedV3Obligation.claim"
    );
    Ok(())
}

#[test]
#[ignore = "requires NOBLE_LEAN sandbox toolchain"]
fn rev3_strict_lean_consumer_accepts_folded_requires_and_bridge(
) -> Result<(), Box<dyn std::error::Error>> {
    super::require_sandbox_tools()?;
    let id_mod = "module idmod@1 [ export id export id-ok def id [ 0 + ] contract 3 id-law [ subject id input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 0)) ] ] proof 3 id-ok for id-law [ (export-named-claim (join (refl (add (in x) 0)))) ] ]";
    let folded_mod = "module folded_bridge@1 [ export f def f [ d.id ] contract 3 f-law [ subject f input [ x I64 ] output [ y I64 ] requires [ (and (eq (add 2 3) 5) (not (and (eq 1 2) (eq (in x) (in x))))) ] ensures [ (eq 1 1) ] ] proof 3 f-ok for f-law [ (export-named-claim (pc-call 0 d.id-ok (use f0)) (join (by-true))) ] ]";
    let fix_fold = Fixture::new(
        "lean-folded",
        &[
            ("idmod.noble", id_mod),
            ("imp_d.noble", "import idmod@1 as d"),
            ("folded.noble", folded_mod),
        ],
    )?;
    let out_f = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify-module")
        .arg(fix_fold.0.join("folded.noble"))
        .arg("--module")
        .arg(fix_fold.0.join("idmod.noble"))
        .arg("--module")
        .arg(fix_fold.0.join("imp_d.noble"))
        .args(["--timeout-ms", "600000"])
        .env("NOBLE_CONTRACT_LIBRARY", super::library_path())
        .output()?;
    let stdout_f = String::from_utf8(out_f.stdout)?;
    assert!(
        out_f.status.success(),
        "verify-module folded.noble failed: {stdout_f}"
    );
    let json_f: serde_json::Value = serde_json::from_str(&stdout_f)?;
    assert_eq!(json_f["outcome"], "proved");
    Ok(())
}

#[test]
#[ignore = "requires NOBLE_LEAN sandbox toolchain"]
fn rev3_strict_lean_consumer_accepts_nested_callee_ensures(
) -> Result<(), Box<dyn std::error::Error>> {
    super::require_sandbox_tools()?;
    let id_mod = "module idmod@1 [ export id export id-ok def id [ 0 + ] contract 3 id-law [ subject id input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 0)) ] ] proof 3 id-ok for id-law [ (export-named-claim (join (refl (add (in x) 0)))) ] ]";
    let ce_callee = "module ce@1 [ export inc export inc-ok def inc [ 1 + ] contract 3 inc-law [ subject inc input [ x I64 ] output [ y I64 ] requires [ (lt 0 (in x)) ] ensures [ (and (and (eq (out y) (add (in x) 1)) (eq 1 1)) (eq 2 2)) ] ] proof 3 inc-ok for inc-law [ (export-named-claim (join (refl (add (in x) 1)))) ] ]";
    let ce_caller = "module ce_caller@1 [ export f def f [ ce.inc d.id ] contract 3 f-law [ subject f input [ x I64 ] output [ y I64 ] requires [ (lt 0 (in x)) ] ensures [ (eq 1 1) ] ] proof 3 f-ok for f-law [ (export-named-claim (pc-call 0 ce.inc-ok (use f0)) (pc-call 1 d.id-ok (use f2)) (join (use f3))) ] ]";
    let fix_ce = Fixture::new(
        "lean-ce-caller",
        &[
            ("idmod.noble", id_mod),
            ("imp_d.noble", "import idmod@1 as d"),
            ("ce.noble", ce_callee),
            ("imp_ce.noble", "import ce@1 as ce"),
            ("ce_caller.noble", ce_caller),
        ],
    )?;
    let out_f = Command::new(env!("CARGO_BIN_EXE_noble"))
        .arg("verify-module")
        .arg(fix_ce.0.join("ce_caller.noble"))
        .arg("--module")
        .arg(fix_ce.0.join("idmod.noble"))
        .arg("--module")
        .arg(fix_ce.0.join("imp_d.noble"))
        .arg("--module")
        .arg(fix_ce.0.join("ce.noble"))
        .arg("--module")
        .arg(fix_ce.0.join("imp_ce.noble"))
        .args(["--timeout-ms", "600000"])
        .env("NOBLE_CONTRACT_LIBRARY", super::library_path())
        .output()?;
    let stdout_f = String::from_utf8(out_f.stdout)?;
    assert!(
        out_f.status.success(),
        "verify-module ce_caller.noble failed: {stdout_f}"
    );
    let json_f: serde_json::Value = serde_json::from_str(&stdout_f)?;
    assert_eq!(json_f["outcome"], "proved");
    Ok(())
}
