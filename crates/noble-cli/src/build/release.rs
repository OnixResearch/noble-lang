#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; record_admission converts the core's closed admission decision into an outcome, refusals and release decision. A refused or inconclusive admission is a reporting branch, not an assertion."
)]
pub(super) fn record(
    report: &mut super::report::Document,
    core: &noble_contracts::companion::Core,
    admission: noble_contracts::companion::Admission,
    submission: &noble_kernel::execution::Submission,
) {
    let refusals: std::vec::Vec<&'static str> = admission
        .refusals
        .iter()
        .map(|refusal| refusal.code())
        .collect();
    let outcome = admission.outcome;
    report.outcome = outcome_of(outcome);
    let evidence = admission.evidence;
    let contract = admission.contract;
    if let noble_contracts::companion::Outcome::Proved = outcome {
        let Some(evidence) = evidence else {
            report.release = super::report::ReleaseDecision {
                allowed: false,
                reason: "admission reported proved without an accepted evidence entry".into(),
                refusals,
                core_subject: None,
                core_statement: None,
                core_contract: contract.map(|contract| contract.0),
                core_evidence: None,
            };
            return;
        };
        let release = core
            .bind_artifact(evidence, submission)
            .and_then(|_| core.release(evidence));
        record_bound(report, &admission, release, refusals);
        return;
    }
    report.release = super::report::ReleaseDecision {
        allowed: false,
        reason: std::format!(
            "release requires an applicable accepted proved claim; the core reported `{}`",
            outcome.code()
        ),
        refusals,
        core_subject: None,
        core_statement: None,
        core_contract: contract.map(|contract| contract.0),
        core_evidence: evidence.map(|evidence| evidence.0),
    };
}

pub(super) fn allowed(reason: &str) -> super::report::ReleaseDecision {
    super::report::ReleaseDecision {
        allowed: true,
        reason: reason.into(),
        refusals: std::vec::Vec::new(),
        core_subject: None,
        core_statement: None,
        core_contract: None,
        core_evidence: None,
    }
}

const fn outcome_of(
    outcome: noble_contracts::companion::Outcome,
) -> crate::workflow::output::Outcome {
    match outcome {
        noble_contracts::companion::Outcome::Proved => crate::workflow::output::Outcome::Proved,
        noble_contracts::companion::Outcome::Disproved => {
            crate::workflow::output::Outcome::Disproved
        }
        noble_contracts::companion::Outcome::Unknown => crate::workflow::output::Outcome::Unknown,
        noble_contracts::companion::Outcome::Timeout => crate::workflow::output::Outcome::Timeout,
        noble_contracts::companion::Outcome::Unsupported => {
            crate::workflow::output::Outcome::Unsupported
        }
        noble_contracts::companion::Outcome::Error => crate::workflow::output::Outcome::Error,
        noble_contracts::companion::Outcome::NotRun => crate::workflow::output::Outcome::NotRun,
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; record_bound formats owned release reasons, appends refusal diagnostics to growable vectors and replaces the report's owned release record. Runtime formatting, vector mutation and destruction are not const on the pinned compiler."
)]
fn record_bound(
    report: &mut super::report::Document,
    admission: &noble_contracts::companion::Admission,
    release: Result<noble_contracts::companion::Release, noble_contracts::companion::Refusal>,
    refusals: std::vec::Vec<&'static str>,
) {
    match release {
        Ok(release) => {
            report.release = super::report::ReleaseDecision {
                    allowed: true,
                    reason: std::format!(
                        "independently checked evidence binds the compiled subject, claim, context and policy revision {}",
                        super::report::POLICY_REVISION
                    ),
                    refusals,
                    core_subject: Some(release.subject.0),
                    core_statement: Some(release.statement),
                    core_contract: Some(release.contract.0),
                    core_evidence: Some(release.evidence.0),
                };
        }
        Err(refusal) => {
            let mut refused = refusals;
            refused.push(refusal.code());
            report.outcome = crate::workflow::output::Outcome::Unknown;
            report.diagnostics.push(super::artifact_diagnostic(
                refusal.code(),
                "the compiled artifact does not satisfy the accepted evidence binding".into(),
            ));
            report.release = super::report::ReleaseDecision {
                    allowed: false,
                    reason: std::format!(
                        "admitted evidence is not applicable to the exact subject, claim or current policy revision: {}",
                        refusal.code()
                    ),
                    refusals: refused,
                    core_subject: None,
                    core_statement: report.statement_digest,
                    core_contract: admission.contract.map(|contract| contract.0),
                    core_evidence: admission.evidence.map(|evidence| evidence.0),
                };
        }
    }
}
