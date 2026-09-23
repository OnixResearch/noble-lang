//! Actual independently checked MC2 security controls for the acceptance gate.

fn annotate_last(
    controls: &mut [crate::workflow::encoding::Json],
    key: &'static str,
    value: crate::workflow::encoding::Json,
) {
    let Some(crate::workflow::encoding::Json::Object(control)) = controls.last_mut() else {
        panic!("control object");
    };
    let (_, crate::workflow::encoding::Json::Object(observed)) = control
        .iter_mut()
        .find(|(key, _)| *key == "observed")
        .expect("observed object")
    else {
        panic!("observed object");
    };
    observed.push((key, value));
}

fn report_text<'a>(value: &'a crate::core::report::Value, key: &str) -> Option<&'a str> {
    value.member(key).and_then(crate::core::report::Value::text)
}

fn events_json(events: &[(u32, u64)]) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::Json::Array(
        events
            .iter()
            .map(|(tag, operand)| {
                crate::workflow::encoding::Json::Array(vec![
                    crate::workflow::encoding::Json::Number(u64::from(*tag)),
                    crate::workflow::encoding::Json::Number(*operand),
                ])
            })
            .collect(),
    )
}

fn premises_json(
    premises: &[noble_contracts::companion::EvidenceId],
) -> crate::workflow::encoding::Json {
    crate::workflow::encoding::Json::Array(
        premises
            .iter()
            .map(|id| crate::workflow::encoding::Json::Number(u64::from(id.0)))
            .collect(),
    )
}

mod adversarial;
mod binding;
mod checker;
mod composition;
mod context;
mod derivation;
mod eligibility;

pub(super) const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};
pub(super) const INCREMENT: &[u8] =
    include_bytes!("../../../../../verification/mc2/contracts/increment.contract");
pub(super) const PROOF: &[u8] =
    include_bytes!("../../../../../verification/mc2/contracts/increment.proof.lean");
pub(super) const FAMILY: &[u8] =
    include_bytes!("../../../../../verification/mc2/contracts/builder.contract");
pub(super) const FAMILY_PROOF: &[u8] =
    include_bytes!("../../../../../verification/mc2/contracts/builder.proof.lean");
pub(super) const GUARDED: &[u8] =
    include_bytes!("../../../../../verification/mc2/contracts/guarded.contract");
pub(super) const GUARDED_PROOF: &[u8] =
    include_bytes!("../../../../../verification/mc2/contracts/guarded.proof.lean");

pub(super) fn prepared(source: &[u8]) -> noble_contracts::Prepared {
    noble_contracts::prepare(source, LIMITS).expect("test input prepares")
}

pub(super) fn offer(bytes: &[u8]) -> noble_contracts::companion::EvidenceOffer {
    noble_contracts::companion::EvidenceOffer {
        class: noble_contracts::companion::EvidenceClass::LeanExact,
        refutation: false,
        payload: noble_contracts::companion::EvidencePayload::Declaration(bytes.to_vec()),
    }
}

pub(super) fn retained_subject(
    core: &noble_contracts::companion::Core,
    admission: &noble_contracts::companion::Admission,
    prepared: &noble_contracts::Prepared,
) -> noble_contracts::companion::Subject {
    let events = core
        .contract_program(admission.contract.expect("retained contract"))
        .expect("program");
    let interface = &prepared.checked().interface;
    noble_contracts::companion::observe(
        events,
        noble_contracts::companion::InterfaceSignatures {
            input: noble_contracts::companion::interface_signature(&interface.stack_in),
            output: noble_contracts::companion::interface_signature(&interface.stack_out),
        },
    )
}

pub(super) fn increment_subject(value: i64) -> noble_contracts::companion::Subject {
    let signature =
        noble_contracts::companion::interface_signature(&[noble_kernel::types::Ty::I64]);
    noble_contracts::companion::observe(
        &[(1, u64::from_ne_bytes(value.to_ne_bytes())), (2, 4)],
        noble_contracts::companion::InterfaceSignatures {
            input: signature,
            output: signature,
        },
    )
}

