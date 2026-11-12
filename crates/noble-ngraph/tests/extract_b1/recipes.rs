//! The static expansion bound's upper-bound property.

use noble_kernel::types::Ty;
use noble_ngraph::check::Budgets;
use noble_ngraph::extract::{self, source, Budget, Request};

use super::common::IDS;

/// The frontend's root submission and the caller's claimed closure.
fn prepare(root: &str, inputs: &[Ty]) -> source::Prepared {
    source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        &[],
        root,
        inputs,
    )
    .unwrap_or_else(|e| panic!("frontend accepts: {e}"))
}

fn admit(p: &source::Prepared) -> extract::Admitted<'_> {
    extract::admit(
        &IDS,
        &p.submission,
        &Request {
            profile: "ngraph-i64/0",
            claimed_closure: &p.claimed_closure,
            budget: Budget::default(),
        },
    )
    .expect("admitted")
}

#[test]
fn the_expansion_bound_counts_scalar_leaves_of_the_root_inputs() {
    let pair = Ty::Pair(Box::new(Ty::I64), Box::new(Ty::I64));
    let p = prepare("unpair +", std::slice::from_ref(&pair));
    let admitted = admit(&p);
    let x = extract::extract(&IDS, &admitted, &Budgets::default()).expect("extracts");
    // Two input nodes for the pair's scalar leaves, then one `+` node.
    assert_eq!(x.stats.expanded_nodes, 3);
    assert_eq!(x.stats.static_expansion_bound, 3);
    assert!(
        x.stats.expanded_nodes <= x.stats.static_expansion_bound,
        "the bound is an upper bound on expanded nodes"
    );
}
