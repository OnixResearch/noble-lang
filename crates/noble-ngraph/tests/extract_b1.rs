//! Phase B B1: lowering of the design's I1–I4 Core programs through D7
//! extraction, evaluated by L3 (reference) on the design's vectors.
//!
//! Surface syntax: the design writes `(block (I64 I64) (I64) [ … ])` and
//! `sq = [ dup * ]`; the current Core frontend has no typed block form and
//! spells definitions `def sq [ dup * ]`, so each root is the block's body
//! prepared with the block's input types (bottom-first) — exactly what
//! `noble session` submits when those values are on its stack.

mod common;

use common::IDS;
use noble_kernel::types::Ty;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::extract::{self, source, Budget, Request};
use noble_ngraph::ir::{Op, SubjectKind};
use noble_ngraph::Value;

fn lower(defs: &[&str], root: &str, inputs: &[Ty]) -> extract::Extraction {
    let p = source::prepare(defs, root, inputs).expect("frontend accepts");
    let admitted = extract::admit(
        &IDS,
        &p.submission,
        &Request {
            profile: "ngraph-i64/0",
            claimed_closure: &p.claimed_closure,
            budget: Budget::default(),
        },
    )
    .expect("admitted");
    extract::extract(&IDS, &admitted, &noble_ngraph::check::Budgets::default()).expect("extracted")
}

fn run(x: &extract::Extraction, inputs: &[i64]) -> Vec<Value> {
    let r = eval::reference(
        &x.checked,
        &eval::i64_inputs(inputs),
        None,
        &Control::default(),
    );
    match r.obs {
        Obs::Normal(v) => v,
        other => panic!("unexpected observation {other:?}"),
    }
}

fn ops(x: &extract::Extraction) -> Vec<Op> {
    x.checked
        .graph()
        .nodes
        .iter()
        .map(|n| n.op.clone())
        .collect()
}

#[test]
fn i1_routing_constants_and_operand_order() {
    let x = lower(&[], "dup * swap 3 * + 7 -", &source::i64s(2));
    assert_eq!(run(&x, &[5, 4]), vec![Value::I64(24)]);
    let g = x.checked.graph();
    assert_eq!(g.subject.kind, SubjectKind::Extracted);
    // y*y + 3x - 7: `-` keeps the deeper operand first (K-NUM-01).
    assert!(matches!(ops(&x).last(), Some(Op::I64Sub(..))));
    assert_eq!(x.stats.pruned_nodes, 0);
}

#[test]
fn i2_captures_composition_and_one_shared_capture() {
    let x = lower(
        &[],
        "2 quote [ + ] compose dup compose run",
        &source::i64s(1),
    );
    assert_eq!(run(&x, &[40]), vec![Value::I64(44)]);
    // One `const 2` node, used by both additions (the dup'd composition
    // shares the captured id; no copy is made).
    let o = ops(&x);
    let consts: Vec<usize> = (0..o.len())
        .filter(|&i| matches!(o[i], Op::Const(_)))
        .collect();
    assert_eq!(consts.len(), 1);
    let c = noble_ngraph::NodeId(consts[0] as u32);
    let uses = o
        .iter()
        .filter(|op| matches!(op, Op::I64Add(_, b) if *b == c))
        .count();
    assert_eq!(uses, 2);
}

#[test]
fn i3_repeated_named_call_to_one_identity() {
    let x = lower(&["def sq [ dup * ]"], "sq swap sq +", &source::i64s(2));
    assert_eq!(run(&x, &[3, 4]), vec![Value::I64(25)]);
    let g = x.checked.graph();
    assert_eq!(
        g.subject.closure.len(),
        1,
        "both calls resolve to one identity"
    );
    let call_sites: Vec<_> = g
        .nodes
        .iter()
        .filter_map(|n| n.origin.as_ref())
        .filter_map(|o| o.expansion.first())
        .collect();
    assert!(call_sites.iter().all(|s| s.definition.is_none()));
    assert_eq!(x.stats.max_inline_depth, 1);
}

#[test]
fn i4_transition_for_1000_host_steps() {
    let x = lower(&[], "swap [ * ] dip + dup", &source::i64s(3));
    let mut s: i64 = 1;
    let mut seed: u64 = 0x1404_0001;
    for _ in 0..1000 {
        seed = seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let i = (seed >> 7) as i64;
        let k = (seed.rotate_left(17) | 1) as i64;
        let out = run(&x, &[s, i, k]);
        let next = s.wrapping_mul(k).wrapping_add(i);
        assert_eq!(out, vec![Value::I64(next), Value::I64(next)]);
        s = next;
    }
}
