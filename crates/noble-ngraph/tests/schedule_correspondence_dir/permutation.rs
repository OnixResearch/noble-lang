#![allow(
    tigerstyle::ambient_clock,
    tigerstyle::assertion_density,
    tigerstyle::explicit_defaults,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::numeric_units,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this existing integration target's opt-in permutation controls deliberately mutate one producer field at a time and measure bounded checker resources."
)]
//! Independent real-graph/real-stream controls for the separate profile (issue #129).

use std::time::Instant;

use noble_ngraph::check;
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{GraphBuilder, Op, ScalarType, SubProfile, Subject};
use noble_ngraph::layout;
use noble_ngraph::permutation::{self, Artifact, Refusal};
use noble_ngraph::schedule::{self, I64Instruction};
use noble_ngraph::sem::Value;

fn fixture() -> (check::CheckedGraph, Artifact) {
    let (checked, _, _) = super::base::packed(super::base::interleaved());
    let compiler = Compiled::new(&super::common::IDS, &checked);
    let artifact = permutation::from_compiled(&checked, &compiler).expect("total graph");
    assert!(permutation::check(&checked, &artifact).is_ok());
    (checked, artifact)
}

fn swap_pair(artifact: &mut Artifact, first: usize, second: usize) {
    artifact.stream.swap(first, second);
    artifact.witness.swap(first, second);
}

pub(super) fn variants() -> (check::CheckedGraph, [Artifact; 3]) {
    let (checked, canonical) = fixture();
    assert_eq!(canonical.witness, [3, 4, 5, 6, 7]);
    assert_eq!(canonical.layout.layers[1], (0, 2));
    assert_eq!(canonical.layout.layers[2], (2, 4));
    let mut first = canonical.clone();
    swap_pair(&mut first, 0, 1);
    let mut second = canonical.clone();
    swap_pair(&mut second, 2, 3);
    (checked, [canonical, first, second])
}

#[test]
fn canonical_and_two_distinct_noncanonical_orders_check_and_execute() {
    let (checked, artifacts) = variants();
    let pairs = [
        (3, 8),
        (0, 0),
        (i64::MAX, i64::MIN),
        (-1, 1),
        (i64::MIN, -1),
    ];
    for artifact in &artifacts {
        let accepted = permutation::check(&checked, artifact).expect("own complete witness");
        let bytes = permutation::encode(artifact).expect("bounded export");
        let parsed = permutation::decode(&bytes).expect("real byte roundtrip");
        assert_eq!(&parsed, artifact);
        assert!(permutation::check(&checked, &parsed).is_ok());
        for (left, right) in pairs {
            let inputs = [Value::I64(left), Value::I64(right)];
            let actual = permutation::execute(
                accepted,
                &inputs,
                checked.graph().nodes.len() as u64,
                u64::from(artifact.layout.regs) * 8,
            )
            .expect("enough fuel and scratch");
            let sum = left.wrapping_add(right);
            let delta = right.wrapping_sub(left);
            let expected = vec![
                Value::Bool(sum.wrapping_mul(-3) == delta.wrapping_mul(-3)),
                Value::I64(sum),
            ];
            assert_eq!(actual, expected, "independent wrapping-word expectation");
            assert_eq!(
                eval::reference(&checked, &inputs, None, &mut Control::default()).obs,
                Obs::Normal(expected),
                "reference is supporting execution evidence"
            );
        }
    }
    assert_ne!(artifacts[0].stream, artifacts[1].stream);
    assert_ne!(artifacts[0].stream, artifacts[2].stream);
    assert_ne!(artifacts[1].stream, artifacts[2].stream);
    // Neither existing canonical acceptance boundary is broadened.
    for permutation in artifacts.iter().skip(1) {
        assert_eq!(
            schedule::check(&checked, &permutation.layout, &permutation.stream),
            Err(schedule::StreamRefusal::BodyMismatch)
        );
    }
}

