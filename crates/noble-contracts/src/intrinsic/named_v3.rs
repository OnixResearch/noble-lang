//! The opt-in revision-3 reusable named-call profile: exact envelope
//! reconstruction, a deterministic symbolic checker over the bounded
//! `I64`/`Bool` vocabulary, a bounded scalar-term equality, isolated
//! dependency replay, and closed `NamedV3Lifted` Lean lowering.
//! Revisions 1 and 2 keep their own modules and meanings.

mod lower;

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use super::{
    atom, head_atom, list, operation, reject, unsupported, CheckedProof, ContractGoal, Form,
    FormKind, Meter, MeterConfig, NamedUseOrigin, PendingGoal, ProofBudgets, ProofDependency,
    ProofKind, ProofObligation, Transition, TreeBounds,
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

/// A Decision 6 diagnostic: a reason code followed by bounded detail text that
/// never claims the target proposition is false.
struct Reason<'a> {
    code: &'a str,
    detail: &'a str,
}

fn refined(span: Span, reason: Reason<'_>) -> Diagnostic {
    let mut message = String::from(reason.code);
    message.push_str(": ");
    message.push_str(reason.detail);
    Diagnostic::new(DiagnosticKind::Invalid, span, &message)
}

/// One Type0 sort the profile admits.
#[derive(Clone, Copy, PartialEq, Eq)]
#[octet::sealed_enum]
enum Sort3 {
    I64,
    Bool,
}

impl Sort3 {
    const fn from_word(word: &str) -> Option<Self> {
        if str_eq(word, "I64") {
            Some(Self::I64)
        } else if str_eq(word, "Bool") {
            Some(Self::Bool)
        } else {
            None
        }
    }

    const fn from_ty(ty: &noble_kernel::types::Ty) -> Option<Self> {
        match ty {
            noble_kernel::types::Ty::I64 => Some(Self::I64),
            noble_kernel::types::Ty::Bool => Some(Self::Bool),
            noble_kernel::types::Ty::Unit
            | noble_kernel::types::Ty::Text
            | noble_kernel::types::Ty::Syntax
            | noble_kernel::types::Ty::Contract
            | noble_kernel::types::Ty::Evidence
            | noble_kernel::types::Ty::Certified
            | noble_kernel::types::Ty::Pair(..)
            | noble_kernel::types::Ty::Sum(..)
            | noble_kernel::types::Ty::List(..)
            | noble_kernel::types::Ty::Program(..)
            | noble_kernel::types::Ty::LiveRef(..)
            | noble_kernel::types::Ty::Resource(..)
            | noble_kernel::types::Ty::Nominal(..)
            | noble_kernel::types::Ty::GenericNominal(..) => None,
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
#[octet::sealed_enum]
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
#[octet::sealed_enum]
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
#[octet::sealed_enum]
enum Statement3 {
    Prop(Prop3),
    Pi {
        name: String,
        code: Sort3,
        body: Box<Statement3>,
    },
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; constant byte-wise equality check for Aeneas extraction"
)]
const fn str_eq(text: &str, expected: &str) -> bool {
    let a = text.as_bytes();
    let b = expected.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut is_same = true;
    let mut i = 0usize;
    while is_same && i < a.len() {
        if a[i] != b[i] {
            is_same = false;
        }
        i += 1;
    }
    is_same
}

/// The name-resolution scope of one assertion, motive, or lemma statement.
#[derive(Clone, Copy)]
struct Scope<'a> {
    inputs: &'a [(String, Sort3)],
    outputs: &'a [(String, Sort3)],
    bound: &'a [(String, Sort3)],
    /// Lemma statements may not mention a program interface.
    interface: bool,
    /// Lemma statements and `subst` motives accept `(Eq Sort left right)` as well as `(eq left right)`.
    allow_typed_eq: bool,
}

impl Scope<'_> {
    fn sort(&self, term: &Term3) -> Option<Sort3> {
        sort_of(term, self.inputs, self.outputs, self.bound)
    }
}

#[octet::sealed_enum]
#[derive(Clone, Copy)]
enum ScopeMode {
    Assertion,
    Motive,
    Lemma,
}

const fn make_scope<'a>(
    inputs: &'a [(String, Sort3)],
    outputs: &'a [(String, Sort3)],
    bound: &'a [(String, Sort3)],
    mode: ScopeMode,
) -> Scope<'a> {
    let (interface, allow_typed_eq) = match mode {
        ScopeMode::Assertion => (true, false),
        ScopeMode::Motive => (true, true),
        ScopeMode::Lemma => (false, true),
    };
    Scope {
        inputs,
        outputs,
        bound,
        interface,
        allow_typed_eq,
    }
}

#[octet::sealed_enum]
enum SortStep {
    Visit(Term3),
    CombineArith,
    CombineEq,
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes work items and resolved sorts onto caller stacks"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates iterative sort work and value stacks"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; Vec::push/pop are not const"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; bounded iterative sort check keeps scope slices and work/value stacks explicit"
)]
fn sort_step(
    step: SortStep,
    inputs: &[(String, Sort3)],
    outputs: &[(String, Sort3)],
    bound: &[(String, Sort3)],
    work: &mut Vec<SortStep>,
    values: &mut Vec<Sort3>,
) -> bool {
    match step {
        SortStep::Visit(node) => match node {
            Term3::Input(index) => match inputs.get(index) {
                Some((_, code)) => {
                    values.push(*code);
                    true
                }
                None => false,
            },
            Term3::Output(index) => match outputs.get(index) {
                Some((_, code)) => {
                    values.push(*code);
                    true
                }
                None => false,
            },
            Term3::Bound(index) => match bound.get(index) {
                Some((_, code)) => {
                    values.push(*code);
                    true
                }
                None => false,
            },
            Term3::Ghost(_, _, code) => {
                values.push(code);
                true
            }
            Term3::I64(_) => {
                values.push(Sort3::I64);
                true
            }
            Term3::Bool(_) => {
                values.push(Sort3::Bool);
                true
            }
            Term3::Add(left, right) | Term3::Sub(left, right) | Term3::Mul(left, right) => {
                work.push(SortStep::CombineArith);
                work.push(SortStep::Visit(*right));
                work.push(SortStep::Visit(*left));
                true
            }
            Term3::Equals(left, right) => {
                work.push(SortStep::CombineEq);
                work.push(SortStep::Visit(*right));
                work.push(SortStep::Visit(*left));
                true
            }
        },
        SortStep::CombineArith => match (values.pop(), values.pop()) {
            (Some(Sort3::I64), Some(Sort3::I64)) => {
                values.push(Sort3::I64);
                true
            }
            _ => false,
        },
        SortStep::CombineEq => match (values.pop(), values.pop()) {
            (Some(right), Some(left)) if left == right => {
                values.push(Sort3::Bool);
                true
            }
            _ => false,
        },
    }
}

/// The sort of a term under one scope, checked iteratively without recursion.
fn sort_of(
    term: &Term3,
    inputs: &[(String, Sort3)],
    outputs: &[(String, Sort3)],
    bound: &[(String, Sort3)],
) -> Option<Sort3> {
    let mut work = Vec::new();
    work.push(SortStep::Visit(term.clone()));
    let mut values: Vec<Sort3> = Vec::new();
    let mut is_ok = true;
    while is_ok {
        let Some(step) = work.pop() else {
            break;
        };
        is_ok = sort_step(step, inputs, outputs, bound, &mut work, &mut values);
    }
    if is_ok && values.len() == 1 {
        values.pop()
    } else {
        None
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; bounded iterative tree predicate"
)]
fn mentions_output(term: &Term3) -> bool {
    let mut pending = Vec::from([term]);
    while let Some(node) = pending.pop() {
        match node {
            Term3::Output(_) => return true,
            Term3::Add(left, right)
            | Term3::Sub(left, right)
            | Term3::Mul(left, right)
            | Term3::Equals(left, right) => {
                pending.push(right);
                pending.push(left);
            }
            Term3::Input(_)
            | Term3::Bound(_)
            | Term3::Ghost(..)
            | Term3::I64(_)
            | Term3::Bool(_) => {}
        }
    }
    false
}

fn prop_mentions_output(prop: &Prop3) -> bool {
    let mut pending = Vec::from([prop]);
    while let Some(node) = pending.pop() {
        match node {
            Prop3::True | Prop3::False => {}
            Prop3::Not(body) => pending.push(body),
            Prop3::And(left, right) => {
                pending.push(right);
                pending.push(left);
            }
            Prop3::Eq(left, right) | Prop3::Lt(left, right) | Prop3::Le(left, right) => {
                if mentions_output(left) || mentions_output(right) {
                    return true;
                }
            }
        }
    }
    false
}

#[derive(Clone, Copy)]
#[octet::sealed_enum]
enum BinTermOp {
    Add,
    Sub,
    Mul,
    Equals,
}

#[octet::sealed_enum]
enum NormTermStep {
    Visit(Term3),
    Combine(BinTermOp),
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; updates caller workstack and output vector in place"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; advances bounded normalization workstack and output stack"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; Vec::push/pop are not const"
)]
fn norm_term_step(step: NormTermStep, work: &mut Vec<NormTermStep>, out: &mut Vec<Term3>) {
    match step {
        NormTermStep::Visit(node) => match node {
            Term3::Add(left, right) => {
                work.push(NormTermStep::Combine(BinTermOp::Add));
                work.push(NormTermStep::Visit(*right));
                work.push(NormTermStep::Visit(*left));
            }
            Term3::Sub(left, right) => {
                work.push(NormTermStep::Combine(BinTermOp::Sub));
                work.push(NormTermStep::Visit(*right));
                work.push(NormTermStep::Visit(*left));
            }
            Term3::Mul(left, right) => {
                work.push(NormTermStep::Combine(BinTermOp::Mul));
                work.push(NormTermStep::Visit(*right));
                work.push(NormTermStep::Visit(*left));
            }
            Term3::Equals(left, right) => {
                work.push(NormTermStep::Combine(BinTermOp::Equals));
                work.push(NormTermStep::Visit(*right));
                work.push(NormTermStep::Visit(*left));
            }
            Term3::Input(_)
            | Term3::Output(_)
            | Term3::Bound(_)
            | Term3::Ghost(..)
            | Term3::I64(_)
            | Term3::Bool(_) => out.push(node),
        },
        NormTermStep::Combine(op) => {
            if let (Some(right), Some(left)) = (out.pop(), out.pop()) {
                out.push(fold_binary_term(op, left, right));
            }
        }
    }
}

/// Fold closed literal arithmetic iteratively.
fn normalize_term(term: &Term3) -> Term3 {
    let mut work = Vec::new();
    work.push(NormTermStep::Visit(term.clone()));
    let mut out: Vec<Term3> = Vec::new();
    while let Some(step) = work.pop() {
        norm_term_step(step, &mut work, &mut out);
    }
    out.pop().unwrap_or(Term3::Bool(false))
}

fn fold_binary_term(op: BinTermOp, left: Term3, right: Term3) -> Term3 {
    match op {
        BinTermOp::Add => match (&left, &right) {
            (Term3::I64(a), Term3::I64(b)) => Term3::I64(a.wrapping_add(*b)),
            _ => Term3::Add(Box::new(left), Box::new(right)),
        },
        BinTermOp::Sub => match (&left, &right) {
            (Term3::I64(a), Term3::I64(b)) => Term3::I64(a.wrapping_sub(*b)),
            _ => Term3::Sub(Box::new(left), Box::new(right)),
        },
        BinTermOp::Mul => match (&left, &right) {
            (Term3::I64(a), Term3::I64(b)) => Term3::I64(a.wrapping_mul(*b)),
            _ => Term3::Mul(Box::new(left), Box::new(right)),
        },
        BinTermOp::Equals => match (&left, &right) {
            (Term3::I64(a), Term3::I64(b)) => Term3::Bool(a == b),
            (Term3::Bool(a), Term3::Bool(b)) => Term3::Bool(a == b),
            _ => Term3::Equals(Box::new(left), Box::new(right)),
        },
    }
}

#[octet::sealed_enum]
enum NormPropStep {
    Visit(Prop3),
    CombineNot,
    CombineAnd,
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; updates caller workstack and output vector in place"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; advances bounded proposition normalization workstack"
)]
fn norm_prop_step(step: NormPropStep, work: &mut Vec<NormPropStep>, out: &mut Vec<Prop3>) {
    match step {
        NormPropStep::Visit(node) => match node {
            Prop3::True | Prop3::False => out.push(node),
            Prop3::Not(body) => {
                work.push(NormPropStep::CombineNot);
                work.push(NormPropStep::Visit(*body));
            }
            Prop3::And(left, right) => {
                work.push(NormPropStep::CombineAnd);
                work.push(NormPropStep::Visit(*right));
                work.push(NormPropStep::Visit(*left));
            }
            Prop3::Eq(left, right) => {
                out.push(fold_eq_prop(normalize_term(&left), normalize_term(&right)));
            }
            Prop3::Lt(left, right) => {
                out.push(fold_order_prop(
                    normalize_term(&left),
                    normalize_term(&right),
                    true,
                ));
            }
            Prop3::Le(left, right) => {
                out.push(fold_order_prop(
                    normalize_term(&left),
                    normalize_term(&right),
                    false,
                ));
            }
        },
        NormPropStep::CombineNot => {
            if let Some(body) = out.pop() {
                out.push(match body {
                    Prop3::True => Prop3::False,
                    Prop3::False => Prop3::True,
                    Prop3::Not(inner) => *inner,
                    other => Prop3::Not(Box::new(other)),
                });
            }
        }
        NormPropStep::CombineAnd => {
            if let (Some(right), Some(left)) = (out.pop(), out.pop()) {
                out.push(match (&left, &right) {
                    (Prop3::False, _) | (_, Prop3::False) => Prop3::False,
                    (Prop3::True, _) => right,
                    (_, Prop3::True) => left,
                    _ => Prop3::And(Box::new(left), Box::new(right)),
                });
            }
        }
    }
}

fn normalize_prop(prop: &Prop3) -> Prop3 {
    let mut work = Vec::new();
    work.push(NormPropStep::Visit(prop.clone()));
    let mut out: Vec<Prop3> = Vec::new();
    while let Some(step) = work.pop() {
        norm_prop_step(step, &mut work, &mut out);
    }
    out.pop().unwrap_or(Prop3::False)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; `PartialEq` is not const on the pinned compiler"
)]
fn fold_eq_prop(left: Term3, right: Term3) -> Prop3 {
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

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; `Term3` owns boxed allocations so `fold_order_prop` cannot be `const`"
)]
fn fold_order_prop(left: Term3, right: Term3, is_strict: bool) -> Prop3 {
    match (&left, &right) {
        (Term3::I64(a), Term3::I64(b)) => {
            let is_true = if is_strict { *a < *b } else { *a <= *b };
            if is_true {
                Prop3::True
            } else {
                Prop3::False
            }
        }
        _ => {
            if is_strict {
                Prop3::Lt(left, right)
            } else {
                Prop3::Le(left, right)
            }
        }
    }
}

#[octet::sealed_enum]
enum MapTermStep {
    Visit(Term3),
    Build(BinTermOp),
}

#[derive(Clone, Copy)]
#[octet::sealed_enum]
enum TermSubst<'a> {
    Interface {
        inputs: &'a [Term3],
        outputs: &'a [Term3],
    },
    Binder {
        level: usize,
        argument: &'a Term3,
    },
}

