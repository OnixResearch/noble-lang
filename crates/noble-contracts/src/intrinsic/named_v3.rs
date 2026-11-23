//! The opt-in revision-3 reusable named-call profile: exact envelope
//! reconstruction, a deterministic symbolic checker over the bounded
//! `I64`/`Bool` vocabulary, a bounded scalar-term equality, and isolated
//! dependency replay. Revisions 1 and 2 keep their own modules and meanings.
//!
//! A call never reads a callee body as a proof: it consumes the callee's
//! accepted contract claim, the caller supplies an explicit bridge and a final
//! join, and every distinct closure member is replayed by the same bounded
//! checkers before the caller is accepted.

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use super::{
    atom, head_atom, list, operation, reject, unsupported, CheckedProof, ContractGoal, Form,
    FormKind, Meter, NamedUseOrigin, PendingGoal, ProofBudgets, ProofDependency, ProofKind,
    ProofObligation, TreeBounds, Transition, MeterConfig,
};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use crate::{Diagnostic, DiagnosticKind, Limits, Span};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use alloc::{boxed::Box, string::String, vec::Vec};

/// Bound on call occurrences in one caller body.
const MAX_OCCURRENCES: usize = 16;
/// Bound on distinct callee proofs in one caller proof.
const MAX_DISTINCT_CALLEES: usize = 8;
/// Bound on distinct logical lemma proofs in one caller proof.
const MAX_LEMMA_USES: usize = 16;
/// Bound on visited nodes in one assertion or logical term.
const MAX_ASSERTION_NODES: usize = 256;
/// Bound on the symbolic stack.
const MAX_STACK: usize = 16;
/// Bound on the contract-call depth of the replayed closure.
const MAX_CLOSURE_DEPTH: u32 = 4;
/// Bound on the accumulated fact set.
const MAX_FACTS: usize = 64;
/// Bound on a caller or callee interface in either direction.
const MAX_INTERFACE: usize = 4;
/// Bound on distinct replayed closure members.
const MAX_CLOSURE: usize = 32;

/// Charge one visited assertion/term node against the shared admission budget
/// and the profile's per-assertion node bound.
macro_rules! spend {
    ($remaining:ident,$meter:ident,$depth:expr,$span:expr) => {{
        let Some(left) = (*$remaining).checked_sub(1) else {
            return Err(budget($span));
        };
        *$remaining = left;
        let (next, outcome) = $meter.visit($depth, $span);
        *$meter = next;
        outcome?;
    }};
}

fn invalid(span: Span, message: &str) -> Diagnostic {
    reject(span, message)
}

fn unsupported_profile(span: Span, message: &str) -> Diagnostic {
    unsupported(span, message)
}

fn budget(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::Exhausted,
        span,
        "revision-3 budget exhausted",
    )
}

/// A Decision 6 diagnostic: a code, then a bounded detail that never claims
/// the target proposition is false.
fn refined(span: Span, code: &str, detail: &str) -> Diagnostic {
    let mut message = String::from(code);
    message.push_str(": ");
    message.push_str(detail);
    Diagnostic::new(DiagnosticKind::Invalid, span, &message)
}

/// One Type0 sort the profile admits.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort3 {
    I64,
    Bool,
}

impl Sort3 {
    fn from_word(word: &str) -> Option<Self> {
        if word == "I64" {
            Some(Self::I64)
        } else if word == "Bool" {
            Some(Self::Bool)
        } else {
            None
        }
    }

    fn from_ty(ty: &noble_kernel::types::Ty) -> Option<Self> {
        match ty {
            noble_kernel::types::Ty::I64 => Some(Self::I64),
            noble_kernel::types::Ty::Bool => Some(Self::Bool),
            _ => None,
        }
    }

    const fn word(self) -> &'static str {
        match self {
            Self::I64 => "I64",
            Self::Bool => "Bool",
        }
    }
}

/// A bounded scalar term. `Input`/`Output` name one assertion binder,
/// `Bound` one explicit lemma or motive binder, and `Ghost` the fresh value of
/// one call occurrence's output that no postcondition equation determined.
#[derive(Clone, PartialEq, Eq)]
enum Term3 {
    Input(usize),
    Output(usize),
    Bound(usize),
    Ghost(u32, u32, Sort3),
    I64(i64),
    Bool(bool),
    Add(Box<Term3>, Box<Term3>),
    Sub(Box<Term3>, Box<Term3>),
    Mul(Box<Term3>, Box<Term3>),
    Equals(Box<Term3>, Box<Term3>),
}

/// One proposition of the revision-3 assertion vocabulary.
#[derive(Clone, PartialEq, Eq)]
enum Prop3 {
    True,
    False,
    Not(Box<Prop3>),
    And(Box<Prop3>, Box<Prop3>),
    Eq(Term3, Term3),
    Lt(Term3, Term3),
    Le(Term3, Term3),
}

/// A lemma's statement: explicit `I64`/`Bool` binders ending in one
/// proposition. Only lemma proofs carry a Pi; assertions never do.
#[derive(Clone)]
enum Statement3 {
    Prop(Prop3),
    Pi {
        name: String,
        code: Sort3,
        body: Box<Statement3>,
    },
}

/// The name-resolution scope of one assertion or lemma statement.
struct Scope<'a> {
    inputs: &'a [(String, Sort3)],
    outputs: &'a [(String, Sort3)],
    bound: &'a [(String, Sort3)],
    /// Lemma statements may not mention a program interface.
    interface: bool,
}

impl Scope<'_> {
    fn sort(&self, term: &Term3) -> Option<Sort3> {
        sort_of(term, self.inputs, self.outputs, self.bound)
    }
}

fn make_scope<'a>(
    inputs: &'a [(String, Sort3)],
    outputs: &'a [(String, Sort3)],
    bound: &'a [(String, Sort3)],
    interface: bool,
) -> Scope<'a> {
    Scope {
        inputs,
        outputs,
        bound,
        interface,
    }
}

/// The sort of a term under one scope.
fn sort_of(
    term: &Term3,
    inputs: &[(String, Sort3)],
    outputs: &[(String, Sort3)],
    bound: &[(String, Sort3)],
) -> Option<Sort3> {
    match term {
        Term3::Input(index) => inputs.get(*index).map(|(_, code)| *code),
        Term3::Output(index) => outputs.get(*index).map(|(_, code)| *code),
        Term3::Bound(index) => bound.get(*index).map(|(_, code)| *code),
        Term3::Ghost(_, _, code) => Some(*code),
        Term3::I64(_) => Some(Sort3::I64),
        Term3::Bool(_) => Some(Sort3::Bool),
        Term3::Add(left, right) | Term3::Sub(left, right) | Term3::Mul(left, right) => {
            if sort_of(left, inputs, outputs, bound) == Some(Sort3::I64)
                && sort_of(right, inputs, outputs, bound) == Some(Sort3::I64)
            {
                Some(Sort3::I64)
            } else {
                None
            }
        }
        Term3::Equals(left, right) => {
            let left = sort_of(left, inputs, outputs, bound);
            let right = sort_of(right, inputs, outputs, bound);
            if left.is_some() && left == right {
                Some(Sort3::Bool)
            } else {
                None
            }
        }
    }
}

fn mentions_output(term: &Term3) -> bool {
    match term {
        Term3::Output(_) => true,
        Term3::Add(left, right)
        | Term3::Sub(left, right)
        | Term3::Mul(left, right)
        | Term3::Equals(left, right) => mentions_output(left) || mentions_output(right),
        Term3::Input(_) | Term3::Bound(_) | Term3::Ghost(..) | Term3::I64(_) | Term3::Bool(_) => {
            false
        }
    }
}

fn prop_mentions_output(prop: &Prop3) -> bool {
    match prop {
        Prop3::True | Prop3::False => false,
        Prop3::Not(body) => prop_mentions_output(body),
        Prop3::And(left, right) => prop_mentions_output(left) || prop_mentions_output(right),
        Prop3::Eq(left, right) | Prop3::Lt(left, right) | Prop3::Le(left, right) => {
            mentions_output(left) || mentions_output(right)
        }
    }
}

/// Fold closed literal arithmetic and simplify the two-valued connectives.
/// Nothing is reordered: the fold decides only on literal children, which is
/// Decision 3's bounded closed-literal computation.
fn normalize_term(term: &Term3) -> Term3 {
    match term {
        Term3::Add(left, right) => {
            let left = normalize_term(left);
            let right = normalize_term(right);
            match (&left, &right) {
                (Term3::I64(a), Term3::I64(b)) => Term3::I64(a.wrapping_add(*b)),
                _ => Term3::Add(Box::new(left), Box::new(right)),
            }
        }
        Term3::Sub(left, right) => {
            let left = normalize_term(left);
            let right = normalize_term(right);
            match (&left, &right) {
                (Term3::I64(a), Term3::I64(b)) => Term3::I64(a.wrapping_sub(*b)),
                _ => Term3::Sub(Box::new(left), Box::new(right)),
            }
        }
        Term3::Mul(left, right) => {
            let left = normalize_term(left);
            let right = normalize_term(right);
            match (&left, &right) {
                (Term3::I64(a), Term3::I64(b)) => Term3::I64(a.wrapping_mul(*b)),
                _ => Term3::Mul(Box::new(left), Box::new(right)),
            }
        }
        Term3::Equals(left, right) => {
            let left = normalize_term(left);
            let right = normalize_term(right);
            match (&left, &right) {
                (Term3::I64(a), Term3::I64(b)) => Term3::Bool(a == b),
                (Term3::Bool(a), Term3::Bool(b)) => Term3::Bool(a == b),
                _ => Term3::Equals(Box::new(left), Box::new(right)),
            }
        }
        Term3::Input(_)
        | Term3::Output(_)
        | Term3::Bound(_)
        | Term3::Ghost(..)
        | Term3::I64(_)
        | Term3::Bool(_) => term.clone(),
    }
}

