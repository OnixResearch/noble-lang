//! Consumer-visible named v2 proof controls. The strict-kernel cases run when
//! the pinned Lean sandbox tools are explicitly configured for this host.
use std::{io::Write,path::{Path, PathBuf}, process::Command};

const DEFINITIONS: &str = "module Definitions@5 [ export step def step [ 1 + ] ]";
const IMPORT: &str = "import Definitions@5 as d";
const SUBJECT: &str = "module Subject@3 [ def twice [ d.step d.step ] contract 2 twice-law [ subject twice input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ] proof 2 twice-correct for twice-law [ (export-named-unary-I64 (pc-named-call 0 (exec-literal-1) (exec-add)) (pc-named-call 1 (exec-literal-1) (exec-add)) (by-exact-tail) (named-true-eq-wrap)) ] ]";

struct Fixture(PathBuf);
impl Fixture {
    fn new(temporary: &Path, label: &str) -> std::io::Result<Self> {
        let base = temporary.join(format!("noble-named-v2-{label}-{}",std::process::id()));
        std::fs::create_dir(&base)?;
        std::fs::write(base.join("definitions.noble"),DEFINITIONS)?;
        std::fs::write(base.join("import.noble"),IMPORT)?;
        Ok(Self(base))
    }
    fn command(&self, source: &str) -> std::io::Result<Command> {
        let target=self.0.join("subject.noble");
        std::fs::write(&target,source)?;
        let mut cmd=Command::new(env!("CARGO_BIN_EXE_noble"));
        cmd.arg("verify-module").arg(&target)
            .arg("--module").arg(self.0.join("definitions.noble"))
            .arg("--module").arg(self.0.join("import.noble"))
            .args(["--timeout-ms","600000"]);
        cmd.env("NOBLE_CONTRACT_LIBRARY",library_path());
        Ok(cmd)
    }
}
fn library_path()->PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proofs/mc1")
}
impl Drop for Fixture {
    fn drop(&mut self) { let _=std::fs::remove_dir_all(&self.0); }
}
fn run(cmd:&mut Command)->Result<(bool,String),Box<dyn std::error::Error>> {
    let output=cmd.output()?;
    Ok((output.status.success(),String::from_utf8(output.stdout)?))
}
/// Copies the trusted Lean library tree for the hostile model mutation. Like the
/// CLI's rule-library loader, it refuses symlinks (a descent cycle or an escape
/// from the reviewed root) and charges every path, Lean or not, to 512 paths and
/// Lean source to 4 MiB, so a tree past either bound could not be loaded anyway.
/// Refusing every special file, which could block a copy, is a stricter test-copy guard.
fn copy_lean_tree(from:&Path,to:&Path)->std::io::Result<()> {
    const PATH_BUDGET:usize=512;
    const SOURCE_BUDGET:u64=4_194_304;
    let refuse=|reason:String| std::io::Error::new(std::io::ErrorKind::InvalidData,reason);
    std::fs::create_dir_all(to)?;
    let mut remaining_paths=PATH_BUDGET;
    let mut remaining_bytes=SOURCE_BUDGET;
    let mut pending = vec![(to.to_path_buf(), std::fs::read_dir(from)?)];
    while let Some((destination, entries)) = pending.last_mut() {
        let Some(entry) = entries.next() else {
            pending.pop();
            continue;
        };
        let entry=entry?;
        let path=entry.path();
        let Some(paths)=remaining_paths.checked_sub(1) else {
            return Err(refuse(format!("{} exceeds the {PATH_BUDGET}-path library budget",path.display())));
        };
        remaining_paths=paths;
        let target=destination.join(entry.file_name());
        let kind=entry.file_type()?;
        if kind.is_dir() {
            std::fs::create_dir_all(&target)?;
            pending.push((target, std::fs::read_dir(&path)?));
            continue;
        }
        if !kind.is_file() {
            return Err(refuse(format!("{} is a symlink or special file, not reviewed source",path.display())));
        }
        if path.extension().is_none_or(|ext| ext!="lean") {
            continue;
        }
        let size=entry.metadata()?.len();
        remaining_bytes=remaining_bytes.checked_sub(size).ok_or_else(||
            refuse(format!("{} exceeds the {SOURCE_BUDGET}-byte library budget",path.display())))?;
        if std::fs::copy(&path,&target)?!=size {
            return Err(refuse(format!("{} changed while it was copied",path.display())));
        }
    }
    Ok(())
}

