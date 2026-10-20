#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::excessive_file_length,
    tigerstyle::explicit_defaults,
    tigerstyle::function_length,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Graph files (design D4, NGRAPH-FORMAT-01, NGRAPH-CASE-06 encodings): the
//! printed form parses back to the same graph and identity for every record
//! kind, the `authored-name` sugar and layout are not identity-bearing, and
//! malformed subject, shape and provenance records are refused.

mod common;

use common::IDS;
use noble_kernel::types::Ty;
use noble_ngraph::check::Budgets;
use noble_ngraph::extract::{self, source, Budget, Request};
use noble_ngraph::format::graph::{parse, print};
use noble_ngraph::ir::{
    AffineOperand, Fact, GraphBuilder, Literal, Op, OutOfRange, Provenance, ScalarType, ShapeTree,
    SubProfile, Subject, SubjectKind,
};
use noble_ngraph::Graph;

/// Every node kind, fact kind and provenance of `ngraph-f32/0`, with shapes,
/// a captured subject and a closure. Structurally valid only for the codec;
/// the checker would refuse it.
fn f32_zoo() -> Graph {
    let subject = Subject::derive(&IDS, SubjectKind::Captured, &[b"zoo"], vec![7, 42]);
    let mut b = GraphBuilder::new(SubProfile::F32, subject);
    let x = b.input(ScalarType::F32);
    let i = b.input(ScalarType::F32);
    let p = b.param();
    let c = b.f32_const(1.5);
    let add = b.op(Op::F32Add(x, p));
    let sub = b.op(Op::F32Sub(add, c));
    let mul = b.op(Op::F32Mul(sub, x));
    let aff = b.affine(
        -0.0,
        &[(2.0, AffineOperand::Node(mul)), (0.5, AffineOperand::One)],
    );
    let relu = b.op(Op::F32Relu(aff));
    let reglu = b.op(Op::F32Reglu {
        left: relu,
        gate: x,
    });
    let guard = b.guard(
        Fact::Range {
            lo: Literal::f32(0.0),
            hi: Literal::f32(1.0),
        },
        x,
    );
    let st = b.op(Op::F32Step(x));
    let sel = b.op(Op::F32Select {
        bit: st,
        off: reglu,
        on: c,
    });
    let idx = b.op(Op::F32Index {
        index: i,
        table: vec![sel, c, p],
        below: OutOfRange::Clamp,
        above: OutOfRange::Zero,
    });
    b.guard(
        Fact::OneHot {
            group: vec![st, sel],
            allow_zero: true,
        },
        st,
    );
    b.op(Op::F32Index {
        index: i,
        table: vec![],
        below: OutOfRange::Refuse,
        above: OutOfRange::Refuse,
    });
    b.output(idx);
    b.output(sel);
    b.fact(
        x,
        Fact::Range {
            lo: Literal::F32(0x8000_0000),
            hi: Literal::F32(0x7fc0_0001),
        },
        Provenance::InputValidated,
    );
    b.fact(i, Fact::Integral, Provenance::InputValidated);
    b.fact(
        st,
        Fact::Bit,
        Provenance::Derived("step rule #1 100% \"q\"".into()),
    );
    b.fact(sel, Fact::Finite, Provenance::Derived(String::new()));
    b.fact(
        c,
        Fact::Grid { log2: -1 },
        Provenance::Evidence(vec![0xde, 0xad, 0x00]),
    );
    b.fact(p, Fact::Shape(3), Provenance::Evidence(vec![]));
    b.fact(
        add,
        Fact::Range {
            lo: Literal::I64(-3),
            hi: Literal::Bool(true),
        },
        Provenance::Guard(guard),
    );
    b.fact(mul, Fact::Integral, Provenance::Asserted);
    let pair = ShapeTree::pair(ShapeTree::SCALAR, ShapeTree::UNIT).expect("a shallow pair");
    b.shapes(Some(vec![pair, ShapeTree::SCALAR]), Some(vec![]));
    b.finish()
}