struct BoundLookup {
    index: usize,
    level: usize,
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; clones `argument: &Term3` when binder index matches"
)]
fn map_bound_term(lookup: BoundLookup, argument: &Term3) -> Term3 {
    if lookup.index == lookup.level {
        argument.clone()
    } else if lookup.index > lookup.level {
        Term3::Bound(lookup.index.saturating_sub(1))
    } else {
        Term3::Bound(lookup.index)
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; updates caller workstack and output vector in place"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; advances bounded term substitution workstack"
)]
fn map_term_step(
    step: MapTermStep,
    mode: TermSubst<'_>,
    work: &mut Vec<MapTermStep>,
    out: &mut Vec<Term3>,
) {
    match step {
        MapTermStep::Visit(node) => match node {
            Term3::Input(index) => match mode {
                TermSubst::Interface { inputs, .. } => {
                    out.push(inputs.get(index).cloned().unwrap_or(node));
                }
                TermSubst::Binder { .. } => out.push(node),
            },
            Term3::Output(index) => match mode {
                TermSubst::Interface { outputs, .. } => {
                    out.push(outputs.get(index).cloned().unwrap_or(node));
                }
                TermSubst::Binder { .. } => out.push(node),
            },
            Term3::Bound(index) => match mode {
                TermSubst::Interface { .. } => out.push(node),
                TermSubst::Binder { level, argument } => {
                    out.push(map_bound_term(BoundLookup { index, level }, argument));
                }
            },
            Term3::Ghost(..) | Term3::I64(_) | Term3::Bool(_) => out.push(node),
            Term3::Add(left, right) => {
                work.push(MapTermStep::Build(BinTermOp::Add));
                work.push(MapTermStep::Visit(*right));
                work.push(MapTermStep::Visit(*left));
            }
            Term3::Sub(left, right) => {
                work.push(MapTermStep::Build(BinTermOp::Sub));
                work.push(MapTermStep::Visit(*right));
                work.push(MapTermStep::Visit(*left));
            }
            Term3::Mul(left, right) => {
                work.push(MapTermStep::Build(BinTermOp::Mul));
                work.push(MapTermStep::Visit(*right));
                work.push(MapTermStep::Visit(*left));
            }
            Term3::Equals(left, right) => {
                work.push(MapTermStep::Build(BinTermOp::Equals));
                work.push(MapTermStep::Visit(*right));
                work.push(MapTermStep::Visit(*left));
            }
        },
        MapTermStep::Build(op) => {
            if let (Some(right), Some(left)) = (out.pop(), out.pop()) {
                let pair = (Box::new(left), Box::new(right));
                out.push(match op {
                    BinTermOp::Add => Term3::Add(pair.0, pair.1),
                    BinTermOp::Sub => Term3::Sub(pair.0, pair.1),
                    BinTermOp::Mul => Term3::Mul(pair.0, pair.1),
                    BinTermOp::Equals => Term3::Equals(pair.0, pair.1),
                });
            }
        }
    }
}

fn map_term(term: &Term3, mode: TermSubst<'_>) -> Term3 {
    let mut work = Vec::new();
    work.push(MapTermStep::Visit(term.clone()));
    let mut out: Vec<Term3> = Vec::new();
    while let Some(step) = work.pop() {
        map_term_step(step, mode, &mut work, &mut out);
    }
    out.pop().unwrap_or(Term3::Bool(false))
}

fn instantiate_term(term: &Term3, inputs: &[Term3], outputs: &[Term3]) -> Term3 {
    map_term(term, TermSubst::Interface { inputs, outputs })
}

#[octet::sealed_enum]
enum MapPropStep {
    Visit(Prop3),
    BuildNot,
    BuildAnd,
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; updates caller workstack and output vector in place"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; advances bounded proposition substitution workstack"
)]
fn map_prop_step(
    step: MapPropStep,
    mode: TermSubst<'_>,
    work: &mut Vec<MapPropStep>,
    out: &mut Vec<Prop3>,
) {
    match step {
        MapPropStep::Visit(node) => match node {
            Prop3::True | Prop3::False => out.push(node),
            Prop3::Not(body) => {
                work.push(MapPropStep::BuildNot);
                work.push(MapPropStep::Visit(*body));
            }
            Prop3::And(left, right) => {
                work.push(MapPropStep::BuildAnd);
                work.push(MapPropStep::Visit(*right));
                work.push(MapPropStep::Visit(*left));
            }
            Prop3::Eq(left, right) => {
                let mapped_left = map_term(&left, mode);
                let mapped_right = map_term(&right, mode);
                out.push(Prop3::Eq(mapped_left, mapped_right));
            }
            Prop3::Lt(left, right) => {
                let mapped_left = map_term(&left, mode);
                let mapped_right = map_term(&right, mode);
                out.push(Prop3::Lt(mapped_left, mapped_right));
            }
            Prop3::Le(left, right) => {
                let mapped_left = map_term(&left, mode);
                let mapped_right = map_term(&right, mode);
                out.push(Prop3::Le(mapped_left, mapped_right));
            }
        },
        MapPropStep::BuildNot => {
            if let Some(body) = out.pop() {
                out.push(Prop3::Not(Box::new(body)));
            }
        }
        MapPropStep::BuildAnd => {
            if let (Some(right), Some(left)) = (out.pop(), out.pop()) {
                out.push(Prop3::And(Box::new(left), Box::new(right)));
            }
        }
    }
}

fn map_prop(prop: &Prop3, mode: TermSubst<'_>) -> Prop3 {
    let mut work = Vec::new();
    work.push(MapPropStep::Visit(prop.clone()));
    let mut out: Vec<Prop3> = Vec::new();
    while let Some(step) = work.pop() {
        map_prop_step(step, mode, &mut work, &mut out);
    }
    out.pop().unwrap_or(Prop3::False)
}

fn instantiate_prop(prop: &Prop3, inputs: &[Term3], outputs: &[Term3]) -> Prop3 {
    map_prop(prop, TermSubst::Interface { inputs, outputs })
}

fn replace_prop(prop: &Prop3, level: usize, argument: &Term3) -> Prop3 {
    map_prop(prop, TermSubst::Binder { level, argument })
}

fn find_binder_index(bindings: &[(String, Sort3)], name: &str) -> Option<usize> {
    let mut index = 0;
    while index < bindings.len() {
        if bindings[index].0 == name {
            return Some(index);
        }
        index += 1;
    }
    None
}

#[octet::sealed_enum]
enum ParseTermFrame {
    Visit(Form, u32),
    Build(BinTermOp),
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; updates caller parse workstack and output vector"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; advances bounded scalar term parsing worklist and budget"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; keeps worklist, output stack, budgets, and root span explicit"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid scalar terms return typed Diagnostic"
)]
fn parse_term_step(
    frame: ParseTermFrame,
    scope: Scope<'_>,
    work: &mut Vec<ParseTermFrame>,
    out: &mut Vec<Term3>,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
    root_span: Span,
) -> Result<(), Diagnostic> {
    match frame {
        ParseTermFrame::Visit(node, cur_depth) => {
            spend!(remaining, meter, cur_depth, node.span);
            match &node.kind {
                FormKind::Atom(text) => out.push(parse_term_atom(text, node.span, scope)?),
                FormKind::List(items) => {
                    let Some(head) = head_atom(items) else {
                        return Err(invalid(node.span, "expected bounded scalar term"));
                    };
                    if let Some(indexed) = parse_io_binder(head, items, scope)? {
                        out.push(indexed);
                    } else {
                        let op = parse_binary_term_op(head, items, node.span)?;
                        let Some(next) = cur_depth.checked_add(1) else {
                            return Err(budget(node.span));
                        };
                        work.push(ParseTermFrame::Build(op));
                        work.push(ParseTermFrame::Visit(items[2].clone(), next));
                        work.push(ParseTermFrame::Visit(items[1].clone(), next));
                    }
                }
            }
        }
        ParseTermFrame::Build(op) => {
            let (Some(right), Some(left)) = (out.pop(), out.pop()) else {
                return Err(invalid(root_span, "expected bounded scalar term"));
            };
            let pair = (Box::new(left), Box::new(right));
            out.push(match op {
                BinTermOp::Add => Term3::Add(pair.0, pair.1),
                BinTermOp::Sub => Term3::Sub(pair.0, pair.1),
                BinTermOp::Mul => Term3::Mul(pair.0, pair.1),
                BinTermOp::Equals => Term3::Equals(pair.0, pair.1),
            });
        }
    }
    Ok(())
}

/// Parse one bounded scalar term iteratively, checking `bound` before `inputs`/`outputs`.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; charges bounded term node budget and meter"
)]
fn parse_term(
    form: &Form,
    scope: Scope<'_>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Term3, Diagnostic> {
    let mut work = Vec::new();
    work.push(ParseTermFrame::Visit(form.clone(), depth));
    let mut out: Vec<Term3> = Vec::new();
    let mut outcome = Ok(());
    while let Some(frame) = work.pop() {
        outcome = parse_term_step(
            frame, scope, &mut work, &mut out, remaining, meter, form.span,
        );
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    let Some(res) = out.pop() else {
        return Err(invalid(form.span, "expected bounded scalar term"));
    };
    Ok(res)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; name comparisons and integer parsing are not const"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; returns typed Diagnostic on unknown scalar atom"
)]
fn parse_term_atom(text: &str, span: Span, scope: Scope<'_>) -> Result<Term3, Diagnostic> {
    if let Some(index) = find_binder_index(scope.bound, text) {
        return Ok(Term3::Bound(index));
    }
    if let Some(index) = find_binder_index(scope.inputs, text) {
        return Ok(Term3::Input(index));
    }
    if let Some(index) = find_binder_index(scope.outputs, text) {
        return Ok(Term3::Output(index));
    }
    if text == "true" {
        return Ok(Term3::Bool(true));
    }
    if text == "false" {
        return Ok(Term3::Bool(false));
    }
    match text.parse::<i64>() {
        Ok(value) => Ok(Term3::I64(value)),
        Err(_) => Err(invalid(span, "expected bounded scalar term")),
    }
}

fn parse_io_binder(
    head: &str,
    items: &[Form],
    scope: Scope<'_>,
) -> Result<Option<Term3>, Diagnostic> {
    if items.len() == 2 && head == "in" && scope.interface {
        let name = atom(&items[1])?;
        let Some(index) = find_binder_index(scope.inputs, name) else {
            return Err(invalid(items[1].span, "unbound input binder"));
        };
        return Ok(Some(Term3::Input(index)));
    }
    if items.len() == 2 && head == "out" && scope.interface {
        let name = atom(&items[1])?;
        let Some(index) = find_binder_index(scope.outputs, name) else {
            return Err(invalid(items[1].span, "unbound output binder"));
        };
        return Ok(Some(Term3::Output(index)));
    }
    Ok(None)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; head string comparisons are not const"
)]
fn parse_binary_term_op(head: &str, items: &[Form], span: Span) -> Result<BinTermOp, Diagnostic> {
    if items.len() != 3 {
        return Err(invalid(span, "expected bounded scalar term"));
    }
    match head {
        "add" => Ok(BinTermOp::Add),
        "sub" => Ok(BinTermOp::Sub),
        "mul" => Ok(BinTermOp::Mul),
        "eq" => Ok(BinTermOp::Equals),
        _ => Err(invalid(span, "expected bounded scalar term")),
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; `scope.sort` and `invalid` are not const"
)]
fn require_i64(term: &Term3, scope: Scope<'_>, span: Span) -> Result<(), Diagnostic> {
    if scope.sort(term) == Some(Sort3::I64) {
        Ok(())
    } else {
        Err(invalid(span, "expected I64 scalar term"))
    }
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; `scope.sort` and `invalid` are not const"
)]
fn require_same_sort(
    left: &Term3,
    right: &Term3,
    scope: Scope<'_>,
    span: Span,
) -> Result<(), Diagnostic> {
    let left_sort = scope.sort(left);
    if left_sort.is_some() && left_sort == scope.sort(right) {
        Ok(())
    } else {
        Err(invalid(span, "equality operands have different sorts"))
    }
}

#[octet::sealed_enum]
enum ParsePropFrame {
    Visit(Form, u32),
    BuildNot,
    BuildAnd,
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; updates caller parse workstack and output vector"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; advances bounded proposition parsing worklist and budget"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; bounded iterative step keeps proposition parsing state explicit"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid propositions are rejected through typed diagnostics"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; keeps the connective dispatch in one bounded step"
)]
fn parse_prop_step(
    frame: ParsePropFrame,
    scope: Scope<'_>,
    work: &mut Vec<ParsePropFrame>,
    out: &mut Vec<Prop3>,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
    root_span: Span,
) -> Result<(), Diagnostic> {
    match frame {
        ParsePropFrame::Visit(node, cur_depth) => {
            spend!(remaining, meter, cur_depth, node.span);
            if let FormKind::Atom(word) = &node.kind {
                if word == "true" {
                    out.push(Prop3::True);
                    return Ok(());
                }
                if word == "false" {
                    out.push(Prop3::False);
                    return Ok(());
                }
            }
            let items = list(&node)?;
            let Some(head) = head_atom(items) else {
                return Err(invalid(node.span, "expected logical proposition"));
            };
            let Some(next) = cur_depth.checked_add(1) else {
                return Err(budget(node.span));
            };
            match head {
                "true" if items.len() == 1 => out.push(Prop3::True),
                "false" if items.len() == 1 => out.push(Prop3::False),
                "not" if items.len() == 2 => {
                    work.push(ParsePropFrame::BuildNot);
                    work.push(ParsePropFrame::Visit(items[1].clone(), next));
                }
                "and" if items.len() == 3 => {
                    work.push(ParsePropFrame::BuildAnd);
                    work.push(ParsePropFrame::Visit(items[2].clone(), next));
                    work.push(ParsePropFrame::Visit(items[1].clone(), next));
                }
                "eq" if items.len() == 3 => {
                    let left = parse_term(&items[1], scope, next, remaining, meter)?;
                    let right = parse_term(&items[2], scope, next, remaining, meter)?;
                    require_same_sort(&left, &right, scope, node.span)?;
                    out.push(Prop3::Eq(left, right));
                }
                "Eq" if scope.allow_typed_eq && items.len() == 4 => {
                    let Some(expected) = Sort3::from_word(atom(&items[1])?) else {
                        return Err(unsupported_profile(
                            items[1].span,
                            "equality sort is not I64 or Bool",
                        ));
                    };
                    let left = parse_term(&items[2], scope, next, remaining, meter)?;
                    let right = parse_term(&items[3], scope, next, remaining, meter)?;
                    if scope.sort(&left) != Some(expected) || scope.sort(&right) != Some(expected) {
                        return Err(invalid(node.span, "typed equality operand sort mismatch"));
                    }
                    out.push(Prop3::Eq(left, right));
                }
                "lt" | "le" if items.len() == 3 => {
                    let left = parse_term(&items[1], scope, next, remaining, meter)?;
                    let right = parse_term(&items[2], scope, next, remaining, meter)?;
                    require_i64(&left, scope, items[1].span)?;
                    require_i64(&right, scope, items[2].span)?;
                    out.push(if str_eq(head, "lt") {
                        Prop3::Lt(left, right)
                    } else {
                        Prop3::Le(left, right)
                    });
                }
                _ => return Err(invalid(node.span, "expected logical proposition")),
            }
        }
        ParsePropFrame::BuildNot => {
            let Some(body) = out.pop() else {
                return Err(invalid(root_span, "expected logical proposition"));
            };
            out.push(Prop3::Not(Box::new(body)));
        }
        ParsePropFrame::BuildAnd => {
            let (Some(right), Some(left)) = (out.pop(), out.pop()) else {
                return Err(invalid(root_span, "expected logical proposition"));
            };
            out.push(Prop3::And(Box::new(left), Box::new(right)));
        }
    }
    Ok(())
}

