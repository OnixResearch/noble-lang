use std::path::Path;

use super::common::rewrite_corpus;
use super::common::IDS;
use noble_ngraph::cert::{self, Derivation, Proposer, Refusal};

/// One measured row: certificate verdict, residual sites and whether the target was reached.
pub(super) struct Row {
    verdict: Result<(), Refusal>,
    residual: usize,
    target_reached: bool,
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this runtime helper invokes graph proposal and certificate checking."
)]
fn measure(graph: &noble_ngraph::ir::Graph, proposer: Proposer) -> Row {
    let (derivation, result): (Derivation, noble_ngraph::ir::Graph) =
        cert::propose(&IDS, proposer, graph);
    let verdict = cert::check(&IDS, graph, &result, &derivation);
    let residual = cert::residual_fold_sites(&result);
    Row {
        verdict,
        residual,
        target_reached: residual == 0,
    }
}

/// The reviewed corpus, in manifest order, with each fixture's measured number of foldable
/// sites. Pinning both binds the fixtures' *content*, not just their names: a fixture that
/// disappears, is renamed, is relabelled unsupported, or has its declared nodes weakened changes
/// this row and fails here, so coverage cannot improve by editing the manifest alone.
const REVIEWED_FIXTURES: [(&str, usize); 7] = [
    ("independent-folds", 2),
    ("single-fold", 1),
    ("chained-folds", 1),
    ("ordered-outputs", 2),
    ("shared-operand", 1),
    ("wrapping-boundary", 1),
    ("operand-order", 1),
];

pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let manifest = rewrite_corpus::parse(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/ngraph/rewrite-quality-v1.txt"
    )))?;
    assert_manifest(&manifest);
    let mut valid_but_unoptimized = 0_usize;
    for fixture in &manifest.fixtures {
        valid_but_unoptimized += verify_fixture(fixture)?;
    }
    assert!(
        valid_but_unoptimized >= 2,
        "the corpus must show valid-but-unoptimized controls: {valid_but_unoptimized}"
    );
    Ok(())
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; manifest validation allocates the expected rule strings and compares runtime String and collected Vec values, whose PartialEq implementations are not const."
)]
fn assert_manifest(manifest: &rewrite_corpus::Manifest) {
    assert_eq!(manifest.schema, "noble-rewrite-quality/v1");
    assert_eq!(
        manifest.rules,
        vec![
            "fold-add-constants".to_string(),
            "neutral-add-left".to_string(),
            "neutral-mul-right".to_string()
        ],
        "the manifest binds the rules it exercises"
    );
    assert!(!manifest.fixtures.is_empty(), "the corpus is not empty");
    // The corpus itself is frozen: declared order and measured sites must match the reviewed
    // rows, so a fixture cannot silently disappear or be weakened to improve coverage.
    assert_eq!(
        manifest
            .fixtures
            .iter()
            .map(|fixture| fixture.name.as_str())
            .collect::<Vec<_>>(),
        REVIEWED_FIXTURES
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
        "the reviewed fixture set changed"
    );
}

fn verify_fixture(fixture: &rewrite_corpus::Fixture) -> Result<usize, Box<dyn std::error::Error>> {
    assert_eq!(fixture.target, "no-foldable-sites");
    let graph = rewrite_corpus::build(&IDS, fixture)?;
    let sites = cert::residual_fold_sites(&graph);
    assert!(sites > 0, "{} declares nothing to optimize", fixture.name);
    let reviewed = REVIEWED_FIXTURES
        .iter()
        .find(|(name, _)| *name == fixture.name)
        .map(|(_, sites)| *sites)
        .ok_or_else(|| std::format!("{} is not a reviewed fixture", fixture.name))?;
    assert_eq!(
        sites, reviewed,
        "{} declares {} foldable sites, the reviewed corpus declares {reviewed}",
        fixture.name, sites
    );

    let identity = measure(&graph, Proposer::Identity);
    assert_eq!(
        identity.verdict,
        Ok(()),
        "{}: the identity proposer emits a valid certificate over identical endpoints",
        fixture.name
    );
    assert_eq!(
        identity.residual, sites,
        "{}: nothing was rewritten",
        fixture.name
    );
    assert!(
        !identity.target_reached,
        "{}: correctness alone must not satisfy a quality target",
        fixture.name
    );
    let mut valid_but_unoptimized = 1;

    let first = measure(&graph, Proposer::FirstEligible);
    assert_eq!(first.verdict, Ok(()), "{}", fixture.name);
    // One fold was applied. A chained fixture can then *gain* a site (the node whose operand
    // became constant), so the residual is never below the untouched original count minus one
    // and is allowed to be higher.
    assert!(
        first.residual >= sites.saturating_sub(1),
        "{}: at most one fold was applied",
        fixture.name
    );
    if sites > 1 {
        assert!(
            !first.target_reached,
            "{}: a first-rewrite-only proposer leaves sites behind",
            fixture.name
        );
        valid_but_unoptimized += 1;
    }

    let single = measure(&graph, Proposer::SinglePass);
    assert_eq!(single.verdict, Ok(()), "{}", fixture.name);
    let fixed = measure(&graph, Proposer::FixedPoint);
    assert_fixed_point(&single, &fixed, &fixture.name);
    Ok(valid_but_unoptimized)
}

pub(super) fn assert_fixed_point(single: &Row, fixed: &Row, fixture: &str) {
    if single.residual == 0 {
        assert_eq!(fixed.verdict, Ok(()), "{}", fixture);
        assert_eq!(fixed.residual, 0, "{}", fixture);
    } else {
        // A chained opportunity: the certificate format binds every premise to the original
        // graph, so the step that would fold a node produced by an earlier step is refused as
        // malformed instead of being silently accepted. That is an unsupported-derivation class,
        // never a proof of optimization.
        assert_eq!(fixed.verdict, Err(Refusal::Malformed), "{}", fixture);
        assert_eq!(fixed.residual, 0, "{}: the rewrites were possible", fixture);
    }
}
