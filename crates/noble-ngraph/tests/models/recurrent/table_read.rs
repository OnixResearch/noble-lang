//! Finite, one-hot-key table read after the accumulator. This is a distinct
//! checked F32Index transition, not a host-side lookup posing as a graph.

use crate::common;
use noble_ngraph::check::{self, CheckedGraph};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::ir::{
    AffineOperand, Fact, Graph, GraphBuilder, Literal, Op, OutOfRange, Provenance, ScalarType,
    SubProfile, Subject,
};
use noble_ngraph::sem::Value;
use noble_ngraph::snapshot::ParamSnapshot;

const KEYS: [&str; 15] = [
    "", "A", "B", "AA", "AB", "BA", "BB", "AAA", "AAB", "ABA", "ABB", "BAA", "BAB",
    "BBA", "BBB",
];
const CELLS: usize = 19; // state, numeric index, 17 one-hot selectors (-1, 0..14, 15).
const MAX_STEPS: usize = 64;

fn key_index(key: &str) -> Option<usize> {
    if key.len() > 3 || !key.bytes().all(|byte| byte == b'A' || byte == b'B') {
        return None;
    }
    let mut suffix = 0usize;
    for byte in key.bytes() {
        suffix = 2 * suffix + usize::from(byte == b'B');
    }
    Some((1 << key.len()) - 1 + suffix)
}

fn source(state: u32, key: &str) -> Option<[u32; 2]> {
    if state > 1024 || key_index(key).is_none() {
        return None;
    }
    let hit = match key {
        "A" => 3,
        "BB" => 5,
        "ABA" => 9,
        _ => 0,
    };
    Some([state + hit, hit])
}

fn range(lo: f32, hi: f32) -> Fact {
    Fact::Range {
        lo: Literal::f32(lo),
        hi: Literal::f32(hi),
    }
}

fn graph() -> Graph {
    let mut b = GraphBuilder::new(
        SubProfile::F32,
        Subject::authored(&common::IDS, "recurrent-weights-table-read-one-hot-v1"),
    );
    let state = b.input(ScalarType::F32);
    let index = b.input(ScalarType::F32);
    let selectors: Vec<_> = (0..17).map(|_| b.input(ScalarType::F32)).collect();
    b.fact(state, Fact::Integral, Provenance::InputValidated);
    b.fact(state, range(0.0, 1024.0), Provenance::InputValidated);
    b.fact(index, Fact::Integral, Provenance::InputValidated);
    b.fact(index, range(-1.0, 15.0), Provenance::InputValidated);
    b.fact(
        selectors[0],
        Fact::OneHot {
            group: selectors.clone(),
            allow_zero: false,
        },
        Provenance::InputValidated,
    );
    // The checked graph itself binds the numeric F32Index operand to the
    // one-hot key; neither the exporter nor the host may select a different row.
    let terms: Vec<_> = selectors
        .iter()
        .enumerate()
        .map(|(position, &bit)| ((position as i32 - 1) as f32, AffineOperand::Node(bit)))
        .collect();
    let encoded = b.affine(0.0, &terms);
    let difference = b.op(Op::F32Sub(index, encoded));
    b.guard(range(0.0, 0.0), difference);
    let zero = b.f32_const(0.0);
    let three = b.f32_const(3.0);
    let five = b.f32_const(5.0);
    let nine = b.f32_const(9.0);
    let mut table = vec![zero; KEYS.len()];
    table[1] = three; // "A"
    table[6] = five; // "BB"
    table[9] = nine; // "ABA"
    let hit = b.op(Op::F32Index {
        index,
        table,
        below: OutOfRange::Zero,
        above: OutOfRange::Zero,
    });
    let next = b.op(Op::F32Add(state, hit));
    b.output(next);
    b.output(hit);
    b.finish()
}

struct Candidate {
    checked: CheckedGraph,
    compiled: Compiled,
    snapshot: ParamSnapshot,
}