/// Parse one assertion, motive, or lemma conclusion proposition iteratively.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; charges bounded proposition node budget and meter"
)]
fn parse_prop(
    form: &Form,
    scope: Scope<'_>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Prop3, Diagnostic> {
    let mut work = Vec::new();
    work.push(ParsePropFrame::Visit(form.clone(), depth));
    let mut out: Vec<Prop3> = Vec::new();
    let mut outcome = Ok(());
    while let Some(frame) = work.pop() {
        outcome = parse_prop_step(
            frame, scope, &mut work, &mut out, remaining, meter, form.span,
        );
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    let Some(res) = out.pop() else {
        return Err(invalid(form.span, "expected logical proposition"));
    };
    Ok(res)
}

/// Parse one source assertion with a fresh profile node budget.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; `meter` charges assertion parsing nodes"
)]
fn assertion(
    form: &Form,
    inputs: &[(String, Sort3)],
    outputs: &[(String, Sort3)],
    meter: &mut Meter<'_>,
) -> Result<Prop3, Diagnostic> {
    let scope = make_scope(inputs, outputs, &[], ScopeMode::Assertion);
    let mut remaining = MAX_ASSERTION_NODES;
    parse_prop(form, scope, 0, &mut remaining, meter)
}

/// Parse one lemma statement iteratively: explicit `I64`/`Bool` binders
/// ending in an interface-free proposition (accepting `(Eq Sort l r)`).
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes parsed Pi binder into caller vector"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; appends Pi binder and charges meter"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid Pi binders return typed Diagnostic"
)]
fn parse_statement_pi_step(
    current: &Form,
    binders: &mut Vec<(String, Sort3)>,
    depth: &mut u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Option<Form>, Diagnostic> {
    spend!(remaining, meter, *depth, current.span);
    let FormKind::List(items) = &current.kind else {
        return Ok(None);
    };
    if head_atom(items) != Some("Pi") || items.len() != 3 {
        return Ok(None);
    }
    let binder = list(&items[1])?;
    if binder.len() != 2 {
        return Err(invalid(items[1].span, "Pi needs one typed binder"));
    }
    let name = String::from(atom(&binder[0])?);
    let Some(code) = Sort3::from_word(atom(&binder[1])?) else {
        return Err(unsupported_profile(
            binder[1].span,
            "lemma binder is not I64 or Bool",
        ));
    };
    if find_binder_index(binders, &name).is_some() {
        return Err(invalid(binder[0].span, "repeated lemma binder"));
    }
    binders.push((name, code));
    let Some(next_depth) = (*depth).checked_add(1) else {
        return Err(budget(current.span));
    };
    *depth = next_depth;
    Ok(Some(items[2].clone()))
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; `meter` charges lemma statement parsing nodes"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid Pi binders and conclusions return typed diagnostics"
)]
fn parse_statement(form: &Form, meter: &mut Meter<'_>) -> Result<Statement3, Diagnostic> {
    let mut remaining = MAX_ASSERTION_NODES;
    let mut binders: Vec<(String, Sort3)> = Vec::new();
    let mut current = form.clone();
    let mut depth = 0u32;
    let mut outcome: Result<(), Diagnostic> = Ok(());
    let mut is_keep_going = true;
    while is_keep_going {
        match parse_statement_pi_step(&current, &mut binders, &mut depth, &mut remaining, meter) {
            Ok(Some(next)) => current = next,
            Ok(None) => is_keep_going = false,
            Err(problem) => {
                outcome = Err(problem);
                is_keep_going = false;
            }
        }
    }
    outcome?;
    let scope = make_scope(&[], &[], &binders, ScopeMode::Lemma);
    let conclusion = parse_prop(&current, scope, depth, &mut remaining, meter)?;
    let mut statement = Statement3::Prop(conclusion);
    while let Some((name, code)) = binders.pop() {
        statement = Statement3::Pi {
            name,
            code,
            body: Box::new(statement),
        };
    }
    Ok(statement)
}

/// One proof value the bidirectional revision-3 checker infers.
#[octet::sealed_enum]
enum Value3 {
    Statement(Statement3),
}

/// One accepted lemma proof and its parsed statement.
#[derive(Clone)]
struct Lemma {
    pub reference: String,
    pub statement: Statement3,
}

/// The logical environment a bridge or join proof may use.
#[derive(Clone, Copy)]
struct ProofScope<'a> {
    pub facts: &'a [Prop3],
    pub lemmas: &'a [Lemma],
    pub contracts: &'a [String],
    pub assignment: Scope<'a>,
}

/// The index of a `fN` fact name.
fn fact_index(name: &str) -> Option<usize> {
    let digits = crate::scan::strip_prefix_str(name, "f")?;
    if digits.is_empty() || digits.len() > 3 {
        return None;
    }
    let bytes = digits.as_bytes();
    let mut at = 0;
    let mut is_all_digits = true;
    while at < bytes.len() && is_all_digits {
        is_all_digits = bytes[at].is_ascii_digit();
        at += 1;
    }
    if !is_all_digits {
        return None;
    }
    digits.parse::<usize>().ok()
}

fn fact_statement(scope: ProofScope<'_>, name: &str) -> Option<Statement3> {
    let index = fact_index(name)?;
    let fact = scope.facts.get(index)?;
    Some(Statement3::Prop(normalize_prop(fact)))
}

fn lemma_has_ref(lemma: &Lemma, name: &str) -> bool {
    str_eq(&lemma.reference, name)
}

fn lemma_statement<'a>(lemmas: &'a [Lemma], name: &str) -> Option<&'a Statement3> {
    let mut found = None;
    let mut li = 0usize;
    while found.is_none() && li < lemmas.len() {
        if lemma_has_ref(&lemmas[li], name) {
            found = Some(li);
        }
        li += 1;
    }
    match found {
        Some(idx) => Some(&lemmas[idx].statement),
        None => None,
    }
}

/// The three reviewed arithmetic rule identities.
#[derive(Clone, Copy)]
#[octet::sealed_enum]
enum Rule3 {
    AddAssoc,
    AddComm,
    MulAdd,
}

#[octet::sealed_enum]
enum InferFrame {
    Eval(Form, u32),
    Apply {
        argument: Form,
        span: Span,
        depth: u32,
    },
    SubstAfterEq {
        motive_scope: Vec<(String, Sort3)>,
        body: Prop3,
        level: usize,
        eq_span: Span,
        premise: Form,
        depth: u32,
    },
    SubstAfterPremise {
        body: Prop3,
        level: usize,
        left: Term3,
        right: Term3,
        premise_span: Span,
    },
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes proof inference frames and values into caller stacks"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; advances inference stacks and proof meter"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; keeps inference stacks, scope, and budgets explicit"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid proof steps return typed Diagnostic"
)]
fn infer_step(
    frame: InferFrame,
    scope: ProofScope<'_>,
    frames: &mut Vec<InferFrame>,
    values: &mut Vec<Value3>,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    match frame {
        InferFrame::Eval(term, depth) => {
            spend!(remaining, meter, depth, term.span);
            if let Some(frame_outcome) =
                eval_leaf_or_push(&term, depth, scope, frames, remaining, meter)?
            {
                values.push(frame_outcome);
            }
        }
        InferFrame::Apply {
            argument,
            span,
            depth,
        } => {
            let Some(function) = values.pop() else {
                return Err(invalid(span, "missing function proof"));
            };
            values.push(apply_inferred(
                function, &argument, scope, depth, remaining, meter, span,
            )?);
        }
        InferFrame::SubstAfterEq {
            motive_scope,
            body,
            level,
            eq_span,
            premise,
            depth,
        } => {
            let Some(equality) = values.pop() else {
                return Err(invalid(eq_span, "subst requires equality evidence"));
            };
            let (left, right) = extract_subst_eq(equality, &motive_scope, scope, eq_span)?;
            frames.push(InferFrame::SubstAfterPremise {
                body,
                level,
                left,
                right,
                premise_span: premise.span,
            });
            frames.push(InferFrame::Eval(premise, depth));
        }
        InferFrame::SubstAfterPremise {
            body,
            level,
            left,
            right,
            premise_span,
        } => {
            let Some(premise) = values.pop() else {
                return Err(invalid(premise_span, "subst premise is missing"));
            };
            let Value3::Statement(Statement3::Prop(premise_prop)) = premise else {
                return Err(invalid(premise_span, "subst premise is not a proposition"));
            };
            let before = replace_prop(&body, level, &left);
            if normalize_prop(&premise_prop) != normalize_prop(&before) {
                return Err(invalid(
                    premise_span,
                    "subst premise does not match the motive instance",
                ));
            }
            values.push(Value3::Statement(Statement3::Prop(replace_prop(
                &body, level, &right,
            ))));
        }
    }
    Ok(())
}

/// Infer the statement of one bridge or join proof term iteratively without mutual recursion.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; `remaining` and `meter` advance once per visited proof form"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; malformed proof terms return typed diagnostics"
)]
fn infer(
    root: &Form,
    scope: ProofScope<'_>,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Value3, Diagnostic> {
    let mut frames = Vec::new();
    frames.push(InferFrame::Eval(root.clone(), 0));
    let mut values: Vec<Value3> = Vec::new();
    let mut outcome = Ok(());
    while let Some(frame) = frames.pop() {
        outcome = infer_step(frame, scope, &mut frames, &mut values, remaining, meter);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    let Some(res) = values.pop() else {
        return Err(invalid(
            root.span,
            "proof term establishes no revision-3 proposition",
        ));
    };
    Ok(res)
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes pending proof frames onto caller vector"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; schedules proof frames and charges meter"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; keeps term, scope, frame worklist, and budgets explicit"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; unknown forms return explicit typed diagnostics"
)]
fn eval_leaf_or_push(
    term: &Form,
    depth: u32,
    scope: ProofScope<'_>,
    frames: &mut Vec<InferFrame>,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Option<Value3>, Diagnostic> {
    if let Ok(args) = operation(term, "use", 1) {
        return infer_use(atom(&args[0])?, scope, term.span).map(Some);
    }
    if let Ok(args) = operation(term, "apply", 2) {
        let Some(next) = depth.checked_add(1) else {
            return Err(budget(term.span));
        };
        frames.push(InferFrame::Apply {
            argument: args[1].clone(),
            span: term.span,
            depth: next,
        });
        frames.push(InferFrame::Eval(args[0].clone(), next));
        return Ok(None);
    }
    if let Ok(args) = operation(term, "refl", 1) {
        let scalar = parse_term(&args[0], scope.assignment, depth, remaining, meter)?;
        if scope.assignment.sort(&scalar).is_none() {
            return Err(invalid(args[0].span, "refl operand has no admitted sort"));
        }
        return Ok(Some(Value3::Statement(Statement3::Prop(Prop3::Eq(
            scalar.clone(),
            scalar,
        )))));
    }
    if let Ok(args) = operation(term, "subst", 3) {
        let Some(next) = depth.checked_add(1) else {
            return Err(budget(term.span));
        };
        let (motive_scope, body, level) =
            parse_subst_motive(&args[0], scope, next, remaining, meter)?;
        frames.push(InferFrame::SubstAfterEq {
            motive_scope,
            body,
            level,
            eq_span: args[1].span,
            premise: args[2].clone(),
            depth: next,
        });
        frames.push(InferFrame::Eval(args[1].clone(), next));
        return Ok(None);
    }
    if let Ok(args) = operation(term, "bv-add-assoc", 3) {
        return rule_inferred(Rule3::AddAssoc, args, scope, depth, remaining, meter).map(Some);
    }
    if let Ok(args) = operation(term, "bv-add-comm", 2) {
        return rule_inferred(Rule3::AddComm, args, scope, depth, remaining, meter).map(Some);
    }
    if let Ok(args) = operation(term, "bv-mul-add", 3) {
        return rule_inferred(Rule3::MulAdd, args, scope, depth, remaining, meter).map(Some);
    }
    if operation(term, "by-true", 0).is_ok() {
        return Ok(Some(Value3::Statement(Statement3::Prop(Prop3::True))));
    }
    Err(invalid(
        term.span,
        "proof term establishes no revision-3 proposition",
    ))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; unresolved or non-lemma uses return explicit typed diagnostics"
)]
fn infer_use(name: &str, scope: ProofScope<'_>, span: Span) -> Result<Value3, Diagnostic> {
    if let Some(statement) = fact_statement(scope, name) {
        return Ok(Value3::Statement(statement));
    }
    if let Some(statement) = lemma_statement(scope.lemmas, name) {
        return Ok(Value3::Statement(statement.clone()));
    }
    if replay_seen(scope.contracts, name) {
        return Err(refined(
            span,
            Reason {
                code: "preuse-sort-mismatch",
                detail: "a contract proof names a program claim, not a lemma",
            },
        ));
    }
    Err(invalid(
        span,
        "unresolved, private, forward or cyclic proof use",
    ))
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; charges argument parsing against the proof budget"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; carries the apply argument, scope, depth, budgets, and source span without extra struct boilerplate"
)]
fn apply_inferred(
    function: Value3,
    argument: &Form,
    scope: ProofScope<'_>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
    span: Span,
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

#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; binder stack bounded by MAX_ASSERTION_NODES"
)]
fn peel_statement_pi(statement: &Statement3) -> (Vec<(String, Sort3)>, &Statement3) {
    let mut binders: Vec<(String, Sort3)> = Vec::new();
    let mut current = statement;
    while let Statement3::Pi { name, code, body } = current {
        binders.push((name.clone(), *code));
        current = body;
    }
    (binders, current)
}

/// Replace the outermost explicit binder of one statement iteratively.
fn replace_statement(statement: &Statement3, argument: &Term3) -> Statement3 {
    let (mut binders, current) = peel_statement_pi(statement);
    let prop = match current {
        Statement3::Prop(prop) => replace_prop(prop, 0, argument),
        Statement3::Pi { .. } => Prop3::False,
    };
    let mut result = Statement3::Prop(prop);
    while let Some((name, code)) = binders.pop() {
        result = Statement3::Pi {
            name,
            code,
            body: Box::new(result),
        };
    }
    result
}

