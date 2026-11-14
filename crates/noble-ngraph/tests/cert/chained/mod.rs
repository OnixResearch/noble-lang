use crate::differential::{self, TestIds};
use noble_ngraph::cert::{self, ChainedAction, ChainedDerivation, ChainedStep, Proposer, Refusal};
use noble_ngraph::ir::{Graph, GraphBuilder, NodeId, Op, SubProfile, Subject};

fn chain_graph() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&TestIds, "cert-chained-folds"),
    );
    let five = builder.i64_const(5);
    let seven = builder.i64_const(7);
    let first = builder.op(Op::I64Add(five, seven));
    let three = builder.i64_const(3);
    let second = builder.op(Op::I64Add(first, three));
    builder.output(second);
    builder.output(first);
    builder.finish()
}

fn ordered_graph() -> Graph {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&TestIds, "cert-chained-ordered-outputs"),
    );
    let two = builder.i64_const(2);
    let three = builder.i64_const(3);
    let first = builder.op(Op::I64Add(two, three));
    let ten = builder.i64_const(10);
    let second = builder.op(Op::I64Add(two, ten));
    builder.output(second);
    builder.output(first);
    builder.finish()
}

#[path = "corpus.rs"]
mod corpus;

const COMMITTED: &str = include_str!(
    "../../../../../proofs/foundation/NobleFoundation/CertificateChainedCorpus.lean"
);

#[test]
fn the_chained_lean_corpus_is_fresh_from_real_rust_certificates() {
    assert_eq!(
        corpus::lean_file(),
        COMMITTED,
        "a source graph or emitted certificate changed; regenerate the chained corpus and rerun the Lean audit"
    );
}

#[test]
#[ignore = "writes CertificateChainedCorpus.lean from emitted Rust certificate wire bytes"]
fn write_certificate_chained_lean() {
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proofs/foundation/NobleFoundation/CertificateChainedCorpus.lean"
        ),
        corpus::lean_file(),
    )
    .expect("generated chained certificate corpus written");
}

fn certified() -> (Graph, ChainedDerivation, Graph) {
    let graph = chain_graph();
    let (derivation, result) =
        cert::propose_chained(&TestIds, Proposer::FixedPoint, &graph).expect("bounded proposal");
    assert_eq!(
        cert::check_chained(&TestIds, &graph, &result, &derivation),
        Ok(())
    );
    (graph, derivation, result)
}

#[test]
fn chained_codec_refuses_stale_overbound_truncated_and_trailing_envelopes() {
    assert_eq!(
        cert::decode_chained("ngraph-cert/1 ngraph-sem/0 0"),
        Err(Refusal::StaleRevision)
    );
    assert_eq!(
        cert::decode_chained("ngraph-cert/2 ngraph-sem/1 0"),
        Err(Refusal::StaleModel)
    );
    assert_eq!(
        cert::decode_chained("ngraph-cert/2 ngraph-sem/0 65"),
        Err(Refusal::TooManySteps),
        "the declared count is refused before digest or step allocation"
    );

    let (_, derivation, _) = certified();
    let wire = cert::encode_chained(&derivation);
    let mut truncated = wire.clone();
    truncated.pop();
    assert_eq!(cert::decode_chained(&truncated), Err(Refusal::Malformed));
    assert_eq!(
        cert::decode_chained(&format!("{wire} 0")),
        Err(Refusal::Malformed),
        "trailing tokens are not ignored"
    );
}

#[test]
fn chained_checker_binds_each_transition_and_requires_a_final_single_prune() {
    let (graph, derivation, result) = certified();

    let mut wrong_before = derivation.clone();
    wrong_before.steps[1].before[0] ^= 1;
    assert_eq!(
        cert::check_chained(&TestIds, &graph, &result, &wrong_before),
        Err(Refusal::EndpointMismatch)
    );

    let mut wrong_after = derivation.clone();
    wrong_after.steps[0].after[0] ^= 1;
    assert_eq!(
        cert::check_chained(&TestIds, &graph, &result, &wrong_after),
        Err(Refusal::ResultMismatch)
    );

    let intermediate = derivation.steps[1].before;
    let mut premature_prune = derivation;
    premature_prune.steps.insert(
        1,
        ChainedStep {
            action: ChainedAction::PruneUnreachable,
            before: intermediate,
            after: intermediate,
        },
    );
    assert_eq!(
        cert::check_chained(&TestIds, &graph, &result, &premature_prune),
        Err(Refusal::Order),
        "pruning is a single, final transition"
    );
}

