use super::*;

#[test]
fn the_profile_and_interface_are_pinned() {
    assert_eq!(llvm::PROFILE, "ngraph-llvm-i64/0");
    assert_eq!(llvm::TARGET_TRIPLE, "x86_64-unknown-linux-gnu");
    assert_eq!(llvm::CALLING_CONVENTION, "ccc");
    assert_eq!(llvm::FUNCTION, "noble_i64");
    assert_eq!((llvm::PARAMETERS, llvm::OUTPUTS), (2, 1));
    assert!(!llvm::DATA_LAYOUT.is_empty(), "the data layout is declared");
    assert!(
        llvm::DATA_LAYOUT.contains("-i64:64"),
        "the data layout declares the 64-bit integer ABI"
    );
    let expected = [
        (Refusal::Header, "L-HEADER"),
        (Refusal::Target, "L-TARGET"),
        (Refusal::Interface, "L-INTERFACE"),
        (Refusal::Param, "L-PARAM"),
        (Refusal::Shape, "L-SHAPE"),
        (Refusal::Fact, "L-FACT"),
        (Refusal::Guard, "L-GUARD"),
        (Refusal::Equality, "L-EQUALITY"),
        (Refusal::Operation("f32.add"), "L-OPERATION"),
        (Refusal::Output, "L-OUTPUT"),
        (Refusal::Instruction, "L-INSTRUCTION"),
        (Refusal::Opcode, "L-OPCODE"),
        (Refusal::Flags, "L-FLAGS"),
        (Refusal::Operand, "L-OPERAND"),
        (Refusal::Return, "L-RETURN"),
        (Refusal::NodeMap, "L-NODEMAP"),
        (Refusal::Operands, "L-OPERANDS"),
        (Refusal::Profile("ngraph-f32/0"), "L-PROFILE"),
    ];
    for (refusal, label) in expected {
        assert_eq!(refusal.name(), label, "the diagnostic name is pinned");
    }
}

#[test]
fn admission_names_every_out_of_profile_graph_feature() {
    let mut graph = simple_graph("llvm-profile-refusals");

    graph.profile = SubProfile::F32;
    assert_eq!(
        llvm::admit_graph(&graph),
        Err(Refusal::Profile("ngraph-f32/0"))
    );

    graph = simple_graph("llvm-interface-refusal");
    graph.slots.pop();
    assert_eq!(llvm::admit_graph(&graph), Err(Refusal::Interface));

    graph = simple_graph("llvm-param-refusal");
    graph.slots[0].kind = SlotKind::Param;
    assert_eq!(llvm::admit_graph(&graph), Err(Refusal::Param));

    graph = simple_graph("llvm-shape-refusal");
    graph.input_shape = Some(vec![ShapeTree::UNIT]);
    assert_eq!(llvm::admit_graph(&graph), Err(Refusal::Shape));

    graph = simple_graph("llvm-fact-refusal");
    graph.facts.push(noble_ngraph::ir::FactRecord {
        node: noble_ngraph::ir::NodeId(0),
        fact: Fact::Bit,
        provenance: Provenance::Asserted,
    });
    assert_eq!(llvm::admit_graph(&graph), Err(Refusal::Fact));

    graph = simple_graph("llvm-output-refusal");
    graph.outputs.push(graph.outputs[0]);
    assert_eq!(llvm::admit_graph(&graph), Err(Refusal::Output));

    graph = simple_graph("llvm-equality-refusal");
    graph.nodes[2].op = Op::I64Eq(noble_ngraph::ir::NodeId(0), noble_ngraph::ir::NodeId(1));
    assert_eq!(llvm::admit_graph(&graph), Err(Refusal::Equality));

    graph = simple_graph("llvm-guard-refusal");
    graph.nodes[2].op = Op::Guard {
        fact: Fact::Bit,
        value: noble_ngraph::ir::NodeId(0),
    };
    assert_eq!(llvm::admit_graph(&graph), Err(Refusal::Guard));

    graph = simple_graph("llvm-param-node-refusal");
    graph.nodes[0].op = Op::Param(0);
    assert_eq!(llvm::admit_graph(&graph), Err(Refusal::Param));

    graph = simple_graph("llvm-f32-refusal");
    graph.nodes[2].op = Op::F32Add(noble_ngraph::ir::NodeId(0), noble_ngraph::ir::NodeId(1));
    assert_eq!(
        llvm::admit_graph(&graph),
        Err(Refusal::Operation("f32.add"))
    );
}

