#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::clone_then_borrow,
    tigerstyle::explicit_defaults,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::missing_const_fn,
    tigerstyle::numeric_units,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form. The reserved capacity is an abstract element count, not a byte or time quantity."
)]
//! Retained-byte accounting (`graph_bytes`).
//!
//! A graph handed to `check_with` is retained by the host, so the estimate
//! that decides admission must cover every graph-owned heap allocation.
//! These tests pin the observable contract: growing retained metadata raises
//! the charge, a budget covering only a smaller charge refuses the graph with
//! `E-BUDGET-GRAPH-BYTES`, and graphs are still admitted once the budget
//! covers the charge. They do not measure process memory.

mod common;

use common::IDS;
use noble_ngraph::check::{check_with, retained_bytes, Budgets};
use noble_ngraph::diag::{BudgetKind, Code};
use noble_ngraph::ir::{
    AffineOperand, AffineTerm, Fact, FactRecord, Graph, GraphBuilder, NodeId, Provenance,
    ShapeTree, Subject,
};
use noble_ngraph::{Op, ScalarType, SubProfile};

fn small_graph() -> Graph {
    let mut b = GraphBuilder::new(SubProfile::I64, Subject::authored(&IDS, "retained_bytes"));
    let x = b.input(ScalarType::I64);
    b.output(x);
    b.finish()
}

fn budgets_for(graph_bytes: u64) -> Budgets {
    Budgets {
        graph_bytes,
        ..Budgets::default()
    }
}

/// Refuses `graph` under `graph_bytes` and returns the observed charge.
fn refusal_charge(graph: Graph, graph_bytes: u64) -> u64 {
    let d = check_with(&IDS, graph, &budgets_for(graph_bytes))
        .expect_err("the budget must refuse the graph");
    assert_eq!(d.code, Code::Budget(BudgetKind::GraphBytes), "{d}");
    let (limit, observed) = d.budget.expect("budget refusals carry limit and observed");
    assert_eq!(limit, graph_bytes);
    observed
}

#[test]
fn ordinary_small_graph_is_admitted_under_its_own_charge() {
    let g = small_graph();
    let charge = retained_bytes(&g);
    assert!(charge > 0);
    assert!(check_with(&IDS, g.clone(), &budgets_for(charge)).is_ok());
    assert!(check_with(&IDS, g, &Budgets::default()).is_ok());
}

#[test]
fn closure_storage_is_charged_and_budgeted() {
    let mut g = small_graph();
    let empty = retained_bytes(&g.clone());
    assert!(check_with(&IDS, g.clone(), &budgets_for(empty)).is_ok());

    g.subject.closure = (0..8_192u64).collect();
    let payload = 8_192 * core::mem::size_of::<u64>() as u64;
    let charged = retained_bytes(&g);
    assert!(
        charged >= empty + payload,
        "charge {charged} must cover the {payload}-byte closure above {empty}"
    );
    // `Vec::clone` normalises spare capacity, so the refusal's observed charge
    // is the clone's estimate; it must still cover the closure payload.
    let observed = refusal_charge(g.clone(), empty);
    assert!(
        observed >= empty + payload,
        "observed {observed} empty {empty} payload {payload}"
    );

    assert!(check_with(&IDS, g, &budgets_for(charged)).is_ok());
}

#[test]
fn charge_is_monotone_in_closure_length() {
    let mut g = small_graph();
    let mut previous = retained_bytes(&g);
    for entries in [1usize, 2, 64, 4_096] {
        g.subject.closure = (0..entries as u64).collect();
        let charge = retained_bytes(&g);
        assert!(
            charge >= previous,
            "charge must not fall when the closure grows to {entries}: {charge} < {previous}"
        );
        assert!(
            charge >= (entries as u64) * core::mem::size_of::<u64>() as u64,
            "the closure payload of {entries} entries must be covered"
        );
        previous = charge;
    }
}

#[test]
fn shape_storage_is_charged_and_budgeted() {
    let mut g = small_graph();
    let flat = retained_bytes(&g.clone());
    assert!(check_with(&IDS, g.clone(), &budgets_for(flat)).is_ok());

    // A shallow wide shape: 64 scalars nested right, one pair per level.
    let pairs = 63u64;
    let mut tree = ShapeTree::SCALAR;
    for _ in 0..pairs {
        tree = ShapeTree::pair(tree, ShapeTree::SCALAR).expect("pairs stay inside MAX_SHAPE_DEPTH");
    }
    g.input_shape = Some(vec![tree]);
    g.output_shape = Some(vec![ShapeTree::SCALAR]);

    let pair_bytes = pairs * 2 * core::mem::size_of::<ShapeTree>() as u64;
    let charged = retained_bytes(&g);
    assert!(
        charged >= flat + pair_bytes,
        "charge {charged} must cover the {pair_bytes} bytes of boxed shape pairs above {flat}"
    );
    assert!(refusal_charge(g.clone(), flat) >= flat + pair_bytes);

    assert!(check_with(&IDS, g, &budgets_for(charged)).is_ok());
}

