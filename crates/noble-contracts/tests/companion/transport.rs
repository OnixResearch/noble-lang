//! The transport rules restate an existing theorem; they never mint a second
//! entry for the same contract, claim and subject.

#[test]
fn identity_rules_restate_a_premise_and_refuse_a_duplicate() -> Result<(), String> {
    // `admit-lean`, `guard`, `project` and `invoke` transport an existing
    // theorem; they never mint a second entry for the same contract, claim and
    // subject, so the only sound replay result is `DuplicateEntry`.
    for (contract_bytes, proof, rules) in [
        (
            super::INCREMENT,
            super::PROOF,
            &[
                noble_contracts::companion::RuleId::AdmitLeanV1,
                noble_contracts::companion::RuleId::ProjectV1,
                noble_contracts::companion::RuleId::InvokeV1,
            ][..],
        ),
        (
            super::GUARDED,
            super::GUARDED_PROOF,
            &[
                noble_contracts::companion::RuleId::AdmitLeanV1,
                noble_contracts::companion::RuleId::GuardV1,
                noble_contracts::companion::RuleId::ProjectV1,
                noble_contracts::companion::RuleId::InvokeV1,
            ][..],
        ),
    ] {
        assert!(
            !rules.is_empty(),
            "every contract restates at least one rule"
        );
        let contract = super::prepared(contract_bytes)?;
        let mut core = noble_contracts::companion::Core::new(super::LIMITS);
        let (admission, subject) = super::proved_evidence(&mut core, &contract, proof)?;
        let evidence = admission.evidence.ok_or("proved without evidence")?;
        // The transport rules compare against the retained subject, which
        // `bind` returns after checking the observation applies.
        let retained = core
            .bind(evidence, &subject)
            .map_err(|error| format!("bind: {error:?}"))?;
        for rule in rules {
            assert_eq!(
                core.replay(&super::derivation(
                    *rule,
                    &admission,
                    &retained,
                    vec![evidence]
                )?),
                Err(noble_contracts::companion::Refusal::DuplicateEntry),
                "{rule:?} restates the premise instead of minting"
            );
        }
    }
    Ok(())
}
