//! Finite specification-quality derivation over a checked native contract (issue #65).
//!
//! Types and predicates come only from `Prepared`. The manifest restricts evaluation to its
//! declared input/parameter domain and permitted output stacks; these results do not establish
//! universal realizability or determinacy of the unrestricted native contract. Submitted flags
//! are compared with the derivation, never accepted as evidence.

use super::super::report::Value;
use crate::workflow::encoding::Json;

const DOMAIN_LIMIT: usize = 64;
const OUTPUT_LIMIT: usize = 16;

/// One exactly typed scalar stack entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scalar {
    I64(i64),
    Bool(bool),
}

struct Assignment {
    params: std::vec::Vec<Scalar>,
    inputs: std::vec::Vec<Scalar>,
}

struct Samples {
    domain: std::vec::Vec<Assignment>,
    permitted: std::vec::Vec<std::vec::Vec<Scalar>>,
}

pub(super) struct Derived {
    pub(super) inhabited: bool,
    pub(super) realizable: bool,
    pub(super) deterministic: bool,
    pub(super) vacuous: bool,
    pub(super) assignments: usize,
    pub(super) permitted: usize,
    refutation: Option<Refutation>,
}

/// Exhaustive finite evidence, with indices into the original manifest arrays.
enum Refutation {
    NoOutput {
        assignment: usize,
    },
    Ambiguous {
        assignment: usize,
        first: usize,
        second: usize,
    },
}

