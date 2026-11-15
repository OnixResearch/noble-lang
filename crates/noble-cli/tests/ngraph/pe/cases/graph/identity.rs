use super::*;

fn original() -> Result<noble_ngraph::check::CheckedGraph, Box<dyn std::error::Error>> {
    Ok(noble_ngraph::check::check(
        &TestIds,
        noble_ngraph::format::graph::parse(&TestIds, PE_GRAPH)?,
    )?)
}

fn specialize_map(
    graph: &noble_ngraph::check::CheckedGraph,
    map: &[Option<i64>],
) -> Result<noble_ngraph::specialize::Residual, Box<dyn std::error::Error>> {
    Ok(noble_ngraph::specialize::specialize(
        &TestIds,
        graph,
        &noble_ngraph::specialize::Request {
            static_inputs: map,
            budgets: &noble_ngraph::check::Budgets::default(),
        },
    )?)
}

fn run(name: &str) -> Value {
    serde_json::json!({
        "kind":"library", "name":name, "exit":null, "stage":"residual-derivation",
        "outcome":"normal", "code":null, "guest_requests":0,
        "protected_operations":0, "published_output":false,
    })
}

pub(crate) fn case_08() -> TestResult {
    let graph = original()?;
    let residual = specialize_map(&graph, &[Some(5), None])?;
    assert_ne!(
        graph.graph().subject.identity,
        residual.graph().subject.identity
    );
    assert_ne!(graph.id(), residual.checked.id());
    assert_eq!(
        graph.graph().subject.closure,
        residual.graph().subject.closure
    );
    assert_eq!(residual.report.original_graph, graph.id());
    assert_eq!(residual.report.residual_graph, residual.checked.id());
    case_observation(
        8,
        vec![run("subject-and-graph-identities")],
        serde_json::json!({
            "fresh_subject":true,
            "fresh_graph":true,
            "residual_closure_matches_original":true,
            "record_binds_both_identities":true,
        }),
    )
}

fn derive_subject(
    original_graph: noble_ngraph::ir::Digest,
    closure: &[u64],
    encoded_map: &[u8],
    revision: &[u8],
) -> noble_ngraph::ir::Subject {
    noble_ngraph::ir::Subject::derive(
        &TestIds,
        noble_ngraph::ir::SubjectKind::Extracted,
        &[
            b"partial-evaluation",
            revision,
            &original_graph,
            encoded_map,
        ],
        closure.to_vec(),
    )
}

pub(crate) fn case_13() -> TestResult {
    let scratch = Scratch::new("pe-case13")?;
    let graph_path = scratch.write("original.ngraph", PE_GRAPH)?;
    let source = scratch.path("residual.noble");
    let graph_out = scratch.path("residual.ngraph");
    let command = specialize(&graph_path, "0=5", &source)
        .arg("--graph-out")
        .arg(&graph_out);
    let reported = expect_normal(&scratch.run(&command)?, "specialize")?;
    let original = original()?;
    let residual = specialize_map(&original, &[Some(5), None])?;
    assert_eq!(
        reported["graph"],
        blake3::Hash::from(original.id()).to_hex().to_string()
    );
    assert_eq!(
        reported["residual_graph"],
        blake3::Hash::from(residual.checked.id())
            .to_hex()
            .to_string()
    );
    let emitted_source = std::fs::read(&source)?;
    assert_eq!(
        reported["source_blake3"],
        blake3::hash(&emitted_source).to_hex().to_string()
    );
    let independently_checked = noble_ngraph::check::check(
        &TestIds,
        noble_ngraph::format::graph::parse(&TestIds, &std::fs::read_to_string(&graph_out)?)?,
    )?;
    assert_eq!(independently_checked.id(), residual.checked.id());
    let revision = noble_ngraph::specialize::ACCOUNTING_REVISION.as_bytes();
    let encoded = [0x01, 0x01, 5, 0, 0, 0, 0, 0, 0, 0, 0x00];
    let expected = derive_subject(
        original.id(),
        &original.graph().subject.closure,
        &encoded,
        revision,
    );
    assert_eq!(expected.identity, residual.graph().subject.identity);
    let mut changed_map = encoded;
    changed_map[2] = 6;
    let mut changed_original = original.id();
    changed_original[0] ^= 0x01;
    let rows = [
        (
            "static-map",
            derive_subject(
                original.id(),
                &original.graph().subject.closure,
                &changed_map,
                revision,
            ),
        ),
        (
            "accounting-revision",
            derive_subject(
                original.id(),
                &original.graph().subject.closure,
                &encoded,
                b"ngraph-pe/other",
            ),
        ),
        (
            "original-identity",
            derive_subject(
                changed_original,
                &original.graph().subject.closure,
                &encoded,
                revision,
            ),
        ),
    ];
    let mut runs = Vec::new();
    for (name, mutated) in rows {
        assert_ne!(
            mutated.identity,
            residual.graph().subject.identity,
            "{name}: identity collision"
        );
        runs.push(run(name));
    }
    case_observation(
        13,
        runs,
        serde_json::json!({
            "all_recorded_identities_match":true,
            "each_mutated_record_reproduces_residual_identity":false,
        }),
    )
}

pub(crate) fn case_14() -> TestResult {
    let graph = original()?;
    let changed = PE_GRAPH.replace("%3 = i64.add %2 %0", "%3 = i64.sub %2 %0");
    let changed = noble_ngraph::check::check(
        &TestIds,
        noble_ngraph::format::graph::parse(&TestIds, &changed)?,
    )?;
    let first = specialize_map(&graph, &[Some(5), None])?;
    let changed_map = specialize_map(&graph, &[None, Some(5)])?;
    assert_eq!(
        first.report.original_graph,
        changed_map.report.original_graph
    );
    assert_ne!(first.report.static_inputs, changed_map.report.static_inputs);
    assert_ne!(first.checked.id(), changed_map.checked.id());
    let changed_graph = specialize_map(&changed, &[Some(5), None])?;
    assert_ne!(
        first.report.original_graph,
        changed_graph.report.original_graph
    );
    assert_eq!(
        first.report.static_inputs,
        changed_graph.report.static_inputs
    );
    assert_ne!(first.checked.id(), changed_graph.checked.id());
    case_observation(
        14,
        vec![run("only-static-map"), run("only-original-graph")],
        serde_json::json!({
            "residual_identities_distinct":true,
            "binding_fields_distinct":true,
        }),
    )
}
