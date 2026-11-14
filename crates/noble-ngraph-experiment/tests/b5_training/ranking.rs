#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the experiment harness serializes a fixed deterministic corpus and checks its independent rows."
)]

use noble_contracts::source::chunks::{
    self as chunks, Binding, Budget, Capture, Envelope, Fixture, FixtureSet, Instantiation,
    Interface, Library, Node, Op, Recipe, Request, Scalar, Task,
};
use noble_contracts::source::editor::{Candidate as EditorCandidate, Node as EditorNode};
use noble_kernel::types::{EffSet, Ty};
use serde_json::{json, Value};

const FAMILY_NAMES: [&str; 5] = [
    "arithmetic",
    "ordered-stack-routing",
    "boolean",
    "nested-quotation",
    "equal-signature-contrast",
];
const FIXTURE_CASES: usize = 4;

#[derive(Clone, Debug)]
struct TaskCase {
    inputs: Vec<Scalar>,
    outputs: Vec<Scalar>,
}

#[derive(Clone, Debug)]
struct TaskSpec {
    family: usize,
    block: u8,
    within_block: u8,
    stack_in: Vec<Ty>,
    stack_out: Vec<Ty>,
    cases: Vec<TaskCase>,
    primitive_source: Vec<u8>,
    target: Target,
}

#[derive(Clone, Copy, Debug)]
enum Target {
    Arithmetic(Op),
    Swap,
    Boolean(u8),
    Integer(i64),
}

impl TaskSpec {
    fn id(&self) -> String {
        format!("{}/b{:02}/t{:02}", FAMILY_NAMES[self.family], self.block, self.within_block)
    }

    fn split(&self) -> &'static str {
        match self.block {
            0..=6 => "train",
            7..=8 => "validation",
            _ => "test",
        }
    }
}

