//! C1 workloads: shapes as specified, and the host-native baselines compute
//! bitwise the same observations as L3/L4 (checked before any timing).

use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{f32_inputs, i64_inputs, reference, Control};
use noble_ngraph::prng::SplitMix64;
use noble_ngraph::snapshot::ParamSnapshot;
use noble_ngraph::train::init_params;
use noble_ngraph::{check, Obs, Value};
use noble_ngraph_experiment::random::random_f32;
use noble_ngraph_experiment::scheme::BLAKE3;
use noble_ngraph_experiment::workloads::{self, W3_SIZES, W4_TABLE};

#[test]
fn w3_sizes_reach_gallery_scale_and_native_agrees() {
    let ops: Vec<usize> = W3_SIZES.iter().map(|(_, n)| n.ops()).collect();
    assert_eq!(ops, vec![76, 418, 6270]);
    assert_eq!(W3_SIZES[2].1.params(), 243);
    let mut rng = SplitMix64::new(3);
    for (name, net) in W3_SIZES {
        let g = check(&BLAKE3, net.graph(name)).expect("checks");
        assert_eq!(g.node_count(), net.inputs() + net.params() + net.ops());
        let p = init_params(7, net.params());
        let s = ParamSnapshot::new(&BLAKE3, &g, &p).expect("snapshot");
        let c = Compiled::new(&BLAKE3, &g);
        for _ in 0..50 {
            let x: Vec<f32> = (0..net.inputs()).map(|_| random_f32(&mut rng)).collect();
            let mut out = vec![0f32; net.batch];
            net.native(&p, &x, &mut out);
            let native = Obs::Normal(out.iter().map(|v| Value::f32(*v)).collect());
            let l3 = reference(&g, &f32_inputs(&x), Some(&s), &mut Control::default()).obs;
            let l4 = c
                .evaluate(&f32_inputs(&x), Some(&s), &mut Control::default())
                .obs;
            assert!(l3.obs_eq(&native) && l4.obs_eq(&native), "{name}");
        }
    }
}

#[test]
fn w4_native_agrees() {
    let g = check(&BLAKE3, workloads::w4_graph()).expect("checks");
    let c = Compiled::new(&BLAKE3, &g);
    let table: Vec<f32> = (0..W4_TABLE).map(workloads::w4_entry).collect();
    let mut rng = SplitMix64::new(4);
    for _ in 0..20 {
        let idx: Vec<f32> = (0..workloads::W4_READS)
            .map(|_| rng.below(W4_TABLE as u64) as f32)
            .collect();
        let n = workloads::w4_native(&table, &idx).expect("in range");
        let want = Obs::Normal(vec![Value::f32(n)]);
        assert!(
            reference(&g, &f32_inputs(&idx), None, &mut Control::default())
                .obs
                .obs_eq(&want)
        );
        assert!(c
            .evaluate(&f32_inputs(&idx), None, &mut Control::default())
            .obs
            .obs_eq(&want));
    }
}

#[test]
fn compiled_tables_preserve_order_rules_and_guard_precedence() {
    use noble_ngraph::ir::{AffineOperand, Fact, GraphBuilder, Op, Provenance, ScalarType, Subject, SubProfile};
    use noble_ngraph_experiment::fixtures::{range, RULES};

    for len in [1, 2, 7, 257] {
        for below in RULES {
            for above in RULES {
                let mut b = GraphBuilder::new(SubProfile::F32, Subject::authored(&BLAKE3, "table boundaries"));
                let i = b.input(ScalarType::F32);
                let a = b.input(ScalarType::F32);
                let z = b.f32_const(-0.0);
                b.fact(i, Fact::Integral, Provenance::InputValidated);
                b.fact(i, range(-1.0, 257.0), Provenance::InputValidated);
                // Affine storage on both sides of the tables catches stale
                // coefficient/source offsets, not just a constant-table case.
                let pre = b.affine(-0.0, &[(0.5, AffineOperand::Node(a))]);
                let deep = b.op(Op::F32Add(pre, pre));
                b.guard(Fact::Finite, deep);
                let table: Vec<_> = (0..len).map(|k| if k % 2 == 0 { a } else { z }).collect();
                let first = b.op(Op::F32Index { index: i, table: table.clone(), below, above });
                let second = b.op(Op::F32Index { index: i, table: table.into_iter().rev().collect(), below, above });
                // Canonically later but shallower: when both fail, the first
                // guard must still win despite the compiled layer schedule.
                b.guard(range(0.0, 0.0), a);
                let post = b.affine(-0.0, &[(2.0, AffineOperand::Node(first)), (1.0, AffineOperand::Node(second))]);
                b.output(first);
                b.output(second);
                b.output(post);
                b.output(z);
                let checked = check(&BLAKE3, b.finish()).expect("checks");
                let compiled = Compiled::new(&BLAKE3, &checked);
                let mut workspace = compiled.workspace();
                for index in [-1.0, -0.0, (len - 1) as f32, len as f32] {
                    for value in [-0.0, 0.0, f32::from_bits(1), f32::INFINITY, f32::from_bits(0xffc0_0042)] {
                        let inputs = f32_inputs(&[index, value]);
                        let reference = reference(&checked, &inputs, None, &mut Control::default());
                        let result = compiled.evaluate_in(&mut workspace, &inputs, None, &mut Control::default());
                        assert!(result.obs.obs_eq(&reference.obs), "len={len} {below:?}/{above:?} index={index:?} value={value:?}: {:?} != {:?}", result.obs, reference.obs);
                    }
                }
            }
        }
    }
}

#[test]
fn w1_w2_native_agree() {
    let g1 = check(&BLAKE3, workloads::w1_graph()).expect("checks");
    let r = reference(&g1, &i64_inputs(&[5, 4]), None, &mut Control::default()).obs;
    assert_eq!(r, Obs::Normal(vec![Value::I64(workloads::w1_native(5, 4))]));
    assert_eq!(workloads::w1_native(5, 4), 24);
    let g2 = check(&BLAKE3, workloads::w2_graph()).expect("checks");
    let c2 = Compiled::new(&BLAKE3, &g2);
    let (i, k) = (3, -7);
    let mut s = 11i64;
    for _ in 0..1000 {
        let o = c2
            .evaluate(&i64_inputs(&[s, i, k]), None, &mut Control::default())
            .obs;
        let Obs::Normal(v) = o else { panic!() };
        let Value::I64(n) = v[0] else { panic!() };
        s = n;
    }
    assert_eq!(s, workloads::w2_native(11, i, k, 1000));
}