#[test]
fn witnessed_bijection_layers_and_operation_bodies_are_not_self_attested() {
    let (checked, [canonical, reordered, _]) = variants();
    let mut wrong_node = canonical.clone();
    wrong_node.witness.swap(0, 1);
    assert!(matches!(
        permutation::check(&checked, &wrong_node),
        Err(Refusal::BodyMismatch)
    ));
    let mut invalid_node = canonical.clone();
    invalid_node.witness[0] = u32::MAX;
    assert!(matches!(
        permutation::check(&checked, &invalid_node),
        Err(Refusal::WrongWitness)
    ));
    let mut duplicate = canonical.clone();
    duplicate.stream[1] = duplicate.stream[0];
    duplicate.witness[1] = duplicate.witness[0];
    assert_eq!(
        0i64.wrapping_add(0),
        0i64.wrapping_sub(0),
        "the duplicate mutation agrees on a sampled input but remains invalid"
    );
    assert!(matches!(
        permutation::check(&checked, &duplicate),
        Err(Refusal::DuplicateNode)
    ));
    let mut omitted = canonical.clone();
    omitted.stream.pop();
    omitted.witness.pop();
    assert!(matches!(
        permutation::check(&checked, &omitted),
        Err(Refusal::MissingNode)
    ));
    let mut unpaired = canonical.clone();
    unpaired.stream.pop();
    assert!(matches!(
        permutation::check(&checked, &unpaired),
        Err(Refusal::Shape)
    ));
    let mut cross_layer = reordered;
    swap_pair(&mut cross_layer, 1, 2);
    assert!(matches!(
        permutation::check(&checked, &cross_layer),
        Err(Refusal::CrossLayer)
    ));
    let mut early = canonical.clone();
    if let I64Instruction::Add(dst, _, right) = early.stream[0] {
        early.stream[0] = I64Instruction::Add(dst, canonical.layout.reg[5], right);
    }
    assert!(matches!(
        permutation::check(&checked, &early),
        Err(Refusal::EarlyRead)
    ));
    let mut double_writer = canonical.clone();
    if let I64Instruction::Sub(_, a, b) = double_writer.stream[1] {
        double_writer.stream[1] = I64Instruction::Sub(canonical.layout.reg[3], a, b);
    }
    assert!(matches!(
        permutation::check(&checked, &double_writer),
        Err(Refusal::DuplicateDestination)
    ));
    let mut swapped_sub = canonical.clone();
    if let I64Instruction::Sub(dst, a, b) = swapped_sub.stream[1] {
        swapped_sub.stream[1] = I64Instruction::Sub(dst, b, a);
    }
    assert!(matches!(
        permutation::check(&checked, &swapped_sub),
        Err(Refusal::BodyMismatch)
    ));
    let mut changed_const = canonical.clone();
    changed_const.layout.consts[0].1 ^= 1;
    assert!(matches!(
        permutation::check(&checked, &changed_const),
        Err(Refusal::Layout(layout::Refusal::ConstantTable))
    ));
    let mut changed_slot = canonical.clone();
    changed_slot.layout.slot_regs[0].1 ^= 1;
    assert!(matches!(
        permutation::check(&checked, &changed_slot),
        Err(Refusal::Layout(layout::Refusal::SlotTable))
    ));
    let mut changed_out = canonical.clone();
    changed_out.output_regs.swap(0, 1);
    assert!(matches!(
        permutation::check(&checked, &changed_out),
        Err(Refusal::OutputMismatch)
    ));
    let mut substituted = canonical.clone();
    substituted.graph_id[0] ^= 1;
    assert!(matches!(
        permutation::check(&checked, &substituted),
        Err(Refusal::GraphIdentity)
    ));
}

#[test]
fn replay_binds_all_revisions_and_independent_subjects() {
    let (checked, canonical) = fixture();
    let ids = &super::common::IDS;
    let baseline = permutation::identities(ids, &checked, &canonical);
    assert!(permutation::replay(
        ids,
        &checked,
        &canonical,
        &baseline,
        permutation::MAX_CHECK_WORK
    )
    .is_ok());
    let mut candidates = Vec::new();
    let mut changed = canonical.clone();
    changed.stream.swap(0, 1);
    candidates.push(changed);
    let mut changed = canonical.clone();
    changed.witness.swap(0, 1);
    candidates.push(changed);
    let mut changed = canonical.clone();
    changed.layout.reg.swap(3, 4);
    candidates.push(changed);
    let mut changed = canonical.clone();
    changed.output_regs.swap(0, 1);
    candidates.push(changed);
    let mut changed = canonical.clone();
    changed.profile.push('x');
    candidates.push(changed);
    let mut changed = canonical.clone();
    changed.semantic_revision.push('x');
    candidates.push(changed);
    let mut changed = canonical.clone();
    changed.checker_revision.push('x');
    candidates.push(changed);
    let mut changed = canonical.clone();
    changed.model_revision.push('x');
    candidates.push(changed);
    let mut changed = canonical.clone();
    changed.export_revision.push('x');
    candidates.push(changed);
    let mut changed = canonical.clone();
    changed.layout.layout_key[0] ^= 1;
    candidates.push(changed);
    for (index, changed) in candidates.iter().enumerate() {
        assert!(
            matches!(
                permutation::replay(
                    ids,
                    &checked,
                    changed,
                    &baseline,
                    permutation::MAX_CHECK_WORK
                ),
                Err(Refusal::StaleReplay)
            ),
            "subject {index}"
        );
    }
    for mutate in [
        |artifact: &mut Artifact| artifact.profile.push('x'),
        |artifact: &mut Artifact| artifact.semantic_revision.push('x'),
        |artifact: &mut Artifact| artifact.checker_revision.push('x'),
        |artifact: &mut Artifact| artifact.model_revision.push('x'),
        |artifact: &mut Artifact| artifact.export_revision.push('x'),
    ] {
        let mut changed = canonical.clone();
        mutate(&mut changed);
        assert!(matches!(
            permutation::check(&checked, &changed),
            Err(Refusal::StaleIdentity)
        ));
    }
    let mut replaced_graph = super::base::interleaved();
    replaced_graph.outputs.swap(0, 1);
    let other = check::check(ids, replaced_graph).expect("a differently ordered graph");
    assert!(matches!(
        permutation::replay(
            ids,
            &other,
            &canonical,
            &baseline,
            permutation::MAX_CHECK_WORK
        ),
        Err(Refusal::StaleReplay)
    ));
}

