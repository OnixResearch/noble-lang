//! Law-versus-candidate comparison for the independent release consumer.
//!
//! Every comparison the frozen law states is re-derived here from the
//! consumer's own regeneration of the candidate contract: the subject
//! interface, the formal obligation (`requires`/`ensures`), the declaration
//! name, the recorded assumptions and the normal-return scope. Each check
//! records what was observed and what the law expected, and a mismatch refuses
//! with the check that refused.

use super::terms;

/// One recorded comparison observation.
pub(crate) struct Check {
    pub(crate) name: &'static str,
    pub(crate) status: &'static str,
    pub(crate) observed: std::string::String,
    pub(crate) expected: std::string::String,
}

impl Check {
    fn matched(
        name: &'static str,
        observed: std::string::String,
        expected: std::string::String,
    ) -> Self {
        Self {
            name,
            status: "matched",
            observed,
            expected,
        }
    }

    fn mismatched(
        name: &'static str,
        observed: std::string::String,
        expected: std::string::String,
    ) -> Self {
        Self {
            name,
            status: "mismatched",
            observed,
            expected,
        }
    }
}

/// The candidate contract the consumer prepared and compared itself.
pub(crate) struct Candidate {
    pub(crate) prepared: noble_contracts::Prepared,
    /// The exact generated Lean proposition the consumer regenerated.
    pub(crate) statement: std::string::String,
    pub(crate) statement_sha256: std::string::String,
}

/// The one precondition and the four assumptions the frozen law may name.
const PRECONDITION_WELL_TYPED: &str = "well-typed-I64-input";
const ASSUMPTION_WELL_TYPED: &str = "well-typed-I64-input";
const ASSUMPTION_WRAPPING: &str = "wrapping-I64-semantics";
const ASSUMPTION_NORMAL_RETURN: &str = "normal-return-only";
const ASSUMPTION_NO_QUOTA: &str = "quota-and-host-availability-not-proved";

/// The MC1 claim shape: a normal-return partial-correctness claim over the
/// generated program, its exact input/output types and the two `Holds`.
const CLAIM_SHAPE: &str = "def claim : Prop := exportedClaim program inputTypes outputTypes paramTypes\n  (Holds precondition) (Holds postcondition)";

/// Prepare the submitted contract and compare it with the frozen law.
///
/// # Errors
/// `owner-law-contract-invalid` when the contract is not accepted ordinary
/// typing, and one law mismatch code per compared field otherwise.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the compared law, the consumer's independently regenerated preparation and the accumulating check record are borrowed exactly as read; copying them would duplicate owner-law and typed-IR state"
)]
pub(crate) fn compare(
    law: &super::law::Law,
    contract_bytes: &[u8],
    checks: &mut std::vec::Vec<Check>,
) -> Result<Candidate, super::Refusal> {
    let prepared = match noble_contracts::prepare(contract_bytes, crate::core::SOURCE_LIMITS) {
        Ok(prepared) => prepared,
        Err(error) => {
            return Err(super::Refusal::rejected(
                "owner-law-contract-invalid",
                "contract-preparation",
                std::format!(
                    "submitted contract was not prepared under ordinary typing: {:?}: {}",
                    error.kind,
                    error.message
                ),
            ))
        }
    };

    interface(law, &prepared, checks)?;
    declarations(law, &prepared, checks)?;
    obligation(law, &prepared, checks)?;
    preconditions(law, checks)?;
    let statement = noble_contracts::export_lean(&prepared);
    let statement_sha256 = crate::workflow::intrinsic::sha256(statement.as_bytes());
    assumptions(law, &prepared, &statement, checks)?;
    scope(law, &statement, checks)?;
    resource_free(&prepared, checks)?;

    Ok(Candidate {
        prepared,
        statement,
        statement_sha256,
    })
}

/// Split the law's `I64 -- I64 ! {}` subject interface into its ordered type
/// spellings and its effect bound.
fn interface_text(
    text: &str,
) -> Result<(std::vec::Vec<&str>, std::vec::Vec<&str>, &str), std::string::String> {
    let (stacks, effects) = text.split_once('!').ok_or_else(|| {
        std::string::String::from("subject interface has no `!` effect separator")
    })?;
    if stacks.contains('!') {
        return Err(std::string::String::from(
            "subject interface has more than one `!` separator",
        ));
    }
    let (input, output) = stacks.split_once("--").ok_or_else(|| {
        std::string::String::from("subject interface has no `--` stack separator")
    })?;
    if output.contains("--") {
        return Err(std::string::String::from(
            "subject interface has more than one `--` separator",
        ));
    }
    Ok((
        input.split_whitespace().collect(),
        output.split_whitespace().collect(),
        effects.trim(),
    ))
}

