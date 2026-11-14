#![allow(
    tigerstyle::explicit_defaults,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the experiment harness serializes a fixed deterministic corpus and checks its independent rows."
)]

use noble_contracts::source::chunks::{
    self as chunks, Binding, Budget, Envelope, Fixture, FixtureSet, Instantiation, Interface,
    Library, Node, Op, Recipe, Request, Scalar, Task,
};
use noble_contracts::source::editor::{Candidate as EditorCandidate, Node as EditorNode};
use noble_kernel::types::{EffSet, Ty};
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{GraphBuilder, Op as NumericOp, ScalarType, SubProfile, Subject};
use noble_ngraph::snapshot::{layout_key, ParamSnapshot};
use noble_ngraph::train::{self, Adam, Contract, Point, TrainControl, Trained};
use noble_ngraph::{check, CheckedGraph};
use noble_ngraph_experiment::scheme::BLAKE3;
use serde_json::{json, Value};

const FAMILY_NAMES: [&str; 5] = [
    "arithmetic",
    "ordered-stack-routing",
    "boolean",
    "nested-quotation",
    "equal-signature-contrast",
];
const FIXTURE_CASES: usize = 4;
const FROZEN_CORPUS: &[u8] =
    include_bytes!("../../../../verification/typed-chunk-ranking/corpus-v1.json");