fn normalize_prop(prop: &Prop3) -> Prop3 {
    match prop {
        Prop3::True | Prop3::False => prop.clone(),
        Prop3::Not(body) => match normalize_prop(body) {
            Prop3::True => Prop3::False,
            Prop3::False => Prop3::True,
            Prop3::Not(inner) => *inner,
            other => Prop3::Not(Box::new(other)),
        },
        Prop3::And(left, right) => {
            let left = normalize_prop(left);
            let right = normalize_prop(right);
            match (&left, &right) {
                (Prop3::False, _) | (_, Prop3::False) => Prop3::False,
                (Prop3::True, _) => right,
                (_, Prop3::True) => left,
                _ => Prop3::And(Box::new(left), Box::new(right)),
            }
        }
        Prop3::Eq(left, right) => {
            let left = normalize_term(left);
            let right = normalize_term(right);
            match (&left, &right) {
                (Term3::I64(a), Term3::I64(b)) => {
                    if a == b {
                        Prop3::True
                    } else {
                        Prop3::False
                    }
                }
                (Term3::Bool(a), Term3::Bool(b)) => {
                    if a == b {
                        Prop3::True
                    } else {
                        Prop3::False
                    }
                }
                _ => Prop3::Eq(left, right),
            }
        }
        Prop3::Lt(left, right) => {
            let left = normalize_term(left);
            let right = normalize_term(right);
            match (&left, &right) {
                (Term3::I64(a), Term3::I64(b)) => {
                    if a < b {
                        Prop3::True
                    } else {
                        Prop3::False
                    }
                }
                _ => Prop3::Lt(left, right),
            }
        }
        Prop3::Le(left, right) => {
            let left = normalize_term(left);
            let right = normalize_term(right);
            match (&left, &right) {
                (Term3::I64(a), Term3::I64(b)) => {
                    if a <= b {
                        Prop3::True
                    } else {
                        Prop3::False
                    }
                }
                _ => Prop3::Le(left, right),
            }
        }
    }
}

/// Replace the assertion's `in`/`out` binders by concrete terms.
fn instantiate_term(term: &Term3, inputs: &[Term3], outputs: &[Term3]) -> Term3 {
    match term {
        Term3::Input(index) => match inputs.get(*index) {
            Some(argument) => argument.clone(),
            None => term.clone(),
        },
        Term3::Output(index) => match outputs.get(*index) {
            Some(argument) => argument.clone(),
            None => term.clone(),
        },
        Term3::Add(left, right) => Term3::Add(
            Box::new(instantiate_term(left, inputs, outputs)),
            Box::new(instantiate_term(right, inputs, outputs)),
        ),
        Term3::Sub(left, right) => Term3::Sub(
            Box::new(instantiate_term(left, inputs, outputs)),
            Box::new(instantiate_term(right, inputs, outputs)),
        ),
        Term3::Mul(left, right) => Term3::Mul(
            Box::new(instantiate_term(left, inputs, outputs)),
            Box::new(instantiate_term(right, inputs, outputs)),
        ),
        Term3::Equals(left, right) => Term3::Equals(
            Box::new(instantiate_term(left, inputs, outputs)),
            Box::new(instantiate_term(right, inputs, outputs)),
        ),
        Term3::Bound(_) | Term3::Ghost(..) | Term3::I64(_) | Term3::Bool(_) => term.clone(),
    }
}

fn instantiate_prop(prop: &Prop3, inputs: &[Term3], outputs: &[Term3]) -> Prop3 {
    match prop {
        Prop3::True | Prop3::False => prop.clone(),
        Prop3::Not(body) => Prop3::Not(Box::new(instantiate_prop(body, inputs, outputs))),
        Prop3::And(left, right) => Prop3::And(
            Box::new(instantiate_prop(left, inputs, outputs)),
            Box::new(instantiate_prop(right, inputs, outputs)),
        ),
        Prop3::Eq(left, right) => Prop3::Eq(
            instantiate_term(left, inputs, outputs),
            instantiate_term(right, inputs, outputs),
        ),
        Prop3::Lt(left, right) => Prop3::Lt(
            instantiate_term(left, inputs, outputs),
            instantiate_term(right, inputs, outputs),
        ),
        Prop3::Le(left, right) => Prop3::Le(
            instantiate_term(left, inputs, outputs),
            instantiate_term(right, inputs, outputs),
        ),
    }
}

/// Replace one explicit binder by an applied argument and lower the levels
/// above it. Bound levels are de Bruijn levels, and an argument never mentions
/// a level, so no capture is possible.
fn replace_term(term: &Term3, level: usize, argument: &Term3) -> Term3 {
    match term {
        Term3::Bound(index) if *index == level => argument.clone(),
        Term3::Bound(index) if *index > level => Term3::Bound(index.saturating_sub(1)),
        Term3::Add(left, right) => Term3::Add(
            Box::new(replace_term(left, level, argument)),
            Box::new(replace_term(right, level, argument)),
        ),
        Term3::Sub(left, right) => Term3::Sub(
            Box::new(replace_term(left, level, argument)),
            Box::new(replace_term(right, level, argument)),
        ),
        Term3::Mul(left, right) => Term3::Mul(
            Box::new(replace_term(left, level, argument)),
            Box::new(replace_term(right, level, argument)),
        ),
        Term3::Equals(left, right) => Term3::Equals(
            Box::new(replace_term(left, level, argument)),
            Box::new(replace_term(right, level, argument)),
        ),
        Term3::Input(_)
        | Term3::Output(_)
        | Term3::Bound(_)
        | Term3::Ghost(..)
        | Term3::I64(_)
        | Term3::Bool(_) => term.clone(),
    }
}

fn replace_prop(prop: &Prop3, level: usize, argument: &Term3) -> Prop3 {
    match prop {
        Prop3::True | Prop3::False => prop.clone(),
        Prop3::Not(body) => Prop3::Not(Box::new(replace_prop(body, level, argument))),
        Prop3::And(left, right) => Prop3::And(
            Box::new(replace_prop(left, level, argument)),
            Box::new(replace_prop(right, level, argument)),
        ),
        Prop3::Eq(left, right) => Prop3::Eq(
            replace_term(left, level, argument),
            replace_term(right, level, argument),
        ),
        Prop3::Lt(left, right) => Prop3::Lt(
            replace_term(left, level, argument),
            replace_term(right, level, argument),
        ),
        Prop3::Le(left, right) => Prop3::Le(
            replace_term(left, level, argument),
            replace_term(right, level, argument),
        ),
    }
}

/// Parse one bounded scalar term.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass reader keeps every constructor and refusal together"
)]
fn parse_term(
    form: &Form,
    scope: &Scope<'_>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Term3, Diagnostic> {
    spend!(remaining, meter, depth, form.span);
    match &form.kind {
        FormKind::Atom(text) => {
            if let Some((index, _)) = scope
                .inputs
                .iter()
                .enumerate()
                .find(|(_, (name, _))| name == text)
            {
                return Ok(Term3::Input(index));
            }
            if let Some((index, _)) = scope
                .outputs
                .iter()
                .enumerate()
                .find(|(_, (name, _))| name == text)
            {
                return Ok(Term3::Output(index));
            }
            if let Some((index, _)) = scope
                .bound
                .iter()
                .enumerate()
                .find(|(_, (name, _))| name == text)
            {
                return Ok(Term3::Bound(index));
            }
            if text == "true" {
                return Ok(Term3::Bool(true));
            }
            if text == "false" {
                return Ok(Term3::Bool(false));
            }
            match text.parse::<i64>() {
                Ok(value) => Ok(Term3::I64(value)),
                Err(_) => Err(invalid(form.span, "expected bounded scalar term")),
            }
        }
        FormKind::List(items) => {
            let head = head_atom(items)
                .ok_or_else(|| invalid(form.span, "expected bounded scalar term"))?;
            if items.len() == 2 && head == "in" && scope.interface {
                let name = atom(&items[1])?;
                let index = scope
                    .inputs
                    .iter()
                    .position(|(candidate, _)| candidate == name)
                    .ok_or_else(|| invalid(items[1].span, "unbound input binder"))?;
                return Ok(Term3::Input(index));
            }
            if items.len() == 2 && head == "out" && scope.interface {
                let name = atom(&items[1])?;
                let index = scope
                    .outputs
                    .iter()
                    .position(|(candidate, _)| candidate == name)
                    .ok_or_else(|| invalid(items[1].span, "unbound output binder"))?;
                return Ok(Term3::Output(index));
            }
            if items.len() != 3 {
                return Err(invalid(form.span, "expected bounded scalar term"));
            }
            let next = depth.checked_add(1).ok_or_else(|| budget(form.span))?;
            let left = parse_term(&items[1], scope, next, remaining, meter)?;
            let right = parse_term(&items[2], scope, next, remaining, meter)?;
            let pair = (Box::new(left), Box::new(right));
            match head {
                "add" => Ok(Term3::Add(pair.0, pair.1)),
                "sub" => Ok(Term3::Sub(pair.0, pair.1)),
                "mul" => Ok(Term3::Mul(pair.0, pair.1)),
                "eq" => Ok(Term3::Equals(pair.0, pair.1)),
                _ => Err(invalid(form.span, "expected bounded scalar term")),
            }
        }
    }
}

fn require_i64(term: &Term3, scope: &Scope<'_>, span: Span) -> Result<(), Diagnostic> {
    if scope.sort(term) == Some(Sort3::I64) {
        Ok(())
    } else {
        Err(invalid(span, "expected I64 scalar term"))
    }
}

fn require_same_sort(
    left: &Term3,
    right: &Term3,
    scope: &Scope<'_>,
    span: Span,
) -> Result<(), Diagnostic> {
    let left_sort = scope.sort(left);
    if left_sort.is_some() && left_sort == scope.sort(right) {
        Ok(())
    } else {
        Err(invalid(span, "equality operands have different sorts"))
    }
}