#[test]
fn bounded_codec_and_check_distinguish_malformed_unsupported_and_exhausted() {
    let (checked, canonical) = fixture();
    let work = permutation::declared_work(&checked, &canonical);
    assert!(permutation::check_within(&checked, &canonical, work).is_ok());
    assert!(matches!(
        permutation::check_within(&checked, &canonical, work - 1),
        Err(Refusal::Exhausted { .. })
    ));
    let bytes = permutation::encode(&canonical).expect("export");
    assert!(matches!(
        permutation::encode_within(&canonical, bytes.len() as u64 - 1),
        Err(Refusal::Exhausted { .. })
    ));
    assert!(matches!(
        permutation::decode_within(&bytes, bytes.len() as u64 - 1),
        Err(Refusal::Exhausted { .. })
    ));
    assert!(matches!(
        permutation::decode(&bytes[..bytes.len() - 1]),
        Err(Refusal::Malformed)
    ));
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(matches!(
        permutation::decode(&trailing),
        Err(Refusal::Malformed)
    ));
    let mut bad_magic = bytes.clone();
    bad_magic[0] = b'?';
    assert!(matches!(
        permutation::decode(&bad_magic),
        Err(Refusal::Unsupported)
    ));
    let mut bad_utf8 = bytes.clone();
    bad_utf8[12] = 255;
    assert!(matches!(
        permutation::decode(&bad_utf8),
        Err(Refusal::Malformed)
    ));
    let mut guard = canonical.clone();
    guard.stream[0] = I64Instruction::Guard { node: 3 };
    assert!(matches!(
        permutation::check(&checked, &guard),
        Err(Refusal::Unsupported)
    ));
    assert!(matches!(
        permutation::encode(&guard),
        Err(Refusal::Unsupported)
    ));
    let mut builder = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&super::common::IDS, "permutation-f32-refusal"),
    );
    let constant = builder.f32_const(0.5);
    builder.output(constant);
    let other = check::check(&super::common::IDS, builder.finish()).expect("F32 graph");
    assert!(matches!(
        permutation::check(&other, &canonical),
        Err(Refusal::Unsupported)
    ));
}

#[test]
fn repeated_outputs_and_budget_preflight_preserve_observation() {
    let mut graph = super::base::interleaved();
    let output = graph.outputs[0];
    graph.outputs.push(output);
    let checked = check::check(&super::common::IDS, graph).expect("repeated output graph");
    let compiler = Compiled::new(&super::common::IDS, &checked);
    let artifact = permutation::from_compiled(&checked, &compiler).expect("total graph");
    assert_eq!(artifact.output_regs[0], artifact.output_regs[2]);
    let accepted = permutation::check(&checked, &artifact).expect("repeated output retained");
    let args = [Value::I64(3), Value::I64(8)];
    let actual = permutation::execute(
        accepted,
        &args,
        checked.graph().nodes.len() as u64,
        u64::from(artifact.layout.regs) * 8,
    )
    .expect("sufficient budgets");
    assert_eq!(actual[0], actual[2]);
    assert_eq!(
        compiler.evaluate(&args, None, &mut Control::default()).obs,
        Obs::Normal(actual)
    );
    for (fuel, scratch) in [
        (checked.graph().nodes.len() as u64 - 1, u64::MAX),
        (u64::MAX, u64::from(artifact.layout.regs) * 8 - 1),
    ] {
        assert!(matches!(
            permutation::execute(accepted, &args, fuel, scratch),
            Err(permutation::RunRefusal::Exhausted)
        ));
    }
}

