//! Design D3 (NGRAPH-CASE-14): every identity, observation and training
//! result that the pre-cutover experiment recorded at 66eb7cd is reproduced
//! byte for byte by `noble-ngraph` with the injected BLAKE3 scheme. The
//! authored identities are also cross-checked against the Phase B receipt
//! (`verification/ngraph/phase-b-core.json`).

use std::collections::BTreeMap;

use noble_ngraph::eval::{f32_inputs, reference, Control};
use noble_ngraph::ir::hex;
use noble_ngraph::train::{self, TrainControl};
use noble_ngraph::{check, Obs, Value};
use noble_ngraph_experiment::scheme::BLAKE3;
use noble_ngraph_experiment::{differential, fixtures, rejections, suites};

const RECORD: &str = include_str!("fixtures/identities-66eb7cd.txt");

/// Receipt values (phase-b-core.json): F1, the B5 data digest, S₀ and the
/// final B5 snapshot.
const F1_RECEIPT: &str = "6065e93ff4262398603bd7c25392b6461335dbc24867024f25910cee0758d661";
const B5_DATA_RECEIPT: &str = "7c40293c13b0089020b0a66ffbccf2d523be6171887ede799f38ee4d728f0692";
const B5_S0_RECEIPT: &str = "a912b177c248ed35d5d7b5943e08e2efec7466623dffc385c6a8ab77f8998046";
const B5_FINAL_RECEIPT: &str = "075f58cadc9a3abc91161b4224fd48bf9426af0ed8660851670e0ff0536f7f9d";

fn record(kind: &str) -> Vec<&'static str> {
    RECORD
        .lines()
        .filter_map(|l| l.strip_prefix(kind).and_then(|r| r.strip_prefix(' ')))
        .collect()
}

fn named(kind: &str) -> BTreeMap<&'static str, &'static str> {
    record(kind)
        .into_iter()
        .map(|l| l.rsplit_once(" => ").expect("NAME => VALUE"))
        .collect()
}

fn obs_text(o: &Obs) -> String {
    match o {
        Obs::Normal(vs) => {
            let items: Vec<String> = vs
                .iter()
                .map(|v| match v {
                    Value::F32(b) => format!("f32:0x{b:08x}"),
                    Value::I64(x) => format!("i64:{x}"),
                    Value::Bool(x) => format!("bool:{x}"),
                })
                .collect();
            format!("Normal[{}]", items.join(","))
        }
        Obs::Refused(d) => format!("Refused({})", d.code_string()),
        Obs::GuardFailed { node, fact } => format!("GuardFailed(%{},{})", node.0, fact.kind_name()),
        Obs::Exhausted(k) => format!("Exhausted({})", k.name()),
        Obs::Cancelled => "Cancelled".into(),
    }
}

#[test]
fn graph_identities_and_fact_bindings_match_the_record() {
    let recorded: BTreeMap<&str, (&str, &str)> = record("graph")
        .into_iter()
        .map(|l| {
            let mut t = l.split(' ');
            let (n, id, facts) = (t.next().unwrap(), t.next().unwrap(), t.next().unwrap());
            (n, (id, facts))
        })
        .collect();
    let corpus = suites::identity_corpus();
    assert_eq!(
        corpus.len(),
        recorded.len(),
        "every recorded graph is rebuilt"
    );
    for (name, g) in corpus {
        let (id, facts) = recorded
            .get(name.as_str())
            .unwrap_or_else(|| panic!("{name} recorded"));
        let c = check(&BLAKE3, g).unwrap_or_else(|d| panic!("{name}: {d}"));
        assert_eq!(hex(&c.id()), *id, "{name} GraphLogicalId");
        assert_eq!(
            hex(&c.fact_bindings(&BLAKE3)),
            *facts,
            "{name} fact bindings"
        );
    }
    assert_eq!(recorded["F1"].0, F1_RECEIPT);
}

#[test]
fn b2_vector_observations_and_b3_codes_match_the_record() {
    let vectors = named("vector");
    let built = fixtures::b2_vectors();
    assert_eq!(built.len(), vectors.len());
    for v in built {
        let c = check(&BLAKE3, v.graph).expect("vector graph");
        let r = reference(&c, &f32_inputs(&v.inputs), None, &mut Control::default());
        assert_eq!(obs_text(&r.obs), vectors[v.name], "{}", v.name);
    }
    let codes = named("rejection");
    let observed = rejections::run_all();
    assert_eq!(observed.len(), codes.len());
    for r in observed {
        assert_eq!(r.observed, codes[r.name], "{}", r.name);
    }
}

#[test]
fn seeded_campaigns_match_the_record() {
    for line in record("campaign") {
        let t: Vec<&str> = line.split(' ').collect();
        let seed: u64 = t[0].parse().unwrap();
        let c = differential::campaign(seed, 48, 2_000);
        let ours = format!(
            "{seed} {} vectors={} normal={} guard_failed={} refused={} disagreements={}",
            c.graph_id, c.vectors, c.normal, c.guard_failed, c.refused, c.disagreements
        );
        assert_eq!(ours, line);
    }
}

#[test]
fn b5_training_identities_match_the_record_and_the_receipt() {
    let b5 = named_b5();
    let c = suites::b5_contract(suites::B5_SEED, suites::B5_DATA_SEED);
    let t = train::train(
        &BLAKE3,
        &c,
        None,
        suites::B5_STEPS,
        &mut TrainControl::default(),
    )
    .expect("trains");
    let mut h = blake3::Hasher::new();
    for l in &t.losses {
        h.update(&l.to_bits().to_le_bytes());
    }
    let losses = format!(
        "{} count={} first=0x{:016x} last=0x{:016x}",
        h.finalize().to_hex(),
        t.losses.len(),
        t.losses[0].to_bits(),
        t.losses.last().unwrap().to_bits()
    );
    assert_eq!(hex(&train::data_digest(&BLAKE3, &c.data)), b5["data"]);
    assert_eq!(hex(&t.initial.id()), b5["s0"]);
    assert_eq!(hex(&t.snapshot.id()), b5["final"]);
    assert_eq!(hex(&t.state.digest(&BLAKE3)), b5["adam-state"]);
    assert_eq!(hex(&t.provenance.facts), b5["facts"]);
    assert_eq!(losses, b5["losses"]);
    assert_eq!(b5["data"], B5_DATA_RECEIPT);
    assert_eq!(b5["s0"], B5_S0_RECEIPT);
    assert_eq!(b5["final"], B5_FINAL_RECEIPT);
}

fn named_b5() -> BTreeMap<&'static str, &'static str> {
    record("b5")
        .into_iter()
        .map(|l| l.split_once(' ').expect("b5 KEY VALUE"))
        .collect()
}