/// Parse one assertion proposition.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass reader keeps every connective and refusal together"
)]
fn parse_prop(
    form: &Form,
    scope: &Scope<'_>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Prop3, Diagnostic> {
    spend!(remaining, meter, depth, form.span);
    let items = list(form)?;
    let head = head_atom(items).ok_or_else(|| invalid(form.span, "expected logical proposition"))?;
    match head {
        "true" if items.len() == 1 => Ok(Prop3::True),
        "false" if items.len() == 1 => Ok(Prop3::False),
        "not" if items.len() == 2 => {
            let next = depth.checked_add(1).ok_or_else(|| budget(form.span))?;
            Ok(Prop3::Not(Box::new(parse_prop(
                &items[1], scope, next, remaining, meter,
            )?)))
        }
        "and" if items.len() == 3 => {
            let next = depth.checked_add(1).ok_or_else(|| budget(form.span))?;
            let left = parse_prop(&items[1], scope, next, remaining, meter)?;
            let right = parse_prop(&items[2], scope, next, remaining, meter)?;
            Ok(Prop3::And(Box::new(left), Box::new(right)))
        }
        "eq" if items.len() == 3 => {
            let next = depth.checked_add(1).ok_or_else(|| budget(form.span))?;
            let left = parse_term(&items[1], scope, next, remaining, meter)?;
            let right = parse_term(&items[2], scope, next, remaining, meter)?;
            require_same_sort(&left, &right, scope, form.span)?;
            Ok(Prop3::Eq(left, right))
        }
        "lt" | "le" if items.len() == 3 => {
            let next = depth.checked_add(1).ok_or_else(|| budget(form.span))?;
            let left = parse_term(&items[1], scope, next, remaining, meter)?;
            let right = parse_term(&items[2], scope, next, remaining, meter)?;
            require_i64(&left, scope, items[1].span)?;
            require_i64(&right, scope, items[2].span)?;
            if head == "lt" {
                Ok(Prop3::Lt(left, right))
            } else {
                Ok(Prop3::Le(left, right))
            }
        }
        _ => Err(invalid(form.span, "expected logical proposition")),
    }
}

/// Parse one source assertion with a fresh profile node budget.
fn assertion(
    form: &Form,
    inputs: &[(String, Sort3)],
    outputs: &[(String, Sort3)],
    meter: &mut Meter<'_>,
) -> Result<Prop3, Diagnostic> {
    let scope = make_scope(inputs, outputs, &[], true);
    let mut remaining = MAX_ASSERTION_NODES;
    let prop = parse_prop(form, &scope, 0, &mut remaining, meter)?;
    Ok(prop)
}

/// Parse one lemma statement: explicit `I64`/`Bool` binders ending in an
/// interface-free proposition.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass statement reader keeps the Pi grammar and its refusals together"
)]
fn parse_statement(
    form: &Form,
    bound: &mut Vec<(String, Sort3)>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Statement3, Diagnostic> {
    spend!(remaining, meter, depth, form.span);
    if let FormKind::List(items) = &form.kind {
        if head_atom(items) == Some("Pi") && items.len() == 3 {
            let binder = list(&items[1])?;
            if binder.len() != 2 {
                return Err(invalid(items[1].span, "Pi needs one typed binder"));
            }
            let name = String::from(atom(&binder[0])?);
            let code = Sort3::from_word(atom(&binder[1])?).ok_or_else(|| {
                unsupported_profile(binder[1].span, "lemma binder is not I64 or Bool")
            })?;
            if bound.iter().any(|(candidate, _)| candidate == &name) {
                return Err(invalid(binder[0].span, "repeated lemma binder"));
            }
            bound.push((name.clone(), code));
            let next = depth.checked_add(1).ok_or_else(|| budget(form.span))?;
            let body = parse_statement(&items[2], bound, next, remaining, meter);
            bound.pop();
            return Ok(Statement3::Pi {
                name,
                code,
                body: Box::new(body?),
            });
        }
    }
    let scope = make_scope(&[], &[], bound, false);
    Ok(Statement3::Prop(parse_prop(
        form, &scope, depth, remaining, meter,
    )?))
}

/// One proof value the bidirectional revision-3 checker infers.
enum Value3 {
    Statement(Statement3),
}

/// One accepted lemma proof and its parsed statement.
struct Lemma {
    reference: String,
    statement: Statement3,
}

/// The logical environment a bridge or join proof may use.
struct ProofScope<'a> {
    facts: &'a [Prop3],
    lemmas: &'a [Lemma],
    contracts: &'a [&'a str],
    assignment: &'a Scope<'a>,
}

/// The index of a `fN` fact name.
fn fact_index(name: &str) -> Option<usize> {
    let digits = name.strip_prefix('f')?;
    if digits.is_empty()
        || digits.len() > 3
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    digits.parse::<usize>().ok()
}

fn fact_statement(scope: &ProofScope<'_>, name: &str) -> Option<Statement3> {
    let index = fact_index(name)?;
    scope
        .facts
        .get(index)
        .map(|fact| Statement3::Prop(normalize_prop(fact)))
}

fn lemma_statement<'a>(scope: &'a ProofScope<'a>, name: &str) -> Option<&'a Statement3> {
    scope
        .lemmas
        .iter()
        .find(|lemma| lemma.reference == name)
        .map(|lemma| &lemma.statement)
}

/// The three reviewed arithmetic rule identities.
#[derive(Clone, Copy)]
enum Rule3 {
    AddAssoc,
    AddComm,
    MulAdd,
}

/// Infer the statement of one bridge or join proof term.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the bidirectional inferer keeps every constructor's rule in one reviewable dispatch"
)]
fn infer(
    term: &Form,
    scope: &ProofScope<'_>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Value3, Diagnostic> {
    spend!(remaining, meter, depth, term.span);
    if let Ok(args) = operation(term, "use", 1) {
        let name = atom(&args[0])?;
        if let Some(statement) = fact_statement(scope, name) {
            return Ok(Value3::Statement(statement));
        }
        if let Some(statement) = lemma_statement(scope, name) {
            return Ok(Value3::Statement(statement.clone()));
        }
        if scope.contracts.iter().any(|reference| *reference == name) {
            return Err(refined(
                term.span,
                "preuse-sort-mismatch",
                "a contract proof names a program claim, not a lemma",
            ));
        }
        return Err(invalid(
            term.span,
            "unresolved, private, forward or cyclic proof use",
        ));
    }
    if let Ok(args) = operation(term, "apply", 2) {
        let next = depth.checked_add(1).ok_or_else(|| budget(term.span))?;
        let function = infer(&args[0], scope, next, remaining, meter)?;
        return apply_inferred(function, &args[1], scope, term.span, next, remaining, meter);
    }
    if let Ok(args) = operation(term, "refl", 1) {
        let scalar = parse_term(&args[0], scope.assignment, depth, remaining, meter)?;
        if scope.assignment.sort(&scalar).is_none() {
            return Err(invalid(args[0].span, "refl operand has no admitted sort"));
        }
        return Ok(Value3::Statement(Statement3::Prop(Prop3::Eq(
            scalar.clone(),
            scalar,
        ))));
    }
    if let Ok(args) = operation(term, "subst", 3) {
        return subst_inferred(args, scope, depth, remaining, meter);
    }
    if let Ok(args) = operation(term, "bv-add-assoc", 3) {
        return rule_inferred(Rule3::AddAssoc, args, scope, depth, remaining, meter);
    }
    if let Ok(args) = operation(term, "bv-add-comm", 2) {
        return rule_inferred(Rule3::AddComm, args, scope, depth, remaining, meter);
    }
    if let Ok(args) = operation(term, "bv-mul-add", 3) {
        return rule_inferred(Rule3::MulAdd, args, scope, depth, remaining, meter);
    }
    if operation(term, "by-true", 0).is_ok() {
        return Ok(Value3::Statement(Statement3::Prop(Prop3::True)));
    }
    Err(invalid(
        term.span,
        "proof term establishes no revision-3 proposition",
    ))
}

