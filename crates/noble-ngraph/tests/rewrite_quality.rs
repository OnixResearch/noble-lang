//! Frozen optimization-quality corpus for certified graph rewrites (issue #64).
//!
//! Certificate validity and optimization coverage are measured separately: every proposer control
//! here emits a derivation the same validator checks, and the report says for each one whether the
//! result reached the fixture's declared residual target. A valid certificate over an unchanged
//! graph therefore fails quality acceptance rather than passing it.

use std::path::Path;

use noble_ngraph::cert::{self, Derivation, Proposer, Refusal};
use noble_ngraph::identity::{IdentityHasher, IdentityScheme};
use noble_ngraph::ir::{Digest, Graph, GraphBuilder, NodeId, Op, SubProfile, Subject};

struct TestHasher(u64);

impl IdentityHasher for TestHasher {
    fn update(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(byte));
        }
    }
    fn finish(self: Box<Self>) -> Digest {
        let mut digest = [0u8; 32];
        digest[..8].copy_from_slice(&self.0.to_le_bytes());
        digest
    }
}

struct TestIds;

impl IdentityScheme for TestIds {
    fn name(&self) -> &'static str {
        "test"
    }
    fn start(&self) -> Box<dyn IdentityHasher + '_> {
        Box::new(TestHasher(0))
    }
}

/// One frozen fixture as the manifest declares it.
struct Fixture {
    name: String,
    target: String,
    nodes: Vec<String>,
}

struct Manifest {
    schema: String,
    rules: Vec<String>,
    fixtures: Vec<Fixture>,
}

fn parse(path: &Path) -> Result<Manifest, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let mut schema = String::new();
    let mut rules = Vec::new();
    let mut fixtures = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("schema ") {
            schema = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("rules ") {
            rules = rest.split_whitespace().map(str::to_string).collect();
        } else if let Some(rest) = line.strip_prefix("fixture ") {
            let mut parts = rest.split('|').map(str::trim);
            let name = parts.next().ok_or("fixture name")?.to_string();
            let target = parts
                .next()
                .and_then(|part| part.strip_prefix("target "))
                .ok_or("fixture target")?
                .to_string();
            let nodes = parts
                .next()
                .and_then(|part| part.strip_prefix("nodes "))
                .ok_or("fixture nodes")?
                .split_whitespace()
                .map(str::to_string)
                .collect();
            fixtures.push(Fixture {
                name,
                target,
                nodes,
            });
        }
    }
    Ok(Manifest {
        schema,
        rules,
        fixtures,
    })
}

fn build(ids: &TestIds, fixture: &Fixture) -> Result<Graph, Box<dyn std::error::Error>> {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(ids, &std::format!("rewrite-quality-{}", fixture.name)),
    );
    let mut ids_by_index: Vec<NodeId> = Vec::new();
    for spec in &fixture.nodes {
        let mut parts = spec.split(':');
        let kind = parts.next().ok_or("node kind")?;
        let node = match kind {
            "const" => {
                let value: i64 = parts.next().ok_or("const value")?.parse()?;
                builder.i64_const(value)
            }
            "add" | "mul" => {
                let left: usize = parts.next().ok_or("left operand")?.parse()?;
                let right: usize = parts.next().ok_or("right operand")?.parse()?;
                let left = *ids_by_index.get(left).ok_or("earlier operand")?;
                let right = *ids_by_index.get(right).ok_or("earlier operand")?;
                if kind == "add" {
                    builder.op(Op::I64Add(left, right))
                } else {
                    builder.op(Op::I64Mul(left, right))
                }
            }
            other => return Err(std::format!("unknown node kind {other}").into()),
        };
        ids_by_index.push(node);
    }
    builder.output(*ids_by_index.last().ok_or("fixture output")?);
    Ok(builder.finish())
}

/// One measured row: certificate verdict, residual sites and whether the target was reached.
struct Row {
    verdict: Result<(), Refusal>,
    residual: usize,
    target_reached: bool,
}

fn measure(ids: &TestIds, graph: &Graph, proposer: Proposer) -> Row {
    let (derivation, result): (Derivation, Graph) = cert::propose(ids, proposer, graph);
    let verdict = cert::check(ids, graph, &result, &derivation);
    let residual = cert::residual_fold_sites(&result);
    Row {
        verdict,
        residual,
        target_reached: residual == 0,
    }
}

#[test]
fn quality_acceptance_is_separate_from_certificate_validity(
) -> Result<(), Box<dyn std::error::Error>> {
    let ids = TestIds;
    let manifest = parse(Path::new(concat!(
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

    let mut valid_but_unoptimized = 0_usize;
    for fixture in &manifest.fixtures {
        assert_eq!(fixture.target, "no-foldable-sites");
        let graph = build(&ids, fixture)?;
        let sites = cert::residual_fold_sites(&graph);
        assert!(sites > 0, "{} declares nothing to optimize", fixture.name);

        let identity = measure(&ids, &graph, Proposer::Identity);
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

        let first = measure(&ids, &graph, Proposer::FirstEligible);
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

        let single = measure(&ids, &graph, Proposer::SinglePass);
        assert_eq!(single.verdict, Ok(()), "{}", fixture.name);
        let fixed = measure(&ids, &graph, Proposer::FixedPoint);
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