#[test]
fn a_constant_only_graph_needs_no_instruction_and_empty_observations_are_unsupported() {
    let ids = &super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(ids, "permutation-constant-only"),
    );
    let constant = builder.i64_const(-7);
    builder.output(constant);
    builder.output(constant);
    let checked = check::check(ids, builder.finish()).expect("constant graph checks");
    let compiled = Compiled::new(ids, &checked);
    let artifact = permutation::from_compiled(&checked, &compiled).expect("total constant graph");
    assert!(artifact.stream.is_empty());
    assert!(artifact.witness.is_empty());
    assert_eq!(artifact.output_regs, vec![artifact.layout.reg[0]; 2]);
    let accepted =
        permutation::check(&checked, &artifact).expect("empty work is a valid bijection");
    assert_eq!(
        permutation::execute(accepted, &[], 1, u64::from(artifact.layout.regs) * 8),
        Ok(vec![Value::I64(-7), Value::I64(-7)])
    );
    let empty = check::check(
        ids,
        GraphBuilder::new(SubProfile::I64, Subject::authored(ids, "permutation-empty")).finish(),
    )
    .expect("canonical graph checker accepts emptiness");
    let empty_compiled = Compiled::new(ids, &empty);
    assert!(matches!(
        permutation::from_compiled(&empty, &empty_compiled),
        Err(Refusal::Unsupported)
    ));
}

#[test]
fn checked_permutation_executes_bool_input() {
    let ids = &super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(ids, "permutation-bool-input"),
    );
    let input = builder.input(ScalarType::Bool);
    builder.output(input);
    let checked = check::check(ids, builder.finish()).expect("legal Bool input graph");
    let compiled = Compiled::new(ids, &checked);
    let artifact = permutation::from_compiled(&checked, &compiled).expect("total graph");
    let accepted = permutation::check(&checked, &artifact).expect("valid layout and empty stream");

    let args = [Value::Bool(true)];
    assert_eq!(
        eval::reference(&checked, &args, None, &mut Control::default()).obs,
        Obs::Normal(vec![Value::Bool(true)])
    );
    assert_eq!(
        permutation::execute(
            accepted,
            &args,
            checked.graph().nodes.len() as u64,
            u64::from(artifact.layout.regs) * 8,
        ),
        Ok(vec![Value::Bool(true)])
    );
}

/// Reference, canonical packed and opt-in permutation observations must agree exactly; returns
/// the shared observation.
fn three_way(checked: &check::CheckedGraph, artifact: &Artifact, args: &[Value]) -> Vec<Value> {
    let compiled = Compiled::new(&super::common::IDS, checked);
    let accepted = permutation::check(checked, artifact).expect("own complete witness");
    let actual = permutation::execute(
        accepted,
        args,
        checked.graph().nodes.len() as u64,
        u64::from(artifact.layout.regs) * 8,
    )
    .expect("admitted inputs under sufficient budgets");
    assert_eq!(
        eval::reference(checked, args, None, &mut Control::default()).obs,
        Obs::Normal(actual.clone()),
        "reference agrees with the permutation path"
    );
    assert_eq!(
        compiled.evaluate(args, None, &mut Control::default()).obs,
        Obs::Normal(actual.clone()),
        "canonical packed evaluator agrees with the permutation path"
    );
    actual
}

#[test]
fn bool_identity_inputs_agree_across_reference_canonical_and_permutation() {
    let ids = &super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(ids, "permutation-bool-identity"),
    );
    let input = builder.input(ScalarType::Bool);
    builder.output(input);
    builder.output(input);
    let checked = check::check(ids, builder.finish()).expect("legal Bool input graph");
    let artifact = permutation::from_compiled(&checked, &Compiled::new(ids, &checked))
        .expect("total Bool identity graph");
    assert!(artifact.stream.is_empty());
    for value in [true, false] {
        assert_eq!(
            three_way(&checked, &artifact, &[Value::Bool(value)]),
            vec![Value::Bool(value); 2],
            "Bool({value}) is observed as Bool({value})"
        );
    }
}

