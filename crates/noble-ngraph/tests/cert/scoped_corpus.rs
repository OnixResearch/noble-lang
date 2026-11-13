#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this test exports two fixed, checked I64 certificate fixtures into the smaller Lean model vocabulary and fails closed on every shape it cannot project."
)]
//! Generated acceptance corpus for the Lean scoped-rewrite model (issue #108).
//!
//! The two source graphs are built and certified by the Rust proposer. This exporter
//! accepts only their one-target input/constant/guard fragment, checks the certificate
//! and both graph admissions first, and emits the actual ordered step fields into Lean.

use noble_ngraph::cert_scoped::{self, Derivation, Outcome, Scope, Step};
use noble_ngraph::ir::{Fact, Graph, Literal, NodeId, Op, Provenance, SubProfile};

use super::differential::TestIds;

#[path = "scoped_corpus/fixtures.rs"]
mod fixtures;

const COMMITTED: &str =
    include_str!("../../../../proofs/foundation/NobleFoundation/GuardedRewriteCorpus.lean");

fn bits(value: u64) -> String {
    format!("0x{value:016X}")
}

fn node(graph: &Graph, id: NodeId) -> &noble_ngraph::ir::Node {
    graph.nodes.get(id.index()).expect("fixture node exists")
}

fn operand_term(graph: &Graph, id: NodeId) -> String {
    match &node(graph, id).op {
        Op::Input(slot) => format!("(.input {slot})"),
        Op::Const(Literal::I64(value)) => {
            format!("(.constant {})", bits(value.cast_unsigned()))
        }
        other => panic!(
            "the model exporter only admits input/constant operands, got {}",
            other.name()
        ),
    }
}

fn exact_fact(fact: &Fact) -> u64 {
    match fact {
        Fact::Range {
            lo: Literal::I64(lo),
            hi: Literal::I64(hi),
        } if lo == hi => lo.cast_unsigned(),
        other => panic!("the model exporter requires a single-valued I64 range, got {other:?}"),
    }
}

fn guard_term(graph: &Graph, id: NodeId, model_name: &str) -> String {
    let Op::Guard { fact, value } = &node(graph, id).op else {
        panic!("the recorded scope is not a guard")
    };
    format!(
        "def {model_name} : GuardBefore := ⟨{}, {}, {}⟩\n",
        id.0,
        operand_term(graph, *value),
        bits(exact_fact(fact))
    )
}

fn binary_term(graph: &Graph, id: NodeId) -> String {
    let op = &node(graph, id).op;
    let (name, first, second) = match op {
        Op::I64Add(first, second) => ("add", *first, *second),
        Op::I64Sub(first, second) => ("sub", *first, *second),
        Op::I64Mul(first, second) => ("mul", *first, *second),
        other => panic!(
            "the model exporter only admits I64 arithmetic, got {}",
            other.name()
        ),
    };
    format!(
        ".binary .{name} {} {}",
        operand_term(graph, first),
        operand_term(graph, second)
    )
}

fn fact_terms(
    graph: &Graph,
    prefix: &str,
    declarations: &mut impl core::fmt::Write,
) -> Vec<String> {
    graph
        .facts
        .iter()
        .enumerate()
        .map(|(index, record)| {
            assert_eq!(
                record.provenance,
                Provenance::InputValidated,
                "only input-validated facts project into the model"
            );
            assert!(
                matches!(&node(graph, record.node).op, Op::Input(_)),
                "the model only includes input facts"
            );
            let name = format!("{prefix}InputFact{index}");
            let Op::Input(slot) = &node(graph, record.node).op else {
                panic!("fact source is not an input")
            };
            core::fmt::Write::write_fmt(
                declarations,
                format_args!(
                    "def {name} : InputFact := ⟨{slot}, {}⟩\n",
                    bits(exact_fact(&record.fact))
                ),
            )
            .expect("Lean declaration format writes to String");
            name
        })
        .collect()
}

fn graph_term(prefix: &str, graph: &Graph, target: NodeId, facts: &[String], left: bool) -> String {
    assert_eq!(graph.profile, SubProfile::I64);
    assert!(
        graph.input_shape.is_none(),
        "input shape is not represented"
    );
    assert!(
        graph.output_shape.is_none(),
        "output shape is not represented"
    );
    assert!(
        graph.nodes.iter().enumerate().all(|(index, item)| {
            index == target.index()
                || matches!(
                    &item.op,
                    Op::Input(_) | Op::Const(Literal::I64(_)) | Op::Guard { .. }
                )
        }),
        "the model only represents input, constant, guard and target nodes"
    );
    assert!(
        graph.outputs.as_slice() == [target],
        "the model projection has one result output"
    );
    assert!(
        graph.nodes.iter().enumerate().all(|(index, item)| {
            index <= target.index() || !matches!(&item.op, Op::Guard { .. })
        }),
        "the model fixture has no post-target guards"
    );
    let name = format!("{prefix}{}", if left { "Left" } else { "Right" });
    let target_node = if left {
        binary_term(graph, target)
    } else {
        match &node(graph, target).op {
            Op::Const(Literal::I64(value)) => format!(".constant {}", bits(value.cast_unsigned())),
            other => panic!(
                "the result endpoint must be a folded I64 constant, got {}",
                other.name()
            ),
        }
    };
    let before: Vec<_> = graph
        .nodes
        .iter()
        .enumerate()
        .filter(|&(index, item)| index < target.index() && matches!(&item.op, Op::Guard { .. }))
        .map(|(index, _)| format!("{prefix}Guard{index}"))
        .collect();
    format!(
        "def {name} : Graph :=\n  {{ inputCount := {}\n    inputFacts := [{}]\n    target := {}\n    node := {target_node}\n    before := [{}]\n    after := []\n    outputs := [.result] }}\n",
        graph.slots.len(), facts.join(", "), target.0, before.join(", ")
    )
}