const FROZEN_CRITERIA: &[u8] =
    include_bytes!("../../../../verification/typed-chunk-ranking/criteria-v1.json");

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
        format!(
            "{}/b{:02}/t{:02}",
            FAMILY_NAMES[self.family], self.block, self.within_block
        )
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
            (
                vec![Ty::I64, Ty::I64],
                vec![Ty::I64],
                Target::Arithmetic(op),
                source,
                cases,
            )
        }
        1 => {
            let reversed = !within_block.is_multiple_of(2);
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
                    let boolean =
                        (block + within_block + u8::try_from(case).unwrap_or(0)).is_multiple_of(2);
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
            (
                vec![Ty::I64, Ty::I64],
                vec![Ty::Bool],
                Target::Boolean(mode),
                source,
                cases,
            )
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
            (
                Vec::new(),
                vec![Ty::I64],
                Target::Integer(value),
                source,
                cases,
            )
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
    vec![Instantiation {
        bindings: Vec::new(),
    }]
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

fn family_library(
    session: &noble_contracts::source::Session,
    family: usize,
) -> Result<Library, String> {
    let mut library = Library::new(family_revision(family));
    for envelope in family_envelopes(family) {
        library.admit(session, envelope).map_err(|refusal| {
            format!("admit family {} chunk: {refusal:?}", FAMILY_NAMES[family])
        })?;
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
    let generated = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    assert_eq!(generated, FROZEN_CORPUS, "frozen corpus manifest drifted");
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
        assert_eq!(
            task["independent_fixtures"].as_array().map(Vec::len),
            Some(FIXTURE_CASES)
        );
    }
    assert_eq!(family_counts, [96; 5]);
    assert_eq!(split_counts, [280, 80, 120]);

    let session = noble_contracts::source::Session::without_test_hosts();
    let mut libraries = Vec::with_capacity(FAMILY_NAMES.len());
    for family in 0..FAMILY_NAMES.len() {
        let library = family_library(&session, family)?;
        assert_eq!(library.entries().len(), 3);
        assert!(library
            .entries()
            .iter()
            .all(|entry| entry.interface().effects.is_empty()));
        libraries.push(library);
    }
    for spec in corpus_tasks() {
        let library = libraries
            .get(spec.family)
            .ok_or_else(|| format!("reference library missing for {}", spec.id()))?;
        let prepared = session
            .prepare(
                &spec.primitive_source,
                &spec.stack_in,
                source_limits(request(&spec, library).budget),
            )
            .map_err(|error| format!("primitive reference {} refused: {error:?}", spec.id()))?;
        assert_eq!(
            prepared.output(),
            spec.stack_out,
            "typed reference {}",
            spec.id()
        );
        let envelope = primitive_envelope(&spec);
        for case in &spec.cases {
            let expected = oracle_target(spec.target, &case.inputs)?;
            assert_eq!(expected, case.outputs, "independent fixture {}", spec.id());
            let actual = independent_recipe_output(&envelope, &case.inputs)?;
            assert_eq!(actual, case.outputs, "primitive recipe {}", spec.id());
        }
    }
    Ok(())
}

fn source_limits(budget: Budget) -> noble_contracts::Limits {
    noble_contracts::Limits {
        bytes: budget.source_bytes.min(Budget::PROFILE.source_bytes),
        nodes: budget.source_bytes.min(Budget::PROFILE.source_bytes),
        depth: budget.quotation_depth.min(Budget::PROFILE.quotation_depth),
        work: budget.checking_work.min(Budget::PROFILE.checking_work),
    }
}

#[test]
fn primitive_baseline_uses_the_callers_source_budget_caps() {
    let budget = Budget {
        source_bytes: 1_024,
        quotation_depth: 4,
        checking_work: 1_000,
        ..Budget::PROFILE
    };
    let limits = source_limits(budget);
    assert_eq!(limits.bytes, 1_024);
    assert_eq!(limits.nodes, 1_024);
    assert_eq!(limits.depth, 4);
    assert_eq!(limits.work, 1_000);
}

fn fixture_set(spec: &TaskSpec) -> FixtureSet {
    let cases: Vec<Fixture> = spec
        .cases
        .iter()
        .map(|case| Fixture {
            inputs: case.inputs.clone(),
            outputs: case.outputs.clone(),
        })
        .collect();
    let record = json!(cases
        .iter()
        .map(|case| json!({
            "inputs": case.inputs.iter().copied().map(scalar_json).collect::<Vec<_>>(),
            "outputs": case.outputs.iter().copied().map(scalar_json).collect::<Vec<_>>()
        }))
        .collect::<Vec<_>>());
    let encoded = serde_json::to_vec(&record).unwrap_or_default();
    let digest = blake3::hash(&encoded);
    let mut id = [0u8; chunks::IDENTITY_BYTES];
    id.copy_from_slice(&digest.as_bytes()[..chunks::IDENTITY_BYTES]);
    FixtureSet { id, cases }
}

fn request(spec: &TaskSpec, library: &Library) -> Request {
    let mut budget = Budget::PROFILE;
    budget.candidates = 4;
    budget.composed_chunks = 1;
    Request {
        format: 1,
        semantic_revision: 0,
        profile: String::from("pure-i64-bool/1"),
        task: Task {
            stack_in: spec.stack_in.clone(),
            stack_out: spec.stack_out.clone(),
            allowed_effects: EffSet::empty(),
        },
        editor: EditorCandidate {
            format: 1,
            nodes: vec![EditorNode::Hole],
        },
        hole_path: vec![0],
        library_revision: *library.revision(),
        fixtures: Some(fixture_set(spec)),
        budget,
    }
}

fn monotonic_now_ns() -> u64 {
    static START: std::sync::LazyLock<std::time::Instant> =
        std::sync::LazyLock::new(std::time::Instant::now);
    u64::try_from(START.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

struct CapturedFeature {
    candidate: chunks::CandidateId,
    features: chunks::CandidateFeatureVector,
}

struct CapturingRanker {
    model_identity: [u8; chunks::REVISION_BYTES],
    library_revision: [u8; chunks::REVISION_BYTES],
    identity: std::cell::RefCell<Option<chunks::RankingIdentity>>,
    features: Vec<CapturedFeature>,
}

impl chunks::CandidateRanker for CapturingRanker {
    fn model_identity(&self) -> [u8; chunks::REVISION_BYTES] {
        self.model_identity
    }

    fn provenance_matches(&self, identity: &chunks::RankingIdentity) -> bool {
        let matched = identity.model_identity == Some(self.model_identity)
            && identity.library_revision == self.library_revision
            && identity.feature_revision == chunks::FEATURE_REVISION
            && identity.semantic_revision == 0;
        if matched {
            *self.identity.borrow_mut() = Some(identity.clone());
        }
        matched
    }

    fn score(
        &mut self,
        request: &chunks::CandidateScoreRequest<'_>,
    ) -> Result<chunks::CandidateScoreResponse, chunks::ScorerFailure> {
        if !request.features.iter().all(|feature| feature.is_finite()) {
            return Err(chunks::ScorerFailure::InvalidFeature);
        }
        self.features.push(CapturedFeature {
            candidate: request.candidate,
            features: *request.features,
        });
        Ok(chunks::CandidateScoreResponse {
            request_identity: request.request_identity,
            model_identity: request.model_identity,
            candidate: request.candidate,
            score: 0.0,
        })
    }
}

struct FeatureCapture {
    identity: chunks::RankingIdentity,
    features: Vec<CapturedFeature>,
    outcomes: Vec<chunks::CandidateOutcome>,
    response: chunks::Response,
}

fn collect_features(
    session: &noble_contracts::source::Session,
    library: &Library,
    spec: &TaskSpec,
) -> Result<FeatureCapture, String> {
    let model_identity = [0x74; chunks::REVISION_BYTES];
    let mut ranker = CapturingRanker {
        model_identity,
        library_revision: *library.revision(),
        identity: std::cell::RefCell::new(None),
        features: Vec::new(),
    };
    let mut observed = Vec::new();
    let mut observer = |outcome: &chunks::CandidateOutcome| observed.push(outcome.clone());
    let mut ranker_session = chunks::RankerSession::new(&mut ranker, model_identity);
    let ranked = chunks::complete_ranked_observed(
        session,
        library,
        &request(spec, library),
        &mut ranker_session,
        &mut observer,
        monotonic_now_ns,
    );
    let identity = ranker
        .identity
        .borrow()
        .clone()
        .ok_or_else(|| format!("ranking identity missing for {}", spec.id()))?;
    if ranked.disposition != chunks::RankingDisposition::Applied {
        return Err(format!(
            "feature scorer refused {}: {:?}",
            spec.id(),
            ranked.disposition
        ));
    }
    ranker.features.sort_by_key(|row| row.candidate.ordinal);
    if ranker.features.len() != 4 {
        return Err(format!(
            "feature queue for {} had {} candidates, expected four",
            spec.id(),
            ranker.features.len()
        ));
    }
    Ok(FeatureCapture {
        identity,
        features: ranker.features,
        outcomes: observed,
        response: ranked.response,
    })
}

fn report_of(response: &chunks::Response) -> &chunks::Report {
    match response {
        chunks::Response::Accepted { report, .. }
        | chunks::Response::Invalid { report, .. }
        | chunks::Response::Unsupported { report, .. }
        | chunks::Response::Exhausted { report, .. }
        | chunks::Response::NoResult { report }
        | chunks::Response::InternalFailure { report, .. } => report,
    }
}

fn candidate_attempt_for<'a>(
    outcomes: &'a [chunks::CandidateOutcome],
    sequence: &[u16],
) -> Option<&'a chunks::CandidateOutcome> {
    outcomes.iter().find(|outcome| outcome.sequence == sequence)
}

fn candidate_label(
    session: &noble_contracts::source::Session,
    library: &Library,
    spec: &TaskSpec,
    envelope: &Envelope,
) -> Result<(f32, chunks::CandidateOutcome, chunks::Response), String> {
    let mut isolated = Library::new(*library.revision());
    isolated
        .admit(session, envelope.clone())
        .map_err(|refusal| format!("isolated target envelope refused: {refusal:?}"))?;
    let request = request(spec, &isolated);
    let mut outcomes = Vec::new();
    let mut observer = |outcome: &chunks::CandidateOutcome| outcomes.push(outcome.clone());
    let response = chunks::complete_observed(
        session,
        &isolated,
        &request,
        &mut observer,
        monotonic_now_ns,
    );
    let candidate_outcome = candidate_attempt_for(&outcomes, &[0])
        .cloned()
        .ok_or_else(|| format!("isolated candidate was not attempted for {}", spec.id()))?;
    if candidate_outcome.candidate.ordinal != 1 {
        return Err(format!(
            "isolated candidate ordinal changed for {}",
            spec.id()
        ));
    }
    if candidate_outcome.accepted
        && !matches!(&response, chunks::Response::Accepted { accepted, .. }
            if accepted.selected.as_slice() == [envelope.chunk_id])
    {
        return Err(format!(
            "isolated admission selected a different body for {}",
            spec.id()
        ));
    }
    let target =
        if candidate_outcome.accepted && candidate_outcome.fixture_agreement.unwrap_or(false) {
            1.0
        } else {
            0.0
        };
    Ok((target, candidate_outcome, response))
}

#[derive(Clone, Debug)]
enum OracleValue {
    I64(i64),
    Bool(bool),
    Quotation(Vec<u32>),
}

fn pop_i64(stack: &mut Vec<OracleValue>) -> Result<i64, String> {
    match stack.pop() {
        Some(OracleValue::I64(value)) => Ok(value),
        _ => Err(String::from("independent replay expected I64")),
    }
}

fn evaluate_recipe(
    recipe: &Recipe,
    references: &[u32],
    stack: &mut Vec<OracleValue>,
    depth: u32,
) -> Result<(), String> {
    if depth > 8 {
        return Err(String::from(
            "independent quotation depth exceeded its fixed bound",
        ));
    }
    for reference in references {
        let index = usize::try_from(*reference).map_err(|error| error.to_string())?;
        let node = recipe
            .nodes
            .get(index)
            .ok_or_else(|| String::from("independent recipe reference is outside its arena"))?;
        match node {
            Node::I64(value) => stack.push(OracleValue::I64(*value)),
            Node::Bool(value) => stack.push(OracleValue::Bool(*value)),
            Node::Capture { .. } | Node::Invoke { .. } => {
                return Err(String::from(
                    "fixture oracle does not admit captures or dependencies",
                ));
            }
            Node::Quotation { body, .. } => stack.push(OracleValue::Quotation(body.clone())),
            Node::Operation { op, .. } => match op {
                Op::Dup => {
                    let value = stack.last().cloned().ok_or("independent dup underflow")?;
                    stack.push(value);
                }
                Op::Drop => {
                    let _ = stack.pop().ok_or("independent drop underflow")?;
                }
                Op::Swap => {
                    if stack.len() < 2 {
                        return Err(String::from("independent swap underflow"));
                    }
                    let top = stack.len().saturating_sub(1);
                    let below = stack.len().saturating_sub(2);
                    stack.swap(top, below);
                }
                Op::Add | Op::Sub | Op::Mul | Op::Eq => {
                    let right = pop_i64(stack)?;
                    let left = pop_i64(stack)?;
                    match op {
                        Op::Add => stack.push(OracleValue::I64(left.wrapping_add(right))),
                        Op::Sub => stack.push(OracleValue::I64(left.wrapping_sub(right))),
                        Op::Mul => stack.push(OracleValue::I64(left.wrapping_mul(right))),
                        Op::Eq => stack.push(OracleValue::Bool(left == right)),
                        _ => return Err(String::from("unreachable arithmetic oracle branch")),
                    }
                }
                Op::Run => {
                    let Some(OracleValue::Quotation(body)) = stack.pop() else {
                        return Err(String::from("independent run received a non-quotation"));
                    };
                    evaluate_recipe(recipe, &body, stack, depth.saturating_add(1))?;
                }
            },
        }
    }
    Ok(())
}

fn independent_recipe_output(
    envelope: &Envelope,
    inputs: &[Scalar],
) -> Result<Vec<Scalar>, String> {
    let mut stack: Vec<OracleValue> = inputs
        .iter()
        .map(|value| match value {
            Scalar::I64(value) => OracleValue::I64(*value),
            Scalar::Bool(value) => OracleValue::Bool(*value),
        })
        .collect();
    evaluate_recipe(&envelope.body, &envelope.body.entry, &mut stack, 0)?;
    stack
        .into_iter()
        .map(|value| match value {
            OracleValue::I64(value) => Ok(Scalar::I64(value)),
            OracleValue::Bool(value) => Ok(Scalar::Bool(value)),
            OracleValue::Quotation(_) => {
                Err(String::from("independent replay left quotation data"))
            }
        })
        .collect()
}

fn oracle_target(target: Target, inputs: &[Scalar]) -> Result<Vec<Scalar>, String> {
    match target {
        Target::Arithmetic(op) => {
            let [Scalar::I64(left), Scalar::I64(right)] = inputs else {
                return Err(String::from("arithmetic oracle received malformed inputs"));
            };
            let value = match op {
                Op::Add => left.wrapping_add(*right),
                Op::Sub => left.wrapping_sub(*right),
                Op::Mul => left.wrapping_mul(*right),
                _ => {
                    return Err(String::from(
                        "arithmetic oracle received a non-arithmetic operation",
                    ))
                }
            };
            Ok(vec![Scalar::I64(value)])
        }
        Target::Swap => {
            let mut output = inputs.to_vec();
            output.reverse();
            Ok(output)
        }
        Target::Boolean(mode) => {
            let [Scalar::I64(left), Scalar::I64(right)] = inputs else {
                return Err(String::from("boolean oracle received malformed inputs"));
            };
            let result = match mode {
                0 => left == right,
                1 => left.wrapping_add(*right) == 0,
                _ => left.wrapping_sub(*right) == 0,
            };
            Ok(vec![Scalar::Bool(result)])
        }
        Target::Integer(value) => Ok(vec![Scalar::I64(value)]),
    }
}

fn primitive_envelope(spec: &TaskSpec) -> Envelope {
    match (spec.family, spec.target) {
        (0, Target::Arithmetic(op)) => {
            operation_envelope(op, &[op.spelling().as_bytes(), b" "].concat())
        }
        (1, Target::Swap) => family_envelopes(1)
            .into_iter()
            .find(|envelope| envelope.interface.stack_in == spec.stack_in)
            .unwrap_or_else(|| unreachable!()),
        (2, Target::Boolean(mode)) => match mode {
            0 => boolean_comparison_envelope(Op::Eq, b"= "),
            1 => boolean_comparison_envelope(Op::Add, b"+ 0 = "),
            _ => boolean_comparison_envelope(Op::Sub, b"- 0 = "),
        },
        (3, Target::Integer(0)) => literal_envelope(0, b"0 "),
        (3, Target::Integer(1)) => quotation_envelope(1, 1, b"[ 1 ] run "),
        (3, Target::Integer(_)) => quotation_envelope(2, 2, b"[ [ 2 ] ] run run "),
        (4, Target::Integer(value)) => {
            let source = format!("{value} ");
            literal_envelope(value, source.as_bytes())
        }
        _ => unreachable!(),
    }
}

#[derive(Clone, Debug)]
struct TrainingRow {
    task_id: String,
    family: String,
    split: String,
    ordinal: u32,
    sequence: Vec<u16>,
    candidate: chunks::CandidateId,
    label_candidate: chunks::CandidateId,
    features: chunks::CandidateFeatureVector,
    target: f32,
    accepted: bool,
    fixture_agreement: Option<bool>,
    refusal_class: Option<chunks::Class>,
    outcome: String,
    request_identity: [u8; chunks::REVISION_BYTES],
    label_request_identity: [u8; chunks::REVISION_BYTES],
    candidate_set_identity: [u8; chunks::REVISION_BYTES],
    independent_goal_identity: [u8; chunks::REVISION_BYTES],
    task_interface_identity: [u8; chunks::REVISION_BYTES],
    policy_identity: [u8; chunks::REVISION_BYTES],
    library_revision: [u8; chunks::REVISION_BYTES],
    library_identity: [u8; chunks::REVISION_BYTES],
    model_identity: [u8; chunks::REVISION_BYTES],
    cost: chunks::CandidateAttemptCost,
}

impl TrainingRow {
    fn point(&self) -> Point {
        Point {
            inputs: self.features.to_vec(),
            targets: vec![self.target],
        }
    }

    fn identity_json(&self) -> Value {
        json!({
            "request": hex(&self.request_identity),
            "label_request": hex(&self.label_request_identity),
            "candidate_set": hex(&self.candidate_set_identity),
            "independent_goal": hex(&self.independent_goal_identity),
            "task_interface": hex(&self.task_interface_identity),
            "policy": hex(&self.policy_identity),
            "library_revision": hex(&self.library_revision),
            "library_identity": hex(&self.library_identity),
            "feature_revision": chunks::FEATURE_REVISION,
            "semantic_revision": 0,
            "feature_capture_identity": hex(&self.model_identity)
        })
    }

    fn json(&self) -> Value {
        let feature_bits: Vec<u32> = self.features.iter().map(|value| value.to_bits()).collect();
        json!({
            "task_id": self.task_id,
            "family": self.family,
            "split": self.split,
            "ordinal": self.ordinal,
            "sequence": self.sequence,
            "candidate_id": {"ordinal": self.candidate.ordinal, "digest": hex(&self.candidate.digest)},
            "label_candidate_id": {"ordinal": self.label_candidate.ordinal, "digest": hex(&self.label_candidate.digest)},
            "features_f32_bits": feature_bits,
            "target_f32_bits": self.target.to_bits(),
            "target": self.target,
            "accepted": self.accepted,
            "fixture_agreement": self.fixture_agreement,
            "refusal_class": self.refusal_class.map(chunks::Class::name),
            "outcome": self.outcome,
            "identity": self.identity_json(),
            "attempt_cost_ns": {
                "feature_extraction": self.cost.feature_extraction_ns,
                "scorer": self.cost.scorer_elapsed_ns,
                "re_admission": self.cost.re_admission_ns,
                "resource_deltas": self.cost.resource_deltas
            },
            "proposer_provenance": {
                "source": if self.sequence.is_empty() { "empty-shortlex-candidate/no-library-entry" } else { "immutable-library-entry/exact-body-and-dependency-identity" },
                "candidate_id": {"ordinal": self.label_candidate.ordinal, "digest": hex(&self.label_candidate.digest)},
                "re_admission_ns": self.cost.re_admission_ns
            }
        })
    }
}

#[derive(Clone, Debug)]
struct RankingContext {
    family: String,
    library_identity: [u8; chunks::REVISION_BYTES],
    library_revision: [u8; chunks::REVISION_BYTES],
    candidate_set: [u8; chunks::REVISION_BYTES],
    policy: [u8; chunks::REVISION_BYTES],
    editor_request: [u8; chunks::REVISION_BYTES],
    task_interfaces: Vec<[u8; chunks::REVISION_BYTES]>,
    allowed_candidates: Vec<chunks::CandidateId>,
}

#[derive(Default)]
struct Dataset {
    rows: Vec<TrainingRow>,
    contexts: Vec<RankingContext>,
    feature_collection_ns: u64,
    target_label_ns: u64,
}

fn elapsed_ns(start: std::time::Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn library_identity(library: &Library) -> [u8; chunks::REVISION_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(library.revision());
    for entry in library.entries() {
        hasher.update(&entry.envelope().chunk_id);
        hasher.update(&entry.envelope().source_revision);
    }
    *hasher.finalize().as_bytes()
}

fn label_description(outcome: &chunks::CandidateOutcome, response: &chunks::Response) -> String {
    match (
        outcome.accepted,
        outcome.fixture_agreement,
        outcome.refusal_class,
    ) {
        (true, Some(true), _) => String::from("finite-fixture-success"),
        (true, Some(false), _) => String::from("accepted-fixture-mismatch"),
        (true, None, _) => String::from("accepted-without-fixture"),
        (false, _, Some(class)) => String::from(class.name()),
        (false, _, None) => String::from(response.outcome()),
    }
}

fn collect_split(
    session: &noble_contracts::source::Session,
    libraries: &[Library],
    tasks: &[TaskSpec],
    split: &str,
) -> Result<Dataset, String> {
    let mut data = Dataset::default();
    let mut known_contexts = std::collections::BTreeSet::new();
    for spec in tasks.iter().filter(|task| task.split() == split) {
        let library = libraries
            .get(spec.family)
            .ok_or_else(|| format!("library missing for {}", spec.id()))?;
        let feature_started = std::time::Instant::now();
        let capture = collect_features(session, library, spec)?;
        data.feature_collection_ns = data
            .feature_collection_ns
            .saturating_add(elapsed_ns(feature_started));
        let context_key = (
            spec.family,
            capture.identity.library_revision,
            capture.identity.candidate_set,
            capture.identity.policy,
            capture.identity.editor_request,
        );
        known_contexts.insert(context_key);
        let queue = chunks::candidate_queue(library, request(spec, library).budget.candidates);
        if queue.candidates().len() != capture.features.len() {
            return Err(format!("queue/features diverged for {}", spec.id()));
        }
        for candidate in queue.candidates() {
            let captured = capture
                .features
                .get(usize::try_from(candidate.ordinal).map_err(|error| error.to_string())?)
                .ok_or_else(|| {
                    format!(
                        "feature missing for {} ordinal {}",
                        spec.id(),
                        candidate.ordinal
                    )
                })?;
            if captured.candidate.ordinal != candidate.ordinal {
                return Err(format!("candidate order drifted for {}", spec.id()));
            }
            if !captured.features.iter().all(|feature| feature.is_finite()) {
                return Err(format!(
                    "nonfinite feature in {} ordinal {}",
                    spec.id(),
                    candidate.ordinal
                ));
            }
            let (target, attempt, label_request_identity, outcome_description) = if candidate
                .sequence
                .is_empty()
            {
                let outcome = candidate_attempt_for(&capture.outcomes, &[])
                    .cloned()
                    .ok_or_else(|| format!("empty sequence refusal missing for {}", spec.id()))?;
                let description = label_description(&outcome, &capture.response);
                let request_identity = outcome.request_identity;
                (0.0, outcome, request_identity, description)
            } else {
                if candidate.sequence.len() != 1 {
                    return Err(format!(
                        "training queue exceeded one-chunk scope for {}",
                        spec.id()
                    ));
                }
                let index = usize::from(candidate.sequence[0]);
                let envelope = library
                    .entries()
                    .get(index)
                    .ok_or_else(|| format!("candidate chunk index missing for {}", spec.id()))?
                    .envelope();
                let label_started = std::time::Instant::now();
                let (target, outcome, response) =
                    candidate_label(session, library, spec, envelope)?;
                data.target_label_ns = data
                    .target_label_ns
                    .saturating_add(elapsed_ns(label_started));
                let description = label_description(&outcome, &response);
                let request_identity = outcome.request_identity;
                (target, outcome, request_identity, description)
            };
            let row = TrainingRow {
                task_id: spec.id(),
                family: FAMILY_NAMES[spec.family].to_string(),
                split: split.to_string(),
                ordinal: candidate.ordinal,
                sequence: candidate.sequence.clone(),
                candidate: captured.candidate,
                label_candidate: attempt.candidate,
                features: captured.features,
                target,
                accepted: attempt.accepted,
                fixture_agreement: attempt.fixture_agreement,
                refusal_class: attempt.refusal_class,
                outcome: outcome_description,
                request_identity: capture.identity.request,
                label_request_identity,
                candidate_set_identity: capture.identity.candidate_set,
                independent_goal_identity: capture.identity.independent_goal,
                task_interface_identity: capture.identity.task_interface,
                policy_identity: capture.identity.policy,
                library_revision: capture.identity.library_revision,
                library_identity: library_identity(library),
                model_identity: capture
                    .identity
                    .model_identity
                    .unwrap_or([0; chunks::REVISION_BYTES]),
                cost: attempt.cost,
            };
            data.rows.push(row);
        }
    }
    for (family, revision, candidate_set, policy, editor_request) in known_contexts {
        let library = libraries
            .get(family)
            .ok_or_else(|| String::from("ranking context library missing"))?;
        let mut task_interfaces = Vec::new();
        let mut allowed_candidates = Vec::new();
        for row in data
            .rows
            .iter()
            .filter(|row| row.family == FAMILY_NAMES[family])
        {
            if !task_interfaces.contains(&row.task_interface_identity) {
                task_interfaces.push(row.task_interface_identity);
            }
            if !allowed_candidates.contains(&row.candidate) {
                allowed_candidates.push(row.candidate);
            }
        }
        allowed_candidates.sort_by_key(|candidate| candidate.ordinal);
        data.contexts.push(RankingContext {
            family: FAMILY_NAMES[family].to_string(),
            library_identity: library_identity(library),
            library_revision: revision,
            candidate_set,
            policy,
            editor_request,
            task_interfaces,
            allowed_candidates,
        });
    }
    Ok(data)
}

const TRAIN_SEEDS: [u64; 3] = [0x74031a7e20261029, 0x74031a7e2026102a, 0x74031a7e2026102b];
const PAIRED_SEEDS: [u64; 10] = [
    0x7403000000000001,
    0x7403000000000002,
    0x7403000000000003,
    0x7403000000000004,
    0x7403000000000005,
    0x7403000000000006,
    0x7403000000000007,
    0x7403000000000008,
    0x7403000000000009,
    0x740300000000000a,
];
const TRAIN_STEPS: u64 = 256;
const TRAIN_BATCH_SIZE: u32 = 32;

fn score_graph() -> CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "tchunk-ranking/score-v1"),
    );
    let inputs: Vec<_> = (0..chunks::FEATURE_LANES)
        .map(|_| builder.input(ScalarType::F32))
        .collect();
    let products: Vec<_> = inputs
        .into_iter()
        .map(|input| {
            let parameter = builder.param();
            builder.op(NumericOp::F32Mul(parameter, input))
        })
        .collect();
    let mut total = products[0];
    for product in products.iter().skip(1) {
        total = builder.op(NumericOp::F32Add(total, *product));
    }
    let bias = builder.param();
    let output = builder.op(NumericOp::F32Add(total, bias));
    builder.output(output);
    check(&BLAKE3, builder.finish()).expect("fixed scorer graph admits")
}