#[test]
fn wrong_output_weak_claim_and_cross_version_cannot_verify() -> Result<(),Box<dyn std::error::Error>> {
    let fixture=Fixture::new(&std::env::temp_dir(), "refusals")?;
    for (label,source) in [
        ("wrong output",SUBJECT.replace("(add (in x) 2)","(add (in x) 3)")),
        ("weak claim",SUBJECT.replace("(eq (out y) (add (in x) 2))","true")),
        ("cross version",SUBJECT.replace("proof 2 twice-correct","proof 1 twice-correct")),
        ("unchecked marker",SUBJECT.replace("(export-named-unary-I64 (pc-named-call 0 (exec-literal-1) (exec-add)) (pc-named-call 1 (exec-literal-1) (exec-add)) (by-exact-tail) (named-true-eq-wrap))","(named-two-call-I64)")),
    ] {
        let (passed,report)=run(&mut fixture.command(&source)?)?;
        assert!(!passed && !report.contains("\"outcome\":\"proved\""),"{label}: {report}");
        assert!(report.contains("\"guest_requests\":0"),"{label}: unexpected guest execution");
    }
    Ok(())
}

#[test]
fn exact_named_source_uses_pinned_kernel_and_library() -> Result<(),Box<dyn std::error::Error>> {
    if ["NOBLE_LEAN","NOBLE_BWRAP","NOBLE_PRLIMIT","NOBLE_SYSTEMD_RUN"]
        .iter().any(|key| std::env::var_os(key).is_none()) { return Ok(()); }
    let fixture=Fixture::new(&std::env::temp_dir(), "consumer")?;
    let (passed,report)=run(&mut fixture.command(SUBJECT)?)?;
    assert!(passed,"actual named source was not independently proved: {report}");
    assert!(report.contains("\"claim\":\"NamedV2Obligation.claim\""),"wrong selected claim: {report}");
    assert!(report.contains("\"definition_body_uses\":"),"missing source-bound uses: {report}");
    assert!(report.contains("\"accepted_submission_root\":"),"missing invocation/body distinction: {report}");
    assert!(report.contains("\"guest_requests\":0"),"unexpected guest execution: {report}");

    let library=fixture.0.join("unreviewed-library");
    let root=library_path();
    std::fs::create_dir_all(&library)?;
    std::fs::copy(root.join("NobleContracts.lean"),library.join("NobleContracts.lean"))?;
    copy_lean_tree(&root.join("NobleContracts"),&library.join("NobleContracts"))?;
    std::fs::copy(root.join("lean-toolchain"),library.join("lean-toolchain"))?;
    let named=library.join("NobleContracts/NamedV2.lean");
    let mut changed=std::fs::read_to_string(&named)?;
    changed.push_str("\n-- unreviewed consumer model mutation\n");
    std::fs::write(named,changed)?;
    let mut command=fixture.command(SUBJECT)?;
    command.env("NOBLE_CONTRACT_LIBRARY",library);
    let (passed,report)=run(&mut command)?;
    assert!(!passed && report.contains("named-model-mismatch"),
        "unreviewed named model was not refused: {report}");
    Ok(())
}

