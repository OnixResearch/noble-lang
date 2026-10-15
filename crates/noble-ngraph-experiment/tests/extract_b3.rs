//! Phase B B3: source-level refusals of D7 admission/extraction.
//!
//! Every refusal is an `Err(Diagnostic)` returned before any graph exists,
//! so no graph node is evaluated (there is nothing to evaluate). The
//! ordinary-path behavior of the same sources is recorded by the L1 harness
//! receipt (`verification/ngraph/b3-ordinary-path.json`).

use noble_kernel::contracts::{Behavior, Definition};
use noble_kernel::execution::Submission;
use noble_kernel::types::{EffSet, Ty};
use noble_kernel::untrusted::{Node, NodeId};
use noble_ngraph_experiment::diag::{BudgetKind, Code};
use noble_ngraph_experiment::extract::{self, source, Budget, Request};
use noble_ngraph_experiment::Diagnostic;

fn outcome(
    sub: &Submission,
    claimed: &[u64],
    profile: &str,
    budget: Budget,
) -> Result<(), Diagnostic> {
    let admitted = extract::admit(
        sub,
        &Request {
            profile,
            claimed_closure: claimed,
            budget,
        },
    )?;
    extract::extract(&admitted).map(|_| ())
}

fn refusal(sub: &Submission, claimed: &[u64]) -> Diagnostic {
    outcome(sub, claimed, "ngraph-i64/0", Budget::default()).expect_err("must be refused")
}

fn refuse_source(defs: &[&str], root: &str, inputs: &[Ty]) -> Code {
    let p = source::prepare(defs, root, inputs).expect("ordinarily valid source");
    refusal(&p.submission, &p.claimed_closure).code
}

#[test]
fn host_operation_test_emit() {
    assert_eq!(refuse_source(&[], "\"hi\" test.emit 5", &[]), Code::Effect);
    assert_eq!(
        refuse_source(&[], "[ \"hi\" test.emit ] run 5", &[]),
        Code::Effect
    );
    // A dropped, never-run effectful quotation leaves the root bound empty
    // and is never reached: the subject extracts to `const 5` (the ordinary
    // path emits nothing either).
    let p = source::prepare(&[], "[ \"hi\" test.emit ] drop 5", &[]).expect("source");
    assert!(outcome(
        &p.submission,
        &p.claimed_closure,
        "ngraph-i64/0",
        Budget::default()
    )
    .is_ok());
}

#[test]
fn reflect() {
    assert_eq!(
        refuse_source(&[], "[ 1 ] reflect drop 5", &[]),
        Code::Reflect
    );
}

#[test]
fn control_if_case_list_case() {
    assert_eq!(
        refuse_source(&[], "true [ 1 ] [ 2 ] if", &[]),
        Code::Control
    );
    assert_eq!(
        refuse_source(&[], "5 inl [ 1 + ] [ drop 0 ] case", &[]),
        Code::Control
    );
    assert_eq!(
        refuse_source(&[], "1 nil cons [ 0 ] [ drop drop 1 ] list.case", &[]),
        Code::Control
    );
}

#[test]
fn program_output() {
    assert_eq!(refuse_source(&[], "[ 1 + ]", &[]), Code::ProgramOutput);
}

#[test]
fn run_of_a_program_input() {
    let program = Ty::program(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    assert_eq!(
        refuse_source(&[], "run", &[Ty::I64, program]),
        Code::DynamicRun
    );
}

#[test]
fn resource_input() {
    let limits = noble_contracts::Limits::default();
    let session = noble_contracts::source::ModuleSession::new(&[]).expect("module session");
    let inputs = [
        Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE),
        Ty::I64,
    ];
    let p = session
        .prepare(b"swap", &inputs, limits)
        .expect("declared-module source");
    let sub = p.submission().expect("expression");
    assert_eq!(refusal(sub, &[]).code, Code::Resource);
}