fn ty_spelling(ty: &noble_kernel::types::Ty) -> Option<&'static str> {
    match ty {
        noble_kernel::types::Ty::I64 => Some("I64"),
        noble_kernel::types::Ty::Bool => Some("Bool"),
        noble_kernel::types::Ty::Unit => Some("Unit"),
        noble_kernel::types::Ty::Text => Some("Text"),
        _ => None,
    }
}

fn stack_spellings(
    bindings: &[noble_contracts::NamedType],
) -> Result<std::vec::Vec<&'static str>, std::string::String> {
    bindings
        .iter()
        .map(|binding| {
            ty_spelling(&binding.ty).ok_or_else(|| {
                std::format!(
                    "binding `{}` has a type outside this consumer's closed interface grammar",
                    binding.name
                )
            })
        })
        .collect()
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the compared law, the consumer's independently regenerated preparation and the accumulating check record are borrowed exactly as read; copying them would duplicate owner-law and typed-IR state"
)]
fn interface(
    law: &super::law::Law,
    prepared: &noble_contracts::Prepared,
    checks: &mut std::vec::Vec<Check>,
) -> Result<(), super::Refusal> {
    let (expected_input, expected_output, effects) = interface_text(&law.subject_interface)
        .map_err(|detail| {
            super::Refusal::rejected("owner-law-interface-mismatch", "subject-interface", detail)
        })?;
    let observed_input = stack_spellings(prepared.inputs()).map_err(|detail| {
        super::Refusal::rejected("owner-law-interface-mismatch", "subject-interface", detail)
    })?;
    let observed_output = stack_spellings(prepared.outputs()).map_err(|detail| {
        super::Refusal::rejected("owner-law-interface-mismatch", "subject-interface", detail)
    })?;
    let matched = expected_input == observed_input
        && expected_output == observed_output
        && effects == "{}"
        && prepared.params().is_empty();
    let observed = std::format!(
        "{} -- {} ! {{{}}} ({} ghost parameters)",
        observed_input.join(" "),
        observed_output.join(" "),
        "",
        prepared.params().len()
    );
    if !matched {
        checks.push(Check::mismatched(
            "subject-interface",
            observed.clone(),
            law.subject_interface.clone(),
        ));
        return Err(super::Refusal::rejected(
            "owner-law-interface-mismatch",
            "subject-interface",
            std::format!(
                "submitted subject interface `{observed}` differs from the law's `{}`",
                law.subject_interface
            ),
        ));
    }
    checks.push(Check::matched(
        "subject-interface",
        observed,
        law.subject_interface.clone(),
    ));
    Ok(())
}

fn normalized(text: &str) -> std::string::String {
    let mut out = std::string::String::with_capacity(text.len());
    for character in text.chars() {
        if character != '_' {
            for lower in character.to_lowercase() {
                out.push(lower);
            }
        }
    }
    out
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the compared law, the consumer's independently regenerated preparation and the accumulating check record are borrowed exactly as read; copying them would duplicate owner-law and typed-IR state"
)]
fn declarations(
    law: &super::law::Law,
    prepared: &noble_contracts::Prepared,
    checks: &mut std::vec::Vec<Check>,
) -> Result<(), super::Refusal> {
    let expected = match law.declaration.rsplit_once('.') {
        Some((_, name)) if !name.is_empty() => name,
        _ => {
            return Err(super::Refusal::rejected(
                "owner-law-record-invalid",
                "declaration",
                std::format!(
                    "law declaration `{}` is not a dotted `<owner-scope>.<name>` term",
                    law.declaration
                ),
            ))
        }
    };
    let observed = prepared.name();
    let matched = normalized(expected) == normalized(observed);
    let expected_text = std::format!(
        "{} (law `{}`, matched case- and underscore-insensitively)",
        expected,
        law.declaration
    );
    if !matched {
        checks.push(Check::mismatched(
            "declaration",
            observed.into(),
            expected_text,
        ));
        return Err(super::Refusal::rejected(
            "owner-law-declaration-mismatch",
            "declaration",
            std::format!("submitted contract declares `{observed}`, not the law's `{expected}`"),
        ));
    }
    checks.push(Check::matched(
        "declaration",
        observed.into(),
        expected_text,
    ));
    Ok(())
}