type SubstMotive = (Vec<(String, Sort3)>, Prop3, usize);

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; charges motive parsing against shared meter"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid motives return typed Diagnostic"
)]
fn parse_subst_motive(
    motive_form: &Form,
    scope: ProofScope<'_>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<SubstMotive, Diagnostic> {
    let motive = list(motive_form)?;
    if motive.len() != 3 || head_atom(motive) != Some("motive") {
        return Err(invalid(
            motive_form.span,
            "subst requires an explicit motive",
        ));
    }
    let binder = list(&motive[1])?;
    if binder.len() != 2 {
        return Err(invalid(motive[1].span, "motive needs one typed binder"));
    }
    let name = String::from(atom(&binder[0])?);
    let Some(code) = Sort3::from_word(atom(&binder[1])?) else {
        return Err(unsupported_profile(
            binder[1].span,
            "motive binder is not I64 or Bool",
        ));
    };
    let level = scope.assignment.bound.len();
    let mut bound: Vec<(String, Sort3)> = scope.assignment.bound.to_vec();
    bound.push((name, code));
    let motive_scope = Scope {
        inputs: scope.assignment.inputs,
        outputs: scope.assignment.outputs,
        bound: &bound,
        interface: scope.assignment.interface,
        allow_typed_eq: true,
    };
    let body = parse_prop(&motive[2], motive_scope, depth, remaining, meter)?;
    Ok((bound, body, level))
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; `normalize_prop` and `sort` are not const"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid equality proof returns typed Diagnostic"
)]
fn extract_subst_eq(
    equality: Value3,
    bound: &[(String, Sort3)],
    scope: ProofScope<'_>,
    eq_span: Span,
) -> Result<(Term3, Term3), Diagnostic> {
    let Value3::Statement(Statement3::Prop(equality_prop)) = equality else {
        return Err(invalid(eq_span, "subst requires equality evidence"));
    };
    let Prop3::Eq(left, right) = normalize_prop(&equality_prop) else {
        return Err(invalid(eq_span, "subst requires equality evidence"));
    };
    let Some((_, code)) = bound.last() else {
        return Err(invalid(eq_span, "subst motive binder missing"));
    };
    let motive_scope = Scope {
        inputs: scope.assignment.inputs,
        outputs: scope.assignment.outputs,
        bound,
        interface: scope.assignment.interface,
        allow_typed_eq: true,
    };
    if motive_scope.sort(&left) != Some(*code) || motive_scope.sort(&right) != Some(*code) {
        return Err(invalid(eq_span, "subst equality has the wrong code"));
    }
    Ok((left, right))
}

/// Infer one reviewed arithmetic rule instance.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; charges rule operand parsing against the proof budget"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; passes the rule kind, operand slice, scope, depth, and proof budgets together"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid operand sorts return typed Diagnostic"
)]
fn rule_inferred(
    rule: Rule3,
    args: &[Form],
    scope: ProofScope<'_>,
    depth: u32,
    remaining: &mut usize,
    meter: &mut Meter<'_>,
) -> Result<Value3, Diagnostic> {
    let mut scalars = Vec::with_capacity(args.len());
    let mut index = 0usize;
    let mut outcome = Ok(());
    while index < args.len() {
        let argument = &args[index];
        outcome = match parse_term(argument, scope.assignment, depth, remaining, meter) {
            Ok(scalar) => match require_i64(&scalar, scope.assignment, argument.span) {
                Ok(()) => {
                    scalars.push(scalar);
                    Ok(())
                }
                Err(problem) => Err(problem),
            },
            Err(problem) => Err(problem),
        };
        if outcome.is_err() {
            break;
        }
        index += 1;
    }
    outcome?;
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
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; charges proof inference against `meter`"
)]
fn check_proof(
    proof: &Form,
    expected: &Prop3,
    scope: ProofScope<'_>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let mut remaining = MAX_ASSERTION_NODES;
    let value = infer(proof, scope, &mut remaining, meter)?;
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
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid interfaces return typed diagnostics"
)]
fn interface(
    bindings: &[(String, String)],
    span: Span,
) -> Result<Vec<(String, Sort3)>, Diagnostic> {
    if bindings.len() > MAX_INTERFACE {
        return Err(unsupported_profile(
            span,
            "revision-3 interface exceeds four values",
        ));
    }
    let mut result: Vec<(String, Sort3)> = Vec::with_capacity(bindings.len());
    for (name, ty) in bindings {
        let Some(code) = Sort3::from_word(ty) else {
            return Err(unsupported_profile(
                span,
                "revision-3 interface is not I64/Bool only",
            ));
        };
        if name.is_empty() {
            return Err(invalid(span, "empty interface binder"));
        }
        if find_binder_index(&result, name).is_some() {
            return Err(invalid(span, "repeated interface binder"));
        }
        result.push((name.clone(), code));
    }
    Ok(result)
}

fn interface_codes(entries: &[(String, Sort3)]) -> Vec<Sort3> {
    let mut codes = Vec::with_capacity(entries.len());
    for (_, code) in entries {
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
    text = write_form(text, &goal.requires);
    text.push_str("\nensures ");
    text = write_form(text, &goal.ensures);
    text
}

#[octet::sealed_enum]
enum FormWrite {
    Form(Form, u32),
    Space,
    Close,
}

fn push_form_list(mut work: Vec<FormWrite>, items: &[Form], next: u32) -> Vec<FormWrite> {
    let mut index = items.len();
    while index > 0 {
        let prev = index.saturating_sub(1);
        work.push(FormWrite::Form(items[prev].clone(), next));
        if prev > 0 {
            work.push(FormWrite::Space);
        }
        index = prev;
    }
    work
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends formatted token and pending forms into caller buffers"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates caller formatting buffer and worklist in place"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; String::push and Vec::push are not const"
)]
fn write_form_step(step: FormWrite, text: &mut String, work: &mut Vec<FormWrite>) {
    match step {
        FormWrite::Space => text.push(' '),
        FormWrite::Close => text.push(')'),
        FormWrite::Form(form, depth) => {
            if depth > 64 {
                text.push_str("...");
            } else {
                match &form.kind {
                    FormKind::Atom(word) => text.push_str(word),
                    FormKind::List(items) => {
                        text.push('(');
                        work.push(FormWrite::Close);
                        let next = depth.saturating_add(1);
                        let mut prev_work = core::mem::take(work);
                        prev_work = push_form_list(prev_work, items, next);
                        *work = prev_work;
                    }
                }
            }
        }
    }
}

fn write_form(mut text: String, root: &Form) -> String {
    let mut work = Vec::new();
    work.push(FormWrite::Form(root.clone(), 0));
    while let Some(step) = work.pop() {
        write_form_step(step, &mut text, &mut work);
    }
    text
}

/// Verify the caller subject's own source module and retained `named_imports`.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid sources return typed diagnostics"
)]
fn check_retained_dependencies(goal: &ContractGoal, limits: Limits) -> Result<Span, Diagnostic> {
    let subject = &goal.subject;
    let span = subject.source_span;
    let caller = crate::source::DefinitionOrigin {
        module_source: &subject.module_source,
        module: &subject.module,
        version: subject.version,
        definition: &subject.definition,
        ordinal: subject.definition_ordinal,
        span,
    };
    let subject_body =
        crate::source::verify_definition_origin(caller, &subject.definition_source, limits)?;
    let mut owners = Vec::with_capacity(subject.source_dependencies.len());
    let mut has_duplicate_owner = false;
    let mut dep_at = 0usize;
    while dep_at < subject.source_dependencies.len() && !has_duplicate_owner {
        let owner = subject.source_dependencies[dep_at].owner;
        if owners.contains(&owner) {
            has_duplicate_owner = true;
        } else {
            owners.push(owner);
        }
        dep_at += 1;
    }
    if has_duplicate_owner {
        return Err(invalid(span, "duplicate named source module"));
    }
    let has_own_module = crate::scan::find_first(&subject.source_dependencies, |module| {
        module.owner == subject.definition_owner
            && module.module == subject.module
            && module.version == subject.version
            && module.full_source == subject.module_source
    })
    .is_some();
    if !has_own_module {
        return Err(invalid(span, "subject module is not a retained source"));
    }
    check_retained_import_aliases(subject, span)?;
    Ok(subject_body)
}

fn import_has_alias(binding: &super::NamedImportBinding, alias: &str) -> bool {
    str_eq(&binding.alias, alias)
}

fn count_import_alias(imports: &[super::NamedImportBinding], alias: &str) -> usize {
    let mut count = 0usize;
    let mut other_at = 0usize;
    while other_at < imports.len() {
        if import_has_alias(&imports[other_at], alias) {
            count = count.saturating_add(1);
        }
        other_at += 1;
    }
    count
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; inspects runtime Vec slice fields in SubjectBinding"
)]
fn check_one_import_alias(
    subject: &super::SubjectBinding,
    binding: &super::NamedImportBinding,
) -> u8 {
    match subject.source_dependencies.get(binding.module_index) {
        Some(module) => {
            if module.owner != binding.owner {
                2
            } else if count_import_alias(&subject.named_imports, &binding.alias) != 1 {
                1
            } else {
                0
            }
        }
        None => 2,
    }
}

fn check_retained_import_aliases(
    subject: &super::SubjectBinding,
    span: Span,
) -> Result<(), Diagnostic> {
    let mut failure_code = 0u8;
    let mut binding_at = 0usize;
    while binding_at < subject.named_imports.len() && failure_code == 0 {
        failure_code = check_one_import_alias(subject, &subject.named_imports[binding_at]);
        binding_at += 1;
    }
    match failure_code {
        0 => Ok(()),
        1 => Err(invalid(span, "repeated retained import alias")),
        _ => Err(invalid(
            span,
            "retained import alias lacks its frozen original module",
        )),
    }
}

/// Authenticate every traced occurrence against immutable caller and callee
/// source and the frozen import snapshot.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid occurrence tokens return typed Diagnostic"
)]
fn authenticate_occurrences(
    goal: &ContractGoal,
    occurrences: &[usize],
    limits: Limits,
) -> Result<(), Diagnostic> {
    let subject_body = check_retained_dependencies(goal, limits)?;
    let subject = &goal.subject;
    let span = subject.source_span;
    let caller = crate::source::DefinitionOrigin {
        module_source: &subject.module_source,
        module: &subject.module,
        version: subject.version,
        definition: &subject.definition,
        ordinal: subject.definition_ordinal,
        span,
    };
    let mut index = 0usize;
    let mut outcome = Ok(());
    while index < occurrences.len() {
        let occ = &subject.named_uses[occurrences[index]];
        outcome = authenticate_one_occurrence(subject, caller, subject_body, occ, limits);
        if outcome.is_err() {
            break;
        }
        index += 1;
    }
    outcome
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid call occurrence sources return typed diagnostics"
)]
fn authenticate_one_occurrence(
    subject: &super::SubjectBinding,
    caller: crate::source::DefinitionOrigin<'_>,
    subject_body: Span,
    occurrence: &NamedUseOrigin,
    limits: Limits,
) -> Result<(), Diagnostic> {
    let span = subject.source_span;
    let Some(source_node) = occurrence.source_node else {
        return Err(invalid(span, "call occurrence has no source node"));
    };
    let Some(source_span) = occurrence.source_span else {
        return Err(invalid(span, "call occurrence has no source span"));
    };
    let Some(module) = subject.source_dependencies.get(occurrence.module_index) else {
        return Err(invalid(span, "callee module is not retained"));
    };
    if source_span.start < subject_body.start
        || source_span.end > subject_body.end
        || source_span.start >= source_span.end
    {
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
    verify_occurrence_spelling(
        subject,
        caller,
        occurrence,
        module,
        source_node,
        source_span,
        limits,
    )
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; verifies one authenticated occurrence's token spelling against the frozen import table"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid occurrence tokens return typed Diagnostic"
)]
fn verify_occurrence_spelling(
    subject: &super::SubjectBinding,
    caller: crate::source::DefinitionOrigin<'_>,
    occurrence: &NamedUseOrigin,
    module: &super::ResolvedSourceModule,
    source_node: u32,
    source_span: Span,
    limits: Limits,
) -> Result<(), Diagnostic> {
    let original =
        crate::source::verify_named_call_origin(caller, source_node, source_span, limits)?;
    match crate::scan::rsplit_once_byte(original, b'.') {
        Some((scope, word)) => {
            if word != occurrence.definition_name {
                return Err(invalid(
                    source_span,
                    "original named word differs from the frozen callee",
                ));
            }
            if !names_module(scope, module) && !snapshot_alias(subject, scope, module) {
                return Err(invalid(
                    source_span,
                    "original named scope is not the frozen callee module",
                ));
            }
        }
        None => {
            if original != occurrence.definition_name {
                return Err(invalid(
                    source_span,
                    "original named word differs from the frozen callee",
                ));
            }
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
    Ok(())
}

/// The retained bytes `span` names.
fn source_bytes(source: &[u8], span: Span, at: Span) -> Result<&[u8], Diagnostic> {
    let (Ok(start), Ok(end)) = (usize::try_from(span.start), usize::try_from(span.end)) else {
        return Err(invalid(span, "source span exceeds the address space"));
    };
    let Some(bytes) = source.get(start..end) else {
        return Err(invalid(at, "source span is outside the retained module"));
    };
    Ok(bytes)
}

/// Whether `qualified` is exactly `name@version` naming `module`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; `rsplit_once_byte` and `parse` are not const"
)]
fn names_module(qualified: &str, module: &super::ResolvedSourceModule) -> bool {
    match crate::scan::rsplit_once_byte(qualified, b'@') {
        Some((name, version)) => {
            name == module.module && version.parse::<u32>() == Ok(module.version)
        }
        None => false,
    }
}

/// Whether the frozen snapshot binds `scope` to `module`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; String dereference is not const on the pinned compiler"
)]
fn is_bound_import(
    binding: &super::NamedImportBinding,
    scope: &str,
    owner: u64,
    expected_index: usize,
) -> bool {
    str_eq(&binding.alias, scope)
        && binding.owner == owner
        && binding.module_index == expected_index
}

fn snapshot_alias(
    subject: &super::SubjectBinding,
    scope: &str,
    module: &super::ResolvedSourceModule,
) -> bool {
    let Some(expected_index) = dependency_position(&subject.source_dependencies, module.owner)
    else {
        return false;
    };
    let mut position = 0;
    let mut is_bound = false;
    while !is_bound && position < subject.named_imports.len() {
        is_bound = is_bound_import(
            &subject.named_imports[position],
            scope,
            module.owner,
            expected_index,
        );
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
/// envelope.
#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; matches public revision-3 API naming in the specification"
)]
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

fn callee_identity(occurrence: &NamedUseOrigin, module: &super::ResolvedSourceModule) -> String {
    let mut identity = module.module.clone();
    identity.push('@');
    crate::rendering::number(&mut identity, u64::from(module.version));
    identity.push('.');
    identity.push_str(&occurrence.definition_name);
    identity
}

/// One resolved callee contract and its source-side proof record.
#[derive(Clone, Copy)]
#[octet::sealed_enum]
enum CalleeSource {
    Dep(usize),
    Local(usize),
}

struct Callee {
    reference: String,
    source: CalleeSource,
}

const fn callee_contract<'a>(
    source: CalleeSource,
    dependencies: &'a [ProofDependency],
    local: &'a [ProofObligation],
) -> Option<&'a ContractGoal> {
    match source {
        CalleeSource::Dep(idx) => match &dependencies[idx].declaration.goal {
            PendingGoal::Contract { contract } => Some(contract),
            PendingGoal::Pure { .. } => None,
        },
        CalleeSource::Local(idx) => match &local[idx].goal {
            PendingGoal::Contract { contract } => Some(contract),
            PendingGoal::Pure { .. } => None,
        },
    }
}

const fn callee_dependency(
    source: CalleeSource,
    dependencies: &[ProofDependency],
) -> Option<&ProofDependency> {
    match source {
        CalleeSource::Dep(idx) => Some(&dependencies[idx]),
        CalleeSource::Local(_) => None,
    }
}

fn dependency_has_name(dep: &ProofDependency, name: &str) -> bool {
    str_eq(&dep.name, name)
}

fn obligation_has_name(obligation: &ProofObligation, name: &str) -> bool {
    str_eq(&obligation.name, name)
}

