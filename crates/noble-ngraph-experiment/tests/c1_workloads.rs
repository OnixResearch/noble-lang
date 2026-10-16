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