#[test]
fn framed_session_publishes_only_the_strict_named_proof() -> Result<(),Box<dyn std::error::Error>> {
    if ["NOBLE_LEAN","NOBLE_BWRAP","NOBLE_PRLIMIT","NOBLE_SYSTEMD_RUN"]
        .iter().any(|key| std::env::var_os(key).is_none()) { return Ok(()); }
    let mut child=Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(["session","--framed","--declared-modules","--bindings","/dev/null"])
        .env("NOBLE_CONTRACT_LIBRARY",library_path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let mut stdin=child.stdin.take().ok_or("missing framed session input")?;
    for source in [DEFINITIONS,IMPORT,SUBJECT] {
        stdin.write_all(format!("{}\n",source.len()).as_bytes())?;
        stdin.write_all(source.as_bytes())?;
    }
    drop(stdin);
    let output=child.wait_with_output()?;
    let reports=String::from_utf8(output.stdout)?;
    let linked=reports.lines().nth(2).ok_or("missing proof-bearing module report")?;
    assert!(output.status.success() && linked.contains("\"outcome\":\"linked\""),
        "named proof was not atomically linked: {reports}");
    assert!(linked.contains("\"generated_statement_sha256\":")
        && linked.contains("\"generated_claim\":\"NamedV2Obligation.claim\"")
        && linked.contains("\"wire_mode\":\"named-v2-strict-kernel\"")
        && linked.contains("\"definition_body_uses\":")
        && linked.contains("\"strict_lean_axioms\":"),
        "session failed source-bound strict proof report: {linked}");
    assert!(linked.contains("\"guest_requests\":0")
        && linked.contains("\"host_requests\":0")
        && linked.contains("\"candidate_prepare_requests\":0"),
        "proof linking requested guest effects: {linked}");
    Ok(())
}

#[cfg(target_os="linux")]
fn children(pid:u32)->Vec<u32> {
    let mut pending=vec![pid];
    let mut found=Vec::new();
    while let Some(parent)=pending.pop() {
        let path=format!("/proc/{parent}/task/{parent}/children");
        if let Ok(contents)=std::fs::read_to_string(path) {
            for text in contents.split_whitespace() {
                if let Ok(child)=text.parse::<u32>() {
                    if !found.contains(&child) && found.len()<128 {
                        found.push(child);
                        pending.push(child);
                    }
                }
            }
        }
    }
    found
}

#[cfg(target_os="linux")]
fn named_consumer(pid:u32,lean:&Path)->bool {
    let path=PathBuf::from(format!("/proc/{pid}"));
    if std::fs::canonicalize(path.join("exe")).ok().as_deref()!=Some(lean) {
        return false;
    }
    let Ok(argv)=std::fs::read(path.join("cmdline")) else { return false; };
    argv.split(|byte|*byte==0).any(|arg|arg==b"/consumer/NamedV2Consumer.lean")
}

#[cfg(target_os="linux")]
#[test]
fn dying_real_named_consumer_cannot_report_acceptance() -> Result<(),Box<dyn std::error::Error>> {
    if ["NOBLE_LEAN","NOBLE_BWRAP","NOBLE_PRLIMIT","NOBLE_SYSTEMD_RUN"]
        .iter().any(|key| std::env::var_os(key).is_none()) { return Ok(()); }
    let fixture=Fixture::new(&std::env::temp_dir(), "consumer-fault")?;
    let lean=std::fs::canonicalize(std::env::var_os("NOBLE_LEAN").ok_or("Lean not configured")?)?;
    let mut child=fixture.command(SUBJECT)?
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let until=std::time::Instant::now()+std::time::Duration::from_secs(150);
    let mut killed=false;
    while std::time::Instant::now()<until && child.try_wait()?.is_none() {
        for pid in children(child.id()) {
            if named_consumer(pid,&lean) && named_consumer(pid,&lean) {
                let status=Command::new("kill").args(["-KILL",&pid.to_string()]).status()?;
                if status.success() { killed=true; break; }
            }
        }
        if killed { break; }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if !killed {
        child.kill()?;
        let output=child.wait_with_output()?;
        return Err(format!("exact pinned named consumer was never observed: {}",String::from_utf8_lossy(&output.stdout)).into());
    }
    let output=child.wait_with_output()?;
    let report=String::from_utf8(output.stdout)?;
    assert!(!output.status.success() && !report.contains("\"outcome\":\"proved\"")
        && report.contains("\"independent_recheck\":false"),
        "terminated kernel consumer granted acceptance: {report}");
    Ok(())
}
