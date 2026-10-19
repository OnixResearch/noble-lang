#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::platform_dependent_cast,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Phase B B3: source-level refusals of D7 admission/extraction.
//!
//! Every refusal is an `Err(Diagnostic)` returned before any graph exists,
//! so no graph node is evaluated. The ordinary-path behavior of the same
//! sources is recorded by the `ngraph-phase-b-extract` harness in
//! `verification/ngraph/phase-b-extract.json` (record `B3-I64-source-controls`).

use noble_ngraph::diag::{BudgetKind, Code};
use noble_ngraph::extract::{self, source, Budget, Request};
use noble_ngraph::ir::Op;
use noble_ngraph_experiment::controls::{self, chain, outcome};
use noble_ngraph_experiment::scheme::BLAKE3;

#[test]
fn every_source_control_is_refused_with_its_d6_code() {
    let cases = controls::run();
    assert_eq!(cases.len(), 16);
    for c in &cases {
        assert_eq!(c.observed(), c.expected, "{}: {:?}", c.name, c.result);
    }
    let k20 = cases
        .iter()
        .find(|c| c.name == "expansion-chain-k20")
        .expect("k20");
    let d = k20.result.as_ref().expect_err("refused");
    // Charged before pruning: the 262,145th created node refuses, although
    // the pruned graph would be the single constant 7.
    assert_eq!(d.code, Code::Budget(BudgetKind::Expansion));
    assert_eq!(d.budget, Some((262_144, 262_145)));
}

#[test]
fn unreached_effectful_quotation_is_not_refused() {
    // A dropped, never-run effectful quotation leaves the root bound empty
    // and is never reached: the subject extracts to `const 5` (the ordinary
    // path emits nothing either).
    let p = source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        &[],
        "[ \"hi\" test.emit ] drop 5",
        &[],
    )
    .expect("source");
    let admitted = extract::admit(
        &BLAKE3,
        &p.submission,
        &Request {
            profile: "ngraph-i64/0",
            claimed_closure: &p.claimed_closure,
            budget: Budget::default(),
        },
    )
    .expect("admitted");
    let x = extract::extract(&BLAKE3, &admitted, &noble_ngraph::check::Budgets::default())
        .expect("extracted");
    let ops: Vec<_> = x
        .checked
        .graph()
        .nodes
        .iter()
        .map(|n| n.op.clone())
        .collect();
    assert!(matches!(ops.as_slice(), [Op::Const(_)]));
}

#[test]
fn closure_and_profile_admission() {
    let p = source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        &["def sq [ dup * ]"],
        "sq swap sq +",
        &source::i64s(2),
    )
    .expect("source");
    let refuse = |claimed: &[u64], profile: &str, budget: Budget| {
        outcome(&p.submission, claimed, profile, budget)
            .expect_err("refused")
            .code
    };
    let d = Budget::default();
    // E-CLOSURE: missing, extra and repeated identities.
    assert_eq!(refuse(&[], "ngraph-i64/0", d), Code::Closure);
    let mut extra = p.claimed_closure.clone();
    extra.push(u64::MAX);
    assert_eq!(refuse(&extra, "ngraph-i64/0", d), Code::Closure);
    let twice = [p.claimed_closure[0], p.claimed_closure[0]];
    assert_eq!(refuse(&twice, "ngraph-i64/0", d), Code::Closure);
    // E-PROFILE: unknown sub-profile, F32 (authored directly under B0(b)),
    // and a budget raised above the /0 default.
    assert_eq!(refuse(&p.claimed_closure, "ngraph-i64/1", d), Code::Profile);
    assert_eq!(refuse(&p.claimed_closure, "ngraph-f32/0", d), Code::Profile);
    let raised = Budget {
        work_units: d.work_units + 1,
        ..d
    };
    assert_eq!(
        refuse(&p.claimed_closure, "ngraph-i64/0", raised),
        Code::Profile
    );
    // E-SUBJECT: a forged root whose `swap` became `+` is not kernel-accepted.
    let mut forged = p.submission.clone();
    if let noble_kernel::untrusted::Node::Invocation { def, .. } =
        &mut forged.body.candidate.nodes[1]
    {
        *def = noble_kernel::contracts::Definition(4);
    }
    assert_eq!(
        outcome(&forged, &p.claimed_closure, "ngraph-i64/0", d)
            .expect_err("forged")
            .code,
        Code::Subject
    );
}

#[test]
fn supplied_dependency_list_must_match_the_body() {
    let (mut sub, closure) = chain(2);
    let d1 = sub.definitions[1].definition;
    sub.environment.deps[d1.0 as usize].clear();
    let d = outcome(&sub, &closure, "ngraph-i64/0", Budget::default()).expect_err("refused");
    assert_eq!(d.code, Code::Closure);
}

#[test]
fn chain_budgets_are_charged_before_pruning() {
    let (sub, closure) = chain(4);
    let admitted = extract::admit(
        &BLAKE3,
        &sub,
        &Request {
            profile: "ngraph-i64/0",
            claimed_closure: &closure,
            budget: Budget::default(),
        },
    )
    .expect("admitted");
    let x = extract::extract(&BLAKE3, &admitted, &noble_ngraph::check::Budgets::default())
        .expect("extracted");
    // const 0, 16 × (const 1, add), const 7: 34 created, 33 pruned.
    assert_eq!(x.stats.expanded_nodes, 34);
    assert_eq!(x.stats.pruned_nodes, 33);
    assert_eq!(x.checked.graph().nodes.len(), 1);
    assert_eq!(x.stats.static_expansion_bound, 34);
    assert_eq!(x.stats.max_inline_depth, 5);
    let d = Budget::default();
    let refuse = |budget: Budget| {
        outcome(&sub, &closure, "ngraph-i64/0", budget)
            .expect_err("refused")
            .code
    };
    assert_eq!(
        refuse(Budget {
            expanded_nodes: 33,
            ..d
        }),
        Code::Budget(BudgetKind::Expansion)
    );
    assert_eq!(
        refuse(Budget {
            inline_depth: 4,
            ..d
        }),
        Code::Budget(BudgetKind::InlineDepth)
    );
    assert_eq!(
        refuse(Budget {
            work_units: 20,
            ..d
        }),
        Code::Budget(BudgetKind::Work)
    );
}
