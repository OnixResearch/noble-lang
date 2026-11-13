use super::*;

fn revision_text() -> String {
    format!(
        concat!(
            "  \"revisions\": {{\n",
            "    \"ir\": {},\n",
            "    \"semantic\": {},\n",
            "    \"facts\": {},\n",
            "    \"lowering\": {},\n",
            "    \"llvm_parser\": {},\n",
            "    \"llvm_validator\": {},\n",
            "    \"rewrite_certificate\": {},\n",
            "    \"production_backend\": {}\n  }},\n",
        ),
        json_string(noble_ngraph::ir::IR_VERSION),
        json_string(noble_ngraph::ir::SEMANTIC_REVISION),
        json_string(noble_ngraph::ir::FACT_RULE_REVISION),
        json_string(llvm::PROFILE),
        json_string(llvm::PROFILE),
        json_string(llvm::PROFILE),
        json_string(noble_ngraph::cert::CERT_REVISION),
        json_string(noble_ngraph::compiled::COMPILER_REVISION)
    )
}

fn assumption_text() -> &'static str {
    concat!(
        "  \"assumptions\": [\n",
        "    \"I64 addition, subtraction and multiplication wrap modulo 2^64; flagged LLVM forms are refused.\",\n",
        "    \"The ABI is exactly two i64 noundef arguments and one returned i64; memory, calls, globals and externals are outside the profile.\",\n",
        "    \"The input domain is every unsigned 64-bit pair and the observation is the returned i64 value.\",\n",
        "    \"The Lean theorem proves the parsed-instruction model; Rust parser-to-model correspondence and native-code correspondence are not proved here.\",\n",
        "    \"Recorded concrete executions are spot checks and do not replace the universal theorem.\"\n",
        "  ],\n"
    )
}

fn target_text() -> String {
    format!(
        "  \"target\": {{\n    \"triple\": {},\n    \"data_layout\": {},\n    \
         \"calling_convention\": {},\n    \"function\": {},\n    \
         \"parameters\": {},\n    \"result\": \"i64\",\n    \"attributes\": [],\n    \
         \"flags\": []\n  }},\n",
        json_string(llvm::TARGET_TRIPLE),
        json_string(llvm::DATA_LAYOUT),
        json_string(llvm::CALLING_CONVENTION),
        json_string(llvm::FUNCTION),
        json_list(&["i64 noundef %a0".to_owned(), "i64 noundef %a1".to_owned()])
    )
}

fn execution_entries(held: &Artifact) -> String {
    EXECUTIONS
        .iter()
        .enumerate()
        .map(|(index, inputs)| {
            let observed = executed(held, *inputs);
            format!(
                "{{\n          \"theorem\": {},\n          \"inputs\": [{}, {}],\n          \
                 \"module\": {},\n          \"reference\": {}\n        }}",
                json_string(&format!(
                    "{}_execution_{index}_agrees",
                    lean_name(held.name)
                )),
                json_string(&hex_bits(inputs[0])),
                json_string(&hex_bits(inputs[1])),
                json_string(&hex_bits(observed)),
                json_string(&hex_bits(reference_value(&held.checked, *inputs)))
            )
        })
        .collect::<Vec<String>>()
        .join(",\n        ")
}

