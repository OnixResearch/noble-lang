//! Clean-commit producer for the preregistered first-projection receipt.

use super::*;
use noble_ngraph::specialize::cache::Limits;
use serde_json::json;
use std::process::Command;

#[path = "measurement/record.rs"]
mod record;
#[path = "measurement/stages.rs"]
mod stages;
use record::measure_case;
use stages::timed;

const REPEATS: usize = 11;
const SAMPLES: usize = 31;
const BATCH: usize = 1_024;

fn version(spec: (&str, &str)) -> String {
    let (command, arg) = spec;
    let output = Command::new(command)
        .arg(arg)
        .output()
        .expect("tool version executable");
    assert!(output.status.success(), "tool version command: {command}");
    String::from_utf8(output.stdout)
        .expect("version UTF-8")
        .trim()
        .to_owned()
}

fn clean_revision() -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    let output = Command::new("git")
        .args(["-C", root, "status", "--porcelain", "--untracked-files=all"])
        .output()
        .expect("check source cleanliness");
    assert!(output.status.success(), "git status succeeded");
    assert!(
        output.stdout.is_empty(),
        "receipt requires clean committed tree"
    );
    let revision = Command::new("git")
        .args(["-C", root, "rev-parse", "HEAD"])
        .output()
        .expect("read source revision");
    assert!(revision.status.success());
    let revision = String::from_utf8(revision.stdout)
        .expect("revision UTF-8")
        .trim()
        .to_owned();
    assert_eq!(
        revision,
        std::env::var("NOBLE_INTERP_SOURCE_REV").expect("frozen source revision")
    );
    revision
}

fn receipt(
    revision: String,
    build_ns: u64,
    pairs: &[(i64, i64)],
    records: Vec<serde_json::Value>,
) -> serde_json::Value {
    let host = std::fs::read_to_string("/proc/cpuinfo").expect("host CPU model");
    let cpu = host
        .lines()
        .find(|line| line.starts_with("model name"))
        .unwrap_or("model name: unsupported")
        .to_owned();
    json!({
        "schema": "noble-interpreter-first-projection/v1", "issue": 34,
        "source_revision": revision, "clean_source": true,
        "criteria": "verification/interpreter-specialization/criteria.md",
        "profile": "ngraph-i64/0", "specializer": specialize::ACCOUNTING_REVISION,
        "graph_semantics": noble_ngraph::ir::SEMANTIC_REVISION,
        "compiled_backend": noble_ngraph::compiled::COMPILER_REVISION,
        "specializer_revision": "ngraph-pe-specializer/2",
        "frontend_limits": {
            "interpreter": {"bytes": 65_536, "nodes": 524_288, "depth": 64, "work": 512_000_000},
            "residual_and_direct": {
                "bytes": noble_contracts::Limits::default().bytes,
                "nodes": noble_contracts::Limits::default().nodes,
                "depth": noble_contracts::Limits::default().depth,
                "work": noble_contracts::Limits::default().work,
            },
            "graph_budgets": format!("{:?}", Budgets::default()),
        },
        "optimization_flags": {"rust_debug_assertions": cfg!(debug_assertions),
            "source": "unoptimized ordinary Noble graph; no Wasm or wasm-opt path used"},
        "hardware": {"cpu": cpu, "os": version(("uname", "-a"))},
        "toolchain": {"rustc": version(("rustc", "--version")), "cargo": version(("cargo", "--version"))},
        "dataset": {"seed": format!("0x{SEED:016x}"), "boundary_and_seeded_rows": pairs.len(),
            "timing_batch": BATCH, "timing_rows": "the same deterministic cyclic batch of boundary and seeded rows on all paths"},
        "repetitions": {"stage": REPEATS, "warm_samples": SAMPLES, "warm_batch": BATCH},
        "cache": {"limits": {"entries": Limits::default().entries,
            "variants_per_subject": Limits::default().variants_per_subject,
            "manifest_bytes": Limits::default().manifest_bytes,
            "retained_bytes": Limits::default().retained_bytes,
            "lookup_work": Limits::default().lookup_work,
            "eviction_work": Limits::default().eviction_work},
            "state": "new empty session for each setup repetition, miss then verified hit; no persisted executable"},
        "initial_interpreter_ordinary_admission_extraction_and_build_ns": build_ns,
        "unsupported": {"actual_peak_scratch_heap": "no allocator peak instrumentation; compiled workspace bytes are measured structurally",
            "end_to_end_cli_prepare_compile_instantiate": "no CLI Wasm process in this in-process graph experiment; cannot turn graph latency into CLI cost",
            "Wasm_code_bytes": "compiled backend is the in-memory layered graph evaluator, not a Wasm code emitter",
            "second_or_third_projection_cost": "no accepted representation of the Rust specializer as a Noble subject"},
        "proof_boundary": "existing FirstProjection/OperandForwarding Lean graph model, no Rust correspondence or Program refinement",
        "records": records,
    })
}

#[test]
#[ignore = "writes a receipt to the absent NOBLE_INTERP_RECEIPT path after a clean commit"]
fn source_bound_first_projection_receipt() {
    let revision = clean_revision();
    let path =
        std::env::var("NOBLE_INTERP_RECEIPT").expect("receipt destination outside repository");
    assert!(
        !std::path::Path::new(&path).exists(),
        "receipt destination must not exist"
    );
    let (build_ns, base) = timed(admit);
    let pairs = program_rows();
    let count = pairs.len();
    assert!(count != 0, "the frozen dataset has rows");
    let scalar_rows: Vec<(i64, i64)> = (0..BATCH).map(|i| pairs[i % count]).collect();
    let mut records = Vec::with_capacity(9);
    for first in Opcode::ALL {
        for second in Opcode::ALL {
            records.push(measure_case(&base, [first, second], &scalar_rows));
        }
    }
    std::fs::write(
        path,
        serde_json::to_string_pretty(&receipt(revision, build_ns, &pairs, records)).unwrap() + "\n",
    )
    .expect("write receipt");
}
