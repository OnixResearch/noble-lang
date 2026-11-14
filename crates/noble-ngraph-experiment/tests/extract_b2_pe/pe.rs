//! Static-input partial-evaluation cases attached to the existing B2 target.

use super::*;
use noble_ngraph_experiment::b2i64::pe;

struct VectorSpec {
    seed: u64,
    dynamic_inputs: usize,
    count: usize,
}

fn vectors(config: VectorSpec) -> Vec<Vec<i64>> {
    let mut rng = SplitMix64::new(config.seed);
    let edges = [0i64, 1, -1, i64::MAX, i64::MIN, 2, -3, 7];
    const EDGE_STRIDE: usize = 8;
    let edge_count = edges.len();
    assert_ne!(edge_count, 0, "the edge vector is nonempty");
    let mut edge_index = 0usize;
    let mut rows = Vec::with_capacity(config.count);
    for i in 0..config.count {
        let mut row = Vec::with_capacity(config.dynamic_inputs);
        for _ in 0..config.dynamic_inputs {
            row.push(if i % EDGE_STRIDE == 0 {
                *edges.get(edge_index).expect("edge index is in range")
            } else {
                random::value(&mut rng)
            });
        }
        rows.push(row);
        if i % EDGE_STRIDE == 0 {
            edge_index = edge_index.checked_add(1).expect("edge index fits usize");
            if edge_index == edge_count {
                edge_index = 0;
            }
        }
    }
    rows
}

fn fixture_seed(inputs: usize) -> u64 {
    0x3200u64
        .checked_add(u64::try_from(inputs).expect("fixture input count fits u64"))
        .expect("fixture seed addition fits u64")
}

fn cells(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .cloned()
        .map(noble_ngraph_experiment::b2i64::cell)
        .collect()
}

#[path = "differential.rs"]
mod differential;
#[path = "measurement.rs"]
mod measurement;
#[path = "oracle.rs"]
mod oracle;
