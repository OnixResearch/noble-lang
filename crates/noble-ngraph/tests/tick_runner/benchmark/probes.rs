use super::domain::{events, Kind};
use super::encoding::quote;
use super::execution::run;
use super::reporting::scenario_receipt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

fn checked_command(name: &str, args: &[&str]) -> String {
    let output = Command::new(name)
        .args(args)
        .output()
        .expect("required identity command");
    assert!(output.status.success(), "{name} identity command refused");
    String::from_utf8(output.stdout)
        .expect("UTF-8 identity")
        .trim()
        .to_owned()
}

fn tool_digest(name: &str, environment: &str) -> (String, String) {
    let selected = std::env::var_os(environment)
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("PATH").and_then(|paths| {
                std::env::split_paths(&paths)
                    .map(|dir| dir.join(name))
                    .find(|candidate| candidate.is_file())
            })
        })
        .expect("selected compiler tool path");
    let path = selected
        .canonicalize()
        .expect("canonical compiler tool path");
    let text = path.to_str().expect("UTF-8 compiler tool path");
    let digest = checked_command("sha256sum", &[text]);
    (
        text.to_owned(),
        digest
            .split_whitespace()
            .next()
            .expect("tool digest")
            .to_owned(),
    )
}

fn source_identity(output: &Path) -> (String, String, String, String) {
    let root = PathBuf::from(checked_command("git", &["rev-parse", "--show-toplevel"]));
    assert!(output.is_absolute(), "benchmark output must be absolute");
    let parent = output
        .parent()
        .expect("output parent")
        .canonicalize()
        .expect("existing output parent");
    assert!(
        !parent.starts_with(&root),
        "receipt output must be outside the repository"
    );
    assert!(
        !output.exists(),
        "never overwrite a retained measurement receipt"
    );
    assert!(
        checked_command("git", &["status", "--porcelain", "--untracked-files=all"]).is_empty(),
        "benchmark requires committed clean source"
    );
    let commit = checked_command("git", &["rev-parse", "HEAD"]);
    let tree = checked_command("git", &["rev-parse", "HEAD^{tree}"]);
    let exe = std::env::current_exe().expect("current test executable");
    let artifact = checked_command("sha256sum", &[exe.to_str().expect("UTF-8 executable path")]);
    let artifact = artifact
        .split_whitespace()
        .next()
        .expect("sha256sum digest")
        .to_owned();
    let criteria = root.join("verification/tick-runner/continuation-criteria.md");
    let criteria_digest = checked_command(
        "sha256sum",
        &[criteria.to_str().expect("UTF-8 criteria path")],
    );
    let criteria_digest = criteria_digest
        .split_whitespace()
        .next()
        .expect("criteria digest")
        .to_owned();
    (commit, tree, artifact, criteria_digest)
}

fn cpu_model() -> String {
    let Ok(file) = fs::File::open("/proc/cpuinfo") else {
        return "unknown (/proc/cpuinfo unavailable)".to_owned();
    };
    let mut contents = String::new();
    if file.take(64 * 1024).read_to_string(&mut contents).is_err() {
        return "unknown (/proc/cpuinfo unreadable)".to_owned();
    }
    contents
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            ["model name", "Processor", "Hardware"]
                .contains(&key.trim())
                .then(|| value.trim().to_owned())
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown (CPU model absent in bounded /proc/cpuinfo)".to_owned())
}

fn optimization_identity() -> String {
    let profile = option_env!("PROFILE").unwrap_or("unknown");
    let opt_level = option_env!("OPT_LEVEL").unwrap_or("unknown");
    let debug_assertions = option_env!("DEBUG_ASSERTIONS").unwrap_or("unknown");
    format!(
        "PROFILE={profile}; OPT_LEVEL={opt_level}; DEBUG_ASSERTIONS={debug_assertions}; cfg(debug_assertions)={}",
        cfg!(debug_assertions)
    )
}