/// `ngraph-i64/0` nodes with every literal type in its facts.
fn i64_zoo() -> Graph {
    let mut b = GraphBuilder::new(SubProfile::I64, Subject::authored(&IDS, "i64 zoo"));
    let x = b.input(ScalarType::I64);
    let k = b.i64_const(i64::MIN);
    let t = b.constant(Literal::Bool(false));
    let a = b.op(Op::I64Add(x, k));
    let s = b.op(Op::I64Sub(a, x));
    let m = b.op(Op::I64Mul(s, s));
    let e = b.op(Op::I64Eq(m, x));
    b.guard(
        Fact::Range {
            lo: Literal::I64(-5),
            hi: Literal::I64(i64::MAX),
        },
        x,
    );
    b.output(e);
    b.output(t);
    b.fact(
        x,
        Fact::Range {
            lo: Literal::F32(0x3f80_0000),
            hi: Literal::I64(9),
        },
        Provenance::InputValidated,
    );
    b.shapes(None, Some(vec![ShapeTree::SCALAR, ShapeTree::SCALAR]));
    b.finish()
}

fn without_origins(mut g: Graph) -> Graph {
    for n in &mut g.nodes {
        n.origin = None;
    }
    g
}

#[test]
fn printed_graphs_parse_back_to_the_same_graph_and_identity() {
    for g in [f32_zoo(), i64_zoo()] {
        let text = print(&g);
        let back = parse(&IDS, &text).unwrap_or_else(|d| panic!("{d}\n{text}"));
        assert_eq!(back, g, "{text}");
        assert_eq!(back.logical_id(&IDS), g.logical_id(&IDS));
        assert_eq!(print(&back), text, "printing is canonical");
    }
}

#[test]
fn extracted_graphs_round_trip_without_origins() {
    let cases: [(&[&str], &str, usize); 4] = [
        (&[], "dup * swap 3 * + 7 -", 2),
        (&[], "2 quote [ + ] compose dup compose run", 1),
        (&["def sq [ dup * ]"], "sq swap sq +", 2),
        (&[], "swap [ * ] dip + dup", 3),
    ];
    for (defs, root, n) in cases {
        let p = source::prepare(
            noble_contracts::source::Session::new(),
            noble_contracts::Limits::default(),
            defs,
            root,
            &vec![Ty::I64; n],
        )
        .expect("frontend");
        let admitted = extract::admit(
            &IDS,
            &p.submission,
            &Request {
                profile: "ngraph-i64/0",
                claimed_closure: &p.claimed_closure,
                budget: Budget::default(),
            },
        )
        .expect("admitted");
        let x = extract::extract(&IDS, &admitted, &Budgets::default()).expect("extracted");
        let g = x.checked.graph().clone();
        assert!(
            g.nodes.iter().any(|n| n.origin.is_some()),
            "{root}: origins"
        );
        assert_eq!(g.subject.kind, SubjectKind::Extracted);
        let back = parse(&IDS, &print(&g)).expect("parses");
        assert_eq!(back, without_origins(g.clone()), "{root}");
        let again = noble_ngraph::check(&IDS, back).expect("checks");
        assert_eq!(again.id(), x.checked.id(), "{root}");
        assert_eq!(again.fact_bindings(&IDS), x.checked.fact_bindings(&IDS));
    }
}

const BODY: &str = "slot input f32\n%0 = input 0\n%1 = f32.relu %0\nout %1\n";

#[test]
fn authored_name_sugar_comments_and_spacing_are_not_identity_bearing() {
    let sugar = parse(
        &IDS,
        &format!("ngraph-ir/0 ngraph-f32/0\nsubject authored-name my   demo graph\n{BODY}"),
    )
    .expect("parses");
    assert_eq!(sugar.subject, Subject::authored(&IDS, "my demo graph"));
    let printed = print(&sugar);
    assert!(printed.contains("\nsubject authored "), "{printed}");
    assert!(!printed.contains("authored-name"));
    let spaced = "# leading comment\n\n  ngraph-ir/0    ngraph-f32/0  # profile\nsubject authored-name my demo graph\n\nslot   input f32\n%0 =   input 0 # x\n%1 = f32.relu %0\nout %1\n";
    let other = parse(&IDS, spaced).expect("parses");
    assert_eq!(other.logical_id(&IDS), sugar.logical_id(&IDS));
    let renamed = parse(
        &IDS,
        &format!("ngraph-ir/0 ngraph-f32/0\nsubject authored-name another graph\n{BODY}"),
    )
    .expect("parses");
    assert_ne!(renamed.logical_id(&IDS), sugar.logical_id(&IDS));
}

