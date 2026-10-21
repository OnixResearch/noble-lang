//! Phase B B3 source-level controls for the `I64` extraction leg.
//!
//! Each control is an ordinarily prepared (or, where Core source cannot
//! express it, minimally forged and still kernel-accepted) subject, the D6
//! code admission/extraction must report, and how the ordinary path treats
//! the same source. A refusal returns before any graph exists, so the number
//! of evaluated graph nodes is 0 by construction.

use noble_kernel::contracts::{Behavior, Definition};
use noble_kernel::execution::Submission;
use noble_kernel::types::{EffSet, Ty};
use noble_kernel::untrusted::{Node, NodeId};

use noble_ngraph::check::Budgets;
use noble_ngraph::diag::Diagnostic;
use noble_ngraph::extract::{admit, extract, source, Budget, Request};

use crate::scheme::BLAKE3;

/// How the ordinary path is exercised for a control.
#[derive(Clone, Debug)]
pub enum Ordinary {
    /// `noble session` submissions (definitions first); the last one is the
    /// control's root.
    Session(Vec<String>),
    /// The ordinary Core CLI cannot express the subject; why.
    NotExpressible(&'static str),
    /// `noble live slot` JSONL requests; `sources` are the `(id, source)`
    /// pairs the host's authority allowlists by SHA-256. The slot `inc`
    /// (`I64 -- I64`, pure) is published from `def inc [ 1 + ]`, and the
    /// last request invokes the control's root with `5` and `inc` borrowed.
    LiveSlot {
        /// Allowlisted sources.
        sources: Vec<(&'static str, String)>,
        /// Requests.
        requests: Vec<String>,
    },
}

/// One evaluated control.
#[derive(Debug)]
pub struct Case {
    /// Control name.
    pub name: &'static str,
    /// Human description of the subject.
    pub subject: String,
    /// Expected D6 code string.
    pub expected: &'static str,
    /// The refusal (`Ok` if the subject was extracted).
    pub result: Result<(), Diagnostic>,
    /// Ordinary-path exercise.
    pub ordinary: Ordinary,
}

impl Case {
    /// Observed code string (`ACCEPTED` when extraction succeeded).
    #[must_use]
    pub fn observed(&self) -> String {
        match &self.result {
            Ok(()) => "ACCEPTED".to_owned(),
            Err(d) => d.code_string(),
        }
    }
}

/// Admission plus extraction under `budget`.
///
/// # Errors
/// The first D6 refusal.
pub fn outcome(
    sub: &Submission,
    claimed: &[u64],
    profile: &str,
    budget: Budget,
) -> Result<(), Diagnostic> {
    let admitted = admit(
        &BLAKE3,
        sub,
        &Request {
            profile,
            claimed_closure: claimed,
            budget,
        },
    )?;
    extract(&BLAKE3, &admitted, &Budgets::default()).map(|_| ())
}

fn default_outcome(sub: &Submission, claimed: &[u64]) -> Result<(), Diagnostic> {
    outcome(sub, claimed, "ngraph-i64/0", Budget::default())
}

fn session(lines: &[&str]) -> Ordinary {
    Ordinary::Session(lines.iter().map(|l| (*l).to_owned()).collect())
}

fn from_source(
    name: &'static str,
    expected: &'static str,
    defs: &[&str],
    root: &str,
    inputs: &[Ty],
    ordinary: Ordinary,
) -> Case {
    let result = match source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        defs,
        root,
        inputs,
    ) {
        Ok(p) => default_outcome(&p.submission, &p.claimed_closure),
        Err(e) => Err(Diagnostic::new(
            noble_ngraph::diag::Code::Subject,
            noble_ngraph::diag::Site::Graph,
            format!("frontend refused: {e}"),
        )),
    };
    Case {
        name,
        subject: format!("defs {defs:?}; root {root:?}; inputs {inputs:?}"),
        expected,
        result,
        ordinary,
    }
}

/// Replaces definition `at`'s body with one call of `target`.
fn forge_call(sub: &mut Submission, at: usize, target: Definition) {
    sub.definitions[at].body.candidate.nodes = vec![Node::Invocation {
        def: target,
        inst: noble_kernel::words::Inst { bindings: vec![] },
    }];
    sub.definitions[at].body.candidate.body = vec![NodeId(0)];
}

/// `d0 = [ 1 + ]`, `dₖ = [ dₖ₋₁ dₖ₋₁ ]`, root `0 d_k drop 7`, with one
/// shared specialization row per `dₖ` whose two-call body the kernel
/// accepts. Core source cannot express this beyond `k = 9`: the frontend
/// specializes every named use separately and exhausts its 16,384-node
/// limit at `d10`. The subject is therefore the frontend's `d0` submission
/// plus `k` assembled rows (scheme, `Named` kind, dependency list) and the
/// root's call retargeted to `d_k`. Returns the subject and its closure.
///
/// # Panics
/// If the frontend refuses the `d0` seed (a fixed, valid source).
#[must_use]
pub fn chain(k: u32) -> (Submission, Vec<u64>) {
    let p = source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        &["def d0 [ 1 + ]"],
        "0 d0 drop 7",
        &[],
    )
    .expect("d0 seed source");
    let mut sub = p.submission;
    let d0 = sub.definitions[0].definition;
    let mut closure = vec![sub.definitions[0].identity];
    let mut prev = d0;
    for level in 1..=k {
        let row = Definition(u32::try_from(sub.environment.defs.len()).unwrap_or(u32::MAX));
        let env = &mut sub.environment;
        env.defs.push(env.defs[d0.0 as usize].clone());
        env.kinds.push(Behavior::Named);
        env.deps.push(vec![prev]);
        env.definition_owners.push(None);
        let mut def = sub.definitions[0].clone();
        def.definition = row;
        def.identity = 0x5eed_0000 + u64::from(level);
        let call = Node::Invocation {
            def: prev,
            inst: noble_kernel::words::Inst { bindings: vec![] },
        };
        def.body.candidate.nodes = vec![call.clone(), call];
        def.body.candidate.body = vec![NodeId(0), NodeId(1)];
        def.body.texts.clear();
        closure.push(def.identity);
        sub.definitions.push(def);
        prev = row;
    }
    if let Node::Invocation { def, .. } = &mut sub.body.candidate.nodes[1] {
        *def = prev;
    }
    (sub, closure)
}

