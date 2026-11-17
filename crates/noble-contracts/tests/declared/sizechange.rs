//! Size-change evidence over checked definitions (issue #101).
//!
//! Each case is one module in the signed declared-text profile; the assertion is the verdict the
//! front end reports, which the Lean lane re-checks by evaluating the generated certificate.

use noble_contracts::source::{
    Argument, Bound, Evidence, GraphEdge, Measure, MeasureIndex, ModuleSession, Relation,
    RetainedSourceLimit, Unsupported, Verdict,
};

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn retained() -> RetainedSourceLimit {
    RetainedSourceLimit::new(LIMITS.bytes).expect("size-change test source limit")
}

fn session() -> Result<ModuleSession, String> {
    ModuleSession::new_declared_text(&[], true, retained()).map_err(|error| format!("{error:?}"))
}

/// The evidence one module declaration installs.
fn analyze(source: &[u8]) -> Result<Vec<Evidence>, String> {
    let prepared = session()?
        .prepare(source, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    Ok(prepared.size_change_evidence())
}

fn only(evidence: Vec<Evidence>) -> Evidence {
    assert_eq!(evidence.len(), 1, "one recursive definition expected");
    evidence.into_iter().next().expect("one entry")
}

fn module(body: &str) -> Vec<u8> {
    std::format!("module m@1 [ {body} ]").into_bytes()
}

fn graph_edge(indices: (u8, u8)) -> Result<GraphEdge, String> {
    let (source_index, target_index) = indices;
    let source =
        MeasureIndex::new(source_index).ok_or_else(|| "source index is out of range".to_owned())?;
    let target =
        MeasureIndex::new(target_index).ok_or_else(|| "target index is out of range".to_owned())?;
    Ok(GraphEdge { source, target })
}

/// Structural list recursion: the cons branch passes the tail, a structural decrease.
#[test]
fn structural_list_recursion_is_accepted() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature go forall<S:stack> [ S List<I64> -- S I64 ! pure ] export go \
         def go [ [ 0 ] [ swap drop go ] list.case ]",
    ))?);
    assert_eq!(entry.definition, "go");
    assert_eq!(entry.measures, vec![Measure::Structural]);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Tail]);
    assert_eq!(
        entry.sites[0].graph.get(graph_edge((0, 0))?),
        noble_contracts::source::Relation::Strict
    );
    assert!(matches!(
        entry.verdict,
        Verdict::Accepted {
            shared_measure: Some(0),
            ..
        }
    ));
    Ok(())
}

/// A guarded countdown: `0 <` is the positivity guard the counter predecessor needs.
#[test]
fn guarded_countdown_is_accepted() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - down ] [ ] if ]",
    ))?);
    assert_eq!(entry.measures, vec![Measure::Counter]);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Predecessor]);
    assert_eq!(entry.sites[0].guarded, vec![true]);
    assert!(matches!(
        entry.verdict,
        Verdict::Accepted {
            shared_measure: Some(0),
            ..
        }
    ));
    Ok(())
}

/// REC-01's zero test is not a positivity guard: the branch with the predecessor has no `0 <`, so
/// the argument is opaque and the definition is refused.
#[test]
fn zero_test_is_not_a_positivity_guard() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 = [ ] [ 1 - down ] if ]",
    ))?);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Opaque]);
    assert_eq!(entry.sites[0].guarded, vec![false]);
    assert!(matches!(entry.verdict, Verdict::Refused { .. }));
    Ok(())
}

/// A non-decreasing self-call: refused, and the witness names the site that fails the measure.
#[test]
fn non_decreasing_self_call_is_refused_with_its_site() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature same forall<S:stack> [ S I64 -- S I64 ! pure ] export same def same [ same ]",
    ))?);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Same]);
    match entry.verdict {
        Verdict::Refused {
            witnesses, cycle, ..
        } => {
            assert_eq!(witnesses.len(), 1);
            assert_eq!(witnesses[0].measure, 0);
            assert_eq!(witnesses[0].site, 0);
            assert_eq!(witnesses[0].argument, Argument::Same);
            assert_eq!(cycle.path, vec![0]);
            assert_eq!(
                cycle.graph.get(graph_edge((0, 0))?),
                noble_contracts::source::Relation::Same
            );
        }
        other => return Err(std::format!("expected a refusal, got {other:?}")),
    }
    Ok(())
}