fn mse_graph() -> CheckedGraph {
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&BLAKE3, "tchunk-ranking/loss-v1"),
    );
    let score = builder.input(ScalarType::F32);
    let target = builder.input(ScalarType::F32);
    let difference = builder.op(NumericOp::F32Sub(score, target));
    let loss = builder.op(NumericOp::F32Mul(difference, difference));
    builder.output(loss);
    check(&BLAKE3, builder.finish()).expect("fixed MSE loss graph admits")
}

fn numeric_output(
    model: &CheckedGraph,
    snapshot: &ParamSnapshot,
    inputs: &[f32],
) -> Result<f32, String> {
    let mut scratch = noble_ngraph::eval::ScratchCounters::default();
    let mut evaluated_nodes = 0u64;
    numeric_output_traced(model, snapshot, inputs, &mut scratch, &mut evaluated_nodes)
}

fn numeric_output_traced(
    model: &CheckedGraph,
    snapshot: &ParamSnapshot,
    inputs: &[f32],
    scratch: &mut noble_ngraph::eval::ScratchCounters,
    evaluated_nodes: &mut u64,
) -> Result<f32, String> {
    let observation = eval::reference(
        model,
        &eval::f32_inputs(inputs),
        Some(snapshot),
        &mut Control {
            scratch: Some(scratch),
            evaluated_nodes: Some(evaluated_nodes),
            ..Control::default()
        },
    )
    .obs;
    match observation {
        Obs::Normal(values) => match values.as_slice() {
            [noble_ngraph::sem::Value::F32(bits)] => Ok(f32::from_bits(*bits)),
            _ => Err(String::from("scorer returned a non-singleton F32 result")),
        },
        other => Err(format!("scorer evaluation refused: {other:?}")),
    }
}

fn validation_mean_loss(
    model: &CheckedGraph,
    snapshot: &ParamSnapshot,
    rows: &[TrainingRow],
) -> Result<f32, String> {
    let mut sum = 0.0f32;
    for row in rows {
        let score = numeric_output(model, snapshot, &row.features)?;
        let difference = score - row.target;
        sum += difference * difference;
    }
    let count = u32::try_from(rows.len()).map_err(|error| error.to_string())?;
    if count == 0 {
        return Err(String::from("validation set is empty"));
    }
    Ok(sum / count as f32)
}

