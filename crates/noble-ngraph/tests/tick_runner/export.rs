//! Clean-source producer of the actual runner's finite compiler-lab replay trace.
//! Enabled only by NOBLE_TICK_EXPORT_PATH; ordinary tests never write files.

#[path = "export/identity.rs"]
mod identity;
#[path = "export/model.rs"]
mod model;
#[path = "export/rows.rs"]
mod rows;
#[path = "export/sequence.rs"]
mod sequence;

use super::{add_graph, sub_graph, two_graph, ONE, TWO};
use crate::common::IDS;
use noble_ngraph::tick::Runner;

fn header(source_commit: &str) -> String {
    let source_tree = identity::command("git", &["rev-parse", "HEAD^{tree}"]);
    let rustc = identity::command("rustc", &["--version"]);
    let rustc_sha256 = identity::tool_digest("rustc");
    let cargo_sha256 = identity::tool_digest("cargo");
    let producer_sha256 = identity::producer_digest();
    let runner_artifact_sha256 =
        identity::digest(&std::fs::read(std::env::current_exe().unwrap()).unwrap());
    let graphs = [
        model::graph_entry("add", add_graph(), ONE, &[13], &[2]),
        model::graph_entry("sub", sub_graph(), ONE, &[101], &[7]),
        model::graph_entry("two", two_graph(), TWO, &[17, -13], &[2, 3]),
    ]
    .join(",");
    format!("{{\"schema\":\"noble-tick-export/v1\",\"profile\":\"noble-tick-i64/v1\",\"source_commit\":\"{source_commit}\",\"source_tree\":\"{source_tree}\",\"rustc\":\"{rustc}\",\"rustc_sha256\":\"{rustc_sha256}\",\"cargo_sha256\":\"{cargo_sha256}\",\"identity_scheme\":\"{}\",\"producer_sha256\":\"{producer_sha256}\",\"runner_artifact_sha256\":\"{runner_artifact_sha256}\",\"graphs\":[{graphs}],\"initial\":{{\"graph_key\":\"add\",\"state\":[\"13\"],\"controls\":[\"2\"]}},\"limits\":{{\"max_graphs\":16,\"max_nodes\":4096,\"max_words\":256,\"max_events\":65536,\"max_ticks\":3,\"max_work_per_tick\":4096,\"max_trace_bytes\":16777216}}}}", noble_ngraph::IdentityScheme::name(&IDS))
}

#[test]
fn clean_source_trace_when_requested() {
    let Some(path) = std::env::var_os("NOBLE_TICK_EXPORT_PATH") else {
        return;
    };
    assert!(
        identity::command("git", &["status", "--porcelain"]).is_empty(),
        "the producer must run from a clean committed tree"
    );
    let source_commit = identity::command("git", &["rev-parse", "HEAD"]);
    let mut trace = rows::Trace::new(header(&source_commit));
    let mut r = Runner::prepare(
        &IDS,
        super::start(add_graph(), ONE, &[13], &[2]),
        noble_ngraph::tick::Limits {
            ticks: 3,
            ..Default::default()
        },
    )
    .unwrap();
    sequence::replay(&mut trace, &mut r);
    let bytes = trace.finish();
    assert!(bytes.len() <= 16 * 1024 * 1024, "trace cap");
    std::fs::write(&path, bytes.as_bytes()).expect("retained trace outside repository");
    println!(
        "TICK_EXPORT source_commit={source_commit} path={}",
        path.to_string_lossy()
    );
}