/// Wrapping `I64` arithmetic as a measure: unclassifiable, refused rather than assumed decreasing.
#[test]
fn wrapping_arithmetic_is_refused() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature wrap forall<S:stack> [ S I64 -- S I64 ! pure ] export wrap def wrap [ 1 + wrap ]",
    ))?);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Opaque]);
    match entry.verdict {
        Verdict::Refused { witnesses, .. } => {
            assert_eq!(witnesses[0].argument, Argument::Opaque);
        }
        other => return Err(std::format!("expected a refusal, got {other:?}")),
    }
    Ok(())
}

/// A guard on one measure does not justify another measure's predecessor: the guarded measure is
/// the shared measure, the unguarded one stays opaque.
#[test]
fn guard_justifies_only_the_measure_it_names() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature two forall<S:stack> [ S I64 I64 -- S I64 I64 ! pure ] export two \
         def two [ dup 0 swap < [ 1 - swap 1 - swap two ] [ ] if ]",
    ))?);
    assert_eq!(entry.measures, vec![Measure::Counter, Measure::Counter]);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(entry.sites[0].guarded, vec![false, true]);
    assert_eq!(
        entry.sites[0].arguments,
        vec![Argument::Opaque, Argument::Predecessor]
    );
    assert!(matches!(
        entry.verdict,
        Verdict::Accepted {
            shared_measure: Some(1),
            ..
        }
    ));
    Ok(())
}

/// A strict edge followed by an argument permutation is accepted only after composition.
#[test]
fn argument_permutation_is_accepted_by_composed_closure() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature perm forall<S:stack> [ S I64 I64 -- S I64 I64 ! pure ] export perm \
         def perm [ dup 0 swap < [ 1 - swap perm ] [ ] if ]",
    ))?);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(
        entry.sites[0].arguments,
        vec![Argument::Opaque, Argument::Opaque]
    );
    assert_eq!(
        entry.sites[0].graph.get(graph_edge((1, 0))?),
        Relation::Strict
    );
    assert_eq!(
        entry.sites[0].graph.get(graph_edge((0, 1))?),
        Relation::Same
    );
    match entry.verdict {
        Verdict::Accepted {
            shared_measure,
            closure,
            strict_witnesses,
        } => {
            assert_eq!(shared_measure, None);
            assert_eq!(closure.len(), 3);
            assert_eq!(strict_witnesses.len(), 1);
            assert_eq!(strict_witnesses[0].closure_entry, 1);
        }
        other => return Err(std::format!("expected closure acceptance, got {other:?}")),
    }
    Ok(())
}

/// A permutation without a decrease exposes an idempotent identity graph as its refusal witness.
#[test]
fn argument_swap_is_refused_with_idempotent_cycle() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature cycle forall<S:stack> [ S I64 I64 -- S I64 I64 ! pure ] export cycle \
         def cycle [ swap cycle ]",
    ))?);
    assert_eq!(entry.sites.len(), 1);
    assert_eq!(
        entry.sites[0].arguments,
        vec![Argument::Opaque, Argument::Opaque]
    );
    assert_eq!(
        entry.sites[0].graph.get(graph_edge((0, 1))?),
        Relation::Same
    );
    assert_eq!(
        entry.sites[0].graph.get(graph_edge((1, 0))?),
        Relation::Same
    );
    match entry.verdict {
        Verdict::Refused { closure, cycle, .. } => {
            assert_eq!(closure.len(), 2);
            assert_eq!(cycle.path, vec![0, 0]);
            assert_eq!(cycle.graph.get(graph_edge((0, 0))?), Relation::Same);
            assert_eq!(cycle.graph.get(graph_edge((1, 1))?), Relation::Same);
        }
        other => {
            return Err(std::format!(
                "expected an idempotent-cycle refusal, got {other:?}"
            ))
        }
    }
    Ok(())
}

/// A measure cap is a distinct incomplete-analysis result, not a divergence verdict.
#[test]
fn excess_parameters_report_bound_exceeded() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature big forall<S:stack> [ S I64 I64 I64 I64 I64 I64 I64 I64 I64 -- \
         S I64 I64 I64 I64 I64 I64 I64 I64 I64 ! pure ] export big def big [ big ]",
    ))?);
    assert!(matches!(
        entry.verdict,
        Verdict::BoundExceeded {
            bound: Bound::Parameters,
            ..
        }
    ));
    Ok(())
}