fn recursion(name: &'static str, mutual: bool, consistent_deps: bool) -> Case {
    let (defs, root): (&[&str], &str) = if mutual {
        (&["def g [ 1 + ]", "def f [ g ]"], "5 f")
    } else {
        (&["def f [ dup drop ]"], "5 f")
    };
    let p = source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        defs,
        root,
        &[],
    )
    .expect("recursion seed source");
    let mut sub = p.submission;
    let f = sub.definitions[0].definition;
    let (at, row) = if mutual {
        (1, sub.definitions[1].definition)
    } else {
        (0, f)
    };
    forge_call(&mut sub, at, f);
    if consistent_deps {
        sub.environment.deps[row.0 as usize] = vec![f];
    }
    let shape = if mutual {
        "f = [ g ], g = [ f ]"
    } else {
        "f = [ f ]"
    };
    Case {
        name,
        subject: format!(
            "forged {shape} from {defs:?} / {root:?}; Env::deps {}",
            if consistent_deps {
                "forged to match"
            } else {
                "left as produced"
            }
        ),
        expected: if consistent_deps {
            "E-SUBJECT"
        } else {
            "E-RECURSION"
        },
        result: default_outcome(&sub, &p.claimed_closure),
        ordinary: if mutual {
            // Core resolves only earlier definitions, so `f` cannot name a
            // later `g`: mutual recursion is unbound in ordinary source.
            session(&["def f [ g ]"])
        } else {
            session(&["def f [ f ]", "5 f"])
        },
    }
}

fn live_slot_session() -> Result<noble_contracts::source::Session, String> {
    let env = noble_kernel::contracts::environment()
        .map_err(|e| format!("{e:?}"))?
        .enable_live_slots();
    noble_contracts::source::Session::new_live_slots(env).map_err(|e| format!("{e:?}"))
}