fn task_spec(family: usize, block: u8, within_block: u8) -> TaskSpec {
    let case_seed = i64::try_from(family.saturating_add(1)).unwrap_or(0) * 1_000_003
        + i64::from(block) * 8_191
        + i64::from(within_block) * 257;
    let (stack_in, stack_out, target, primitive_source, cases) = match family {
        0 => {
            let operation = (block + within_block) % 3;
            let op = [Op::Add, Op::Sub, Op::Mul][usize::from(operation)];
            let source = op.spelling().as_bytes().to_vec();
            let cases = (0..FIXTURE_CASES)
                .map(|case| {
                    let delta = i64::try_from(case).unwrap_or(0);
                    let left = (case_seed + delta * 19) % 97 - 48;
                    let right = (case_seed * 3 - delta * 11) % 89 - 44;
                    let output = match op {
                        Op::Add => left.wrapping_add(right),
                        Op::Sub => left.wrapping_sub(right),
                        Op::Mul => left.wrapping_mul(right),
                        _ => 0,
                    };
                    TaskCase {
                        inputs: vec![Scalar::I64(left), Scalar::I64(right)],
                        outputs: vec![Scalar::I64(output)],
                    }
                })
                .collect();
            (vec![Ty::I64, Ty::I64], vec![Ty::I64], Target::Arithmetic(op), source, cases)
        }
        1 => {
            let reversed = within_block % 2 != 0;
            let stack_in = if reversed {
                vec![Ty::Bool, Ty::I64]
            } else {
                vec![Ty::I64, Ty::Bool]
            };
            let stack_out = if reversed {
                vec![Ty::I64, Ty::Bool]
            } else {
                vec![Ty::Bool, Ty::I64]
            };
            let cases = (0..FIXTURE_CASES)
                .map(|case| {
                    let delta = i64::try_from(case).unwrap_or(0);
                    let integer = case_seed + delta * 31;
                    let boolean = (block + within_block + u8::try_from(case).unwrap_or(0)) % 2 == 0;
                    let inputs = if reversed {
                        vec![Scalar::Bool(boolean), Scalar::I64(integer)]
                    } else {
                        vec![Scalar::I64(integer), Scalar::Bool(boolean)]
                    };
                    let outputs = if reversed {
                        vec![Scalar::I64(integer), Scalar::Bool(boolean)]
                    } else {
                        vec![Scalar::Bool(boolean), Scalar::I64(integer)]
                    };
                    TaskCase { inputs, outputs }
                })
                .collect();
            (stack_in, stack_out, Target::Swap, b"swap".to_vec(), cases)
        }
        2 => {
            let mode = (block + within_block) % 3;
            let source = match mode {
                0 => b"=".to_vec(),
                1 => b"+ 0 =".to_vec(),
                _ => b"- 0 =".to_vec(),
            };
            let cases = (0..FIXTURE_CASES)
                .map(|case| {
                    let delta = i64::try_from(case).unwrap_or(0);
                    let left = case_seed + delta * 13;
                    let right = case_seed / 3 - delta * 7;
                    let compared = match mode {
                        0 => left == right,
                        1 => left.wrapping_add(right) == 0,
                        _ => left.wrapping_sub(right) == 0,
                    };
                    TaskCase {
                        inputs: vec![Scalar::I64(left), Scalar::I64(right)],
                        outputs: vec![Scalar::Bool(compared)],
                    }
                })
                .collect();
            (vec![Ty::I64, Ty::I64], vec![Ty::Bool], Target::Boolean(mode), source, cases)
        }
        3 | 4 => {
            let value = i64::from((block * 8 + within_block) % 3);
            let source = if family == 3 {
                match value {
                    0 => b"0".to_vec(),
                    1 => b"[ 1 ] run".to_vec(),
                    _ => b"[ [ 2 ] ] run run".to_vec(),
                }
            } else {
                value.to_string().into_bytes()
            };
            let cases = (0..FIXTURE_CASES)
                .map(|_| TaskCase {
                    inputs: Vec::new(),
                    outputs: vec![Scalar::I64(value)],
                })
                .collect();
            (Vec::new(), vec![Ty::I64], Target::Integer(value), source, cases)
        }
        _ => unreachable!(),
    };
    TaskSpec {
        family,
        block,
        within_block,
        stack_in,
        stack_out,
        cases,
        primitive_source,
        target,
    }
}

fn corpus_tasks() -> Vec<TaskSpec> {
    let mut tasks = Vec::with_capacity(480);
    for family in 0..FAMILY_NAMES.len() {
        for block in 0..12 {
            for within_block in 0..8 {
                tasks.push(task_spec(family, block, within_block));
            }
        }
    }
    tasks
}

fn empty_instantiation() -> Vec<Instantiation> {
    vec![Instantiation { bindings: Vec::new() }]
}

fn operation(op: Op) -> Node {
    Node::Operation {
        op,
        instantiation: 0,
    }
}

fn envelope(
    stack_in: &[Ty],
    stack_out: &[Ty],
    body: Recipe,
    source: &[u8],
    instantiations: Vec<Instantiation>,
) -> Envelope {
    let mut envelope = Envelope {
        format: 1,
        semantic_revision: 0,
        profile: String::from("pure-i64-bool/1"),
        chunk_id: [0; chunks::IDENTITY_BYTES],
        source_revision: chunks::canonical_revision(source),
        interface: Interface {
            stack_in: stack_in.to_vec(),
            stack_out: stack_out.to_vec(),
            effects: EffSet::empty(),
        },
        captures: Vec::new(),
        dependencies: Vec::new(),
        instantiations,
        body,
    };
    envelope.chunk_id = chunks::chunk_identity_of(&envelope);
    envelope
}

fn literal_envelope(value: i64, source: &[u8]) -> Envelope {
    envelope(
        &[],
        &[Ty::I64],
        Recipe {
            nodes: vec![Node::I64(value)],
            entry: vec![0],
        },
        source,
        Vec::new(),
    )
}