/// Infer one `apply`: consume the outermost explicit Pi binder.
fn apply_inferred(
    function: Value3,
    argument: &Form,
    scope: &ProofScope<'_>,
    span: Span,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Value3, Diagnostic> {
    let Value3::Statement(function) = function;
    let Statement3::Pi { body, code, .. } = function else {
        return Err(invalid(span, "apply requires a dependent Pi proof"));
    };
    let scalar = parse_term(argument, scope.assignment, depth, remaining, meter)?;
    if scope.assignment.sort(&scalar) != Some(code) {
        return Err(invalid(argument.span, "apply argument has wrong sort"));
    }
    Ok(Value3::Statement(replace_statement(&body, &scalar)))
}

/// Replace the outermost explicit binder of one statement.
fn replace_statement(statement: &Statement3, argument: &Term3) -> Statement3 {
    match statement {
        Statement3::Prop(prop) => Statement3::Prop(replace_prop(prop, 0, argument)),
        Statement3::Pi { name, code, body } => Statement3::Pi {
            name: name.clone(),
            code: *code,
            body: Box::new(replace_statement(body, argument)),
        },
    }
}

/// Infer one `subst` over a bounded scalar equality: `M(A)` and `A = B` give
/// `M(B)`.
fn subst_inferred(
    args: &[Form],
    scope: &ProofScope<'_>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Value3, Diagnostic> {
    let motive = list(&args[0])?;
    if motive.len() != 3 || head_atom(motive) != Some("motive") {
        return Err(invalid(args[0].span, "subst requires an explicit motive"));
    }
    let binder = list(&motive[1])?;
    if binder.len() != 2 {
        return Err(invalid(motive[1].span, "motive needs one typed binder"));
    }
    let name = String::from(atom(&binder[0])?);
    let code = Sort3::from_word(atom(&binder[1])?)
        .ok_or_else(|| unsupported_profile(binder[1].span, "motive binder is not I64 or Bool"))?;
    let level = scope.assignment.bound.len();
    let mut bound: Vec<(String, Sort3)> = scope.assignment.bound.to_vec();
    bound.push((name, code));
    let motive_scope = make_scope(scope.assignment.inputs, scope.assignment.outputs, &bound, scope.assignment.interface);
    let next = depth.checked_add(1).ok_or_else(|| budget(args[0].span))?;
    let body = parse_prop(&motive[2], &motive_scope, next, remaining, meter)?;
    let equality = infer(&args[2], scope, next, remaining, meter)?;
    let Value3::Statement(Statement3::Prop(equality)) = equality else {
        return Err(invalid(args[2].span, "subst requires equality evidence"));
    };
    let Prop3::Eq(left, right) = normalize_prop(&equality) else {
        return Err(invalid(args[2].span, "subst requires equality evidence"));
    };
    let left_sort = motive_scope.sort(&left);
    if left_sort != Some(code) || motive_scope.sort(&right) != Some(code) {
        return Err(invalid(
            args[2].span,
            "subst equality has the wrong code",
        ));
    }
    let premise = infer(&args[1], scope, next, remaining, meter)?;
    let Value3::Statement(Statement3::Prop(premise)) = premise else {
        return Err(invalid(args[1].span, "subst premise is not a proposition"));
    };
    let before = replace_prop(&body, level, &left);
    if normalize_prop(&premise) != normalize_prop(&before) {
        return Err(invalid(
            args[1].span,
            "subst premise does not match the motive instance",
        ));
    }
    Ok(Value3::Statement(Statement3::Prop(replace_prop(
        &body, level, &right,
    ))))
}

/// Infer one reviewed arithmetic rule instance.
fn rule_inferred(
    rule: Rule3,
    args: &[Form],
    scope: &ProofScope<'_>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Value3, Diagnostic> {
    let mut scalars = Vec::with_capacity(args.len());
    for argument in args {
        let scalar = parse_term(argument, scope.assignment, depth, remaining, meter)?;
        require_i64(&scalar, scope.assignment, argument.span)?;
        scalars.push(scalar);
    }
    let statement = match rule {
        Rule3::AddAssoc => {
            let (a, b, c) = (&scalars[0], &scalars[1], &scalars[2]);
            Prop3::Eq(
                Term3::Add(
                    Box::new(Term3::Add(Box::new(a.clone()), Box::new(b.clone()))),
                    Box::new(c.clone()),
                ),
                Term3::Add(
                    Box::new(a.clone()),
                    Box::new(Term3::Add(Box::new(b.clone()), Box::new(c.clone()))),
                ),
            )
        }
        Rule3::AddComm => {
            let (a, b) = (&scalars[0], &scalars[1]);
            Prop3::Eq(
                Term3::Add(Box::new(a.clone()), Box::new(b.clone())),
                Term3::Add(Box::new(b.clone()), Box::new(a.clone())),
            )
        }
        Rule3::MulAdd => {
            let (a, b, c) = (&scalars[0], &scalars[1], &scalars[2]);
            Prop3::Eq(
                Term3::Mul(
                    Box::new(a.clone()),
                    Box::new(Term3::Add(Box::new(b.clone()), Box::new(c.clone()))),
                ),
                Term3::Add(
                    Box::new(Term3::Mul(Box::new(a.clone()), Box::new(b.clone()))),
                    Box::new(Term3::Mul(Box::new(a.clone()), Box::new(c.clone()))),
                ),
            )
        }
    };
    Ok(Value3::Statement(Statement3::Prop(statement)))
}

/// Check that `proof` establishes exactly `expected` up to literal folding.
fn check_proof(
    proof: &Form,
    expected: &Prop3,
    scope: &ProofScope<'_>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let mut remaining = MAX_ASSERTION_NODES;
    let value = infer(proof, scope, 0, &mut remaining, meter)?;
    let Value3::Statement(statement) = value;
    let Statement3::Prop(established) = statement else {
        return Err(invalid(proof.span, "proof is not a proposition instance"));
    };
    if normalize_prop(&established) == normalize_prop(expected) {
        Ok(())
    } else {
        Err(invalid(
            proof.span,
            "proof does not establish the exact expected proposition",
        ))
    }
}

/// The ordered `name Type` interface of a contract, or a refusal.
fn interface(bindings: &[(String, String)], span: Span) -> Result<Vec<(String, Sort3)>, Diagnostic> {
    if bindings.len() > MAX_INTERFACE {
        return Err(unsupported_profile(
            span,
            "revision-3 interface exceeds four values",
        ));
    }
    let mut result = Vec::with_capacity(bindings.len());
    for (name, ty) in bindings {
        let code = Sort3::from_word(ty).ok_or_else(|| {
            unsupported_profile(span, "revision-3 interface is not I64/Bool only")
        })?;
        if name.is_empty() {
            return Err(invalid(span, "empty interface binder"));
        }
        if result.iter().any(|(candidate, _)| candidate == name) {
            return Err(invalid(span, "repeated interface binder"));
        }
        result.push((name.clone(), code));
    }
    Ok(result)
}

fn interface_codes(interface: &[(String, Sort3)]) -> Vec<Sort3> {
    let mut codes = Vec::with_capacity(interface.len());
    for (_, code) in interface {
        codes.push(*code);
    }
    codes
}

/// The canonical source-level claim text of one revision-3 contract. It is an
/// integrity index and human-readable projection, never Lean authority.
fn claim_text(goal: &ContractGoal) -> String {
    let mut text = String::from("Named-Contract-Calls-v3 claim\n");
    text.push_str(&goal.subject.module);
    text.push('@');
    crate::rendering::number(&mut text, u64::from(goal.subject.version));
    text.push('.');
    text.push_str(&goal.contract_name);
    text.push_str("\nrequires ");
    write_form(&mut text, &goal.requires, 0);
    text.push_str("\nensures ");
    write_form(&mut text, &goal.ensures, 0);
    text
}

fn write_form(text: &mut String, form: &Form, depth: u32) {
    if depth > 64 {
        text.push_str("...");
        return;
    }
    match &form.kind {
        FormKind::Atom(word) => text.push_str(word),
        FormKind::List(items) => {
            text.push('(');
            let mut first = true;
            for item in items {
                if !first {
                    text.push(' ');
                }
                first = false;
                write_form(text, item, depth.saturating_add(1));
            }
            text.push(')');
        }
    }
}

/// Authenticate every traced occurrence against the immutable caller and
/// callee source and the frozen import snapshot: the full-token span, the
/// spelled scope, the callee's exported original definition and its body.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the revision-3 contract suite."
)]
fn authenticate_occurrences(
    goal: &ContractGoal,
    occurrences: &[&NamedUseOrigin],
    limits: Limits,
) -> Result<(), Diagnostic> {
    let subject = &goal.subject;
    let span = subject.source_span;
    let caller = || crate::source::DefinitionOrigin {
        module_source: &subject.module_source,
        module: &subject.module,
        version: subject.version,
        definition: &subject.definition,
        ordinal: subject.definition_ordinal,
        span,
    };
    let subject_body = crate::source::verify_definition_origin(
        caller(),
        &subject.definition_source,
        limits,
    )?;
    // The subject's own original module must be one retained dependency, and
    // no module owner may repeat.
    let mut owners = Vec::with_capacity(subject.source_dependencies.len());
    for module in &subject.source_dependencies {
        if owners.iter().any(|owner| *owner == module.owner) {
            return Err(invalid(span, "duplicate named source module"));
        }
        owners.push(module.owner);
    }
    if !subject.source_dependencies.iter().any(|module| {
        module.owner == subject.definition_owner
            && module.module == subject.module
            && module.version == subject.version
            && module.full_source == subject.module_source
    }) {
        return Err(invalid(span, "subject module is not a retained source"));
    }
    for binding in &subject.named_imports {
        match subject.source_dependencies.get(binding.module_index) {
            Some(module) if module.owner == binding.owner => {}
            _ => {
                return Err(invalid(
                    span,
                    "retained import alias lacks its frozen original module",
                ))
            }
        }
        if subject
            .named_imports
            .iter()
            .filter(|other| other.alias == binding.alias)
            .count()
            != 1
        {
            return Err(invalid(span, "repeated retained import alias"));
        }
    }
    for occurrence in occurrences {
        let Some(source_node) = occurrence.source_node else {
            return Err(invalid(span, "call occurrence has no source node"));
        };
        let Some(source_span) = occurrence.source_span else {
            return Err(invalid(span, "call occurrence has no source span"));
        };
        let module = subject
            .source_dependencies
            .get(occurrence.module_index)
            .ok_or_else(|| invalid(span, "callee module is not retained"))?;
        if occurrence.body_span != subject_body {
            return Err(invalid(
                occurrence.definition_span,
                "call occurrence is outside the caller's original body",
            ));
        }
        let callee_body = crate::source::verify_definition_origin(
            crate::source::DefinitionOrigin {
                module_source: &module.full_source,
                module: &module.module,
                version: module.version,
                definition: &occurrence.definition_name,
                ordinal: occurrence.definition_ordinal,
                span: occurrence.definition_span,
            },
            source_bytes(
                &module.full_source,
                occurrence.definition_span,
                occurrence.definition_span,
            )?,
            limits,
        )?;
        if callee_body != occurrence.body_span {
            return Err(invalid(
                occurrence.definition_span,
                "callee body span differs from its original definition",
            ));
        }
        if module.owner != subject.definition_owner {
            crate::source::verify_named_export(
                &module.full_source,
                &module.module,
                module.version,
                &occurrence.definition_name,
                occurrence.definition_ordinal,
                occurrence.definition_span,
                limits,
            )?;
        }
        let original = crate::source::verify_named_call_origin(
            caller(),
            source_node,
            source_span,
            limits,
        )?;
        let (scope, word) = match crate::scan::rsplit_once_byte(original, b'.') {
            Some((scope, word)) => (Some(scope), word),
            None => (None, original),
        };
        if word != occurrence.definition_name {
            return Err(invalid(
                source_span,
                "original named word differs from the frozen callee",
            ));
        }
        match scope {
            Some(scope) => {
                if !names_module(scope, module) && !snapshot_alias(subject, scope, module) {
                    return Err(invalid(
                        source_span,
                        "original named scope is not the frozen callee module",
                    ));
                }
            }
            None => {
                if module.owner != subject.definition_owner
                    || occurrence.module_index != subject.named_uses[0].module_index
                {
                    return Err(invalid(
                        source_span,
                        "unqualified named word has another original owner",
                    ));
                }
            }
        }
    }
    Ok(())
}

/// The retained bytes `span` names, or an empty slice when they are absent.
fn source_bytes<'a>(
    source: &'a [u8],
    span: Span,
    at: Span,
) -> Result<&'a [u8], Diagnostic> {
    let (Ok(start), Ok(end)) = (
        usize::try_from(span.start),
        usize::try_from(span.end),
    ) else {
        return Err(invalid(span, "source span exceeds the address space"));
    };
    source
        .get(start..end)
        .ok_or_else(|| invalid(at, "source span is outside the retained module"))
}

/// Whether `qualified` is exactly `name@version` naming `module`.
fn names_module(qualified: &str, module: &super::ResolvedSourceModule) -> bool {
    match crate::scan::rsplit_once_byte(qualified, b'@') {
        Some((name, version)) => {
            name == module.module && version.parse::<u32>() == Ok(module.version)
        }
        None => false,
    }
}

