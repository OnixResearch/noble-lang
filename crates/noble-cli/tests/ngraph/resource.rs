//! Source-bound byte-certificate replay against the opt-in CLI net backend.
//! The only test target is the existing `ngraph` integration target.

use super::{eval_lines, vector_file, Ngraph, Scratch, TestResult};
use noble_ngraph::identity::{IdentityHasher, IdentityScheme};
use noble_ngraph::{net, resource, GraphBuilder, ScalarType, SubProfile, Value};
use serde_json::json;

struct Blake3;
struct Running(blake3::Hasher);

impl IdentityHasher for Running {
    fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    fn finish(self: Box<Self>) -> [u8; 32] {
        *self.0.finalize().as_bytes()
    }
}

impl IdentityScheme for Blake3 {
    fn name(&self) -> &'static str {
        "blake3"
    }

    fn start(&self) -> Box<dyn IdentityHasher + '_> {
        Box::new(Running(blake3::Hasher::new()))
    }
}

const IDS: Blake3 = Blake3;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unhex(text: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if !text.len().is_multiple_of(2) {
        return Err("retained hex field has odd length".into());
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| -> Result<u8, Box<dyn std::error::Error>> {
            Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?)
        })
        .collect()
}

fn chain(count: usize) -> (noble_ngraph::ir::Graph, Vec<Value>) {
    let subject = noble_ngraph::ir::Subject::authored(&IDS, &format!("resource-chain-{count}"));
    let mut builder = GraphBuilder::new(SubProfile::I64, subject);
    let input = builder.input(ScalarType::I64);
    let mut current = input;
    for _ in 0..count {
        current = builder.op(noble_ngraph::Op::I64Add(current, input));
    }
    builder.output(current);
    (builder.finish(), vec![Value::I64(i64::MAX)])
}

fn balanced(
    depth: usize,
) -> Result<(noble_ngraph::ir::Graph, Vec<Value>), std::num::TryFromIntError> {
    let subject = noble_ngraph::ir::Subject::authored(&IDS, &format!("resource-tree-{depth}"));
    let mut builder = GraphBuilder::new(SubProfile::I64, subject);
    let count = 1usize << depth;
    let mut row = Vec::with_capacity(count);
    let mut nodes = Vec::with_capacity(count);
    for index in 0..count {
        nodes.push(builder.input(ScalarType::I64));
        row.push(Value::I64(i64::try_from(index)? - 4));
    }
    while nodes.len() > 1 {
        nodes = nodes
            .chunks_exact(2)
            .map(|pair| builder.op(noble_ngraph::Op::I64Sub(pair[0], pair[1])))
            .collect();
    }
    builder.output(nodes[0]);
    Ok((builder.finish(), row))
}

fn fanout_erase(copies: usize) -> (noble_ngraph::ir::Graph, Vec<Value>) {
    let subject = noble_ngraph::ir::Subject::authored(&IDS, &format!("resource-fanout-{copies}"));
    let mut builder = GraphBuilder::new(SubProfile::I64, subject);
    let live = builder.input(ScalarType::I64);
    let _discarded = builder.input(ScalarType::I64);
    for _ in 0..copies {
        builder.output(live);
    }
    (builder.finish(), vec![Value::I64(-17), Value::I64(22)])
}