fn operation_envelope(op: Op, source: &[u8]) -> Envelope {
    let (stack_in, stack_out) = match op {
        Op::Add | Op::Sub | Op::Mul => (vec![Ty::I64, Ty::I64], vec![Ty::I64]),
        Op::Eq => (vec![Ty::I64, Ty::I64], vec![Ty::Bool]),
        _ => unreachable!(),
    };
    envelope(
        &stack_in,
        &stack_out,
        Recipe {
            nodes: vec![operation(op)],
            entry: vec![0],
        },
        source,
        empty_instantiation(),
    )
}

fn generic_operation_envelope(
    op: Op,
    stack_in: &[Ty],
    stack_out: &[Ty],
    bindings: Vec<Binding>,
    source: &[u8],
) -> Envelope {
    envelope(
        stack_in,
        stack_out,
        Recipe {
            nodes: vec![operation(op)],
            entry: vec![0],
        },
        source,
        vec![Instantiation { bindings }],
    )
}

fn boolean_comparison_envelope(op: Op, source: &[u8]) -> Envelope {
    let nodes = if op == Op::Eq {
        vec![operation(Op::Eq)]
    } else {
        vec![operation(op), Node::I64(0), operation(Op::Eq)]
    };
    let entry = (0..nodes.len())
        .map(|index| u32::try_from(index).unwrap_or(0))
        .collect();
    envelope(
        &[Ty::I64, Ty::I64],
        &[Ty::Bool],
        Recipe { nodes, entry },
        source,
        empty_instantiation(),
    )
}

fn quotation_envelope(value: i64, depth: u8, source: &[u8]) -> Envelope {
    let mut nodes = vec![Node::I64(value)];
    let mut previous = 0u32;
    for _ in 0..depth {
        let at = u32::try_from(nodes.len()).unwrap_or(0);
        nodes.push(Node::Quotation {
            body: vec![previous],
            instantiation: 0,
        });
        previous = at;
    }
    let mut entry = vec![previous];
    for _ in 0..depth {
        let at = u32::try_from(nodes.len()).unwrap_or(0);
        nodes.push(operation(Op::Run));
        entry.push(at);
    }
    envelope(
        &[],
        &[Ty::I64],
        Recipe { nodes, entry },
        source,
        empty_instantiation(),
    )
}

fn family_envelopes(family: usize) -> Vec<Envelope> {
    match family {
        0 => vec![
            operation_envelope(Op::Add, b"+ "),
            operation_envelope(Op::Sub, b"- "),
            operation_envelope(Op::Mul, b"* "),
        ],
        1 => vec![
            generic_operation_envelope(
                Op::Swap,
                &[Ty::I64, Ty::Bool],
                &[Ty::Bool, Ty::I64],
                vec![
                    Binding {
                        variable: 1,
                        kind: noble_kernel::words::VariableKind::Value,
                        ty: Ty::I64,
                    },
                    Binding {
                        variable: 2,
                        kind: noble_kernel::words::VariableKind::Value,
                        ty: Ty::Bool,
                    },
                ],
                b"swap ",
            ),
            generic_operation_envelope(
                Op::Swap,
                &[Ty::Bool, Ty::I64],
                &[Ty::I64, Ty::Bool],
                vec![
                    Binding {
                        variable: 1,
                        kind: noble_kernel::words::VariableKind::Value,
                        ty: Ty::Bool,
                    },
                    Binding {
                        variable: 2,
                        kind: noble_kernel::words::VariableKind::Value,
                        ty: Ty::I64,
                    },
                ],
                b"swap ",
            ),
            generic_operation_envelope(
                Op::Dup,
                &[Ty::I64],
                &[Ty::I64, Ty::I64],
                vec![Binding {
                    variable: 1,
                    kind: noble_kernel::words::VariableKind::Value,
                    ty: Ty::I64,
                }],
                b"dup ",
            ),
        ],
        2 => vec![
            boolean_comparison_envelope(Op::Eq, b"= "),
            boolean_comparison_envelope(Op::Add, b"+ 0 = "),
            boolean_comparison_envelope(Op::Sub, b"- 0 = "),
        ],
        3 => vec![
            literal_envelope(0, b"0 "),
            quotation_envelope(1, 1, b"[ 1 ] run "),
            quotation_envelope(2, 2, b"[ [ 2 ] ] run run "),
        ],
        4 => vec![
            literal_envelope(0, b"0 "),
            literal_envelope(1, b"1 "),
            literal_envelope(2, b"2 "),
        ],
        _ => unreachable!(),
    }
}