fn binding_text(bindings: &[noble_contracts::NamedType]) -> std::string::String {
    let mut out = std::string::String::new();
    for binding in bindings {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push('(');
        out.push_str(&binding.name);
        if let Some(spelling) = ty_spelling(&binding.ty) {
            out.push(' ');
            out.push_str(spelling);
        }
        out.push(')');
    }
    out
}

fn law_binding_text(
    bindings: &[(std::string::String, std::string::String)],
) -> std::string::String {
    let mut out = std::string::String::new();
    for (name, ty) in bindings {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push('(');
        out.push_str(name);
        out.push(' ');
        out.push_str(ty);
        out.push(')');
    }
    out
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the compared law, the consumer's independently regenerated preparation and the accumulating check record are borrowed exactly as read; copying them would duplicate owner-law and typed-IR state"
)]
fn obligation(
    law: &super::law::Law,
    prepared: &noble_contracts::Prepared,
    checks: &mut std::vec::Vec<Check>,
) -> Result<(), super::Refusal> {
    for (name, expected, observed, code) in [
        (
            "contract-inputs",
            law_binding_text(&law.obligation.input),
            binding_text(prepared.inputs()),
            "owner-law-inputs-mismatch",
        ),
        (
            "contract-outputs",
            law_binding_text(&law.obligation.output),
            binding_text(prepared.outputs()),
            "owner-law-outputs-mismatch",
        ),
    ] {
        let matched = expected == observed;
        if !matched {
            checks.push(Check::mismatched(name, observed.clone(), expected.clone()));
            return Err(super::Refusal::rejected(
                code,
                name,
                std::format!("submitted {name} `{observed}` differ from the law's `{expected}`"),
            ));
        }
        checks.push(Check::matched(name, observed, expected));
    }

    let input_names: std::vec::Vec<std::string::String> = law
        .obligation
        .input
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    let output_names: std::vec::Vec<std::string::String> = law
        .obligation
        .output
        .iter()
        .map(|(name, _)| name.clone())
        .collect();
    let names = terms::Names::new(&input_names, &output_names);

    let requires = terms::parse(&law.obligation.requires, &names).map_err(|detail| {
        super::Refusal::rejected("owner-law-obligation-unsupported", "requires", detail)
    })?;
    terms::compare(&requires, prepared, prepared.requires()).map_err(|detail| {
        super::Refusal::rejected(
            "owner-law-requires-mismatch",
            "requires",
            std::format!(
                "submitted precondition differs from the law's `{}`: {detail}",
                law.obligation.requires
            ),
        )
    })?;
    let observed_requires = terms::render(prepared, prepared.requires()).map_err(|detail| {
        super::Refusal::rejected("owner-law-obligation-unsupported", "requires", detail)
    })?;
    checks.push(Check::matched(
        "requires",
        observed_requires,
        law.obligation.requires.clone(),
    ));

    let ensures = terms::parse(&law.obligation.ensures, &names).map_err(|detail| {
        super::Refusal::rejected("owner-law-obligation-unsupported", "ensures", detail)
    })?;
    terms::compare(&ensures, prepared, prepared.ensures()).map_err(|detail| {
        super::Refusal::rejected(
            "owner-law-ensures-mismatch",
            "ensures",
            std::format!(
                "submitted postcondition differs from the law's `{}`: {detail}",
                law.obligation.ensures
            ),
        )
    })?;
    let observed_ensures = terms::render(prepared, prepared.ensures()).map_err(|detail| {
        super::Refusal::rejected("owner-law-obligation-unsupported", "ensures", detail)
    })?;
    checks.push(Check::matched(
        "ensures",
        observed_ensures,
        law.obligation.ensures.clone(),
    ));
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the compared law, the consumer's independently regenerated preparation and the accumulating check record are borrowed exactly as read; copying them would duplicate owner-law and typed-IR state"
)]
fn preconditions(
    law: &super::law::Law,
    checks: &mut std::vec::Vec<Check>,
) -> Result<(), super::Refusal> {
    let observed = law.preconditions.join(",");
    if law.preconditions.len() != 1 || law.preconditions[0] != PRECONDITION_WELL_TYPED {
        checks.push(Check::mismatched(
            "preconditions",
            observed,
            PRECONDITION_WELL_TYPED.into(),
        ));
        return Err(super::Refusal::rejected(
            "owner-law-precondition-unsupported",
            "preconditions",
            std::format!(
                "law preconditions `{}` are outside this consumer's closed table (`{PRECONDITION_WELL_TYPED}`)",
                law.preconditions.join(",")
            ),
        ));
    }
    checks.push(Check::matched(
        "preconditions",
        observed,
        PRECONDITION_WELL_TYPED.into(),
    ));
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the compared law, the consumer's independently regenerated preparation and the accumulating check record are borrowed exactly as read; copying them would duplicate owner-law and typed-IR state"
)]
fn assumptions(
    law: &super::law::Law,
    prepared: &noble_contracts::Prepared,
    statement: &str,
    checks: &mut std::vec::Vec<Check>,
) -> Result<(), super::Refusal> {
    if law.assumptions.is_empty() {
        return Err(super::Refusal::rejected(
            "owner-law-record-invalid",
            "assumptions",
            "owner law records no assumptions".into(),
        ));
    }
    for assumption in &law.assumptions {
        let observation = match assumption.as_str() {
            ASSUMPTION_WELL_TYPED => well_typed(prepared),
            ASSUMPTION_WRAPPING => wrapping(),
            ASSUMPTION_NORMAL_RETURN => normal_return(statement),
            ASSUMPTION_NO_QUOTA => resource_free_observation(prepared),
            _ => {
                return Err(super::Refusal::rejected(
                    "owner-law-assumption-unsupported",
                    "assumptions",
                    std::format!(
                        "law assumption `{assumption}` is outside this consumer's closed table"
                    ),
                ))
            }
        }?;
        checks.push(Check::matched(
            "assumption",
            std::format!("{assumption}: {observation}"),
            std::format!("{assumption}: discharged by the candidate's own generated proposition"),
        ));
    }
    Ok(())
}