/// The call-site cap is distinct from the walk's ordinary unsupported-profile budgets.
#[test]
fn excess_call_sites_report_bound_exceeded() -> Result<(), String> {
    let sites = std::string::String::from("same ").repeat(257);
    let source = std::format!(
        "module m@1 [ signature same forall<S:stack> [ S I64 -- S I64 ! pure ] \
         export same def same [ {sites} ] ]"
    );
    let entry = only(analyze(source.as_bytes())?);
    assert!(matches!(
        entry.verdict,
        Verdict::BoundExceeded {
            bound: Bound::CallSites,
            ..
        }
    ));
    Ok(())
}

/// Several sites all passing the same measure as its predecessor certify the family.
#[test]
fn two_sites_sharing_the_measure_are_accepted() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - dup 0 swap < [ down ] [ down ] if ] [ ] if ]",
    ))?);
    assert_eq!(entry.sites.len(), 2);
    assert_eq!(entry.sites[0].guarded, vec![true]);
    assert_eq!(entry.sites[1].guarded, vec![true]);
    assert!(matches!(
        entry.verdict,
        Verdict::Accepted {
            shared_measure: Some(0),
            ..
        }
    ));
    Ok(())
}

/// One decreasing branch cannot hide another self-call that passes the measure unchanged.
#[test]
fn one_non_decreasing_site_refuses_the_shared_measure() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - down ] [ down ] if ]",
    ))?);
    assert_eq!(entry.sites.len(), 2);
    assert_eq!(entry.sites[0].arguments, vec![Argument::Predecessor]);
    assert_eq!(entry.sites[1].arguments, vec![Argument::Same]);
    match entry.verdict {
        Verdict::Refused { witnesses, .. } => {
            assert_eq!(witnesses.len(), 1);
            assert_eq!(witnesses[0].site, 1);
            assert_eq!(witnesses[0].argument, Argument::Same);
        }
        other => return Err(std::format!("expected a refusal, got {other:?}")),
    }
    Ok(())
}

/// A module with no self call installs no evidence.
#[test]
fn non_recursive_definitions_carry_no_evidence() -> Result<(), String> {
    let evidence = analyze(&module(
        "signature twice forall<S:stack> [ S I64 -- S I64 ! pure ] export twice \
         def twice [ 1 + 1 + ]",
    ))?;
    assert!(
        evidence.is_empty(),
        "expected no evidence, got {evidence:?}"
    );
    Ok(())
}

/// A signature outside the bounded profile reports `unsupported`, never a verdict.
#[test]
fn signature_outside_the_profile_is_unsupported() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature flag forall<S:stack> [ S Bool -- S Bool ! pure ] export flag def flag [ flag ]",
    ))?);
    assert!(
        matches!(
            &entry.verdict,
            Verdict::Unsupported {
                reason: Unsupported::SignatureOutsideProfile,
                ..
            }
        ),
        "expected an unsupported verdict, got {:?}",
        entry.verdict
    );
    Ok(())
}

/// A body whose programs the walk cannot model reports `unsupported` rather than guessing.
#[test]
fn unmodelled_program_construct_is_unsupported() -> Result<(), String> {
    let entry = only(analyze(&module(
        "signature spin forall<S:stack> [ S I64 -- S I64 ! pure ] export spin \
         def spin [ 1 quote run drop spin ]",
    ))?);
    assert!(
        matches!(
            entry.verdict,
            Verdict::Unsupported {
                reason: Unsupported::UnmodelledCall,
                ..
            }
        ),
        "expected an unmodelled-call verdict, got {:?}",
        entry.verdict
    );
    Ok(())
}

/// A named call may execute its own recursion, so a direct decreasing self-call cannot hide it.
#[test]
fn named_recursive_helper_is_not_assumed_terminating() -> Result<(), String> {
    let evidence = analyze(&module(
        "signature spin forall<S:stack> [ S I64 -- S I64 ! pure ] export spin \
         def spin [ spin ] \
         signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - down ] [ ] if spin ]",
    ))?;
    let entry = evidence
        .iter()
        .find(|entry| entry.definition == "down")
        .ok_or_else(|| std::string::String::from("missing recursive caller"))?;
    assert!(
        matches!(
            &entry.verdict,
            Verdict::Unsupported {
                reason: Unsupported::UnmodelledCall,
                ..
            }
        ),
        "recursive helper must not be treated as returning: {:?}",
        entry.verdict
    );
    Ok(())
}