#[test]
fn mixed_i64_and_bool_slots_agree_in_canonical_and_permuted_orders() {
    let ids = &super::common::IDS;
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(ids, "permutation-mixed-bool-input"),
    );
    let left = builder.input(ScalarType::I64);
    let flag = builder.input(ScalarType::Bool);
    let right = builder.input(ScalarType::I64);
    let other = builder.input(ScalarType::Bool);
    let sum = builder.op(Op::I64Add(left, right));
    let delta = builder.op(Op::I64Sub(right, left));
    let equal = builder.op(Op::I64Eq(sum, delta));
    for output in [flag, equal, sum, other, flag] {
        builder.output(output);
    }
    let checked = check::check(ids, builder.finish()).expect("legal mixed I64/Bool graph");
    let canonical = permutation::from_compiled(&checked, &Compiled::new(ids, &checked))
        .expect("total mixed graph");
    assert_eq!(canonical.stream.len(), 3);
    let mut permuted = canonical.clone();
    swap_pair(&mut permuted, 0, 1);
    assert_ne!(permuted.stream, canonical.stream);
    for artifact in [&canonical, &permuted] {
        for (a, b) in [(true, false), (false, true), (true, true), (false, false)] {
            for (x, y) in [(3, 8), (0, 0), (i64::MAX, i64::MIN), (i64::MIN, -1)] {
                let args = [Value::I64(x), Value::Bool(a), Value::I64(y), Value::Bool(b)];
                let s = x.wrapping_add(y);
                let d = y.wrapping_sub(x);
                assert_eq!(
                    three_way(&checked, artifact, &args),
                    vec![
                        Value::Bool(a),
                        Value::Bool(s == d),
                        Value::I64(s),
                        Value::Bool(b),
                        Value::Bool(a),
                    ],
                    "independent wrapping-word and Bool expectation"
                );
            }
        }
    }
    // Staging Bool as 0/1 does not widen input admission: a mistyped slot is still refused
    // before execution, exactly as the reference and canonical evaluators refuse it.
    let accepted = permutation::check(&checked, &canonical).expect("canonical witness");
    for args in [
        [
            Value::I64(1),
            Value::I64(1),
            Value::I64(2),
            Value::Bool(false),
        ],
        [
            Value::Bool(true),
            Value::Bool(true),
            Value::I64(2),
            Value::Bool(false),
        ],
    ] {
        assert!(matches!(
            permutation::execute(accepted, &args, u64::MAX, u64::MAX),
            Err(permutation::RunRefusal::Input(_))
        ));
        assert!(matches!(
            eval::reference(&checked, &args, None, &mut Control::default()).obs,
            Obs::Refused(_)
        ));
    }
}

fn resident_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/statm").ok()?;
    text.split_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()
        .map(|pages| pages * 4096)
}

#[test]
#[ignore = "reporting-only bounded measurement; run with --ignored --nocapture cost_report"]
fn cost_report() {
    let mut builder = GraphBuilder::new(
        SubProfile::I64,
        Subject::authored(&super::common::IDS, "permutation-cost"),
    );
    let input = builder.input(ScalarType::I64);
    for _ in 0..2048 {
        let node = builder.op(Op::I64Add(input, input));
        builder.output(node);
    }
    let checked =
        check::check(&super::common::IDS, builder.finish()).expect("inside graph budgets");
    let compiled = Compiled::new(&super::common::IDS, &checked);
    let artifact = permutation::from_compiled(&checked, &compiled).expect("total graph");
    let wire = permutation::encode(&artifact).expect("bounded wire");
    let before = resident_bytes();
    let mut times = Vec::new();
    for _ in 0..9 {
        let started = Instant::now();
        assert!(permutation::check(&checked, &artifact).is_ok());
        times.push(started.elapsed().as_nanos());
    }
    times.sort_unstable();
    let rss_growth = resident_bytes()
        .unwrap_or(0)
        .saturating_sub(before.unwrap_or(0));
    let layout_bytes = artifact.layout.reg.len() * 4
        + artifact.layout.consts.len() * 12
        + artifact.layout.slot_regs.len() * 8
        + artifact.layout.layers.len() * 8
        + artifact.layout.fail_suffix_min.len() * 4;
    let retained_bytes = layout_bytes
        + artifact.stream.len() * core::mem::size_of::<I64Instruction>()
        + artifact.witness.len() * 4
        + artifact.output_regs.len() * 4;
    println!("PERM_MEASURE {{\"nodes\":{},\"instructions\":{},\"checker_median_ns\":{},\"witness_bytes\":{},\"encoded_bytes\":{},\"retained_payload_bytes\":{},\"checker_scratch_bool_bytes\":{},\"execution_scratch_bytes\":{},\"rss_growth_bytes\":{},\"samples\":9}}",
        checked.graph().nodes.len(), artifact.stream.len(), times[4],
        artifact.witness.len() * 4, wire.len(), retained_bytes,
        checked.graph().nodes.len() + usize::try_from(artifact.layout.regs).expect("u32 register count fits"),
        usize::try_from(artifact.layout.regs).expect("u32 register count fits") * 8, rss_growth);
}
