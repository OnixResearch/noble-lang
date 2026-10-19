fn limits() -> noble_contracts::Limits {
    noble_contracts::Limits {
        bytes: 65_536,
        nodes: 16_384,
        depth: 64,
        work: 2_000_000,
    }
}

fn hole() -> noble_contracts::source::editor::Candidate {
    noble_contracts::source::editor::Candidate {
        format: noble_contracts::source::editor::FORMAT,
        nodes: vec![
            noble_contracts::source::editor::Node::Integer(1),
            noble_contracts::source::editor::Node::Hole,
            noble_contracts::source::editor::Node::Word(String::from("+")),
        ],
    }
}

#[test]
fn editor_analysis_retains_real_word_constraints_without_admission() -> Result<(), String> {
    let session = noble_contracts::source::Session::new();
    let result = session
        .analyze_editor(&hole(), &[], limits())
        .map_err(|error| format!("analyze: {error:?}"))?;
    assert_eq!(result.holes.len(), 1);
    let constraint = &result.holes[0];
    assert_eq!(constraint.input_stack, "?stack I64");
    assert_eq!(constraint.output_stack, "?stack I64 I64");
    assert_eq!(constraint.required_output_suffix, ["I64"]);
    assert!(constraint.unresolved_effect);
    assert!(constraint.known_effects.is_empty());
    assert!(result.unresolved_effect);
    assert!(result.known_effects.is_empty());
    let effectful = noble_contracts::source::editor::Candidate {
        format: noble_contracts::source::editor::FORMAT,
        nodes: vec![
            noble_contracts::source::editor::Node::Integer(1),
            noble_contracts::source::editor::Node::Hole,
            noble_contracts::source::editor::Node::Word(String::from("test.emit")),
        ],
    };
    let effectful = session
        .analyze_editor(&effectful, &[], limits())
        .map_err(|error| format!("effectful scheme: {error:?}"))?;
    assert_eq!(effectful.holes[0].output_stack, "?stack Text");
    assert_eq!(effectful.holes[0].required_output_suffix, ["Text"]);
    assert!(effectful.holes[0].unresolved_effect);
    assert!(effectful.unresolved_effect);
    assert_eq!(effectful.known_effects, [0]);
    assert_eq!(session.generation(), 0);
    Ok(())
}

#[test]
fn typed_direct_admission_refuses_hole_before_prepared() {
    let candidate = hole();
    let refusal = candidate
        .admitted_source(limits())
        .expect_err("direct hole admission");
    assert_eq!(refusal.stage(), noble_contracts::source::Stage::Acceptance);
    assert_eq!(
        refusal.diagnostic().message,
        "editor hole cannot enter source admission"
    );
    let session = noble_contracts::source::Session::new();
    assert_eq!(session.generation(), 0);
}

#[test]
fn typed_nested_quotation_admission_refuses_hole_before_prepared() -> Result<(), String> {
    let nested = noble_contracts::source::editor::Candidate {
        format: noble_contracts::source::editor::FORMAT,
        nodes: vec![noble_contracts::source::editor::Node::Quotation(vec![
            noble_contracts::source::editor::Node::Hole,
        ])],
    };
    let session = noble_contracts::source::Session::new();
    assert_eq!(
        session
            .analyze_editor(&nested, &[], limits())
            .map_err(|error| format!("nested: {error:?}"))?
            .holes
            .len(),
        1
    );
    let refusal = nested
        .admitted_source(limits())
        .expect_err("nested hole admission");
    assert_eq!(refusal.stage(), noble_contracts::source::Stage::Acceptance);
    assert_eq!(
        refusal.diagnostic().message,
        "editor hole cannot enter source admission"
    );
    assert_eq!(session.generation(), 0);
    Ok(())
}

#[test]
fn editor_hole_free_source_uses_ordinary_kernel_preparation() -> Result<(), String> {
    let session = noble_contracts::source::Session::new();
    let valid = noble_contracts::source::editor::Candidate {
        format: noble_contracts::source::editor::FORMAT,
        nodes: vec![
            noble_contracts::source::editor::Node::Integer(1),
            noble_contracts::source::editor::Node::Integer(2),
            noble_contracts::source::editor::Node::Word(String::from("+")),
        ],
    };
    let source = valid
        .admitted_source(limits())
        .map_err(|error| format!("source encoding: {error:?}"))?;
    let prepared = session
        .prepare(&source, &[], limits())
        .map_err(|error| format!("kernel acceptance: {error:?}"))?;
    assert_eq!(prepared.output(), &[noble_kernel::types::Ty::I64]);
    assert!(prepared.submission().is_some());

    for word in ["[", "]", "true", "1", "1 +", "test.emit\""] {
        let forged = noble_contracts::source::editor::Candidate {
            format: noble_contracts::source::editor::FORMAT,
            nodes: vec![noble_contracts::source::editor::Node::Word(String::from(
                word,
            ))],
        };
        assert!(
            forged.admitted_source(limits()).is_err(),
            "forged word {word:?}"
        );
    }
    let mut tiny = limits();
    tiny.bytes = 4;
    assert!(valid.admitted_source(tiny).is_err());
    tiny = limits();
    tiny.nodes = 1;
    assert!(valid.admitted_source(tiny).is_err());
    tiny = limits();
    tiny.depth = 0;
    let nested = noble_contracts::source::editor::Candidate {
        format: noble_contracts::source::editor::FORMAT,
        nodes: vec![noble_contracts::source::editor::Node::Quotation(vec![
            noble_contracts::source::editor::Node::Integer(1),
        ])],
    };
    assert!(nested.admitted_source(tiny).is_err());
    Ok(())
}