/// The certificate carries a checked closure for acceptance and a path-bearing refusal witness.
#[test]
fn certificate_checks_closures_and_refusals() -> Result<(), String> {
    let accepted = only(analyze(&module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - down ] [ ] if ]",
    ))?);
    let refused = only(analyze(&module(
        "signature same forall<S:stack> [ S I64 -- S I64 ! pure ] export same def same [ same ]",
    ))?);
    let certificate = noble_contracts::source::certificate(&[accepted, refused]);
    assert!(certificate.contains("import NobleM2.SizeChangeSCT"));
    assert!(certificate.contains("def definition_0Sites : Fin 1 → MGraph 1"));
    assert!(certificate.contains("def definition_0Entries : List (MatrixClosureEntry 1 1)"));
    assert!(certificate.contains("def definition_0Certificate : MatrixClosureCertificate"));
    assert!(certificate.contains("theorem definition_0Criterion"));
    assert!(certificate.contains("definition_00"));
    assert!(certificate.contains("def definition_1ClosureSetCertificate"));
    assert!(certificate.contains("def definition_1Refusal : MatrixIdempotentRefusal"));
    assert!(certificate.contains("theorem definition_1RefusalGraphIsClosure"));
    assert!(certificate.contains("theorem definition_1RefusalHasNoStrictDiagonal"));
    assert!(certificate.contains("decide"));
    Ok(())
}

/// Signed-owner admission meters every source/draft comparison of the symbolic check in a fixed
/// order: the first admitting work bound and the sequence of exhausted steps are pinned.
#[test]
fn signed_owner_admission_meters_a_fixed_work_bound() -> Result<(), String> {
    let source = module(
        "signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - down ] [ ] if ]",
    );
    let declared = session()?;
    let sweep = super::first_admitting_bound(|limits| declared.prepare(&source, &[], limits))?;
    assert_eq!(sweep.bound, 3_410);
    assert_eq!(sweep.refusals.len(), 150);
    Ok(())
}

/// A signed owner calling a named helper also meters each namespace word it binds, scanned from
/// the last binding: the first admitting work bound and the sequence of exhausted steps are pinned.
#[test]
fn signed_owner_with_named_helper_meters_a_fixed_work_bound() -> Result<(), String> {
    let source = module(
        "signature first forall<S:stack> [ S I64 -- S I64 ! pure ] export first def first [ 1 + ] \
         signature spin forall<S:stack> [ S I64 -- S I64 ! pure ] export spin def spin [ spin ] \
         signature down forall<S:stack> [ S I64 -- S I64 ! pure ] export down \
         def down [ dup 0 swap < [ 1 - down ] [ ] if spin ]",
    );
    let declared = session()?;
    let sweep = super::first_admitting_bound(|limits| declared.prepare(&source, &[], limits))?;
    assert_eq!(sweep.bound, 8_068);
    assert_eq!(sweep.refusals.len(), 224);
    Ok(())
}

/// The walk stops at its first unmodelled word: a later well-formed self call is never recorded.
#[test]
fn walk_stops_at_its_first_unmodelled_word() -> Result<(), String> {
    let evidence = analyze(&module(
        "signature spin forall<S:stack> [ S I64 -- S I64 ! pure ] export spin def spin [ spin ] \
         signature step forall<S:stack> [ S I64 -- S I64 ! pure ] export step \
         def step [ spin step ]",
    ))?;
    let entry = evidence
        .iter()
        .find(|entry| entry.definition == "step")
        .ok_or_else(|| std::string::String::from("missing recursive caller"))?;
    assert!(
        entry.sites.is_empty(),
        "sites after the halt: {:?}",
        entry.sites
    );
    assert!(
        matches!(
            entry.verdict,
            Verdict::Unsupported {
                reason: Unsupported::UnmodelledCall,
                ..
            }
        ),
        "expected an unmodelled-call verdict, got {:?}",
        entry.verdict
    );
    Ok(())
}
