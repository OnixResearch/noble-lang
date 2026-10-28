//! Derivation of specification-quality propositions from a *retained relation*, independently of the
//! submitted proof text (issue #65 bounded work).
//!
//! A submission states the relation once — ordered exact `I64`/`Bool` parameter/input/output stacks,
//! a finite declared domain of admissible assignments, a finite set of permitted output stacks, and a
//! `pre`/`post` predicate over them — and this module derives inhabitation, realizability and
//! determinacy itself. The submitted `checked`/`vacuous` flags are then only compared with what was
//! derived, so a manifest cannot attest its own obligations.
//!
//! Every refusal here fails closed. A predicate that is undefined for an assignment in the declared
//! domain is refused (`undefined-relation`) rather than read as false, an index outside the declared
//! stacks is undefined, a construct outside the bounded profile is `unsupported-relation`, and the
//! bounds below keep the enumeration finite.

use super::super::report::Value;
use crate::workflow::encoding::Json;

/// Declared domain cap: every admissible assignment is enumerated.
const DOMAIN_LIMIT: usize = 64;
/// Permitted-output cap per assignment.
const OUTPUT_LIMIT: usize = 16;
/// Term nesting cap.
const TERM_DEPTH_LIMIT: usize = 8;

/// One exactly typed scalar stack entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Scalar {
    I64(i64),
    Bool(bool),
}

/// The kind a declared stack position must hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    I64,
    Bool,
}

/// A predicate term over the declared stacks.
#[derive(Debug)]
enum Term {
    Const(Scalar),
    Param(usize),
    Input(usize),
    Output(usize),
    Eq(Box<Term>, Box<Term>),
    Lt(Box<Term>, Box<Term>),
    And(std::vec::Vec<Term>),
    Or(std::vec::Vec<Term>),
    Not(Box<Term>),
    Add(Box<Term>, Box<Term>),
}

/// One assignment and the stacks it binds.
struct Assignment {
    params: std::vec::Vec<Scalar>,
    inputs: std::vec::Vec<Scalar>,
}

/// The retained relation, validated and bounded. The declared kinds are consumed while parsing the
/// domain and the permitted stacks, so only what the derivation reads is retained.
struct Relation {
    domain: std::vec::Vec<Assignment>,
    permitted: std::vec::Vec<std::vec::Vec<Scalar>>,
    pre: Term,
    post: Term,
}

/// What the producer derived for the retained relation.
pub(super) struct Derived {
    pub(super) inhabited: bool,
    pub(super) realizable: bool,
    pub(super) deterministic: bool,
    pub(super) vacuous: bool,
    pub(super) assignments: usize,
    pub(super) permitted: usize,
    /// A checked witness for a refused proposition, never a failed search.
    refutation: Option<Refutation>,
}