#[test]
fn chained_fixed_point_and_pruning_round_trip_and_execute() {
    let (graph, derivation, result) = certified();
    assert_eq!(derivation.revision, cert::CHAINED_REVISION);
    assert_eq!(derivation.steps.len(), 3, "two folds and a final prune");
    assert!(matches!(derivation.steps[0].action, ChainedAction::Rewrite(_)));
    assert!(matches!(derivation.steps[1].action, ChainedAction::Rewrite(_)));
    assert!(matches!(
        derivation.steps[2].action,
        ChainedAction::PruneUnreachable
    ));
    assert_eq!(result.nodes.len(), 2, "only the two output values survive");
    assert_eq!(result.outputs, [NodeId(1), NodeId(0)]);

    let wire = cert::encode_chained(&derivation);
    assert_eq!(cert::decode_chained(&wire), Ok(derivation.clone()));
    assert_eq!(
        cert::decode(&wire),
        Err(Refusal::StaleRevision),
        "the v1 reader does not reinterpret the v2 transition grammar"
    );

    let before = noble_ngraph::check(&TestIds, graph.clone()).expect("source admission");
    let after = noble_ngraph::check(&TestIds, result.clone()).expect("pruned candidate admission");
    let before_reference = noble_ngraph::reference(
        &before,
        &[],
        None,
        &mut noble_ngraph::Control::default(),
    );
    let after_reference = noble_ngraph::reference(
        &after,
        &[],
        None,
        &mut noble_ngraph::Control::default(),
    );
    assert_eq!(before_reference.obs, after_reference.obs);
    assert_eq!(
        after_reference.obs,
        noble_ngraph::Obs::Normal(vec![
            noble_ngraph::Value::I64(15),
            noble_ngraph::Value::I64(12),
        ])
    );
    let compiled = noble_ngraph::compiled::Compiled::new(&TestIds, &after);
    assert_eq!(
        compiled.evaluate(&[], None, &mut noble_ngraph::Control::default()).obs,
        after_reference.obs,
        "compiled ordered outputs equal the reference vector"
    );

    let legacy = cert::Derivation {
        revision: String::from(cert::LEGACY_CHAINLESS_REVISION),
        model: String::from(cert::MODEL_REVISION),
        steps: derivation
            .steps
            .iter()
            .filter_map(|transition| match &transition.action {
                ChainedAction::Rewrite(step) => Some(*step),
                ChainedAction::PruneUnreachable => None,
            })
            .collect(),
        left: derivation.left,
        right: derivation.right,
    };
    assert_eq!(
        cert::check(&TestIds, &graph, &result, &legacy),
        Err(Refusal::Malformed),
        "the predecessor binds every premise to the original graph"
    );
}

#[test]
fn certified_pruning_refuses_live_node_removal_wrong_renumbering_and_output_reordering() {
    let (graph, derivation, result) = certified();

    let mut removed_live = result.clone();
    removed_live.nodes.remove(0);
    removed_live.outputs = vec![NodeId(0), NodeId(0)];
    let mut removed_certificate = derivation.clone();
    removed_certificate.right = removed_live.logical_id(&TestIds);
    assert_eq!(
        cert::check_chained(&TestIds, &graph, &removed_live, &removed_certificate),
        Err(Refusal::ResultMismatch),
        "both survivors are output-reachable"
    );

    let mut wrong_renumbering = result.clone();
    wrong_renumbering.nodes.swap(0, 1);
    wrong_renumbering.outputs = vec![NodeId(0), NodeId(1)];
    let mut renumbered_certificate = derivation.clone();
    renumbered_certificate.right = wrong_renumbering.logical_id(&TestIds);
    assert_eq!(
        cert::check_chained(&TestIds, &graph, &wrong_renumbering, &renumbered_certificate),
        Err(Refusal::ResultMismatch),
        "survivor renumbering is the re-derived creation-order bijection"
    );

    let mut reordered_outputs = result.clone();
    reordered_outputs.outputs.reverse();
    let mut reordered_certificate = derivation;
    reordered_certificate.right = reordered_outputs.logical_id(&TestIds);
    assert_eq!(
        cert::check_chained(&TestIds, &graph, &reordered_outputs, &reordered_certificate),
        Err(Refusal::ResultMismatch),
        "ordered outputs are preserved, not treated as a set"
    );
}

#[test]
fn a_prune_without_a_rewrite_is_identity_only_for_already_admitted_graphs() {
    let graph = differential::symbolic_sum();
    let (derivation, result) = cert::propose_chained(&TestIds, Proposer::Identity, &graph)
        .expect("the input graph needs no normalization");
    assert!(derivation.steps.is_empty());
    assert_eq!(result, graph);
    assert_eq!(cert::check_chained(&TestIds, &graph, &result, &derivation), Ok(()));
    assert!(matches!(result.nodes[0].op, Op::Input(_)));
    assert!(matches!(result.nodes[1].op, Op::Input(_)));
    assert_eq!(result.nodes[2].op, Op::I64Add(NodeId(0), NodeId(1)));
}