impl Candidate {
    fn new(graph: Graph) -> Self {
        let checked = check::check(&common::IDS, graph).expect("admit the F32Index graph");
        let snapshot = ParamSnapshot::new(&common::IDS, &checked, &[]).expect("empty snapshot");
        let compiled = Compiled::new(&common::IDS, &checked);
        Self { checked, compiled, snapshot }
    }

    fn outcomes(&self, cells: &[Value]) -> (Obs, Obs) {
        let compiled = self.compiled.evaluate(cells, Some(&self.snapshot), &mut Control::default());
        let reference = eval::reference(&self.checked, cells, Some(&self.snapshot), &mut Control::default());
        (compiled.obs, reference.obs)
    }

    fn step(&self, state: u32, index: i32) -> [u32; 2] {
        let mut cells = [Value::f32(0.0); CELLS];
        cells[0] = Value::f32(state as f32);
        cells[1] = Value::f32(index as f32);
        cells[(index + 3) as usize] = Value::f32(1.0);
        let (compiled, reference) = self.outcomes(&cells);
        let (Obs::Normal(actual), Obs::Normal(expected)) = (compiled, reference) else {
            panic!("valid indexed transition refused");
        };
        assert_eq!(actual, expected, "compiled/reference F32Index disagreement");
        let [Value::F32(next), Value::F32(hit)] = actual.as_slice() else {
            panic!("wrong table-read output ABI");
        };
        let next = f32::from_bits(*next);
        let hit = f32::from_bits(*hit);
        assert!(next.is_finite() && hit.is_finite() && next.fract() == 0.0 && hit.fract() == 0.0);
        [next as u32, hit as u32]
    }
}

fn rows(candidate: &Candidate) -> (String, String) {
    let tiny = (-1..=15)
        .map(|index| {
            let key = if (0..15).contains(&index) {
                format!("\"{}\"", KEYS[index as usize])
            } else {
                "null".to_owned()
            };
            let [next, hit] = candidate.step(0, index);
            if (0..15).contains(&index) {
                assert_eq!(Some([next, hit]), source(0, KEYS[index as usize]));
            } else {
                assert_eq!([next, hit], [0, 0], "Zero fallback");
            }
            format!("[{key},{index},0,{next},{hit}]")
        })
        .collect::<Vec<_>>()
        .join(",");
    let mut state = 0;
    let free = (0..MAX_STEPS)
        .map(|step| {
            let key = KEYS[(step * 7 + step / 4) % KEYS.len()];
            let index = key_index(key).expect("domain key");
            let [next, hit] = candidate.step(state, index as i32);
            assert_eq!(Some([next, hit]), source(state, key));
            let row = format!("[\"{key}\",{index},{state},{next},{hit}]");
            state = next;
            row
        })
        .collect::<Vec<_>>()
        .join(",");
    (tiny, free)
}

