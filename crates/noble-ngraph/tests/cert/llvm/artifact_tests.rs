use super::*;

#[test]
fn the_retained_pair_is_exported_and_bound() {
    let everything = artifacts();
    let foldable = artifact_named(&everything, "foldable");
    let rewritten = artifact_named(&everything, "foldable-rewritten");
    let (derivation, endpoint) = noble_ngraph::cert::emit(&TestIds, &foldable.graph);
    assert_eq!(
        endpoint, rewritten.graph,
        "the retained rewritten endpoint is the certificate's own output"
    );
    assert_eq!(
        noble_ngraph::cert::check(&TestIds, &foldable.graph, &rewritten.graph, &derivation),
        Ok(()),
        "the retained pair is accepted under its derivation"
    );
    for inputs in EXECUTIONS {
        let original = executed(foldable, inputs);
        let rewritten_value = executed(rewritten, inputs);
        assert_eq!(
            original, rewritten_value,
            "the two lowerings agree at {inputs:?}"
        );
    }
}

#[test]
fn the_lean_corpus_is_the_real_export() {
    let everything = artifacts();
    let records = control_records(&everything);
    let corpus = read_repo_file("proofs/foundation/NobleFoundation/LlvmLoweringCorpus.lean");
    assert_eq!(
        corpus_text(&everything, &records),
        corpus,
        "the exported artifacts changed: regenerate LlvmLoweringCorpus.lean with \
         `cargo test -p noble-ngraph --test cert_differential -- --ignored write_the_artifacts` and \
         rerun the foundation lane"
    );
}

#[test]
fn the_retained_bundle_is_the_real_export() {
    let everything = artifacts();
    let records = control_records(&everything);
    let bundle = read_repo_file("verification/llvm-lowering/bundle.json");
    assert_eq!(
        bundle_text(&everything, &records),
        bundle,
        "the retained bundle changed: regenerate it with \
         `cargo test -p noble-ngraph --test cert_differential -- --ignored write_the_artifacts`"
    );
    assert_eq!(
        request_text(),
        read_repo_file("verification/llvm-lowering/request.json"),
        "the compiler-lab request changed: regenerate it with the same command"
    );
    for (name, body) in retained_files(&everything, &records) {
        let path = format!("{LANE}/{name}");
        let retained = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("the retained file {path} is readable: {error}"));
        assert_eq!(retained, body, "the retained file {path} is current");
    }
}

#[test]
fn fresh_process_replays_retained_bindings_and_stages() {
    let output = std::process::Command::new("node")
        .arg(format!("{LANE}/replay.mjs"))
        .output()
        .expect("the pinned Node executable starts a fresh replay process");
    assert!(
        output.status.success(),
        "fresh replay succeeds\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("6 graph/module bindings"));
    assert!(stdout.contains("semantic validation and concrete agreement remain separate"));
}

#[test]
#[ignore = "writes the generated Lean corpus and the retained bundle; run after a deliberate change"]
fn write_the_artifacts() {
    let everything = artifacts();
    let records = control_records(&everything);
    std::fs::create_dir_all(LANE).expect("the retained lane directory exists");
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proofs/foundation/NobleFoundation/LlvmLoweringCorpus.lean"
        ),
        corpus_text(&everything, &records),
    )
    .expect("the generated corpus is written");
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../verification/llvm-lowering/bundle.json"
        ),
        bundle_text(&everything, &records),
    )
    .expect("the retained bundle is written");
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../verification/llvm-lowering/request.json"
        ),
        request_text(),
    )
    .expect("the compiler-lab request is written");
    for (name, body) in retained_files(&everything, &records) {
        let path = format!("{LANE}/{name}");
        if let Some(parent) = std::path::Path::new(&path).parent() {
            std::fs::create_dir_all(parent).expect("the artifact directory exists");
        }
        std::fs::write(&path, body).expect("the retained file is written");
    }
}