fn live_slot_session() -> noble_contracts::source::Session {
    let env = noble_kernel::contracts::environment()
        .expect("environment")
        .enable_live_slots();
    noble_contracts::source::Session::new_live_slots(env).expect("live-slot session")
}

#[test]
fn live_ref_input_and_live_slot_dispatch() {
    let limits = noble_contracts::Limits::default();
    let session = live_slot_session();
    let target = Ty::live_ref(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    let inputs = [Ty::I64, target];

    let p = session
        .prepare(b"1 +", &inputs, limits)
        .expect("LiveRef input source");
    assert_eq!(
        refusal(p.submission().expect("expression"), &[]).code,
        Code::Resource
    );

    let p = session
        .prepare(b"slot.invoke", &inputs, limits)
        .expect("dispatch source");
    assert_eq!(
        refusal(p.submission().expect("expression"), &[]).code,
        Code::LiveSlot
    );
}

/// Core source refuses recursion at resolution ("recursive definitions
/// require a signature and are outside Core-Bootstrap"), so a recursive
/// subject can only be forged. Two forgeries of the frontend's own
/// submission are exercised:
/// - body forged, dependency list left as produced: every body is still
///   kernel-accepted, and admission's closure walk refuses the identity
///   reached again below itself with `E-RECURSION`;
/// - body and `Env::deps` forged consistently: the kernel itself refuses the
///   recursive dependency (`Unsupported(RecursiveDependency)`), so admission
///   reports `E-SUBJECT`.
fn forge_call(sub: &mut Submission, at: usize, target: Definition) {
    sub.definitions[at].body.candidate.nodes = vec![Node::Invocation {
        def: target,
        inst: noble_kernel::words::Inst { bindings: vec![] },
    }];
    sub.definitions[at].body.candidate.body = vec![NodeId(0)];
}

#[test]
fn self_recursion() {
    let p = source::prepare(&["def f [ dup drop ]"], "5 f", &[]).expect("source");
    let mut sub = p.submission;
    let f = sub.definitions[0].definition;
    forge_call(&mut sub, 0, f);
    let d = refusal(&sub, &p.claimed_closure);
    assert_eq!(d.code, Code::Recursion, "{d}");
    sub.environment.deps[f.0 as usize] = vec![f];
    assert_eq!(refusal(&sub, &p.claimed_closure).code, Code::Subject);
}

#[test]
fn mutual_recursion() {
    let p = source::prepare(&["def g [ 1 + ]", "def f [ g ]"], "5 f", &[]).expect("source");
    let mut sub = p.submission;
    // definitions: [f, g] in pre-order; make g call f.
    let f = sub.definitions[0].definition;
    let g = sub.definitions[1].definition;
    assert_eq!(sub.environment.kind(g), Some(Behavior::Named));
    forge_call(&mut sub, 1, f);
    let d = refusal(&sub, &p.claimed_closure);
    assert_eq!(d.code, Code::Recursion, "{d}");
    sub.environment.deps[g.0 as usize] = vec![f];
    assert_eq!(refusal(&sub, &p.claimed_closure).code, Code::Subject);
}

#[test]
fn supplied_dependency_list_must_match_the_body() {
    let (mut sub, closure) = chain(2);
    let d1 = sub.definitions[1].definition;
    sub.environment.deps[d1.0 as usize].clear();
    assert_eq!(refusal(&sub, &closure).code, Code::Closure);
}

#[test]
fn subject_and_closure_and_profile() {
    let p =
        source::prepare(&["def sq [ dup * ]"], "sq swap sq +", &source::i64s(2)).expect("source");
    // E-CLOSURE: missing, extra and repeated identities.
    assert_eq!(refusal(&p.submission, &[]).code, Code::Closure);
    let mut extra = p.claimed_closure.clone();
    extra.push(u64::MAX);
    assert_eq!(refusal(&p.submission, &extra).code, Code::Closure);
    let twice = [p.claimed_closure[0], p.claimed_closure[0]];
    assert_eq!(refusal(&p.submission, &twice).code, Code::Closure);
    // E-SUBJECT: the exact subject must be kernel-accepted (here a forged
    // root whose `swap` became `+` no longer matches its witnesses).
    let mut forged = p.submission.clone();
    if let Node::Invocation { def, .. } = &mut forged.body.candidate.nodes[1] {
        *def = Definition(4);
    }
    assert_eq!(refusal(&forged, &p.claimed_closure).code, Code::Subject);
    // E-PROFILE: unknown, F32 (no source extraction under B0(b)) and a
    // raised budget.
    for profile in ["ngraph-i64/1", "ngraph-f32/0"] {
        let d = outcome(
            &p.submission,
            &p.claimed_closure,
            profile,
            Budget::default(),
        )
        .expect_err("profile");
        assert_eq!(d.code, Code::Profile);
    }
    let raised = Budget {
        work_units: Budget::default().work_units + 1,
        ..Budget::default()
    };
    let d = outcome(&p.submission, &p.claimed_closure, "ngraph-i64/0", raised).expect_err("raised");
    assert_eq!(d.code, Code::Profile);
}

/// `d0 = [ 1 + ]`, `dₖ = [ dₖ₋₁ dₖ₋₁ ]`, root `0 d_k drop 7`, as one shared
/// specialization row per `dₖ` (the kernel accepts each two-call body).
/// The Core frontend itself specializes every named use separately and
/// exhausts its 16,384-node limit at `d10`, so this subject is assembled
/// from the frontend's `d0` submission plus `k` hand-built rows.
fn chain(k: u32) -> (Submission, Vec<u64>) {
    let p = source::prepare(&["def d0 [ 1 + ]"], "0 d0 drop 7", &[]).expect("source");
    let mut sub = p.submission;
    let d0 = sub.definitions[0].definition;
    let mut closure = vec![sub.definitions[0].identity];
    let mut prev = d0;
    for level in 1..=k {
        let row = Definition(u32::try_from(sub.environment.defs.len()).expect("row"));
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

#[test]
fn exponential_named_chain_k20_is_refused_even_when_dropped() {
    let (sub, closure) = chain(20);
    let d = refusal(&sub, &closure);
    assert_eq!(d.code, Code::Budget(BudgetKind::Expansion), "{d}");
    assert_eq!(d.budget, Some((262_144, 262_145)));
}

#[test]
fn small_chain_extracts_and_prunes_to_the_constant() {
    let (sub, closure) = chain(4);
    let admitted = extract::admit(
        &sub,
        &Request {
            profile: "ngraph-i64/0",
            claimed_closure: &closure,
            budget: Budget::default(),
        },
    )
    .expect("admitted");
    let x = extract::extract(&admitted).expect("extracted");
    // 0, then 16 × (const 1, add), then 7: 34 expanded, 33 pruned.
    assert_eq!(x.stats.expanded_nodes, 34);
    assert_eq!(x.checked.graph().nodes.len(), 1);
    assert_eq!(x.stats.static_expansion_bound, 34);
    // The same subject under a lowered expansion budget is refused although
    // its pruned graph has one node: pruning cannot launder the expansion.
    let low = Budget {
        expanded_nodes: 33,
        ..Budget::default()
    };
    let d = outcome(&sub, &closure, "ngraph-i64/0", low).expect_err("budget");
    assert_eq!(d.code, Code::Budget(BudgetKind::Expansion));
    let shallow = Budget {
        inline_depth: 4,
        ..Budget::default()
    };
    let d = outcome(&sub, &closure, "ngraph-i64/0", shallow).expect_err("depth");
    assert_eq!(d.code, Code::Budget(BudgetKind::InlineDepth));
    let work = Budget {
        work_units: 20,
        ..Budget::default()
    };
    let d = outcome(&sub, &closure, "ngraph-i64/0", work).expect_err("work");
    assert_eq!(d.code, Code::Budget(BudgetKind::Work));
}