/// Whether the frozen snapshot binds `scope` to exactly `module`.
fn snapshot_alias(
    subject: &super::SubjectBinding,
    scope: &str,
    module: &super::ResolvedSourceModule,
) -> bool {
    let mut position = 0;
    let mut is_bound = false;
    while !is_bound && position < subject.named_imports.len() {
        let binding = &subject.named_imports[position];
        is_bound = binding.alias == scope
            && binding.owner == module.owner
            && binding.module_index == dependency_position(&subject.source_dependencies, module.owner).unwrap_or(usize::MAX);
        position += 1;
    }
    is_bound
}

fn dependency_position(modules: &[super::ResolvedSourceModule], owner: u64) -> Option<usize> {
    let mut index = 0;
    let mut found = None;
    while found.is_none() && index < modules.len() {
        if modules[index].owner == owner {
            found = Some(index);
        }
        index += 1;
    }
    found
}

/// One exact source-visible call occurrence in the reconstructed envelope.
#[derive(Clone)]
pub struct CallRecord {
    /// Source-order ordinal among the caller's own call occurrences.
    pub ordinal: u32,
    pub source_span: Span,
    pub callee_module: String,
    pub callee_version: u32,
    pub callee_definition: String,
    pub callee_ordinal: u32,
    pub callee_span: Span,
    pub specialization: u32,
    pub proof: String,
    pub inputs: Vec<(String, &'static str)>,
    pub outputs: Vec<(String, &'static str)>,
}

/// The independently reconstructed subject and dependency envelope.
#[derive(Clone)]
pub struct Envelope {
    pub module: String,
    pub version: u32,
    pub definition: String,
    pub contract: String,
    pub occurrences: Vec<CallRecord>,
    /// The distinct replayed closure identities, in replay (post) order.
    pub closure: Vec<String>,
    /// The distinct logical lemma identities the proof used.
    pub lemmas: Vec<String>,
}

/// A checked revision-3 caller proof: its regenerated canonical claim and its
/// envelope. No Lean lowering is produced by this profile yet, so the consumer
/// refuses publication rather than claiming an unproved theorem.
pub struct PreparedV3 {
    pub claim: String,
    pub envelope: Envelope,
}

fn replay_identity(dependency: &ProofDependency) -> String {
    let mut identity = dependency.module.clone();
    identity.push('@');
    crate::rendering::number(&mut identity, u64::from(dependency.version));
    identity.push('/');
    identity.push_str(&dependency.declaration.name);
    identity
}

fn callee_identity(
    occurrence: &NamedUseOrigin,
    module: &super::ResolvedSourceModule,
) -> String {
    let mut identity = module.module.clone();
    identity.push('@');
    crate::rendering::number(&mut identity, u64::from(module.version));
    identity.push('.');
    identity.push_str(&occurrence.definition_name);
    identity
}

/// One resolved callee contract and its source-side proof record.
struct Callee<'a> {
    contract: &'a ContractGoal,
    dependency: Option<&'a ProofDependency>,
    reference: &'a str,
}

/// Resolve one `pc-call` reference among the imported dependency snapshot and
/// the caller module's earlier proofs.
fn resolve_callee<'a>(
    reference: &'a str,
    dependencies: &'a [ProofDependency],
    local: &'a [ProofObligation],
    span: Span,
) -> Result<Callee<'a>, Diagnostic> {
    for dependency in dependencies {
        if dependency.name == reference {
            if !dependency.exported {
                return Err(refined(
                    span,
                    "preuse-dependency-missing",
                    "the imported proof is private",
                ));
            }
            let PendingGoal::Contract { contract } = &dependency.declaration.goal else {
                return Err(refined(
                    span,
                    "preuse-sort-mismatch",
                    "a lemma proof is not a program claim",
                ));
            };
            return Ok(Callee {
                contract,
                dependency: Some(dependency),
                reference,
            });
        }
    }
    for obligation in local {
        if obligation.name == reference {
            let PendingGoal::Contract { contract } = &obligation.goal else {
                return Err(refined(
                    span,
                    "preuse-sort-mismatch",
                    "a lemma proof is not a program claim",
                ));
            };
            return Ok(Callee {
                contract,
                dependency: None,
                reference,
            });
        }
    }
    Err(refined(
        span,
        "preuse-dependency-missing",
        "no exported contract proof has this reference",
    ))
}

/// The fields of one occurrence that the callee contract must reproduce
/// exactly. The first differing field names the refusal.
fn callee_differs(
    occurrence: &NamedUseOrigin,
    contract: &ContractGoal,
    module: &super::ResolvedSourceModule,
) -> Option<&'static str> {
    if contract.revision != 1 && contract.revision != 3 {
        return Some("revision");
    }
    if contract.subject.module != module.module
        || contract.subject.version != module.version
        || contract.subject.module_source != module.full_source
    {
        return Some("module identity");
    }
    if contract.subject.definition != occurrence.definition_name
        || contract.subject.definition_ordinal != occurrence.definition_ordinal
        || contract.subject.source_span != occurrence.definition_span
        || contract.subject.definition_identity != occurrence.definition_identity
        || contract.subject.definition_owner != occurrence.definition_owner
    {
        return Some("callee subject");
    }
    let (Ok(start), Ok(end)) = (
        usize::try_from(contract.subject.source_span.start),
        usize::try_from(contract.subject.source_span.end),
    ) else {
        return Some("callee source span");
    };
    match contract.subject.module_source.get(start..end) {
        Some(body) if body == contract.subject.definition_source => {}
        _ => return Some("callee definition source"),
    }
    if !contract.subject.effects.is_empty() {
        return Some("effects");
    }
    if contract.subject.input_types.len() > MAX_INTERFACE
        || contract.subject.output_types.len() > MAX_INTERFACE
        || contract.inputs.len() != contract.subject.input_types.len()
        || contract.outputs.len() != contract.subject.output_types.len()
    {
        return Some("interface");
    }
    let mut index = 0;
    let mut differs = None;
    while differs.is_none() && index < contract.inputs.len() {
        differs = match Sort3::from_ty(&contract.subject.input_types[index]) {
            Some(code) if Sort3::from_word(&contract.inputs[index].1) == Some(code) => None,
            _ => Some("input interface"),
        };
        index += 1;
    }
    let mut index = 0;
    while differs.is_none() && index < contract.outputs.len() {
        differs = match Sort3::from_ty(&contract.subject.output_types[index]) {
            Some(code) if Sort3::from_word(&contract.outputs[index].1) == Some(code) => None,
            _ => Some("output interface"),
        };
        index += 1;
    }
    differs
}

/// One caller-body step the symbolic walk supports.
enum Step {
    Literal(noble_kernel::untrusted::Lit),
    Builtin(u32),
    Call(usize),
}

fn classify(
    node: &noble_kernel::untrusted::Node,
    occurrence: Option<usize>,
) -> Option<Step> {
    match node {
        noble_kernel::untrusted::Node::Literal { lit, inst } if inst.bindings.is_empty() => {
            Some(Step::Literal(lit.clone()))
        }
        noble_kernel::untrusted::Node::Invocation { def, inst } if inst.bindings.is_empty() => {
            match def.0 {
                0 | 1 | 2 | 4 | 5 | 6 | 7 => Some(Step::Builtin(def.0)),
                _ => occurrence.map(Step::Call),
            }
        }
        _ => None,
    }
}

fn call_slice(slice: &Form) -> Result<(usize, &str, &Form), Diagnostic> {
    let call = operation(slice, "pc-call", 3)?;
    let ordinal = atom(&call[0])?
        .parse::<usize>()
        .map_err(|_| invalid(call[0].span, "call slice ordinal is not a number"))?;
    let reference = atom(&call[1])?;
    Ok((ordinal, reference, &call[2]))
}

fn join_target(term: &Form) -> Result<&Form, Diagnostic> {
    let items = list(term)?;
    if head_atom(items) != Some("export-named-claim") || items.len() < 2 {
        return Err(invalid(
            term.span,
            "revision-3 proof requires export-named-claim",
        ));
    }
    let join = list(&items[items.len().saturating_sub(1)])?;
    if head_atom(join) != Some("join") || join.len() != 2 {
        return Err(invalid(term.span, "revision-3 proof requires a final join"));
    }
    Ok(&join[1])
}

fn slices(term: &Form) -> Result<&[Form], Diagnostic> {
    let items = list(term)?;
    if head_atom(items) != Some("export-named-claim") || items.len() < 2 {
        return Err(invalid(
            term.span,
            "revision-3 proof requires export-named-claim",
        ));
    }
    Ok(&items[1..items.len().saturating_sub(1)])
}

fn replay_seen(seen: &[String], identity: &str) -> bool {
    seen.iter().any(|item| item == identity)
}

fn add_fact(facts: &mut Vec<Prop3>, prop: &Prop3) -> Result<(), Diagnostic> {
    match prop {
        Prop3::And(left, right) => {
            add_fact(facts, left)?;
            add_fact(facts, right)
        }
        _ => {
            if facts.len() >= MAX_FACTS {
                return Err(budget(Span { start: 0, end: 0 }));
            }
            facts.push(normalize_prop(prop));
            Ok(())
        }
    }
}

fn collect_conjuncts<'a>(prop: &'a Prop3, out: &mut Vec<&'a Prop3>) {
    match prop {
        Prop3::And(left, right) => {
            collect_conjuncts(left, out);
            collect_conjuncts(right, out);
        }
        _ => out.push(prop),
    }
}

/// The direct output term `U[in ↦ arguments]` of a top-level
/// `(eq (out j) U)` conjunct that mentions no output.
fn direct_output(ensures: &Prop3, index: usize, arguments: &[Term3]) -> Option<Term3> {
    let mut conjuncts: Vec<&Prop3> = Vec::new();
    collect_conjuncts(ensures, &mut conjuncts);
    for conjunct in conjuncts {
        let Prop3::Eq(left, right) = conjunct else {
            continue;
        };
        if matches!(left, Term3::Output(found) if *found == index) && !mentions_output(right) {
            return Some(normalize_term(&instantiate_term(right, arguments, &[])));
        }
    }
    None
}

/// Scan every `(use NAME)` of a proof term, in form order.
fn lemma_names(form: &Form, names: &mut Vec<String>) -> Result<(), Diagnostic> {
    match &form.kind {
        FormKind::Atom(_) => Ok(()),
        FormKind::List(items) => {
            if head_atom(items) == Some("use") && items.len() == 2 {
                let name = atom(&items[1])?;
                let is_fact = fact_index(name).is_some();
                if !is_fact && !names.iter().any(|item| item == name) {
                    if names.len() >= MAX_LEMMA_USES {
                        return Err(budget(form.span));
                    }
                    names.push(String::from(name));
                }
            }
            for item in items {
                lemma_names(item, names)?;
            }
            Ok(())
        }
    }
}