/// The measured reason a proposition is refused: distinct from "no witness was found".
pub(super) enum Refutation {
    /// An admissible input has no permitted output satisfying the postcondition.
    NoOutput { assignment: usize },
    /// One admissible input has two distinct permitted outputs satisfying the postcondition.
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

fn kinds(value: &Value, class: &'static str, what: &str) -> Result<std::vec::Vec<Kind>, Refused> {
    let items = value.items().ok_or_else(|| {
        refused(
            class,
            std::format!("{what} must be an array of declared kinds"),
        )
    })?;
    let mut collected = std::vec::Vec::with_capacity(items.len());
    for item in items {
        let kind = match item.text() {
            Some("i64") => Kind::I64,
            Some("bool") => Kind::Bool,
            _ => {
                return Err(refused(
                    "unsupported-relation",
                    std::format!("{what} declares a kind outside the pure I64/Bool profile"),
                ));
            }
        };
        collected.push(kind);
    }
    Ok(collected)
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

fn stack(value: &Value, expected: &[Kind], what: &str) -> Result<std::vec::Vec<Scalar>, Refused> {
    let items = value
        .items()
        .ok_or_else(|| refused("invalid-manifest", std::format!("{what} must be an array")))?;
    if items.len() != expected.len() {
        return Err(refused(
            "unsupported-relation",
            std::format!("{what} length does not match the declared ordered stack"),
        ));
    }
    let mut collected = std::vec::Vec::with_capacity(items.len());
    for (item, kind) in items.iter().zip(expected) {
        let entry = scalar(item)?;
        let agrees = matches!(
            (entry, kind),
            (Scalar::I64(_), Kind::I64) | (Scalar::Bool(_), Kind::Bool)
        );
        if !agrees {
            return Err(refused(
                "unsupported-relation",
                std::format!("{what} entry does not have its declared exact kind"),
            ));
        }
        collected.push(entry);
    }
    Ok(collected)
}

fn term(value: &Value, depth: usize) -> Result<Term, Refused> {
    if depth >= TERM_DEPTH_LIMIT {
        return Err(refused(
            "unsupported-relation",
            "predicate nesting exceeds the bound",
        ));
    }
    let Value::Object(fields) = value else {
        return Err(refused(
            "invalid-manifest",
            "predicate term must be an object",
        ));
    };
    let op = fields
        .get("op")
        .and_then(Value::text)
        .ok_or_else(|| refused("invalid-manifest", "predicate term must name its op"))?;
    let args = fields.get("args").and_then(Value::items).unwrap_or(&[]);
    let inner = depth.saturating_add(1);
    let unary = || -> Result<&Value, Refused> {
        match args {
            [only] => Ok(only),
            _ => Err(refused(
                "invalid-manifest",
                "unary op takes exactly one argument",
            )),
        }
    };
    let binary = || -> Result<(&Value, &Value), Refused> {
        match args {
            [left, right] => Ok((left, right)),
            _ => Err(refused(
                "invalid-manifest",
                "binary op takes exactly two arguments",
            )),
        }
    };
    let index = |arg: &Value| -> Result<usize, Refused> {
        arg.index()
            .map(|index| usize::try_from(index).unwrap_or(usize::MAX))
            .ok_or_else(|| refused("invalid-manifest", "stack reference must be an index"))
    };
    match op {
        "constant" => Ok(Term::Const(scalar(unary()?)?)),
        "param" => Ok(Term::Param(index(unary()?)?)),
        "input" => Ok(Term::Input(index(unary()?)?)),
        "output" => Ok(Term::Output(index(unary()?)?)),
        "eq" => {
            let (left, right) = binary()?;
            Ok(Term::Eq(
                Box::new(term(left, inner)?),
                Box::new(term(right, inner)?),
            ))
        }
        "lt" => {
            let (left, right) = binary()?;
            Ok(Term::Lt(
                Box::new(term(left, inner)?),
                Box::new(term(right, inner)?),
            ))
        }
        "add" => {
            let (left, right) = binary()?;
            Ok(Term::Add(
                Box::new(term(left, inner)?),
                Box::new(term(right, inner)?),
            ))
        }
        "not" => Ok(Term::Not(Box::new(term(unary()?, inner)?))),
        "and" | "or" => {
            let mut nested = std::vec::Vec::with_capacity(args.len());
            for arg in args {
                nested.push(term(arg, inner)?);
            }
            if nested.is_empty() {
                return Err(refused(
                    "invalid-manifest",
                    "conjunction takes at least one argument",
                ));
            }
            Ok(if op == "and" {
                Term::And(nested)
            } else {
                Term::Or(nested)
            })
        }
        _ => Err(refused(
            "unsupported-relation",
            std::format!("predicate op {op} is outside the bounded profile"),
        )),
    }
}

/// The stacks a term reads. `None` for the output stack means "postcondition not evaluated".
struct Binding<'a> {
    params: &'a [Scalar],
    inputs: &'a [Scalar],
    output: Option<&'a [Scalar]>,
}

fn evaluate(term: &Term, at: &Binding<'_>) -> Option<Scalar> {
    let boolean = |value: Option<Scalar>| match value {
        Some(Scalar::Bool(flag)) => Some(flag),
        _ => None,
    };
    match term {
        Term::Const(value) => Some(*value),
        Term::Param(index) => at.params.get(*index).copied(),
        Term::Input(index) => at.inputs.get(*index).copied(),
        Term::Output(index) => at.output.and_then(|stack| stack.get(*index)).copied(),
        Term::Eq(left, right) => match (evaluate(left, at), evaluate(right, at)) {
            (Some(Scalar::I64(one)), Some(Scalar::I64(other))) => Some(Scalar::Bool(one == other)),
            (Some(Scalar::Bool(one)), Some(Scalar::Bool(other))) => {
                Some(Scalar::Bool(one == other))
            }
            _ => None,
        },
        Term::Lt(left, right) => match (evaluate(left, at), evaluate(right, at)) {
            (Some(Scalar::I64(one)), Some(Scalar::I64(other))) => Some(Scalar::Bool(one < other)),
            _ => None,
        },
        Term::Add(left, right) => match (evaluate(left, at), evaluate(right, at)) {
            (Some(Scalar::I64(one)), Some(Scalar::I64(other))) => {
                Some(Scalar::I64(one.wrapping_add(other)))
            }
            _ => None,
        },
        Term::Not(inner) => boolean(evaluate(inner, at)).map(|flag| Scalar::Bool(!flag)),
        Term::And(terms) => {
            let mut value = true;
            for entry in terms {
                value &= boolean(evaluate(entry, at))?;
            }
            Some(Scalar::Bool(value))
        }
        Term::Or(terms) => {
            let mut value = false;
            for entry in terms {
                value |= boolean(evaluate(entry, at))?;
            }
            Some(Scalar::Bool(value))
        }
    }
}

fn holds(term: &Term, at: &Binding<'_>, what: &str) -> Result<bool, Refused> {
    match evaluate(term, at) {
        Some(Scalar::Bool(flag)) => Ok(flag),
        _ => Err(refused(
            "undefined-relation",
            std::format!("{what} is undefined for an assignment in the declared domain"),
        )),
    }
}

fn relation(value: &Value) -> Result<Relation, Refused> {
    let Value::Object(fields) = value else {
        return Err(refused("invalid-manifest", "relation must be an object"));
    };
    for key in fields.keys() {
        if !matches!(
            key.as_str(),
            "params" | "inputs" | "outputs" | "domain" | "permitted" | "pre" | "post"
        ) {
            return Err(refused("invalid-manifest", "unknown relation field"));
        }
    }
    let params = kinds(
        fields
            .get("params")
            .ok_or_else(|| refused("invalid-manifest", "relation params are missing"))?,
        "invalid-manifest",
        "relation params",
    )?;
    let inputs = kinds(
        fields
            .get("inputs")
            .ok_or_else(|| refused("invalid-manifest", "relation inputs are missing"))?,
        "invalid-manifest",
        "relation inputs",
    )?;
    let outputs = kinds(
        fields
            .get("outputs")
            .ok_or_else(|| refused("invalid-manifest", "relation outputs are missing"))?,
        "invalid-manifest",
        "relation outputs",
    )?;
    let domain_items = fields
        .get("domain")
        .and_then(Value::items)
        .ok_or_else(|| refused("invalid-manifest", "relation domain must be an array"))?;
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
        let params_stack = stack(
            entry
                .get("params")
                .ok_or_else(|| refused("invalid-manifest", "assignment params are missing"))?,
            &params,
            "assignment params",
        )?;
        let inputs_stack = stack(
            entry
                .get("inputs")
                .ok_or_else(|| refused("invalid-manifest", "assignment inputs are missing"))?,
            &inputs,
            "assignment inputs",
        )?;
        domain.push(Assignment {
            params: params_stack,
            inputs: inputs_stack,
        });
    }
    let permitted_items = fields
        .get("permitted")
        .and_then(Value::items)
        .ok_or_else(|| {
            refused(
                "invalid-manifest",
                "relation permitted outputs must be an array",
            )
        })?;
    if permitted_items.is_empty() || permitted_items.len() > OUTPUT_LIMIT {
        return Err(refused(
            "unsupported-relation",
            std::format!("permitted outputs must hold between 1 and {OUTPUT_LIMIT} stacks"),
        ));
    }
    let mut permitted = std::vec::Vec::with_capacity(permitted_items.len());
    for item in permitted_items {
        permitted.push(stack(item, &outputs, "permitted output")?);
    }
    let pre = term(
        fields
            .get("pre")
            .ok_or_else(|| refused("invalid-manifest", "relation precondition is missing"))?,
        0,
    )?;
    let post = term(
        fields
            .get("post")
            .ok_or_else(|| refused("invalid-manifest", "relation postcondition is missing"))?,
        0,
    )?;
    Ok(Relation {
        domain,
        permitted,
        pre,
        post,
    })
}

