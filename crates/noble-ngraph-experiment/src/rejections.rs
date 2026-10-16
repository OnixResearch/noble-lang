//! B3 rejection controls on directly authored `ngraph-ir/0` (core part):
//! false/unchecked facts, bad indices, malformed IR and budgets. Source-level
//! controls (effects, resources, dynamic `run`, recursion, `reflect`, control,
//! Program outputs, the expansion chain) belong to the `I64` extraction leg.
//!
//! Every case records the D6 code observed and the number of graph nodes
//! evaluated before the refusal (0 for every static or preflight refusal).

use crate::scheme::BLAKE3;
use noble_ngraph::check::{check_with, Budgets};
use noble_ngraph::compiled::Compiled;
use noble_ngraph::diag::Diagnostic;
use noble_ngraph::eval::{self, Control, Obs};
use noble_ngraph::format::graph;

/// One rejection control.
#[derive(Clone, Debug)]
pub struct Rejection {
    /// Case name.
    pub name: &'static str,
    /// Group (false-fact, bad-index, malformed, budget).
    pub group: &'static str,
    /// Expected D6 code string.
    pub expected: &'static str,
    /// Observed code string (`ACCEPTED` if nothing was refused).
    pub observed: String,
    /// Graph nodes evaluated by L3 before the observation (0 for static refusals).
    pub l3_evaluated: u64,
    /// Graph nodes evaluated by L4 before the observation.
    pub l4_evaluated: u64,
    /// Whether L3 and L4 agree on the observation.
    pub legs_agree: bool,
    /// Stage at which the refusal happened.
    pub stage: &'static str,
    /// Diagnostic detail.
    pub detail: String,
}

impl Rejection {
    /// Whether the observed code is the expected one.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.observed == self.expected && self.legs_agree
    }
}

const F32H: &str = "ngraph-ir/0 ngraph-f32/0\n";

/// Parses an authored control, adding the `subject authored-name` record
/// that the graph file form requires (each control's subject is its name).
fn parse(src: &str, name: &str) -> Result<noble_ngraph::Graph, Diagnostic> {
    let (head, rest) = src.split_once('\n').unwrap_or((src, ""));
    graph::parse(
        &BLAKE3,
        &format!("{head}\nsubject authored-name {name}\n{rest}"),
    )
}

fn static_case(
    name: &'static str,
    group: &'static str,
    expected: &'static str,
    src: &str,
    budgets: &Budgets,
) -> Rejection {
    let r = parse(src, name).and_then(|g| check_with(&BLAKE3, g, budgets));
    let (observed, stage, detail) = match r {
        Ok(_) => ("ACCEPTED".to_owned(), "none", String::new()),
        Err(d) => (d.code_string(), stage_of(&d), d.detail),
    };
    Rejection {
        name,
        group,
        expected,
        observed,
        l3_evaluated: 0,
        l4_evaluated: 0,
        legs_agree: true,
        stage,
        detail,
    }
}

const fn stage_of(d: &Diagnostic) -> &'static str {
    match d.site {
        noble_ngraph::diag::Site::Line(_) => "parse",
        _ => "graph-check",
    }
}

fn eval_case(
    name: &'static str,
    group: &'static str,
    expected: &'static str,
    src: &str,
    inputs: &[f32],
    control_scratch: Option<u64>,
) -> Rejection {
    let g = match parse(src, name).and_then(|g| noble_ngraph::check::check(&BLAKE3, g)) {
        Ok(g) => g,
        Err(d) => {
            return Rejection {
                name,
                group,
                expected,
                observed: d.code_string(),
                l3_evaluated: 0,
                l4_evaluated: 0,
                legs_agree: true,
                stage: stage_of(&d),
                detail: d.detail,
            }
        }
    };
    let control = Control {
        forward_scratch: control_scratch,
        ..Control::default()
    };
    let ins = eval::f32_inputs(inputs);
    let l3 = eval::reference(&g, &ins, None, &control);
    let l4 = Compiled::new(&BLAKE3, &g).evaluate(&ins, None, &control);
    let (observed, stage, detail) = match &l3.obs {
        Obs::Refused(d) => (d.code_string(), "preflight", d.detail.clone()),
        Obs::GuardFailed { node, fact } => (
            "G-FACT-FALSE".to_owned(),
            "evaluation",
            format!("node %{} fact {}", node.0, fact.kind_name()),
        ),
        Obs::Normal(_) => ("ACCEPTED".to_owned(), "none", String::new()),
        Obs::Exhausted(k) => (
            format!("EXHAUSTED-{}", k.name()),
            "evaluation",
            String::new(),
        ),
        Obs::Cancelled => ("C-CANCELLED".to_owned(), "evaluation", String::new()),
    };
    Rejection {
        name,
        group,
        expected,
        observed,
        l3_evaluated: l3.evaluated_nodes,
        l4_evaluated: l4.evaluated_nodes,
        legs_agree: l3.obs.obs_eq(&l4.obs),
        stage,
        detail,
    }
}