fn assert_equal_signature_features(rows: &[TrainingRow]) -> Result<(), String> {
    for ordinal in 1..=3 {
        let selected: Vec<&TrainingRow> = rows
            .iter()
            .filter(|row| row.family == FAMILY_NAMES[4] && row.ordinal == ordinal)
            .collect();
        let Some(first) = selected.first() else {
            return Err(String::from("equal-signature training rows are missing"));
        };
        let expected_bits: Vec<u32> = first.features.iter().map(|value| value.to_bits()).collect();
        let mut labels = std::collections::BTreeSet::new();
        for row in selected {
            let actual_bits: Vec<u32> = row.features.iter().map(|value| value.to_bits()).collect();
            if actual_bits != expected_bits {
                return Err(format!(
                    "fixture leakage changed equal-signature features for ordinal {ordinal}"
                ));
            }
            labels.insert(row.target.to_bits());
        }
        if labels.len() != 2 {
            return Err(format!(
                "equal-signature ordinal {ordinal} lacks both behavioral labels"
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct SeedSummary {
    seed: u64,
    validation_loss: f32,
    training_elapsed_ns: u64,
    steps: u64,
    optimizer_state_digest: [u8; chunks::REVISION_BYTES],
    optimizer_state_bytes: u64,
    parameter_bytes: u64,
    provenance_data: [u8; chunks::REVISION_BYTES],
}

struct ModelFit {
    graph: CheckedGraph,
    loss_graph: CheckedGraph,
    snapshot: ParamSnapshot,
    provenance: train::Provenance,
    state: train::AdamState,
    selected_seed: u64,
    validation_loss: f32,
    seeds: Vec<SeedSummary>,
    training_rows: Dataset,
    validation_rows: Dataset,
    training_elapsed_ns: u64,
    model_selection_elapsed_ns: u64,
    train_scratch_peak_bytes: u64,
    train_evaluated_nodes: u64,
}

struct ModelRanker<'a> {
    model_identity: [u8; chunks::REVISION_BYTES],
    graph: &'a CheckedGraph,
    snapshot: &'a ParamSnapshot,
    contexts: &'a [RankingContext],
    active_identity: std::cell::RefCell<Option<chunks::RankingIdentity>>,
    score_calls: u32,
    scorer_elapsed_ns: u64,
    evaluated_nodes: u64,
    scratch: noble_ngraph::eval::ScratchCounters,
    scored: Vec<ScoredCandidate>,
}

#[derive(Clone, Debug)]
struct ScoredCandidate {
    candidate: chunks::CandidateId,
    features: chunks::CandidateFeatureVector,
    score: f32,
}

impl ModelRanker<'_> {
    fn context_matches(&self, identity: &chunks::RankingIdentity) -> bool {
        identity.model_identity == Some(self.model_identity)
            && identity.feature_revision == chunks::FEATURE_REVISION
            && identity.semantic_revision == 0
            && self.contexts.iter().any(|context| {
                identity.library_revision == context.library_revision
                    && identity.candidate_set == context.candidate_set
                    && identity.policy == context.policy
                    && identity.editor_request == context.editor_request
                    && context.task_interfaces.contains(&identity.task_interface)
            })
    }
}

impl chunks::CandidateRanker for ModelRanker<'_> {
    fn model_identity(&self) -> [u8; chunks::REVISION_BYTES] {
        self.model_identity
    }

    fn provenance_matches(&self, identity: &chunks::RankingIdentity) -> bool {
        let matched = self.context_matches(identity);
        *self.active_identity.borrow_mut() = matched.then(|| identity.clone());
        matched
    }

    fn score(
        &mut self,
        request: &chunks::CandidateScoreRequest<'_>,
    ) -> Result<chunks::CandidateScoreResponse, chunks::ScorerFailure> {
        let active_identity = self.active_identity.borrow();
        let Some(identity) = active_identity.as_ref() else {
            return Err(chunks::ScorerFailure::StaleProvenance);
        };
        if request.request_identity != identity.request
            || request.model_identity != self.model_identity
        {
            return Err(chunks::ScorerFailure::StaleProvenance);
        }
        let Some(context) = self.contexts.iter().find(|context| {
            identity.library_revision == context.library_revision
                && identity.candidate_set == context.candidate_set
                && identity.policy == context.policy
                && identity.editor_request == context.editor_request
                && context.task_interfaces.contains(&identity.task_interface)
        }) else {
            return Err(chunks::ScorerFailure::StaleProvenance);
        };
        if !context.allowed_candidates.contains(&request.candidate) {
            return Err(chunks::ScorerFailure::InvalidResponse);
        }
        if !request.features.iter().all(|feature| feature.is_finite()) {
            return Err(chunks::ScorerFailure::InvalidFeature);
        }
        let started = std::time::Instant::now();
        let score = numeric_output_traced(
            self.graph,
            self.snapshot,
            request.features,
            &mut self.scratch,
            &mut self.evaluated_nodes,
        )
        .map_err(|_| chunks::ScorerFailure::Evaluation)?;
        self.scorer_elapsed_ns = self.scorer_elapsed_ns.saturating_add(elapsed_ns(started));
        self.score_calls = self.score_calls.saturating_add(1);
        self.scored.push(ScoredCandidate {
            candidate: request.candidate,
            features: *request.features,
            score,
        });
        if !score.is_finite() {
            return Err(chunks::ScorerFailure::Evaluation);
        }
        Ok(chunks::CandidateScoreResponse {
            request_identity: request.request_identity,
            model_identity: self.model_identity,
            candidate: request.candidate,
            score,
        })
    }
}

struct ShuffledRanker {
    model_identity: [u8; chunks::REVISION_BYTES],
    contexts: Vec<RankingContext>,
    active_identity: std::cell::RefCell<Option<chunks::RankingIdentity>>,
    scores: std::collections::BTreeMap<u32, f32>,
    score_calls: u32,
    scored: Vec<ScoredCandidate>,
}

impl chunks::CandidateRanker for ShuffledRanker {
    fn model_identity(&self) -> [u8; chunks::REVISION_BYTES] {
        self.model_identity
    }

    fn provenance_matches(&self, identity: &chunks::RankingIdentity) -> bool {
        let matched = identity.model_identity == Some(self.model_identity)
            && identity.feature_revision == chunks::FEATURE_REVISION
            && identity.semantic_revision == 0
            && self.contexts.iter().any(|context| {
                identity.library_revision == context.library_revision
                    && identity.candidate_set == context.candidate_set
                    && identity.policy == context.policy
                    && identity.editor_request == context.editor_request
                    && context.task_interfaces.contains(&identity.task_interface)
            });
        *self.active_identity.borrow_mut() = matched.then(|| identity.clone());
        matched
    }

    fn score(
        &mut self,
        request: &chunks::CandidateScoreRequest<'_>,
    ) -> Result<chunks::CandidateScoreResponse, chunks::ScorerFailure> {
        let active_identity = self.active_identity.borrow();
        let Some(identity) = active_identity.as_ref() else {
            return Err(chunks::ScorerFailure::StaleProvenance);
        };
        if request.request_identity != identity.request
            || request.model_identity != self.model_identity
            || !request.features.iter().all(|feature| feature.is_finite())
        {
            return Err(chunks::ScorerFailure::StaleProvenance);
        }
        let Some(score) = self.scores.get(&request.candidate.ordinal).copied() else {
            return Err(chunks::ScorerFailure::InvalidResponse);
        };
        self.score_calls = self.score_calls.saturating_add(1);
        self.scored.push(ScoredCandidate {
            candidate: request.candidate,
            features: *request.features,
            score,
        });
        Ok(chunks::CandidateScoreResponse {
            request_identity: request.request_identity,
            model_identity: self.model_identity,
            candidate: request.candidate,
            score,
        })
    }
}

fn optimizer_state_bytes(state: &train::AdamState) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(
        state
            .m
            .len()
            .saturating_add(state.v.len())
            .saturating_mul(core::mem::size_of::<f64>())
            .saturating_add(2 * core::mem::size_of::<f64>())
            .saturating_add(core::mem::size_of::<u64>()),
    );
    for value in &state.m {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    for value in &state.v {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    bytes.extend_from_slice(&state.b1t.to_bits().to_le_bytes());
    bytes.extend_from_slice(&state.b2t.to_bits().to_le_bytes());
    bytes.extend_from_slice(&state.t.to_le_bytes());
    bytes
}

fn fit_model(
    session: &noble_contracts::source::Session,
    libraries: &[Library],
    tasks: &[TaskSpec],
) -> Result<ModelFit, String> {
    let training_started = std::time::Instant::now();
    let training_rows = collect_split(session, libraries, tasks, "train")?;
    let validation_rows = collect_split(session, libraries, tasks, "validation")?;
    assert_eq!(training_rows.rows.len(), 280 * 4);
    assert_eq!(validation_rows.rows.len(), 80 * 4);
    assert_equal_signature_features(&training_rows.rows)?;

    let model = score_graph();
    let loss = mse_graph();
    if model.param_slots().len() != 65 || model.graph().input_slots().len() != 64 {
        return Err(String::from(
            "scorer graph differs from the 64-input/65-parameter contract",
        ));
    }
    let adam = Adam::default();
    if adam.lr.to_bits() != 0x3f947ae147ae147b
        || adam.beta1.to_bits() != 0x3feccccccccccccd
        || adam.beta2.to_bits() != 0x3feff7ced916872b
        || adam.eps.to_bits() != 0x3e45798ee2308c3a
    {
        return Err(String::from(
            "default adam/0 parameter bits differ from frozen criteria",
        ));
    }
    let train_points: Vec<Point> = training_rows.rows.iter().map(TrainingRow::point).collect();
    let validation_points: Vec<Point> = validation_rows
        .rows
        .iter()
        .map(TrainingRow::point)
        .collect();
    let mut summaries = Vec::with_capacity(TRAIN_SEEDS.len());
    let mut best: Option<(u64, f32, Trained)> = None;
    let selection_started = std::time::Instant::now();
    let mut train_scratch_peak_bytes = 0u64;
    let mut train_evaluated_nodes = 0u64;
    for seed in TRAIN_SEEDS {
        let contract = Contract {
            params: model.param_slots().to_vec(),
            model: model.clone(),
            loss: loss.clone(),
            deriv: noble_ngraph::grad::DerivOptions::default(),
            adam,
            seed,
            data: train_points.clone(),
        };
        let started = std::time::Instant::now();
        let mut scratch = noble_ngraph::eval::ScratchCounters::default();
        let mut evaluated_nodes = 0u64;
        let mut control = TrainControl {
            scratch: Some(&mut scratch),
            evaluated_nodes: Some(&mut evaluated_nodes),
            batch_size: Some(TRAIN_BATCH_SIZE),
            ..TrainControl::default()
        };
        let trained = train::train(&BLAKE3, &contract, None, TRAIN_STEPS, &mut control).map_err(
            |stopped| {
                format!(
                    "training seed {seed:016x} stopped: {:?}",
                    stopped.diagnostic
                )
            },
        )?;
        let elapsed = elapsed_ns(started);
        train_scratch_peak_bytes = train_scratch_peak_bytes.max(scratch.peak_bytes);
        train_evaluated_nodes = train_evaluated_nodes.saturating_add(evaluated_nodes);
        let validation_loss =
            validation_mean_loss(&model, &trained.snapshot, &validation_rows.rows)?;
        if !validation_loss.is_finite() {
            return Err(format!("validation loss is nonfinite for seed {seed:016x}"));
        }
        let state_bytes = optimizer_state_bytes(&trained.state);
        let summary = SeedSummary {
            seed,
            validation_loss,
            training_elapsed_ns: elapsed,
            steps: trained.provenance.steps,
            optimizer_state_digest: *blake3::hash(&state_bytes).as_bytes(),
            optimizer_state_bytes: u64::try_from(state_bytes.len()).unwrap_or(u64::MAX),
            parameter_bytes: u64::try_from(trained.snapshot.bytes().len()).unwrap_or(u64::MAX),
            provenance_data: trained.provenance.data,
        };
        summaries.push(summary);
        let is_better = match &best {
            None => true,
            Some((best_seed, best_loss, _)) => {
                validation_loss.total_cmp(best_loss).is_lt()
                    || (validation_loss.to_bits() == best_loss.to_bits() && seed < *best_seed)
            }
        };
        if is_better {
            best = Some((seed, validation_loss, trained));
        }
    }
    let (selected_seed, validation_loss, trained) =
        best.ok_or_else(|| String::from("no training seed produced a snapshot"))?;
    let model_selection_elapsed_ns = elapsed_ns(selection_started);
    if train_points.len() != 1_120 || validation_points.len() != 320 {
        return Err(String::from(
            "split point counts changed from the frozen 280/80 task sets",
        ));
    }
    Ok(ModelFit {
        graph: model,
        loss_graph: loss,
        snapshot: trained.snapshot,
        provenance: trained.provenance,
        state: (*trained.state).clone(),
        selected_seed,
        validation_loss,
        seeds: summaries,
        training_elapsed_ns: elapsed_ns(training_started),
        model_selection_elapsed_ns,
        training_rows,
        validation_rows,
        train_scratch_peak_bytes,
        train_evaluated_nodes,
    })
}

fn contexts_json(contexts: &[RankingContext]) -> Vec<Value> {
    contexts
        .iter()
        .map(|context| {
            json!({
                "family": context.family,
                "library_identity": hex(&context.library_identity),
                "library_revision": hex(&context.library_revision),
                "candidate_set": hex(&context.candidate_set),
                "policy": hex(&context.policy),
                "editor_request": hex(&context.editor_request),
                "task_interfaces": context.task_interfaces.iter().map(|digest| hex(digest)).collect::<Vec<_>>(),
                "allowed_candidates": context.allowed_candidates.iter().map(|candidate| json!({
                    "ordinal": candidate.ordinal,
                    "digest": hex(&candidate.digest)
                })).collect::<Vec<_>>()
            })
        })
        .collect()
}

fn model_identity(
    fit: &ModelFit,
    source_commit: &str,
    rustc_version: &str,
    cargo_version: &str,
) -> [u8; chunks::REVISION_BYTES] {
    let graph_text = noble_ngraph::format::graph::print(fit.graph.graph());
    let loss_text = noble_ngraph::format::graph::print(fit.loss_graph.graph());
    let state_bytes = optimizer_state_bytes(&fit.state);
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"noble.tchunk-ranking-model/1\0");
    hasher.update(source_commit.as_bytes());
    hasher.update(rustc_version.as_bytes());
    hasher.update(cargo_version.as_bytes());
    hasher.update(blake3::hash(FROZEN_CRITERIA).as_bytes());
    hasher.update(blake3::hash(FROZEN_CORPUS).as_bytes());
    hasher.update(graph_text.as_bytes());
    hasher.update(loss_text.as_bytes());
    hasher.update(&fit.snapshot.id());
    hasher.update(fit.snapshot.bytes());
    hasher.update(&fit.provenance.parent);
    hasher.update(&fit.provenance.graph);
    hasher.update(&fit.provenance.loss);
    hasher.update(&fit.provenance.data);
    hasher.update(&fit.provenance.optimizer_state);
    hasher.update(fit.provenance.tools.as_bytes());
    hasher.update(&fit.selected_seed.to_le_bytes());
    hasher.update(&fit.validation_loss.to_bits().to_le_bytes());
    for seed in &fit.seeds {
        hasher.update(&seed.seed.to_le_bytes());
        hasher.update(&seed.validation_loss.to_bits().to_le_bytes());
        hasher.update(&seed.provenance_data);
        hasher.update(&seed.optimizer_state_digest);
    }
    for context in &fit.training_rows.contexts {
        hasher.update(context.family.as_bytes());
        hasher.update(&context.library_identity);
        hasher.update(&context.library_revision);
        hasher.update(&context.candidate_set);
        hasher.update(&context.policy);
        hasher.update(&context.editor_request);
        for interface in &context.task_interfaces {
            hasher.update(interface);
        }
        for candidate in &context.allowed_candidates {
            hasher.update(&candidate.ordinal.to_le_bytes());
            hasher.update(&candidate.digest);
        }
    }
    hasher.update(blake3::hash(&state_bytes).as_bytes());
    *hasher.finalize().as_bytes()
}

fn reload_model(
    graph_text: &str,
    parameter_bytes: &[u8],
) -> Result<(CheckedGraph, ParamSnapshot), String> {
    let graph = noble_ngraph::format::graph::parse(&BLAKE3, graph_text)
        .map_err(|diagnostic| format!("serialized graph parse refused: {diagnostic:?}"))?;
    let graph = check(&BLAKE3, graph)
        .map_err(|diagnostic| format!("serialized graph check refused: {diagnostic:?}"))?;
    let snapshot = ParamSnapshot::from_parts(
        &BLAKE3,
        graph.id(),
        layout_key(&BLAKE3, graph.graph()),
        graph.param_slots().to_vec(),
        parameter_bytes.to_vec(),
    );
    snapshot
        .check_use(&graph)
        .map_err(|diagnostic| format!("serialized snapshot refused: {diagnostic:?}"))?;
    Ok((graph, snapshot))
}

struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn index(&mut self, upper: usize) -> usize {
        let upper = u64::try_from(upper).unwrap_or(1).max(1);
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let value = self.next();
            if value >= threshold {
                return usize::try_from(value % upper).unwrap_or(0);
            }
        }
    }
}

