//! Source-bound byte-certificate replay against the opt-in CLI net backend.
//! The only test target is the existing `ngraph` integration target.

use super::{corpus_codec, eval_lines, vector_file, Ngraph, Scratch, TestResult};
use noble_ngraph::identity::{IdentityHasher, IdentityScheme};
use noble_ngraph::{net, resource, GraphBuilder, ScalarType, SubProfile, Value};
use serde_json::json;
use std::fmt::Write as _;

const CORPUS_PREAMBLE: &str = r#"import NobleFoundation.NetI64Resource
import NobleFoundation.NetI64Corpus

/-!
Finite byte fixture emitted from the checked Rust net and independently accepted resource
certificate. The Rust freshness test binds the literals to exact bytes and BLAKE3 digests.
The kernel checks the certificate field offsets, the translated net, and its cost/agent claims.
BLAKE3, the Rust binary decoder, and universal Rust/Lean correspondence are NOT proved here.
-/

namespace NobleFoundation.NetI64ResourceCorpus

open NobleFoundation.NetI64
open NobleFoundation.NetI64Resource

private def compareId : List Nat → List Nat → Ordering
  | [], [] => .eq
  | [], _ :: _ => .lt
  | _ :: _, [] => .gt
  | left :: leftTail, right :: rightTail =>
      match compare left right with
      | .eq => compareId leftTail rightTail
      | order => order

private def comparePort (left right : INet.Port) : Ordering :=
  match compareId left.1 right.1 with
  | .eq => compare left.2 right.2
  | order => order

private def compareWire (left right : INet.Port × INet.Port) : Ordering :=
  match comparePort left.1 right.1 with
  | .eq => comparePort left.2 right.2
  | order => order

private def insertSorted {α : Type} (less : α → α → Bool) (value : α) : List α → List α
  | [] => [value]
  | head :: rest =>
      if less value head then value :: head :: rest
      else head :: insertSorted less value rest

private def sortSorted {α : Type} (less : α → α → Bool) : List α → List α
  | [] => []
  | head :: rest => insertSorted less head (sortSorted less rest)

private def canonicalNet (net : INet.XNet NetSym) : INet.XNet NetSym :=
  { agents := sortSorted (fun left right => compareId left.1 right.1 == .lt) net.agents
    free := sortSorted (fun left right => comparePort left right == .lt) net.free
    wires := sortSorted (fun left right => compareWire left right == .lt) net.wires }

private def littleEndian (bytes : List Nat) : Nat :=
  bytes.reverse.foldl (fun value byte => value * 256 + byte) 0

"#;

fn byte_list(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn corpus_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../proofs/foundation/NobleFoundation/NetI64ResourceCorpus.lean")
}

fn retain_followup_evidence(
    checked: &noble_ngraph::CheckedGraph,
    issued: &resource::Artifact,
) -> TestResult {
    let corpus = std::fs::read_to_string(corpus_path())?;
    assert_eq!(
        corpus,
        accepted_corpus()?,
        "kernel fixture must match freshly accepted bytes before receipt production"
    );
    let evidence = json!({
        "graph_id": hex(&checked.id()),
        "net_blake3": blake3::hash(&issued.net_bytes).to_hex().to_string(),
        "certificate_blake3": blake3::hash(&issued.certificate_bytes).to_hex().to_string(),
        "lean_corpus_blake3": blake3::hash(corpus.as_bytes()).to_hex().to_string(),
        "controls": {
            "omitted_dup_cost": "StaleRuleTable",
            "negative_cost_annotation": "StaleRuleTable",
            "negative_potential": "WrongCostModel",
            "disconnected_rebound_net": "InvalidNet"
        }
    });
    let retained_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../verification/ngraph/resource-followup-2026-11-15.json");
    if retained_path.exists() {
        let retained: serde_json::Value = serde_json::from_slice(&std::fs::read(&retained_path)?)?;
        assert_eq!(retained["schema"], "ngraph-resource-followup/1");
        assert_eq!(
            retained["evidence"], evidence,
            "follow-up resource evidence drift"
        );
        assert_eq!(
            retained["source_commit"].as_str().map(str::len),
            Some(40),
            "retained source commit must be a full digest"
        );
    }
    if let Some(output) = std::env::var_os("NOBLE_RESOURCE_FOLLOWUP_RECEIPT") {
        assert!(
            !retained_path.exists(),
            "do not overwrite immutable evidence"
        );
        let source = std::env::var("NOBLE_RESOURCE_SOURCE_COMMIT")?;
        assert_eq!(source.len(), 40, "source commit must be a full SHA");
        std::fs::write(
            output,
            serde_json::to_vec_pretty(&json!({
                "schema": "ngraph-resource-followup/1",
                "source_commit": source,
                "evidence": evidence
            }))?,
        )?;
    }
    Ok(())
}