#[test]
fn malformed_records_are_refused_with_their_codes() {
    let head = "ngraph-ir/0 ngraph-f32/0\n";
    let subj = "subject authored-name t\n";
    let hex64 = "ab".repeat(32);
    let cases: Vec<(String, &str)> = vec![
        (BODY.to_owned(), "E-GRAPH-MALFORMED"),
        (
            format!("ngraph-ir/0 ngraph-f64/0\n{subj}{BODY}"),
            "E-PROFILE",
        ),
        (
            format!("ngraph-ir/0 ngraph-f32/0 extra\n{subj}{BODY}"),
            "E-GRAPH-MALFORMED",
        ),
        (format!("{head}{BODY}"), "E-GRAPH-MALFORMED"),
        (format!("{head}{subj}{subj}{BODY}"), "E-GRAPH-MALFORMED"),
        (
            format!("{head}subject authored {}\n{BODY}", "ab".repeat(31)),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}subject authored {hex64} {BODY}"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}subject authored {hex64} closure x\n{BODY}"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}subject sideways {hex64} closure -\n{BODY}"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}subject authored-name\n{BODY}"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}{subj}shape input s\nshape input s\n{BODY}"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}{subj}shape input (p s)\n{BODY}"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}{subj}shape input (q s s)\n{BODY}"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}{subj}shape sideways s\n{BODY}"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}{subj}{BODY}fact %0 finite evidence abc\n"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}{subj}{BODY}fact %0 finite evidence zz\n"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}{subj}{BODY}fact %0 finite derived\n"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}{subj}{BODY}fact %0 finite derived a%zz\n"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}{subj}{BODY}fact %0 range i64:x 1\n"),
            "E-GRAPH-MALFORMED",
        ),
        (
            format!("{head}{subj}slot input f32\n%0 = const f32 0.1\nout %0\n"),
            "E-LITERAL-INEXACT",
        ),
        (
            format!("{head}{subj}slot input f32\n%0 = const f32 inf\nout %0\n"),
            "E-LITERAL-NONFINITE",
        ),
        (
            format!("{head}{subj}slot input f32\n%0 = f32.frob %0\nout %0\n"),
            "E-GRAPH-MALFORMED",
        ),
    ];
    for (text, code) in cases {
        match parse(&IDS, &text) {
            Ok(g) => panic!("accepted:\n{text}\n{g:?}"),
            Err(d) => assert_eq!(d.code_string(), code, "{text}\n{d}"),
        }
    }
}

#[test]
fn evidence_and_range_literals_are_symmetric() {
    let text = format!(
        "ngraph-ir/0 ngraph-f32/0\nsubject authored-name t\n{BODY}fact %0 finite evidence 00ff\nfact %0 range 0 i64:3 asserted\nfact %0 range true 0x3f800000 asserted\n"
    );
    let g = parse(&IDS, &text).expect("parses");
    assert_eq!(
        g.facts[0].provenance,
        Provenance::Evidence(vec![0x00, 0xff])
    );
    assert_eq!(
        g.facts[1].fact,
        Fact::Range {
            lo: Literal::F32(0),
            hi: Literal::I64(3)
        }
    );
    assert_eq!(
        g.facts[2].fact,
        Fact::Range {
            lo: Literal::Bool(true),
            hi: Literal::F32(0x3f80_0000)
        }
    );
    assert_eq!(parse(&IDS, &print(&g)).expect("reparses"), g);
    // Evidence stays refused by the checker (P5).
    let only_evidence = parse(
        &IDS,
        &format!("ngraph-ir/0 ngraph-f32/0\nsubject authored-name t\n{BODY}fact %0 finite evidence 00ff\n"),
    )
    .expect("parses");
    let d = noble_ngraph::check(&IDS, only_evidence).expect_err("evidence refused");
    assert_eq!(d.code_string(), "E-FACT-UNCHECKED");
}