/// Resolve one `pc-call` reference among the imported dependency snapshot and
/// the caller module's earlier proofs.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; missing or non-contract references return typed diagnostics"
)]
fn resolve_callee(
    reference: &str,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    span: Span,
) -> Result<Callee, Diagnostic> {
    let mut dep_idx = dependencies.len();
    let mut di = 0usize;
    while dep_idx == dependencies.len() && di < dependencies.len() {
        if dependency_has_name(&dependencies[di], reference) {
            dep_idx = di;
        }
        di += 1;
    }
    if dep_idx < dependencies.len() {
        let idx = dep_idx;
        let dependency = &dependencies[idx];
        if !dependency.exported {
            return Err(refined(
                span,
                Reason {
                    code: "preuse-dependency-missing",
                    detail: "the imported proof is private",
                },
            ));
        }
        let PendingGoal::Contract { .. } = &dependency.declaration.goal else {
            return Err(refined(
                span,
                Reason {
                    code: "preuse-sort-mismatch",
                    detail: "a lemma proof is not a program claim",
                },
            ));
        };
        return Ok(Callee {
            reference: String::from(reference),
            source: CalleeSource::Dep(idx),
        });
    }
    let mut loc_idx = local.len();
    let mut li = 0usize;
    while loc_idx == local.len() && li < local.len() {
        if obligation_has_name(&local[li], reference) {
            loc_idx = li;
        }
        li += 1;
    }
    if loc_idx < local.len() {
        let idx = loc_idx;
        let PendingGoal::Contract { .. } = &local[idx].goal else {
            return Err(refined(
                span,
                Reason {
                    code: "preuse-sort-mismatch",
                    detail: "a lemma proof is not a program claim",
                },
            ));
        };
        return Ok(Callee {
            reference: String::from(reference),
            source: CalleeSource::Local(idx),
        });
    }
    Err(refined(
        span,
        Reason {
            code: "preuse-dependency-missing",
            detail: "no exported contract proof has this reference",
        },
    ))
}

/// The fields of one occurrence that the callee contract must reproduce.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; slice comparisons are not const"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; returns mismatch label when metadata diverges"
)]
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
    if let Some(mismatch) = callee_subject_differs(occurrence, contract) {
        return Some(mismatch);
    }
    let (Ok(start), Ok(end)) = (
        usize::try_from(contract.subject.source_span.start),
        usize::try_from(contract.subject.source_span.end),
    ) else {
        return Some("callee source span");
    };
    match contract.subject.module_source.get(start..end) {
        Some(body) if body == contract.subject.definition_source => {}
        Some(_) | None => return Some("callee definition source"),
    }
    callee_interface_differs(contract)
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; String equality checks are not const"
)]
fn callee_subject_differs(
    occurrence: &NamedUseOrigin,
    contract: &ContractGoal,
) -> Option<&'static str> {
    if contract.subject.definition != occurrence.definition_name
        || contract.subject.definition_ordinal != occurrence.definition_ordinal
    {
        return Some("callee subject");
    }
    if contract.subject.source_span != occurrence.definition_span
        || contract.subject.definition_identity != occurrence.definition_identity
        || contract.subject.definition_owner != occurrence.definition_owner
    {
        return Some("callee subject");
    }
    None
}

const fn sort_matches_word(ty: &noble_kernel::types::Ty, word: &str) -> bool {
    match (Sort3::from_ty(ty), Sort3::from_word(word)) {
        (Some(a), Some(b)) => a as u8 == b as u8,
        _ => false,
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; returns mismatch label on invalid interface"
)]
fn callee_interface_differs(contract: &ContractGoal) -> Option<&'static str> {
    if !contract.subject.effects.is_empty() {
        return Some("effects");
    }
    if contract.subject.input_types.len() > MAX_INTERFACE
        || contract.subject.output_types.len() > MAX_INTERFACE
    {
        return Some("interface");
    }
    if contract.inputs.len() != contract.subject.input_types.len()
        || contract.outputs.len() != contract.subject.output_types.len()
    {
        return Some("interface");
    }
    let mut mismatch_code = 0u8;
    let mut index = 0;
    while index < contract.inputs.len() && mismatch_code == 0 {
        if !sort_matches_word(
            &contract.subject.input_types[index],
            &contract.inputs[index].1,
        ) {
            mismatch_code = 1;
        }
        index += 1;
    }
    let mut index = 0;
    while index < contract.outputs.len() && mismatch_code == 0 {
        if !sort_matches_word(
            &contract.subject.output_types[index],
            &contract.outputs[index].1,
        ) {
            mismatch_code = 2;
        }
        index += 1;
    }
    match mismatch_code {
        0 => None,
        1 => Some("input interface"),
        _ => Some("output interface"),
    }
}

/// One caller-body step the symbolic walk supports.
#[octet::sealed_enum]
enum Step {
    Literal(noble_kernel::untrusted::Lit),
    Builtin(u32),
    Call(usize),
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; `Option::map` with enum constructor is not const"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; returns `None` outside the pure profile"
)]
fn classify(node: &noble_kernel::untrusted::Node, occurrence: Option<usize>) -> Option<Step> {
    if let Some(index) = occurrence {
        return match node {
            noble_kernel::untrusted::Node::Invocation { .. } => Some(Step::Call(index)),
            noble_kernel::untrusted::Node::Literal { .. }
            | noble_kernel::untrusted::Node::SignedSelf { .. }
            | noble_kernel::untrusted::Node::Quotation { .. }
            | noble_kernel::untrusted::Node::SlotInvoke { .. } => None,
        };
    }
    match node {
        noble_kernel::untrusted::Node::Literal { lit, .. } => Some(Step::Literal(*lit)),
        noble_kernel::untrusted::Node::Invocation { def, .. } => match def.0 {
            0 | 1 | 2 | 4..=7 => Some(Step::Builtin(def.0)),
            _ => None,
        },
        noble_kernel::untrusted::Node::SignedSelf { .. }
        | noble_kernel::untrusted::Node::Quotation { .. }
        | noble_kernel::untrusted::Node::SlotInvoke { .. } => None,
    }
}

fn call_slice(slice: &Form) -> Result<(usize, &str, &Form), Diagnostic> {
    let call = operation(slice, "pc-call", 3)?;
    let Ok(ordinal) = atom(&call[0])?.parse::<usize>() else {
        return Err(invalid(call[0].span, "call slice ordinal is not a number"));
    };
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
    let mut index = 0;
    let mut is_found = false;
    while !is_found && index < seen.len() {
        if str_eq(&seen[index], identity) {
            is_found = true;
        }
        index += 1;
    }
    is_found
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; `facts` is the symbolic state's owned fact vector"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends normalized conjuncts to the caller's fact vector"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; enforces MAX_FACTS via typed Diagnostic"
)]
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; bounded by MAX_FACTS check before push"
)]
fn add_fact(facts: &mut Vec<Prop3>, prop: &Prop3) -> Result<(), Diagnostic> {
    let mut pending = Vec::new();
    pending.push(prop.clone());
    while let Some(node) = pending.pop() {
        match node {
            Prop3::And(left, right) => {
                pending.push(*right);
                pending.push(*left);
            }
            Prop3::True
            | Prop3::False
            | Prop3::Not(_)
            | Prop3::Eq(..)
            | Prop3::Lt(..)
            | Prop3::Le(..) => {
                if facts.len() >= MAX_FACTS {
                    return Err(budget(Span { start: 0, end: 0 }));
                }
                facts.push(normalize_prop(&node));
            }
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends flattened conjuncts into caller vector"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; collects top-level conjuncts into caller buffer"
)]
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; bounded by validated proposition AST node count"
)]
fn collect_conjuncts_owned(prop: &Prop3, out: &mut Vec<Prop3>) {
    let mut pending = Vec::new();
    pending.push(prop.clone());
    while let Some(node) = pending.pop() {
        match node {
            Prop3::And(left, right) => {
                pending.push(*right);
                pending.push(*left);
            }
            Prop3::True
            | Prop3::False
            | Prop3::Not(_)
            | Prop3::Eq(..)
            | Prop3::Lt(..)
            | Prop3::Le(..) => out.push(node),
        }
    }
}

/// The direct output term `U[in ↦ arguments]` of a top-level
/// `(eq (out j) U)` conjunct that mentions no output.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; calls Vec-backed mentions_output traversal"
)]
fn is_direct_output_eq(prop: &Prop3, index: usize) -> bool {
    match prop {
        Prop3::Eq(Term3::Output(out_i), right) => *out_i == index && !mentions_output(right),
        _ => false,
    }
}

fn direct_output(ensures: &Prop3, index: usize, arguments: &[Term3]) -> Option<Term3> {
    let mut conjuncts: Vec<Prop3> = Vec::new();
    collect_conjuncts_owned(ensures, &mut conjuncts);
    let mut pos = 0;
    let mut found: Option<usize> = None;
    while pos < conjuncts.len() && found.is_none() {
        if is_direct_output_eq(&conjuncts[pos], index) {
            found = Some(pos);
        }
        pos += 1;
    }
    match found {
        Some(idx) => match &conjuncts[idx] {
            Prop3::Eq(_, right) => Some(normalize_term(&instantiate_term(right, arguments, &[]))),
            _ => None,
        },
        None => None,
    }
}

/// Scan every `(use NAME)` of a proof term in form order, iteratively.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; appends distinct lemma names into the caller's list"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes resolved lemma names into `names`"
)]
fn record_lemma_name(name: &str, span: Span, names: &mut Vec<String>) -> Result<(), Diagnostic> {
    let is_fact = fact_index(name).is_some();
    if is_fact || replay_seen(names, name) {
        return Ok(());
    }
    if names.len() >= MAX_LEMMA_USES {
        return Err(budget(span));
    }
    names.push(String::from(name));
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes pending AST items in reverse order"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates lemma traversal worklist"
)]
fn push_pending_forms_rev(items: &[Form], pending: &mut Vec<Form>) {
    let mut idx = items.len();
    while idx > 0 {
        let prev = idx.saturating_sub(1);
        pending.push(items[prev].clone());
        idx = prev;
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes pending AST nodes and lemma names into caller vectors"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates lemma traversal worklist and name list"
)]
fn lemma_names_step(
    node: Form,
    pending: &mut Vec<Form>,
    names: &mut Vec<String>,
) -> Result<(), Diagnostic> {
    if let FormKind::List(items) = &node.kind {
        if head_atom(items) == Some("use") && items.len() == 2 {
            let name = atom(&items[1])?;
            record_lemma_name(name, node.span, names)?;
        }
        push_pending_forms_rev(items, pending);
    }
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appends distinct lemma names into caller's preallocated vector"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; appends distinct lemma names in form traversal order"
)]
fn lemma_names(form: &Form, names: &mut Vec<String>) -> Result<(), Diagnostic> {
    let mut pending = Vec::new();
    pending.push(form.clone());
    let mut outcome = Ok(());
    while let Some(node) = pending.pop() {
        outcome = lemma_names_step(node, &mut pending, names);
        if outcome.is_err() {
            break;
        }
    }
    outcome
}

/// Resolve every distinct lemma use against the imported dependency snapshot
/// or earlier local pure obligations, and parse its statement.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; `meter` charges statement parsing against the proof budget"
)]
fn resolved_lemmas(
    names: &[String],
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    span: Span,
    meter: &mut Meter<'_>,
) -> Result<Vec<Lemma>, Diagnostic> {
    let mut lemmas = Vec::with_capacity(names.len());
    for name in names {
        let proposition = find_lemma_proposition(name, dependencies, local, span)?;
        let statement = parse_statement(proposition, meter)?;
        lemmas.push(Lemma {
            reference: name.clone(),
            statement,
        });
    }
    Ok(lemmas)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; missing or mismatched lemma targets return typed diagnostics"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; lemma lookup via closure is not const"
)]
fn find_lemma_proposition<'a>(
    name: &str,
    dependencies: &'a [ProofDependency],
    local: &'a [ProofObligation],
    span: Span,
) -> Result<&'a Form, Diagnostic> {
    if let Some(dependency) = crate::scan::find_first(dependencies, |item| item.name == name) {
        if !dependency.exported {
            return Err(refined(
                span,
                Reason {
                    code: "preuse-dependency-missing",
                    detail: "the imported lemma is private",
                },
            ));
        }
        let PendingGoal::Pure { proposition } = &dependency.declaration.goal else {
            return Err(refined(
                span,
                Reason {
                    code: "preuse-sort-mismatch",
                    detail: "a program contract is not a logical lemma",
                },
            ));
        };
        return Ok(proposition);
    }
    if let Some(obligation) = crate::scan::find_first(local, |item| item.name == name) {
        let PendingGoal::Pure { proposition } = &obligation.goal else {
            return Err(refined(
                span,
                Reason {
                    code: "preuse-sort-mismatch",
                    detail: "a program contract is not a logical lemma",
                },
            ));
        };
        return Ok(proposition);
    }
    Err(refined(
        span,
        Reason {
            code: "preuse-dependency-missing",
            detail: "no exported lemma proof has this reference",
        },
    ))
}

const fn is_caller_def(
    caller: Option<noble_kernel::contracts::Definition>,
    target: noble_kernel::contracts::Definition,
) -> bool {
    match caller {
        Some(d) => d.0 == target.0,
        None => false,
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; filters and deterministically sorts caller occurrences"
)]
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; bounded by `goal.subject.named_uses.len()`"
)]
fn caller_occurrences(
    goal: &ContractGoal,
    selected: &noble_kernel::execution::Definition,
) -> Vec<usize> {
    let mut result: Vec<usize> = Vec::new();
    let mut index = 1usize;
    while index < goal.subject.named_uses.len() {
        if is_caller_def(
            goal.subject.named_uses[index].caller_definition,
            selected.definition,
        ) {
            result.push(index);
        }
        index += 1;
    }
    // Insertion sort preserves deterministic source span order without `slice::sort_by_key`.
    let mut i = 1usize;
    while i < result.len() {
        let mut j = i;
        while j > 0 {
            let prev_idx = j.saturating_sub(1);
            let prev_start = match goal.subject.named_uses[result[prev_idx]].source_span {
                Some(s) => s.start,
                None => 0,
            };
            let cur_start = match goal.subject.named_uses[result[j]].source_span {
                Some(s) => s.start,
                None => 0,
            };
            if prev_start <= cur_start {
                break;
            }
            result.swap(prev_idx, j);
            j = prev_idx;
        }
        i += 1;
    }
    result
}

/// The symbolic state of one caller body walk.
struct State {
    stack: Vec<Term3>,
    facts: Vec<Prop3>,
    ghosts: u32,
}

/// Push one value, refusing a stack over the profile bound.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; pushes one symbolic term onto the owned execution stack"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; requires growable stack buffer to push checked values"
)]
fn push(stack: &mut Vec<Term3>, term: Term3, span: Span) -> Result<(), Diagnostic> {
    if stack.len() >= MAX_STACK {
        return Err(budget(span));
    }
    stack.push(term);
    Ok(())
}

