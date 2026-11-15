//! Direct-library Program refusal matrix. `residualize` receives no runtime or
//! host callback; an `Err` cannot contain a partial residual Program or graph.

use super::{source, Budget, Request, BLAKE3};
use noble_kernel::execution::Submission;
use noble_kernel::types::{EffSet, Ty};
use noble_ngraph::check::{self, Budgets};
use noble_ngraph::diag::{BudgetKind, Code, Diagnostic};
use noble_ngraph::{extract, specialize::program};

fn prepared(defs: &[&str], body: &str, inputs: &[Ty]) -> source::Prepared {
    source::prepare(
        noble_contracts::source::Session::new(),
        noble_contracts::Limits::default(),
        defs,
        body,
        inputs,
    )
    .expect("ordinary source preparation")
}

fn residualize(
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
    refused("duplicate", &original.submission, &[id, id], &[None], Code::Closure);
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
    refused("I64 value for Bool", &boolean.submission, &[], &[Some(5)], Code::Interface);

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
    refused("forged dependency list", &wrong_dependencies, &claimed, &[], Code::Closure);
}

#[test]
fn program_refuses_effect_resource_reflection_dynamic_run_and_recursion() {
    let effect = prepared(&[], "\"hi\" test.emit 5", &[]);
    refused("host effect", &effect.submission, &[], &[], Code::Effect);
    let reflect = prepared(&[], "[ 1 ] reflect drop 5", &[]);
    refused("reflection", &reflect.submission, &[], &[], Code::Reflect);
    let program_type = Ty::program(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    let dynamic = prepared(&[], "run", &[Ty::I64, program_type]);
    refused("dynamic run", &dynamic.submission, &[], &[], Code::DynamicRun);

    // Core source cannot supply a Resource. Use the same declared-module
    // producer as the existing B3 control, then re-check its submission.
    let retained = noble_contracts::source::RetainedSourceLimit::new(
        noble_contracts::Limits::default().bytes,
    )
    .expect("default source bytes are retainable");
    let module = noble_contracts::source::ModuleSession::new(&[], retained)
        .expect("declared-module session");
    let resource = module
        .prepare(
            b"swap",
            &[Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE), Ty::I64],
            noble_contracts::Limits::default(),
        )
        .expect("resource subject is ordinarily prepared");
    refused("resource interface", resource.submission().expect("submission"), &[], &[], Code::Resource);

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
    let budgets = Budgets { expanded_nodes: 33, ..Budgets::default() };
    let diagnostic = residualize(&original, &closure, &[], &budgets)
        .expect_err("34 expanded nodes must refuse, despite pruning to one constant");
    assert_eq!(diagnostic.code, Code::Budget(BudgetKind::Expansion));
    assert_eq!(diagnostic.budget, Some((33, 34)));
    for bound in [34, 35] {
        let budgets = Budgets { expanded_nodes: bound, ..Budgets::default() };
        assert!(residualize(&original, &closure, &[], &budgets).is_ok());
    }
}

fn extraction(sub: &Submission, closure: &[u64], budgets: &Budgets) -> extract::Extraction {
    let admitted = extract::admit(
        &BLAKE3,
        sub,
        &Request {
            profile: "ngraph-i64/0",
            claimed_closure: closure,
            budget: Budget::default(),
        },
    )
    .expect("ordinary graph admission");
    extract::extract(&BLAKE3, &admitted, budgets).expect("ordinary graph extraction")
}

fn set_cap(budgets: &mut Budgets, kind: BudgetKind, value: u64) {
    use BudgetKind::{
        AffineTerms, BackwardScratch, Expansion, Facts, ForwardScratch, GraphBytes, InlineDepth,
        Inputs, Layers, LiveNodes, Outputs, ResidualNodes, SnapshotBytes, Steps, TableEntries,
        TableLength, Variants, Work,
    };
    match kind {
        Work => budgets.work_units = value,
        Expansion => budgets.expanded_nodes = value,
        InlineDepth => budgets.inline_depth = value,
        LiveNodes => budgets.live_nodes = value,
        Layers => budgets.layers = value,
        Inputs => budgets.inputs = value,
        Outputs => budgets.outputs = value,
        GraphBytes => budgets.graph_bytes = value,
        ResidualNodes => budgets.residual_nodes = value,
        Variants => budgets.variants = value,
        AffineTerms => budgets.affine_terms = value,
        TableLength => budgets.table_length_entries = value,
        TableEntries => budgets.table_entries = value,
        Facts => budgets.facts = value,
        SnapshotBytes => budgets.snapshot_bytes = value,
        ForwardScratch => budgets.forward_scratch = value,
        BackwardScratch => budgets.backward_scratch = value,
        Steps => budgets.optimizer_steps = value,
        BudgetKind::Fuel | BudgetKind::LiteralBytes => panic!("not Program request budgets"),
    }
}

