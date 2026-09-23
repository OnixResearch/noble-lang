//! Deterministic companion security regressions; real-Lean controls live in the CLI.
#![cfg(test)]

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};
const INCREMENT: &[u8] = include_bytes!("../../../verification/mc2/contracts/increment.contract");
const PROOF: &[u8] = include_bytes!("../../../verification/mc2/contracts/increment.proof.lean");
const GUARDED: &[u8] = include_bytes!("../../../verification/mc2/contracts/guarded.contract");

fn prepared(source: &[u8]) -> Result<noble_contracts::Prepared, String> {
    noble_contracts::prepare(source, LIMITS).map_err(|error| format!("{error:?}"))
}

fn offer(bytes: &[u8]) -> noble_contracts::companion::EvidenceOffer {
    noble_contracts::companion::EvidenceOffer {
        class: noble_contracts::companion::EvidenceClass::LeanExact,
        refutation: false,
        payload: noble_contracts::companion::EvidencePayload::Declaration(bytes.to_vec()),
    }
}

fn retained_subject(
    core: &noble_contracts::companion::Core,
    admission: &noble_contracts::companion::Admission,
    prepared: &noble_contracts::Prepared,
) -> Result<noble_contracts::companion::Subject, String> {
    let events = core
        .contract_program(admission.contract.ok_or("missing retained contract")?)
        .map_err(|error| format!("missing retained program: {error:?}"))?;
    Ok(noble_contracts::companion::observe(
        events,
        noble_contracts::companion::InterfaceSignatures {
            input: noble_contracts::companion::interface_signature(
                &prepared.checked().interface.stack_in,
            ),
            output: noble_contracts::companion::interface_signature(
                &prepared.checked().interface.stack_out,
            ),
        },
    ))
}

fn derivation(
    rule: noble_contracts::companion::RuleId,
    admission: &noble_contracts::companion::Admission,
    subject: &noble_contracts::companion::Subject,
    premises: Vec<noble_contracts::companion::EvidenceId>,
) -> Result<noble_contracts::companion::Derivation, String> {
    Ok(noble_contracts::companion::Derivation {
        rule,
        contract: admission.contract.ok_or("missing retained contract")?,
        statement: admission.statement_digest,
        subject: subject.clone(),
        premises,
    })
}

#[test]
fn serialized_status_and_matching_digest_never_admit_a_proof() -> Result<(), String> {
    let contract = prepared(INCREMENT)?;
    let mut core = noble_contracts::companion::Core::new(LIMITS);
    let header = format!(
        "-- noble-evidence v1 statement {}\ncertified: true\nlean wire",
        noble_contracts::companion::statement_digest(&contract)
    );
    for payload in [
        header.as_bytes(),
        PROOF,
        b"{\"tag\":\"Certified\",\"accepted\":true}",
    ] {
        let result = core.admit(&contract, offer(payload));
        assert_eq!(
            result.refusals,
            [noble_contracts::companion::Refusal::ForgedStatus]
        );
        assert_eq!(result.evidence, None);
    }
    assert_eq!(
        core.release(noble_contracts::companion::EvidenceId(0)),
        Err(noble_contracts::companion::Refusal::UnknownEvidence)
    );
    Ok(())
}

#[test]
fn zero_premise_replay_cannot_turn_an_unproved_contract_into_authority() -> Result<(), String> {
    let contract = prepared(INCREMENT)?;
    let mut core = noble_contracts::companion::Core::new(LIMITS);
    let unproved = core.admit(&contract, offer(&[]));
    let subject = retained_subject(&core, &unproved, &contract)?;
    for rule in [
        noble_contracts::companion::RuleId::AdmitLeanV1,
        noble_contracts::companion::RuleId::ComposeV1,
        noble_contracts::companion::RuleId::InstantiateV1,
        noble_contracts::companion::RuleId::GuardV1,
        noble_contracts::companion::RuleId::ProjectV1,
        noble_contracts::companion::RuleId::InvokeV1,
    ] {
        assert_eq!(
            core.replay(&derivation(rule, &unproved, &subject, vec![])?),
            Err(noble_contracts::companion::Refusal::MissingPremise)
        );
    }
    assert_eq!(
        core.release(noble_contracts::companion::EvidenceId(0)),
        Err(noble_contracts::companion::Refusal::UnknownEvidence)
    );
    Ok(())
}