/// A `shape input` tree nesting `depth` pairs: `(p (p … s u) u)`.
fn nested_shape(depth: usize) -> String {
    format!("{}s{}", "(p ".repeat(depth), " u)".repeat(depth))
}

fn shaped(depth: usize) -> String {
    format!(
        "ngraph-ir/0 ngraph-f32/0\nsubject authored-name deep\nshape input {}\n{BODY}",
        nested_shape(depth)
    )
}

#[test]
fn shape_trees_nest_at_most_127_pairs() {
    // 127 is the deepest pair nesting an extracted interface can have (a
    // 256-node Core type); it parses, prints back and has an identity.
    let deepest = parse(&IDS, &shaped(127)).expect("127 nested pairs parse");
    let tree = &deepest.input_shape.as_ref().expect("a shape")[0];
    assert_eq!(tree.depth(), 127);
    let again = parse(&IDS, &print(&deepest)).expect("reparses");
    assert_eq!(again, deepest);
    noble_ngraph::check(&IDS, again).expect("shapes are not checker input");
    // One more pair, and a tree deep enough to exhaust a recursive walk,
    // are refused at their line before the parser descends any further.
    for depth in [128, 40_000] {
        let d = parse(&IDS, &shaped(depth)).expect_err("too deep");
        assert_eq!(d.code_string(), "E-GRAPH-MALFORMED", "{depth}: {d}");
        assert_eq!(d.site, noble_ngraph::Site::Line(3), "{depth}");
    }
    // Builders get the same bound: only `ShapeTree::pair` makes a pair.
    let mut tree = ShapeTree::SCALAR;
    for _ in 0..127 {
        tree = ShapeTree::pair(tree, ShapeTree::UNIT).expect("within the bound");
    }
    let refused = ShapeTree::pair(ShapeTree::UNIT, tree).expect_err("a 128th pair");
    assert_eq!(refused.code_string(), "E-GRAPH-MALFORMED");
}

#[test]
fn number_tokens_take_no_plus_sign_and_at_most_eight_hex_digits() {
    let head = "ngraph-ir/0 ngraph-f32/0\nsubject authored-name t\n";
    let i64_head = "ngraph-ir/0 ngraph-i64/0\nsubject authored-name t\n";
    let refused = [
        format!("{head}slot input f32\n%0 = const f32 +1.5\nout %0\n"),
        format!("{head}slot input f32\n%0 = const f32 0x03f800000\nout %0\n"),
        format!("{head}slot input f32\n%0 = const f32 0x+3f80000\nout %0\n"),
        format!("{head}slot input f32\n%0 = input +0\nout %0\n"),
        format!("{head}slot input f32\n%+0 = input 0\nout %0\n"),
        format!("{head}slot input f32\n%0 = input 0\nout %+0\n"),
        format!("{i64_head}slot input i64\n%0 = const i64 +5\nout %0\n"),
        format!("{i64_head}slot input i64\n%0 = input 0\nfact %0 range +1 2 asserted\nout %0\n"),
        format!("{head}{BODY}fact %0 range i64:+1 2 asserted\n"),
        format!("{head}{BODY}fact %0 finite derived a%+f\n"),
        format!(
            "ngraph-ir/0 ngraph-f32/0\nsubject authored {} closure +1\n{BODY}",
            "ab".repeat(32)
        ),
    ];
    for text in refused {
        let d = parse(&IDS, &text).expect_err(&text);
        assert_eq!(d.code_string(), "E-GRAPH-MALFORMED", "{text}\n{d}");
    }
    // The accepted spellings are unchanged.
    let g = parse(
        &IDS,
        &format!("{head}slot input f32\n%0 = input 0\n%1 = const f32 0x3f800000\n%2 = const f32 -1.5e0\n%3 = f32.affine 0x0 1 %0 1.0 %1 0x1 %2\nout %3\n"),
    )
    .expect("strict spellings parse");
    assert_eq!(g.nodes.len(), 4);
}
