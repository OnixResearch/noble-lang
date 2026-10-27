//! Shared reader for the frozen rewrite corpora (issue #64).
//!
//! Both the quality lane and the cost lane read the same fixture syntax and build the same graph, so
//! the syntax and the node vocabulary live here once instead of being re-implemented per lane. A
//! lane adds only what its own manifest declares (`rules` for quality, `threshold`/`stage` for cost).

#![allow(dead_code)]

use std::path::Path;

use noble_ngraph::ir::{Graph, GraphBuilder, NodeId, Op, SubProfile, Subject};
use noble_ngraph::IdentityScheme;

/// One frozen fixture as a manifest declares it.
pub struct Fixture {
    pub name: String,
    pub target: String,
    pub nodes: Vec<String>,
}

/// A pre-registered continuation threshold (issue #64 item 7).
pub struct Threshold {
    pub residual: String,
    pub check_per_step_us: f64,
}

/// One declared end-to-end stage of issue #64 item 5.
pub struct Stage {
    pub name: String,
    pub status: String,
    pub reason: String,
}

/// One manifest, whichever corpus it describes.
pub struct Manifest {
    pub schema: String,
    pub rules: Vec<String>,
    pub lens: String,
    pub corpus: String,
    pub threshold: Option<Threshold>,
    pub stages: Vec<Stage>,
    pub fixtures: Vec<Fixture>,
}

/// Parse a manifest and the fixtures it declares in the same file.
pub fn parse(path: &Path) -> Result<Manifest, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path)?;
    let mut schema = String::new();
    let mut rules = Vec::new();
    let mut lens = String::new();
    let mut corpus = String::new();
    let mut threshold = None;
    let mut stages = Vec::new();
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
        } else if let Some(rest) = line.strip_prefix("lens ") {
            lens = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("corpus ") {
            corpus = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("threshold ") {
            let mut residual = String::new();
            let mut check = 0.0_f64;
            for part in rest.split('|').map(str::trim) {
                if let Some(value) = part.strip_prefix("residual ") {
                    residual = value.trim().to_string();
                } else if let Some(value) = part.strip_prefix("check-per-step-us ") {
                    check = value.trim().parse()?;
                }
            }
            threshold = Some(Threshold {
                residual,
                check_per_step_us: check,
            });
        } else if let Some(rest) = line.strip_prefix("stage ") {
            let mut parts = rest.split('|').map(str::trim);
            let name = parts.next().ok_or("stage name")?.to_string();
            let status = parts
                .next()
                .and_then(|part| part.strip_prefix("status "))
                .ok_or("stage status")?
                .to_string();
            let reason = parts
                .next()
                .and_then(|part| part.strip_prefix("reason "))
                .ok_or("stage reason")?
                .to_string();
            stages.push(Stage {
                name,
                status,
                reason,
            });
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
        lens,
        corpus,
        threshold,
        stages,
        fixtures,
    })
}

/// Build one fixture's graph exactly as the manifest declares its nodes.
///
/// Node specifications are `const:<decimal>`, `add:<left>:<right>` and `mul:<left>:<right>`, where
/// the operands name earlier nodes by canonical creation index. The last node is the output.
pub fn build(ids: &dyn IdentityScheme, fixture: &Fixture) -> Result<Graph, Box<dyn std::error::Error>> {
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