#[test]
fn capability_and_encoding_refusals_are_distinct_and_do_not_register() -> Result<(), String> {
    let contract = prepared(INCREMENT)?;
    for payload in [
        noble_contracts::companion::EvidencePayload::Resource(7),
        noble_contracts::companion::EvidencePayload::ServiceCapability(9),
    ] {
        let mut core = noble_contracts::companion::Core::new(LIMITS);
        let result = core.admit(
            &contract,
            noble_contracts::companion::EvidenceOffer {
                payload,
                ..offer(&[])
            },
        );
        assert_eq!(
            result.refusals,
            [noble_contracts::companion::Refusal::LiveCapabilityInEvidence]
        );
        assert_eq!(
            core.contract_program(noble_contracts::companion::ContractId(0)),
            Err(noble_contracts::companion::Refusal::UnknownContract)
        );
    }
    let result = noble_contracts::companion::Core::new(LIMITS).admit(&contract, offer(&[0xff]));
    assert_eq!(
        result.refusals,
        [noble_contracts::companion::Refusal::InvalidEvidenceEncoding]
    );
    Ok(())
}

#[test]
fn admission_and_observation_exhaustion_fail_before_retention() -> Result<(), String> {
    let contract = prepared(INCREMENT)?;
    for limits in [
        noble_contracts::Limits { work: 0, ..LIMITS },
        noble_contracts::Limits { nodes: 0, ..LIMITS },
    ] {
        let mut core = noble_contracts::companion::Core::new(limits);
        let result = core.admit(&contract, offer(&[]));
        assert_eq!(
            result.refusals,
            [noble_contracts::companion::Refusal::ExhaustedRegistry]
        );
        assert_eq!(
            core.contract_program(noble_contracts::companion::ContractId(0)),
            Err(noble_contracts::companion::Refusal::UnknownContract)
        );
    }
    let mut core = noble_contracts::companion::Core::new(LIMITS);
    let oversized = vec![(1, 1); 2049];
    assert_eq!(
        core.observe_and_register(
            &oversized,
            noble_contracts::companion::InterfaceSignatures {
                input: 1,
                output: 1
            },
        ),
        Err(noble_contracts::companion::Refusal::ExhaustedRegistry)
    );
    Ok(())
}

#[test]
fn nested_interfaces_and_effects_have_distinct_identities() {
    let integer = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
        noble_kernel::types::EffSet::empty(),
    );
    let boolean = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::Bool],
        vec![noble_kernel::types::Ty::Bool],
        noble_kernel::types::EffSet::empty(),
    );
    let effectful = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
        noble_kernel::types::EffSet::from_ids(&[noble_kernel::types::EffId(17)]),
    );
    assert_ne!(
        noble_contracts::companion::interface_signature(std::slice::from_ref(&integer)),
        noble_contracts::companion::interface_signature(&[boolean])
    );
    assert_ne!(
        noble_contracts::companion::interface_signature(&[integer]),
        noble_contracts::companion::interface_signature(&[effectful])
    );
}

#[test]
fn guard_recognition_does_not_weaken_a_larger_or_unrelated_precondition() -> Result<(), String> {
    let source = std::str::from_utf8(GUARDED).map_err(|error| error.to_string())?;
    for predicate in [
        "(and (lt (in x) 9223372036854775807) (eq (in x) 0))",
        "(not (lt (in x) 9223372036854775807))",
        "(lt 0 9223372036854775807)",
    ] {
        let changed = source.replace("(lt (in x) 9223372036854775807)", predicate);
        let mut core = noble_contracts::companion::Core::new(LIMITS);
        let admission = core.admit(&prepared(changed.as_bytes())?, offer(&[]));
        assert_eq!(
            core.guard_templates(admission.contract.ok_or("missing retained contract")?),
            Err(noble_contracts::companion::Refusal::UnsupportedGuardTemplate)
        );
    }
    let mut core = noble_contracts::companion::Core::new(LIMITS);
    let admission = core.admit(&prepared(GUARDED)?, offer(&[]));
    assert_eq!(
        core.guard_templates(admission.contract.ok_or("missing retained contract")?),
        Ok(vec![noble_contracts::companion::GuardTemplate::LtI64Max])
    );
    Ok(())
}
