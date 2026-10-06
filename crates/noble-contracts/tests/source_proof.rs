use noble_contracts::{source::{Session, ModuleSession, proof::{ProofContext, SourceProofRefusal}}, Limits};
use noble_kernel::{types::{Ty, NominalShape, NominalTypeId, ResourceKind}, untrusted::NodeId};

const LIMITS: Limits = Limits { bytes: 65_536, nodes: 16_384, depth: 64, work: 2_000_000 };
const MODULE: &[u8] = b"module selected@1 [ def selected [ 1 + ] contract 1 law [ subject selected input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 1 verified for law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]";

fn live() -> Result<Session, String> {
    Session::new_live_slots(noble_kernel::contracts::environment()
        .map_err(|error| format!("{error:?}"))?.enable_live_slots())
        .map_err(|error| format!("{error:?}"))
}

// This deliberately exercises *only* the source-candidate correspondence:
// synthetic revisions are not Lean receipts, so no test treats this as proof
// authorization. The actual pinned independent checker belongs to the host.
fn source_only_session() -> Result<ModuleSession, String> {
    let session = ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    let prepared = session.prepare(MODULE, &[], LIMITS).map_err(|error| format!("{error:?}"))?;
    let (session, outcome) = session.commit_verified(prepared, |batch| {
        let mut source_checked = noble_contracts::intrinsic::check_batch(batch, LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        for checked in &mut source_checked {
            checked.model_revision = "a".repeat(64);
            checked.checker_revision = "b".repeat(64);
        }
        Ok(source_checked)
    });
    outcome.map_err(|error| format!("{error:?}"))?;
    Ok(session)
}

fn context(session: &ModuleSession) -> ProofContext<'static> {
    const MODEL: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const CHECKER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    ProofContext { module_generation: session.generation(), model_revision: MODEL,
        checker_revision: CHECKER }
}

