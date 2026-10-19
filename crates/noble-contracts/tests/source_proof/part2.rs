#[test]
fn source_target_rejects_old_snapshot_and_wrappers() -> Result<(), String> {
    let mut source = super::live()?;
    let definition = source
        .prepare(b"def selected [ 1 + ]", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source
        .commit(definition)
        .map_err(|error| format!("{error:?}"))?;
    let stale = source
        .prepare(b"selected", &[noble_kernel::types::Ty::I64], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let wrapper = source
        .prepare(b"[ selected ]", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        source.checked_selected_target(&wrapper, "selected"),
        Err(noble_contracts::source::proof::SourceProofRefusal::MismatchedDefinition)
    ));
    let two_nodes = source
        .prepare(
            b"0 selected",
            &[noble_kernel::types::Ty::I64],
            super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        source.checked_selected_target(&two_nodes, "selected"),
        Err(noble_contracts::source::proof::SourceProofRefusal::MismatchedDefinition)
    ));
    let second = source
        .prepare(b"def selected [ 2 + ]", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source
        .commit(second)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        source.checked_selected_target(&stale, "selected"),
        Err(noble_contracts::source::proof::SourceProofRefusal::StaleContext)
    ));
    let current = source
        .prepare(b"selected", &[noble_kernel::types::Ty::I64], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let current = source
        .checked_selected_target(&current, "selected")
        .map_err(|error| format!("{error:?}"))?;
    let modules = super::source_only_session()?;
    assert!(matches!(
        modules.correspond_checked_target(
            "selected",
            1,
            "law",
            "verified",
            &current,
            super::context(&modules),
            super::LIMITS,
        ),
        Err(noble_contracts::source::proof::SourceProofRefusal::MismatchedSource)
    ));
    Ok(())
}

#[test]
fn equal_recipe_identity_cannot_swap_resolved_named_target() -> Result<(), String> {
    let mut source = super::live()?;
    for definition in [b"def selected [ 1 + ]".as_slice(), b"def other [ 1 + ]"] {
        let prepared = source
            .prepare(definition, &[], super::LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        source
            .commit(prepared)
            .map_err(|error| format!("{error:?}"))?;
    }
    let wrong_root = source
        .prepare(b"other", &[noble_kernel::types::Ty::I64], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        source.checked_selected_target(&wrong_root, "selected"),
        Err(noble_contracts::source::proof::SourceProofRefusal::MismatchedDefinition)
    ));
    Ok(())
}

#[test]
fn proof_candidate_refuses_context_claim_evidence_and_resource_interface() -> Result<(), String> {
    let mut source = super::live()?;
    let definition = source
        .prepare(b"def selected [ 1 + ]", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    source
        .commit(definition)
        .map_err(|error| format!("{error:?}"))?;
    let prepared = source
        .prepare(b"selected", &[noble_kernel::types::Ty::I64], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let target = source
        .checked_selected_target(&prepared, "selected")
        .map_err(|error| format!("{error:?}"))?;
    let modules = super::source_only_session()?;
    assert!(matches!(
        modules.correspond_checked_target(
            "selected",
            1,
            "law",
            "verified",
            &target,
            noble_contracts::source::proof::ProofContext {
                module_generation: 0,
                ..super::context(&modules)
            },
            super::LIMITS,
        ),
        Err(noble_contracts::source::proof::SourceProofRefusal::StaleContext)
    ));
    assert!(matches!(
        modules.correspond_checked_target(
            "selected",
            1,
            "law",
            "verified",
            &target,
            noble_contracts::source::proof::ProofContext {
                checker_revision: "unreviewed",
                ..super::context(&modules)
            },
            super::LIMITS,
        ),
        Err(noble_contracts::source::proof::SourceProofRefusal::MismatchedClaim)
    ));
    assert!(matches!(
        modules.correspond_checked_target(
            "selected",
            1,
            "wrong-law",
            "verified",
            &target,
            super::context(&modules),
            super::LIMITS,
        ),
        Err(noble_contracts::source::proof::SourceProofRefusal::MissingEvidence)
    ));
    refuses_missing_evidence(&modules, &target)?;

    refuses_resource_interface_and_context_claims(&modules)
}

/// The live-slot resource session must refuse context, claim, evidence and interface drift.
fn refuses_resource_interface_and_context_claims(
    modules: &noble_contracts::source::ModuleSession,
) -> Result<(), String> {
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
    let environment = noble_kernel::contracts::environment()
        .map_err(|error| format!("{error:?}"))?
        .enable_live_slots()
        .register_live_resource(nominal.clone())
        .map_err(|error| format!("{error:?}"))?;
    let mut resources = noble_contracts::source::Session::new_live_slots(environment)
        .map_err(|error| format!("{error:?}"))?;
    resources
        .register_live_resource("Account@1", nominal.id)
        .map_err(|error| format!("{error:?}"))?;
    let definition = resources
        .prepare(b"def selected [ 1 + ]", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    resources
        .commit(definition)
        .map_err(|error| format!("{error:?}"))?;
    let account = resources
        .parse_live_type("Account@1")
        .map_err(|error| format!("{error:?}"))?;
    let prepared = resources
        .prepare(
            b"selected",
            &[account, noble_kernel::types::Ty::I64],
            super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    let target = resources
        .checked_selected_target(&prepared, "selected")
        .map_err(|error| format!("{error:?}"))?;
    assert!(matches!(
        modules.correspond_checked_target(
            "selected",
            1,
            "law",
            "verified",
            &target,
            super::context(&modules),
            super::LIMITS,
        ),
        Err(noble_contracts::source::proof::SourceProofRefusal::UnsupportedProfile)
    ));
    assert_eq!(resources.generation(), 2);
    Ok(())
}

/// A candidate without retained evidence must be refused as MissingEvidence.
fn refuses_missing_evidence<'a>(
    modules: &noble_contracts::source::ModuleSession,
    target: &noble_contracts::source::proof::CheckedSelectedTarget<'a>,
) -> Result<(), String> {
    assert!(matches!(
        modules.correspond_checked_target(
            "selected",
            1,
            "law",
            "unproved",
            target,
            super::context(modules),
            super::LIMITS,
        ),
        Err(noble_contracts::source::proof::SourceProofRefusal::MissingEvidence)
    ));
    Ok(())
}
