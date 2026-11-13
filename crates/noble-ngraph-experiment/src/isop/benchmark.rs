use super::{
    optimize_interval, verify_interval, BoolNode, BooleanDag, Cover, Error, Limits, Optimization,
};
use crate::bench::warm;
use crate::json::Json;
use std::hint::black_box;
use std::time::Instant;

const SAMPLES: usize = 30;

struct Workload {
    name: &'static str,
    graph: BooleanDag,
}

fn dag(atoms: usize, nodes: Vec<BoolNode>, root: usize) -> Result<BooleanDag, Error> {
    BooleanDag::try_new(atoms, nodes, root)
}

fn corpus() -> Result<Vec<Workload>, Error> {
    Ok(vec![
        Workload {
            name: "absorption-2-atom",
            graph: dag(
                2,
                vec![
                    BoolNode::Atom(0),
                    BoolNode::Atom(1),
                    BoolNode::And(0, 1),
                    BoolNode::Not(1),
                    BoolNode::And(0, 3),
                    BoolNode::Or(2, 4),
                ],
                5,
            )?,
        },
        Workload {
            name: "absorption-6-atom-interface",
            graph: dag(
                6,
                vec![
                    BoolNode::Atom(0),
                    BoolNode::Atom(1),
                    BoolNode::And(0, 1),
                    BoolNode::Not(1),
                    BoolNode::And(0, 3),
                    BoolNode::Or(2, 4),
                ],
                5,
            )?,
        },
        Workload {
            name: "parity-4",
            graph: dag(
                4,
                vec![
                    BoolNode::Atom(0),
                    BoolNode::Atom(1),
                    BoolNode::Atom(2),
                    BoolNode::Atom(3),
                    BoolNode::Or(0, 1),
                    BoolNode::And(0, 1),
                    BoolNode::Not(5),
                    BoolNode::And(4, 6),
                    BoolNode::Or(7, 2),
                    BoolNode::And(7, 2),
                    BoolNode::Not(9),
                    BoolNode::And(8, 10),
                    BoolNode::Or(11, 3),
                    BoolNode::And(11, 3),
                    BoolNode::Not(13),
                    BoolNode::And(12, 14),
                ],
                15,
            )?,
        },
        Workload {
            name: "shared-factor-3-atom",
            graph: dag(
                3,
                vec![
                    BoolNode::Atom(0),
                    BoolNode::Atom(1),
                    BoolNode::Or(0, 1),
                    BoolNode::Atom(2),
                    BoolNode::And(2, 3),
                    BoolNode::Not(3),
                    BoolNode::And(2, 5),
                    BoolNode::Or(4, 6),
                ],
                7,
            )?,
        },
        Workload {
            name: "constants-repeated-atom-3-atom",
            graph: dag(
                3,
                vec![
                    BoolNode::Const(true),
                    BoolNode::Atom(0),
                    BoolNode::And(1, 1),
                    BoolNode::Const(false),
                    BoolNode::And(0, 2),
                    BoolNode::Or(4, 3),
                ],
                5,
            )?,
        },
    ])
}

fn folded(graph: &BooleanDag) -> Result<BooleanDag, Error> {
    let mut nodes = Vec::with_capacity(graph.nodes.len());
    let mut remap = Vec::with_capacity(graph.nodes.len());
    for node in &graph.nodes {
        let remapped = |index: u16| remap[usize::from(index)];
        let next = match *node {
            BoolNode::Const(value) => push(&mut nodes, BoolNode::Const(value)),
            BoolNode::Atom(slot) => push(&mut nodes, BoolNode::Atom(slot)),
            BoolNode::Not(input) => {
                let input = remapped(input);
                match nodes[usize::from(input)] {
                    BoolNode::Const(value) => push(&mut nodes, BoolNode::Const(!value)),
                    BoolNode::Not(inner) => inner,
                    _ => push(&mut nodes, BoolNode::Not(input)),
                }
            }
            BoolNode::And(left, right) => {
                let left = remapped(left);
                let right = remapped(right);
                match (nodes[usize::from(left)], nodes[usize::from(right)]) {
                    (BoolNode::Const(false), _) | (_, BoolNode::Const(false)) => {
                        push(&mut nodes, BoolNode::Const(false))
                    }
                    (BoolNode::Const(true), _) => right,
                    (_, BoolNode::Const(true)) => left,
                    _ if left == right => left,
                    _ => push(&mut nodes, BoolNode::And(left, right)),
                }
            }
            BoolNode::Or(left, right) => {
                let left = remapped(left);
                let right = remapped(right);
                match (nodes[usize::from(left)], nodes[usize::from(right)]) {
                    (BoolNode::Const(true), _) | (_, BoolNode::Const(true)) => {
                        push(&mut nodes, BoolNode::Const(true))
                    }
                    (BoolNode::Const(false), _) => right,
                    (_, BoolNode::Const(false)) => left,
                    _ if left == right => left,
                    _ => push(&mut nodes, BoolNode::Or(left, right)),
                }
            }
        };
        remap.push(next);
    }
    BooleanDag::try_new(
        graph.atom_count(),
        nodes,
        usize::from(remap[usize::from(graph.root)]),
    )
}

fn push(nodes: &mut Vec<BoolNode>, node: BoolNode) -> u16 {
    let index = nodes.len() as u16;
    nodes.push(node);
    index
}

fn literals(cover: &Cover) -> usize {
    cover
        .cubes
        .iter()
        .map(|cube| cube.care.count_ones() as usize)
        .sum()
}