#[test]
fn selected_live_definition_is_independently_checked_but_not_a_proof_receipt() -> Result<(), String> {
    let mut source = live()?;
    let definition = source.prepare(b"def selected [ 1 + ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(definition.is_definition());
    assert!(definition.submission().is_none(), "a declaration is not a checked target");
    source.commit(definition).map_err(|error| format!("{error:?}"))?;
    let selected = source.prepare(b"selected", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let target = source.checked_selected_target(&selected, "selected")
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(target.body().interface.stack_in, [Ty::I64]);
    assert_eq!(target.body().interface.stack_out, [Ty::I64]);
    assert!(target.body().interface.effects.is_empty());
    assert_eq!(target.submission().definitions.len(), 1);
    assert!(target.body().live_sites.is_empty());
    let modules = source_only_session()?;
    let candidate = modules.correspond_checked_target(
        "selected", 1, "law", "verified", &target, context(&modules), LIMITS,
    ).map_err(|error| format!("{error:?}"))?;
    assert_eq!(candidate.definition_id(), target.definition_id());
    assert_eq!(candidate.interface().stack_in, [Ty::I64]);
    assert_eq!(candidate.captures().len(), 0);
    assert_eq!(candidate.assumptions().len(), 0);
    Ok(())
}

#[test]
fn selected_quote_site_binds_original_token_and_checked_i64_operand() -> Result<(), String> {
    let mut source = live()?;
    let definition = source.prepare(b"def builder [ quote [ + ] compose ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source.commit(definition).map_err(|error| format!("{error:?}"))?;
    let prepared = source.prepare(b"builder", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = source.checked_selected_target(&prepared, "builder")
        .map_err(|error| format!("{error:?}"))?;
    let node = NodeId(0);
    let site = selected.checked_quote_site(node).map_err(|error| format!("{error:?}"))?;
    assert_eq!(site.interface().stack_in.last(), Some(&Ty::I64));
    assert!(matches!(site.interface().stack_out.last(), Some(Ty::Program(..))));
    assert!(matches!(selected.checked_quote_site(NodeId(1)),
        Err(SourceProofRefusal::MismatchedSource)));
    Ok(())
}

#[test]
fn quote_site_rejects_non_quote_words_and_non_i64_capture() -> Result<(), String> {
    let mut source = live()?;
    let definition = source.prepare(b"def arithmetic [ 1 + ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source.commit(definition).map_err(|error| format!("{error:?}"))?;
    let prepared = source.prepare(b"arithmetic", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = source.checked_selected_target(&prepared, "arithmetic")
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(selected.checked_quote_site(NodeId(0)),
        Err(SourceProofRefusal::UnsupportedProfile)), "literal is not a quote site");
    assert!(matches!(selected.checked_quote_site(NodeId(1)),
        Err(SourceProofRefusal::UnsupportedProfile)), "arithmetic builtin is not quote");

    let mut other = live()?;
    let definition = other.prepare(b"def non-i64 [ unit quote ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    other.commit(definition).map_err(|error| format!("{error:?}"))?;
    let prepared = other.prepare(b"non-i64", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = other.checked_selected_target(&prepared, "non-i64")
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(selected.checked_quote_site(NodeId(1)),
        Err(SourceProofRefusal::UnsupportedProfile)), "Unit capture is not I64");
    Ok(())
}

#[test]
fn quote_site_refuses_nested_occurrence_and_rebound_source() -> Result<(), String> {
    let mut nested = live()?;
    let definition = nested.prepare(b"def nested [ [ quote ] ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    nested.commit(definition).map_err(|error| format!("{error:?}"))?;
    let prepared = nested.prepare(b"nested", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = nested.checked_selected_target(&prepared, "nested")
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(selected.checked_quote_site(NodeId(0)),
        Err(SourceProofRefusal::MismatchedSource)), "inner quote is not a root-body site");

    let mut source = live()?;
    let original = source.prepare(b"def builder [ quote [ + ] compose ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source.commit(original).map_err(|error| format!("{error:?}"))?;
    let old = source.prepare(b"builder", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let old_span = {
        let selected = source.checked_selected_target(&old, "builder")
            .map_err(|error| format!("{error:?}"))?;
        selected.checked_quote_site(NodeId(0))
            .map_err(|error| format!("{error:?}"))?.span()
    };
    let replacement = source.prepare(b"def builder [ 0 drop quote [ + ] compose ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source.commit(replacement).map_err(|error| format!("{error:?}"))?;
    assert!(matches!(source.checked_selected_target(&old, "builder"),
        Err(SourceProofRefusal::StaleContext)));
    let current = source.prepare(b"builder", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = source.checked_selected_target(&current, "builder")
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(selected.checked_quote_site(NodeId(0)),
        Err(SourceProofRefusal::UnsupportedProfile)), "old node selects a literal now");
    let site = selected.checked_quote_site(NodeId(2))
        .map_err(|error| format!("{error:?}"))?;
    assert_ne!(site.span(), old_span, "old source occurrence cannot stand in for replacement");
    Ok(())
}

#[test]
fn source_target_rejects_old_snapshot_and_wrappers() -> Result<(), String> {
    let mut source = live()?;
    let definition = source.prepare(b"def selected [ 1 + ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source.commit(definition).map_err(|error| format!("{error:?}"))?;
    let stale = source.prepare(b"selected", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let wrapper = source.prepare(b"[ selected ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(source.checked_selected_target(&wrapper, "selected"),
        Err(SourceProofRefusal::MismatchedDefinition)));
    let two_nodes = source.prepare(b"0 selected", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(source.checked_selected_target(&two_nodes, "selected"),
        Err(SourceProofRefusal::MismatchedDefinition)));
    let second = source.prepare(b"def selected [ 2 + ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source.commit(second).map_err(|error| format!("{error:?}"))?;
    assert!(matches!(source.checked_selected_target(&stale, "selected"),
        Err(SourceProofRefusal::StaleContext)));
    let current = source.prepare(b"selected", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let current = source.checked_selected_target(&current, "selected")
        .map_err(|error| format!("{error:?}"))?;
    let modules = source_only_session()?;
    assert!(matches!(modules.correspond_checked_target(
        "selected", 1, "law", "verified", &current, context(&modules), LIMITS,
    ), Err(SourceProofRefusal::MismatchedSource)));
    Ok(())
}

#[test]
fn equal_recipe_identity_cannot_swap_resolved_named_target() -> Result<(), String> {
    let mut source = live()?;
    for definition in [b"def selected [ 1 + ]".as_slice(), b"def other [ 1 + ]"] {
        let prepared = source.prepare(definition, &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        source.commit(prepared).map_err(|error| format!("{error:?}"))?;
    }
    let wrong_root = source.prepare(b"other", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(source.checked_selected_target(&wrong_root, "selected"),
        Err(SourceProofRefusal::MismatchedDefinition)));
    Ok(())
}

#[test]
fn proof_candidate_refuses_context_claim_evidence_and_resource_interface() -> Result<(), String> {
    let mut source = live()?;
    let definition = source.prepare(b"def selected [ 1 + ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source.commit(definition).map_err(|error| format!("{error:?}"))?;
    let prepared = source.prepare(b"selected", &[Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let target = source.checked_selected_target(&prepared, "selected")
        .map_err(|error| format!("{error:?}"))?;
    let modules = source_only_session()?;
    assert!(matches!(modules.correspond_checked_target(
        "selected", 1, "law", "verified", &target,
        ProofContext { module_generation: 0, ..context(&modules) }, LIMITS,
    ), Err(SourceProofRefusal::StaleContext)));
    assert!(matches!(modules.correspond_checked_target(
        "selected", 1, "law", "verified", &target,
        ProofContext { checker_revision: "unreviewed", ..context(&modules) }, LIMITS,
    ), Err(SourceProofRefusal::MismatchedClaim)));
    assert!(matches!(modules.correspond_checked_target(
        "selected", 1, "wrong-law", "verified", &target, context(&modules), LIMITS,
    ), Err(SourceProofRefusal::MissingEvidence)));
    assert!(matches!(modules.correspond_checked_target(
        "selected", 1, "law", "unproved", &target, context(&modules), LIMITS,
    ), Err(SourceProofRefusal::MissingEvidence)));

    let nominal = noble_kernel::contracts::NominalDecl {
        id: NominalTypeId { module: 302, ordinal: 0 },
        shape: NominalShape::Opaque(Box::new(Ty::Resource(ResourceKind(51)))),
        exported: true, public: [false, false],
    };
    let environment = noble_kernel::contracts::environment()
        .map_err(|error| format!("{error:?}"))?.enable_live_slots()
        .register_live_resource(nominal.clone()).map_err(|error| format!("{error:?}"))?;
    let mut resources = Session::new_live_slots(environment)
        .map_err(|error| format!("{error:?}"))?;
    resources.register_live_resource("Account@1", nominal.id)
        .map_err(|error| format!("{error:?}"))?;
    let definition = resources.prepare(b"def selected [ 1 + ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    resources.commit(definition).map_err(|error| format!("{error:?}"))?;
    let account = resources.parse_live_type("Account@1")
        .map_err(|error| format!("{error:?}"))?;
    let prepared = resources.prepare(b"selected", &[account, Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let target = resources.checked_selected_target(&prepared, "selected")
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(modules.correspond_checked_target(
        "selected", 1, "law", "verified", &target, context(&modules), LIMITS,
    ), Err(SourceProofRefusal::UnsupportedProfile)));
    Ok(())
}