fn well_typed(prepared: &noble_contracts::Prepared) -> Result<std::string::String, super::Refusal> {
    let input = stack_spellings(prepared.inputs()).map_err(|detail| {
        super::Refusal::rejected("owner-law-assumption-mismatch", "assumption", detail)
    })?;
    let output = stack_spellings(prepared.outputs()).map_err(|detail| {
        super::Refusal::rejected("owner-law-assumption-mismatch", "assumption", detail)
    })?;
    if input != ["I64"] || output != ["I64"] {
        return Err(super::Refusal::rejected(
            "owner-law-assumption-mismatch",
            "assumption",
            std::format!(
                "the well-typed-I64-input assumption needs I64 -- I64, observed {} -- {}",
                input.join(" "),
                output.join(" ")
            ),
        ));
    }
    Ok(std::string::String::from(
        "ordinary typing accepted exactly one I64 input and one I64 output; the law's requires is `true`",
    ))
}

fn wrapping() -> Result<std::string::String, super::Refusal> {
    Ok(std::string::String::from(
        "the pinned MC1 model's .add/.sub/.mul on I64 are BitVec 64 operations, so arithmetic wraps; the proof is checked against that same selected library",
    ))
}

fn normal_return(statement: &str) -> Result<std::string::String, super::Refusal> {
    if !statement.contains(CLAIM_SHAPE) {
        return Err(super::Refusal::rejected(
            "owner-law-scope-mismatch",
            "assumption",
            "the generated proposition is not the MC1 normal-return Holds claim shape".into(),
        ));
    }
    Ok(std::string::String::from(
        "exportedClaim ... (Holds precondition) (Holds postcondition) is partial correctness for each normal return",
    ))
}