/// Resolve every distinct lemma use against the imported dependency snapshot
/// and parse its statement.
fn resolved_lemmas(
    names: &[String],
    dependencies: &[ProofDependency],
    span: Span,
    meter: &mut Meter<'_>,
) -> Result<Vec<Lemma>, Diagnostic> {
    let mut lemmas = Vec::with_capacity(names.len());
    for name in names {
        let Some(dependency) = dependencies.iter().find(|item| item.name == *name) else {
            return Err(refined(
                span,
                "preuse-dependency-missing",
                "no exported lemma proof has this reference",
            ));
        };
        if !dependency.exported {
            return Err(refined(
                span,
                "preuse-dependency-missing",
                "the imported lemma is private",
            ));
        }
        let PendingGoal::Pure { proposition } = &dependency.declaration.goal else {
            return Err(refined(
                span,
                "preuse-sort-mismatch",
                "a program contract is not a logical lemma",
            ));
        };
        let mut bound: Vec<(String, Sort3)> = Vec::new();
        let mut remaining = MAX_ASSERTION_NODES;
        let statement = parse_statement(proposition, &mut bound, 0, &mut remaining, meter)?;
        lemmas.push(Lemma {
            reference: name.clone(),
            statement,
        });
    }
    Ok(lemmas)
}

fn caller_occurrences<'a>(
    goal: &'a ContractGoal,
    selected: &'a noble_kernel::execution::Definition,
) -> Vec<&'a NamedUseOrigin> {
    let mut result: Vec<&NamedUseOrigin> = goal
        .subject
        .named_uses
        .iter()
        .skip(1)
        .filter(|origin| origin.caller_definition == Some(selected.definition))
        .collect();
    result.sort_by_key(|origin| origin.source_span.map_or(0, |span| span.start));
    result
}

/// The symbolic state of one caller body walk.
struct State {
    stack: Vec<Term3>,
    facts: Vec<Prop3>,
    ghosts: u32,
}

/// Push one value, refusing a stack over the profile bound.
fn push(stack: &mut Vec<Term3>, term: Term3, span: Span) -> Result<(), Diagnostic> {
    if stack.len() >= MAX_STACK {
        return Err(budget(span));
    }
    stack.push(term);
    Ok(())
}

/// Apply one pure builtin step.
fn builtin_step(stack: &mut Vec<Term3>, word: u32, span: Span) -> Result<(), Diagnostic> {
    match word {
        0 => {
            let Some(top) = stack.last().cloned() else {
                return Err(invalid(span, "dup below the empty stack"));
            };
            push(stack, top, span)?;
        }
        1 => {
            if stack.pop().is_none() {
                return Err(invalid(span, "drop below the empty stack"));
            }
        }
        2 => {
            if stack.len() < 2 {
                return Err(invalid(span, "swap below a two-value stack"));
            }
            let top = stack.len().saturating_sub(1);
            stack.swap(top, top.saturating_sub(1));
        }
        4 | 5 | 6 => {
            let Some(right) = stack.pop() else {
                return Err(invalid(span, "arithmetic below a two-value stack"));
            };
            let Some(left) = stack.pop() else {
                return Err(invalid(span, "arithmetic below a two-value stack"));
            };
            let pair = (Box::new(left), Box::new(right));
            let term = match word {
                4 => Term3::Add(pair.0, pair.1),
                5 => Term3::Sub(pair.0, pair.1),
                _ => Term3::Mul(pair.0, pair.1),
            };
            push(stack, term, span)?;
        }
        7 => {
            let Some(right) = stack.pop() else {
                return Err(invalid(span, "equality below a two-value stack"));
            };
            let Some(left) = stack.pop() else {
                return Err(invalid(span, "equality below a two-value stack"));
            };
            push(stack, Term3::Equals(Box::new(left), Box::new(right)), span)?;
        }
        _ => {
            return Err(unsupported_profile(
                span,
                "builtin is outside the pure vocabulary",
            ))
        }
    }
    Ok(())
}

/// Allocate one fresh ghost name for a call output.
fn ghost(state: &mut State, index: u32, code: Sort3) -> Term3 {
    let name = Term3::Ghost(state.ghosts, index, code);
    name
}

/// Apply one callee contract step: exact argument sorting, explicit bridge,
/// contract outputs and the carried postcondition.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the call rule keeps argument sorting, the bridge, the output terms and the fact addition in one reviewable sequence"
)]
fn call_step(
    state: &mut State,
    callee: &Callee<'_>,
    bridge: &Form,
    contracts: &[&str],
    lemmas: &[Lemma],
    inputs: &[(String, Sort3)],
    outputs: &[(String, Sort3)],
    span: Span,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let contract = callee.contract;
    let callee_inputs = interface(&contract.inputs, contract.contract_span)?;
    let callee_outputs = interface(&contract.outputs, contract.contract_span)?;
    let arity = callee_inputs.len();
    if state.stack.len() < arity {
        return Err(invalid(
            span,
            "call consumes more values than the stack holds",
        ));
    }
    let split = state.stack.len().saturating_sub(arity);
    let arguments: Vec<Term3> = state.stack[split..].to_vec();
    for (argument, (_, code)) in arguments.iter().zip(&callee_inputs) {
        if sort_of(argument, inputs, outputs, &[]) != Some(*code) {
            return Err(invalid(span, "call argument has the wrong ordered sort"));
        }
    }
    let requires = assertion(
        &contract.requires,
        &callee_inputs,
        &callee_outputs,
        meter,
    )?;
    if prop_mentions_output(&requires) {
        return Err(unsupported_profile(
            contract.requires.span,
            "callee precondition mentions an output",
        ));
    }
    let bridge_goal = normalize_prop(&instantiate_prop(&requires, &arguments, &[]));
    let assignment = make_scope(inputs, outputs, &[], true);
    let proof_scope = ProofScope {
        facts: &state.facts,
        lemmas,
        contracts,
        assignment: &assignment,
    };
    check_proof(bridge, &bridge_goal, &proof_scope, meter)
        .map_err(|problem| refined(bridge.span, "preuse-bridge-unproved", &problem.message))?;
    let ensures = assertion(
        &contract.ensures,
        &callee_inputs,
        &callee_outputs,
        meter,
    )?;
    let mut results: Vec<Term3> = Vec::with_capacity(callee_outputs.len());
    for (index, (_, code)) in callee_outputs.iter().enumerate() {
        match direct_output(&ensures, index, &arguments) {
            Some(term) => results.push(term),
            None => {
                let fresh = ghost(
                    state,
                    u32::try_from(index).unwrap_or(u32::MAX),
                    *code,
                );
                results.push(fresh);
            }
        }
    }
    state.stack.truncate(split);
    let instance = normalize_prop(&instantiate_prop(&ensures, &arguments, &results));
    add_fact(&mut state.facts, &instance)?;
    for term in results {
        push(&mut state.stack, term, span)?;
    }
    state.ghosts = state.ghosts.saturating_add(1);
    Ok(())
}

/// Replay every distinct closure member once. Contract members are replayed by
/// their revision's own checker; a revision-3 member re-enters this module at
/// the next depth.
fn replay(
    local: &[ProofObligation],
    plan: &[&ProofDependency],
    local_plan: &[&ProofObligation],
    replayed: &mut Vec<String>,
    closure: &mut Vec<String>,
    depth: u32,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    if closure.len().saturating_add(plan.len()).saturating_add(local_plan.len()) > MAX_CLOSURE {
        return Err(budget(Span { start: 0, end: 0 }));
    }
    for dependency in plan {
        let identity = replay_identity(dependency);
        if replay_seen(replayed, &identity) {
            continue;
        }
        match dependency.declaration.revision {
            1 => {
                let (next, outcome) = super::check_dependency(dependency, false, *meter);
                *meter = next;
                outcome?;
            }
            3 => {
                let PendingGoal::Contract { contract } = &dependency.declaration.goal else {
                    return Err(invalid(
                        dependency.declaration.span,
                        "revision-3 dependency is not a contract",
                    ));
                };
                verify(contract, &dependency.dependencies, &[], &dependency.declaration.term, depth, meter)?;
            }
            _ => {
                return Err(unsupported_profile(
                    dependency.declaration.span,
                    "closure member has an unsupported revision",
                ));
            }
        }
        replayed.push(identity.clone());
        closure.push(identity);
    }
    for obligation in local_plan {
        let mut identity = String::from("local/");
        identity.push_str(&obligation.name);
        if replay_seen(replayed, &identity) {
            continue;
        }
        match &obligation.goal {
            PendingGoal::Pure { .. } => {
                return Err(unsupported_profile(
                    obligation.span,
                    "local closure member is not a contract",
                ))
            }
            PendingGoal::Contract { contract } => match contract.revision {
                1 => {
                    let (next, outcome) = super::check_contract(obligation, contract, *meter);
                    *meter = next;
                    outcome?;
                }
                3 => {
                    verify(contract, &[], local, &obligation.term, depth, meter)?;
                }
                _ => {
                    return Err(unsupported_profile(
                        obligation.span,
                        "local closure member has an unsupported revision",
                    ))
                }
            },
        }
        replayed.push(identity.clone());
        closure.push(identity);
    }
    Ok(())
}

/// Check one revision-3 caller proof. `depth` is its contract-call depth in the
/// replayed closure; it never exceeds `MAX_CLOSURE_DEPTH`.
/// The subject and occurrence view both contract staging and proof checking
/// reconstruct: the declared interface, the selected subject, its independent
/// kernel acceptance, and the authenticated ordered call occurrences.
struct SubjectView<'a> {
    inputs: Vec<(String, Sort3)>,
    outputs: Vec<(String, Sort3)>,
    input_codes: Vec<Sort3>,
    output_codes: Vec<Sort3>,
    selected: &'a noble_kernel::execution::Definition,
    checked: noble_kernel::untrusted::Checked,
    occurrences: Vec<&'a NamedUseOrigin>,
}