fn controls(
    checked: &noble_ngraph::CheckedGraph,
    row: &[Value],
    artifact: &resource::Artifact,
) -> serde_json::Value {
    let certificate = &artifact.certificate_bytes;
    let net = &artifact.net_bytes;
    let count_at = certificate.len() - IDS.name().len() - 1 - 16;
    let mut wrong_count = certificate.clone();
    wrong_count[count_at..count_at + 8].copy_from_slice(&0u64.to_le_bytes());
    let mut wrong_bound = certificate.clone();
    wrong_bound[count_at + 8..count_at + 16].copy_from_slice(&0u64.to_le_bytes());
    let mut wrong_cost = certificate.clone();
    wrong_cost[count_at - 32] ^= 1;
    let mut changed_net = net.clone();
    changed_net[8] ^= 1;
    let mut stale_rules = resource::RULE_TABLE_BYTES.to_vec();
    stale_rules.pop();
    let truncated = &certificate[..certificate.len() - 1];
    let refusal = |cert: &[u8], net_bytes: &[u8], rules: &[u8]| {
        format!(
            "{:?}",
            resource::check(
                &IDS,
                checked,
                row,
                resource::CheckBytes {
                    certificate: cert,
                    net: net_bytes,
                    rules
                }
            )
            .expect_err("forged resource certificate must refuse")
        )
    };
    let observed = json!({
        "wrong_interactions": refusal(&wrong_count, net, resource::RULE_TABLE_BYTES),
        "wrong_live_bound": refusal(&wrong_bound, net, resource::RULE_TABLE_BYTES),
        "changed_net_bytes": refusal(certificate, &changed_net, resource::RULE_TABLE_BYTES),
        "stale_rule_table": refusal(certificate, net, &stale_rules),
        "wrong_cost_model": refusal(&wrong_cost, net, resource::RULE_TABLE_BYTES),
        "truncated_certificate": refusal(truncated, net, resource::RULE_TABLE_BYTES)
    });
    assert_eq!(
        observed,
        json!({
            "wrong_interactions": "WrongInteractionCount",
            "wrong_live_bound": "WrongLiveAgentBound",
            "changed_net_bytes": "NetDigestMismatch",
            "stale_rule_table": "StaleRuleTable",
            "wrong_cost_model": "WrongCostModel",
            "truncated_certificate": "Malformed"
        })
    );
    observed
}