/// Apply one pure builtin step.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates the owned symbolic stack"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pops and pushes on the owned symbolic stack vector"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; stack underflow returns typed Diagnostic"
)]
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
        4..=6 => {
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
const fn ghost(state: &State, index: u32, code: Sort3) -> Term3 {
    Term3::Ghost(state.ghosts, index, code)
}

/// Apply one callee contract step: exact argument sorting, explicit bridge,
/// contract outputs and the carried postcondition.
#[derive(Clone, Copy)]
struct CallSite<'a> {
    callee: &'a Callee,
    bridge: &'a Form,
    context: WalkContext<'a>,
    span: Span,
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates symbolic state and meter for this call site"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; mismatched arguments or unproved bridges return typed diagnostics"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; validates callee interface, checks bridge, updates stack and facts atomically"
)]
fn call_step(
    state: &mut State,
    site: CallSite<'_>,
    meter: &mut Meter<'_>,
) -> Result<CallStepInfo, Diagnostic> {
    let CallSite {
        callee,
        bridge,
        context,
        span,
    } = site;
    let Some(contract) = callee_contract(callee.source, context.dependencies, context.local) else {
        return Err(invalid(span, "missing callee contract goal"));
    };
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
    let mut is_bad_arg = false;
    let mut arg_at = 0;
    while arg_at < arguments.len() {
        let is_mismatch = sort_of(&arguments[arg_at], context.inputs, context.outputs, &[])
            != Some(callee_inputs[arg_at].1);
        if is_mismatch {
            is_bad_arg = true;
            break;
        }
        arg_at += 1;
    }
    if is_bad_arg {
        return Err(invalid(span, "call argument has the wrong ordered sort"));
    }
    let requires = assertion(&contract.requires, &callee_inputs, &callee_outputs, meter)?;
    if prop_mentions_output(&requires) {
        return Err(unsupported_profile(
            contract.requires.span,
            "callee precondition mentions an output",
        ));
    }
    let bridge_goal = normalize_prop(&instantiate_prop(&requires, &arguments, &[]));
    let assignment = make_scope(context.inputs, context.outputs, &[], ScopeMode::Motive);
    let proof_scope = ProofScope {
        facts: &state.facts,
        lemmas: context.lemmas,
        contracts: context.contracts,
        assignment,
    };
    if let Err(problem) = check_proof(bridge, &bridge_goal, proof_scope, meter) {
        return Err(refined(
            bridge.span,
            Reason {
                code: "preuse-bridge-unproved",
                detail: &problem.message,
            },
        ));
    }
    let ensures = assertion(&contract.ensures, &callee_inputs, &callee_outputs, meter)?;
    let (results, direct_outputs) =
        collect_callee_outputs(state, &ensures, &callee_outputs, &arguments, span)?;
    state.stack.truncate(split);
    let instance = normalize_prop(&instantiate_prop(&ensures, &arguments, &results));
    add_fact(&mut state.facts, &instance)?;
    let mut outcome = Ok(());
    let mut ri = 0usize;
    while ri < results.len() {
        outcome = push(&mut state.stack, results[ri].clone(), span);
        if outcome.is_err() {
            break;
        }
        ri += 1;
    }
    outcome?;
    state.ghosts = state.ghosts.saturating_add(1);
    Ok(CallStepInfo {
        callee_inputs,
        callee_outputs,
        callee_requires: requires,
        callee_ensures: ensures,
        direct_outputs,
    })
}

struct CallStepInfo {
    callee_inputs: Vec<(String, Sort3)>,
    callee_outputs: Vec<(String, Sort3)>,
    callee_requires: Prop3,
    callee_ensures: Prop3,
    direct_outputs: Vec<Option<Term3>>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; output count bounded by MAX_INTERFACE"
)]
fn collect_callee_outputs(
    state: &State,
    ensures: &Prop3,
    callee_outputs: &[(String, Sort3)],
    arguments: &[Term3],
    span: Span,
) -> Result<(Vec<Term3>, Vec<Option<Term3>>), Diagnostic> {
    let mut results: Vec<Term3> = Vec::with_capacity(callee_outputs.len());
    let mut direct: Vec<Option<Term3>> = Vec::with_capacity(callee_outputs.len());
    let mut index = 0;
    while index < callee_outputs.len() {
        let code = callee_outputs[index].1;
        match direct_output(ensures, index, arguments) {
            Some(term) => {
                direct.push(Some(term.clone()));
                results.push(term);
            }
            None => {
                let Ok(out_u32) = u32::try_from(index) else {
                    return Err(budget(span));
                };
                let fresh = ghost(state, out_u32, code);
                direct.push(None);
                results.push(fresh);
            }
        }
        index += 1;
    }
    Ok((results, direct))
}

/// The subject and occurrence view reconstructed by both contract staging and
/// proof checking.
struct SubjectView {
    pub inputs: Vec<(String, Sort3)>,
    pub outputs: Vec<(String, Sort3)>,
    pub input_codes: Vec<Sort3>,
    pub output_codes: Vec<Sort3>,
    pub selected_idx: usize,
    pub checked: noble_kernel::untrusted::Checked,
    pub occurrences: Vec<usize>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; non-I64/Bool interface types return typed Diagnostic"
)]
fn collect_interface_codes(
    subject: &super::SubjectBinding,
    span: Span,
) -> Result<(Vec<Sort3>, Vec<Sort3>), Diagnostic> {
    let mut subject_inputs = Vec::with_capacity(subject.input_types.len());
    let mut is_bad_interface = false;
    let mut in_at = 0usize;
    while in_at < subject.input_types.len() && !is_bad_interface {
        if let Some(code) = Sort3::from_ty(&subject.input_types[in_at]) {
            subject_inputs.push(code);
        } else {
            is_bad_interface = true;
        }
        in_at += 1;
    }
    let mut subject_outputs = Vec::with_capacity(subject.output_types.len());
    let mut out_at = 0usize;
    while out_at < subject.output_types.len() && !is_bad_interface {
        if let Some(code) = Sort3::from_ty(&subject.output_types[out_at]) {
            subject_outputs.push(code);
        } else {
            is_bad_interface = true;
        }
        out_at += 1;
    }
    if is_bad_interface {
        return Err(unsupported_profile(
            span,
            "revision-3 subject interface is not I64/Bool only",
        ));
    }
    Ok((subject_inputs, subject_outputs))
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid root metadata returns typed Diagnostic"
)]
fn validate_root_origin(
    subject: &super::SubjectBinding,
    span: Span,
) -> Result<(&NamedUseOrigin, usize), Diagnostic> {
    let Some(root) = subject.named_uses.first() else {
        return Err(invalid(span, "revision-3 subject has no origin"));
    };
    let defs = &subject.accepted_submission.definitions;
    let mut selected_idx = None;
    let mut di = 0usize;
    while selected_idx.is_none() && di < defs.len() {
        if defs[di].definition.0 == root.definition.0 {
            selected_idx = Some(di);
        }
        di += 1;
    }
    let Some(selected_idx) = selected_idx else {
        return Err(invalid(
            span,
            "unique selected revision-3 subject is absent",
        ));
    };
    let selected = &defs[selected_idx];
    if selected.identity != subject.definition_identity || root.definition_span != span {
        return Err(invalid(
            span,
            "revision-3 subject root is not the selected definition",
        ));
    }
    if root.caller_definition.is_some() || root.source_span.is_some() || root.source_node.is_some()
    {
        return Err(invalid(
            span,
            "revision-3 subject root is not the selected definition",
        ));
    }
    if root.definition_name != subject.definition
        || root.definition_ordinal != subject.definition_ordinal
        || root.definition_owner != subject.definition_owner
    {
        return Err(invalid(
            span,
            "revision-3 subject root is not the selected definition",
        ));
    }
    Ok((root, selected_idx))
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; records seen slot ids and distinct callee identities into caller buffers"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates caller last-node cursor and seen/distinct accumulators"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; keeps subject, body, occurrence, cursor, and seen sets explicit"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; out-of-order or duplicate occurrences return typed Diagnostic"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; Vec mutation and identity formatting are not const"
)]
fn check_one_occurrence_order(
    subject: &super::SubjectBinding,
    body: &[noble_kernel::untrusted::NodeId],
    occurrence: &NamedUseOrigin,
    span: Span,
    last_node: &mut Option<usize>,
    slot_seen: &mut Vec<u32>,
    distinct: &mut Vec<String>,
) -> Result<(), Diagnostic> {
    let Some(source_span) = occurrence.source_span else {
        return Err(invalid(span, "call occurrence has no source span"));
    };
    let Some(source_node) = occurrence.source_node else {
        return Err(invalid(span, "call occurrence has no source node"));
    };
    let Ok(node_idx) = usize::try_from(source_node) else {
        return Err(invalid(
            source_span,
            "call occurrence is outside the ordered source body",
        ));
    };
    let Some(&body_node) = body.get(node_idx) else {
        return Err(invalid(
            source_span,
            "call occurrence is outside the ordered source body",
        ));
    };
    let is_out_of_order = match *last_node {
        Some(prev) => prev >= node_idx,
        None => false,
    };
    if body_node != occurrence.candidate_node || is_out_of_order {
        return Err(invalid(
            source_span,
            "call occurrence is outside the ordered source body",
        ));
    }
    if source_span.start < span.start
        || source_span.end > span.end
        || source_span.start >= source_span.end
    {
        return Err(invalid(
            source_span,
            "call occurrence is outside the ordered source body",
        ));
    }
    *last_node = Some(node_idx);
    if slot_seen.contains(&occurrence.definition.0) {
        return Err(invalid(
            occurrence.definition_span,
            "two call occurrences share one specialization slot",
        ));
    }
    slot_seen.push(occurrence.definition.0);
    let Some(module) = subject.source_dependencies.get(occurrence.module_index) else {
        return Err(invalid(
            occurrence.definition_span,
            "callee module is not retained",
        ));
    };
    let identity = callee_identity(occurrence, module);
    if !replay_seen(distinct, &identity) {
        if distinct.len() >= MAX_DISTINCT_CALLEES {
            return Err(budget(occurrence.definition_span));
        }
        distinct.push(identity);
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; out-of-order occurrences return typed Diagnostic"
)]
fn check_occurrences_order(
    subject: &super::SubjectBinding,
    body: &[noble_kernel::untrusted::NodeId],
    occurrences: &[usize],
    span: Span,
) -> Result<(), Diagnostic> {
    let mut slot_seen: Vec<u32> = Vec::with_capacity(occurrences.len());
    let mut distinct: Vec<String> = Vec::with_capacity(MAX_DISTINCT_CALLEES);
    let mut last_node: Option<usize> = None;
    let mut index = 0usize;
    let mut outcome = Ok(());
    while index < occurrences.len() {
        let occurrence = &subject.named_uses[occurrences[index]];
        outcome = check_one_occurrence_order(
            subject,
            body,
            occurrence,
            span,
            &mut last_node,
            &mut slot_seen,
            &mut distinct,
        );
        if outcome.is_err() {
            break;
        }
        index += 1;
    }
    outcome
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; rejects duplicate or untraced kernel specializations with typed diagnostics"
)]
fn check_accepted_definitions_traced(
    accepted: &noble_kernel::execution::Submission,
    selected_def: noble_kernel::contracts::Definition,
    named_uses: &[NamedUseOrigin],
    span: Span,
) -> Result<(), Diagnostic> {
    let mut seen: Vec<u32> = Vec::with_capacity(accepted.definitions.len());
    let mut declaration_at = 0;
    while declaration_at < accepted.definitions.len() {
        let definition = accepted.definitions[declaration_at].definition;
        if seen.contains(&definition.0)
            || (definition != selected_def
                && crate::scan::find_first(&named_uses[1..], |origin| {
                    origin.definition == definition
                })
                .is_none())
        {
            return Err(invalid(
                span,
                "accepted revision-3 graph holds an untraced specialization",
            ));
        }
        seen.push(definition.0);
        declaration_at += 1;
    }
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; malformed contracts return typed diagnostics"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; validates profile, kernel acceptance, occurrence ordering, and untraced defs sequentially"
)]
fn subject_view(goal: &ContractGoal, limits: Limits) -> Result<SubjectView, Diagnostic> {
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
    let (subject_inputs, subject_outputs) = collect_interface_codes(subject, span)?;
    if subject_inputs != input_codes || subject_outputs != output_codes {
        return Err(invalid(
            span,
            "subject interface differs from the declared contract",
        ));
    }
    if !subject.effects.is_empty() {
        return Err(unsupported_profile(span, "revision-3 subjects are pure"));
    }
    let (root, selected_idx) = validate_root_origin(subject, span)?;
    let selected = &subject.accepted_submission.definitions[selected_idx];
    let accepted = &subject.accepted_submission;
    if accepted.body.candidate.body != [root.candidate_node] {
        return Err(invalid(
            span,
            "accepted submission root is not the exact subject invocation",
        ));
    }
    let _accepted_check = super::checked_flat(
        &accepted.body,
        &accepted.request,
        &accepted.environment,
        span,
        MAX_ASSERTION_NODES,
    )?;
    let selected_request = noble_kernel::untrusted::Request {
        input_bytes: accepted.request.input_bytes,
        expected: selected.expected.clone(),
        limits: accepted.request.limits,
    };
    let checked = super::checked_flat(
        &selected.body,
        &selected_request,
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
    check_occurrences_order(subject, body, &occurrences, span)?;
    authenticate_occurrences(goal, &occurrences, limits)?;
    check_accepted_definitions_traced(accepted, selected.definition, &subject.named_uses, span)?;
    Ok(SubjectView {
        inputs,
        outputs,
        input_codes,
        output_codes,
        selected_idx,
        checked,
        occurrences,
    })
}

#[octet::sealed_enum]
enum ReplayAction {
    Dep(ProofDependency, u32),
    Local(ProofObligation, u32),
    Finish(String),
}

/// Replay the entire transitive call closure iteratively in topological post-order.
struct ReplayLocalSpec<'a> {
    obligation: &'a ProofObligation,
    contract: &'a ContractGoal,
    local: &'a [ProofObligation],
    identity: String,
    depth: u32,
}

fn collect_input_terms(count: usize) -> Vec<Term3> {
    let mut terms = Vec::with_capacity(count);
    let mut idx = 0usize;
    while idx < count {
        terms.push(Term3::Input(idx));
        idx = idx.saturating_add(1);
    }
    terms
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes local replay actions onto caller workstack"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; pushes local replay actions onto caller workstack"
)]
fn push_replay_local_plan(work: &mut Vec<ReplayAction>, plan: &[ProofObligation], next_depth: u32) {
    let mut li = plan.len();
    while li > 0 {
        let prev = li.saturating_sub(1);
        work.push(ReplayAction::Local(plan[prev].clone(), next_depth));
        li = prev;
    }
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes dependency replay actions onto caller workstack"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; pushes dependency replay actions onto caller workstack"
)]
fn push_replay_dep_plan(work: &mut Vec<ReplayAction>, plan: &[ProofDependency], next_depth: u32) {
    let mut di = plan.len();
    while di > 0 {
        let prev = di.saturating_sub(1);
        work.push(ReplayAction::Dep(plan[prev].clone(), next_depth));
        di = prev;
    }
}

