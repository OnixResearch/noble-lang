use super::{Accepted, Report};
use noble_contracts::source::chunks as chunk;

pub(crate) fn maybe_write_measurement_receipt(
    accepted: &Accepted,
    report: &Report,
    receipt_path: Option<&str>,
    source_commit: Option<&str>,
) -> Result<(), String> {
    let Some(path) = receipt_path else {
        return Ok(());
    };
    let source_commit =
        source_commit.ok_or_else(|| String::from("source commit environment is not configured"))?;
    let source_commit = source_commit.trim();
    let worktree = worktree_root()?;
    verify_source_commit(&worktree, source_commit)?;
    let output = std::path::PathBuf::from(path);
    let output_parent = canonical_output_parent(&output)?;
    if output_parent.starts_with(&worktree) {
        return Err(String::from("receipt output must be outside the worktree"));
    }
    let receipt = encode_receipt(accepted, report, source_commit);
    write_new_receipt(&output, &receipt)
}

fn worktree_root() -> Result<std::path::PathBuf, String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or_else(|| String::from("worktree root unavailable"))?;
    std::fs::canonicalize(root).map_err(|error| format!("canonicalize worktree: {error}"))
}

fn verify_source_commit(worktree: &std::path::Path, source_commit: &str) -> Result<(), String> {
    let head = git_output(worktree, &["rev-parse", "HEAD"], "read source commit")?;
    let head = String::from_utf8(head).map_err(|error| format!("decode source commit: {error}"))?;
    if source_commit.trim() != head.trim() {
        return Err(String::from("receipt source commit differs from HEAD"));
    }
    let status = git_output(worktree, &["status", "--porcelain"], "check source tree")?;
    if !status.is_empty() {
        return Err(String::from("receipt source tree is not clean"));
    }
    assert!(!source_commit.trim().is_empty());
    assert!(source_commit
        .trim()
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit()));
    Ok(())
}

fn git_output(
    worktree: &std::path::Path,
    args: &[&str],
    operation: &str,
) -> Result<Vec<u8>, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(args)
        .output()
        .map_err(|error| format!("{operation}: {error}"))?;
    if !output.status.success() {
        return Err(format!("git {operation} failed"));
    }
    Ok(output.stdout)
}

fn canonical_output_parent(output: &std::path::Path) -> Result<std::path::PathBuf, String> {
    if !output.is_absolute() {
        return Err(String::from("receipt output path must be absolute"));
    }
    let parent = output
        .parent()
        .ok_or_else(|| String::from("receipt output parent unavailable"))?;
    std::fs::canonicalize(parent).map_err(|error| format!("canonicalize receipt parent: {error}"))
}

fn write_new_receipt(output: &std::path::Path, receipt: &str) -> Result<(), String> {
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| format!("create new measurement receipt: {error}"))?;
    std::io::Write::write_all(&mut output, receipt.as_bytes())
        .map_err(|error| format!("write measurement receipt: {error}"))
}

fn encode_receipt(accepted: &Accepted, report: &Report, source_commit: &str) -> String {
    assert_eq!(report.accepted, 1);
    assert!(report.checked_candidates <= report.candidates);
    assert!(report.rejected <= report.candidates);
    assert!(u32::try_from(accepted.selected.len())
        .is_ok_and(|count| count <= chunk::MAX_COMPOSED_CHUNKS));
    format!(
        concat!(
            "{{\n",
            "  \"schema\": \"typed-chunk-measurement/1\",\n",
            "  \"source_commit\": \"{}\",\n",
            "  \"scenario\": \"two-chunk-stack-tail\",\n",
            "  \"outcome\": \"accepted\",\n",
            "  \"enumerated_candidates\": {},\n",
            "  \"checked_candidates\": {},\n",
            "  \"rejected_candidates\": {},\n",
            "  \"expanded_nodes\": {},\n",
            "  \"dependency_steps\": {},\n",
            "  \"instantiation_bindings\": {},\n",
            "  \"checking_work\": {},\n",
            "  \"evaluation_steps\": {},\n",
            "  \"source_bytes\": {},\n",
            "  \"retained_bytes\": {},\n",
            "  \"completed_source_bytes\": {},\n",
            "  \"selected_chunks\": {},\n",
            "  \"total_latency_ns\": {}\n",
            "}}\n"
        ),
        source_commit,
        report.candidates,
        report.checked_candidates,
        report.rejected,
        report.expanded_nodes,
        report.dependency_steps,
        report.instantiation_bindings,
        report.checking_work,
        report.evaluation_steps,
        report.source_bytes,
        report.retained_bytes,
        accepted.source.len(),
        accepted.selected.len(),
        report.total_latency_ns,
    )
}
