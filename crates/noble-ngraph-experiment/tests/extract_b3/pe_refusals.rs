//! Direct-library Program refusal matrix. `residualize` receives no runtime or
//! host callback; an `Err` cannot contain a partial residual Program or graph.

use super::{source, BLAKE3};
use noble_kernel::execution::Submission;
use noble_kernel::types::{EffSet, Ty};
use noble_ngraph::check::Budgets;
use noble_ngraph::diag::{BudgetKind, Code, Diagnostic};
use noble_ngraph::specialize::program;

pub(super) fn prepared(defs: &[&str], body: &str, inputs: &[Ty]) -> source::Prepared {
    source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        defs,
        body,
        inputs,
    )
    .expect("ordinary source preparation")
}

pub(super) fn residualize(
    original: &Submission,
    closure: &[u64],
    static_inputs: &[Option<i64>],
    budgets: &Budgets,
) -> Result<program::Residual, Diagnostic> {
    program::residualize(
        &BLAKE3,
        &program::Request {
            original,
            claimed_closure: closure,
            expected_original: None,
            preserved_tail: 0,
            static_inputs,
            budgets,
            source_limits: noble_contracts::Limits::default(),
        },
    )
}

fn refused(
    label: &str,
    original: &Submission,
    closure: &[u64],
    static_inputs: &[Option<i64>],
    expected: Code,
) {
    let diagnostic = residualize(original, closure, static_inputs, &Budgets::default())
        .expect_err("refusal cannot publish a Program or a graph");
    assert_eq!(diagnostic.code, expected, "{label}: {diagnostic}");
}

#[test]
fn program_rejects_untrusted_subject_closure_and_binding_errors() {
    let original = prepared(&["def inc [ 1 + ]"], "inc", &[Ty::I64]);
    let id = original.claimed_closure[0];
    refused("missing", &original.submission, &[], &[None], Code::Closure);
    refused(
        "extra",
        &original.submission,
        &[id, u64::MAX],
        &[None],
        Code::Closure,
    );
    refused(
        "duplicate",
        &original.submission,
        &[id, id],
        &[None],
        Code::Closure,
    );
    refused(
        "missing binding",
        &original.submission,
        &original.claimed_closure,
        &[],
        Code::Interface,
    );
    refused(
        "extra binding",
        &original.submission,
        &original.claimed_closure,
        &[None, Some(5)],
        Code::Interface,
    );
    let boolean = prepared(&[], "drop 7", &[Ty::Bool]);
    refused(
        "I64 value for Bool",
        &boolean.submission,
        &[],
        &[Some(5)],
        Code::Interface,
    );

    let mut malformed = original.clone();
    malformed
        .submission
        .body
        .candidate
        .body
        .push(noble_kernel::untrusted::NodeId(u32::MAX));
    refused(
        "forged candidate",
        &malformed.submission,
        &malformed.claimed_closure,
        &[None],
        Code::Subject,
    );
    let (mut wrong_dependencies, claimed) = super::chain(2);
    let row = wrong_dependencies.definitions[1].definition.0 as usize;
    wrong_dependencies.environment.deps[row].clear();
    refused(
        "forged dependency list",
        &wrong_dependencies,
        &claimed,
        &[],
        Code::Closure,
    );
}

#[test]
fn program_refuses_effect_resource_reflection_dynamic_run_and_recursion() {
    let effect = prepared(&[], "\"hi\" test.emit 5", &[]);
    refused("host effect", &effect.submission, &[], &[], Code::Effect);
    let reflect = prepared(&[], "[ 1 ] reflect drop 5", &[]);
    refused("reflection", &reflect.submission, &[], &[], Code::Reflect);
    let program_type = Ty::program(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    let dynamic = prepared(&[], "run", &[Ty::I64, program_type]);
    refused(
        "dynamic run",
        &dynamic.submission,
        &[],
        &[],
        Code::DynamicRun,
    );

    // Core source cannot supply a Resource. Use the same declared-module
    // producer as the existing B3 control, then re-check its submission.
    let retained =
        noble_contracts::source::RetainedSourceLimit::new(noble_contracts::Limits::default().bytes)
            .expect("default source bytes are retainable");
    let module = noble_contracts::source::ModuleSession::new(&[], retained)
        .expect("declared-module session");
    let resource = module
        .prepare(
            b"swap",
            &[
                Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE),
                Ty::I64,
            ],
            noble_contracts::Limits::default(),
        )
        .expect("resource subject is ordinarily prepared");
    refused(
        "resource interface",
        resource.submission().expect("submission"),
        &[],
        &[],
        Code::Resource,
    );

    // The frontend cannot produce recursive definitions: forge a call in a
    // previously prepared body and leave its producer dependency list stale.
    let mut recursive = prepared(&["def f [ dup drop ]"], "5 f", &[]);
    let target = recursive.submission.definitions[0].definition;
    let body = &mut recursive.submission.definitions[0].body.candidate;
    body.nodes = vec![noble_kernel::untrusted::Node::Invocation {
        def: target,
        inst: noble_kernel::words::Inst { bindings: vec![] },
    }];
    body.body = vec![noble_kernel::untrusted::NodeId(0)];
    refused(
        "producer-impossible self recursion",
        &recursive.submission,
        &recursive.claimed_closure,
        &[],
        Code::Recursion,
    );
}

#[test]
fn program_refuses_pre_pruning_explosion_without_constructing_a_large_chain() {
    let (original, closure) = super::chain(4);
    let budgets = Budgets {
        expanded_nodes: 33,
        ..Budgets::default()
    };
    let diagnostic = residualize(&original, &closure, &[], &budgets)
        .expect_err("34 expanded nodes must refuse, despite pruning to one constant");
    assert_eq!(diagnostic.code, Code::Budget(BudgetKind::Expansion));
    assert_eq!(diagnostic.budget, Some((33, 34)));
    for bound in [34, 35] {
        let budgets = Budgets {
            expanded_nodes: bound,
            ..Budgets::default()
        };
        assert!(residualize(&original, &closure, &[], &budgets).is_ok());
    }
}