pub(super) type Refused = (&'static str, std::string::String);

fn refused(class: &'static str, message: impl Into<std::string::String>) -> Refused {
    (class, message.into())
}

fn scalar(value: &Value) -> Result<Scalar, Refused> {
    let Value::Object(fields) = value else {
        return Err(refused("invalid-manifest", "scalar must be an object"));
    };
    if fields.len() != 1 {
        return Err(refused(
            "invalid-manifest",
            "scalar must carry exactly one kind tag",
        ));
    }
    match fields.get("i64") {
        Some(Value::Number(value)) => return Ok(Scalar::I64(*value)),
        None => (),
        Some(_) => return Err(refused("invalid-manifest", "i64 scalar must be a number")),
    }
    match fields.get("bool") {
        Some(Value::Bool(value)) => Ok(Scalar::Bool(*value)),
        None => Err(refused(
            "unsupported-relation",
            "scalar carries an unsupported kind tag",
        )),
        Some(_) => Err(refused("invalid-manifest", "bool scalar must be a boolean")),
    }
}

fn stack(
    value: &Value,
    expected: &[noble_contracts::NamedType],
    what: &str,
) -> Result<std::vec::Vec<Scalar>, Refused> {
    let items = value
        .items()
        .ok_or_else(|| refused("invalid-manifest", std::format!("{what} must be an array")))?;
    if items.len() != expected.len() {
        return Err(refused(
            "unsupported-relation",
            std::format!("{what} length does not match the native ordered stack"),
        ));
    }
    let mut collected = std::vec::Vec::with_capacity(items.len());
    for (item, declared) in items.iter().zip(expected) {
        let entry = scalar(item)?;
        let agrees = matches!(
            (entry, &declared.ty),
            (Scalar::I64(_), noble_kernel::types::Ty::I64)
                | (Scalar::Bool(_), noble_kernel::types::Ty::Bool)
        );
        if !agrees {
            return Err(refused(
                "unsupported-relation",
                std::format!("{what} entry does not have its native exact kind"),
            ));
        }
        collected.push(entry);
    }
    Ok(collected)
}

fn profile(contract: &noble_contracts::Prepared) -> Result<(), Refused> {
    if !contract.checked().interface.effects.is_empty()
        || contract
            .inputs()
            .iter()
            .chain(contract.outputs())
            .chain(contract.params())
            .any(|entry| {
                !matches!(
                    entry.ty,
                    noble_kernel::types::Ty::I64 | noble_kernel::types::Ty::Bool
                )
            })
    {
        return Err(refused(
            "unsupported-relation",
            "native contract is outside the pure I64/Bool interface profile",
        ));
    }
    super::predicate::check(contract)
}

fn samples(
    contract: &noble_contracts::Prepared,
    domain_value: &Value,
    permitted_value: &Value,
) -> Result<Samples, Refused> {
    let domain_items = domain_value
        .items()
        .ok_or_else(|| refused("invalid-manifest", "domain must be an array"))?;
    if domain_items.is_empty() || domain_items.len() > DOMAIN_LIMIT {
        return Err(refused(
            "unsupported-relation",
            std::format!("declared domain must hold between 1 and {DOMAIN_LIMIT} assignments"),
        ));
    }
    let mut domain = std::vec::Vec::with_capacity(domain_items.len());
    for item in domain_items {
        let Value::Object(entry) = item else {
            return Err(refused(
                "invalid-manifest",
                "domain assignment must be an object",
            ));
        };
        if entry
            .keys()
            .any(|key| !matches!(key.as_str(), "inputs" | "params"))
        {
            return Err(refused("invalid-manifest", "unknown assignment field"));
        }
        domain.push(Assignment {
            params: stack(
                entry
                    .get("params")
                    .ok_or_else(|| refused("invalid-manifest", "assignment params are missing"))?,
                contract.params(),
                "assignment params",
            )?,
            inputs: stack(
                entry
                    .get("inputs")
                    .ok_or_else(|| refused("invalid-manifest", "assignment inputs are missing"))?,
                contract.inputs(),
                "assignment inputs",
            )?,
        });
    }
    let permitted_items = permitted_value
        .items()
        .ok_or_else(|| refused("invalid-manifest", "permitted outputs must be an array"))?;
    if permitted_items.is_empty() || permitted_items.len() > OUTPUT_LIMIT {
        return Err(refused(
            "unsupported-relation",
            std::format!("permitted outputs must hold between 1 and {OUTPUT_LIMIT} stacks"),
        ));
    }
    let mut permitted = std::vec::Vec::with_capacity(permitted_items.len());
    for item in permitted_items {
        permitted.push(stack(item, contract.outputs(), "permitted output")?);
    }
    Ok(Samples { domain, permitted })
}

/// Evaluate the checked predicates exhaustively over the declared finite restriction.
pub(super) fn derive(
    contract: &noble_contracts::Prepared,
    domain: &Value,
    permitted: &Value,
) -> Result<Derived, Refused> {
    profile(contract)?;
    let retained = samples(contract, domain, permitted)?;
    let mut admitted = 0_usize;
    let mut realizable = true;
    let mut deterministic = true;
    let mut refutation = None;
    for (index, assignment) in retained.domain.iter().enumerate() {
        let pre = super::predicate::Binding {
            params: &assignment.params,
            inputs: &assignment.inputs,
            output: None,
        };
        if !super::predicate::holds(contract, contract.requires(), &pre, "precondition")? {
            continue;
        }
        admitted = admitted.saturating_add(1);
        let mut first = None;
        let mut second = None;
        for (position, candidate) in retained.permitted.iter().enumerate() {
            let post = super::predicate::Binding {
                params: &assignment.params,
                inputs: &assignment.inputs,
                output: Some(candidate),
            };
            if super::predicate::holds(contract, contract.ensures(), &post, "postcondition")? {
                match first {
                    None => first = Some(position),
                    Some(prior) => {
                        // Duplicates cannot hide a later distinct satisfying output. Continue
                        // evaluating every output even after retaining the first distinct pair.
                        if second.is_none() && retained.permitted[prior] != *candidate {
                            second = Some(position);
                        }
                    }
                }
            }
        }
        match (first, second) {
            (None, _) => {
                realizable = false;
                refutation = Some(Refutation::NoOutput { assignment: index });
            }
            (Some(first), Some(second)) => {
                deterministic = false;
                refutation = Some(Refutation::Ambiguous {
                    assignment: index,
                    first,
                    second,
                });
            }
            (Some(_), None) => (),
        }
    }
    let inhabited = admitted > 0;
    Ok(Derived {
        inhabited,
        realizable,
        deterministic,
        vacuous: !inhabited,
        assignments: retained.domain.len(),
        permitted: retained.permitted.len(),
        refutation,
    })
}

impl Derived {
    pub(super) fn json(&self) -> Json {
        use crate::workflow::encoding::{object, string};
        let refutation = match &self.refutation {
            None => Json::Null,
            Some(Refutation::NoOutput { assignment }) => Json::Array(std::vec![
                string("no-output"),
                Json::Number(u64::try_from(*assignment).unwrap_or(u64::MAX)),
            ]),
            Some(Refutation::Ambiguous {
                assignment,
                first,
                second,
            }) => Json::Array(std::vec![
                string("two-distinct-outputs"),
                Json::Number(u64::try_from(*assignment).unwrap_or(u64::MAX)),
                Json::Number(u64::try_from(*first).unwrap_or(u64::MAX)),
                Json::Number(u64::try_from(*second).unwrap_or(u64::MAX)),
            ]),
        };
        object([
            ("inhabited", Json::Bool(self.inhabited)),
            ("realizable", Json::Bool(self.realizable)),
            ("deterministic", Json::Bool(self.deterministic)),
            ("vacuous", Json::Bool(self.vacuous)),
            (
                "assignments",
                Json::Number(u64::try_from(self.assignments).unwrap_or(u64::MAX)),
            ),
            (
                "permitted",
                Json::Number(u64::try_from(self.permitted).unwrap_or(u64::MAX)),
            ),
            ("refutation", refutation),
        ])
    }
}