#[test]
fn opt_in_source_bound_cost_comparison() {
    let Some(destination) = std::env::var_os("NOBLE_TICK_BENCH_PATH") else {
        return;
    };
    let path = PathBuf::from(destination);
    let (commit, tree, executable, criteria) = source_identity(&path);
    let (rustc_path, rustc_sha) = tool_digest("rustc", "RUSTC");
    let (cargo_path, cargo_sha) = tool_digest("cargo", "CARGO");
    let rustc = checked_command(&rustc_path, &["-Vv"]);
    let mut cases = Vec::with_capacity(9);
    for kind in [Kind::Add, Kind::Two, Kind::Sub] {
        for tick_count in [32, 512, 4096] {
            cases.push(scenario_receipt(kind, tick_count));
        }
    }
    let report = format!("{{\"schema\":\"noble-tick-cost/v1\",\"profile\":\"noble-tick-i64/v1\",\"source_commit\":{},\"source_tree\":{},\"test_executable_sha256\":{},\"criteria_sha256\":{},\"rustc\":{},\"rustc_path\":{},\"rustc_sha256\":{},\"cargo_path\":{},\"cargo_sha256\":{},\"identity_scheme\":\"test-siphash13x4 (not deployed)\",\"optimization\":{},\"host\":{},\"cpu_model\":{},\"rounds\":11,\"first_process_round\":0,\"warm_rounds\":10,\"cases\":[{}]}}\n",
        quote(&commit), quote(&tree), quote(&executable), quote(&criteria), quote(&rustc),
        quote(&rustc_path), quote(&rustc_sha), quote(&cargo_path), quote(&cargo_sha),
        quote(&optimization_identity()),
        quote(&checked_command("uname", &["-smr"])), quote(&cpu_model()), cases.join(","));
    fs::write(path, report).expect("write source-bound receipt to caller-selected output");
}

/// A separately selected, single-arm process for external DHAT. Allocation
/// totals include fixture construction and test-harness effects; report the
/// zero-operation control and inspect DHAT program points before attributing
/// any delta to Runner itself. No global allocator is installed by this test.
#[test]
fn opt_in_allocation_observation() {
    let Some(arm) = std::env::var("NOBLE_TICK_ALLOC_ARM").ok() else {
        return;
    };
    assert!(
        ["runner", "graph", "native"].contains(&arm.as_str()),
        "unknown allocation arm"
    );
    let tick_count = std::env::var("NOBLE_TICK_ALLOC_LENGTH")
        .expect("explicit allocation length")
        .parse::<usize>()
        .expect("numeric allocation length");
    assert!(
        [0, 32, 512].contains(&tick_count),
        "allocation length outside frozen probes"
    );
    assert!(
        checked_command("git", &["status", "--porcelain", "--untracked-files=all"]).is_empty(),
        "allocation probe requires committed clean source"
    );
    let commit = checked_command("git", &["rev-parse", "HEAD"]);
    let exe = std::env::current_exe().expect("exact test executable");
    let digest = checked_command("sha256sum", &[exe.to_str().expect("UTF-8 executable path")]);
    let executable_sha = digest
        .split_whitespace()
        .next()
        .expect("test executable digest");
    let operations = events(Kind::Add, tick_count);
    let result = run(
        if arm == "runner" {
            "runner"
        } else if arm == "graph" {
            "graph"
        } else {
            "native"
        },
        Kind::Add,
        &operations,
        false,
        false,
    );
    println!("ALLOC {{\"schema\":\"noble-tick-allocation-observation/v1\",\"arm\":{},\"successful_ticks\":{},\"source_commit\":{},\"test_executable_sha256\":{},\"operation_count\":{},\"harness_included\":true,\"scratch_bytes_declared\":{},\"retained_bytes_charged\":{}}}",
        quote(&arm), tick_count, quote(&commit), quote(executable_sha), operations.len(),
        result.scratch_bytes.map_or("null".to_owned(), |v| v.to_string()),
        result.retained_bytes);
}