fn certificate_digest_offset() -> usize {
    8 + [
        resource::CERTIFICATE_REVISION,
        resource::CHECKER_REVISION,
        resource::PROOF_MODEL_REVISION,
        resource::COST_MODEL_REVISION,
    ]
    .iter()
    .map(|revision| 2 + revision.len())
    .sum::<usize>()
}

fn certificate_digest(domain: &[u8], bytes: &[u8]) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    let mut hasher = IDS.start();
    hasher.update(domain);
    hasher.update(&u64::try_from(bytes.len())?.to_le_bytes());
    hasher.update(bytes);
    Ok(hasher.finish())
}

fn accepted_corpus() -> Result<String, Box<dyn std::error::Error>> {
    let (graph, inputs) = fanout_erase(4);
    let checked = noble_ngraph::check(&IDS, graph).map_err(|error| format!("{error:?}"))?;
    let limits = net::Limits {
        max_agent_slots: 1_048_576,
        max_wires: 2_097_152,
        interaction_fuel: 2_097_152,
    };
    let lowered =
        net::lower(&IDS, &checked, &inputs, &limits).map_err(|error| format!("{error:?}"))?;
    let artifact =
        resource::issue(&IDS, &checked, &lowered, &inputs).map_err(|error| format!("{error:?}"))?;
    let verified = resource::check(
        &IDS,
        &checked,
        &inputs,
        resource::CheckBytes {
            certificate: &artifact.certificate_bytes,
            net: &artifact.net_bytes,
            rules: resource::RULE_TABLE_BYTES,
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    let graph = checked.graph();
    let nodes = corpus_codec::lean_nodes(graph)?;
    let rust_net = corpus_codec::rust_net_literal(&lowered)?;
    let outputs = graph
        .outputs
        .iter()
        .map(|node| node.0.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let lean_inputs = inputs
        .iter()
        .map(|value| match value {
            Value::I64(value) => format!("BitVec.ofNat 64 {}", *value as u64),
            other => panic!("unexpected corpus input: {other:?}"),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let output = corpus_codec::lean_value(inputs[0]);
    let expected_outputs = vec![output; 4].join(", ");
    let versions_bytes = certificate_digest_offset();
    let mut source = String::from(CORPUS_PREAMBLE);
    writeln!(
        source,
        "-- Exact accepted Rust bytes: net={}, certificate={}.",
        hex(&blake3::hash(&artifact.net_bytes).as_bytes()[..]),
        hex(&blake3::hash(&artifact.certificate_bytes).as_bytes()[..])
    )?;
    writeln!(source, "def program : Program := {{ inputArity := {}, nodes := [{nodes}], outputs := [{outputs}] }}", graph.slots.len())?;
    writeln!(source, "def inputs : List (BitVec 64) := [{lean_inputs}]")?;
    writeln!(source, "def rustNet : INet.XNet NetSym := {rust_net}")?;
    writeln!(
        source,
        "def rustOutputs : List NobleFoundation.PackedSchedule.Val := [{expected_outputs}]"
    )?;
    writeln!(
        source,
        "def certificateBytes : List Nat := [{}]",
        byte_list(&artifact.certificate_bytes)
    )?;
    writeln!(
        source,
        "def netBytes : List Nat := [{}]",
        byte_list(&artifact.net_bytes)
    )?;
    writeln!(
        source,
        "def netBlake3 : List Nat := [{}]",
        byte_list(blake3::hash(&artifact.net_bytes).as_bytes())
    )?;
    writeln!(
        source,
        "def certificateBlake3 : List Nat := [{}]",
        byte_list(blake3::hash(&artifact.certificate_bytes).as_bytes())
    )?;
    writeln!(
        source,
        "def graphId : List Nat := [{}]",
        byte_list(&checked.id())
    )?;
    writeln!(
        source,
        "def buildKey : List Nat := [{}]",
        byte_list(&lowered.build_key)
    )?;
    writeln!(
        source,
        "def certificateNetDigest : List Nat := [{}]",
        byte_list(&artifact.certificate_bytes[versions_bytes..versions_bytes + 32])
    )?;
    writeln!(
        source,
        "def certificateRuleDigest : List Nat := [{}]",
        byte_list(&artifact.certificate_bytes[versions_bytes + 32..versions_bytes + 64])
    )?;
    writeln!(
        source,
        "def certificateCostDigest : List Nat := [{}]",
        byte_list(&artifact.certificate_bytes[versions_bytes + 64..versions_bytes + 96])
    )?;
    let mut offset_bytes = 8;
    let mut schema = vec!["certificateBytes.take 8 = [78, 71, 82, 83, 67, 49, 0, 0]".to_owned()];
    for revision in [
        resource::CERTIFICATE_REVISION,
        resource::CHECKER_REVISION,
        resource::PROOF_MODEL_REVISION,
        resource::COST_MODEL_REVISION,
    ] {
        let length_bytes = u16::try_from(revision.len())?;
        schema.push(format!(
            "(certificateBytes.drop {offset_bytes}).take 2 = [{}]",
            byte_list(&length_bytes.to_le_bytes())
        ));
        offset_bytes += 2;
        schema.push(format!(
            "(certificateBytes.drop {offset_bytes}).take {} = [{}]",
            revision.len(),
            byte_list(revision.as_bytes())
        ));
        offset_bytes += revision.len();
    }
    assert_eq!(offset_bytes, versions_bytes);
    writeln!(
        source,
        "\n/-- The magic and all four length-prefixed identity revisions are byte-exact. -/\ntheorem fixture_schema :\n    {} := by decide",
        schema.join(" ∧\n    ")
    )?;
    writeln!(
        source,
        "\n/-- Binary field positions are decoded from the literal, rather than postulated. -/"
    )?;
    writeln!(source, "theorem fixture_byte_fields :\n    netBytes.take 8 = [78, 71, 82, 78, 69, 84, 51, 0] ∧\n    (netBytes.drop 8).take 32 = graphId ∧\n    (netBytes.drop 40).take 32 = buildKey ∧\n    (certificateBytes.drop {versions_bytes}).take 32 = certificateNetDigest ∧\n    (certificateBytes.drop {}).take 32 = certificateRuleDigest ∧\n    (certificateBytes.drop {}).take 32 = certificateCostDigest ∧\n    littleEndian ((certificateBytes.drop {}).take 8) = {} ∧\n    littleEndian ((certificateBytes.drop {}).take 8) = {} := by decide",
        versions_bytes + 32,
        versions_bytes + 64,
        versions_bytes + 96,
        verified.interactions,
        versions_bytes + 104,
        verified.live_agent_bound
    )?;
    writeln!(
        source,
        "\n/-- The independently accepted decoded net agrees with this source translation. -/"
    )?;
    writeln!(source, "theorem fixture_translation :\n    (Program.translateChecked program inputs).map canonicalNet = some (canonicalNet rustNet) := by decide")?;
    writeln!(source, "\ntheorem fixture_resources :\n    program.interactionCount = {} ∧\n    (program.translate inputs).agents.length = {} ∧\n    NobleFoundation.NetI64.checkCertificate program inputs (Certificate.forProgram program inputs) = true := by decide",
        verified.interactions, verified.live_agent_bound)?;
    writeln!(source, "\n/-- Byte-decoded claims agree with the kernel-checked translated program. -/\ntheorem fixture_byte_claims_match_translation :\n    littleEndian ((certificateBytes.drop {}).take 8) = program.interactionCount ∧\n    littleEndian ((certificateBytes.drop {}).take 8) = (program.translate inputs).agents.length := by decide",
        versions_bytes + 96, versions_bytes + 104)?;
    source.push_str("\nend NobleFoundation.NetI64ResourceCorpus\n");
    Ok(source)
}

#[test]
fn accepted_lean_corpus_is_fresh() -> TestResult {
    assert_eq!(
        std::fs::read_to_string(corpus_path())?,
        accepted_corpus()?,
        "resource proof literals differ from freshly issued and independently accepted bytes"
    );
    Ok(())
}

#[test]
#[ignore = "explicitly regenerate the source-bound Lean fixture after changing the Rust producer"]
fn regenerate_lean_corpus() -> TestResult {
    std::fs::write(corpus_path(), accepted_corpus()?)?;
    Ok(())
}

#[test]
fn byte_checker_refuses_missing_dup_negative_costs_and_false_premise() -> TestResult {
    let (graph, inputs) = fanout_erase(4);
    let checked = noble_ngraph::check(&IDS, graph).map_err(|error| format!("{error:?}"))?;
    let limits = net::Limits {
        max_agent_slots: 1_048_576,
        max_wires: 2_097_152,
        interaction_fuel: 2_097_152,
    };
    let lowered =
        net::lower(&IDS, &checked, &inputs, &limits).map_err(|error| format!("{error:?}"))?;
    let issued =
        resource::issue(&IDS, &checked, &lowered, &inputs).map_err(|error| format!("{error:?}"))?;
    let check = |certificate: &[u8], net_bytes: &[u8], rules: &[u8]| {
        resource::check(
            &IDS,
            &checked,
            &inputs,
            resource::CheckBytes {
                certificate,
                net: net_bytes,
                rules,
            },
        )
    };
    let mut omitted_dup = resource::RULE_TABLE_BYTES.to_vec();
    let at = omitted_dup
        .windows(b"dup-num:".len())
        .position(|window| window == b"dup-num:")
        .ok_or("missing DUP rule")?;
    omitted_dup.drain(at..at + b"dup-num:".len());
    assert_eq!(
        check(&issued.certificate_bytes, &issued.net_bytes, &omitted_dup),
        Err(resource::Refusal::StaleRuleTable)
    );

    let mut negative_annotation = resource::RULE_TABLE_BYTES.to_vec();
    let marker = b"dup-num: dup + num(v) -> num(v) + num(v); cost=1";
    let at = negative_annotation
        .windows(marker.len())
        .position(|window| window == marker)
        .ok_or("missing DUP charge")?
        + marker.len()
        - 1;
    negative_annotation.splice(at..at + 1, b"-1".iter().copied());
    assert_eq!(
        check(
            &issued.certificate_bytes,
            &issued.net_bytes,
            &negative_annotation,
        ),
        Err(resource::Refusal::StaleRuleTable)
    );

    // There is no signed potential field in the fixed binary certificate schema. A forged
    // negative potential changes the cost-model identity and cannot pass its fixed profile.
    let mut negative_potential = resource::COST_MODEL_BYTES.to_vec();
    let at = negative_potential
        .windows(b"dup=1".len())
        .position(|window| window == b"dup=1")
        .ok_or("missing DUP potential")?
        + b"dup=".len();
    negative_potential.splice(at..at + 1, b"-1".iter().copied());
    let mut stale_cost = issued.certificate_bytes.clone();
    let digest_offset_bytes = certificate_digest_offset();
    stale_cost[digest_offset_bytes + 64..digest_offset_bytes + 96].copy_from_slice(
        &certificate_digest(b"noble.ngraph.resource.cost.v0", &negative_potential)?,
    );
    assert_eq!(
        check(&stale_cost, &issued.net_bytes, resource::RULE_TABLE_BYTES),
        Err(resource::Refusal::WrongCostModel)
    );

    // Recompute the digest after breaking a live port's reciprocal wire while retaining
    // well-formed field encodings and the original graph/build identity. A digest match
    // cannot establish the fully connected structural premise.
    let mut disconnected = lowered.clone();
    disconnected.agents[0].ports[0] = Some(net::Endpoint::Output(0));
    let false_premise = resource::encode_net(&disconnected)
        .map_err(|error| format!("encoding forged structural premise: {error:?}"))?;
    let mut rebound = issued.certificate_bytes.clone();
    rebound[digest_offset_bytes..digest_offset_bytes + 32].copy_from_slice(&certificate_digest(
        b"noble.ngraph.resource.net.v0",
        &false_premise,
    )?);
    assert_eq!(
        check(&rebound, &false_premise, resource::RULE_TABLE_BYTES),
        Err(resource::Refusal::InvalidNet)
    );
    retain_followup_evidence(&checked, &issued)?;
    Ok(())
}

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
