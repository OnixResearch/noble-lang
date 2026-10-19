#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! B3: every core rejection control yields its D6 code before protected
//! execution (0 evaluated nodes for static and preflight refusals; a guard
//! failure evaluates no dependent node and yields no outputs).

use noble_ngraph_experiment::rejections::run_all;

#[test]
fn every_core_rejection_yields_its_code() {
    let all = run_all();
    let mut failures = Vec::new();
    for r in &all {
        if !r.passed() {
            failures.push(format!(
                "{}: expected {} observed {} ({})",
                r.name, r.expected, r.observed, r.detail
            ));
        }
        if r.observed != "G-FACT-FALSE" {
            assert_eq!((r.l3_evaluated, r.l4_evaluated), (0, 0), "{}", r.name);
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
    for group in ["false-fact", "bad-index", "malformed", "budget"] {
        assert!(all.iter().any(|r| r.group == group));
    }
}

#[test]
fn guard_failure_stops_before_the_dependent_node() {
    let all = run_all();
    let r = all
        .iter()
        .find(|r| r.name.starts_with("violating intermediate"))
        .expect("case");
    // Nodes: %0 input, %1 add, %2 guard (fails); %3 select never runs.
    assert_eq!(r.l3_evaluated, 3);
}