fn shuffled_model_identity(
    seed: u64,
    model_id: &[u8; chunks::REVISION_BYTES],
) -> [u8; chunks::REVISION_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"noble.tchunk-ranking-shuffle/1\0");
    hasher.update(model_id);
    hasher.update(&seed.to_le_bytes());
    *hasher.finalize().as_bytes()
}

fn shuffled_scores(context: &RankingContext, seed: u64) -> std::collections::BTreeMap<u32, f32> {
    let mut ordinals: Vec<u32> = context
        .allowed_candidates
        .iter()
        .map(|candidate| candidate.ordinal)
        .collect();
    let mut rng = SplitMix64 { state: seed };
    for upper in (2..=ordinals.len()).rev() {
        let index = rng.index(upper);
        ordinals.swap(upper.saturating_sub(1), index);
    }
    ordinals
        .into_iter()
        .enumerate()
        .map(|(position, ordinal)| {
            let score = match position {
                0 => 4.0,
                1 => 3.0,
                2 => 2.0,
                _ => 1.0,
            };
            (ordinal, score)
        })
        .collect()
}

#[derive(Clone, Copy)]
enum EvaluationMode {
    PrimitiveOnly,
    FixedChunkShortlex,
    LearnedRanking,
    ShuffledControl,
}

impl EvaluationMode {
    const fn name(self) -> &'static str {
        match self {
            Self::PrimitiveOnly => "primitive-only",
            Self::FixedChunkShortlex => "fixed-chunk-shortlex",
            Self::LearnedRanking => "learned-ranking",
            Self::ShuffledControl => "splitmix64-shuffled-control",
        }
    }
}

fn ranking_disposition_name(disposition: chunks::RankingDisposition) -> &'static str {
    match disposition {
        chunks::RankingDisposition::NotEvaluated => "not-evaluated",
        chunks::RankingDisposition::Applied => "applied",
        chunks::RankingDisposition::AppliedWithInvalidScores => "applied-with-invalid-scores",
        chunks::RankingDisposition::FallbackStaleProvenance => "fallback-stale-provenance",
        chunks::RankingDisposition::FallbackScorerFailure => "fallback-scorer-failure",
        chunks::RankingDisposition::FallbackInvalidResponse => "fallback-invalid-response",
        chunks::RankingDisposition::FallbackAllInvalidScores => "fallback-all-invalid-scores",
    }
}

fn response_outcome(response: &chunks::Response) -> &'static str {
    match response {
        chunks::Response::Accepted { .. } => "accepted",
        chunks::Response::Invalid { .. } => "invalid",
        chunks::Response::Unsupported { .. } => "unsupported",
        chunks::Response::Exhausted { .. } => "exhausted",
        chunks::Response::NoResult { .. } => "no-result",
        chunks::Response::InternalFailure { .. } => "internal-failure",
    }
}

fn fallback_disposition(disposition: chunks::RankingDisposition) -> bool {
    matches!(
        disposition,
        chunks::RankingDisposition::FallbackStaleProvenance
            | chunks::RankingDisposition::FallbackScorerFailure
            | chunks::RankingDisposition::FallbackInvalidResponse
            | chunks::RankingDisposition::FallbackAllInvalidScores
    )
}

fn candidate_outcome_json(outcome: &chunks::CandidateOutcome) -> Value {
    json!({
        "candidate_id": {"ordinal": outcome.candidate.ordinal, "digest": hex(&outcome.candidate.digest)},
        "ordinal": outcome.ordinal,
        "sequence": outcome.sequence,
        "accepted": outcome.accepted,
        "refusal_class": outcome.refusal_class.map(chunks::Class::name),
        "fixture_agreement": outcome.fixture_agreement,
        "request_identity": hex(&outcome.request_identity),
        "cost": {
            "feature_extraction_ns": outcome.cost.feature_extraction_ns,
            "scorer_elapsed_ns": outcome.cost.scorer_elapsed_ns,
            "re_admission_ns": outcome.cost.re_admission_ns,
            "resource_deltas": outcome.cost.resource_deltas
        }
    })
}

fn scored_candidate_json(candidate: &ScoredCandidate) -> Value {
    json!({
        "candidate_id": {"ordinal": candidate.candidate.ordinal, "digest": hex(&candidate.candidate.digest)},
        "features_f32_bits": candidate.features.iter().map(|value| value.to_bits()).collect::<Vec<_>>(),
        "score_f32_bits": candidate.score.to_bits()
    })
}

