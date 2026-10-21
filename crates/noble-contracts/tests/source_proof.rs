#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::underscore_in_module_filename,
    tigerstyle::assertion_density,
    tigerstyle::compound_assertion,
    tigerstyle::excessive_file_length,
    tigerstyle::function_length,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::platform_dependent_cast,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};
const MODULE: &[u8] = b"module selected@1 [ def selected [ 1 + ] contract 1 law [ subject selected input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 1)) ] ] proof 1 verified for law [ (export-unary-I64 (intro (tail Stack) (intro (x I64) (pc-sequence (pc-exact (exec-literal 1)) (pc-exact (exec-add)) (bridge (by-exact-append-assoc)) (join (by-exact-result))))) (mc1-true-eq-wrap)) ] ]";

fn live() -> Result<noble_contracts::source::Session, String> {
    noble_contracts::source::Session::new_live_slots(
        noble_kernel::contracts::environment()
            .map_err(|error| format!("{error:?}"))?
            .enable_live_slots(),
    )
    .map_err(|error| format!("{error:?}"))
}

// This deliberately exercises *only* the source-candidate correspondence:
// synthetic revisions are not Lean receipts, so no test treats this as proof
// authorization. The actual pinned independent checker belongs to the host.
fn source_only_session() -> Result<noble_contracts::source::ModuleSession, String> {
    let session = noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid retained source cap"),
    )
    .map_err(|error| format!("{error:?}"))?;
    let prepared = session
        .prepare(MODULE, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
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

fn context(
    session: &noble_contracts::source::ModuleSession,
) -> noble_contracts::source::proof::ProofContext<'static> {
    const MODEL: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const CHECKER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    noble_contracts::source::proof::ProofContext {
        module_generation: session.generation(),
        model_revision: MODEL,
        checker_revision: CHECKER,
    }
}