fn collect_used_lemmas(names: &[String]) -> Vec<String> {
    let mut used = Vec::with_capacity(names.len());
    let mut ni = 0usize;
    while ni < names.len() {
        if !replay_seen(&used, &names[ni]) {
            used.push(names[ni].clone());
        }
        ni += 1;
    }
    used
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes new local/dep replay frames onto caller workstack"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; queues verified dependency closure actions onto `work` and charges `meter`"
)]
fn replay_local_rev3(
    spec: ReplayLocalSpec<'_>,
    work: &mut Vec<ReplayAction>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    if spec.depth > MAX_CLOSURE_DEPTH {
        return Err(budget(spec.obligation.span));
    }
    let outcome = verify_single(spec.contract, &[], spec.local, &spec.obligation.term, meter)?;
    let Some(next_depth) = spec.depth.checked_add(1) else {
        return Err(budget(spec.obligation.span));
    };
    work.push(ReplayAction::Finish(spec.identity));
    push_replay_local_plan(work, &outcome.local_plan, next_depth);
    push_replay_dep_plan(work, &outcome.plan, next_depth);
    Ok(())
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; pushes replayed identities, closure receipts, and pending actions onto caller vectors"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; updates replay tracking sets, workstack, and meter"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; keeps action, local obligations, replay/closure sets, workstack, and meter explicit"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; unsupported or cyclic replay dependencies return typed Diagnostic"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; handles dependency and local replay frames atomically"
)]
fn replay_step(
    action: ReplayAction,
    local: &[ProofObligation],
    replayed: &mut Vec<String>,
    closure: &mut Vec<String>,
    work: &mut Vec<ReplayAction>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    match action {
        ReplayAction::Finish(identity) => {
            if !replay_seen(closure, &identity) {
                closure.push(identity);
            }
        }
        ReplayAction::Dep(dependency, depth) => {
            let identity = replay_identity(&dependency);
            if replay_seen(replayed, &identity) {
                return Ok(());
            }
            if replayed.len() >= MAX_CLOSURE {
                return Err(budget(dependency.declaration.span));
            }
            replayed.push(identity.clone());
            match dependency.declaration.revision {
                1 => {
                    #[expect(
                        tigerstyle::fragile_exhaustive_enum_match,
                        reason = "Owner: noble-contracts; PendingGoal only has Pure and Contract variants in rev-1 replay"
                    )]
                    let has_contracts = match dependency.declaration.goal {
                        PendingGoal::Contract { .. } => true,
                        PendingGoal::Pure { .. } => false,
                    };
                    let (next, outcome) =
                        super::check_dependency(&dependency, has_contracts, *meter);
                    *meter = next;
                    outcome?;
                    closure.push(identity);
                }
                3 => {
                    if depth > MAX_CLOSURE_DEPTH {
                        return Err(budget(dependency.declaration.span));
                    }
                    let PendingGoal::Contract { contract } = &dependency.declaration.goal else {
                        return Err(invalid(
                            dependency.declaration.span,
                            "revision-3 dependency is not a contract",
                        ));
                    };
                    let outcome = verify_single(
                        contract,
                        &dependency.dependencies,
                        &[],
                        &dependency.declaration.term,
                        meter,
                    )?;
                    let Some(next_depth) = depth.checked_add(1) else {
                        return Err(budget(contract.contract_span));
                    };
                    work.push(ReplayAction::Finish(identity));
                    push_replay_local_plan(work, &outcome.local_plan, next_depth);
                    push_replay_dep_plan(work, &outcome.plan, next_depth);
                }
                _ => {
                    return Err(unsupported_profile(
                        dependency.declaration.span,
                        "closure member has an unsupported revision",
                    ));
                }
            }
        }
        ReplayAction::Local(obligation, depth) => {
            let mut identity = String::from("local/");
            identity.push_str(&obligation.name);
            if replay_seen(replayed, &identity) {
                return Ok(());
            }
            if replayed.len() >= MAX_CLOSURE {
                return Err(budget(obligation.span));
            }
            replayed.push(identity.clone());
            match &obligation.goal {
                PendingGoal::Pure { .. } => {
                    return Err(unsupported_profile(
                        obligation.span,
                        "local closure member is not a contract",
                    ))
                }
                PendingGoal::Contract { contract } => match contract.revision {
                    1 => {
                        let (next, outcome) = super::check_contract(&obligation, contract, *meter);
                        *meter = next;
                        outcome?;
                        closure.push(identity);
                    }
                    3 => replay_local_rev3(
                        ReplayLocalSpec {
                            obligation: &obligation,
                            contract,
                            local,
                            identity,
                            depth,
                        },
                        work,
                        meter,
                    )?,
                    _ => {
                        return Err(unsupported_profile(
                            obligation.span,
                            "local closure member has an unsupported revision",
                        ))
                    }
                },
            }
        }
    }
    Ok(())
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; drives replay checks over the closure workstack and shared meter"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; enforces MAX_CLOSURE and MAX_CLOSURE_DEPTH with typed diagnostics"
)]
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; `replayed`, `closure`, and `work` are bounded by MAX_CLOSURE and MAX_CLOSURE_DEPTH"
)]
fn replay_closure(
    root_plan: Vec<ProofDependency>,
    root_local_plan: Vec<ProofObligation>,
    local: &[ProofObligation],
    start_depth: u32,
    meter: &mut Meter<'_>,
) -> Result<Vec<String>, Diagnostic> {
    let mut replayed: Vec<String> = Vec::with_capacity(MAX_CLOSURE);
    let mut closure: Vec<String> = Vec::with_capacity(MAX_CLOSURE);
    let mut work: Vec<ReplayAction> = Vec::new();
    let mut idx = root_local_plan.len();
    while idx > 0 {
        let prev = idx.saturating_sub(1);
        work.push(ReplayAction::Local(
            root_local_plan[prev].clone(),
            start_depth,
        ));
        idx = prev;
    }
    let mut idx = root_plan.len();
    while idx > 0 {
        let prev = idx.saturating_sub(1);
        work.push(ReplayAction::Dep(root_plan[prev].clone(), start_depth));
        idx = prev;
    }
    let mut outcome = Ok(());
    while let Some(action) = work.pop() {
        outcome = replay_step(action, local, &mut replayed, &mut closure, &mut work, meter);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    Ok(closure)
}

struct SingleOutcome {
    envelope_base: Envelope,
    plan: Vec<ProofDependency>,
    local_plan: Vec<ProofObligation>,
    trace: LowerTrace,
}

/// Trace of one verified revision-3 proof used for Lean lowering.
struct CallTrace {
    pub occ_idx: usize,
    pub occ_slot: u32,
    pub callee_index: usize,
    pub callee_inputs: Vec<(String, Sort3)>,
    pub callee_outputs: Vec<(String, Sort3)>,
    pub pre_stack: Vec<Term3>,
    pub pre_facts: Vec<Prop3>,
    pub bridge_form: Form,
    pub callee_requires: Prop3,
    pub callee_ensures: Prop3,
    pub direct_outputs: Vec<Option<Term3>>,
    pub post_stack: Vec<Term3>,
    pub post_facts: Vec<Prop3>,
}

#[octet::sealed_enum]
enum TraceStep {
    Literal(noble_kernel::untrusted::Lit),
    Builtin(u32),
    Call(usize),
}

struct LowerTrace {
    pub view: SubjectView,
    pub requires: Prop3,
    pub ensures: Prop3,
    pub initial_facts: Vec<Prop3>,
    pub steps: Vec<TraceStep>,
    pub calls: Vec<CallTrace>,
    pub distinct_callees: Vec<ContractGoal>,
    pub lemmas: Vec<Lemma>,
    pub join_form: Form,
    pub final_stack: Vec<Term3>,
}

/// Verify one revision-3 contract proof body without recursive closure replay.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; charges verification work on `meter`"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid proof terms return typed diagnostics"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; validates slices, resolves lemmas/callees, executes body symbolically, checks join, and records trace"
)]
fn verify_single(
    goal: &ContractGoal,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    term: &Form,
    meter: &mut Meter<'_>,
) -> Result<SingleOutcome, Diagnostic> {
    let view = subject_view(goal, meter.limits)?;
    let subject = &goal.subject;
    let span = subject.source_span;
    let (slice_bridge, slice_reference) = match_slices(term, &view.occurrences, span)?;
    let callees = verify_callees(
        subject,
        &view.occurrences,
        &slice_reference,
        &view.checked,
        dependencies,
        local,
        span,
    )?;
    let mut names: Vec<String> = Vec::with_capacity(MAX_LEMMA_USES);
    lemma_names(term, &mut names)?;
    if names.len() > MAX_LEMMA_USES {
        return Err(budget(term.span));
    }
    let lemmas = resolved_lemmas(&names, dependencies, local, span, meter)?;
    let mut contracts: Vec<String> = Vec::with_capacity(callees.len());
    let mut ci = 0usize;
    while ci < callees.len() {
        contracts.push(callees[ci].reference.clone());
        ci += 1;
    }
    let mut initial_stack = Vec::with_capacity(view.input_codes.len());
    let mut in_idx = 0usize;
    while in_idx < view.input_codes.len() {
        initial_stack.push(Term3::Input(in_idx));
        in_idx += 1;
    }
    let mut state = State {
        stack: initial_stack,
        facts: Vec::with_capacity(MAX_FACTS),
        ghosts: 0,
    };
    let requires = assertion(&goal.requires, &view.inputs, &view.outputs, meter)?;
    if prop_mentions_output(&requires) {
        return Err(unsupported_profile(
            goal.requires.span,
            "caller precondition mentions an output",
        ));
    }
    add_fact(&mut state.facts, &requires)?;
    let initial_facts = state.facts.clone();
    let selected_def = &subject.accepted_submission.definitions[view.selected_idx];
    let context = WalkContext {
        nodes: &selected_def.body.candidate.nodes,
        named_uses: &subject.named_uses,
        occurrences: &view.occurrences,
        modules: &subject.source_dependencies,
        callees: &callees,
        bridges: &slice_bridge,
        contracts: &contracts,
        lemmas: &lemmas,
        dependencies,
        local,
        inputs: &view.inputs,
        outputs: &view.outputs,
        span,
    };
    let body = &selected_def.body.candidate.body;
    let mut accum = WalkAccum {
        records: Vec::with_capacity(view.occurrences.len()),
        plan: Vec::with_capacity(callees.len()),
        local_plan: Vec::with_capacity(callees.len()),
        distinct_callees: Vec::with_capacity(callees.len()),
        distinct_refs: Vec::with_capacity(callees.len()),
        steps: Vec::with_capacity(body.len()),
        calls: Vec::with_capacity(view.occurrences.len()),
    };
    let mut body_at = 0usize;
    let mut walk_outcome = Ok(());
    while body_at < body.len() {
        walk_outcome = walk_node(&mut state, context, body[body_at], &mut accum, meter);
        if walk_outcome.is_err() {
            break;
        }
        body_at += 1;
    }
    walk_outcome?;
    verify_final_outputs(
        &state.stack,
        &view.inputs,
        &view.outputs,
        &view.output_codes,
        span,
    )?;
    let ensures = assertion(&goal.ensures, &view.inputs, &view.outputs, meter)?;
    let final_inputs = collect_input_terms(view.input_codes.len());
    let _join_goal = normalize_prop(&instantiate_prop(&ensures, &final_inputs, &state.stack));
    let assignment = make_scope(&view.inputs, &view.outputs, &[], ScopeMode::Motive);
    let proof_scope = ProofScope {
        facts: &state.facts,
        lemmas: &lemmas,
        contracts: &contracts,
        assignment,
    };
    let join_form = join_target(term)?;
    if let Err(problem) = check_proof(join_form, &_join_goal, proof_scope, meter) {
        return Err(refined(
            join_form.span,
            Reason {
                code: "preuse-join-unproved",
                detail: &problem.message,
            },
        ));
    }
    let used_lemmas = collect_used_lemmas(&names);
    let final_stack = state.stack;
    let trace = LowerTrace {
        view,
        requires,
        ensures,
        initial_facts,
        steps: accum.steps,
        calls: accum.calls,
        distinct_callees: accum.distinct_callees,
        lemmas,
        join_form: join_form.clone(),
        final_stack,
    };
    Ok(SingleOutcome {
        envelope_base: Envelope {
            module: subject.module.clone(),
            version: subject.version,
            definition: subject.definition.clone(),
            contract: goal.contract_name.clone(),
            occurrences: accum.records,
            closure: Vec::new(),
            lemmas: used_lemmas,
        },
        plan: accum.plan,
        local_plan: accum.local_plan,
        trace,
    })
}

type CallSliceForms = (Vec<Option<Form>>, Vec<Option<String>>);

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; fills caller slice-indexed bridge and reference slots"
)]
fn match_one_slice(
    slice: &Form,
    slice_bridge: &mut [Option<Form>],
    slice_reference: &mut [Option<String>],
) -> Result<(), Diagnostic> {
    let (ordinal, reference, bridge) = call_slice(slice)?;
    if ordinal >= slice_bridge.len() {
        return Err(invalid(
            slice.span,
            "call slice does not match source order",
        ));
    }
    if slice_bridge[ordinal].is_some() {
        return Err(invalid(slice.span, "repeated call slice"));
    }
    slice_bridge[ordinal] = Some(bridge.clone());
    slice_reference[ordinal] = Some(String::from(reference));
    Ok(())
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; duplicate or out-of-range slices return typed Diagnostic"
)]
fn match_slices(
    term: &Form,
    occurrences: &[usize],
    _span: Span,
) -> Result<CallSliceForms, Diagnostic> {
    let mut slice_bridge: Vec<Option<Form>> = Vec::with_capacity(occurrences.len());
    let mut slice_reference: Vec<Option<String>> = Vec::with_capacity(occurrences.len());
    let mut init = 0;
    while init < occurrences.len() {
        slice_bridge.push(None);
        slice_reference.push(None);
        init += 1;
    }
    let slice_list = slices(term)?;
    let mut slice_at = 0usize;
    let mut outcome = Ok(());
    while slice_at < slice_list.len() {
        outcome = match_one_slice(
            &slice_list[slice_at],
            &mut slice_bridge,
            &mut slice_reference,
        );
        if outcome.is_err() {
            break;
        }
        slice_at += 1;
    }
    outcome?;
    Ok((slice_bridge, slice_reference))
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; validates one call occurrence against frozen callee metadata"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; occurrence/callee mismatches return typed Diagnostic"
)]
fn verify_one_callee(
    subject: &super::SubjectBinding,
    occurrence: &NamedUseOrigin,
    reference: Option<&str>,
    checked: &noble_kernel::untrusted::Checked,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    span: Span,
) -> Result<Callee, Diagnostic> {
    let Some(reference) = reference else {
        return Err(invalid(span, "missing call slice"));
    };
    let callee = resolve_callee(reference, dependencies, local, span)?;
    let Some(callee_goal) = callee_contract(callee.source, dependencies, local) else {
        return Err(invalid(span, "missing callee contract goal"));
    };
    if callee_goal.revision == 2 {
        return Err(unsupported_profile(
            occurrence.definition_span,
            "preuse-unsupported-callee: a revision-2 claim is not reusable here",
        ));
    }
    let module = &subject.source_dependencies[occurrence.module_index];
    if let Some(field) = callee_differs(occurrence, callee_goal, module) {
        let mut detail = String::from(field);
        detail.push_str(" differs from the frozen occurrence");
        return Err(refined(
            occurrence.definition_span,
            Reason {
                code: "preuse-callee-mismatch",
                detail: &detail,
            },
        ));
    }
    let Some(source_node) = occurrence.source_node else {
        return Err(invalid(span, "call occurrence has no derivation"));
    };
    let Ok(derivation_index) = usize::try_from(source_node) else {
        return Err(invalid(span, "call occurrence has no derivation"));
    };
    let Some(derivation) = checked.derivations.get(derivation_index) else {
        return Err(invalid(span, "call occurrence has no derivation"));
    };
    if derivation.interface.stack_in != callee_goal.subject.input_types
        || derivation.interface.stack_out != callee_goal.subject.output_types
        || derivation.interface.effects != noble_kernel::types::EffSet::empty()
    {
        return Err(refined(
            occurrence.definition_span,
            Reason {
                code: "preuse-callee-mismatch",
                detail:
                    "the caller derivation does not have the callee's complete ordered interface",
            },
        ));
    }
    if let Some(dependency) = callee_dependency(callee.source, dependencies) {
        if dependency.source != module.full_source {
            return Err(refined(
                occurrence.definition_span,
                Reason {
                    code: "preuse-callee-mismatch",
                    detail: "the proof module source differs from the retained callee module",
                },
            ));
        }
    }
    Ok(callee)
}