#[test]
fn byte_certificates_replay_against_cli_traces() -> TestResult {
    let scratch = Scratch::new("resource-replay")?;
    let cases = [
        ("chain-3", chain(3), 9, 7),
        ("chain-5", chain(5), 15, 11),
        ("balanced-2", balanced(2)?, 6, 7),
        ("balanced-3", balanced(3)?, 14, 15),
        ("fanout-4-erase", fanout_erase(4), 4, 6),
        ("fanout-6-erase", fanout_erase(6), 6, 8),
    ];
    let mut families = Vec::new();
    let mut replay_inputs = Vec::new();
    for (name, (graph, row), count, agents) in cases {
        let printed = noble_ngraph::format::graph::print(&graph);
        let graph_path = scratch.write(&format!("{name}.ngraph"), &printed)?;
        let checked = noble_ngraph::check(&IDS, graph)
            .map_err(|error| format!("graph admission: {error:?}"))?;
        let limits = net::Limits {
            max_agent_slots: 1_048_576,
            max_wires: 2_097_152,
            interaction_fuel: 2_097_152,
        };
        let net = net::lower(&IDS, &checked, &row, &limits)
            .map_err(|error| format!("lowering: {error:?}"))?;
        let artifact = resource::issue(&IDS, &checked, &net, &row)
            .map_err(|error| format!("issuance: {error:?}"))?;
        let verified = resource::check(
            &IDS,
            &checked,
            &row,
            resource::CheckBytes {
                certificate: &artifact.certificate_bytes,
                net: &artifact.net_bytes,
                rules: resource::RULE_TABLE_BYTES,
            },
        )
        .map_err(|error| format!("independent checker: {error:?}"))?;
        assert_eq!(
            (verified.interactions, verified.live_agent_bound),
            (count, agents),
            "{name}"
        );

        let types = vec!["i64"; row.len()].join(" ");
        let values = row
            .iter()
            .map(|value| match value {
                Value::I64(number) => number.to_string(),
                other => panic!("outside I64 profile: {other:?}"),
            })
            .collect::<Vec<_>>()
            .join(" ");
        let vectors = scratch.write(
            &format!("{name}.vectors"),
            &vector_file(&types, &[&format!("v {values}")]),
        )?;
        let run = scratch.run(
            &Ngraph::new("eval")
                .arg(&graph_path)
                .arg(&vectors)
                .arg("--net"),
        )?;
        let (header, lines) = eval_lines(&run)?;
        assert_eq!(header["backend"], "net");
        assert_eq!(lines.len(), 1, "one input row per family");
        let trace = &lines[0];
        assert_eq!(trace["outcome"], "normal", "{trace}");
        assert_eq!(trace["interactions"], count, "{name}: {trace}");
        assert!(
            trace["peak_live_agents"]
                .as_u64()
                .ok_or("missing peak live agents")?
                <= agents,
            "{name}: {trace}"
        );

        for schedule in [
            net::Schedule::Lifo,
            net::Schedule::Fifo,
            net::Schedule::SplitMix64 { seed: 0x109 },
        ] {
            let report = net::reduce(
                &IDS,
                net::ReduceRequest {
                    checked: &checked,
                    candidate: &net,
                    inputs: &row,
                    limits: &limits,
                    plan: net::Plan {
                        schedule: Some(schedule),
                        interaction_fuel: None,
                        cancel: None,
                        storage_policy: None,
                        max_plan_scratch_bytes: None,
                        max_worklist_entries: None,
                        plan_observer: None,
                    },
                },
            );
            assert_eq!(
                report.interactions, verified.interactions,
                "{name}: {schedule:?}"
            );
            assert!(
                report.peak_live_agents <= verified.live_agent_bound,
                "{name}: {schedule:?}"
            );
            let net::Obs::Normal(values) = report.obs else {
                panic!("{name}: {schedule:?} did not normalize: {report:?}");
            };
            let rendered = values
                .iter()
                .map(|value| noble_ngraph::format::value::render(*value))
                .collect::<Vec<_>>();
            assert_eq!(trace["outputs"], json!(rendered), "{name}: {schedule:?}");
        }
        families.push(json!({
            "name": name,
            "graph_id": hex(&checked.id()),
            "graph": printed,
            "inputs": values,
            "net_bytes": hex(&artifact.net_bytes),
            "certificate_bytes": hex(&artifact.certificate_bytes),
            "net_blake3": blake3::hash(&artifact.net_bytes).to_hex().to_string(),
            "certificate_blake3": blake3::hash(&artifact.certificate_bytes).to_hex().to_string(),
            "certified_interactions": verified.interactions,
            "certified_live_agents": verified.live_agent_bound,
            "initial_wires": verified.initial_wires,
            "cli_interactions": trace["interactions"],
            "cli_peak_live_agents": trace["peak_live_agents"],
            "controls": controls(&checked, &row, &artifact)
        }));
        replay_inputs.push((checked, row));
    }
    let receipt = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../verification/ngraph/resource-receipt.json");
    if receipt.exists() {
        let retained: serde_json::Value = serde_json::from_slice(&std::fs::read(&receipt)?)?;
        assert_eq!(
            retained["families"],
            json!(&families),
            "retained bytes/trace replay drift"
        );
        for (entry, (checked, row)) in retained["families"]
            .as_array()
            .ok_or("missing retained families")?
            .iter()
            .zip(&replay_inputs)
        {
            let certificate = unhex(
                entry["certificate_bytes"]
                    .as_str()
                    .ok_or("missing certificate bytes")?,
            )?;
            let net_bytes = unhex(entry["net_bytes"].as_str().ok_or("missing net bytes")?)?;
            let verified = resource::check(
                &IDS,
                checked,
                row,
                resource::CheckBytes {
                    certificate: &certificate,
                    net: &net_bytes,
                    rules: resource::RULE_TABLE_BYTES,
                },
            )
            .map_err(|error| format!("retained checker replay: {error:?}"))?;
            assert_eq!(verified.interactions, entry["certified_interactions"]);
            assert_eq!(verified.live_agent_bound, entry["certified_live_agents"]);
        }
    }
    if let Some(path) = std::env::var_os("NOBLE_RESOURCE_RECEIPT") {
        assert!(
            !receipt.exists(),
            "do not overwrite an immutable retained receipt"
        );
        let source = std::env::var("NOBLE_RESOURCE_SOURCE_COMMIT")?;
        assert_eq!(source.len(), 40, "source commit must be full SHA");
        let artifact = json!({
            "revision": resource::CERTIFICATE_REVISION,
            "source_commit": source,
            "identity_scheme": IDS.name(),
            "checker_revision": resource::CHECKER_REVISION,
            "proof_revision": resource::PROOF_MODEL_REVISION,
            "rule_revision": net::CONTRACT_REVISION,
            "rule_table_blake3": blake3::hash(resource::RULE_TABLE_BYTES).to_hex().to_string(),
            "cost_revision": resource::COST_MODEL_REVISION,
            "cost_model_blake3": blake3::hash(resource::COST_MODEL_BYTES).to_hex().to_string(),
            "families": families,
            "limitations": "abstract interactions and committed live agents only; no Rust/Lean refinement proof, arena reservation or physical cost claim"
        });
        std::fs::write(path, serde_json::to_vec_pretty(&artifact)?)?;
    }
    Ok(())
}