#[test]
fn selected_live_definition_is_independently_checked_but_not_a_proof_receipt() -> Result<(), String>
{
    let mut source = live()?;
    let definition = source
        .prepare(b"def selected [ 1 + ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(definition.is_definition());
    assert!(
        definition.submission().is_none(),
        "a declaration is not a checked target"
    );
    source
        .commit(definition)
        .map_err(|error| format!("{error:?}"))?;
    let selected = source
        .prepare(b"selected", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let target = source
        .checked_selected_target(&selected, "selected")
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        target.body().interface.stack_in,
        [noble_kernel::types::Ty::I64]
    );
    assert_eq!(
        target.body().interface.stack_out,
        [noble_kernel::types::Ty::I64]
    );
    assert!(target.body().interface.effects.is_empty());
    assert_eq!(target.submission().definitions.len(), 1);
    assert!(target.body().live_sites.is_empty());
    let modules = source_only_session()?;
    let candidate = modules
        .correspond_checked_target(
            "selected",
            1,
            "law",
            "verified",
            &target,
            context(&modules),
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(candidate.definition_id(), target.definition_id());
    assert_eq!(
        candidate.interface().stack_in,
        [noble_kernel::types::Ty::I64]
    );
    assert_eq!(candidate.captures().len(), 0);
    assert_eq!(candidate.assumptions().len(), 0);
    Ok(())
}

#[test]
fn selected_quote_site_binds_original_token_and_checked_i64_operand() -> Result<(), String> {
    let mut source = live()?;
    let definition = source
        .prepare(b"def builder [ quote [ + ] compose ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source
        .commit(definition)
        .map_err(|error| format!("{error:?}"))?;
    let prepared = source
        .prepare(b"builder", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = source
        .checked_selected_target(&prepared, "builder")
        .map_err(|error| format!("{error:?}"))?;
    let node = noble_kernel::untrusted::NodeId(0);
    let site = selected
        .checked_quote_site(node)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        site.interface().stack_in.last(),
        Some(&noble_kernel::types::Ty::I64)
    );
    assert!(matches!(
        site.interface().stack_out.last(),
        Some(noble_kernel::types::Ty::Program(..))
    ));
    Ok(())
}

#[test]
fn quote_site_rejects_non_quote_words_and_non_i64_capture() -> Result<(), String> {
    let mut source = live()?;
    let definition = source
        .prepare(b"def arithmetic [ 1 + ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source
        .commit(definition)
        .map_err(|error| format!("{error:?}"))?;
    let prepared = source
        .prepare(b"arithmetic", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = source
        .checked_selected_target(&prepared, "arithmetic")
        .map_err(|error| format!("{error:?}"))?;
    assert!(
        matches!(
            selected.checked_quote_site(noble_kernel::untrusted::NodeId(0)),
            Err(noble_contracts::source::proof::SourceProofRefusal::UnsupportedProfile)
        ),
        "literal is not a quote site"
    );
    assert!(
        matches!(
            selected.checked_quote_site(noble_kernel::untrusted::NodeId(1)),
            Err(noble_contracts::source::proof::SourceProofRefusal::UnsupportedProfile)
        ),
        "arithmetic builtin is not quote"
    );

    let mut other = live()?;
    let definition = other
        .prepare(b"def non-i64 [ unit quote ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    other
        .commit(definition)
        .map_err(|error| format!("{error:?}"))?;
    let prepared = other
        .prepare(b"non-i64", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = other
        .checked_selected_target(&prepared, "non-i64")
        .map_err(|error| format!("{error:?}"))?;
    assert!(
        matches!(
            selected.checked_quote_site(noble_kernel::untrusted::NodeId(1)),
            Err(noble_contracts::source::proof::SourceProofRefusal::UnsupportedProfile)
        ),
        "Unit capture is not I64"
    );
    Ok(())
}

#[test]
fn quote_site_accepts_checked_nested_occurrence_and_refuses_rebound_source() -> Result<(), String> {
    let mut nested = live()?;
    let definition = nested
        .prepare(b"def nested [ [ 2 quote ] ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    nested
        .commit(definition)
        .map_err(|error| format!("{error:?}"))?;
    let prepared = nested
        .prepare(b"nested", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = nested
        .checked_selected_target(&prepared, "nested")
        .map_err(|error| format!("{error:?}"))?;
    let nested_site = selected
        .checked_quote_site(noble_kernel::untrusted::NodeId(1))
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(nested_site.node(), noble_kernel::untrusted::NodeId(1));
    assert_eq!(
        nested_site.interface().stack_in.last(),
        Some(&noble_kernel::types::Ty::I64)
    );

    let mut source = live()?;
    let original = source
        .prepare(b"def builder [ quote [ + ] compose ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source
        .commit(original)
        .map_err(|error| format!("{error:?}"))?;
    let old = source
        .prepare(b"builder", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let old_span = {
        let selected = source
            .checked_selected_target(&old, "builder")
            .map_err(|error| format!("{error:?}"))?;
        selected
            .checked_quote_site(noble_kernel::untrusted::NodeId(0))
            .map_err(|error| format!("{error:?}"))?
            .authored_span()
            .ok_or("authored quote lost its source token")?
    };
    let replacement = source
        .prepare(b"def builder [ 0 drop quote [ + ] compose ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source
        .commit(replacement)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        source.checked_selected_target(&old, "builder"),
        Err(noble_contracts::source::proof::SourceProofRefusal::StaleContext)
    ));
    let current = source
        .prepare(b"builder", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = source
        .checked_selected_target(&current, "builder")
        .map_err(|error| format!("{error:?}"))?;
    assert!(
        matches!(
            selected.checked_quote_site(noble_kernel::untrusted::NodeId(0)),
            Err(noble_contracts::source::proof::SourceProofRefusal::UnsupportedProfile)
        ),
        "old node selects a literal now"
    );
    let site = selected
        .checked_quote_site(noble_kernel::untrusted::NodeId(2))
        .map_err(|error| format!("{error:?}"))?;
    assert_ne!(
        site.authored_span(),
        Some(old_span),
        "old source occurrence cannot stand in for replacement"
    );
    Ok(())
}

#[test]
fn lexical_binding_replay_authenticates_generated_checked_dip_and_quote() -> Result<(), String> {
    use noble_contracts::source::proof::CheckedLexicalKind;
    use noble_kernel::untrusted::Node;
    let mut source = live()?;
    let bytes = b"def builder [ let x x quote [ + ] compose ]";
    let definition = source
        .prepare(bytes, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source
        .commit(definition)
        .map_err(|error| format!("{error:?}"))?;
    let prepared = source
        .prepare(b"builder", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = source
        .checked_selected_target(&prepared, "builder")
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        selected.body().interface.stack_in,
        [noble_kernel::types::Ty::I64]
    );
    assert!(matches!(
        selected.body().interface.stack_out.as_slice(),
        [noble_kernel::types::Ty::Program(..)]
    ));
    assert!(
        selected
            .candidate_to_lexical()
            .iter()
            .enumerate()
            .any(|(checked, lexical)| checked as u32 != *lexical),
        "source projection must retain its distinct checked and lexical NodeIds"
    );
    let relocated_quotation = selected
        .definition()
        .body
        .candidate
        .nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| match node {
            Node::Quotation { body, .. }
                if body
                    .iter()
                    .any(|child| selected.lexical_source_node(*child) != Some(child.0))
                    && selected
                        .lexical_source_node(noble_kernel::untrusted::NodeId(index as u32))
                        .is_some_and(|lexical| lexical >= selected.original_lexical_nodes()) =>
            {
                Some(noble_kernel::untrusted::NodeId(index as u32))
            }
            _ => None,
        })
        .ok_or("lowering did not relocate a quotation's checked children")?;
    let relocated_site = selected
        .checked_operation_site(relocated_quotation)
        .map_err(|error| format!("relocated quotation lost lexical correspondence: {error:?}"))?;
    assert!(relocated_site.lexical_node >= selected.original_lexical_nodes());
    assert!(relocated_site.generated_witness.is_some());
    let mut cleanup = 0;
    let mut projection = 0;
    for lexical in selected.original_lexical_nodes()..selected.generated_lexical_end() {
        let node = selected
            .checked_lexical_node(lexical)
            .map_err(|error| format!("{error:?}"))?;
        assert!(
            node.authored_span.is_none(),
            "generated node borrowed an authored span"
        );
        let witness = node
            .generated_witness
            .ok_or("generated node lost lexical replay witness")?;
        if witness.generation() == "cleanup-drop" {
            assert!(matches!(node.kind, CheckedLexicalKind::Builtin(1)));
            assert_eq!(witness.occurrence_node(), None);
            assert!(witness.binder_node().is_some());
            cleanup += 1;
        } else if witness.generation() == "projection-dup" {
            assert!(witness.occurrence_node().is_some());
            assert!(witness.binder_node().is_some());
            projection += 1;
        }
    }
    assert!(
        cleanup > 0 && projection > 0,
        "the checked expansion must bind actual cleanup and projection to original local"
    );
    let mut generated_dips = 0;
    for (index, node) in selected
        .definition()
        .body
        .candidate
        .nodes
        .iter()
        .enumerate()
    {
        if matches!(node, Node::Invocation { def, .. } if def.0 == 3) {
            let site = selected
                .checked_operation_site(noble_kernel::untrusted::NodeId(index as u32))
                .map_err(|error| format!("{error:?}"))?;
            assert_eq!(
                site.authored_span, None,
                "generated lexical dip cannot acquire an authored token span"
            );
            assert_eq!(
                site.generated_witness
                    .ok_or("generated dip has no replay witness")?
                    .generation(),
                "wrap-dip"
            );
            assert!(site.lexical_node >= selected.original_lexical_nodes());
            generated_dips += 1;
        }
    }
    assert!(
        generated_dips > 0,
        "physical hidden-value projection must be source-bound"
    );
    let authored_quote = selected
        .definition()
        .body
        .candidate
        .nodes
        .iter()
        .enumerate()
        .find_map(|(index, node)| {
            matches!(node,
            Node::Invocation { def, .. } if def.0 == 8)
            .then_some(noble_kernel::untrusted::NodeId(index as u32))
            .filter(|id| {
                selected
                    .checked_operation_site(*id)
                    .is_ok_and(|site| site.authored_span.is_some())
            })
        })
        .ok_or("authored quote has no checked kernel site")?;
    let quote = selected
        .checked_quote_site(authored_quote)
        .map_err(|error| format!("{error:?}"))?;
    let span = quote
        .authored_span()
        .ok_or("authored quote lost source span")?;
    assert_eq!(&bytes[span.start as usize..span.end as usize], b"quote");
    Ok(())
}

#[test]
fn lexical_replay_uses_historical_named_first_shadow_and_not_later_namespace() -> Result<(), String>
{
    use noble_kernel::untrusted::Node;
    let mut source = live()?;
    // An authored `dup` is a call to the prior named body, not the bootstrap
    // builtin. Independently replaying a spelling as a builtin would silently
    // claim the wrong source operation.
    for text in [
        b"def dup [ 1 + ]".as_slice(),
        b"def shadowed [ dup quote [ + ] compose ]",
    ] {
        let declaration = source
            .prepare(text, &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        source
            .commit(declaration)
            .map_err(|error| format!("{error:?}"))?;
    }
    let shadowed = source
        .prepare(b"shadowed", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let checked = source
        .checked_selected_target(&shadowed, "shadowed")
        .map_err(|error| format!("prior named shadow: {error:?}"))?;
    assert!(matches!(checked.definition().body.candidate.nodes.first(),
        Some(Node::Invocation { def, .. }) if def.0 != 0));
    let quote = checked
        .checked_quote_site(noble_kernel::untrusted::NodeId(1))
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        quote.interface().stack_in.last(),
        Some(&noble_kernel::types::Ty::I64)
    );

    // A later shadow is excluded from the original declaration's immutable
    // namespace, even though it is visible to new source submissions.
    let fresh = source
        .prepare(b"def earlier [ quote [ + ] compose ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source.commit(fresh).map_err(|error| format!("{error:?}"))?;
    let later = source
        .prepare(b"def + [ 1 ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source.commit(later).map_err(|error| format!("{error:?}"))?;
    let prepared = source
        .prepare(b"earlier", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let selected = source
        .checked_selected_target(&prepared, "earlier")
        .map_err(|error| format!("later namespace shadow: {error:?}"))?;
    assert_eq!(
        selected.body().interface.stack_in,
        [noble_kernel::types::Ty::I64]
    );
    assert!(selected
        .checked_quote_site(noble_kernel::untrusted::NodeId(0))
        .is_ok());
    Ok(())
}

#[test]
fn source_target_rejects_old_snapshot_and_wrappers() -> Result<(), String> {
    let mut source = live()?;
    let definition = source
        .prepare(b"def selected [ 1 + ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source
        .commit(definition)
        .map_err(|error| format!("{error:?}"))?;
    let stale = source
        .prepare(b"selected", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let wrapper = source
        .prepare(b"[ selected ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        source.checked_selected_target(&wrapper, "selected"),
        Err(noble_contracts::source::proof::SourceProofRefusal::MismatchedDefinition)
    ));
    let two_nodes = source
        .prepare(b"0 selected", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        source.checked_selected_target(&two_nodes, "selected"),
        Err(noble_contracts::source::proof::SourceProofRefusal::MismatchedDefinition)
    ));
    let second = source
        .prepare(b"def selected [ 2 + ]", &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source
        .commit(second)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        source.checked_selected_target(&stale, "selected"),
        Err(noble_contracts::source::proof::SourceProofRefusal::StaleContext)
    ));
    let current = source
        .prepare(b"selected", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let current = source
        .checked_selected_target(&current, "selected")
        .map_err(|error| format!("{error:?}"))?;
    let modules = source_only_session()?;
    assert!(matches!(
        modules.correspond_checked_target(
            "selected",
            1,
            "law",
            "verified",
            &current,
            context(&modules),
            LIMITS,
        ),
        Err(noble_contracts::source::proof::SourceProofRefusal::MismatchedSource)
    ));
    Ok(())
}

#[test]
fn selected_target_rejects_prepared_source_from_another_live_slot_environment() -> Result<(), String>
{
    let nominal = noble_kernel::contracts::NominalDecl {
        id: noble_kernel::types::NominalTypeId {
            module: 302,
            ordinal: 0,
        },
        shape: noble_kernel::types::NominalShape::Opaque(Box::new(
            noble_kernel::types::Ty::Resource(noble_kernel::types::ResourceKind(51)),
        )),
        exported: true,
        public: [false, false],
    };
    let base = noble_kernel::contracts::environment()
        .map_err(|error| format!("{error:?}"))?
        .enable_live_slots();
    let registered = base
        .clone()
        .register_live_resource(nominal.clone())
        .map_err(|error| format!("{error:?}"))?;
    let mut source = noble_contracts::source::Session::new_live_slots(registered)
        .map_err(|error| format!("{error:?}"))?;
    let mut foreign = noble_contracts::source::Session::new_live_slots(base.clone())
        .map_err(|error| format!("{error:?}"))?;
    let mut identical = noble_contracts::source::Session::new_live_slots(
        base.clone()
            .register_live_resource(nominal.clone())
            .map_err(|error| format!("{error:?}"))?,
    )
    .map_err(|error| format!("{error:?}"))?;
    for session in [&mut source, &mut foreign, &mut identical] {
        let definition = session
            .prepare(b"def selected [ 1 + ]", &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        session
            .commit(definition)
            .map_err(|error| format!("{error:?}"))?;
    }
    let account = noble_kernel::types::Ty::Nominal(nominal.id, Box::new(nominal.shape.clone()));
    let prepared = source
        .prepare(
            b"selected",
            &[account.clone(), noble_kernel::types::Ty::I64],
            LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(
        foreign
            .prepare(
                b"selected",
                &[account, noble_kernel::types::Ty::I64],
                LIMITS
            )
            .is_err(),
        "foreign environment must not independently admit the registered resource"
    );
    assert!(
        matches!(
            foreign.checked_selected_target(&prepared, "selected"),
            Err(noble_contracts::source::proof::SourceProofRefusal::StaleContext)
        ),
        "foreign session must not borrow the accepted submission/environment of another owner"
    );
    assert!(matches!(identical.checked_selected_target(&prepared, "selected"),
        Err(noble_contracts::source::proof::SourceProofRefusal::StaleContext)),
        "equal source bytes and environment capabilities do not grant a different session the preparation");
    source
        .checked_selected_target(&prepared, "selected")
        .map_err(|error| format!("original source session lost its own target: {error:?}"))?;
    source
        .register_live_resource("Account@1", nominal.id)
        .map_err(|error| format!("{error:?}"))?;
    assert!(
        matches!(
            source.checked_selected_target(&prepared, "selected"),
            Err(noble_contracts::source::proof::SourceProofRefusal::StaleContext)
        ),
        "a registered resource changed the original session after preparation"
    );
    Ok(())
}

#[test]
fn equal_recipe_identity_cannot_swap_resolved_named_target() -> Result<(), String> {
    let mut source = live()?;
    for definition in [b"def selected [ 1 + ]".as_slice(), b"def other [ 1 + ]"] {
        let prepared = source
            .prepare(definition, &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        source
            .commit(prepared)
            .map_err(|error| format!("{error:?}"))?;
    }
    let wrong_root = source
        .prepare(b"other", &[noble_kernel::types::Ty::I64], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        source.checked_selected_target(&wrong_root, "selected"),
        Err(noble_contracts::source::proof::SourceProofRefusal::MismatchedDefinition)
    ));
    Ok(())
}

#[path = "source_proof/part2.rs"]
mod part2;