#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; resolves and validates every occurrence callee against frozen metadata"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; occurrence/callee mismatches return typed Diagnostic"
)]
fn verify_callees(
    subject: &super::SubjectBinding,
    occurrences: &[usize],
    slice_reference: &[Option<String>],
    checked: &noble_kernel::untrusted::Checked,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    span: Span,
) -> Result<Vec<Callee>, Diagnostic> {
    let mut callees: Vec<Callee> = Vec::with_capacity(occurrences.len());
    let mut outcome = Ok(());
    let mut ordinal = 0usize;
    while ordinal < occurrences.len() {
        let occ = &subject.named_uses[occurrences[ordinal]];
        let ref_opt = slice_reference[ordinal].as_deref();
        outcome = match verify_one_callee(subject, occ, ref_opt, checked, dependencies, local, span)
        {
            Ok(callee) => {
                callees.push(callee);
                Ok(())
            }
            Err(problem) => Err(problem),
        };
        if outcome.is_err() {
            break;
        }
        ordinal += 1;
    }
    outcome?;
    Ok(callees)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; mismatched final output arity or sort returns typed Diagnostic"
)]
fn verify_final_outputs(
    stack: &[Term3],
    inputs: &[(String, Sort3)],
    outputs: &[(String, Sort3)],
    output_codes: &[Sort3],
    span: Span,
) -> Result<(), Diagnostic> {
    if stack.len() != output_codes.len() {
        return Err(invalid(
            span,
            "final symbolic stack differs from the caller outputs",
        ));
    }
    let mut is_mismatch = false;
    let mut index = 0;
    while index < stack.len() && !is_mismatch {
        if sort_of(&stack[index], inputs, outputs, &[]) != Some(output_codes[index]) {
            is_mismatch = true;
        }
        index += 1;
    }
    if is_mismatch {
        return Err(invalid(
            span,
            "final symbolic stack differs from the caller outputs",
        ));
    }
    Ok(())
}

/// Verify one revision-3 caller proof and its complete transitive closure.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; advances the shared verification meter"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; preserves the original public verifier signature cleanly"
)]
pub fn verify(
    goal: &ContractGoal,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    term: &Form,
    depth: u32,
    meter: &mut Meter<'_>,
) -> Result<Envelope, Diagnostic> {
    if depth > MAX_CLOSURE_DEPTH {
        return Err(budget(goal.contract_span));
    }
    let outcome = verify_single(goal, dependencies, local, term, meter)?;
    let Some(start_depth) = depth.checked_add(1) else {
        return Err(budget(goal.subject.source_span));
    };
    let closure = replay_closure(outcome.plan, outcome.local_plan, local, start_depth, meter)?;
    let mut envelope = outcome.envelope_base;
    envelope.closure = closure;
    Ok(envelope)
}

/// The fixed inputs of one caller body walk.
#[derive(Clone, Copy)]
struct WalkContext<'a> {
    nodes: &'a [noble_kernel::untrusted::Node],
    named_uses: &'a [NamedUseOrigin],
    occurrences: &'a [usize],
    modules: &'a [super::ResolvedSourceModule],
    callees: &'a [Callee],
    bridges: &'a [Option<Form>],
    contracts: &'a [String],
    lemmas: &'a [Lemma],
    dependencies: &'a [ProofDependency],
    local: &'a [ProofObligation],
    inputs: &'a [(String, Sort3)],
    outputs: &'a [(String, Sort3)],
    span: Span,
}

struct WalkAccum {
    records: Vec<CallRecord>,
    plan: Vec<ProofDependency>,
    local_plan: Vec<ProofObligation>,
    distinct_callees: Vec<ContractGoal>,
    distinct_refs: Vec<String>,
    steps: Vec<TraceStep>,
    calls: Vec<CallTrace>,
}

fn find_occurrence_pos(
    named_uses: &[NamedUseOrigin],
    occurrences: &[usize],
    node_id: noble_kernel::untrusted::NodeId,
) -> Option<usize> {
    let mut occurrence_index = None;
    let mut occ_at = 0usize;
    while occurrence_index.is_none() && occ_at < occurrences.len() {
        if named_uses[occurrences[occ_at]].candidate_node.0 == node_id.0 {
            occurrence_index = Some(occ_at);
        }
        occ_at += 1;
    }
    occurrence_index
}

fn find_local_obligation<'a>(
    local: &'a [ProofObligation],
    name: &str,
) -> Option<&'a ProofObligation> {
    let mut idx = 0usize;
    let mut found = local.len();
    while found == local.len() && idx < local.len() {
        if obligation_has_name(&local[idx], name) {
            found = idx;
        }
        idx += 1;
    }
    if found < local.len() {
        Some(&local[found])
    } else {
        None
    }
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; records distinct callees in accumulated vector"
)]
fn find_or_insert_distinct_callee(
    accum: &mut WalkAccum,
    reference: &str,
    contract: &ContractGoal,
) -> usize {
    let mut callee_index = accum.distinct_refs.len();
    let mut ref_idx = 0;
    while callee_index == accum.distinct_refs.len() && ref_idx < accum.distinct_refs.len() {
        let is_matched = str_eq(&accum.distinct_refs[ref_idx], reference);
        callee_index = if is_matched { ref_idx } else { callee_index };
        ref_idx += 1;
    }
    if callee_index < accum.distinct_refs.len() {
        callee_index
    } else {
        let next = accum.distinct_callees.len();
        accum.distinct_refs.push(String::from(reference));
        accum.distinct_callees.push(contract.clone());
        next
    }
}

/// Walk one caller body position over the symbolic state.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; keeps literal, builtin, and call record bookkeeping together"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; advances the symbolic state, accumulated records/traces, and shared meter"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid body steps return typed diagnostics"
)]
fn walk_node(
    state: &mut State,
    context: WalkContext<'_>,
    node_id: noble_kernel::untrusted::NodeId,
    accum: &mut WalkAccum,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let span = context.span;
    let Ok(index) = usize::try_from(node_id.0) else {
        return Err(invalid(
            span,
            "revision-3 node id exceeds the address space",
        ));
    };
    let Some(node) = context.nodes.get(index) else {
        return Err(invalid(span, "revision-3 body node is absent"));
    };
    let occurrence_index = find_occurrence_pos(context.named_uses, context.occurrences, node_id);
    let Some(step) = classify(node, occurrence_index) else {
        return Err(unsupported_profile(
            span,
            "revision-3 body node is outside the pure vocabulary",
        ));
    };
    match step {
        Step::Literal(noble_kernel::untrusted::Lit::I64(value)) => {
            push(&mut state.stack, Term3::I64(value), span)?;
            accum
                .steps
                .push(TraceStep::Literal(noble_kernel::untrusted::Lit::I64(value)));
        }
        Step::Literal(noble_kernel::untrusted::Lit::Bool(value)) => {
            push(&mut state.stack, Term3::Bool(value), span)?;
            accum
                .steps
                .push(TraceStep::Literal(noble_kernel::untrusted::Lit::Bool(
                    value,
                )));
        }
        Step::Literal(noble_kernel::untrusted::Lit::Text)
        | Step::Literal(noble_kernel::untrusted::Lit::Unit) => {
            return Err(unsupported_profile(
                span,
                "revision-3 literal is not I64 or Bool",
            ));
        }
        Step::Builtin(word) => {
            builtin_step(&mut state.stack, word, span)?;
            accum.steps.push(TraceStep::Builtin(word));
        }
        Step::Call(occ_pos) => {
            let Some(&occ_idx) = context.occurrences.get(occ_pos) else {
                return Err(invalid(span, "call occurrence has no resolved callee"));
            };
            let Some(callee) = context.callees.get(occ_pos) else {
                return Err(invalid(span, "call occurrence has no resolved contract"));
            };
            let Some(Some(bridge_ref)) = context.bridges.get(occ_pos) else {
                return Err(invalid(span, "call occurrence has no bridge"));
            };
            let pre_stack = state.stack.clone();
            let pre_facts = state.facts.clone();
            let info = call_step(
                state,
                CallSite {
                    callee,
                    bridge: bridge_ref,
                    context,
                    span,
                },
                meter,
            )?;
            record_call_step(
                CallStepMeta {
                    occ_idx,
                    occ_pos,
                    callee,
                    bridge: bridge_ref.clone(),
                    info,
                    pre_stack,
                    pre_facts,
                },
                state,
                context,
                accum,
                span,
            )?;
        }
    }
    Ok(())
}

struct CallStepMeta<'a> {
    occ_idx: usize,
    occ_pos: usize,
    callee: &'a Callee,
    bridge: Form,
    info: CallStepInfo,
    pre_stack: Vec<Term3>,
    pre_facts: Vec<Prop3>,
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; pushes call record, replay plan entry, and lowering trace"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; missing retained module returns typed Diagnostic"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; updates call record, replay plan, and lower trace atomically"
)]
fn record_call_step(
    meta: CallStepMeta<'_>,
    state: &State,
    context: WalkContext<'_>,
    accum: &mut WalkAccum,
    span: Span,
) -> Result<(), Diagnostic> {
    let CallStepMeta {
        occ_idx,
        occ_pos,
        callee,
        bridge,
        info,
        pre_stack,
        pre_facts,
    } = meta;
    let occurrence = &context.named_uses[occ_idx];
    let Some(contract) = callee_contract(callee.source, context.dependencies, context.local) else {
        return Err(invalid(span, "missing callee contract goal"));
    };
    let mut inputs = Vec::with_capacity(info.callee_inputs.len());
    let mut ii = 0usize;
    while ii < info.callee_inputs.len() {
        inputs.push((
            info.callee_inputs[ii].0.clone(),
            info.callee_inputs[ii].1.word(),
        ));
        ii += 1;
    }
    let mut outputs = Vec::with_capacity(info.callee_outputs.len());
    let mut oi = 0usize;
    while oi < info.callee_outputs.len() {
        outputs.push((
            info.callee_outputs[oi].0.clone(),
            info.callee_outputs[oi].1.word(),
        ));
        oi += 1;
    }
    let Some(module) = context.modules.get(occurrence.module_index) else {
        return Err(invalid(span, "callee module is not retained"));
    };
    let Ok(ordinal) = u32::try_from(occ_pos) else {
        return Err(budget(span));
    };
    accum.records.push(CallRecord {
        ordinal,
        source_span: occurrence.source_span.unwrap_or(span),
        callee_module: module.module.clone(),
        callee_version: module.version,
        callee_definition: occurrence.definition_name.clone(),
        callee_ordinal: occurrence.definition_ordinal,
        callee_span: occurrence.definition_span,
        specialization: occurrence.definition.0,
        proof: callee.reference.clone(),
        inputs,
        outputs,
    });
    if let Some(dependency) = callee_dependency(callee.source, context.dependencies) {
        let identity = replay_identity(dependency);
        let mut is_seen = false;
        let mut idx = 0;
        while !is_seen && idx < accum.plan.len() {
            is_seen = str_eq(&replay_identity(&accum.plan[idx]), &identity);
            idx += 1;
        }
        if !is_seen {
            accum.plan.push(dependency.clone());
        }
    } else {
        let Some(local_item) = find_local_obligation(context.local, &callee.reference) else {
            return Err(invalid(span, "local callee proof is no longer staged"));
        };
        let mut is_seen = false;
        let mut idx = 0;
        while !is_seen && idx < accum.local_plan.len() {
            is_seen = str_eq(&accum.local_plan[idx].name, &local_item.name);
            idx += 1;
        }
        if !is_seen {
            accum.local_plan.push(local_item.clone());
        }
    }
    let callee_index = find_or_insert_distinct_callee(accum, &callee.reference, contract);
    let call_trace_index = accum.calls.len();
    accum.calls.push(CallTrace {
        occ_idx,
        occ_slot: occurrence.definition.0,
        callee_index,
        callee_inputs: info.callee_inputs,
        callee_outputs: info.callee_outputs,
        pre_stack,
        pre_facts,
        bridge_form: bridge,
        callee_requires: info.callee_requires,
        callee_ensures: info.callee_ensures,
        direct_outputs: info.direct_outputs,
        post_stack: state.stack.clone(),
        post_facts: state.facts.clone(),
    });
    accum.steps.push(TraceStep::Call(call_trace_index));
    Ok(())
}

/// Build the complete revision-3 preparation of one contract.
#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; matches public revision-3 API naming in the specification"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; verifies bounds and returns typed Diagnostic on invalid proof"
)]
pub fn prepare_named_v3(
    goal: &ContractGoal,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    term: &Form,
    limits: Limits,
) -> Result<PreparedV3, Diagnostic> {
    let Some(tree_bounds) = TreeBounds::new(limits) else {
        return Err(Diagnostic::new(
            DiagnosticKind::Exhausted,
            goal.contract_span,
            "revision-3 bounds exceed the host",
        ));
    };
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

/// Build the host-authenticated `NamedV3Obligation.lean` specification module for
/// one revision-3 contract goal without reading any proof term.
#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; matches public revision-3 API naming in the specification"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; verifies bounds and returns typed Diagnostic on invalid goal"
)]
pub fn prepare_named_v3_contract(
    goal: &ContractGoal,
    limits: Limits,
) -> Result<String, Diagnostic> {
    let Some(tree_bounds) = TreeBounds::new(limits) else {
        return Err(Diagnostic::new(
            DiagnosticKind::Exhausted,
            goal.contract_span,
            "revision-3 bounds exceed the host",
        ));
    };
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
    let view = subject_view(goal, limits)?;
    let requires = assertion(&goal.requires, &view.inputs, &view.outputs, &mut meter)?;
    if prop_mentions_output(&requires) {
        return Err(unsupported_profile(
            goal.requires.span,
            "caller precondition mentions an output",
        ));
    }
    let ensures = assertion(&goal.ensures, &view.inputs, &view.outputs, &mut meter)?;
    lower::obligation(goal, &view, &requires, &ensures, limits)
}

/// The contract-time check: the subject, its independent kernel acceptance and
/// its authenticated ordered occurrences.
pub fn claim_shape(goal: &ContractGoal, limits: Limits) -> Result<String, Diagnostic> {
    subject_view(goal, limits)?;
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

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; verifies the obligation, replays its closure, and lowers its proof term"
)]
fn checked(
    obligation: &ProofObligation,
    goal: &ContractGoal,
    dependencies: &[ProofDependency],
    local: &[ProofObligation],
    meter: &mut Meter<'_>,
) -> Result<CheckedProof, Diagnostic> {
    let outcome = verify_single(goal, dependencies, local, &obligation.term, meter)?;
    let start_depth = 1u32;
    let _closure = replay_closure(outcome.plan, outcome.local_plan, local, start_depth, meter)?;
    let term = lower::proof(goal, &outcome.trace, meter)?;
    Ok(CheckedProof {
        name: obligation.name.clone(),
        kind: ProofKind::NamedV3Contract,
        lean_term: term,
        claim: String::from("NamedV3Obligation.claim"),
        model_revision: String::new(),
        checker_revision: String::new(),
    })
}
