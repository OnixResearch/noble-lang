use super::*;

fn builder(subject: &str) -> GraphBuilder {
    GraphBuilder::new(SubProfile::I64, Subject::authored(&TestIds, subject))
}

pub(super) fn simple_graph(subject: &str) -> Graph {
    let mut b = builder(subject);
    let left = b.input(ScalarType::I64);
    let right = b.input(ScalarType::I64);
    let output = b.op(Op::I64Add(left, right));
    b.output(output);
    b.finish()
}

/// `(a0 + i64::MAX) * i64::MIN * a1`: both boundary constants, and every operation wraps.
fn wrapping() -> Graph {
    let mut b = builder("llvm-wrapping");
    let left = b.input(ScalarType::I64);
    let right = b.input(ScalarType::I64);
    let max = b.i64_const(i64::MAX);
    let min = b.i64_const(i64::MIN);
    let sum = b.op(Op::I64Add(left, max));
    let negated = b.op(Op::I64Mul(min, sum));
    let scaled = b.op(Op::I64Mul(negated, right));
    b.output(scaled);
    b.finish()
}

/// `(a0 * -3) - -7`, then `* a1`: negative constants as exact immediates.
fn negative() -> Graph {
    let mut b = builder("llvm-negative");
    let left = b.input(ScalarType::I64);
    let right = b.input(ScalarType::I64);
    let minus_three = b.i64_const(-3);
    let minus_seven = b.i64_const(-7);
    let scaled = b.op(Op::I64Mul(left, minus_three));
    let shifted = b.op(Op::I64Sub(scaled, minus_seven));
    let out = b.op(Op::I64Mul(shifted, right));
    b.output(out);
    b.finish()
}

/// `(a0 - a1) * (a1 - a0)`: subtraction order is observable, and both orders are in one artifact.
fn subtractive() -> Graph {
    let mut b = builder("llvm-subtractive");
    let left = b.input(ScalarType::I64);
    let right = b.input(ScalarType::I64);
    let forward = b.op(Op::I64Sub(left, right));
    let backward = b.op(Op::I64Sub(right, left));
    let out = b.op(Op::I64Mul(forward, backward));
    b.output(out);
    b.finish()
}

/// `(a0 * a0 + a0 * a0) * -1 - (a0 * a0) * -1 + a1`: shared node and literal have multiple uses.
fn shared() -> Graph {
    let mut b = builder("llvm-shared");
    let left = b.input(ScalarType::I64);
    let right = b.input(ScalarType::I64);
    let square = b.op(Op::I64Mul(left, left));
    let twice = b.op(Op::I64Add(square, square));
    let minus_one = b.i64_const(-1);
    let flipped = b.op(Op::I64Mul(twice, minus_one));
    let other = b.op(Op::I64Mul(square, minus_one));
    let difference = b.op(Op::I64Sub(flipped, other));
    let out = b.op(Op::I64Add(difference, right));
    b.output(out);
    b.finish()
}

/// `(3 + 5) * a0 + a1 + 3 + 5`: the fold is real and both original constants stay live.
fn foldable() -> Graph {
    let mut b = builder("llvm-foldable");
    let left = b.input(ScalarType::I64);
    let right = b.input(ScalarType::I64);
    let three = b.i64_const(3);
    let five = b.i64_const(5);
    let eight = b.op(Op::I64Add(three, five));
    let scaled = b.op(Op::I64Mul(left, eight));
    let with_input = b.op(Op::I64Add(scaled, right));
    let with_three = b.op(Op::I64Add(with_input, three));
    let out = b.op(Op::I64Add(with_three, five));
    b.output(out);
    b.finish()
}

struct Spec {
    name: &'static str,
    doc: &'static str,
    graph: Graph,
}

fn specs() -> Vec<Spec> {
    vec![
        Spec {
            name: "wrapping",
            doc: "MIN and MAX constants: `(a0 + i64::MAX) * i64::MIN * a1` wraps at every step",
            graph: wrapping(),
        },
        Spec {
            name: "negative",
            doc: "negative constants: `(a0 * -3) - -7`, then `* a1`",
            graph: negative(),
        },
        Spec {
            name: "subtractive",
            doc: "subtraction order: `(a0 - a1) * (a1 - a0)`",
            graph: subtractive(),
        },
        Spec {
            name: "shared",
            doc: "sharing: one `a0 * a0` node and one `-1` literal each feed two consumers",
            graph: shared(),
        },
        Spec {
            name: "foldable",
            doc: "a fold LLVM may already perform: `(3 + 5) * a0 + a1 + 3 + 5`",
            graph: foldable(),
        },
    ]
}

pub(super) struct Artifact {
    pub(super) name: &'static str,
    pub(super) doc: &'static str,
    pub(super) graph: Graph,
    pub(super) checked: CheckedGraph,
    pub(super) text: String,
    pub(super) module: Module,
}

fn artifact(spec: &Spec) -> Artifact {
    let checked = check::check(&TestIds, spec.graph.clone()).expect("the fixture is admitted");
    let text = llvm::emit(&checked).expect("the fixture is inside the profile");
    let module = llvm::validate(&checked, &text).expect("the emitted module validates");
    Artifact {
        name: spec.name,
        doc: spec.doc,
        graph: spec.graph.clone(),
        checked,
        text,
        module,
    }
}

/// The five fixtures and the certified-rewritten endpoint of `foldable`.
pub(super) fn artifacts() -> Vec<Artifact> {
    let mut all: Vec<Artifact> = specs().iter().map(artifact).collect();
    let original = all
        .iter()
        .find(|held| held.name == "foldable")
        .expect("the foldable fixture");
    let (derivation, rewritten) = noble_ngraph::cert::emit(&TestIds, &original.graph);
    assert_eq!(
        noble_ngraph::cert::check(&TestIds, &original.graph, &rewritten, &derivation),
        Ok(()),
        "the certificate the pair ships with is accepted"
    );
    assert!(
        !derivation.steps.is_empty(),
        "the pair records at least one rewrite step"
    );
    all.push(artifact(&Spec {
        name: "foldable-rewritten",
        doc: "the certified rewrite of `foldable`: `add(3, 5)` is replaced in place by the constant 8",
        graph: rewritten,
    }));
    all
}

pub(super) fn artifact_named<'a>(all: &'a [Artifact], name: &str) -> &'a Artifact {
    all.iter()
        .find(|held| held.name == name)
        .unwrap_or_else(|| panic!("the fixture {name} exists"))
}
