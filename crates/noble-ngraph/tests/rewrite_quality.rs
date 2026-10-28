#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::assertion_density,
    tigerstyle::ambiguous_params,
    tigerstyle::fallible_int_fallback,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; this integration harness reads frozen fixtures and prices their measured costs, so terse fixture names, unwraps and long setup legs are the readable form."
)]
mod common;

use std::path::Path;

use common::rewrite_corpus;
use common::IDS;
use noble_ngraph::cert::{self, Derivation, Proposer, Refusal};

/// One measured row: certificate verdict, residual sites and whether the target was reached.
struct Row {
    verdict: Result<(), Refusal>,
    residual: usize,
    target_reached: bool,
}

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

#[test]
fn quality_acceptance_is_separate_from_certificate_validity(
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = rewrite_corpus::parse(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../verification/ngraph/rewrite-quality-v1.txt"
    )))?;
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

    let mut valid_but_unoptimized = 0_usize;
    for fixture in &manifest.fixtures {
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
        valid_but_unoptimized += 1;

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
        if single.residual == 0 {
            assert_eq!(fixed.verdict, Ok(()), "{}", fixture.name);
            assert_eq!(fixed.residual, 0, "{}", fixture.name);
        } else {
            // A chained opportunity: the certificate format binds every premise to the original
            // graph, so the step that would fold a node produced by an earlier step is refused as
            // malformed instead of being silently accepted. That is an unsupported-derivation class,
            // never a proof of optimization.
            assert_eq!(fixed.verdict, Err(Refusal::Malformed), "{}", fixture.name);
            assert_eq!(
                fixed.residual, 0,
                "{}: the rewrites were possible",
                fixture.name
            );
        }
    }
    assert!(
        valid_but_unoptimized >= 2,
        "the corpus must show valid-but-unoptimized controls: {valid_but_unoptimized}"
    );
    Ok(())
}