#[test]
fn every_fixture_class_is_covered() {
    let names: Vec<&str> = artifacts().iter().map(|held| held.name).collect();
    for required in [
        "wrapping",
        "negative",
        "subtractive",
        "shared",
        "foldable",
        "foldable-rewritten",
    ] {
        assert!(
            names.contains(&required),
            "the fixture set holds {required}"
        );
    }
    let everything = artifacts();
    let foldable = artifact_named(&everything, "foldable");
    let rewritten = artifact_named(&everything, "foldable-rewritten");
    assert!(
        foldable.text.contains(" = add i64 3, 5\n"),
        "the foldable artifact really spells the constant addition LLVM may fold"
    );
    assert!(
        !rewritten.text.contains(" = add i64 3, 5\n"),
        "the rewritten endpoint has no constant addition left"
    );
    assert_ne!(
        foldable.text, rewritten.text,
        "the pair's two modules are different artifacts"
    );
}

#[test]
fn a_shared_fixture_lowers_to_the_expected_module() {
    let everything = artifacts();
    let held = artifact_named(&everything, "shared");
    assert_eq!(
        held.text,
        format!(
            concat!(
                "{header}\n",
                "target datalayout = \"{layout}\"\n",
                "target triple = \"{triple}\"\n\n",
                "define i64 @noble_i64(i64 noundef %a0, i64 noundef %a1) {{\n",
                "entry:\n",
                "  %v2 = mul i64 %a0, %a0\n",
                "  %v3 = add i64 %v2, %v2\n",
                "  %v5 = mul i64 %v3, -1\n",
                "  %v6 = mul i64 %v2, -1\n",
                "  %v7 = sub i64 %v5, %v6\n",
                "  %v8 = add i64 %v7, %a1\n",
                "  ret i64 %v8\n",
                "}}\n",
            ),
            header = llvm::HEADER_LINE,
            layout = llvm::DATA_LAYOUT,
            triple = llvm::TARGET_TRIPLE,
        ),
        "the emitted module is exactly the documented text form"
    );
    assert_eq!(
        held.module.instructions.len(),
        6,
        "one instruction per arithmetic node"
    );
    assert_eq!(
        held.module.output,
        Operand::Value(8),
        "the single ret names the selected output node"
    );
    assert!(
        held.module
            .instructions
            .iter()
            .any(|instruction| instruction.operands[0] == Operand::Value(2)
                && instruction.operands[1] == Operand::Value(2)),
        "the shared node is named twice rather than recomputed"
    );
}

#[test]
fn the_validator_accepts_every_retained_artifact() {
    for held in artifacts() {
        let text = llvm::emit(&held.checked).expect("inside the profile");
        assert_eq!(
            text, held.text,
            "the emission is deterministic for {}",
            held.name
        );
        let module = llvm::validate(&held.checked, &held.text)
            .unwrap_or_else(|refusal| panic!("{} validates: {refusal:?}", held.name));
        assert_eq!(module, held.module, "the parsed form is stable");
        for inputs in EXECUTIONS {
            executed(&held, inputs);
        }
    }
}

#[test]
fn interpreter_storage_does_not_scale_with_an_ssa_identifier() {
    let text = format!(
        concat!(
            "{}\n",
            "target datalayout = \"{}\"\n",
            "target triple = \"{}\"\n\n",
            "define i64 @{}(i64 noundef %a0, i64 noundef %a1) {{\n",
            "entry:\n",
            "  %v4294967295 = add i64 %a0, %a1\n",
            "  ret i64 %v4294967295\n",
            "}}\n",
        ),
        llvm::HEADER_LINE,
        llvm::DATA_LAYOUT,
        llvm::TARGET_TRIPLE,
        llvm::FUNCTION
    );
    let module = llvm::parse(&text).expect("the maximum u32 destination is a canonical token");
    assert_eq!(
        llvm::execute(&module, [19, 23]),
        Some(42),
        "interpreter storage scales with parsed instructions, not the numeric destination"
    );
}

#[test]
fn every_control_is_refused_with_its_own_name() {
    let everything = artifacts();
    let records = control_records(&everything);
    let classes: Vec<&str> = records.iter().map(|record| record.class).collect();
    for required in [
        "operand-swapped",
        "constant-changed",
        "negative-zero",
        "return-changed",
        "flag-nsw",
        "flag-nuw",
        "opcode-unsupported",
        "instruction-call",
        "attribute-dropped",
        "convention-named",
        "triple-changed",
        "datalayout-changed",
        "artifact-substituted",
        "return-duplicated",
        "text-after-brace",
        "instruction-missing",
        "destination-changed",
    ] {
        assert!(classes.contains(&required), "the controls cover {required}");
    }
    assert!(
        records.iter().filter(|record| record.model).count() >= 5,
        "at least five controls reach the semantic model, not only the parser"
    );
}