fn measure(workload: &Workload, samples: usize) -> Result<Json, String> {
    let graph = &workload.graph;
    let folded_graph = folded(graph).map_err(|e| format!("{} folding: {e:?}", workload.name))?;
    let cover = match optimize_interval(graph, graph, Limits::default())
        .map_err(|e| format!("{} ISOP: {e:?}", workload.name))?
    {
        Optimization::Optimized(cover) => cover,
        Optimization::Fallback(reason) => {
            return Err(format!("{} ISOP fallback: {reason:?}", workload.name));
        }
    };
    if !verify_interval(graph, graph, &cover)
        .map_err(|e| format!("{} checker: {e:?}", workload.name))?
    {
        return Err(format!(
            "{} ISOP candidate failed independent check",
            workload.name
        ));
    }
    let rows = 1usize << graph.atom_count;
    assert!(rows != 0);
    for assignment in 0..rows {
        let source = super::evaluate_topological(graph, assignment as u8);
        if source != super::check_dag(&folded_graph, assignment as u8)
            || source != cover.evaluate(assignment as u8)
        {
            return Err(format!(
                "{} arm disagreement at row {assignment}",
                workload.name
            ));
        }
    }

    let started = Instant::now();
    let mut no_pass_row = 0usize;
    let (no_pass, no_pass_median, _) = warm(samples, || {
        let assignment = (no_pass_row % rows) as u8;
        no_pass_row += 1;
        black_box(super::evaluate_topological(graph, black_box(assignment)));
    });
    let no_pass_wall = started.elapsed().as_secs_f64();

    let started = Instant::now();
    let mut folding_row = 0usize;
    let (folded_eval, folded_eval_median, _) = warm(samples, || {
        let assignment = (folding_row % rows) as u8;
        folding_row += 1;
        black_box(super::evaluate_topological(
            &folded_graph,
            black_box(assignment),
        ));
    });
    let folded_eval_wall = started.elapsed().as_secs_f64();

    let (fold_build, fold_build_median, _) = warm(samples, || {
        black_box(folded(graph).expect("valid frozen folding corpus"));
    });
    let mut isop_build_failure = None;
    let (isop_build, isop_build_median, _) = warm(samples, || {
        if isop_build_failure.is_some() {
            return;
        }
        match optimize_interval(graph, graph, Limits::default()) {
            Ok(Optimization::Optimized(candidate)) => {
                black_box(candidate);
            }
            Ok(Optimization::Fallback(reason)) => {
                isop_build_failure = Some(format!("fallback: {reason:?}"));
            }
            Err(error) => {
                isop_build_failure = Some(format!("error: {error:?}"));
            }
        }
    });
    if let Some(failure) = isop_build_failure {
        return Err(format!("{} ISOP build {failure}", workload.name));
    }
    let (isop_check, isop_check_median, _) = warm(samples, || {
        black_box(verify_interval(graph, graph, &cover).expect("valid frozen cover"));
    });

    let mut isop_row = 0usize;
    let (isop_eval, isop_eval_median, _) = warm(samples, || {
        let assignment = (isop_row % rows) as u8;
        isop_row += 1;
        black_box(cover.evaluate(black_box(assignment)));
    });
    let (cube_count, literal_count) = (cover.len(), literals(&cover));
    let cover_storage_bytes = std::mem::size_of::<super::Cover>()
        + cover.cubes.capacity() * std::mem::size_of::<super::Cube>();
    Ok(Json::obj([
        ("workload", Json::s(workload.name)),
        ("atoms", Json::U(graph.atom_count() as u64)),
        ("rows", Json::U(rows as u64)),
        ("dag_nodes", Json::U(graph.node_count() as u64)),
        ("folded_nodes", Json::U(folded_graph.node_count() as u64)),
        ("cube_count", Json::U(cube_count as u64)),
        ("literal_count", Json::U(literal_count as u64)),
        ("cover_storage_bytes", Json::U(cover_storage_bytes as u64)),
        ("independent_check_passed", Json::Bool(true)),
        ("no_pass_eval", no_pass),
        ("no_pass_eval_median_ns", Json::F(no_pass_median)),
        ("no_pass_arm_wall_seconds", Json::F(no_pass_wall)),
        ("folding_build", fold_build),
        ("folding_build_median_ns", Json::F(fold_build_median)),
        ("folding_eval", folded_eval),
        ("folding_eval_median_ns", Json::F(folded_eval_median)),
        ("folding_eval_arm_wall_seconds", Json::F(folded_eval_wall)),
        ("isop_build", isop_build),
        ("isop_build_median_ns", Json::F(isop_build_median)),
        ("isop_independent_check", isop_check),
        (
            "isop_independent_check_median_ns",
            Json::F(isop_check_median),
        ),
        ("isop_eval", isop_eval),
        ("isop_eval_median_ns", Json::F(isop_eval_median)),
    ]))
}

/// Runs the frozen five-workload synthetic comparison with separate analysis,
/// independent-checking and warm-evaluation measurements.
pub fn run_benchmark(samples: usize) -> Result<Json, String> {
    if samples != SAMPLES {
        return Err(format!(
            "the frozen benchmark requires exactly {SAMPLES} samples per arm"
        ));
    }
    let workloads = corpus().map_err(|e| format!("invalid frozen corpus: {e:?}"))?;
    let runs = workloads
        .iter()
        .map(|workload| measure(workload, samples))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json::obj([
        ("schema", Json::s("noble-isop-synthetic-benchmark/v1")),
        ("samples_per_arm", Json::U(samples as u64)),
        ("calibrated_batch_minimum_ns", Json::U(20_000)),
        (
            "arms",
            Json::s("no-pass, folding-only, interval-isop plus independent checker"),
        ),
        (
            "note",
            Json::s("Synthetic Boolean DAGs only; no source or generated-code lowering exists."),
        ),
        ("workloads", Json::A(runs)),
    ]))
}
