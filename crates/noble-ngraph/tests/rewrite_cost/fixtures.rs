use std::path::Path;

use super::common::rewrite_corpus;

/// The inherited v2 output vectors and node declarations are fixed independently of each run.
const REVIEWED_FIXTURES: [(&str, &[&str], &[usize]); 8] = [
    (
        "independent-folds",
        &[
            "const:5", "const:7", "add:0:1", "const:3", "const:4", "add:3:4",
        ],
        &[2, 5],
    ),
    ("single-fold", &["const:5", "const:7", "add:0:1"], &[2]),
    (
        "chained-folds",
        &["const:5", "const:7", "add:0:1", "const:3", "add:2:3"],
        &[4],
    ),
    (
        "ordered-outputs",
        &["const:2", "const:3", "add:0:1", "const:10", "add:0:3"],
        &[4, 2],
    ),
    (
        "shared-operand",
        &["const:5", "const:1", "add:0:1", "mul:2:0"],
        &[3],
    ),
    (
        "wrapping-boundary",
        &["const:9223372036854775807", "const:1", "add:0:1"],
        &[2],
    ),
    ("operand-order", &["const:5", "const:7", "add:1:0"], &[2]),
    (
        "repeated-outputs",
        &["const:5", "const:7", "add:0:1"],
        &[2, 2],
    ),
];

pub(super) fn load(
) -> Result<(rewrite_corpus::Manifest, rewrite_corpus::Manifest), Box<dyn std::error::Error>> {
    let manifest = rewrite_corpus::parse(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/ngraph/rewrite-cost-v4.txt"
    )))?;
    validate_cost_manifest(&manifest)?;
    let corpus = rewrite_corpus::parse(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/ngraph/rewrite-quality-v2.txt"
    )))?;
    validate_quality_corpus(&corpus)?;
    Ok((manifest, corpus))
}

fn validate_cost_manifest(
    manifest: &rewrite_corpus::Manifest,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(manifest.schema, "noble-rewrite-cost/v4");
    assert_eq!(manifest.corpus, "rewrite-quality-v2.txt");
    let threshold = manifest.threshold.as_ref().ok_or("threshold")?;
    assert_eq!(threshold.residual, "no-foldable-sites");
    assert!(
        threshold.check_per_step_us > 0.0,
        "the continuation threshold is pre-registered, not discovered"
    );
    assert!(
        manifest
            .stages
            .iter()
            .any(|stage| stage.status == "measured"),
        "some stage is observed"
    );
    assert!(
        manifest
            .stages
            .iter()
            .filter(|stage| stage.status == "unmeasured")
            .all(|stage| !stage.reason.is_empty()),
        "an unmeasured stage always states why"
    );
    assert!(
        manifest
            .stages
            .iter()
            .all(|stage| stage.status == "measured" || stage.status == "unmeasured"),
        "a stage is measured or unmeasured, never assumed zero"
    );
    Ok(())
}

fn validate_quality_corpus(
    corpus: &rewrite_corpus::Manifest,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(corpus.schema, "noble-rewrite-quality/v2");
    assert_eq!(corpus.fixtures.len(), REVIEWED_FIXTURES.len());
    for (fixture, (name, nodes, outputs)) in corpus.fixtures.iter().zip(REVIEWED_FIXTURES) {
        assert_eq!(fixture.name, name, "reviewed v2 fixture order changed");
        assert_eq!(
            fixture.target, "no-foldable-sites",
            "{name}: target changed"
        );
        assert_eq!(
            fixture.nodes,
            nodes
                .iter()
                .map(|node| (*node).to_string())
                .collect::<Vec<_>>(),
            "{name}: reviewed node declarations changed"
        );
        assert_eq!(
            fixture.outputs.as_deref(),
            Some(outputs),
            "{name}: outputs changed"
        );
    }
    Ok(())
}