fn scope_term(prefix: &str, graph: &Graph, step: &Step, scope: Scope, position: usize) -> String {
    match scope {
        Scope::Constant => ".constant".to_owned(),
        Scope::InputFact => {
            let operand = step.operands[position];
            let value = step.operand_values[position];
            let (index, _) = graph
                .facts
                .iter()
                .enumerate()
                .find(|(_, record)| {
                    record.node == operand
                        && record.provenance == Provenance::InputValidated
                        && exact_fact(&record.fact) == value
                })
                .expect("input-fact scope has a matching checked fact");
            format!(".inputFact {prefix}InputFact{index}")
        }
        Scope::Guard(id) => {
            assert!(
                id.0 < step.node.0,
                "a scoped certificate names only earlier guards"
            );
            format!(".guard {prefix}Guard{}", id.0)
        }
    }
}

fn step_term(prefix: &str, graph: &Graph, step: &Step) -> String {
    let rule = match step.rule {
        cert_scoped::Rule::Scoped => ".scoped",
        cert_scoped::Rule::Preflight => ".preflight",
    };
    let first = operand_term(graph, step.operands[0]);
    let second = operand_term(graph, step.operands[1]);
    let first_scope = scope_term(prefix, graph, step, step.scopes[0], 0);
    let second_scope = scope_term(prefix, graph, step, step.scopes[1], 1);
    format!(
        "{{ rule := {rule}, node := {}, first := {first}, second := {second}, firstScope := {first_scope}, secondScope := {second_scope}, firstValue := {}, secondValue := {}, claimed := {} }}",
        step.node.0, bits(step.operand_values[0]), bits(step.operand_values[1]), bits(step.claimed)
    )
}

fn export_case(name: &str, graph: Graph) -> String {
    let (certificate, result): (Derivation, Graph) = cert_scoped::propose(&TestIds, &graph);
    assert_eq!(certificate.revision, cert_scoped::SCOPED_REVISION);
    assert_eq!(certificate.relation, cert_scoped::Relation::Equality);
    assert_eq!(
        certificate.steps.len(),
        1,
        "each exported model case is one checked fold"
    );
    assert_eq!(
        cert_scoped::check(&TestIds, &graph, &result, &certificate),
        Outcome::Accepted
    );
    noble_ngraph::check::check(&TestIds, graph.clone()).expect("source graph is admitted");
    noble_ngraph::check::check(&TestIds, result.clone())
        .expect("candidate graph is admitted before export");
    let step = certificate.steps[0];
    let mut facts_text = String::new();
    let facts = fact_terms(&graph, name, &mut facts_text);
    let mut guards_text = String::new();
    for (index, item) in graph.nodes.iter().enumerate() {
        if !matches!(&item.op, Op::Guard { .. }) {
            continue;
        }
        let id = NodeId(u32::try_from(index).expect("fixture node index fits"));
        let guard_name = format!("{name}Guard{index}");
        guards_text.push_str(&guard_term(&graph, id, &guard_name));
    }
    let mut text = facts_text;
    text.push_str(&guards_text);
    text.push_str(&graph_term(name, &graph, step.node, &facts, true));
    text.push_str(&graph_term(name, &result, step.node, &facts, false));
    text.push_str(&format!(
        "def {name}Certificate : Derivation := {{ revision := 0, relation := .equality, steps := [{}] }}\n",
        step_term(name, &graph, &step)
    ));
    text.push_str(&format!(
        "theorem {name}_accepted : check {name}Left {name}Right {name}Certificate = .accepted := by\n  decide\n\n"
    ));
    text.push_str(&format!(
        "theorem {name}_preserves_all_inputs (inputs : Nat → Bits) :\n    observe {name}Left inputs = observe {name}Right inputs :=\n  accepted_preserves_observations {name}Left {name}Right {name}Certificate inputs {name}_accepted\n\n"
    ));
    text
}

fn lean_file() -> String {
    let mut text = String::from(
        "import NobleFoundation.GuardedRewrite\n\n/-!\n# Real Rust scoped-rewrite certificates, decided by the Lean model (issue #108)\n\nGenerated by `crates/noble-ngraph/tests/cert/scoped_corpus.rs` from certificates produced by the Rust proposer and accepted by the Rust checker. The exporter first admits both endpoints, then writes every step field into this smaller model fragment. The `*_accepted` theorems kernel-decide the encoded model certificates; `*_preserves_all_inputs` uses the model's universal observation theorem. This is not Rust implementation correspondence.\n-/\n\nnamespace NobleFoundation.GuardedRewriteCorpus\n\nopen NobleFoundation.GuardedRewrite\n\n"
    );
    text.push_str(&export_case("rustGuarded", fixtures::guarded_fixture()));
    text.push_str(&export_case("rustPreflight", fixtures::preflight_fixture()));
    text.push_str("end NobleFoundation.GuardedRewriteCorpus\n");
    text
}

#[test]
fn the_lean_file_is_the_real_rust_certificate_emission() {
    assert_eq!(lean_file(), COMMITTED, "regenerate GuardedRewriteCorpus.lean with the ignored writer, then rerun the foundation lane");
}

#[test]
#[ignore = "writes proofs/foundation/NobleFoundation/GuardedRewriteCorpus.lean; run after a deliberate certificate change"]
fn write_the_lean_file() {
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proofs/foundation/NobleFoundation/GuardedRewriteCorpus.lean"
        ),
        lean_file(),
    )
    .expect("generated file written");
}