fn ranking_identity_json(identity: &chunks::RankingIdentity) -> Value {
    let budgets: Vec<Value> = chunks::Resource::ALL
        .iter()
        .map(|resource| {
            json!({
                "resource": format!("{resource:?}"),
                "limit": identity.budget.limit(*resource)
            })
        })
        .collect();
    json!({
        "request": hex(&identity.request),
        "task_interface": hex(&identity.task_interface),
        "independent_goal": hex(&identity.independent_goal),
        "candidate_set": hex(&identity.candidate_set),
        "library_revision": hex(&identity.library_revision),
        "feature_revision": identity.feature_revision,
        "semantic_revision": identity.semantic_revision,
        "policy": hex(&identity.policy),
        "editor_request": hex(&identity.editor_request),
        "model_identity": identity.model_identity.map(|model| hex(&model)),
        "budget": budgets
    })
}

fn independent_replay_agrees(
    response: &chunks::Response,
    library: &Library,
    spec: &TaskSpec,
) -> Result<bool, String> {
    let chunks::Response::Accepted { accepted, .. } = response else {
        return Ok(false);
    };
    if accepted.selected.len() != 1 {
        return Ok(false);
    }
    let selected = accepted.selected[0];
    let envelope = library
        .entries()
        .iter()
        .map(|entry| entry.envelope())
        .find(|envelope| envelope.chunk_id == selected)
        .ok_or_else(|| {
            format!(
                "accepted chunk identity is absent from library for {}",
                spec.id()
            )
        })?;
    for case in &spec.cases {
        let actual = independent_recipe_output(envelope, &case.inputs)?;
        if actual != case.outputs {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Clone, Copy)]
struct EvaluationMeta<'a> {
    mode: EvaluationMode,
    repetition: usize,
    seed: u64,
    spec: &'a TaskSpec,
    library: &'a Library,
}

struct EvaluationRun<'a> {
    metadata: EvaluationMeta<'a>,
    session: &'a noble_contracts::source::Session,
    fit: &'a ModelFit,
    model_id: &'a [u8; chunks::REVISION_BYTES],
    graph_text: &'a str,
    model_payload_bytes: u64,
}

struct EvaluationRecord<'a> {
    metadata: EvaluationMeta<'a>,
    response: chunks::Response,
    attempts: Vec<chunks::CandidateOutcome>,
    disposition: Option<chunks::RankingDisposition>,
    invalid_scores: u32,
    ranked_candidates: Vec<chunks::CandidateId>,
    candidate_row_count: usize,
    ranking_identity: Option<chunks::RankingIdentity>,
    scored: Vec<ScoredCandidate>,
    score_calls: u32,
    scorer_elapsed_ns: u64,
    score_evaluated_nodes: u64,
    score_scratch_peak_bytes: u64,
    model_load_ns: u64,
    model_payload_bytes: u64,
    started: std::time::Instant,
}

fn evaluation_row(record: EvaluationRecord<'_>) -> Result<Value, String> {
    let EvaluationRecord {
        metadata,
        response,
        attempts,
        disposition,
        invalid_scores,
        ranked_candidates,
        candidate_row_count,
        ranking_identity,
        scored,
        score_calls,
        scorer_elapsed_ns,
        score_evaluated_nodes,
        score_scratch_peak_bytes,
        model_load_ns,
        model_payload_bytes,
        started,
    } = record;
    let EvaluationMeta {
        mode,
        repetition,
        seed,
        spec,
        library,
    } = metadata;
    let replay_started = std::time::Instant::now();
    let independent_replay = independent_replay_agrees(&response, library, spec)?;
    let independent_replay_ns = elapsed_ns(replay_started);
    let report = report_of(&response);
    let fixture_agreement = report.fixtures.as_ref().map(|fixtures| fixtures.agreement);
    let structural_acceptance = matches!(&response, chunks::Response::Accepted { .. });
    let finite_fixture_success = fixture_agreement.unwrap_or(false);
    let fallback = disposition.is_some_and(fallback_disposition);
    let fallback_rows = if fallback {
        u32::try_from(candidate_row_count).unwrap_or(u32::MAX)
    } else {
        0
    };
    let expanded_nodes_slot = chunks::Resource::ALL
        .iter()
        .position(|resource| *resource == chunks::Resource::ExpandedNodes)
        .ok_or_else(|| String::from("resource roster omitted expanded-node accounting"))?;
    let maximum_expanded_program_nodes = attempts
        .iter()
        .map(|outcome| outcome.cost.resource_deltas[expanded_nodes_slot])
        .max();
    let total_charged_work = u64::from(report.checking_work);
    let work = json!({
        "total_charged_work": total_charged_work,
        "total_shared_work_definition": "Report.checking_work; distinct budget dimensions remain separately reported",
        "feature_extraction": score_calls.saturating_mul(chunks::FEATURE_EXTRACTION_WORK),
        "score_evaluation": score_evaluated_nodes,
        "queue_management": ranked_candidates.len(),
        "candidate_proposals": report.candidates,
        "candidate_checks": report.checked_candidates,
        "expanded_nodes": report.expanded_nodes,
        "maximum_expanded_program_nodes": maximum_expanded_program_nodes,
        "dependency_work": report.dependency_steps,
        "instantiation_work": report.instantiation_bindings,
        "source_preparation_work": null,
        "kernel_check_work": report.checking_work,
        "fixture_steps": report.evaluation_steps,
        "unmeasured": ["host source-preparation internals are not separately exposed by Session::prepare"]
    });
    let report_json = json!({
        "candidates": report.candidates,
        "checked_candidates": report.checked_candidates,
        "accepted": report.accepted,
        "rejected": report.rejected,
        "expanded_nodes": report.expanded_nodes,
        "maximum_expanded_program_nodes": maximum_expanded_program_nodes,
        "maximum_expanded_program_nodes_definition": "maximum per-admission expanded-node resource delta; null when no checker attempt occurred",
        "dependency_steps": report.dependency_steps,
        "instantiation_bindings": report.instantiation_bindings,
        "checking_work": report.checking_work,
        "evaluation_steps": report.evaluation_steps,
        "composed_chunks": report.composed_chunks,
        "source_bytes": report.source_bytes,
        "stack_height": report.stack_height,
        "quotation_depth": report.quotation_depth,
        "retained_bytes": report.retained_bytes,
        "core_latency_ns": report.total_latency_ns
    });
    let memory = json!({
        "score_graph_peak_scratch_bytes": score_scratch_peak_bytes,
        "actual_peak_retained_bytes": null,
        "retained_bytes_proxy": u64::from(library.retained()).saturating_add(u64::from(report.retained_bytes)).saturating_add(model_payload_bytes),
        "retained_bytes_proxy_definition": "library/report budget-retained bytes plus serialized model payload; allocator overhead is not measured",
        "library_retained_bytes": library.retained(),
        "request_retained_bytes": report.retained_bytes,
        "model_payload_bytes": model_payload_bytes
    });
    let ranked_candidates_json = ranked_candidates
        .iter()
        .map(|candidate| json!({ "ordinal": candidate.ordinal, "digest": hex(&candidate.digest) }))
        .collect::<Vec<_>>();
    let mut row = json!({
        "task_id": spec.id(),
        "family": FAMILY_NAMES[spec.family],
        "split": spec.split(),
        "repetition": repetition,
        "seed": format!("0x{seed:016x}"),
        "mode": mode.name(),
        "response_outcome": response_outcome(&response),
        "structural_acceptance": structural_acceptance,
        "finite_fixture_success": finite_fixture_success,
        "fixture_agreement": fixture_agreement,
        "independent_replay": independent_replay,
        "independent_replay_ns": independent_replay_ns,
        "ranking_disposition": disposition.map(ranking_disposition_name),
        "ranking_identity": ranking_identity.as_ref().map(ranking_identity_json),
        "invalid_score_rows": invalid_scores,
        "fallback_candidate_rows": fallback_rows,
        "candidate_rows": candidate_row_count,
        "score_calls": score_calls,
        "scorer_elapsed_ns": scorer_elapsed_ns,
        "score_evaluated_nodes": score_evaluated_nodes,
        "score_scratch_peak_bytes": score_scratch_peak_bytes,
        "model_load_ns": model_load_ns,
        "model_payload_bytes": model_payload_bytes,
        "latency_ns": 0,
        "work": work,
        "report": report_json,
        "memory": memory,
        "ranked_candidates": ranked_candidates_json,
        "scored_candidates": scored.iter().map(scored_candidate_json).collect::<Vec<_>>(),
        "attempts": attempts.iter().map(candidate_outcome_json).collect::<Vec<_>>()
    });
    row["latency_ns"] = json!(elapsed_ns(started));
    Ok(row)
}