#[test]
fn fact_payloads_are_charged() {
    let mut g = small_graph();
    // `Vec::clone` normalizes spare capacity, so compare against a clone: the
    // baseline must have the same node allocation shape as `with_rule`.
    let bare = retained_bytes(&g.clone());

    g.facts = vec![FactRecord {
        node: NodeId(0),
        fact: Fact::OneHot {
            group: (0u32..512).map(NodeId).collect(),
            allow_zero: false,
        },
        provenance: Provenance::Evidence(vec![0u8; 256]),
    }];
    let charged = retained_bytes(&g);
    let group_bytes = 512 * core::mem::size_of::<NodeId>() as u64;
    let evidence_bytes = 256u64;
    assert!(
        charged >= bare + group_bytes + evidence_bytes,
        "charge {charged} must cover the one-hot group and evidence payloads above {bare}"
    );

    // A derived rule string is charged too.
    let with_rule = {
        let mut copy = g.clone();
        copy.facts[0].provenance = Provenance::Derived("ir_check.transfer.rule".to_string());
        copy
    };
    assert!(retained_bytes(&with_rule) >= bare + group_bytes);
}

#[test]
fn node_operation_payloads_are_charged() {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&IDS, "retained_bytes_ops"),
    );
    let x = b.input(ScalarType::F32);
    let terms: Vec<(f32, AffineOperand)> =
        (0..256).map(|_| (1.0f32, AffineOperand::Node(x))).collect();
    let y = b.affine(0.0, &terms);
    b.output(y);
    let graph = b.finish();

    let term_bytes = 256 * core::mem::size_of::<AffineTerm>() as u64;
    assert!(
        retained_bytes(&graph) >= term_bytes,
        "the affine term payload must be covered"
    );
    let small = {
        let mut copy = graph.clone();
        if let Some(node) = copy
            .nodes
            .iter_mut()
            .find(|n| matches!(n.op, Op::F32Affine { .. }))
        {
            if let Op::F32Affine { terms, .. } = &mut node.op {
                terms.truncate(1);
                terms.shrink_to_fit();
            }
        }
        copy
    };
    let dropped = term_bytes - core::mem::size_of::<AffineTerm>() as u64;
    assert!(
        retained_bytes(&graph) >= retained_bytes(&small) + dropped,
        "shrinking the term list must lower the charge by at least the dropped payload"
    );
}

#[test]
fn node_spare_capacity_is_charged_and_refused() {
    let mut g = small_graph();
    let empty = retained_bytes(&g);
    let previous_capacity = g.nodes.capacity();
    g.nodes.reserve_exact(previous_capacity + 4_096);
    let retained_capacity = g.nodes.capacity();
    assert!(retained_capacity > previous_capacity);
    assert_eq!(g.nodes.len(), 1);

    let node_bytes = core::mem::size_of::<noble_ngraph::ir::Node>() as u64;
    let growth = (retained_capacity - previous_capacity) as u64 * node_bytes;
    assert_eq!(
        retained_bytes(&g),
        empty.saturating_add(growth),
        "node capacity growth must add exactly its element storage"
    );

    // Consume the reserved graph directly: a clone need not preserve capacity.
    let observed = refusal_charge(g, empty);
    assert!(
        observed >= empty.saturating_add(growth),
        "observed {observed} must cover {growth} spare node bytes above {empty}"
    );

    let mut admitted = small_graph();
    admitted.nodes.reserve_exact(previous_capacity + 4_096);
    let corrected = retained_bytes(&admitted);
    let checked = check_with(&IDS, admitted, &budgets_for(corrected))
        .expect("the corrected budget covers the spare node capacity");
    assert_eq!(checked.graph().nodes.capacity(), retained_capacity);
}

#[test]
fn fact_spare_capacity_is_charged_and_refused() {
    let mut g = small_graph();
    let empty = retained_bytes(&g);
    let previous_capacity = g.facts.capacity();
    g.facts.reserve_exact(previous_capacity + 4_096);
    let retained_capacity = g.facts.capacity();
    assert!(retained_capacity > previous_capacity);
    assert!(g.facts.is_empty());

    let fact_bytes = core::mem::size_of::<noble_ngraph::ir::FactRecord>() as u64;
    let growth = (retained_capacity - previous_capacity) as u64 * fact_bytes;
    assert_eq!(
        retained_bytes(&g),
        empty.saturating_add(growth),
        "empty fact capacity must charge its backing storage"
    );

    let observed = refusal_charge(g, empty);
    assert!(
        observed >= empty.saturating_add(growth),
        "observed {observed} must cover {growth} spare fact bytes above {empty}"
    );

    let mut admitted = small_graph();
    admitted.facts.reserve_exact(previous_capacity + 4_096);
    let corrected = retained_bytes(&admitted);
    let checked = check_with(&IDS, admitted, &budgets_for(corrected))
        .expect("the corrected budget covers the spare fact capacity");
    assert_eq!(checked.graph().facts.capacity(), retained_capacity);
}
