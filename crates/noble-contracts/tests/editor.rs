use noble_contracts::source::editor::{Candidate, Node, FORMAT};

fn limits() -> noble_contracts::Limits {
    noble_contracts::Limits::default()
}

fn hole() -> Candidate {
    Candidate {
        format: FORMAT,
        nodes: vec![
            Node::Integer(1),
            Node::Hole,
            Node::Word(String::from("+")),
        ],
    }
}

#[test]
fn editor_analysis_retains_real_word_constraints_without_admission() {
    let session = noble_contracts::source::Session::new();
    let result = session.analyze_editor(&hole(), &[], limits()).expect("analyze");
    assert_eq!(result.holes.len(), 1);
    let constraint = &result.holes[0];
    assert_eq!(constraint.input_stack, "?stack I64");
    assert_eq!(constraint.output_stack, "?stack I64 I64");
    assert_eq!(constraint.required_output_suffix, ["I64"]);
    assert!(constraint.unresolved_effect);
    assert!(constraint.known_effects.is_empty());
    assert!(result.unresolved_effect);
    assert!(result.known_effects.is_empty());
    let effectful = Candidate {
        format: FORMAT,
        nodes: vec![Node::Integer(1), Node::Hole, Node::Word(String::from("test.emit"))],
    };
    let effectful = session.analyze_editor(&effectful, &[], limits()).expect("effectful scheme");
    assert_eq!(effectful.holes[0].output_stack, "?stack Text");
    assert_eq!(effectful.holes[0].required_output_suffix, ["Text"]);
    assert!(effectful.holes[0].unresolved_effect);
    assert!(effectful.unresolved_effect);
    assert_eq!(effectful.known_effects, [0]);
    assert_eq!(session.generation(), 0);
}

#[test]
fn typed_direct_admission_refuses_hole_before_prepared() {
    let candidate = hole();
    let refusal = candidate.admitted_source(limits()).expect_err("direct hole admission");
    assert_eq!(refusal.stage(), noble_contracts::source::Stage::Acceptance);
    assert_eq!(refusal.diagnostic().message, "editor hole cannot enter source admission");
    let session = noble_contracts::source::Session::new();
    assert_eq!(session.generation(), 0);
}

#[test]
fn typed_nested_quotation_admission_refuses_hole_before_prepared() {
    let nested = Candidate {
        format: FORMAT,
        nodes: vec![Node::Quotation(vec![Node::Hole])],
    };
    let session = noble_contracts::source::Session::new();
    assert_eq!(session.analyze_editor(&nested, &[], limits()).expect("nested").holes.len(), 1);
    let refusal = nested.admitted_source(limits()).expect_err("nested hole admission");
    assert_eq!(refusal.stage(), noble_contracts::source::Stage::Acceptance);
    assert_eq!(refusal.diagnostic().message, "editor hole cannot enter source admission");
    assert_eq!(session.generation(), 0);
}

#[test]
fn editor_hole_free_source_uses_ordinary_kernel_preparation() {
    let session = noble_contracts::source::Session::new();
    let valid = Candidate {
        format: FORMAT,
        nodes: vec![
            Node::Integer(1),
            Node::Integer(2),
            Node::Word(String::from("+")),
        ],
    };
    let source = valid.admitted_source(limits()).expect("source encoding");
    let prepared = session.prepare(&source, &[], limits()).expect("kernel acceptance");
    assert_eq!(prepared.output(), &[noble_kernel::types::Ty::I64]);
    assert!(prepared.submission().is_some());

    for word in ["[", "]", "true", "1", "1 +", "test.emit\""] {
        let forged = Candidate {
            format: FORMAT,
            nodes: vec![Node::Word(String::from(word))],
        };
        assert!(forged.admitted_source(limits()).is_err(), "forged word {word:?}");
    }
    let mut tiny = limits();
    tiny.bytes = 4;
    assert!(valid.admitted_source(tiny).is_err());
    tiny = limits();
    tiny.nodes = 1;
    assert!(valid.admitted_source(tiny).is_err());
    tiny = limits();
    tiny.depth = 0;
    let nested = Candidate {
        format: FORMAT,
        nodes: vec![Node::Quotation(vec![Node::Integer(1)])],
    };
    assert!(nested.admitted_source(tiny).is_err());
}
