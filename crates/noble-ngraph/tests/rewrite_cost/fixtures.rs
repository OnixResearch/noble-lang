use std::path::Path;

use super::common::rewrite_corpus;

pub(super) fn load(
) -> Result<(rewrite_corpus::Manifest, rewrite_corpus::Manifest), Box<dyn std::error::Error>> {
    let manifest = rewrite_corpus::parse(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/ngraph/rewrite-cost-v2.txt"
    )))?;
    validate_cost_manifest(&manifest)?;
    let corpus = rewrite_corpus::parse(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/ngraph/rewrite-quality-v1.txt"
    )))?;
    assert!(
        !corpus.fixtures.is_empty(),
        "the corpus named by the cost manifest is not empty"
    );
    Ok((manifest, corpus))
}

fn validate_cost_manifest(
    manifest: &rewrite_corpus::Manifest,
) -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(manifest.schema, "noble-rewrite-cost/v2");
    assert_eq!(manifest.corpus, "rewrite-quality-v1.txt");
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