fn artifact_entry(held: &Artifact) -> String {
    let graph_path = format!("artifacts/{}.graph", held.name);
    let module_path = format!("artifacts/{}.ll", held.name);
    let graph_text = noble_ngraph::format::graph::print(&held.graph);
    format!(
        concat!(
            "    {{\n      \"name\": {},\n      \"doc\": {},\n      \"role\": {},\n",
            "      \"graph\": {},\n      \"module\": {},\n      \"graph_identity\": {},\n",
            "      \"graph_text_identity\": {},\n      \"module_identity\": {},\n",
            "      \"nodes\": {},\n      \"instructions\": {},\n      \"output\": {},\n",
            "      \"validator\": \"accepted\",\n      \"executions\": [\n        {}\n",
            "      ]\n    }}",
        ),
        json_string(held.name),
        json_string(held.doc),
        json_string(if held.name.ends_with("rewritten") {
            "rewritten"
        } else {
            "original"
        }),
        json_string(&graph_path),
        json_string(&module_path),
        json_string(&noble_ngraph::ir::hex(&held.checked.id())),
        json_string(&retained_identity(
            "noble.ngraph.retained-text.v1",
            &graph_path,
            &graph_text
        )),
        json_string(&noble_ngraph::ir::hex(&llvm::module_identity(
            &TestIds, &held.text
        ))),
        held.graph.nodes.len(),
        held.module.instructions.len(),
        json_string(&operand_text(held.module.output)),
        execution_entries(held)
    )
}

fn control_entry(record: &ControlRecord) -> String {
    format!(
        concat!(
            "    {{\n      \"artifact\": {},\n      \"class\": {},\n",
            "      \"refusal\": {},\n      \"stage\": {},\n      \"model\": {},\n",
            "      \"path\": {},\n      \"text_identity\": {}\n    }}",
        ),
        json_string(record.artifact),
        json_string(record.class),
        json_string(record.refusal),
        json_string(record.stage),
        record.model,
        json_string(&control_path(record.artifact, record.class)),
        json_string(&retained_identity(
            "noble.ngraph.retained-text.v1",
            &control_path(record.artifact, record.class),
            &record.text
        ))
    )
}

fn source_binding_entries() -> String {
    source_bindings()
        .iter()
        .map(|(path, source)| {
            format!(
                "    {{\"path\": {}, \"identity\": {}}}",
                json_string(path),
                json_string(&retained_identity(
                    "noble.ngraph.retained-text.v1",
                    path,
                    source
                ))
            )
        })
        .collect::<Vec<String>>()
        .join(",\n")
}

pub(super) fn retained_json(all: &[Artifact], records: &[ControlRecord]) -> String {
    let certificate = certificate_text(all);
    let request = request_text();
    let corpus = lean_source(all, records);
    let mut text = String::from("{\n  \"schema\": \"noble-llvm-lowering-bundle/v1\",\n");
    text.push_str(&format!(
        "  \"profile\": {},\n  \"identity_scheme\": {},\n",
        json_string(llvm::PROFILE),
        json_string(TestIds.name())
    ));
    text.push_str(&revision_text());
    text.push_str(assumption_text());
    text.push_str(&target_text());
    text.push_str(concat!(
        "  \"pair\": {\n",
        "    \"original\": \"foldable\", \"rewritten\": \"foldable-rewritten\",\n",
        "    \"certificate\": \"pair/certificate.json\", \"relation\": \"equivalence\",\n",
        "    \"direction\": \"both\", \"request\": \"request.json\",\n",
        "    \"source_module\": \"pair/source.ll\", \"target_module\": \"pair/target.ll\"\n",
        "  },\n",
    ));
    text.push_str(&format!(
        "  \"bindings\": {{\n    \"certificate_identity\": {},\n    \
         \"request_identity\": {},\n    \"lean_corpus\": {},\n    \
         \"lean_corpus_identity\": {}\n  }},\n",
        json_string(&retained_identity(
            "noble.ngraph.retained-text.v1",
            "pair/certificate.json",
            &certificate
        )),
        json_string(&retained_identity(
            "noble.ngraph.retained-text.v1",
            "request.json",
            &request
        )),
        json_string("proofs/foundation/NobleFoundation/LlvmLoweringCorpus.lean"),
        json_string(&retained_identity(
            "noble.ngraph.retained-text.v1",
            "proofs/foundation/NobleFoundation/LlvmLoweringCorpus.lean",
            &corpus
        ))
    ));
    text.push_str("  \"source_bindings\": [\n");
    text.push_str(&source_binding_entries());
    text.push_str("\n  ],\n  \"artifacts\": [\n");
    let artifacts = all
        .iter()
        .map(artifact_entry)
        .collect::<Vec<_>>()
        .join(",\n");
    text.push_str(&artifacts);
    text.push_str("\n  ],\n  \"controls\": [\n");
    let controls = records
        .iter()
        .map(control_entry)
        .collect::<Vec<_>>()
        .join(",\n");
    text.push_str(&controls);
    text.push_str(concat!(
        "\n  ],\n  \"stages\": [\n",
        "    {\"stage\": \"graph-rewrite\", \"status\": \"accepted\", \"evidence\": \"cert::check accepts the retained derivation\"},\n",
        "    {\"stage\": \"graph-to-llvm-lowering\", \"status\": \"validated\", \"evidence\": \"llvm::validate on the retained bytes\"},\n",
        "    {\"stage\": \"semantic-validation\", \"status\": \"proved\", \"evidence\": \"LlvmLowering.validate_sound over all BitVec 64 input pairs\"},\n",
        "    {\"stage\": \"concrete-agreement\", \"status\": \"checked\", \"evidence\": \"parsed artifacts versus graph reference at the recorded finite inputs\"},\n",
        "    {\"stage\": \"llvm-optimisation\", \"status\": \"not-run\", \"evidence\": \"compiler-lab #124 owns it; no pinned LLVM here\"},\n",
        "    {\"stage\": \"native-artifact\", \"status\": \"not-run\", \"evidence\": \"compiler-lab #124 owns it; no pinned LLVM here\"}\n",
        "  ]\n}\n",
    ));
    text
}