fn live(name: &'static str, expected: &'static str, root: &str) -> Case {
    let target = Ty::live_ref(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    let inputs = [Ty::I64, target];
    let result = live_slot_session().and_then(|s| {
        s.prepare(root.as_bytes(), &inputs, noble_contracts::Limits::default())
            .map_err(|e| format!("{e:?}"))
    });
    let result = match result {
        Ok(p) => match p.submission() {
            Some(sub) => default_outcome(sub, &[]),
            None => Err(frontend("no submission")),
        },
        Err(e) => Err(frontend(&e)),
    };
    Case {
        name,
        subject: format!("live-slot session; root {root:?}; inputs {inputs:?}"),
        expected,
        result,
        ordinary: live_slot_requests(root),
    }
}

fn live_slot_requests(root: &str) -> Ordinary {
    let install = serde_json::json!({
        "operation": "install", "id": "subject", "source": root,
        "inputs": ["I64", "LiveRef<I64,I64,pure>"],
    });
    let invoke = serde_json::json!({
        "operation": "invoke", "id": "subject",
        "inputs": [{ "kind": "i64", "value": "5" }],
        "refs": [{ "position": 1, "ordinal": 0, "slot": "inc" }],
    });
    Ordinary::LiveSlot {
        sources: vec![
            ("subject", root.to_owned()),
            ("definition", "def inc [ 1 + ]".to_owned()),
            ("named", "inc".to_owned()),
        ],
        requests: vec![
            install.to_string(),
            r#"{"operation":"define","id":"definition","name":"inc","source":"def inc [ 1 + ]","inputs":[]}"#.to_owned(),
            r#"{"operation":"install","id":"named","source":"inc","inputs":["I64"],"selected_name":"inc"}"#.to_owned(),
            r#"{"operation":"publish","slot":"inc","id":"named","program_index":0,"expected_epoch":"0"}"#.to_owned(),
            invoke.to_string(),
        ],
    }
}

fn frontend(e: &str) -> Diagnostic {
    Diagnostic::new(
        noble_ngraph::diag::Code::Subject,
        noble_ngraph::diag::Site::Graph,
        format!("frontend refused: {e}"),
    )
}

fn resource() -> Case {
    let inputs = [
        Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE),
        Ty::I64,
    ];
    let retained =
        noble_contracts::source::RetainedSourceLimit::new(noble_contracts::Limits::default().bytes)
            .ok_or_else(|| "default source limit exceeds the retained-source cap".to_string());
    let result = retained
        .and_then(|retained| {
            noble_contracts::source::ModuleSession::new(&[], retained).map_err(|e| format!("{e:?}"))
        })
        .and_then(|s| {
            s.prepare(b"swap", &inputs, noble_contracts::Limits::default())
                .map_err(|e| format!("{e:?}"))
        });
    let result = match result {
        Ok(p) => match p.submission() {
            Some(sub) => default_outcome(sub, &[]),
            None => Err(frontend("no submission")),
        },
        Err(e) => Err(frontend(&e)),
    };
    Case {
        name: "resource-input",
        subject: format!("Declared-Modules-v1 session; root \"swap\"; inputs {inputs:?}"),
        expected: "E-RESOURCE",
        result,
        ordinary: Ordinary::NotExpressible(
            "Core source refuses Resource inputs; only declared-module/component hosts supply them, and `noble session` has no resource input",
        ),
    }
}

/// Every B3 source-level control, in a fixed order.
#[must_use]
pub fn run() -> Vec<Case> {
    let program_input = Ty::program(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    let (k20, k20_closure) = chain(20);
    vec![
        from_source(
            "test.emit",
            "E-EFFECT",
            &[],
            "\"hi\" test.emit 5",
            &[],
            session(&["\"hi\" test.emit 5"]),
        ),
        from_source(
            "test.emit-in-run",
            "E-EFFECT",
            &[],
            "[ \"hi\" test.emit ] run 5",
            &[],
            session(&["[ \"hi\" test.emit ] run 5"]),
        ),
        resource(),
        live("liveref-input", "E-RESOURCE", "1 +"),
        live("live-slot-dispatch", "E-LIVE-SLOT", "slot.invoke"),
        from_source(
            "run-of-program-input",
            "E-DYNAMIC-RUN",
            &[],
            "run",
            &[Ty::I64, program_input],
            session(&["5 [ 1 + ]", "run"]),
        ),
        recursion("self-recursion", false, false),
        recursion("self-recursion-kernel", false, true),
        recursion("mutual-recursion", true, false),
        recursion("mutual-recursion-kernel", true, true),
        from_source(
            "reflect",
            "E-REFLECT",
            &[],
            "[ 1 ] reflect drop 5",
            &[],
            session(&["[ 1 ] reflect drop 5"]),
        ),
        from_source(
            "if",
            "E-CONTROL",
            &[],
            "true [ 1 ] [ 2 ] if",
            &[],
            session(&["true [ 1 ] [ 2 ] if"]),
        ),
        from_source(
            "case",
            "E-CONTROL",
            &[],
            "5 inl [ 1 + ] [ drop 0 ] case",
            &[],
            session(&["5 inl [ 1 + ] [ drop 0 ] case"]),
        ),
        from_source(
            "list.case",
            "E-CONTROL",
            &[],
            "1 nil cons [ 0 ] [ drop drop 1 ] list.case",
            &[],
            session(&["1 nil cons [ 0 ] [ drop drop 1 ] list.case"]),
        ),
        from_source(
            "program-output",
            "E-PROGRAM-OUTPUT",
            &[],
            "[ 1 + ]",
            &[],
            session(&["[ 1 + ]"]),
        ),
        Case {
            name: "expansion-chain-k20",
            subject: "d0 = [ 1 + ], dk = [ dk-1 dk-1 ] (k = 1..20, one shared row each); root `0 d20 drop 7`"
                .to_owned(),
            expected: "E-BUDGET-EXPANSION",
            result: default_outcome(&k20, &k20_closure),
            ordinary: Ordinary::Session(chain_session(20)),
        },
    ]
}

/// `noble session` lines for the `d_k` chain (the frontend exhausts at `d10`).
#[must_use]
pub fn chain_session(k: u32) -> Vec<String> {
    let mut lines = vec!["def d0 [ 1 + ]".to_owned()];
    for level in 1..=k {
        lines.push(format!("def d{level} [ d{p} d{p} ]", p = level - 1));
    }
    lines.push(format!("0 d{k} drop 7"));
    lines
}