fn family_revision(family: usize) -> [u8; chunks::REVISION_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"noble.tchunk-ranking-library/1\0");
    hasher.update(FAMILY_NAMES[family].as_bytes());
    *hasher.finalize().as_bytes()
}

fn family_library(session: &noble_contracts::source::Session, family: usize) -> Result<Library, String> {
    let mut library = Library::new(family_revision(family));
    for envelope in family_envelopes(family) {
        library
            .admit(session, envelope)
            .map_err(|refusal| format!("admit family {} chunk: {refusal:?}", FAMILY_NAMES[family]))?;
    }
    Ok(library)
}

fn ty_name(ty: &Ty) -> &'static str {
    match ty {
        Ty::I64 => "I64",
        Ty::Bool => "Bool",
        _ => "unsupported",
    }
}

fn scalar_json(value: Scalar) -> Value {
    match value {
        Scalar::I64(value) => json!({"I64": value}),
        Scalar::Bool(value) => json!({"Bool": value}),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        use core::fmt::Write;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn corpus_manifest() -> Value {
    let tasks = corpus_tasks();
    let libraries: Vec<Value> = (0..FAMILY_NAMES.len())
        .map(|family| {
            let revision = family_revision(family);
            let entries = family_envelopes(family);
            let mut identity_bytes = Vec::with_capacity(
                revision
                    .len()
                    .saturating_add(entries.len().saturating_mul(
                        chunks::IDENTITY_BYTES.saturating_add(chunks::REVISION_BYTES),
                    )),
            );
            identity_bytes.extend_from_slice(&revision);
            for entry in &entries {
                identity_bytes.extend_from_slice(&entry.chunk_id);
                identity_bytes.extend_from_slice(&entry.source_revision);
            }
            let identity = blake3::hash(&identity_bytes);
            json!({
                "family": FAMILY_NAMES[family],
                "revision": hex(&revision),
                "identity": hex(identity.as_bytes()),
                "identity_rule": "blake3(revision || insertion-ordered-(chunk-identity,source-revision)-pairs)",
                "entries": entries.iter().map(|entry| json!({
                    "chunk_id": hex(&entry.chunk_id),
                    "source_revision": hex(&entry.source_revision),
                    "stack_in": entry.interface.stack_in.iter().map(ty_name).collect::<Vec<_>>(),
                    "stack_out": entry.interface.stack_out.iter().map(ty_name).collect::<Vec<_>>(),
                    "effects": "pure",
                    "dependency_ids": entry.dependencies.iter().map(|dependency| dependency.identity).collect::<Vec<_>>(),
                    "recipe_node_count": entry.body.nodes.len(),
                    "recipe_entry_count": entry.body.entry.len()
                })).collect::<Vec<_>>()
            })
        })
        .collect();
    let records: Vec<Value> = tasks
        .iter()
        .map(|task| {
            json!({
                "id": task.id(),
                "family": FAMILY_NAMES[task.family],
                "chunk_library_identity": libraries[task.family]["identity"].clone(),
                "provenance": {
                    "generator": "tchunk-ranking-corpus/1",
                    "family_index": task.family,
                    "block": task.block,
                    "within_block": task.within_block,
                    "split": task.split()
                },
                "contract": {
                    "stack_in": task.stack_in.iter().map(ty_name).collect::<Vec<_>>(),
                    "stack_out": task.stack_out.iter().map(ty_name).collect::<Vec<_>>(),
                    "effects": "pure"
                },
                "primitive_reference_source": String::from_utf8_lossy(&task.primitive_source),
                "independent_fixtures": task.cases.iter().map(|case| json!({
                    "inputs": case.inputs.iter().copied().map(scalar_json).collect::<Vec<_>>(),
                    "expected": case.outputs.iter().copied().map(scalar_json).collect::<Vec<_>>()
                })).collect::<Vec<_>>(),
                "target_kind": match task.target {
                    Target::Arithmetic(op) => format!("arithmetic:{}", op.spelling()),
                    Target::Swap => "ordered-stack-swap".to_string(),
                    Target::Boolean(mode) => format!("boolean-mode:{mode}"),
                    Target::Integer(value) if task.family == 3 => format!("nested-integer:{value}"),
                    Target::Integer(value) => format!("equal-signature-integer:{value}"),
                }
            })
        })
        .collect();
    json!({
        "schema": "tchunk-ranking-corpus/1",
        "task_count": records.len(),
        "family_count": FAMILY_NAMES.len(),
        "tasks_per_family": 96,
        "provenance_blocks_per_family": 12,
        "tasks_per_block": 8,
        "split": {"train": 280, "validation": 80, "test": 120, "unit": "family-provenance-block"},
        "deduplicate_by": ["canonical-source-bytes", "normalized-recipe-bytes", "ordered-interface", "independent-expected-trace", "generator-provenance", "chunk-library-identity"],
        "expected_values_owner": "independent-consumer-fixture",
        "expected_values_are_features": false,
        "libraries": libraries,
        "tasks": records
    })
}

#[test]
fn frozen_corpus_has_the_registered_split_and_valid_libraries() -> Result<(), String> {
    let manifest = corpus_manifest();
    let tasks = manifest["tasks"]
        .as_array()
        .ok_or_else(|| String::from("manifest task array missing"))?;
    assert_eq!(tasks.len(), 480);
    assert_eq!(manifest["libraries"].as_array().map(Vec::len), Some(5));

    let mut split_counts = [0usize; 3];
    let mut family_counts = [0usize; 5];
    let mut seen_ids = std::collections::BTreeSet::new();
    for task in tasks {
        let id = task["id"].as_str().ok_or("task ID missing")?;
        assert!(seen_ids.insert(id.to_owned()), "duplicate task ID {id}");
        let family = task["provenance"]["family_index"]
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .ok_or("family index missing")?;
        let block = task["provenance"]["block"]
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .ok_or("provenance block missing")?;
        let split = match block {
            0..=6 => 0,
            7..=8 => 1,
            _ => 2,
        };
        family_counts[family] = family_counts[family].saturating_add(1);
        split_counts[split] = split_counts[split].saturating_add(1);
        assert_eq!(task["independent_fixtures"].as_array().map(Vec::len), Some(FIXTURE_CASES));
    }
    assert_eq!(family_counts, [96; 5]);
    assert_eq!(split_counts, [280, 80, 120]);

    let session = noble_contracts::source::Session::without_test_hosts();
    for family in 0..FAMILY_NAMES.len() {
        let library = family_library(&session, family)?;
        assert_eq!(library.entries().len(), 3);
        assert!(library
            .entries()
            .iter()
            .all(|entry| entry.interface().effects.is_empty()));
    }
    Ok(())
}

#[test]
#[ignore = "materializes the frozen corpus only; not an experiment measurement"]
fn materialize_tchunk_ranking_corpus() -> Result<(), String> {
    let output = std::env::var_os("NOBLE_TCRANK_CORPUS_OUT")
        .ok_or_else(|| String::from("NOBLE_TCRANK_CORPUS_OUT must name an external output file"))?;
    let bytes = serde_json::to_vec_pretty(&corpus_manifest()).map_err(|error| error.to_string())?;
    std::fs::write(output, bytes).map_err(|error| error.to_string())
}