pub(super) fn request_text() -> String {
    format!(
        "{{\n  \"schema\": 1,\n  \"kind\": \"compiler-lab-llvm-pair-request\",\n  \
         \"source_module\": {{\n    \"path\": \"pair/source.ll\",\n    \"entry\": {function},\n    \
         \"triple\": {triple},\n    \"data_layout\": {layout}\n  }},\n  \
         \"target_module\": {{\n    \"path\": \"pair/target.ll\",\n    \"entry\": {function},\n    \
         \"triple\": {triple},\n    \"data_layout\": {layout}\n  }},\n  \
         \"relation\": \"equivalence\",\n  \"direction\": \"both\",\n  \
         \"triples\": {{\n    \"source\": {triple},\n    \"target\": {triple}\n  }},\n  \
         \"data_layouts\": {{\n    \"source\": {layout},\n    \"target\": {layout}\n  }},\n  \
         \"arguments\": [\"i64\", \"i64\"],\n  \"result\": \"i64\",\n  \
         \"calling_convention\": {convention},\n  \"attributes\": [],\n  \
         \"input_domain\": \"all-u64-pairs\",\n  \"poison_policy\": \"exclude\",\n  \
         \"permitted_externals\": []\n}}\n",
        function = json_string(llvm::FUNCTION),
        triple = json_string(llvm::TARGET_TRIPLE),
        layout = json_string(llvm::DATA_LAYOUT),
        convention = json_string(llvm::CALLING_CONVENTION)
    )
}

fn pair_texts(all: &[Artifact]) -> (String, String) {
    let source = artifact_named(all, "foldable").text.clone();
    let target = artifact_named(all, "foldable-rewritten").text.clone();
    (source, target)
}

pub(super) fn retained_files(all: &[Artifact], records: &[ControlRecord]) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = all
        .iter()
        .map(|held| (format!("artifacts/{}.ll", held.name), held.text.clone()))
        .collect();
    files.extend(all.iter().map(|held| {
        (
            format!("artifacts/{}.graph", held.name),
            noble_ngraph::format::graph::print(&held.graph),
        )
    }));
    let (source, target) = pair_texts(all);
    files.push(("pair/source.ll".to_owned(), source));
    files.push(("pair/target.ll".to_owned(), target));
    files.push(("pair/certificate.json".to_owned(), certificate_text(all)));
    files.extend(records.iter().map(|record| {
        (
            control_path(record.artifact, record.class),
            record.text.clone(),
        )
    }));
    files
}