/// Runs every core B3 control.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn run_all() -> Vec<Rejection> {
    let d = Budgets::default();
    let mut v = Vec::new();
    let fx = |body: &str| format!("{F32H}slot input f32\n%0 = input 0\n{body}");
    // ---- false / unchecked facts ----
    for (name, fact) in [
        ("caller-asserted range", "range 0 1"),
        ("caller-asserted grid", "grid 0"),
        ("caller-asserted bit", "bit"),
        ("caller-asserted one-hot", "one-hot exact %0"),
    ] {
        v.push(static_case(
            name,
            "false-fact",
            "E-FACT-UNCHECKED",
            &fx(&format!(
                "%1 = f32.step %0\nfact %0 {fact} asserted\nout %1\n"
            )),
            &d,
        ));
    }
    v.push(static_case(
        "step without any range fact",
        "false-fact",
        "E-FACT-UNCHECKED",
        &fx("%1 = f32.step %0\nout %1\n"),
        &d,
    ));
    v.push(static_case(
        "derived claim the checker cannot derive (x+x in [0,1])",
        "false-fact",
        "E-FACT-UNCHECKED",
        &fx("%1 = f32.add %0 %0\n%2 = f32.step %1\nfact %0 range 0 1 input-validated\nfact %1 range 0 1 derived interval\nout %2\n"),
        &d,
    ));
    v.push(static_case(
        "evidence fact (excluded from Phase B by P5)",
        "false-fact",
        "E-FACT-UNCHECKED",
        &fx("%1 = f32.step %0\nfact %0 range 0 1 evidence 00\nout %1\n"),
        &d,
    ));
    v.push(eval_case(
        "violating input with an input-validated range",
        "false-fact",
        "E-FACT-INPUT",
        &fx("%1 = f32.step %0\nfact %0 range 0 1 input-validated\nout %1\n"),
        &[1.5],
        None,
    ));
    v.push(eval_case(
        "violating intermediate under a guard: 0.5 held in a bit",
        "false-fact",
        "G-FACT-FALSE",
        &fx("%1 = f32.add %0 %0\n%2 = guard %1 bit\n%3 = f32.select %1 %0 %1\nout %3\nfact %0 range 0 1 input-validated\n"),
        &[0.25], // 0.25 + 0.25 = 0.5
        None,
    ));
    // ---- bad indices ----
    let idx = |facts: &str, rules: &str| {
        format!("{F32H}slot input f32\nslot input f32\n%0 = input 0\n%1 = input 1\n%2 = f32.index %0 {rules} %1 %1 %1\n{facts}out %2\n")
    };
    v.push(eval_case(
        "fractional index 2.5 under an input-validated integral fact",
        "bad-index",
        "E-FACT-INPUT",
        &idx(
            "fact %0 integral input-validated\nfact %0 range -4 4 input-validated\n",
            "refuse refuse",
        ),
        &[2.5, 1.0],
        None,
    ));
    v.push(static_case(
        "index without an integral fact",
        "bad-index",
        "E-FACT-UNCHECKED",
        &idx("fact %0 range -4 4 input-validated\n", "refuse refuse"),
        &d,
    ));
    v.push(static_case(
        "index range fact reaching 2^24",
        "bad-index",
        "E-FACT-UNCHECKED",
        &idx(
            "fact %0 integral input-validated\nfact %0 range 0 16777216 input-validated\n",
            "refuse refuse",
        ),
        &d,
    ));
    v.push(eval_case(
        "out-of-range index 3 under refuse",
        "bad-index",
        "G-FACT-FALSE",
        &idx(
            "fact %0 integral input-validated\nfact %0 range -4 4 input-validated\n",
            "refuse refuse",
        ),
        &[3.0, 1.0],
        None,
    ));
    v.push(eval_case(
        "NaN index under a guard",
        "bad-index",
        "G-FACT-FALSE",
        &format!("{F32H}slot input f32\nslot input f32\n%0 = input 0\n%1 = input 1\n%2 = guard %0 integral\n%3 = guard %0 range -4 4\n%4 = f32.index %0 clamp clamp %1 %1\nout %4\n"),
        &[f32::NAN, 1.0],
        None,
    ));
    // ---- malformed IR ----
    let m = |name, expected, body: &str| {
        static_case(
            name,
            "malformed",
            expected,
            &format!("{F32H}slot input f32\n{body}"),
            &d,
        )
    };
    v.push(m(
        "cycle",
        "E-GRAPH-MALFORMED",
        "%0 = input 0\n%1 = f32.add %0 %2\n%2 = f32.add %1 %0\nout %2\n",
    ));
    v.push(m(
        "forward reference",
        "E-GRAPH-MALFORMED",
        "%0 = input 0\n%1 = f32.add %0 %2\n%2 = const f32 1\nout %1\n",
    ));
    v.push(m(
        "type mix (i64 op in an f32 graph)",
        "E-TYPE-MIX",
        "%0 = input 0\n%1 = i64.add %0 %0\nout %1\n",
    ));
    v.push(m(
        "type mix (i64 constant in an f32 graph)",
        "E-TYPE-MIX",
        "%0 = input 0\n%1 = const i64 1\n%2 = f32.add %0 %1\nout %2\n",
    ));
    v.push(m(
        "dangling node",
        "E-GRAPH-MALFORMED",
        "%0 = input 0\n%1 = f32.add %0 %0\nout %0\n",
    ));
    v.push(m(
        "unknown node",
        "E-GRAPH-MALFORMED",
        "%0 = input 0\n%1 = f32.add %0 %7\nout %1\n",
    ));
    v.push(m(
        "unknown operation",
        "E-GRAPH-MALFORMED",
        "%0 = input 0\n%1 = f32.div %0 %0\nout %1\n",
    ));
    v.push(m(
        "inexact decimal constant 0.1",
        "E-LITERAL-INEXACT",
        "%0 = input 0\n%1 = const f32 0.1\n%2 = f32.add %0 %1\nout %2\n",
    ));
    v.push(m(
        "NaN constant (bits)",
        "E-LITERAL-NONFINITE",
        "%0 = input 0\n%1 = const f32 0x7fc00000\n%2 = f32.add %0 %1\nout %2\n",
    ));
    v.push(m(
        "NaN constant (word)",
        "E-LITERAL-NONFINITE",
        "%0 = input 0\n%1 = const f32 nan\n%2 = f32.add %0 %1\nout %2\n",
    ));
    v.push(m(
        "infinite affine coefficient",
        "E-LITERAL-NONFINITE",
        "%0 = input 0\n%1 = f32.affine -0 0x7f800000 %0\nout %1\n",
    ));
    v.push(m(
        "duplicate slot",
        "E-GRAPH-MALFORMED",
        "%0 = input 0\n%1 = input 0\n%2 = f32.add %0 %1\nout %2\n",
    ));
    v.push(static_case(
        "param slot in ngraph-i64/0",
        "malformed",
        "E-GRAPH-MALFORMED",
        "ngraph-ir/0 ngraph-i64/0\nslot param i64\n%0 = param 0\nout %0\n",
        &d,
    ));
    v.push(static_case(
        "unknown sub-profile",
        "malformed",
        "E-PROFILE",
        "ngraph-ir/0 ngraph-f64/0\n",
        &d,
    ));
    // ---- budgets (policy-lowered; the /0 defaults are exercised by size) ----
    let chain = |n: usize| {
        let mut s = format!("{F32H}slot input f32\n%0 = input 0\n");
        for i in 1..=n {
            s.push_str(&format!("%{i} = f32.add %{} %0\n", i - 1));
        }
        s.push_str(&format!("out %{n}\n"));
        s
    };
    let lowered = |f: &dyn Fn(&mut Budgets)| {
        let mut b = Budgets::default();
        f(&mut b);
        b
    };
    v.push(static_case(
        "live nodes over a lowered budget",
        "budget",
        "E-BUDGET-LIVE-NODES",
        &chain(100),
        &lowered(&|b| b.live_nodes = 50),
    ));
    v.push(static_case(
        "layers over a lowered budget",
        "budget",
        "E-BUDGET-LAYERS",
        &chain(100),
        &lowered(&|b| b.layers = 64),
    ));
    v.push(static_case(
        "layers over the /0 default (16,384)",
        "budget",
        "E-BUDGET-LAYERS",
        &chain(16_400),
        &d,
    ));
    let wide = {
        let mut s = format!("{F32H}slot input f32\n%0 = input 0\n%1 = f32.affine -0");
        for _ in 0..4097 {
            s.push_str(" 1 %0");
        }
        s.push_str("\nout %1\n");
        s
    };
    v.push(static_case(
        "affine terms over the /0 default (4,096)",
        "budget",
        "E-BUDGET-AFFINE-TERMS",
        &wide,
        &d,
    ));
    let table = {
        let mut s = format!("{F32H}slot input f32\n%0 = input 0\n%1 = f32.index %0 clamp clamp");
        for _ in 0..65_537 {
            s.push_str(" %0");
        }
        s.push_str(
            "\nfact %0 integral input-validated\nfact %0 range 0 1 input-validated\nout %1\n",
        );
        s
    };
    v.push(static_case(
        "index table over the /0 default (65,536)",
        "budget",
        "E-BUDGET-TABLE-LENGTH",
        &table,
        &d,
    ));
    v.push(static_case(
        "raising a budget above /0",
        "budget",
        "E-PROFILE",
        &chain(2),
        &lowered(&|b| b.live_nodes = 1 << 20),
    ));
    v.push(eval_case(
        "forward scratch over a lowered host budget",
        "budget",
        "E-BUDGET-FORWARD-SCRATCH",
        &chain(100),
        &[1.0],
        Some(64),
    ));
    v
}
