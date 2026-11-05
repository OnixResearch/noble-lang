#![allow(
    tigerstyle::ambient_clock,
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::numeric_units,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this reporting-only measurement builds graphs at the checker's default budgets and times their validation with the wall clock, so checked unwraps and owner-path imports are the readable form. Nothing here asserts a timing value."
)]
//! Validation cost of the actual packed stream at the graph checker's default budgets (issue #43:
//! "measure validation time and retained/scratch bytes"), reporting only.
//!
//! Each shape is built, checked and compiled once, then `layout::check` and `schedule::check`
//! (which runs `layout::check` itself) are timed over five runs and the median is reported beside
//! the checker and compiler times of the same graph. The shapes reach the default budgets: one
//! chain as deep as the layer budget allows, sixteen such chains (the node budget), and a balanced
//! reduction tree of the same size with only eighteen layers.
//!
//! What this lane cannot measure: **allocation counts**. The workspace forbids `unsafe_code`, so no
//! counting global allocator can exist here. It reports resident-set growth across the first
//! validation instead, an upper-bound instrument that also counts allocator retention.
//!
//! Run it with `cargo test --release -p noble-ngraph --test schedule_correspondence -- --ignored
//! --nocapture validation_cost_at_the_graph_budget`.

use std::time::Instant;

use noble_ngraph::check;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::ir::{Graph, GraphBuilder, NodeId, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::layout;
use noble_ngraph::schedule;

const RUNS: usize = 5;

/// `chains` independent chains of `length` additions over one input, each chain an output.
fn chains(chains: usize, length: usize) -> Graph {
    let scheme = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "validation-cost"),
    );
    let input = builder.input(ScalarType::I64);
    let mut ends = Vec::with_capacity(chains);
    for _ in 0..chains {
        let mut last = input;
        for _ in 0..length {
            last = builder.op(Op::I64Add(last, last));
        }
        ends.push(last);
    }
    for end in ends {
        builder.output(end);
    }
    builder.finish()
}

/// A balanced reduction tree over `leaves` additions of one input: about `2 * leaves` nodes in
/// `log2(leaves) + 1` layers.
fn tree(leaves: usize) -> Graph {
    let scheme = super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&scheme, "validation-cost"),
    );
    let input = builder.input(ScalarType::I64);
    let mut level: Vec<NodeId> = (0..leaves)
        .map(|_| builder.op(Op::I64Add(input, input)))
        .collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            next.push(match pair {
                [left, right] => builder.op(Op::I64Add(*left, *right)),
                [only] => *only,
                _ => unreachable!("chunks(2) yields one or two nodes"),
            });
        }
        level = next;
    }
    builder.output(level[0]);
    builder.finish()
}

/// Resident set size in bytes, from the kernel's own report.
fn resident_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages = text.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(pages * 4096)
}

fn median(mut samples: Vec<u128>) -> u128 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn report_shape(shape: &str, graph: Graph) {
    let scheme = super::common::IDS;
    let started = Instant::now();
    let accepted = check::check(&scheme, graph).expect("the shape is inside the default budgets");
    let check_ns = started.elapsed().as_nanos();
    let started = Instant::now();
    let compiled = Compiled::new(&scheme, &accepted);
    let compile_ns = started.elapsed().as_nanos();
    let declared = compiled.packed_layout();
    let stream = compiled.i64_instructions().expect("an I64 stream").to_vec();
    let before = resident_bytes();
    assert_eq!(
        schedule::check(&accepted, &declared, &stream),
        Ok(()),
        "{shape}"
    );
    let grown = match (before, resident_bytes()) {
        (Some(before), Some(after)) => after.saturating_sub(before),
        _ => 0,
    };
    let mut layout_ns = Vec::with_capacity(RUNS);
    let mut schedule_ns = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let started = Instant::now();
        assert_eq!(layout::check(&accepted, &declared), Ok(()), "{shape}");
        layout_ns.push(started.elapsed().as_nanos());
        let started = Instant::now();
        assert_eq!(
            schedule::check(&accepted, &declared, &stream),
            Ok(()),
            "{shape}"
        );
        schedule_ns.push(started.elapsed().as_nanos());
    }
    println!(
        "MEASURE {{\"shape\":\"{shape}\",\"nodes\":{},\"layers\":{},\"instructions\":{},\"check_ns\":{check_ns},\"compile_ns\":{compile_ns},\"layout_check_median_ns\":{},\"schedule_check_median_ns\":{},\"runs\":{RUNS},\"first_validation_resident_growth_bytes\":{grown}}}",
        accepted.graph().nodes.len(),
        declared.layers.len(),
        stream.len(),
        median(layout_ns),
        median(schedule_ns),
    );
}

#[test]
#[ignore = "release-mode measurement; run: cargo test --release -p noble-ngraph --test schedule_correspondence -- --ignored --nocapture validation_cost_at_the_graph_budget"]
fn validation_cost_at_the_graph_budget() {
    report_shape("deepest-chain", chains(1, 16_383));
    report_shape("four-deepest-chains", chains(4, 16_383));
    report_shape("sixteen-deepest-chains", chains(16, 16_383));
    report_shape("balanced-tree", tree(131_072));
}