fn node(op: &Op) -> String {
    match op {
        Op::Input(slot) => format!("[\"input\",{slot}]"),
        Op::Const(Literal::F32(bits)) => format!("[\"const\",{bits}]"),
        Op::F32Affine { bias, terms } => {
            let terms = terms.iter().map(|term| match term.operand {
                AffineOperand::Node(id) => format!("[{},{}]", term.coefficient, id.0),
                AffineOperand::One => panic!("table query must be one-hot cells"),
            }).collect::<Vec<_>>().join(",");
            format!("[\"affine\",{bias},[{terms}]]")
        }
        Op::F32Sub(left, right) => format!("[\"sub\",{},{}]", left.0, right.0),
        Op::F32Add(left, right) => format!("[\"add\",{},{}]", left.0, right.0),
        Op::Guard { fact: Fact::Range { lo: Literal::F32(lo), hi: Literal::F32(hi) }, value }
            if *lo == 0 && *hi == 0 => format!("[\"guard_zero\",{}]", value.0),
        Op::F32Index { index, table, below: OutOfRange::Zero, above: OutOfRange::Zero } => {
            let table = table.iter().map(|id| id.0.to_string()).collect::<Vec<_>>().join(",");
            format!("[\"index_zero\",{},[{table}]]", index.0)
        }
        _ => panic!("unsupported checked table-read operator: {}", op.name()),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn checked_index_tiny_near_misses_and_refusals() {
    let candidate = Candidate::new(graph());
    for (index, key) in KEYS.iter().enumerate() {
        assert_eq!(Some(candidate.step(0, index as i32)), source(0, key));
    }
    assert_eq!(candidate.step(0, -1), [0, 0]);
    assert_eq!(candidate.step(0, 15), [0, 0]);
    for (key, neighbours) in [
        ("A", &["", "AA", "AB", "BA"][..]),
        ("BB", &["B", "ABB", "BBA", "BBB"][..]),
        ("ABA", &["AB", "BA"][..]),
    ] {
        let hit = candidate.step(0, key_index(key).expect("declared key") as i32)[1];
        assert!(hit > 0);
        for neighbour in neighbours {
            let observed = candidate.step(0, key_index(neighbour).expect("near miss") as i32)[1];
            assert_eq!(observed, 0, "near-miss key must use the not-found zero");
            assert_ne!(hit, observed);
        }
    }
    let (_, free) = rows(&candidate);
    assert_eq!(free.split("],[").count(), MAX_STEPS);
    assert_eq!(key_index("ABAB"), None);
    assert_eq!(key_index("X"), None);
    let mut cells = [Value::f32(0.0); CELLS];
    cells[2] = Value::f32(1.0); // index -1, valid Zero fallback
    for invalid in [0.5, f32::NAN, -2.0, 16.0, 16_777_216.0] {
        cells[1] = Value::f32(invalid);
        let (compiled, reference) = candidate.outcomes(&cells);
        assert!(matches!(compiled, Obs::Refused(_)) && matches!(reference, Obs::Refused(_)));
    }
    cells[1] = Value::f32(-1.0);
    cells[2] = Value::f32(0.5);
    let (compiled, reference) = candidate.outcomes(&cells);
    assert!(matches!(compiled, Obs::Refused(_)) && matches!(reference, Obs::Refused(_)));
    cells[2] = Value::f32(1.0);
    cells[3] = Value::f32(1.0); // two-hot, not a declared key
    let (compiled, reference) = candidate.outcomes(&cells);
    assert!(matches!(compiled, Obs::Refused(_)) && matches!(reference, Obs::Refused(_)));
    cells[3] = Value::f32(0.0);
    cells[1] = Value::f32(0.0); // integral but disagrees with selector -1
    let (compiled, reference) = candidate.outcomes(&cells);
    assert!(matches!(compiled, Obs::Refused(_)) && matches!(reference, Obs::Refused(_)));
}

#[test]
#[ignore = "emits admitted graph and source vectors for selected offline ORT CPU pilot"]
fn table_read_export() {
    let graph = graph();
    let candidate = Candidate::new(graph.clone());
    let (tiny, free) = rows(&candidate);
    let nodes = graph.nodes.iter().map(|entry| node(&entry.op)).collect::<Vec<_>>().join(",");
    let outputs = graph.outputs.iter().map(|id| id.0.to_string()).collect::<Vec<_>>().join(",");
    let canonical = noble_ngraph::format::graph::print(candidate.checked.graph());
    println!("RECURRENT_TABLE_EXPORT\t{{\"schema\":\"noble.recurrent-table-export/v1\",\"program\":\"table-read\",\"codec\":\"binary32-exact/one-hot-ascii-ab-0..3/v1\",\"graph_id\":\"{}\",\"snapshot_id\":\"{}\",\"snapshot_bytes\":\"{}\",\"canonical_graph\":\"{}\",\"query_shape\":\"one-hot-17\",\"index_range\":[-1,15],\"max_steps\":64,\"slots\":19,\"nodes\":[{nodes}],\"outputs\":[{outputs}],\"vectors\":{{\"tiny\":[{tiny}],\"free_running\":[{free}]}}}}",
        hex(&candidate.checked.id()), hex(&candidate.snapshot.id()), hex(candidate.snapshot.bytes()),
        hex(canonical.as_bytes()));
}