#[test]
fn program_budget_charges_have_inclusive_observed_boundaries() {
    let original = prepared(&["def inc [ 1 + ]"], "inc", &[Ty::I64]);
    let defaults = Budgets::default();
    let baseline = residualize(&original.submission, &original.claimed_closure, &[None], &defaults)
        .expect("baseline Program and independently prepared residual");
    let first = extraction(&original.submission, &original.claimed_closure, &defaults);
    let last = extraction(baseline.prepared.submission().expect("residual submission"), &[], &defaults);
    let checked = [&first.checked, &baseline.specialized.checked, &last.checked];
    let max_checked = |measure: fn(&check::CheckedGraph) -> u64| {
        checked.iter().map(|g| measure(g)).max().expect("three checked graphs")
    };
    let observed = [
        (BudgetKind::Work, first.stats.work_units.max(last.stats.work_units)),
        (BudgetKind::Expansion, first.stats.static_expansion_bound.max(first.stats.expanded_nodes)
            .max(last.stats.static_expansion_bound).max(last.stats.expanded_nodes)),
        (BudgetKind::InlineDepth, first.stats.max_inline_depth.max(last.stats.max_inline_depth)),
        (BudgetKind::LiveNodes, max_checked(|g| g.graph().nodes.len() as u64)),
        (BudgetKind::Layers, max_checked(|g| u64::from(g.layer_count()))),
        (BudgetKind::Inputs, max_checked(|g| g.graph().slots.len() as u64)),
        (BudgetKind::Outputs, max_checked(|g| g.graph().outputs.len() as u64)),
        (BudgetKind::GraphBytes, max_checked(|g| check::retained_bytes(g.graph()))),
        (BudgetKind::ResidualNodes, baseline.specialized.report.original_nodes
            .max(baseline.specialized.report.residual_nodes)),
        (BudgetKind::Variants, baseline.specialized.report.variants),
    ];
    for (kind, count) in observed {
        assert!(count > 0, "{kind:?} must have a reachable positive charge");
        let mut below = defaults.clone();
        set_cap(&mut below, kind, count - 1);
        let diagnostic = residualize(&original.submission, &original.claimed_closure, &[None], &below)
            .expect_err("a below-observed cap cannot publish a partial Program");
        assert_eq!(diagnostic.code, Code::Budget(kind), "{kind:?}: {diagnostic}");
        assert_eq!(diagnostic.budget.expect("budget count").0, count - 1);
        let mut at = defaults.clone();
        set_cap(&mut at, kind, count);
        assert!(residualize(&original.submission, &original.claimed_closure, &[None], &at).is_ok(),
            "{kind:?}: the observed bound must be inclusive");
        let mut above = defaults.clone();
        set_cap(&mut above, kind, count + 1);
        if kind == BudgetKind::Variants {
            assert_eq!(residualize(&original.submission, &original.claimed_closure, &[None], &above)
                .expect_err("raising the /0 variant policy").code, Code::Profile);
        } else {
            assert!(residualize(&original.submission, &original.claimed_closure, &[None], &above).is_ok(),
                "{kind:?}: one above the observed charge is still within policy");
        }
    }
}

// Excluded budget | Proof for this exact admitted pure, flat Program profile
// ----------------|----------------------------------------------------------
// AffineTerms     | Extractor emits Input/Const/I64Add/Sub/Mul/Eq, never F32Affine.
// TableLength/Entries | The same whitelist has no F32Index table operation.
// Facts           | Source extraction emits no facts/guards; PE and re-admission preserve zero.
// SnapshotBytes   | Snapshot construction is a separate post-check operation, not residualization.
// Forward/BackwardScratch | Evaluation/training workspaces are never built or invoked here.
// Steps           | Optimizer host calls do not occur in source preparation or residualization.
// Fuel/LiteralBytes | Not in Budgets: runtime fuel and F32 literal parsing are outside this API.
#[test]
fn unreachable_profile_budget_classes_are_lower_only_not_faux_exhaustion() {
    use BudgetKind::{
        AffineTerms, BackwardScratch, Facts, ForwardScratch, SnapshotBytes, Steps,
        TableEntries, TableLength,
    };
    let original = prepared(&[], "1 +", &[Ty::I64]);
    for kind in [
        AffineTerms, TableLength, TableEntries, Facts, SnapshotBytes, ForwardScratch,
        BackwardScratch, Steps,
    ] {
        let mut lower = Budgets::default();
        set_cap(&mut lower, kind, 0);
        let result = residualize(&original.submission, &[], &[None], &lower)
            .expect("no excluded class charges in the admitted profile");
        assert!(result.specialized.graph().facts.is_empty(), "{kind:?}");
        assert!(result.readmitted.graph().facts.is_empty(), "{kind:?}");
        let mut raised = Budgets::default();
        let default = match kind {
            AffineTerms => raised.affine_terms,
            TableLength => raised.table_length_entries,
            TableEntries => raised.table_entries,
            Facts => raised.facts,
            SnapshotBytes => raised.snapshot_bytes,
            ForwardScratch => raised.forward_scratch,
            BackwardScratch => raised.backward_scratch,
            Steps => raised.optimizer_steps,
            _ => panic!("only excluded request caps belong in this table"),
        };
        set_cap(&mut raised, kind, default + 1);
        assert_eq!(residualize(&original.submission, &[], &[None], &raised)
            .expect_err("raised /0 cap refuses by policy, not exhaustion").code,
            Code::Profile, "{kind:?}");
    }
}
