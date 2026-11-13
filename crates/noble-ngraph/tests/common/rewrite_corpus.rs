//! Shared reader for the frozen rewrite corpora (issue #64).
//!
//! Both the quality lane and the cost lane read the same fixture syntax and build the same graph, so
//! the syntax and the node vocabulary live here once instead of being re-implemented per lane. A
//! lane adds only what its own manifest declares (`rules` for quality, `threshold`/`stage` for cost).

#![allow(dead_code)]
#![allow(
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this shared integration-test module keeps the frozen rewrite_corpus domain path across its independent quality and cost consumers."
)]

use std::path::Path;

use noble_ngraph::ir::{Graph, GraphBuilder, NodeId, Op, SubProfile, Subject};
use noble_ngraph::IdentityScheme;

/// One frozen fixture as a manifest declares it.
pub struct Fixture {
    pub name: String,
    pub target: String,
    pub nodes: Vec<String>,
    /// Explicit ordered output node indices in quality corpus v2. `None` retains v1's last-node
    /// output rule; v2 requires this list and permits repeated indices.
    pub outputs: Option<Vec<usize>>,
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
    let line_count = text.lines().count();
    let mut schema = String::new();
    let mut rules = Vec::new();
    let mut lens = String::new();
    let mut corpus = String::new();
    let mut threshold = None;
    let mut stages = Vec::with_capacity(line_count);
    let mut fixtures = Vec::with_capacity(line_count);
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((kind, rest)) = line.split_once(' ') else {
            continue;
        };
        match kind {
            "schema" => schema = rest.trim().to_string(),
            "rules" => rules = rest.split_whitespace().map(str::to_string).collect(),
            "lens" => lens = rest.trim().to_string(),
            "corpus" => corpus = rest.trim().to_string(),
            "threshold" => threshold = Some(parse_threshold(rest)?),
            "stage" => stages.push(parse_stage(rest)?),
            "fixture" => fixtures.push(parse_fixture(rest)?),
            _ => {}
        }
    }
    match schema.as_str() {
        "noble-rewrite-quality/v1" if fixtures.iter().any(|fixture| fixture.outputs.is_some()) => {
            return Err("quality v1 fixtures use the implicit last-node output".into());
        }
        "noble-rewrite-quality/v2" if fixtures.iter().any(|fixture| fixture.outputs.is_none()) => {
            return Err("quality v2 fixtures require explicit outputs".into());
        }
        _ => {}
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

fn parse_threshold(rest: &str) -> Result<Threshold, Box<dyn std::error::Error>> {
    let mut residual = String::new();
    let mut check = 0.0_f64;
    for part in rest.split('|').map(str::trim) {
        if let Some(value) = part.strip_prefix("residual ") {
            residual = value.trim().to_string();
        } else if let Some(value) = part.strip_prefix("check-per-step-us ") {
            check = value.trim().parse()?;
        }
    }
    Ok(Threshold {
        residual,
        check_per_step_us: check,
    })
}

fn parse_stage(rest: &str) -> Result<Stage, Box<dyn std::error::Error>> {
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
    Ok(Stage {
        name,
        status,
        reason,
    })
}

fn parse_fixture(rest: &str) -> Result<Fixture, Box<dyn std::error::Error>> {
    let mut parts = rest.split('|').map(str::trim);
    let name = parts.next().ok_or("fixture name")?.to_string();
    let target = parts
        .next()
        .and_then(|part| part.strip_prefix("target "))
        .ok_or("fixture target")?
        .to_string();
    let mut outputs = None;
    let mut node_specs = None;
    for part in parts {
        if let Some(list) = part.strip_prefix("outputs ") {
            if outputs.is_some() {
                return Err("duplicate fixture outputs".into());
            }
            outputs = Some(
                list.split(',')
                    .map(|index| index.trim().parse::<usize>().map_err(Into::into))
                    .collect::<Result<Vec<_>, Box<dyn std::error::Error>>>()?,
            );
        } else if let Some(nodes) = part.strip_prefix("nodes ") {
            if node_specs.is_some() {
                return Err("duplicate fixture nodes".into());
            }
            node_specs = Some(nodes);
        } else {
            return Err(std::format!("unknown fixture clause: {part}").into());
        }
    }
    let nodes: Vec<String> = node_specs
        .ok_or("fixture nodes")?
        .split_whitespace()
        .map(str::to_string)
        .collect();
    if outputs.as_ref().is_some_and(|indices| {
        indices.is_empty() || indices.iter().any(|index| *index >= nodes.len())
    }) {
        return Err("fixture output index is empty or out of range".into());
    }
    Ok(Fixture {
        name,
        target,
        nodes,
        outputs,
    })
}

/// Build one fixture's graph exactly as the manifest declares its nodes.
///
/// Node specifications are `const:<decimal>`, `add:<left>:<right>` and `mul:<left>:<right>`, where
/// operands name earlier nodes by canonical creation index. Quality v1 uses the last node as the
/// sole output; quality v2 names an ordered output list, including permitted repetitions.
pub fn build(
    ids: &dyn IdentityScheme,
    fixture: &Fixture,
) -> Result<Graph, Box<dyn std::error::Error>> {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(ids, &std::format!("rewrite-quality-{}", fixture.name)),
    );
    let mut ids_by_index: Vec<NodeId> = Vec::with_capacity(fixture.nodes.len());
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
    match &fixture.outputs {
        Some(outputs) => {
            for output in outputs {
                builder.output(*ids_by_index.get(*output).ok_or("fixture output index")?);
            }
        }
        None => builder.output(*ids_by_index.last().ok_or("fixture output")?),
    }
    Ok(builder.finish())
}
