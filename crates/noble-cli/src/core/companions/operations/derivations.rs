#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; replay issues a new evidence reference only on core acceptance; unknown rules, invalid premises and exhausted budgets remain typed refusals."
)]
pub(in crate::core::companions) fn op_derive(
    driver: &mut crate::core::companions::Driver,
    args: &[&str],
) -> crate::core::companions::Attempt<crate::workflow::encoding::Json> {
    let derivation =
        attempt!(crate::core::companions::operations::derivations::derivation(driver, args));
    match driver.core.replay(&derivation) {
        Ok(evidence) => Ok(crate::core::companions::reporting::line(
            crate::core::companions::reporting::Header {
                operation: "derive",
                outcome: "minted",
            },
            std::vec::Vec::from([
                ("rule", crate::workflow::encoding::string(args[0])),
                (
                    "evidence_reference",
                    crate::workflow::encoding::Json::Number(u64::from(evidence.0)),
                ),
                (
                    "contract_reference",
                    crate::workflow::encoding::Json::Number(u64::from(derivation.contract.0)),
                ),
                (
                    "statement",
                    crate::workflow::encoding::string(crate::workflow::admission::digest_text(
                        derivation.statement,
                    )),
                ),
                ("prover_calls", crate::workflow::encoding::Json::Number(0)),
                (
                    "candidate_prepare_requests",
                    crate::workflow::encoding::Json::Number(0),
                ),
                ("guest_requests", crate::workflow::encoding::Json::Number(0)),
            ]),
        )),
        Err(refusal) => Err(crate::core::companions::Refused::new(
            crate::core::companions::refusal_outcome(&refusal),
            refusal.code(),
            std::format!("the {} derivation was refused", args[0]),
        )),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; rule names, indexes and premise integers are decoded with typed script errors, and the deterministic core validates the resulting untrusted derivation."
)]
fn derivation(
    driver: &mut crate::core::companions::Driver,
    args: &[&str],
) -> crate::core::companions::Attempt<noble_contracts::companion::Derivation> {
    let rule = attempt!(rule_of(args[0]));
    let contract_index = attempt!(args[1].parse::<u32>().map_err(|_| {
        crate::core::companions::Refused::script(std::format!(
            "'{}' is not a contract index",
            args[1]
        ))
    }));
    let subject_index = attempt!(crate::core::companions::reporting::index_of(
        args[2],
        driver.stack.len()
    ));
    let companion = attempt!(driver.certified_at(subject_index));
    let subject_handle = companion.subject;
    let subject = crate::core::companions::ProgramSlot {
        handle: subject_handle,
        ty: attempt!(driver.program_type(subject_handle).ok_or_else(|| {
            crate::core::companions::Refused::script(std::string::String::from(
                "subject is not retained",
            ))
        })),
    };
    let observed = attempt!(driver.observe_subject(&subject));
    let record = attempt!(driver.record_for(contract_index));
    let statement = record.statement;
    let contract = record.contract;
    let mut premises = std::vec::Vec::with_capacity(args.len().saturating_sub(3));
    let mut at = 3;
    while at < args.len() {
        let token = args[at];
        let id = attempt!(token.parse::<u32>().map_err(|_| {
            crate::core::companions::Refused::script(std::format!(
                "'{token}' is not an evidence id"
            ))
        }));
        premises.push(noble_contracts::companion::EvidenceId(id));
        at = at.saturating_add(1);
    }
    Ok(noble_contracts::companion::Derivation {
        rule,
        contract,
        statement,
        subject: observed,
        premises,
    })
}
/// The top Program slot of a reply, with the type the run just committed.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; missing stack entries, non-Program types and absent live handles return typed malformed-worker refusals instead of assertions on external observations."
)]
pub(in crate::core::companions) fn top_program(
    driver: &crate::core::companions::Driver,
    report: &crate::core::report::Value,
) -> crate::core::companions::Attempt<crate::core::companions::ProgramSlot> {
    let entries = attempt!(crate::core::companions::reporting::values::slots(report));
    let (kind, handle) = attempt!(entries.last().ok_or_else(|| {
        crate::core::companions::Refused::new(
            "malformed",
            "engine-report",
            std::string::String::from("reply has an empty stack"),
        )
    }));
    if kind != "Program" {
        return Err(crate::core::companions::Refused::new(
            "malformed",
            "stack-type",
            std::format!("top slot is a {kind}, not a program"),
        ));
    }
    let ty = attempt!(driver.stack.last().cloned().ok_or_else(|| {
        crate::core::companions::Refused::new(
            "malformed",
            "stack-type",
            std::string::String::from("no committed type for the top slot"),
        )
    }));
    Ok(crate::core::companions::ProgramSlot {
        handle: attempt!(handle.ok_or_else(|| {
            crate::core::companions::Refused::script(std::string::String::from(
                "top program slot has no handle",
            ))
        })),
        ty,
    })
}

fn rule_of(name: &str) -> crate::core::companions::Attempt<noble_contracts::companion::RuleId> {
    Ok(match name {
        "admit-lean" => noble_contracts::companion::RuleId::AdmitLeanV1,
        "compose" => noble_contracts::companion::RuleId::ComposeV1,
        "instantiate" => noble_contracts::companion::RuleId::InstantiateV1,
        "guard" => noble_contracts::companion::RuleId::GuardV1,
        "project" => noble_contracts::companion::RuleId::ProjectV1,
        "invoke" => noble_contracts::companion::RuleId::InvokeV1,
        other => {
            return Err(crate::core::companions::Refused::new(
                "refused",
                "unknown-rule",
                std::format!("'{other}' is not a v1 rule"),
            ))
        }
    })
}