pub(super) fn derivation(
    rule: noble_contracts::companion::RuleId,
    admission: &noble_contracts::companion::Admission,
    subject: &noble_contracts::companion::Subject,
    premises: Vec<noble_contracts::companion::EvidenceId>,
) -> noble_contracts::companion::Derivation {
    noble_contracts::companion::Derivation {
        rule,
        contract: admission.contract.expect("contract"),
        statement: admission.statement_digest,
        subject: subject.clone(),
        premises,
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; control asserts the exact observed refusal once, then encodes a fixed receipt schema from that result; duplicate assertions would add no independent security check."
)]
fn control(
    controls: &mut Vec<crate::workflow::encoding::Json>,
    case: (&str, &str),
    input: crate::workflow::encoding::Json,
    result: Result<(), noble_contracts::companion::Refusal>,
    expected: noble_contracts::companion::Refusal,
    baseline: &crate::workflow::encoding::Json,
) {
    let (case, variant) = case;
    assert_eq!(result, Err(expected), "{case}/{variant}");
    let observed = crate::workflow::encoding::object([
        (
            "diagnostic",
            crate::workflow::encoding::optional_string(
                result.as_ref().err().map(|refusal| refusal.code()),
            ),
        ),
        (
            "new_certified_status",
            crate::workflow::encoding::Json::Bool(result.is_ok()),
        ),
        (
            "resource_ownership_duplicated",
            crate::workflow::encoding::Json::Bool(result.is_ok()),
        ),
    ]);
    controls.push(crate::workflow::encoding::object([
        ("case", crate::workflow::encoding::string(case)),
        ("variant", crate::workflow::encoding::string(variant)),
        ("input", input),
        (
            "execution_capability",
            crate::workflow::encoding::string("none"),
        ),
        ("baseline", baseline.clone()),
        ("observed", observed),
    ]));
}

fn admission_result(
    admission: &noble_contracts::companion::Admission,
) -> Result<(), noble_contracts::companion::Refusal> {
    match admission.evidence {
        Some(_) => Ok(()),
        None => Err(*admission.refusals.first().expect("explicit refusal")),
    }
}

struct Harness {
    core: noble_contracts::companion::Core,
    checker: checker::LeanConsumer,
    controls: Vec<crate::workflow::encoding::Json>,
    increment: noble_contracts::Prepared,
    accepted: noble_contracts::companion::Admission,
    evidence: noble_contracts::companion::EvidenceId,
    observed: noble_contracts::companion::Subject,
    bound: noble_contracts::companion::Subject,
    baseline: crate::workflow::encoding::Json,
}

#[test]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; exercise orchestrates the fixed control set and writes its receipt; checked verifies genuine independent admission and each control helper asserts its own observed Core transition."
)]
fn exercise() {
    let Some(destination) = std::env::var_os("NOBLE_MC2_CONTROL_RECEIPT") else {
        eprintln!("MC2 independent-Lean controls not requested; no receipt produced");
        return;
    };
    let binary = std::env::var_os("NOBLE_MC2_CHECKER_BINARY")
        .expect("real noble checker binary is required");
    let destination = std::path::PathBuf::from(destination);
    let directory = destination
        .parent()
        .expect("receipt directory")
        .join("lean-controls");
    let mut checker = checker::LeanConsumer {
        binary: binary.into(),
        directory,
        source: vec![],
        sequence: 0,
        reports: vec![],
        last_report: None,
    };
    let mut core = noble_contracts::companion::Core::new(LIMITS);
    let (increment, accepted, baseline) =
        checker::checked(&mut core, &mut checker, INCREMENT, PROOF);
    let evidence = accepted.evidence.expect("increment evidence");
    let observed = retained_subject(&core, &accepted, &increment);
    let bound = core.bind(evidence, &observed).expect("bound baseline");
    let mut state = Harness {
        core,
        checker,
        controls: Vec::new(),
        increment,
        accepted,
        evidence,
        observed,
        bound,
        baseline,
    };
    adversarial::forgery(&mut state);
    binding::exercise(&mut state);
    derivation::family(&mut state);
    composition::exercise(&mut state);
    context::artifact(&mut state);
    context::renewal(&mut state);
    eligibility::exercise(&mut state);
    eligibility::refutation(&mut state);
    let receipt = crate::workflow::encoding::object([
        (
            "schema",
            crate::workflow::encoding::string("noble-mc2-core-controls/v1"),
        ),
        (
            "controls",
            crate::workflow::encoding::Json::Array(state.controls),
        ),
        (
            "checker_runs",
            crate::workflow::encoding::Json::Array(state.checker.reports),
        ),
    ]);
    std::fs::write(&destination, receipt.encode()).expect("write real core observations");
}