/// Derive the three specification-quality propositions from the retained relation. A refusal here
/// means the relation cannot establish the proposition, never that a search failed.
pub(super) fn derive(value: &Value) -> Result<Derived, Refused> {
    let retained = relation(value)?;
    let mut admitted = 0_usize;
    let mut realizable = true;
    let mut deterministic = true;
    let mut refutation = None;
    for (index, assignment) in retained.domain.iter().enumerate() {
        let pre = Binding {
            params: &assignment.params,
            inputs: &assignment.inputs,
            output: None,
        };
        if !holds(&retained.pre, &pre, "precondition")? {
            continue;
        }
        admitted = admitted.saturating_add(1);
        let mut satisfying = std::vec::Vec::new();
        for (position, candidate) in retained.permitted.iter().enumerate() {
            let post = Binding {
                params: &assignment.params,
                inputs: &assignment.inputs,
                output: Some(candidate),
            };
            if holds(&retained.post, &post, "postcondition")? {
                satisfying.push(position);
            }
        }
        match satisfying.as_slice() {
            [] => {
                realizable = false;
                refutation = Some(Refutation::NoOutput { assignment: index });
            }
            [first, rest @ ..] => {
                // Every later satisfying stack must be compared, not only the second: a relation whose
                // first two permitted outputs coincide can still be nondeterministic through a third.
                let distinct = rest
                    .iter()
                    .copied()
                    .find(|position| retained.permitted[*first] != retained.permitted[*position]);
                if let Some(position) = distinct {
                    deterministic = false;
                    refutation = Some(Refutation::Ambiguous {
                        assignment: index,
                        first: *first,
                        second: position,
                    });
                }
            }
        }
    }
    let inhabited = admitted > 0;
    Ok(Derived {
        inhabited,
        // A universal proposition with an uninhabited domain is provable and vacuous.
        realizable,
        deterministic,
        vacuous: !inhabited,
        assignments: retained.domain.len(),
        permitted: retained.permitted.len(),
        refutation,
    })
}

impl Derived {
    /// No relation was submitted, so nothing is established. A specification-quality claim submitted
    /// without a retained relation therefore agrees with no derivation and is refused rather than
    /// read as unchecked agreement.
    pub(super) fn unavailable() -> Self {
        Self {
            inhabited: false,
            realizable: false,
            deterministic: false,
            vacuous: false,
            assignments: 0,
            permitted: 0,
            refutation: None,
        }
    }

    /// The reported derivation, including the measured refutation when there is one.
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
