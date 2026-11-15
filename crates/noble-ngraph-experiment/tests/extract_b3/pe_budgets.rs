//! Direct-library budget accounting at the Program-to-residual boundary.

use super::pe_refusals::{prepared, residualize};
use super::{Budget, Request, BLAKE3};
use noble_kernel::execution::Submission;
use noble_kernel::types::Ty;
use noble_ngraph::check::{self, Budgets};
use noble_ngraph::diag::{BudgetKind, Code};
use noble_ngraph::extract;

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

fn set_cap(budgets: &mut Budgets, kind: BudgetKind, value: u64) -> bool {
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
        BudgetKind::Fuel | BudgetKind::LiteralBytes => return false,
    }
    true
}

#[test]
fn program_budget_charges_have_inclusive_observed_boundaries() {
    let original = prepared(&["def inc [ 1 + ]"], "inc", &[Ty::I64]);
    let defaults = Budgets::default();
    let baseline = residualize(&original.submission, &original.claimed_closure, &[None], &defaults)
        .expect("baseline Program and independently prepared residual");
    let first = extraction(&original.submission, &original.claimed_closure, &defaults);
    let last = extraction(
        baseline.prepared.submission().expect("residual submission"),
        &[],
        &defaults,
    );
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
        assert!(set_cap(&mut below, kind, count - 1));
        let diagnostic = residualize(&original.submission, &original.claimed_closure, &[None], &below)
            .expect_err("a below-observed cap cannot publish a partial Program");
        assert_eq!(diagnostic.code, Code::Budget(kind), "{kind:?}: {diagnostic}");
        assert_eq!(diagnostic.budget.expect("budget count").0, count - 1);
        let mut at = defaults.clone();
        assert!(set_cap(&mut at, kind, count));
        assert!(residualize(&original.submission, &original.claimed_closure, &[None], &at).is_ok(),
            "{kind:?}: the observed bound must be inclusive");
        let mut above = defaults.clone();
        assert!(set_cap(&mut above, kind, count + 1));
        if kind == BudgetKind::Variants {
            assert_eq!(residualize(&original.submission, &original.claimed_closure, &[None], &above)
                .expect_err("raising the /0 variant policy").code, Code::Profile);
        } else {
            assert!(residualize(&original.submission, &original.claimed_closure, &[None], &above).is_ok(),
                "{kind:?}: one above the observed charge is still within policy");
        }
    }
}

// Excluded budget       | Proof for this exact admitted pure, flat Program profile
// ----------------------|----------------------------------------------------------
// AffineTerms           | Extractor emits Input/Const/I64Add/Sub/Mul/Eq, never F32Affine.
// TableLength/Entries   | The same whitelist has no F32Index table operation.
// Facts                 | Source extraction emits no facts/guards; PE and re-admission preserve zero.
// SnapshotBytes         | Snapshot construction is a separate post-check operation, not residualization.
// Forward/BackwardScratch | Evaluation/training workspaces are never built or invoked here.
// Steps                 | Optimizer host calls do not occur in source preparation or residualization.
// Fuel/LiteralBytes     | Not in Budgets: runtime fuel and F32 literal parsing are outside this API.
#[test]
fn unreachable_profile_budget_classes_are_lower_only_not_faux_exhaustion() {
    use BudgetKind::{
        AffineTerms, BackwardScratch, Facts, ForwardScratch, SnapshotBytes, Steps,
        TableEntries, TableLength,
    };
    let original = prepared(&[], "1 +", &[Ty::I64]);
    let defaults = Budgets::default();
    for (kind, profile_cap) in [
        (AffineTerms, defaults.affine_terms),
        (TableLength, defaults.table_length_entries),
        (TableEntries, defaults.table_entries),
        (Facts, defaults.facts),
        (SnapshotBytes, defaults.snapshot_bytes),
        (ForwardScratch, defaults.forward_scratch),
        (BackwardScratch, defaults.backward_scratch),
        (Steps, defaults.optimizer_steps),
    ] {
        let mut lower = defaults.clone();
        assert!(set_cap(&mut lower, kind, 0));
        let result = residualize(&original.submission, &[], &[None], &lower)
            .expect("no excluded class charges in the admitted profile");
        assert!(result.specialized.graph().facts.is_empty(), "{kind:?}");
        assert!(result.readmitted.graph().facts.is_empty(), "{kind:?}");
        let mut raised = defaults.clone();
        assert!(set_cap(&mut raised, kind, profile_cap + 1));
        assert_eq!(residualize(&original.submission, &[], &[None], &raised)
            .expect_err("raised /0 cap refuses by policy, not exhaustion").code,
            Code::Profile, "{kind:?}");
    }
}
