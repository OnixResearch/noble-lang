//! Executed recurrence and refusal witnesses for the sequential I64 runner.

#[path = "protocol/bounds.rs"]
mod bounds;
#[path = "protocol/refusals.rs"]
mod refusals;
#[path = "protocol/trace.rs"]
mod trace;

use crate::common::IDS;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::Graph;
use noble_ngraph::sem::Value;

fn compare_engines(graph: Graph, inputs: &[i64], expected: &[i64]) {
    let checked = noble_ngraph::check::check(&IDS, graph).expect("checked graph");
    let compiled = Compiled::new(&IDS, &checked);
    let layout = compiled.packed_layout();
    noble_ngraph::schedule::check(
        &checked,
        &layout,
        compiled.i64_instructions().expect("I64 instructions"),
    )
    .expect("actual packed body");
    let values: Vec<_> = inputs.iter().copied().map(Value::I64).collect();
    let want = Obs::Normal(expected.iter().copied().map(Value::I64).collect());
    let reference = eval::reference(&checked, &values, None, &mut Control::default());
    let packed = compiled.evaluate_in(
        &mut compiled.workspace(),
        &values,
        None,
        &mut Control::default(),
    );
    assert_eq!(
        reference.obs, want,
        "independent recurrence versus reference"
    );
    assert_eq!(packed.obs, want, "independent recurrence versus packed");
    assert!(reference.obs.obs_eq(&packed.obs));
}
