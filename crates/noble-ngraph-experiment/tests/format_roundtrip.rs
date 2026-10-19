#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_panic,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Graph files preserve identity (design D4, NGRAPH-FORMAT-01,
//! NGRAPH-CASE-06): for every fixture, workload, seeded random graph and
//! extracted `I1`–`I4`, `parse(print(g))` equals `g` with producer origins
//! cleared and has the same `GraphLogicalId`, fact-binding digest and
//! checker outcome; comments, blank lines and runs of spaces change none of
//! that.

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

/// `text` with a comment and a blank line before the schema line, a blank
/// line and a trailing comment after every record, and runs of spaces
/// around every token.
fn noisy(text: &str) -> String {
    let mut out = String::from("# a comment before the schema line\n\n");
    for line in text.lines() {
        let spaced = line.split(' ').collect::<Vec<_>>().join("   ");
        out.push_str("  ");
        out.push_str(&spaced);
        out.push_str("   # a trailing comment\n\n");
    }
    out
}

#[test]
fn comments_blank_lines_and_spacing_do_not_change_any_corpus_graph() {
    for (name, g) in suites::identity_corpus() {
        let text = print(&g);
        let canonical = parse(&BLAKE3, &text).unwrap_or_else(|d| panic!("{name}: {d}"));
        let noisy_text = noisy(&text);
        let back = parse(&BLAKE3, &noisy_text).unwrap_or_else(|d| panic!("{name}: {d}"));
        assert_eq!(back, canonical, "{name}");
        assert_eq!(print(&back), text, "{name}: printing drops the noise");
        assert_eq!(
            check(&BLAKE3, back)
                .map(|c| (c.id(), c.fact_bindings(&BLAKE3)))
                .map_err(|d| d.code_string()),
            check(&BLAKE3, canonical)
                .map(|c| (c.id(), c.fact_bindings(&BLAKE3)))
                .map_err(|d| d.code_string()),
            "{name}: identity and checker outcome"
        );
    }
}