fn run_completion_mode(run: EvaluationRun<'_>) -> Result<Value, String> {
    let EvaluationRun {
        metadata,
        session,
        fit,
        model_id,
        graph_text,
        model_payload_bytes,
    } = run;
    let EvaluationMeta {
        mode,
        repetition,
        seed,
        spec,
        library,
    } = metadata;
    let mut attempts = Vec::new();
    let mut observer = |outcome: &chunks::CandidateOutcome| attempts.push(outcome.clone());
    let mut model_load_ns = 0u64;
    let mut score_calls = 0u32;
    let mut scorer_elapsed_ns = 0u64;
    let mut score_evaluated_nodes = 0u64;
    let mut score_scratch_peak_bytes = 0u64;
    let mut invalid_scores = 0u32;
    let mut disposition = None;
    let mut ranking_identity = None;
    let ranked_candidates: Vec<chunks::CandidateId>;
    let candidate_row_count: usize;
    let mut scored = Vec::new();
    let started = std::time::Instant::now();
    let request = request(spec, library);
    let response = match mode {
        EvaluationMode::PrimitiveOnly => {
            let prepared = session
                .prepare(
                    &spec.primitive_source,
                    &spec.stack_in,
                    source_limits(request.budget),
                )
                .map_err(|error| {
                    format!("primitive preparation {} refused: {error:?}", spec.id())
                })?;
            if prepared.output() != spec.stack_out {
                return Err(format!(
                    "primitive output interface mismatch for {}",
                    spec.id()
                ));
            }
            let mut outputs_match = true;
            let primitive = primitive_envelope(spec);
            for case in &spec.cases {
                outputs_match &=
                    independent_recipe_output(&primitive, &case.inputs)? == case.outputs;
            }
            if !outputs_match {
                return Err(format!("primitive fixture mismatch for {}", spec.id()));
            }
            let primitive_outcome = json!({
                "task_id": spec.id(),
                "family": FAMILY_NAMES[spec.family],
                "split": "test",
                "repetition": repetition,
                "seed": format!("0x{seed:016x}"),
                "mode": mode.name(),
                "response_outcome": "primitive-prepared",
                "structural_acceptance": true,
                "finite_fixture_success": true,
                "fixture_agreement": true,
                "independent_replay": true,
                "candidate_rows": 0,
                "latency_ns": elapsed_ns(started),
                "report": {
                    "source_bytes": spec.primitive_source.len(),
                    "retained_bytes": 0,
                    "maximum_expanded_program_nodes": null
                },
                "work": {
                    "total_charged_work": null,
                    "total_shared_work_definition": "not exposed by Session::prepare",
                    "feature_extraction": 0,
                    "score_evaluation": 0,
                    "queue_management": 0,
                    "candidate_proposals": 0,
                    "candidate_checks": 0,
                    "expanded_nodes": 0,
                    "maximum_expanded_program_nodes": null,
                    "dependency_work": 0,
                    "instantiation_work": 0,
                    "source_preparation_work": null,
                    "kernel_check_work": null,
                    "fixture_steps": null,
                    "unmeasured": ["primitive preparation work counter is private to noble-contracts"]
                }
            });
            return Ok(primitive_outcome);
        }
        EvaluationMode::FixedChunkShortlex => {
            let response = chunks::complete_observed(
                session,
                library,
                &request,
                &mut observer,
                monotonic_now_ns,
            );
            candidate_row_count = chunks::candidate_queue(library, request.budget.candidates)
                .candidates()
                .len();
            ranked_candidates = attempts.iter().map(|outcome| outcome.candidate).collect();
            response
        }
        EvaluationMode::LearnedRanking => {
            let load_started = std::time::Instant::now();
            let (graph, snapshot) = reload_model(graph_text, fit.snapshot.bytes())?;
            model_load_ns = elapsed_ns(load_started);
            if snapshot.id() != fit.snapshot.id() || graph.id() != fit.graph.id() {
                return Err(String::from(
                    "reloaded model identity differs from the frozen trainer output",
                ));
            }
            let mut ranker = ModelRanker {
                model_identity: *model_id,
                graph: &graph,
                snapshot: &snapshot,
                contexts: &fit.training_rows.contexts,
                active_identity: std::cell::RefCell::new(None),
                score_calls: 0,
                scorer_elapsed_ns: 0,
                evaluated_nodes: 0,
                scratch: noble_ngraph::eval::ScratchCounters::default(),
                scored: Vec::new(),
            };
            let mut ranker_session = chunks::RankerSession::new(&mut ranker, *model_id);
            let ranked = chunks::complete_ranked_observed(
                session,
                library,
                &request,
                &mut ranker_session,
                &mut observer,
                monotonic_now_ns,
            );
            disposition = Some(ranked.disposition);
            ranking_identity = ranked.identity;
            invalid_scores = ranked.invalid_scores;
            ranked_candidates = ranked.ranked_candidates;
            candidate_row_count = ranked_candidates.len();
            score_calls = ranker.score_calls;
            scorer_elapsed_ns = ranker.scorer_elapsed_ns;
            score_evaluated_nodes = ranker.evaluated_nodes;
            score_scratch_peak_bytes = ranker.scratch.peak_bytes;
            scored = std::mem::take(&mut ranker.scored);
            ranked.response
        }
        EvaluationMode::ShuffledControl => {
            let context = fit
                .training_rows
                .contexts
                .iter()
                .find(|context| {
                    context.family == FAMILY_NAMES[spec.family]
                        && context.library_revision == *library.revision()
                        && context.library_identity == library_identity(library)
                })
                .ok_or_else(|| format!("shuffle context absent for {}", spec.id()))?;
            let shuffle_id = shuffled_model_identity(seed, model_id);
            let mut ranker = ShuffledRanker {
                model_identity: shuffle_id,
                contexts: fit.training_rows.contexts.clone(),
                active_identity: std::cell::RefCell::new(None),
                scores: shuffled_scores(context, seed),
                score_calls: 0,
                scored: Vec::new(),
            };
            let mut ranker_session = chunks::RankerSession::new(&mut ranker, shuffle_id);
            let ranked = chunks::complete_ranked_observed(
                session,
                library,
                &request,
                &mut ranker_session,
                &mut observer,
                monotonic_now_ns,
            );
            disposition = Some(ranked.disposition);
            ranking_identity = ranked.identity;
            invalid_scores = ranked.invalid_scores;
            ranked_candidates = ranked.ranked_candidates;
            candidate_row_count = ranked_candidates.len();
            score_calls = ranker.score_calls;
            scored = std::mem::take(&mut ranker.scored);
            ranked.response
        }
    };
    evaluation_row(EvaluationRecord {
        metadata,
        response,
        attempts,
        disposition,
        invalid_scores,
        ranked_candidates,
        candidate_row_count,
        ranking_identity,
        scored,
        score_calls,
        scorer_elapsed_ns,
        score_evaluated_nodes,
        score_scratch_peak_bytes,
        model_load_ns,
        model_payload_bytes: if matches!(mode, EvaluationMode::LearnedRanking) {
            model_payload_bytes
        } else {
            0
        },
        started,
    })
}

fn write_json_lines(path: &std::path::Path, records: &[Value]) -> Result<(), String> {
    let mut contents = String::new();
    for record in records {
        contents.push_str(&serde_json::to_string(record).map_err(|error| error.to_string())?);
        contents.push('\n');
    }
    std::fs::write(path, contents).map_err(|error| error.to_string())
}

fn command_text(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_string())
        .map_err(|error| format!("{program} output was not UTF-8: {error}"))
}

fn score_graph_counts(graph: &CheckedGraph) -> Result<Value, String> {
    let mut counts = std::collections::BTreeMap::<&'static str, u32>::new();
    for node in &graph.graph().nodes {
        let name = match &node.op {
            NumericOp::Input(_) => "input",
            NumericOp::Param(_) => "param",
            NumericOp::F32Mul(_, _) => "f32.mul",
            NumericOp::F32Add(_, _) => "f32.add",
            _ => {
                return Err(format!(
                    "forbidden operation in fixed scorer: {}",
                    node.op.name()
                ))
            }
        };
        let count = counts.entry(name).or_default();
        *count = count.saturating_add(1);
    }
    if counts.get("input") != Some(&64)
        || counts.get("param") != Some(&65)
        || counts.get("f32.mul") != Some(&64)
        || counts.get("f32.add") != Some(&64)
        || graph.graph().outputs.len() != 1
    {
        return Err(format!(
            "scorer graph shape differs from fixed contract: {counts:?}"
        ));
    }
    Ok(json!({
        "input_nodes": counts.get("input"),
        "param_nodes": counts.get("param"),
        "f32_multiply_nodes": counts.get("f32.mul"),
        "ordered_f32_add_nodes": counts.get("f32.add"),
        "output_count": graph.graph().outputs.len(),
        "forbidden_operations": [],
        "profile": graph.graph().profile.name()
    }))
}

fn loss_graph_counts(graph: &CheckedGraph) -> Result<Value, String> {
    let mut inputs = 0u32;
    let mut subtractions = 0u32;
    let mut multiplications = 0u32;
    for node in &graph.graph().nodes {
        match &node.op {
            NumericOp::Input(_) => inputs = inputs.saturating_add(1),
            NumericOp::F32Sub(_, _) => subtractions = subtractions.saturating_add(1),
            NumericOp::F32Mul(_, _) => multiplications = multiplications.saturating_add(1),
            _ => {
                return Err(format!(
                    "forbidden operation in frozen MSE loss graph: {}",
                    node.op.name()
                ))
            }
        }
    }
    if inputs != 2 || subtractions != 1 || multiplications != 1 || graph.graph().outputs.len() != 1
    {
        return Err(String::from(
            "MSE loss graph differs from the two-input stored-order contract",
        ));
    }
    Ok(json!({
        "input_nodes": inputs,
        "f32_subtraction_nodes": subtractions,
        "f32_multiply_nodes": multiplications,
        "output_count": graph.graph().outputs.len(),
        "forbidden_operations": [],
        "profile": graph.graph().profile.name()
    }))
}

