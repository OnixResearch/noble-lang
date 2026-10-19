#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::excessive_file_length,
    tigerstyle::explicit_defaults,
    tigerstyle::function_length,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Snapshot files (design D6, NGRAPH-SNAP-02, NGRAPH-CASE-12/13 at the
//! library level): canonical print and parse, recomputed identities, the
//! documented provenance-digest encoding, and binding refusals.

mod common;

use common::{checked, contract, linear, points, IDS};
use noble_ngraph::check::Budgets;
use noble_ngraph::format::snapshot::SnapshotFile;
use noble_ngraph::grad::BooleanPolicy;
use noble_ngraph::ir::{Digest, NodeId};
use noble_ngraph::snapshot::{layout_key_of, ParamSnapshot};
use noble_ngraph::train::{self, TrainControl};
use noble_ngraph::IdentityScheme;

/// A named edit of the recorded `adam/0` hyperparameters.
type AdamEdit = (&'static str, fn(&mut train::Adam));

fn trained(steps: u64) -> SnapshotFile {
    let c = contract(checked(linear(false, false)), points());
    let t = train::train(&IDS, &c, None, steps, &mut TrainControl::default()).expect("trains");
    SnapshotFile {
        snapshot: t.snapshot,
        provenance: t.provenance,
    }
}

fn load(text: &str) -> Result<SnapshotFile, String> {
    SnapshotFile::parse(&IDS, text, Budgets::default().snapshot_bytes).map_err(|d| d.code_string())
}

/// The D6 encoding written out independently of the library: the identity
/// encoding of every record in file order except the schema and
/// `provenance` lines, with the `param` block after `layout` and the
/// `opt-in` block after `policy`.
fn documented_digest(ids: &dyn IdentityScheme, text: &str) -> Digest {
    let records: Vec<Vec<&str>> = text
        .lines()
        .skip(1)
        .map(|l| l.split_whitespace().collect())
        .filter(|t: &Vec<&str>| !t.is_empty() && t[0] != "provenance")
        .collect();
    let of = |kind: &str| -> Vec<&Vec<&str>> { records.iter().filter(|t| t[0] == kind).collect() };
    let hexd = |s: &str| -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    };
    let bits = |s: &str| u64::from_str_radix(&s[2..], 16).unwrap();
    let mut out: Vec<u8> = b"noble.ngraph.provenance.v0\0".to_vec();
    let bytes = |out: &mut Vec<u8>, b: &[u8]| {
        out.extend_from_slice(&(b.len() as u64).to_le_bytes());
        out.extend_from_slice(b);
    };
    for t in &records {
        match t[0] {
            "snapshot" | "graph" | "optimizer-state" | "parent" | "facts" | "loss" | "data" => {
                bytes(&mut out, &hexd(t[1]));
            }
            "layout" => {
                bytes(&mut out, &hexd(t[1]));
                let params = of("param");
                out.extend_from_slice(&(params.len() as u64).to_le_bytes());
                for p in params {
                    out.extend_from_slice(&p[1].parse::<u32>().unwrap().to_le_bytes());
                    #[allow(clippy::cast_possible_truncation)]
                    out.extend_from_slice(&(bits(p[2]) as u32).to_le_bytes());
                }
            }
            "subject" => {
                bytes(&mut out, t[1].as_bytes());
                bytes(&mut out, &hexd(t[2]));
                let closure: Vec<u64> = if t[4] == "-" {
                    vec![]
                } else {
                    t[4].split(',').map(|d| d.parse().unwrap()).collect()
                };
                out.extend_from_slice(&(closure.len() as u64).to_le_bytes());
                for d in closure {
                    out.extend_from_slice(&d.to_le_bytes());
                }
            }
            "profile" => {
                bytes(&mut out, t[1].as_bytes());
                bytes(&mut out, t[2].as_bytes());
            }
            "policy" => {
                bytes(&mut out, t[1].as_bytes());
                let opts = of("opt-in");
                out.extend_from_slice(&(opts.len() as u64).to_le_bytes());
                for o in opts {
                    out.extend_from_slice(&o[1][1..].parse::<u32>().unwrap().to_le_bytes());
                    bytes(&mut out, o[2].as_bytes());
                    bytes(&mut out, o[3].as_bytes());
                    if let Some(w) = o.get(4) {
                        #[allow(clippy::cast_possible_truncation)]
                        out.extend_from_slice(&(bits(w) as u32).to_le_bytes());
                    }
                }
            }
            "seed" => out.extend_from_slice(&bits(t[1]).to_le_bytes()),
            "prng" => bytes(&mut out, t[1].as_bytes()),
            "optimizer" => {
                bytes(&mut out, t[1].as_bytes());
                for k in [3, 5, 7, 9] {
                    out.extend_from_slice(&bits(t[k]).to_le_bytes());
                }
            }
            "steps" => out.extend_from_slice(&t[1].parse::<u64>().unwrap().to_le_bytes()),
            "tools" => bytes(&mut out, t[1..].join(" ").as_bytes()),
            _ => {}
        }
    }
    let mut h = ids.start();
    h.update(&out);
    h.finish()
}

#[test]
fn printed_files_reload_with_recomputed_identities() {
    for steps in [0, 3] {
        let f = trained(steps);
        let text = f.print(&IDS);
        assert!(
            text.starts_with("noble-ngraph-snapshot/0\nsnapshot "),
            "{text}"
        );
        assert!(
            text.lines().last().unwrap().starts_with("provenance "),
            "{text}"
        );
        let back = load(&text).expect("reloads");
        assert_eq!(back, f);
        assert_eq!(back.print(&IDS), text, "printing is canonical");
    }
}

#[test]
fn the_provenance_digest_follows_the_documented_encoding() {
    let mut f = trained(2);
    f.provenance
        .deriv
        .boolean
        .insert(NodeId(9), BooleanPolicy::ClampedLinear(0.25));
    f.provenance
        .deriv
        .boolean
        .insert(NodeId(2), BooleanPolicy::StraightThrough);
    f.provenance.deriv.index.insert(
        NodeId(2),
        noble_ngraph::grad::IndexPolicy::BackwardDifference,
    );
    f.provenance.subject.closure = vec![5, 18_446_744_073_709_551_615];
    let text = f.print(&IDS);
    let recorded = text
        .lines()
        .last()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .to_owned();
    assert_eq!(
        noble_ngraph::ir::hex(&documented_digest(&IDS, &text)),
        recorded
    );
    assert_eq!(load(&text).expect("reloads"), f);
}

#[test]
fn tampered_or_malformed_files_are_refused() {
    let text = trained(3).print(&IDS);
    let swap = |from: &str, to: &str| -> String {
        assert!(text.contains(from), "{from}");
        text.replacen(from, to, 1)
    };
    let param = text
        .lines()
        .find(|l| l.starts_with("param 1 "))
        .unwrap()
        .to_owned();
    let flipped = {
        let bits = u32::from_str_radix(&param[10..], 16).unwrap() ^ 1;
        format!("param 1 0x{bits:08x}")
    };
    let seed = text
        .lines()
        .find(|l| l.starts_with("seed "))
        .unwrap()
        .to_owned();
    let lines: Vec<&str> = text.lines().collect();
    let reordered = {
        let mut l = lines.clone();
        let a = l.iter().position(|x| x.starts_with("facts ")).unwrap();
        l.swap(a, a + 1);
        l.join("\n") + "\n"
    };
    let cases = [
        (swap(&param, &flipped), "E-SNAPSHOT-INTEGRITY"),
        (
            swap(&seed, "seed 0x0000000000000001"),
            "E-SNAPSHOT-INTEGRITY",
        ),
        (swap("steps 3", "steps 4"), "E-SNAPSHOT-INTEGRITY"),
        (swap(&param, "param 1 0x7fc00000"), "E-SNAPSHOT-INTEGRITY"),
        (
            text.replacen("noble-ngraph-snapshot/0", "noble-ngraph-snapshot/1", 1),
            "E-FORMAT",
        ),
        (reordered, "E-FORMAT"),
        (format!("{text}steps 3\n"), "E-FORMAT"),
        (
            swap("policy ngraph-deriv-real/0", "policy ngraph-deriv-real/1"),
            "E-FORMAT",
        ),
        (swap("prng splitmix64/0", "prng xorshift/0"), "E-FORMAT"),
        (swap("steps 3", "steps +3"), "E-FORMAT"),
        (
            swap(&param, &param.replacen("param 1", "param +1", 1)),
            "E-FORMAT",
        ),
        (swap(&seed, "seed 0x+000000000000001"), "E-FORMAT"),
    ];
    for (bad, want) in cases {
        assert_eq!(load(&bad).err().as_deref(), Some(want), "{bad}");
    }
    let small = SnapshotFile::parse(&IDS, &text, 4).map_err(|d| d.code_string());
    assert_eq!(small.err().as_deref(), Some("E-BUDGET-SNAPSHOT-BYTES"));
}

#[test]
fn recorded_optimizers_outside_the_adam_domain_are_refused_on_load() {
    // Internally consistent forgeries: `print` recomputes every identity.
    let edits: [AdamEdit; 6] = [
        ("lr 0", |a| a.lr = 0.0),
        ("lr NaN", |a| a.lr = f64::NAN),
        ("beta1 1", |a| a.beta1 = 1.0),
        ("beta2 < 0", |a| a.beta2 = -0.25),
        ("eps 0", |a| a.eps = 0.0),
        ("eps -inf", |a| a.eps = f64::NEG_INFINITY),
    ];
    for (name, edit) in edits {
        let mut f = trained(1);
        edit(&mut f.provenance.optimizer);
        let text = f.print(&IDS);
        assert_eq!(
            noble_ngraph::ir::hex(&documented_digest(&IDS, &text)),
            text.lines().last().unwrap()["provenance ".len()..],
            "{name}: the forgery is consistent"
        );
        assert_eq!(
            load(&text).err().as_deref(),
            Some("E-SNAPSHOT-INTEGRITY"),
            "{name}"
        );
    }
    let mut zero_betas = trained(1);
    zero_betas.provenance.optimizer.beta1 = -0.0;
    zero_betas.provenance.optimizer.beta2 = 0.0;
    let text = zero_betas.print(&IDS);
    assert_eq!(load(&text).expect("inside the domain"), zero_betas);
}

#[test]
fn opt_in_records_must_be_canonically_ordered() {
    let mut f = trained(0);
    f.provenance
        .deriv
        .boolean
        .insert(NodeId(3), BooleanPolicy::StraightThrough);
    f.provenance
        .deriv
        .boolean
        .insert(NodeId(4), BooleanPolicy::Refuse);
    let text = f.print(&IDS);
    assert!(text.contains("opt-in %3 boolean straight-through\nopt-in %4 boolean refuse\n"));
    let swapped = text.replace(
        "opt-in %3 boolean straight-through\nopt-in %4 boolean refuse\n",
        "opt-in %4 boolean refuse\nopt-in %3 boolean straight-through\n",
    );
    assert_eq!(load(&swapped).err().as_deref(), Some("E-FORMAT"));
}

#[test]
fn binding_refuses_other_graphs_facts_layouts_and_pins() {
    let f = trained(3);
    let model = checked(linear(false, false));
    f.bind(&IDS, &model, None).expect("binds");
    f.bind(&IDS, &model, Some(&f.snapshot.id()))
        .expect("pinned binds");
    let code = |r: Result<(), noble_ngraph::Diagnostic>| r.err().map(|d| d.code_string());
    let guarded = checked(linear(true, false));
    assert_eq!(
        code(f.bind(&IDS, &guarded, None)).as_deref(),
        Some("E-SNAPSHOT-STALE")
    );
    // Same GraphLogicalId, other fact records.
    let with_fact = checked(linear(false, true));
    assert_eq!(with_fact.id(), model.id());
    assert_eq!(
        code(f.bind(&IDS, &with_fact, None)).as_deref(),
        Some("E-SNAPSHOT-STALE")
    );
    assert_eq!(
        code(f.bind(&IDS, &model, Some(&[7; 32]))).as_deref(),
        Some("E-SNAPSHOT-ID")
    );
    // Swapped param lines with recomputed layout, snapshot and provenance.
    let mut slots = f.snapshot.slots().to_vec();
    slots.swap(0, 1);
    let swapped = SnapshotFile {
        snapshot: ParamSnapshot::from_parts(
            &IDS,
            f.snapshot.graph(),
            layout_key_of(&IDS, &slots),
            slots,
            f.snapshot.bytes().to_vec(),
        ),
        provenance: f.provenance.clone(),
    };
    let mut swapped = swapped;
    swapped.provenance.result = swapped.snapshot.id();
    let reloaded = load(&swapped.print(&IDS)).expect("consistent forgery reloads");
    assert_eq!(
        code(reloaded.bind(&IDS, &model, None)).as_deref(),
        Some("E-SNAPSHOT-LAYOUT")
    );
}