fn subject_view<'a>(
    goal: &'a ContractGoal,
    meter: &mut Meter<'_>,
) -> Result<SubjectView<'a>, Diagnostic> {
    let _ = meter;
    if goal.revision != 3 {
        return Err(invalid(
            goal.contract_span,
            "revision-3 checker received another revision",
        ));
    }
    let subject = &goal.subject;
    let span = subject.source_span;
    let inputs = interface(&goal.inputs, goal.contract_span)?;
    let outputs = interface(&goal.outputs, goal.contract_span)?;
    let input_codes = interface_codes(&inputs);
    let output_codes = interface_codes(&outputs);
    let mut subject_inputs = Vec::with_capacity(subject.input_types.len());
    for ty in &subject.input_types {
        subject_inputs.push(Sort3::from_ty(ty).ok_or_else(|| {
            unsupported_profile(span, "revision-3 subject interface is not I64/Bool only")
        })?);
    }
    let mut subject_outputs = Vec::with_capacity(subject.output_types.len());
    for ty in &subject.output_types {
        subject_outputs.push(Sort3::from_ty(ty).ok_or_else(|| {
            unsupported_profile(span, "revision-3 subject interface is not I64/Bool only")
        })?);
    }
    if subject_inputs != input_codes || subject_outputs != output_codes {
        return Err(invalid(
            span,
            "subject interface differs from the declared contract",
        ));
    }
    if !subject.effects.is_empty() {
        return Err(unsupported_profile(span, "revision-3 subjects are pure"));
    }
    let Some(root) = subject.named_uses.first() else {
        return Err(invalid(span, "revision-3 subject has no origin"));
    };
    let Some(selected) = crate::scan::find_first(&subject.accepted_submission.definitions, |item| {
        item.definition == root.definition
    }) else {
        return Err(invalid(
            span,
            "unique selected revision-3 subject is absent",
        ));
    };
    if selected.identity != subject.definition_identity
        || root.caller_definition.is_some()
        || root.source_span.is_some()
        || root.source_node.is_some()
        || root.definition_span != span
        || root.definition_name != subject.definition
        || root.definition_ordinal != subject.definition_ordinal
        || root.definition_owner != subject.definition_owner
    {
        return Err(invalid(
            span,
            "revision-3 subject root is not the selected definition",
        ));
    }
    let accepted = &subject.accepted_submission;
    if accepted.body.candidate.body != [root.candidate_node] {
        return Err(invalid(
            span,
            "accepted submission root is not the exact subject invocation",
        ));
    }
    let checked = super::checked_flat(
        &accepted.body,
        &accepted.request,
        &accepted.environment,
        span,
        MAX_ASSERTION_NODES,
    )?;
    if selected.identity != subject.definition_identity {
        return Err(invalid(span, "selected subject identity differs"));
    }
    let occurrences = caller_occurrences(goal, selected);
    if occurrences.len() > MAX_OCCURRENCES {
        return Err(budget(span));
    }
    let body = &selected.body.candidate.body;
    if body.len() != selected.body.candidate.nodes.len() {
        return Err(unsupported_profile(
            span,
            "revision-3 caller body is not flat",
        ));
    }
    let mut slot_seen: Vec<u32> = Vec::with_capacity(occurrences.len());
    let mut distinct: Vec<String> = Vec::with_capacity(MAX_DISTINCT_CALLEES);
    for (index, occurrence) in occurrences.iter().enumerate() {
        let Some(source_span) = occurrence.source_span else {
            return Err(invalid(span, "call occurrence has no source span"));
        };
        let Some(source_node) = occurrence.source_node else {
            return Err(invalid(span, "call occurrence has no source node"));
        };
        if usize::try_from(source_node).ok() != Some(index)
            || source_span.start < span.start
            || source_span.end > span.end
            || source_span.start >= source_span.end
        {
            return Err(invalid(
                source_span,
                "call occurrence is outside the ordered source body",
            ));
        }
        if slot_seen.iter().any(|slot| *slot == occurrence.definition.0) {
            return Err(invalid(
                occurrence.definition_span,
                "two call occurrences share one specialization slot",
            ));
        }
        slot_seen.push(occurrence.definition.0);
        let module = subject
            .source_dependencies
            .get(occurrence.module_index)
            .ok_or_else(|| invalid(occurrence.definition_span, "callee module is not retained"))?;
        let identity = callee_identity(occurrence, module);
        if !distinct.iter().any(|item| item == &identity) {
            if distinct.len() >= MAX_DISTINCT_CALLEES {
                return Err(budget(occurrence.definition_span));
            }
            distinct.push(identity);
        }
    }
    authenticate_occurrences(goal, &occurrences, meter.limits)?;
    // Every accepted definition is the caller or one traced occurrence.
    let mut declaration_at = 0;
    while declaration_at < accepted.definitions.len() {
        let definition = accepted.definitions[declaration_at].definition;
        if definition != selected.definition
            && !occurrences
                .iter()
                .any(|occurrence| occurrence.definition == definition)
        {
            return Err(invalid(
                span,
                "accepted revision-3 graph holds an untraced specialization",
            ));
        }
        declaration_at += 1;
    }
    // The caller's own derivation of each occurrence must have exactly the
    // callee contract's complete ordered interface; a suffix or reordering is
    // not accepted (Decision 3, call step 1).
    Ok(SubjectView {
        inputs,
        outputs,
        input_codes,
        output_codes,
        selected,
        checked,
        occurrences,
    })
}

#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the checker's stages are kept in one linear dispatcher so the design's stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the revision-3 contract suite"
)]
pub fn verify(
    goal: &ContractGoal,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    term: &Form,
    depth: u32,
    meter: &mut Meter<'_>,
) -> Result<Envelope, Diagnostic> {
    if goal.revision != 3 {
        return Err(invalid(
            goal.contract_span,
            "revision-3 checker received another revision",
        ));
    }
    if depth > MAX_CLOSURE_DEPTH {
        return Err(budget(goal.contract_span));
    }
    let view = subject_view(goal, meter)?;
    let subject = &goal.subject;
    let span = subject.source_span;
    let inputs = view.inputs;
    let outputs = view.outputs;
    let input_codes = view.input_codes;
    let output_codes = view.output_codes;
    let selected = view.selected;
    let checked = view.checked;
    let occurrences = view.occurrences;
    let body = &selected.body.candidate.body;
    // Match proof slices to occurrences.
    let mut slice_bridge: Vec<Option<&Form>> = Vec::with_capacity(occurrences.len());
    let mut slice_reference: Vec<Option<&str>> = Vec::with_capacity(occurrences.len());
    for _ in &occurrences {
        slice_bridge.push(None);
        slice_reference.push(None);
    }
    for slice in slices(term)? {
        let (ordinal, reference, bridge) = call_slice(slice)?;
        let Some(slot) = slice_bridge.get_mut(ordinal) else {
            return Err(invalid(
                slice.span,
                "call slice does not match source order",
            ));
        };
        if slot.is_some() {
            return Err(invalid(slice.span, "repeated call slice"));
        }
        *slot = Some(bridge);
        if let Some(reference_slot) = slice_reference.get_mut(ordinal) {
            *reference_slot = Some(reference);
        }
    }
    // Resolve every callee and compare the frozen identities.
    let mut callees: Vec<Callee<'_>> = Vec::with_capacity(occurrences.len());
    for (ordinal, occurrence) in occurrences.iter().enumerate() {
        let Some(reference) = slice_reference[ordinal] else {
            return Err(invalid(span, "missing call slice"));
        };
        let Some(bridge) = slice_bridge[ordinal] else {
            return Err(invalid(span, "missing call slice"));
        };
        let callee = resolve_callee(reference, dependencies, local, span)?;
        if callee.contract.revision == 2 {
            return Err(unsupported_profile(
                occurrence.definition_span,
                "preuse-unsupported-callee: a revision-2 claim is not reusable here",
            ));
        }
        let module = &subject.source_dependencies[occurrence.module_index];
        if let Some(field) = callee_differs(occurrence, callee.contract, module) {
            let mut detail = String::from(field);
            detail.push_str(" differs from the frozen occurrence");
            return Err(refined(
                occurrence.definition_span,
                "preuse-callee-mismatch",
                &detail,
            ));
        }
        let source_node = occurrence.source_node.unwrap_or(u32::MAX);
        let derivation_index = usize::try_from(source_node).unwrap_or(usize::MAX);
        let derivation = checked
            .derivations
            .get(derivation_index)
            .ok_or_else(|| invalid(span, "call occurrence has no derivation"))?;
        if derivation.interface.stack_in != callee.contract.subject.input_types
            || derivation.interface.stack_out != callee.contract.subject.output_types
            || derivation.interface.effects != noble_kernel::types::EffSet::empty()
        {
            return Err(refined(
                occurrence.definition_span,
                "preuse-callee-mismatch",
                "the caller derivation does not have the callee's complete ordered interface",
            ));
        }
        if let Some(dependency) = callee.dependency {
            if dependency.source != module.full_source {
                return Err(refined(
                    occurrence.definition_span,
                    "preuse-callee-mismatch",
                    "the proof module source differs from the retained callee module",
                ));
            }
        }
        let _ = bridge;
        callees.push(callee);
    }
    // Scan the proof for lemma uses before the symbolic walk, so every bridge
    // sees the same frozen lemma set.
    let mut names: Vec<String> = Vec::with_capacity(MAX_LEMMA_USES);
    lemma_names(term, &mut names)?;
    if names.len() > MAX_LEMMA_USES {
        return Err(budget(term.span));
    }
    let lemmas = resolved_lemmas(&names, dependencies, span, meter)?;
    let mut contracts: Vec<&str> = Vec::with_capacity(callees.len());
    for callee in &callees {
        contracts.push(callee.reference);
    }
    // The symbolic execution over the caller's complete ordered body.
    let mut state = State {
        stack: input_codes
            .iter()
            .enumerate()
            .map(|(index, _)| Term3::Input(index))
            .collect(),
        facts: Vec::with_capacity(MAX_FACTS),
        ghosts: 0,
    };
    let requires = assertion(&goal.requires, &inputs, &outputs, meter)?;
    if prop_mentions_output(&requires) {
        return Err(unsupported_profile(
            goal.requires.span,
            "caller precondition mentions an output",
        ));
    }
    add_fact(&mut state.facts, &requires)?;
    let mut records: Vec<CallRecord> = Vec::with_capacity(occurrences.len());
    let mut plan: Vec<&ProofDependency> = Vec::with_capacity(callees.len());
    let mut local_plan: Vec<&ProofObligation> = Vec::with_capacity(callees.len());
    let context = WalkContext {
        nodes: &selected.body.candidate.nodes,
        occurrences: &occurrences,
        modules: &subject.source_dependencies,
        callees: &callees,
        bridges: &slice_bridge,
        contracts: &contracts,
        lemmas: &lemmas,
        local,
        inputs: &inputs,
        outputs: &outputs,
        span,
    };
    let mut failure = Ok(());
    let mut body_at = 0;
    while failure.is_ok() && body_at < body.len() {
        failure = walk_node(
            &mut state,
            &context,
            body,
            body_at,
            &mut records,
            &mut plan,
            &mut local_plan,
            meter,
        );
        body_at += 1;
    }
    failure?;
    if state.stack.len() != output_codes.len()
        || state
            .stack
            .iter()
            .zip(&output_codes)
            .any(|(term, code)| sort_of(term, &inputs, &outputs, &[]) != Some(*code))
    {
        return Err(invalid(
            span,
            "final symbolic stack differs from the caller outputs",
        ));
    }
    let ensures = assertion(&goal.ensures, &inputs, &outputs, meter)?;
    let final_inputs: Vec<Term3> = input_codes
        .iter()
        .enumerate()
        .map(|(index, _)| Term3::Input(index))
        .collect();
    let join_goal = normalize_prop(&instantiate_prop(&ensures, &final_inputs, &state.stack));
    let assignment = make_scope(&inputs, &outputs, &[], true);
    let proof_scope = ProofScope {
        facts: &state.facts,
        lemmas: &lemmas,
        contracts: &contracts,
        assignment: &assignment,
    };
    let join = join_target(term)?;
    check_proof(join, &join_goal, &proof_scope, meter)
        .map_err(|problem| refined(join.span, "preuse-join-unproved", &problem.message))?;
    // Isolated replay of the complete finite closure.
    let mut replayed: Vec<String> = Vec::with_capacity(MAX_CLOSURE);
    let mut closure: Vec<String> = Vec::with_capacity(MAX_CLOSURE);
    let next_depth = depth.checked_add(1).ok_or_else(|| budget(span))?;
    replay(
        local,
        &plan,
        &local_plan,
        &mut replayed,
        &mut closure,
        next_depth,
        meter,
    )?;
    let mut used_lemmas: Vec<String> = Vec::with_capacity(names.len());
    for name in &names {
        if !used_lemmas.iter().any(|item| item == name) {
            used_lemmas.push(name.clone());
        }
    }
    Ok(Envelope {
        module: subject.module.clone(),
        version: subject.version,
        definition: subject.definition.clone(),
        contract: goal.contract_name.clone(),
        occurrences: records,
        closure,
        lemmas: used_lemmas,
    })
}