fn resource_free_observation(
    prepared: &noble_contracts::Prepared,
) -> Result<std::string::String, super::Refusal> {
    if !prepared.request().expected.allowed_effects.is_empty()
        || !prepared.checked().interface.effects.is_empty()
        || !prepared.checked().live_sites.is_empty()
        || !prepared.params().is_empty()
    {
        return Err(super::Refusal::rejected(
            "owner-law-assumption-mismatch",
            "assumption",
            "the candidate admits effects, live dispatch sites or ghost parameters, so the quota/host non-claim is contradicted".into(),
        ));
    }
    Ok(std::string::String::from(
        "zero allowed effects, zero derived effects, zero live dispatch sites and no ghost parameters",
    ))
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the compared law, the consumer's independently regenerated preparation and the accumulating check record are borrowed exactly as read; copying them would duplicate owner-law and typed-IR state"
)]
fn scope(
    law: &super::law::Law,
    statement: &str,
    checks: &mut std::vec::Vec<Check>,
) -> Result<(), super::Refusal> {
    if !law.scope.starts_with("normal-return-only") {
        checks.push(Check::mismatched(
            "scope",
            law.scope.clone(),
            "normal-return-only".into(),
        ));
        return Err(super::Refusal::rejected(
            "owner-law-scope-mismatch",
            "scope",
            std::format!(
                "law scope `{}` is not the normal-return-only partial-correctness scope this consumer supports",
                law.scope
            ),
        ));
    }
    if !statement.contains(CLAIM_SHAPE) {
        checks.push(Check::mismatched(
            "scope",
            "generated proposition is not the MC1 normal-return Holds claim".into(),
            law.scope.clone(),
        ));
        return Err(super::Refusal::rejected(
            "owner-law-scope-mismatch",
            "scope",
            "the generated proposition is not the MC1 normal-return Holds claim".into(),
        ));
    }
    checks.push(Check::matched(
        "scope",
        "normal-return-only: each normal return is specified; termination, totality, quota exhaustion and host availability are not claimed"
            .into(),
        law.scope.clone(),
    ));
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers; the compared law, the consumer's independently regenerated preparation and the accumulating check record are borrowed exactly as read; copying them would duplicate owner-law and typed-IR state"
)]
fn resource_free(
    prepared: &noble_contracts::Prepared,
    checks: &mut std::vec::Vec<Check>,
) -> Result<(), super::Refusal> {
    let observation = resource_free_observation(prepared)?;
    checks.push(Check::matched(
        "resource-free",
        observation,
        "MC2-v1 resource-free Core-Bootstrap program".into(),
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::compare;

    const LAW_V1: &[u8] =
        include_bytes!("../../../../../verification/owner-law/laws/owner-law-v1.json");
    const INCREMENT: &[u8] =
        include_bytes!("../../../../../verification/mc1/increment.noble-contract");
    const IDENTITY: &[u8] = include_bytes!(
        "../../../../../verification/owner-law/fixtures/identity-statement.contract"
    );
    const WEAKENED: &[u8] = include_bytes!(
        "../../../../../verification/owner-law/fixtures/weakened-precondition.contract"
    );
    const WRONG_PROGRAM: &[u8] = include_bytes!(
        "../../../../../verification/owner-law/fixtures/example-preserving-wrong.contract"
    );
    const ADD_TWO: &[u8] =
        include_bytes!("../../../../../verification/owner-law/fixtures/add-two.contract");

    fn law() -> super::super::law::Law {
        let sha256 = crate::workflow::intrinsic::sha256(LAW_V1);
        super::super::law::decode(LAW_V1, sha256.clone(), sha256).expect("the frozen law decodes")
    }

    fn checked(contract: &[u8]) -> Result<(), super::super::Refusal> {
        let mut checks = std::vec::Vec::new();
        compare(&law(), contract, &mut checks).map(|_| ())
    }

    #[test]
    fn owner_law_comparison_checks_and_refusals() {
        {
            assert!(checked(INCREMENT).is_ok());
        }
        {
            let refusal = checked(IDENTITY).expect_err("identity statement refuses");
            assert_eq!(refusal.code, "owner-law-ensures-mismatch");
        }
        {
            let refusal = checked(WEAKENED).expect_err("weakened precondition refuses");
            assert_eq!(refusal.code, "owner-law-requires-mismatch");
        }
        {
            let refusal = checked(ADD_TWO).expect_err("add_two refuses under v1");
            assert_eq!(refusal.code, "owner-law-declaration-mismatch");
        }
        {
            // The law comparison is not the program check: the wrong body keeps the
            // law's interface and obligation, so it reaches the exact check, which
            // is the only route that can bind the resolved program identity.
            assert!(checked(WRONG_PROGRAM).is_ok());
        }
    }
}