#[test]
#[ignore = "runs the frozen 480-task training/evaluation experiment and writes retained evidence"]
fn evaluate_tchunk_ranking_experiment() -> Result<(), String> {
    let source_commit = command_text("git", &["rev-parse", "HEAD"])?;
    let dirty = command_text("git", &["status", "--porcelain"])?;
    if !dirty.is_empty() {
        return Err(format!(
            "measurement requires a clean committed tree; status was {dirty}"
        ));
    }
    let rustc_version = command_text("rustc", &["--version", "--verbose"])?;
    let cargo_version = command_text("cargo", &["--version"])?;
    let output_parent = std::env::temp_dir().join("noble-tchunk-ranking");
    std::fs::create_dir_all(&output_parent).map_err(|error| error.to_string())?;
    let output_dir = output_parent.join(format!(
        "run-{}",
        source_commit.get(..12).ok_or("short source commit")?
    ));
    let manifest = corpus_manifest();
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    if manifest_bytes != FROZEN_CORPUS {
        return Err(String::from(
            "generator differs from the committed frozen corpus",
        ));
    }
    let session = noble_contracts::source::Session::without_test_hosts();
    let tasks = corpus_tasks();
    let mut libraries = Vec::with_capacity(FAMILY_NAMES.len());
    for family in 0..FAMILY_NAMES.len() {
        libraries.push(family_library(&session, family)?);
    }

    let fit = fit_model(&session, &libraries, &tasks)?;
    let identity = model_identity(&fit, &source_commit, &rustc_version, &cargo_version);
    let graph_text = noble_ngraph::format::graph::print(fit.graph.graph());
    let loss_text = noble_ngraph::format::graph::print(fit.loss_graph.graph());
    let graph_counts = score_graph_counts(&fit.graph)?;
    let loss_counts = loss_graph_counts(&fit.loss_graph)?;
    let contexts = contexts_json(&fit.training_rows.contexts);
    let contexts_bytes = serde_json::to_vec(&contexts).map_err(|error| error.to_string())?;
    let model_payload_bytes = u64::try_from(graph_text.len())
        .unwrap_or(u64::MAX)
        .saturating_add(u64::try_from(fit.snapshot.bytes().len()).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(contexts_bytes.len()).unwrap_or(u64::MAX))
        .saturating_add(u64::try_from(chunks::FEATURE_REVISION.len()).unwrap_or(u64::MAX));
    let parameter_bytes = fit.snapshot.bytes().to_vec();

    let cold_started = std::time::Instant::now();
    let (loaded_graph, loaded_snapshot) = reload_model(&graph_text, &parameter_bytes)?;
    let cold_model_load_ns = elapsed_ns(cold_started);
    if loaded_snapshot.id() != fit.snapshot.id() || loaded_graph.id() != fit.graph.id() {
        return Err(String::from(
            "cold-loaded model differs from trained graph/snapshot",
        ));
    }
    let mut warm_model_load_samples = Vec::with_capacity(5);
    for _ in 0..5 {
        let warm_started = std::time::Instant::now();
        let (warm_graph, warm_snapshot) = reload_model(&graph_text, &parameter_bytes)?;
        if warm_snapshot.id() != fit.snapshot.id() || warm_graph.id() != fit.graph.id() {
            return Err(String::from(
                "warm-loaded model differs from trained graph/snapshot",
            ));
        }
        warm_model_load_samples.push(elapsed_ns(warm_started));
    }
    let mut sorted_warm_model_load_samples = warm_model_load_samples.clone();
    sorted_warm_model_load_samples.sort_unstable();
    let warm_model_load_median = sorted_warm_model_load_samples
        .get(sorted_warm_model_load_samples.len() / 2)
        .copied()
        .unwrap_or(0);

    let mut training_records = Vec::with_capacity(
        fit.training_rows
            .rows
            .len()
            .saturating_add(fit.validation_rows.rows.len()),
    );
    training_records.extend(fit.training_rows.rows.iter().map(TrainingRow::json));
    training_records.extend(fit.validation_rows.rows.iter().map(TrainingRow::json));

    let mut result_records = Vec::with_capacity(120 * PAIRED_SEEDS.len() * 4);
    let evaluation_session = noble_contracts::source::Session::without_test_hosts();
    for (repetition, seed) in PAIRED_SEEDS.iter().copied().enumerate() {
        for spec in tasks.iter().filter(|task| task.split() == "test") {
            let library = libraries
                .get(spec.family)
                .ok_or_else(|| format!("held-out library missing for {}", spec.id()))?;
            for mode in [
                EvaluationMode::PrimitiveOnly,
                EvaluationMode::FixedChunkShortlex,
                EvaluationMode::LearnedRanking,
                EvaluationMode::ShuffledControl,
            ] {
                result_records.push(run_completion_mode(EvaluationRun {
                    metadata: EvaluationMeta {
                        mode,
                        repetition,
                        seed,
                        spec,
                        library,
                    },
                    session: &evaluation_session,
                    fit: &fit,
                    model_id: &identity,
                    graph_text: &graph_text,
                    model_payload_bytes,
                })?);
            }
        }
    }
    std::fs::create_dir(&output_dir)
        .map_err(|error| format!("measurement output must be fresh: {error}"))?;
    let training_path = output_dir.join("training.jsonl");
    write_json_lines(&training_path, &training_records)?;
    let results_path = output_dir.join("results.jsonl");
    write_json_lines(&results_path, &result_records)?;

    let state_bytes = optimizer_state_bytes(&fit.state);
    let model = json!({
        "schema": "tchunk-ranking-model/1",
        "identity": hex(&identity),
        "source_commit": source_commit,
        "source_tree_clean": true,
        "toolchain": {
            "rustc": rustc_version,
            "cargo": cargo_version
        },
        "criteria_blake3": hex(blake3::hash(FROZEN_CRITERIA).as_bytes()),
        "corpus_blake3": hex(blake3::hash(FROZEN_CORPUS).as_bytes()),
        "feature_revision": chunks::FEATURE_REVISION,
        "graph_text": graph_text,
        "graph_id": hex(&fit.graph.id()),
        "graph_shape": graph_counts,
        "loss_graph_text": loss_text,
        "loss_graph_id": hex(&fit.loss_graph.id()),
        "loss_graph_shape": loss_counts,
        "parameter_snapshot": {
            "id": hex(&fit.snapshot.id()),
            "graph": hex(&fit.snapshot.graph()),
            "layout": hex(&fit.snapshot.layout()),
            "parameter_bytes": hex(&parameter_bytes),
            "parameter_byte_count": parameter_bytes.len()
        },
        "optimizer_state": {
            "digest": hex(blake3::hash(&state_bytes).as_bytes()),
            "provenance_digest": hex(&fit.provenance.optimizer_state),
            "bytes": hex(&state_bytes),
            "byte_count": state_bytes.len()
        },
        "training_provenance": {
            "subject": format!("{:?}", fit.provenance.subject),
            "graph": hex(&fit.provenance.graph),
            "loss_graph": hex(&fit.provenance.loss),
            "data": hex(&fit.provenance.data),
            "tools": fit.provenance.tools,
            "selected_seed": format!("0x{:016x}", fit.selected_seed),
            "steps": fit.provenance.steps,
            "optimizer": "adam/0",
            "derivative_policy": "grad::admit default exact-real; no boolean/index surrogate",
            "derivative_options": format!("{:?}", fit.provenance.deriv),
            "learning_rate_bits": format!("0x{:016x}", fit.provenance.optimizer.lr.to_bits()),
            "beta1_bits": format!("0x{:016x}", fit.provenance.optimizer.beta1.to_bits()),
            "beta2_bits": format!("0x{:016x}", fit.provenance.optimizer.beta2.to_bits()),
            "epsilon_bits": format!("0x{:016x}", fit.provenance.optimizer.eps.to_bits()),
            "validation_loss_bits": format!("0x{:08x}", fit.validation_loss.to_bits())
        },
        "selected_seed_summaries": fit.seeds.iter().map(|seed| json!({
            "seed": format!("0x{:016x}", seed.seed),
            "validation_loss_bits": format!("0x{:08x}", seed.validation_loss.to_bits()),
            "training_elapsed_ns": seed.training_elapsed_ns,
            "steps": seed.steps,
            "optimizer_state_digest": hex(&seed.optimizer_state_digest),
            "optimizer_state_bytes": seed.optimizer_state_bytes,
            "parameter_bytes": seed.parameter_bytes,
            "provenance_data": hex(&seed.provenance_data)
        })).collect::<Vec<_>>(),
        "contexts": contexts,
        "runtime_payload_bytes": model_payload_bytes
    });
    let model_path = output_dir.join("model.json");
    std::fs::write(
        model_path,
        serde_json::to_vec_pretty(&model).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let training_elapsed_ns = fit.training_elapsed_ns;
    let optimizer_training_ns: u64 = fit
        .seeds
        .iter()
        .map(|seed| seed.training_elapsed_ns)
        .fold(0, u64::saturating_add);
    let selection = json!({
        "schema": "tchunk-ranking-selection/1",
        "selected_seed": format!("0x{:016x}", fit.selected_seed),
        "selected_validation_loss_bits": format!("0x{:08x}", fit.validation_loss.to_bits()),
        "training_point_count": fit.training_rows.rows.len(),
        "validation_point_count": fit.validation_rows.rows.len(),
        "training_task_count": 280,
        "validation_task_count": 80,
        "training_feature_collection_ns": fit.training_rows.feature_collection_ns,
        "training_teacher_label_ns": fit.training_rows.target_label_ns,
        "validation_feature_collection_ns": fit.validation_rows.feature_collection_ns,
        "validation_teacher_label_ns": fit.validation_rows.target_label_ns,
        "total_training_elapsed_ns": training_elapsed_ns,
        "optimizer_training_elapsed_ns": optimizer_training_ns,
        "model_selection_elapsed_ns": fit.model_selection_elapsed_ns,
        "optimizer_step_count": fit.seeds.iter().map(|seed| seed.steps).sum::<u64>(),
        "train_scratch_peak_bytes": fit.train_scratch_peak_bytes,
        "train_evaluated_nodes": fit.train_evaluated_nodes,
        "training_memory": {
            "actual_peak_retained_bytes": null,
            "optimizer_state_bytes": state_bytes.len(),
            "parameter_bytes": parameter_bytes.len(),
            "scratch_peak_bytes": fit.train_scratch_peak_bytes,
            "unmeasured_reason": "process or allocator retained-memory peaks are not instrumented"
        },
        "training_cost_amortization_count": 0,
        "amortization_assumption": "none; offline training and model-selection costs stay separate from inference",
        "snapshot_selection": "lowest-validation-mean-loss; exact-f32-loss-ties-choose-lowest-seed",
        "seeds": fit.seeds.iter().map(|seed| json!({
            "seed": format!("0x{:016x}", seed.seed),
            "validation_loss_bits": format!("0x{:08x}", seed.validation_loss.to_bits()),
            "elapsed_ns": seed.training_elapsed_ns,
            "steps": seed.steps,
            "optimizer_state_bytes": seed.optimizer_state_bytes,
            "parameter_bytes": seed.parameter_bytes
        })).collect::<Vec<_>>()
    });
    let selection_path = output_dir.join("selection.json");
    std::fs::write(
        selection_path,
        serde_json::to_vec_pretty(&selection).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;

    let receipt = json!({
        "schema": "tchunk-ranking-run/1",
        "measurement_performed": true,
        "criteria_blake3": hex(blake3::hash(FROZEN_CRITERIA).as_bytes()),
        "corpus_blake3": hex(blake3::hash(FROZEN_CORPUS).as_bytes()),
        "source_base": "272a41f472a46792430d3bd3e531a7c1fa295008",
        "source_commit": source_commit,
        "model_identity": hex(&identity),
        "task_count": 480,
        "train_tasks": 280,
        "validation_tasks": 80,
        "test_tasks": 120,
        "paired_repetitions": PAIRED_SEEDS.len(),
        "test_rows": result_records.len(),
        "training_rows": training_records.len(),
        "modes": ["primitive-only", "fixed-chunk-shortlex", "learned-ranking", "splitmix64-shuffled-control"],
        "model_load": {
            "cold_ns": cold_model_load_ns,
            "warm_ns_samples": warm_model_load_samples,
            "warm_ns_median": warm_model_load_median
        },
        "training_summary": selection,
        "training_labeling": {
            "teacher_assisted_training_labels": true,
            "teacher_calls": fit.training_rows.rows.len().saturating_sub(280)
                .saturating_add(fit.validation_rows.rows.len().saturating_sub(80)),
            "inference_student_only": true,
            "teacher_calls_in_test_inference": 0,
            "student_only_learned_inference_rows": 1200,
            "teacher_assisted_completion_rows": 0,
            "verification_cost": fit.training_rows.target_label_ns.saturating_add(fit.validation_rows.target_label_ns)
        },
        "scope": {
            "library_growth_comparison": "not measured; all ranking modes use the exact same frozen library",
            "selection_only": true,
            "proof_or_completeness_claim": false,
            "sparse_compute_claim": false,
            "compiled_select_claim": "selects already-computed values; does not skip branch computation"
        },
        "artifacts": {
            "model": "model.json",
            "training": "training.jsonl",
            "selection": "selection.json",
            "results": "results.jsonl"
        }
    });
    std::fs::write(
        output_dir.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    println!(
        "tchunk ranking measurement retained: {}",
        output_dir.display()
    );
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
