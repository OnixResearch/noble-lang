//! Graph files preserve identity (design D4, NGRAPH-FORMAT-01,
//! NGRAPH-CASE-06): for every fixture, workload, seeded random graph and
//! extracted `I1`–`I4`, `parse(print(g))` equals `g` with producer origins
//! cleared and has the same `GraphLogicalId`, fact-binding digest and
//! checker outcome.

use noble_ngraph::check;
use noble_ngraph::format::graph::{parse, print};
use noble_ngraph_experiment::scheme::BLAKE3;
use noble_ngraph_experiment::suites;

#[test]
fn every_corpus_graph_round_trips_with_its_identity() {
    for (name, g) in suites::identity_corpus() {
        let text = print(&g);
        let back = parse(&BLAKE3, &text).unwrap_or_else(|d| panic!("{name}: {d}"));
        let mut plain = g.clone();
        for n in &mut plain.nodes {
            n.origin = None;
        }
        assert_eq!(back, plain, "{name}");
        assert_eq!(print(&back), text, "{name}: printing is canonical");
        let (a, b) = (check(&BLAKE3, g), check(&BLAKE3, back));
        match (a, b) {
            (Ok(a), Ok(b)) => {
                assert_eq!(a.id(), b.id(), "{name}");
                assert_eq!(a.fact_bindings(&BLAKE3), b.fact_bindings(&BLAKE3), "{name}");
                assert_eq!(a.layer_count(), b.layer_count(), "{name}");
            }
            (a, b) => assert_eq!(
                a.map(|c| c.id()).map_err(|d| d.code_string()),
                b.map(|c| c.id()).map_err(|d| d.code_string()),
                "{name}: checker outcome"
            ),
        }
    }
}
