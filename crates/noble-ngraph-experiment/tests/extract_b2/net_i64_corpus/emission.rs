use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use super::cases::{build_fixtures, checked_and_net, rust_result, Fixture};
use super::codec::{lean_nodes, lean_value, rust_net_literal};

const PREAMBLE: &str = include_str!("preamble.lean");

fn corpus_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../proofs/foundation/NobleFoundation/NetI64Corpus.lean")
}

#[test]
fn generated_lean_corpus_is_fresh() {
    let path = corpus_path();
    let retained = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert_eq!(
        retained,
        render_corpus(),
        "regenerate the NetI64 Rust corpus"
    );
}

#[test]
#[ignore = "writes the kernel-checked corpus from freshly lowered Rust nets"]
fn regenerate_lean_corpus() {
    let path = corpus_path();
    std::fs::write(&path, render_corpus())
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

fn render_corpus() -> String {
    let mut source = String::from(PREAMBLE);
    for fixture in build_fixtures() {
        write_fixture(&mut source, &fixture);
    }
    source.push_str("end NobleFoundation.NetI64Corpus\n");
    source
}

fn render_inputs(fixture: &Fixture) -> String {
    fixture
        .inputs
        .iter()
        .map(|value| match value {
            noble_ngraph::sem::Value::I64(value) => format!(
                "BitVec.ofNat 64 {}",
                u64::from_le_bytes(value.to_le_bytes())
            ),
            other => panic!("non-I64 corpus input: {other:?}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_fixture(source: &mut String, fixture: &Fixture) {
    let (checked, net) = checked_and_net(fixture);
    let (outputs, interactions) = rust_result(fixture, &checked, &net);
    let nodes = lean_nodes(checked.graph()).expect("whitelisted graph serializes");
    let inputs = render_inputs(fixture);
    let outputs = outputs
        .into_iter()
        .map(lean_value)
        .collect::<Vec<_>>()
        .join(", ");
    let net_literal = rust_net_literal(&net).expect("Rust net serializes");
    let output_nodes = checked
        .graph()
        .outputs
        .iter()
        .map(|node| node.0.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let input_arity = checked.graph().slots.len();
    let graph_id = noble_ngraph::ir::hex(&checked.id());
    let build_key = noble_ngraph::ir::hex(&net.build_key);
    write!(
        source,
        "-- graph={graph_id}; contract={}; lowering={}; build_key={build_key}; inputs=[{}]\ndef {}Program : Program := {{ inputArity := {input_arity}, nodes := [{nodes}], outputs := [{output_nodes}] }}\ndef {}Inputs : List (BitVec 64) := [{inputs}]\ndef {}RustNet : INet.XNet NetSym := {net_literal}\ndef {}RustOutputs : List PackedSchedule.Val := [{outputs}]\ndef {}RustInteractions : Nat := {interactions}\n\ntheorem {}_translation : (Program.translateChecked {}Program {}Inputs).map canonicalNet = some (canonicalNet {}RustNet) := by decide\ntheorem {}_normalization : normalizeReadBack {}Program {}Inputs = some (some {}RustOutputs, {}RustInteractions) := by decide\ntheorem {}_interaction_count : Program.interactionCount {}Program = {}RustInteractions := by decide\n\n",
        net.contract_revision,
        net.lowering_revision,
        net.input_values
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(","),
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
        fixture.name,
    )
    .expect("writing to String");
}