/// The fixed inputs of one caller body walk.
struct WalkContext<'a> {
    nodes: &'a [noble_kernel::untrusted::Node],
    occurrences: &'a [&'a NamedUseOrigin],
    modules: &'a [super::ResolvedSourceModule],
    callees: &'a [Callee<'a>],
    bridges: &'a [Option<&'a Form>],
    contracts: &'a [&'a str],
    lemmas: &'a [Lemma],
    local: &'a [ProofObligation],
    inputs: &'a [(String, Sort3)],
    outputs: &'a [(String, Sort3)],
    span: Span,
}

/// Walk one caller body position over the symbolic state.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; one body step keeps the literal/builtin/call dispatch and its record bookkeeping together"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the record and replay-plan lists are positional by role at the single call site"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the symbolic state, records and replay plans are the checker's own state, advanced one node per step"
)]
fn walk_node<'a>(
    state: &mut State,
    context: &WalkContext<'a>,
    body: &[noble_kernel::untrusted::NodeId],
    at: usize,
    records: &mut Vec<CallRecord>,
    plan: &mut Vec<&'a ProofDependency>,
    local_plan: &mut Vec<&'a ProofObligation>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let span = context.span;
    let node_id = *body
        .get(at)
        .ok_or_else(|| invalid(span, "revision-3 body position is absent"))?;
    let index = usize::try_from(node_id.0)
        .map_err(|_| invalid(span, "revision-3 node id exceeds the address space"))?;
    let node = context
        .nodes
        .get(index)
        .ok_or_else(|| invalid(span, "revision-3 body node is absent"))?;
    let occurrence = context
        .occurrences
        .iter()
        .position(|origin| origin.candidate_node == node_id);
    let Some(step) = classify(node, occurrence) else {
        return Err(unsupported_profile(
            span,
            "revision-3 body node is outside the pure vocabulary",
        ));
    };
    match step {
        Step::Literal(noble_kernel::untrusted::Lit::I64(value)) => {
            push(&mut state.stack, Term3::I64(value), span)?;
        }
        Step::Literal(noble_kernel::untrusted::Lit::Bool(value)) => {
            push(&mut state.stack, Term3::Bool(value), span)?;
        }
        Step::Literal(_) => {
            return Err(unsupported_profile(
                span,
                "revision-3 literal is not I64 or Bool",
            ));
        }
        Step::Builtin(word) => builtin_step(&mut state.stack, word, span)?,
        Step::Call(index) => {
            let occurrence = *context
                .occurrences
                .get(index)
                .ok_or_else(|| invalid(span, "call occurrence has no resolved callee"))?;
            let callee = context
                .callees
                .get(index)
                .ok_or_else(|| invalid(span, "call occurrence has no resolved contract"))?;
            let bridge = context
                .bridges
                .get(index)
                .and_then(|bridge| *bridge)
                .ok_or_else(|| invalid(span, "call occurrence has no bridge"))?;
            call_step(
                state,
                callee,
                bridge,
                context.contracts,
                context.lemmas,
                context.inputs,
                context.outputs,
                span,
                meter,
            )?;
            let callee_inputs =
                interface(&callee.contract.inputs, callee.contract.contract_span)?;
            let callee_outputs =
                interface(&callee.contract.outputs, callee.contract.contract_span)?;
            let mut inputs = Vec::with_capacity(callee_inputs.len());
            for (name, code) in &callee_inputs {
                inputs.push((name.clone(), code.word()));
            }
            let mut outputs = Vec::with_capacity(callee_outputs.len());
            for (name, code) in &callee_outputs {
                outputs.push((name.clone(), code.word()));
            }
            let modules = match context.modules.get(occurrence.module_index) {
                Some(module) => module,
                None => return Err(invalid(span, "callee module is not retained")),
            };
            records.push(CallRecord {
                ordinal: u32::try_from(index).unwrap_or(u32::MAX),
                source_span: occurrence.source_span.unwrap_or(span),
                callee_module: modules.module.clone(),
                callee_version: modules.version,
                callee_definition: occurrence.definition_name.clone(),
                callee_ordinal: occurrence.definition_ordinal,
                callee_span: occurrence.definition_span,
                specialization: occurrence.definition.0,
                proof: String::from(callee.reference),
                inputs,
                outputs,
            });
            if let Some(dependency) = callee.dependency {
                let identity = replay_identity(dependency);
                if !plan
                    .iter()
                    .any(|item| replay_identity(item) == identity)
                {
                    plan.push(dependency);
                }
            } else {
                let local = context
                    .local
                    .iter()
                    .find(|item| item.name == callee.reference)
                    .ok_or_else(|| {
                        invalid(span, "local callee proof is no longer staged")
                    })?;
                if !local_plan.iter().any(|item| item.name == local.name) {
                    local_plan.push(local);
                }
            }
        }
    }
    Ok(())
}

/// Build the complete revision-3 preparation of one contract.
pub fn prepare_named_v3(
    goal: &ContractGoal,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    term: &Form,
    limits: Limits,
) -> Result<PreparedV3, Diagnostic> {
    let tree_bounds = TreeBounds::new(limits).ok_or_else(|| {
        Diagnostic::new(
            DiagnosticKind::Exhausted,
            goal.contract_span,
            "revision-3 bounds exceed the host",
        )
    })?;
    let config = MeterConfig {
        budgets: ProofBudgets::from_limits(limits),
        limits,
        tree_bounds,
    };
    let mut meter = Meter {
        nodes: 0,
        work: 0,
        normalization: 0,
        substitution: 0,
        config: &config,
    };
    let envelope = verify(goal, dependencies, local, term, 0, &mut meter)?;
    Ok(PreparedV3 {
        claim: claim_text(goal),
        envelope,
    })
}

/// The contract-time check: the subject, its independent kernel acceptance and
/// its authenticated ordered occurrences. The proof's bridges, join and
/// closure are checked only with its proof term, so a contract alone is never
/// reported proved.
pub fn claim_shape(goal: &ContractGoal, limits: Limits) -> Result<String, Diagnostic> {
    let tree_bounds = TreeBounds::new(limits).ok_or_else(|| {
        Diagnostic::new(
            DiagnosticKind::Exhausted,
            goal.contract_span,
            "revision-3 bounds exceed the host",
        )
    })?;
    let config = MeterConfig {
        budgets: ProofBudgets::from_limits(limits),
        limits,
        tree_bounds,
    };
    let mut meter = Meter {
        nodes: 0,
        work: 0,
        normalization: 0,
        substitution: 0,
        config: &config,
    };
    subject_view(goal, &mut meter)?;
    Ok(claim_text(goal))
}

pub(super) fn check_contract<'m>(
    obligation: &ProofObligation,
    goal: &ContractGoal,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    mut meter: Meter<'m>,
) -> Transition<'m, CheckedProof> {
    let result = checked(obligation, goal, dependencies, local, &mut meter);
    (meter, result)
}

fn checked(
    obligation: &ProofObligation,
    goal: &ContractGoal,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    meter: &mut Meter<'_>,
) -> Result<CheckedProof, Diagnostic> {
    let envelope = verify(goal, dependencies, local, &obligation.term, 0, meter)?;
    let mut term = String::from("revision-3 envelope\n");
    term.push_str(&claim_text(goal));
    term.push_str("\noccurrences ");
    crate::rendering::number(
        &mut term,
        u64::try_from(envelope.occurrences.len()).unwrap_or(u64::MAX),
    );
    Ok(CheckedProof {
        name: obligation.name.clone(),
        kind: ProofKind::NamedV3Contract,
        lean_term: term,
        claim: String::from("NamedV3Obligation.claim"),
        model_revision: String::new(),
        checker_revision: String::new(),
    })
}
