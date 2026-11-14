use super::*;

fn repository_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root resolves")
}

fn clean_source_commit() -> String {
    let root = repository_root();
    let revision = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&root)
        .output()
        .expect("git rev-parse runs");
    assert!(revision.status.success(), "source revision is readable");
    let dirty = std::process::Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(&root)
        .output()
        .expect("git status runs");
    assert!(dirty.status.success(), "source cleanliness is readable");
    assert!(dirty.stdout.is_empty(), "receipt source commit is clean");
    String::from_utf8(revision.stdout)
        .expect("git revision is UTF-8")
        .trim()
        .to_owned()
}

#[test]
#[ignore = "writes a source-bound cache differential receipt to NOBLE_CACHE_RECEIPT_OUT"]
fn produce_source_bound() {
    let source_revision = clean_source_commit();
    let cases = CASES
        .iter()
        .enumerate()
        .map(|(index, case)| case::measure_fixture(index, *case))
        .collect::<Vec<_>>();
    let receipt = serde_json::json!({
        "schema": "noble-ngraph-specialization-cache-measurement/v1",
        "source_revision": source_revision,
        "finite_implementation_evidence": true,
        "not_a_performance_generalization": true,
        "cases": cases,
    });
    let output = std::env::var_os("NOBLE_CACHE_RECEIPT_OUT")
        .expect("NOBLE_CACHE_RECEIPT_OUT names an output outside the repository");
    let output = std::path::PathBuf::from(output);
    assert!(!output.exists(), "receipt output is absent before writing");
    let parent = output
        .parent()
        .expect("receipt output has a parent directory")
        .canonicalize()
        .expect("receipt output directory exists");
    assert!(
        !parent.starts_with(repository_root()),
        "receipt output is outside the repository"
    );
    std::fs::write(
        output,
        serde_json::to_vec_pretty(&receipt).expect("receipt serializes"),
    )
    .expect("receipt is written outside the repository");
}
