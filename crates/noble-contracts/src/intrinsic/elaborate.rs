//! Untrusted implicit-argument elaboration, revision `implicit-args/v1`.
//!
//! Elaboration is an authoring pass over the explicit source proof core. It
//! admits exactly three abbreviations, each behind the proof declaration's
//! `infer` opt-in:
//!
//! - A1: `(apply F (infer))` places a placeholder at a `Type0` or value
//!   binder of an exactly resolved application.
//! - A2: `(app H a1 … an)` applies an exactly resolved head with an implicit
//!   `Type0` prefix; every non-`Type0` binder is written, possibly as
//!   `(infer)`.
//! - A3: `(exec-literal (infer))` in a revision-1 contract rule takes the
//!   literal the trusted subject preparation extracts from the exact
//!   subject's first slice.
//!
//! The elaborator solves first-order equations over `PureTyCode` terms, value
//! levels and `Pi`/`Eq` propositions by syntactic unification with an occurs
//! check. It reports a unique expansion only when the most general unifier
//! assigns every metavariable a ground term whose free levels lie below the
//! metavariable's depth, and when every premise position holds an explicit
//! term. Otherwise it refuses at the abbreviation's span with a distinct
//! category (`underconstrained`, `invalid-constraint`, `ill-scoped`,
//! `unsupported`, `exhausted`, `internal`). It never searches the local
//! context, imports or an instance table, never defaults an unused `Type0`
//! binder, and never chooses among listed candidates.
//!
//! Elaboration is an untrusted producer: acceptance consumes only the explicit
//! term, which the existing source checker and the pinned Lean consumer
//! recheck independently. Nothing here is proof authority.
//!
//! Fourteen functions carry the Charon `opaque` annotation: the pinned Aeneas
//! refuses their bodies (nested borrows through the elaborator state, `(&Form, _)`
//! worklists and a borrow-joining `match`). They are reviewed extraction
//! boundaries modelled as uninterpreted in `NobleContractImpl/FunsExternal.lean`;
//! no extracted claim depends on what the elaborator returns, because only the
//! explicit term it produces is accepted, and only after rechecking.

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use super::{
    atom, code, deeper, head_atom, list, operation, proposition, shifted_prop, sort, stack_binder,
    Binder, Code, ContractGoal, Diagnostic, DiagnosticKind, Form, FormKind, Limits, Meter,
    MeterConfig, PendingGoal, ProofBudgets, ProofDependency, ProofObligation, ProofWork,
    Proposition, Sort, Span, TreeBounds, WorkStack,
};
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use alloc::{boxed::Box, string::String, vec::Vec};

/// `text![a, b, …]`: the concatenation of its `&str` parts, one `push_str`
/// each. Diagnostic and record text is built this way, never with `format!`:
/// the pinned Aeneas erases `format!`'s arguments, so the M4 extraction
/// refuses it (FORMAT-ERASURE), and a slice of borrowed parts is a nested
/// borrow the pinned Aeneas refuses in a loop.
macro_rules! text {
    ($first:expr $(, $part:expr)* $(,)?) => {{
        let mut text = String::from($first);
        $(text.push_str($part);)*
        text
    }};
}

/// The selected elaborator revision. Any behavior change that could alter an
/// expansion or a refusal must select a new revision.
pub const ELABORATION_REVISION: &str = "implicit-args/v1";

/// One argument the elaborator inserted or filled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InsertedArgument {
    /// The `(infer)` form or `app` head span that produced the insertion.
    pub span: Span,
    /// The telescope binder name, when the selected head declares one.
    pub binder: String,
    /// The binder's 1-based position in the head's telescope.
    pub position: u32,
    /// The exact selected head identity, or `local` for a pattern insertion.
    pub head: String,
    /// The solved code or value, rendered as core source.
    pub solution: String,
    /// The spans of the constraints that mentioned the solved metavariable.
    pub constraints: Vec<Span>,
}

/// The elaboration record of one abbreviated proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElaborationRecord {
    pub proof: String,
    pub revision: String,
    pub inserted: Vec<InsertedArgument>,
    /// The abbreviated proof term, rendered as core source.
    pub abbreviated: String,
    /// The fully explicit proof term, rendered as core source.
    pub explicit: String,
}

/// The result of elaborating one module's proof declarations.
pub struct Elaborated {
    pub obligations: Vec<ProofObligation>,
    pub records: Vec<ElaborationRecord>,
    pub work: ProofWork,
}

/// Elaborate every opted-in proof of `obligations` against the proofs its
/// earlier declarations and imported dependencies make available.
///
/// Non-opted-in proofs pass through unchanged unless they contain an
/// abbreviation, which is refused with the opt-in requirement as its named
/// diagnostic. A refusal never accepts, refutes or assumes anything.
#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; the name matches the established vocabulary used by callers; renaming it would churn call sites without clarifying them."
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
pub fn elaborate_obligations(
    module: &str,
    version: u32,
    obligations: &[ProofObligation],
    dependencies: &[ProofDependency],
    limits: Limits,
    budgets: ProofBudgets,
) -> Result<Elaborated, Diagnostic> {
    let config = MeterConfig {
        budgets,
        limits,
        tree_bounds: TreeBounds::new(limits)
            .ok_or_else(|| stage_exhausted(Span { start: 0, end: 0 }, "tree frame"))?,
    };
    let mut meter = Meter {
        nodes: 0,
        work: 0,
        normalization: 0,
        substitution: 0,
        config: &config,
    };
    let mut elaborated = Vec::with_capacity(obligations.len());
    let mut records = Vec::with_capacity(obligations.len());
    let mut at = 0usize;
    // Aeneas refuses early returns inside loops: the loop only accumulates the
    // first refusal, and one loop-free helper stages one obligation.
    let mut outcome: Result<(), Diagnostic> = Ok(());
    while outcome.is_ok() && at < obligations.len() {
        match stage_obligation(
            module,
            version,
            obligations,
            dependencies,
            at,
            limits,
            meter,
        ) {
            Ok((next, staged)) => {
                meter = next;
                match staged {
                    Some((obligation, record)) => {
                        elaborated.push(obligation);
                        records.push(record);
                    }
                    None => elaborated.push(obligations[at].clone()),
                }
            }
            Err(error) => outcome = Err(error),
        }
        at = at.saturating_add(1);
    }
    outcome?;
    Ok(Elaborated {
        obligations: elaborated,
        records,
        work: ProofWork {
            normalization: meter.normalization,
            substitution: meter.substitution,
            total: meter.work,
        },
    })
}

/// Stage one obligation: check its abbreviations and elaborate it when it
/// opted in. Returns the rewritten obligation and record, or the unchanged
/// pass-through as no staged pair.
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn stage_obligation<'m>(
    module: &str,
    version: u32,
    obligations: &[ProofObligation],
    dependencies: &[ProofDependency],
    at: usize,
    limits: Limits,
    meter: Meter<'m>,
) -> Result<(Meter<'m>, Option<(ProofObligation, ElaborationRecord)>), Diagnostic> {
    let obligation = &obligations[at];
    let abbreviation = scan_abbreviation(&obligation.term, limits)?;
    if !obligation.elaboration {
        if abbreviation.infer {
            let span = find_infer_span(&obligation.term, limits)?.unwrap_or(obligation.term.span);
            return Err(unsupported(
                span,
                "infer requires the infer elaboration opt-in",
            ));
        }
        if abbreviation.app {
            return Err(unsupported(
                obligation.term.span,
                "app requires the infer elaboration opt-in",
            ));
        }
        return Ok((meter, None));
    }
    scan_positions(&obligation.term, limits)?;
    if !abbreviation.infer && !abbreviation.app {
        return Ok((meter, None));
    }
    let mut available = dependency_proofs(dependencies);
    let mut prior = 0usize;
    while prior < at {
        push_available(
            &mut available,
            identity_text(module, version, &obligations[prior].name),
            obligations[prior].name.clone(),
            &obligations[prior].goal,
        );
        prior = prior.saturating_add(1);
    }
    match &obligation.goal {
        PendingGoal::Pure {
            proposition: statement,
        } if obligation.revision == 1 => {
            let (explicit, record, next_meter) =
                pure(obligation, statement, &available, limits, meter)?;
            let mut rewritten = obligation.clone();
            rewritten.term = explicit;
            Ok((next_meter, Some((rewritten, record))))
        }
        PendingGoal::Contract { contract } => {
            let (explicit, record) = contract_rule(obligation, contract, limits)?;
            let mut rewritten = obligation.clone();
            rewritten.term = explicit;
            Ok((meter, Some((rewritten, record))))
        }
        PendingGoal::Pure { .. } => Err(unsupported(
            obligation.term.span,
            "inference requires a revision-1 pure proof",
        )),
    }
}

/// Every imported dependency as an available proof, under its own visible
/// name (alias-qualified for an import, e.g. `alias.poly-refl`), exactly as
/// the explicit checker resolves `use`; its declaration name only enters the
/// identity.
fn dependency_proofs(dependencies: &[ProofDependency]) -> Vec<Available> {
    let mut available = Vec::new();
    let mut dependency_at = 0usize;
    while dependency_at < dependencies.len() {
        let dependency = &dependencies[dependency_at];
        push_available(
            &mut available,
            identity_text(
                &dependency.module,
                dependency.version,
                &dependency.declaration.name,
            ),
            dependency.name.clone(),
            &dependency.declaration.goal,
        );
        dependency_at = dependency_at.saturating_add(1);
    }
    available
}

/// Record one pure proposition goal as an available proof.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the helper appends to the caller's list with Vec::push, so it needs the growable vector, not a slice."
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
fn push_available(
    available: &mut Vec<Available>,
    identity: String,
    name: String,
    goal: &PendingGoal,
) {
    if let PendingGoal::Pure { proposition } = goal {
        available.push(Available {
            identity,
            name,
            names: pi_names(proposition),
            goal: proposition.clone(),
        });
    }
}

/// A proof an abbreviated term may name through `(use …)`.
struct Available {
    identity: String,
    /// The local proof name the checker's `use` resolution matches exactly.
    name: String,
    names: Vec<String>,
    goal: Form,
}

/// The abbreviations a proof term contains, after position checking.
struct Abbreviation {
    infer: bool,
    app: bool,
}

/// Elaborate one revision-1 pure proof.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn pure<'m>(
    obligation: &ProofObligation,
    statement: &Form,
    available: &[Available],
    limits: Limits,
    mut meter: Meter<'m>,
) -> Result<(Form, ElaborationRecord, Meter<'m>), Diagnostic> {
    let (next, parsed) = proposition(statement, &WorkStack::new(), meter, 0);
    meter = next;
    let goal = parsed?;
    let goal_term = prop_term(&goal, obligation.span)?;
    let mut elaborator = Elaborator {
        meter,
        available,
        context: WorkStack::new(),
        context_spans: WorkStack::new(),
        metas: Vec::new(),
        deferred: Vec::new(),
        deferred_forms: Vec::new(),
    };
    let outcome = elaborator.run(&obligation.term, &goal_term);
    let (explicit, inserted) = outcome?;
    let meter = elaborator.meter;
    if inserted.is_empty() {
        return Err(internal(
            obligation.term.span,
            "an opted-in abbreviated proof inserted no argument",
        ));
    }
    let record = ElaborationRecord {
        proof: obligation.name.clone(),
        revision: String::from(ELABORATION_REVISION),
        inserted,
        abbreviated: render_form(&obligation.term, limits)?,
        explicit: render_form(&explicit, limits)?,
    };
    Ok((explicit, record, meter))
}

/// Elaborate the structural literal of a revision-1 contract rule.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn contract_rule(
    obligation: &ProofObligation,
    contract: &ContractGoal,
    limits: Limits,
) -> Result<(Form, ElaborationRecord), Diagnostic> {
    if obligation.revision != 1 || contract.revision != 1 {
        return Err(unsupported(
            obligation.term.span,
            "template inference is limited to the revision-1 exec-literal rule",
        ));
    }
    let (_, literal) = super::prepare_checked_contract(contract, limits)?;
    let literal = literal.ok_or_else(|| {
        unsupported(
            obligation.term.span,
            "exec-literal inference requires an exact subject literal",
        )
    })?;
    let head = identity_text(
        &contract.subject.module,
        contract.subject.version,
        &contract.contract_name,
    );
    let mut inserted = Vec::new();
    let explicit = rewrite_exec_literal(
        &obligation.term,
        literal,
        contract.contract_span,
        &head,
        &mut inserted,
        limits,
        0,
    )?;
    if inserted.is_empty() {
        return Err(internal(
            obligation.term.span,
            "an opted-in contract rule inserted no literal",
        ));
    }
    let record = ElaborationRecord {
        proof: obligation.name.clone(),
        revision: String::from(ELABORATION_REVISION),
        inserted,
        abbreviated: render_form(&obligation.term, limits)?,
        explicit: render_form(&explicit, limits)?,
    };
    Ok((explicit, record))
}

/// Replace every `(exec-literal (infer))` with the exact subject literal.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the rewrite appends each inserted literal to the caller's record list with Vec::push, so it needs the growable vector, not a slice."
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn rewrite_exec_literal(
    form: &Form,
    literal: i64,
    origin: Span,
    head: &str,
    inserted: &mut Vec<InsertedArgument>,
    limits: Limits,
    depth: u32,
) -> Result<Form, Diagnostic> {
    let bound = scan_bound(limits, form.span)?;
    let rule = LiteralRule {
        literal,
        origin,
        limits,
        bound,
    };
    let mut pending = WorkStack::new();
    let mut values = Vec::new();
    pending
        .push_bounded(RewriteFrame::Enter(form, depth), bound)
        .map_err(|()| stage_exhausted(form.span, "tree frame"))?;
    let mut outcome = Ok(());
    while let Some(frame) = pending.pop_bounded() {
        outcome = rewrite_step(frame, rule, head, inserted, &mut pending, &mut values);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    match values.pop() {
        Some(rewritten) => Ok(rewritten),
        None => Err(internal(form.span, "exec-literal rewrite produced no term")),
    }
}

/// The fixed inputs of one exec-literal rewrite: the subject literal, the
/// contract span the inserted argument names, and the bounds.
#[derive(Clone, Copy)]
struct LiteralRule {
    literal: i64,
    origin: Span,
    limits: Limits,
    bound: usize,
}

/// One frame of the exec-literal rewrite: a form to rewrite at its depth, or a
/// list's items with the next item, the items' depth and the list's span.
#[octet::sealed_enum]
enum RewriteFrame<'a> {
    Enter(&'a Form, u32),
    Children(&'a [Form], usize, u32, Span),
}

/// Rewrite one frame: an atom or a placeholder site becomes one value; a list
/// schedules its next item, or gathers its rewritten items into one value once
/// every item is rewritten. A frame walk, not a recursion inside a loop, which
/// the pinned Lean cannot prove monotone.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the step appends inserted literals and finished values with Vec::push, so it needs the caller's growable vectors, not slices."
)]
fn rewrite_step<'a>(
    frame: RewriteFrame<'a>,
    rule: LiteralRule,
    head: &str,
    inserted: &mut Vec<InsertedArgument>,
    pending: &mut WorkStack<RewriteFrame<'a>>,
    values: &mut Vec<Form>,
) -> Result<(), Diagnostic> {
    match frame {
        RewriteFrame::Enter(form, depth) => {
            if depth > rule.limits.depth {
                return Err(stage_exhausted(form.span, "depth"));
            }
            match &form.kind {
                FormKind::Atom(_) => {
                    values.push(form.clone());
                    Ok(())
                }
                FormKind::List(items) => {
                    if is_exec_literal_infer(items) {
                        let text = signed_text(rule.literal);
                        inserted.push(InsertedArgument {
                            span: items[1].span,
                            binder: String::from("exec-literal"),
                            position: 1,
                            head: String::from(head),
                            solution: text.clone(),
                            constraints: alloc::vec![rule.origin],
                        });
                        values.push(Form {
                            kind: FormKind::List(alloc::vec![
                                items[0].clone(),
                                Form {
                                    kind: FormKind::Atom(text),
                                    span: items[1].span,
                                },
                            ]),
                            span: form.span,
                        });
                        return Ok(());
                    }
                    let next = deeper(depth, form.span)?;
                    pending
                        .push_bounded(
                            RewriteFrame::Children(items, 0, next, form.span),
                            rule.bound,
                        )
                        .map_err(|()| stage_exhausted(form.span, "tree frame"))
                }
            }
        }
        // `index` is the next item; past the last, the list's rewritten items
        // are the top of the value stack, in order.
        RewriteFrame::Children(items, index, depth, span) => {
            if let Some(item) = items.get(index) {
                pending
                    .push_bounded(
                        RewriteFrame::Children(items, index.saturating_add(1), depth, span),
                        rule.bound,
                    )
                    .map_err(|()| stage_exhausted(span, "tree frame"))?;
                return pending
                    .push_bounded(RewriteFrame::Enter(item, depth), rule.bound)
                    .map_err(|()| stage_exhausted(span, "tree frame"));
            }
            let Some(at) = values.len().checked_sub(items.len()) else {
                return Err(internal(span, "rewritten list lost its items"));
            };
            let rewritten = values.split_off(at);
            values.push(Form {
                kind: FormKind::List(rewritten),
                span,
            });
            Ok(())
        }
    }
}

/// A parsed proposition-level term over metavariable codes and values.
#[octet::sealed_enum]
enum Ty {
    Unit,
    Bool,
    I64,
    Text,
    Var(u32),
    Pair(Box<Ty>, Box<Ty>),
    Sum(Box<Ty>, Box<Ty>),
    List(Box<Ty>),
    Meta(u32),
}

/// A value position: a concrete level or a value metavariable.
#[octet::sealed_enum]
#[derive(Clone, Copy)]
enum Val {
    Level(u32),
    Meta(u32),
}

/// A sort over metavariable terms.
#[octet::sealed_enum]
enum SortTerm {
    Codes,
    Value(Ty),
    Premise(Box<PropTerm>),
}

/// A proposition over metavariable terms.
#[octet::sealed_enum]
enum PropTerm {
    Pi(u32, SortTerm, Box<PropTerm>),
    Eq(Ty, Val, Val),
}

/// The argument an eliminated binder is replaced with.
#[octet::sealed_enum]
enum ArgTerm {
    /// A code argument, possibly containing metavariables.
    Code(Ty),
    Value(Val),
    Premise,
}

/// What kind of solution a metavariable records.
#[octet::sealed_enum]
#[derive(Clone, Copy, PartialEq)]
enum MetaKind {
    Code,
    Value,
}

/// One metavariable and its solution.
struct Meta {
    kind: MetaKind,
    /// A value metavariable's required code.
    required: Option<Ty>,
    code: Option<Ty>,
    value: Option<Val>,
    depth: u32,
    span: Span,
    binder: String,
    position: u32,
    head: String,
    /// The proof-context binder names at allocation, by level. A solution may
    /// name only levels below this depth, so these names render it exactly.
    names: Vec<String>,
    /// The proof-context binder sorts at allocation, by level, for the
    /// value-metacode rule when a later constraint solves the meta.
    sorts: Vec<Sort>,
    constraints: Vec<Span>,
}

/// A premise argument whose explicit check waits for the metavariable
/// solution.
struct Deferred {
    form: Form,
    expected: PropTerm,
    depth: u32,
}

/// An explicit term under construction.
#[octet::sealed_enum]
enum Out {
    /// An explicit subterm, already complete.
    Done(Form),
    /// A list whose items may still need resolution.
    List(Vec<Out>, Span),
    /// A deferred premise check, resolved after solving.
    Hole(usize, Span),
    /// An inferred argument, resolved after solving.
    Infer(u32, Span),
}

/// One frame of finishing an `Out` forest (the deferred premise forms, then
/// the root). Every frame carries the forest, so every borrow a step schedules
/// derives from the popped frame alone, as in the scans: the pinned Aeneas
/// refuses a loop that schedules borrows taken from a separate parameter.
#[octet::sealed_enum]
enum FinishFrame<'o> {
    /// The forest, the entry at a position in it, its depth and the span that
    /// asked for it.
    Entry(&'o [Out], usize, u32, Span),
    /// The forest and a node to finish at its depth.
    Enter(&'o [Out], &'o Out, u32),
    /// The forest, a list's items, the next item, the items' depth and the
    /// list's span.
    Children(&'o [Out], &'o [Out], usize, u32, Span),
}

/// The source span an output node reports.
const fn out_span(out: &Out) -> Span {
    match out {
        Out::Done(form) => form.span,
        Out::List(_, span) | Out::Hole(_, span) | Out::Infer(_, span) => *span,
    }
}

/// Finish the forest from its root, the entry just past the deferred forms.
/// A frame walk with a value stack and no elaborator: the pinned Aeneas
/// refuses a recursion inside a loop, and a loop whose frames mix the
/// elaborator's borrows with the walk's.
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the deferred-form count and the frame bound are positional and named at the single call site"
)]
fn finish_forest(
    forest: &[Out],
    deferred: usize,
    solutions: &[Result<Form, Diagnostic>],
    bound: usize,
    depth: u32,
    span: Span,
) -> Result<Form, Diagnostic> {
    let mut pending = WorkStack::new();
    let mut values = Vec::new();
    pending
        .push_bounded(FinishFrame::Entry(forest, deferred, depth, span), bound)
        .map_err(|()| stage_exhausted(span, "tree frame"))?;
    let mut outcome = Ok(());
    while let Some(frame) = pending.pop_bounded() {
        outcome = finish_step(frame, deferred, solutions, bound, &mut pending, &mut values);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    match values.pop() {
        Some(form) => Ok(form),
        None => Err(internal(span, "finishing produced no explicit term")),
    }
}

/// Finish one frame. A complete subterm or a solved argument becomes one
/// value; an entry or a hole schedules its forest form; a list schedules its
/// next item, or gathers its finished items into one value once every item is
/// finished.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the deferred-form count and the frame bound are positional and named at the single call site"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the step pushes finished forms onto the caller's value stack, so it needs the growable vector, not a slice."
)]
fn finish_step<'o>(
    frame: FinishFrame<'o>,
    deferred: usize,
    solutions: &[Result<Form, Diagnostic>],
    bound: usize,
    pending: &mut WorkStack<FinishFrame<'o>>,
    values: &mut Vec<Form>,
) -> Result<(), Diagnostic> {
    match frame {
        FinishFrame::Entry(forest, position, depth, span) => {
            let Some(form) = forest.get(position) else {
                return Err(internal(span, "unresolved deferred premise"));
            };
            pending
                .push_bounded(FinishFrame::Enter(forest, form, depth), bound)
                .map_err(|()| stage_exhausted(span, "tree frame"))
        }
        FinishFrame::Enter(_, Out::Done(form), _) => {
            values.push(form.clone());
            Ok(())
        }
        FinishFrame::Enter(forest, Out::List(items, span), depth) => {
            let next = deeper(depth, *span)?;
            pending
                .push_bounded(FinishFrame::Children(forest, items, 0, next, *span), bound)
                .map_err(|()| stage_exhausted(*span, "tree frame"))
        }
        // A hole names a deferred premise form, never the root past them.
        FinishFrame::Enter(forest, Out::Hole(position, span), depth) => {
            if *position >= deferred {
                return Err(internal(*span, "unresolved deferred premise"));
            }
            pending
                .push_bounded(FinishFrame::Entry(forest, *position, depth, *span), bound)
                .map_err(|()| stage_exhausted(*span, "tree frame"))
        }
        FinishFrame::Enter(_, Out::Infer(meta, span), _) => {
            let index = meta_index(*meta, *span)?;
            let Some(solution) = solutions.get(index) else {
                return Err(internal(*span, "unknown metavariable"));
            };
            match solution {
                Ok(form) => {
                    values.push(form.clone());
                    Ok(())
                }
                Err(error) => Err(error.clone()),
            }
        }
        // `index` is the next item; past the last, the list's finished items
        // are the top of the value stack, in order.
        FinishFrame::Children(forest, items, index, depth, span) => {
            if let Some(item) = items.get(index) {
                pending
                    .push_bounded(
                        FinishFrame::Children(forest, items, index.saturating_add(1), depth, span),
                        bound,
                    )
                    .map_err(|()| stage_exhausted(span, "tree frame"))?;
                return pending
                    .push_bounded(FinishFrame::Enter(forest, item, depth), bound)
                    .map_err(|()| stage_exhausted(span, "tree frame"));
            }
            let Some(at) = values.len().checked_sub(items.len()) else {
                return Err(internal(span, "finished list lost its items"));
            };
            let forms = values.split_off(at);
            values.push(Form {
                kind: FormKind::List(forms),
                span,
            });
            Ok(())
        }
    }
}

/// A term whose proposition was inferred.
struct Inferred {
    prop: PropTerm,
    out: Out,
    head: Head,
}

/// The selected head of an application spine.
struct Head {
    identity: String,
    names: Vec<String>,
    next: u32,
}

impl Head {
    /// The next telescope binder's name and position.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper clones a String and renders a decimal text, neither of which is const"
    )]
    fn binder(&self) -> (String, u32) {
        let position = self.next;
        // Aeneas refuses function-definition types with regions; avoid
        // function items and closure chains in place of a plain match.
        let name = match usize::try_from(position) {
            Ok(index) => match self.names.get(index) {
                Some(name) => name.clone(),
                None => text!["#", &u32_text(position)],
            },
            Err(_) => text!["#", &u32_text(position)],
        };
        (name, position)
    }
}

/// The elaboration state of one proof obligation.
struct Elaborator<'m, 'a> {
    meter: Meter<'m>,
    available: &'a [Available],
    context: WorkStack<Binder>,
    /// The source span of each proof-context binder, by level.
    context_spans: WorkStack<Span>,
    metas: Vec<Meta>,
    deferred: Vec<Option<Deferred>>,
    deferred_forms: Vec<Out>,
}

/// One charged meter operation.
#[octet::sealed_enum]
#[derive(Clone, Copy)]
enum MeterStep {
    Visit(u32),
    Charge(usize),
    Normalize(usize),
    Substitute(usize),
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the methods mutate only the elaborator's own metavariable table, deferred queue, binder context and meter for one bounded elaboration; the borrowed proof terms and dependency snapshot stay immutable."
)]
impl Elaborator<'_, '_> {
    /// Elaborate `term` against `goal`, then finish the obligation: resolve
    /// deferred premise checks and verify every metavariable is ground.
    fn run(
        &mut self,
        term: &Form,
        goal: &PropTerm,
    ) -> Result<(Form, Vec<InsertedArgument>), Diagnostic> {
        let out = self.check(term, goal, 0)?;
        self.finish_deferred()?;
        self.verify_ground()?;
        let explicit = self.finish_out(out, 0)?;
        let mut inserted = Vec::with_capacity(self.metas.len());
        let mut index = 0usize;
        let mut outcome: Result<(), Diagnostic> = Ok(());
        while outcome.is_ok() && index < self.metas.len() {
            match self.inserted_row(index) {
                Ok(row) => inserted.push(row),
                Err(error) => outcome = Err(error),
            }
            index = index.saturating_add(1);
        }
        outcome?;
        Ok((explicit, inserted))
    }

    /// Resolve deferred premise checks in insertion order. Each check may add
    /// further deferred checks; the queue drains deterministically.
    fn finish_deferred(&mut self) -> Result<(), Diagnostic> {
        let mut at = 0usize;
        let mut outcome: Result<(), Diagnostic> = Ok(());
        while outcome.is_ok() && at < self.deferred.len() {
            outcome = self.deferred_step(at);
            at = at.saturating_add(1);
        }
        outcome
    }

    /// One deferred step: check the queued premise and resolve its hole.
    fn deferred_step(&mut self, at: usize) -> Result<(), Diagnostic> {
        let Some(item) = self.deferred[at].take() else {
            return Err(internal(
                Span { start: 0, end: 0 },
                "deferred premise processed twice",
            ));
        };
        let out = self.check(&item.form, &item.expected, item.depth)?;
        self.deferred_forms.push(out);
        Ok(())
    }

    /// Refuse unless every metavariable is solved by a ground, well-scoped
    /// term; a value metavariable must be a level below its depth.
    ///
    /// The receiver is exclusive although nothing is written: the pinned
    /// Aeneas fails internally on a loop that calls a method through a shared
    /// borrow of the elaborator, whose own fields hold borrows.
    fn verify_ground(&mut self) -> Result<(), Diagnostic> {
        let mut index = 0usize;
        let mut outcome: Result<(), Diagnostic> = Ok(());
        while outcome.is_ok() && index < self.metas.len() {
            match self.ground_step(index) {
                Ok(()) => {}
                Err(error) => outcome = Err(error),
            }
            index = index.saturating_add(1);
        }
        outcome
    }

    /// One metavariable's groundness and scope check.
    fn ground_step(&self, index: usize) -> Result<(), Diagnostic> {
        let meta = &self.metas[index];
        let level = u32_level(index, meta.span)?;
        match meta.kind {
            MetaKind::Code => {
                let solved = self.resolve_ty(&Ty::Meta(level), meta.span)?;
                if let Ty::Meta(_) = solved {
                    return Err(self.underconstrained(index));
                }
                match self.scope_ty(&solved, meta.depth, meta.span) {
                    Ok(()) => Ok(()),
                    // A fresh borrow: none is held across the call's match.
                    Err(error) => Err(scope_refusal(&self.metas[index], error)),
                }
            }
            MetaKind::Value => {
                let solved = self.resolve_val(Val::Meta(level), meta.span)?;
                match solved {
                    Val::Meta(_) => Err(self.underconstrained(index)),
                    Val::Level(found) => {
                        if found >= meta.depth {
                            return Err(scope_refusal(
                                meta,
                                ill_scoped(
                                    meta.span,
                                    &text![
                                        "solution for `",
                                        &meta.binder,
                                        "` names level ",
                                        &u32_text(found),
                                        " at depth ",
                                        &u32_text(meta.depth),
                                    ],
                                ),
                            ));
                        }
                        Ok(())
                    }
                }
            }
        }
    }

    /// The record row of one metavariable.
    fn inserted_row(&self, index: usize) -> Result<InsertedArgument, Diagnostic> {
        let meta = &self.metas[index];
        let level = u32_level(index, meta.span)?;
        let solution = match meta.kind {
            MetaKind::Code => {
                let solved = self.resolve_ty(&Ty::Meta(level), meta.span)?;
                self.render_ty(&solved, meta.span, &meta.names)?
            }
            MetaKind::Value => {
                let solved = self.resolve_val(Val::Meta(level), meta.span)?;
                self.render_val(solved, meta.span, &meta.names)?
            }
        };
        Ok(InsertedArgument {
            span: meta.span,
            binder: meta.binder.clone(),
            position: meta.position.saturating_add(1),
            head: meta.head.clone(),
            solution,
            constraints: meta.constraints.clone(),
        })
    }

    /// The refusal for a metavariable no constraint solves.
    fn underconstrained(&self, index: usize) -> Diagnostic {
        underconstrained_refusal(&self.metas[index])
    }

    /// Allocate a fresh code metavariable for the head's next binder.
    #[expect(
        tigerstyle::ambiguous_params,
        reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
    )]
    fn alloc_code(
        &mut self,
        binder: &str,
        position: u32,
        head: &str,
        span: Span,
        depth: u32,
    ) -> Result<u32, Diagnostic> {
        let (names, sorts) = self.binder_snapshot();
        self.metas.push(Meta {
            kind: MetaKind::Code,
            required: None,
            code: None,
            value: None,
            depth,
            span,
            binder: String::from(binder),
            position,
            head: String::from(head),
            names,
            sorts,
            constraints: Vec::new(),
        });
        u32_level(self.metas.len().saturating_sub(1), span)
    }

    /// Allocate a fresh value metavariable with `required` as its code.
    #[expect(
        tigerstyle::ambiguous_params,
        reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
    )]
    #[expect(
        tigerstyle::too_many_parameters,
        reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
    )]
    fn alloc_value(
        &mut self,
        binder: &str,
        position: u32,
        head: &str,
        required: Ty,
        span: Span,
        depth: u32,
    ) -> Result<u32, Diagnostic> {
        let (names, sorts) = self.binder_snapshot();
        self.metas.push(Meta {
            kind: MetaKind::Value,
            required: Some(required),
            code: None,
            value: None,
            depth,
            span,
            binder: String::from(binder),
            position,
            head: String::from(head),
            names,
            sorts,
            constraints: Vec::new(),
        });
        u32_level(self.metas.len().saturating_sub(1), span)
    }

    /// The current proof-context binder names and sorts, by level.
    fn binder_snapshot(&self) -> (Vec<String>, Vec<Sort>) {
        context_snapshot(&self.context)
    }

    /// Charge one meter operation, naming the stage and the exceeded budget
    /// when it exhausts.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper calls the non-const meter transitions and meter_exhausted, so the const suggestion does not apply"
    )]
    fn meter_step(
        &mut self,
        step: MeterStep,
        stage: &'static str,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let (next, result) = match step {
            MeterStep::Visit(depth) => self.meter.visit(depth, span),
            MeterStep::Charge(amount) => self.meter.charge(amount, span),
            MeterStep::Normalize(amount) => self.meter.normalize(amount, span),
            MeterStep::Substitute(amount) => self.meter.substitute(amount, span),
        };
        self.meter = next;
        match result {
            Ok(()) => Ok(()),
            Err(error) => {
                if error.kind == DiagnosticKind::Exhausted {
                    let budget = exceeded_budget(&self.meter, step);
                    Err(meter_exhausted(span, stage, budget))
                } else {
                    Err(error)
                }
            }
        }
    }

    fn push_binder(&mut self, binder: Binder, span: Span) -> Result<(), Diagnostic> {
        let bound = self.meter.tree_bounds.frames;
        self.context
            .push_bounded(binder, bound)
            .map_err(|()| stage_exhausted(span, "tree frame"))?;
        // The binder's source span, at the same level, for candidate rows.
        self.context_spans
            .push_bounded(span, bound)
            .map_err(|()| stage_exhausted(span, "tree frame"))
    }

    fn pop_binder(&mut self) {
        self.context.pop_bounded();
        self.context_spans.pop_bounded();
    }
}

/// Render a level index as the metavariable identifier.
fn u32_level(index: usize, span: Span) -> Result<u32, Diagnostic> {
    u32::try_from(index).map_err(|_| internal(span, "metavariable index exceeds 32 bits"))
}

/// The binder at level `index` of a proof context. Free functions over the
/// context, not methods of the elaborator, and owned copies out of them: the
/// pinned Aeneas refuses a borrow returned through the elaborator, whose own
/// fields hold borrows, and a borrow that a loop body keeps.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads the spill vector through the non-const Vec::get"
)]
fn binder_at(context: &WorkStack<Binder>, index: usize) -> Option<&Binder> {
    let inline = context.inline_len;
    if index < inline {
        context.inline[index].as_ref()
    } else {
        context.spill.get(index.saturating_sub(inline))
    }
}

/// An owned copy of the name and sort of the binder at level `index`.
fn binder_parts(context: &WorkStack<Binder>, index: usize) -> Option<(String, Sort)> {
    let binder = binder_at(context, index)?;
    Some((binder.name.clone(), binder.sort.clone()))
}

/// The source span of the binder at level `index`, if recorded.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads the spill vector through the non-const Vec::get"
)]
fn span_at(spans: &WorkStack<Span>, index: usize) -> Option<Span> {
    let inline = spans.inline_len;
    if index < inline {
        return spans.inline[index];
    }
    // A plain assignment rather than `Option::copied`/`map`, which the
    // pinned Aeneas models without the region-free function type it needs.
    let mut found = None;
    if let Some(span) = spans.spill.get(index.saturating_sub(inline)) {
        found = Some(*span);
    }
    found
}

/// Every binder name and sort of a proof context, by level, up to the first
/// missing level.
fn context_snapshot(context: &WorkStack<Binder>) -> (Vec<String>, Vec<Sort>) {
    let mut names = Vec::with_capacity(context.total);
    let mut sorts = Vec::with_capacity(context.total);
    let mut level = 0usize;
    let mut is_complete = true;
    while is_complete && level < context.total {
        match binder_parts(context, level) {
            Some((name, sort)) => {
                names.push(name);
                sorts.push(sort);
            }
            None => is_complete = false,
        }
        level = level.saturating_add(1);
    }
    (names, sorts)
}

/// Resolve a metavariable index.
fn meta_index(value: u32, span: Span) -> Result<usize, Diagnostic> {
    usize::try_from(value).map_err(|_| internal(span, "metavariable index exceeds the host"))
}

/// The budget a failed meter step exceeded. Each transition fails only after
/// it has counted its own step, so the counters above their ceilings name it;
/// a visit that exceeds neither the node nor the work ceiling exceeded depth.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the meter's ceilings are read through its non-const Deref to the shared configuration, so the const suggestion does not apply"
)]
fn exceeded_budget(meter: &Meter<'_>, step: MeterStep) -> &'static str {
    match step {
        MeterStep::Normalize(_) => {
            if meter.normalization > meter.budgets.normalization_work {
                "normalization"
            } else {
                "work"
            }
        }
        MeterStep::Substitute(_) => {
            if meter.substitution > meter.budgets.substitution_work {
                "substitution"
            } else {
                "work"
            }
        }
        MeterStep::Visit(_) => {
            if meter.nodes > meter.limits.nodes {
                "node"
            } else if meter.work > meter.limits.work {
                "work"
            } else {
                "depth"
            }
        }
        MeterStep::Charge(_) => "work",
    }
}

/// The exhaustion of a metered elaboration step: the stage that charged it
/// and the request budget it exceeded. Never acceptance, never refutation.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the stage and budget names are positional in the rendered sentence and named at the single call site"
)]
fn meter_exhausted(span: Span, stage: &'static str, budget: &'static str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::Exhausted,
        span,
        &text![
            "exhausted: elaboration ",
            stage,
            " step exceeded the ",
            budget,
            " budget",
        ],
    )
}

fn stage_exhausted(span: Span, stage: &'static str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::Exhausted,
        span,
        &text!["exhausted: elaboration ", stage, " budget"],
    )
}

/// The most bytes one rendered term contributes to a refusal.
const MAX_TERM_BYTES: usize = 1024;
/// The most bytes of one refusal message, category included.
const MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024;
/// The most originating constraint spans one refusal lists.
const MAX_LISTED_CONSTRAINTS: usize = 16;

/// `text` cut to at most `limit_bytes` bytes with the number of cut bytes named.
/// The cut stops early at the first non-ASCII byte, so it never splits a
/// character; the count still names every byte not shown.
fn capped(text: String, limit_bytes: usize) -> String {
    if text.len() <= limit_bytes {
        return text;
    }
    let bytes = text.as_bytes();
    let mut kept = String::with_capacity(limit_bytes.saturating_add(32));
    let mut index = 0usize;
    while index < limit_bytes && bytes[index] < 0x80 {
        kept.push(char::from(bytes[index]));
        index = index.saturating_add(1);
    }
    kept.push_str("...[");
    kept.push_str(&super::decimal(bytes.len().saturating_sub(index)));
    kept.push_str(" more bytes]");
    kept
}

/// One rendered term of a refusal, bounded by `MAX_TERM_BYTES`.
fn capped_term(text: String) -> String {
    capped(text, MAX_TERM_BYTES)
}

/// `start..end`, the byte range of a source span.
fn span_text(span: Span) -> String {
    // Not `text!`: its first part goes through `String::from`, and the pinned
    // Aeneas library models that conversion only from `&str`, not `&String`.
    let mut text = u32_text(span.start);
    text.push_str("..");
    text.push_str(&u32_text(span.end));
    text
}

/// `ROLE for binder `NAME` (position P) of HEAD`, with the 1-based position.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the role, binder and head texts are positional in the rendered order and named at every call site"
)]
fn binder_origin(role: &str, binder: &str, position: u32, head: &str) -> String {
    text![
        role,
        " for binder `",
        binder,
        "` (position ",
        &u32_text(position.saturating_add(1)),
        ") of ",
        head,
    ]
}

/// The originating constraint spans of one metavariable, at most
/// `MAX_LISTED_CONSTRAINTS`, counting the rest explicitly. Owned texts, not
/// a `&mut String` parameter: the pinned Aeneas cannot join a loop that
/// conditionally reborrows one.
fn constraint_spans(meta: &Meta) -> String {
    let mut text = String::new();
    let mut at = 0usize;
    while at < meta.constraints.len() && at < MAX_LISTED_CONSTRAINTS {
        if at > 0 {
            text.push(',');
        }
        text.push(' ');
        text.push_str(&span_text(meta.constraints[at]));
        at = at.saturating_add(1);
    }
    if meta.constraints.len() > at {
        text.push_str("; and ");
        text.push_str(&super::decimal(meta.constraints.len().saturating_sub(at)));
        text.push_str(" more constraint spans");
    }
    text
}

/// `none` when the metavariable has no recorded constraint, otherwise `some`
/// followed by its originating spans. Early returns rather than a branch
/// join, which the pinned Aeneas does not match for a borrowed metavariable.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the two clause texts are positional (absent, present) and named at every call site"
)]
fn constraint_clause(meta: &Meta, none: &str, some: &str) -> String {
    if meta.constraints.is_empty() {
        return String::from(none);
    }
    let mut clause = String::from(some);
    clause.push_str(&constraint_spans(meta));
    clause
}

/// Metavariable `index`'s binder, position, head identity and originating
/// spans when its latest origin is the failed constraint at `span` (empty
/// otherwise), listing spans while fewer than `MAX_LISTED_CONSTRAINTS` are
/// listed in total, with the updated listed and total counts. The test is
/// here, not in the caller's loop: the pinned Aeneas cannot join a loop body
/// that branches on a borrowed table entry.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the index and the two running counts are positional and named at the single call site"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the helper only renders refusal text; the counts it returns are bounded by saturating arithmetic and checked by the crate's cap tests"
)]
fn meta_origin(
    meta: &Meta,
    index: usize,
    span: Span,
    listed: usize,
    total: usize,
) -> (String, usize, usize) {
    // The comparison `note_ty` uses; a match on the borrowed last entry is an
    // internal error in the pinned Aeneas.
    if meta.constraints.last() != Some(&span) {
        return (String::new(), listed, total);
    }
    let mut text = String::from("; ?");
    text.push_str(&super::decimal(index));
    text.push_str(" is ");
    text.push_str(&binder_origin(
        "the argument",
        &meta.binder,
        meta.position,
        &meta.head,
    ));
    text.push_str(", constrained at");
    let mut listed = listed;
    let mut total = total;
    let mut at = 0usize;
    while at < meta.constraints.len() {
        total = total.saturating_add(1);
        if listed < MAX_LISTED_CONSTRAINTS {
            if at > 0 {
                text.push(',');
            }
            text.push(' ');
            text.push_str(&span_text(meta.constraints[at]));
            listed = listed.saturating_add(1);
        }
        at = at.saturating_add(1);
    }
    (text, listed, total)
}

/// Every metavariable the failed constraint at `span` mentions (each one's
/// latest recorded origin is that constraint): its binder, position, head
/// identity and originating constraint spans, at most
/// `MAX_LISTED_CONSTRAINTS` spans in total, counting the rest explicitly.
fn constraint_origins(metas: &[Meta], span: Span) -> String {
    let mut origins = String::new();
    let mut listed = 0usize;
    let mut total = 0usize;
    let mut index = 0usize;
    while index < metas.len() {
        let (text, now_listed, now_total) = meta_origin(&metas[index], index, span, listed, total);
        origins.push_str(&text);
        listed = now_listed;
        total = now_total;
        index = index.saturating_add(1);
    }
    if total > listed {
        origins.push_str("; and ");
        origins.push_str(&super::decimal(total.saturating_sub(listed)));
        origins.push_str(" more constraint spans");
    }
    origins
}

/// The refusal of a failed constraint: the clash, what it was checking and
/// every metavariable the constraint mentions. Exhaustion and internal
/// failures pass through unchanged.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper clones and extends a String and caps it, none of which is const"
)]
fn explain_constraint(metas: &[Meta], error: Diagnostic, span: Span, origin: &str) -> Diagnostic {
    if error.kind != DiagnosticKind::Invalid {
        return error;
    }
    let kind = error.kind;
    let at = error.span;
    let mut detail = error.message.clone();
    detail.push_str("; checking ");
    detail.push_str(origin);
    detail.push_str(&constraint_origins(metas, span));
    Diagnostic::new(kind, at, &capped(detail, MAX_DIAGNOSTIC_BYTES))
}

/// An ill-scoped solution's refusal, with the metavariable's binder, head
/// identity and originating constraints.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper clones and extends a String and caps it, none of which is const"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the helper only renders refusal text for an already refused solution; the crate's refusal tests check its content"
)]
fn scope_refusal(meta: &Meta, error: Diagnostic) -> Diagnostic {
    if error.kind != DiagnosticKind::Invalid {
        return error;
    }
    let kind = error.kind;
    let at = error.span;
    let mut detail = error.message.clone();
    detail.push_str("; ");
    detail.push_str(&binder_origin(
        "solving",
        &meta.binder,
        meta.position,
        &meta.head,
    ));
    detail.push_str(&constraint_clause(
        meta,
        ", with no constraint",
        ", constrained at",
    ));
    Diagnostic::new(kind, at, &capped(detail, MAX_DIAGNOSTIC_BYTES))
}

/// The refusal for a metavariable no constraint solves: its binder, position
/// and head identity, and the constraints that did not fix it, if any.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the helper only renders refusal text for an already unsolved metavariable; the crate's refusal tests check its content"
)]
fn underconstrained_refusal(meta: &Meta) -> Diagnostic {
    let mut detail = text![
        "binder `",
        &meta.binder,
        "` (position ",
        &u32_text(meta.position.saturating_add(1)),
        ") of ",
        &meta.head,
    ];
    // Constraints that only relate it to other unsolved arguments never fix
    // it; they are listed, not chosen from.
    detail.push_str(&constraint_clause(
        meta,
        " has no constraint",
        " is not fixed by its constraints at",
    ));
    detail.push_str("; any PureTyCode or code-fixing argument applies and none is chosen");
    infer_diagnostic(
        DiagnosticKind::Invalid,
        meta.span,
        "underconstrained",
        &detail,
    )
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
fn infer_diagnostic(
    kind: DiagnosticKind,
    span: Span,
    category: &'static str,
    detail: &str,
) -> Diagnostic {
    Diagnostic::new(
        kind,
        span,
        &capped(text![category, ": ", detail], MAX_DIAGNOSTIC_BYTES),
    )
}

/// `{module}@{version}/{name}`, the identity of an available proof or of a
/// contract head.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the module and declaration names are positional in the identity's print order and named at every call site"
)]
fn identity_text(module: &str, version: u32, name: &str) -> String {
    text![module, "@", &u32_text(version), "/", name]
}

/// The decimal digits of `value`, exactly as `{value}` renders them.
fn u32_text(value: u32) -> String {
    // The digits, least significant first; there is always at least one.
    let mut reversed = String::new();
    let mut rest = value;
    reversed.push(char::from(u32_digit(rest % 10)));
    rest /= 10;
    while rest > 0 {
        reversed.push(char::from(u32_digit(rest % 10)));
        rest /= 10;
    }
    reversed_text(&reversed)
}

/// The decimal text of `value`, exactly as `{value}` renders it. The digits
/// come from the remainders, which lie in -9..=9, so `i64::MIN`, whose
/// magnitude is not an `i64`, never overflows.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the rendering is total over i64 and pinned against `{value}` for the extremes by the crate's digit tests"
)]
fn signed_text(value: i64) -> String {
    let mut reversed = String::new();
    let mut rest = value;
    let mut has_more_digits = true;
    while has_more_digits {
        let remainder = rest % 10;
        let digit = if remainder < 0 {
            0 - remainder
        } else {
            remainder
        };
        reversed.push(char::from(i64_digit(digit)));
        rest /= 10;
        has_more_digits = rest != 0;
    }
    if value < 0 {
        reversed.push('-');
    }
    reversed_text(&reversed)
}

/// `reversed`, an ASCII text, in the opposite order.
fn reversed_text(reversed: &str) -> String {
    let bytes = reversed.as_bytes();
    let mut text = String::with_capacity(bytes.len());
    let mut index = bytes.len();
    while index > 0 {
        index -= 1;
        text.push(char::from(bytes[index]));
    }
    text
}

/// The ASCII digit for `digit`, which is below ten; the last arm is nine.
const fn u32_digit(digit: u32) -> u8 {
    match digit {
        0 => b'0',
        1 => b'1',
        2 => b'2',
        3 => b'3',
        4 => b'4',
        5 => b'5',
        6 => b'6',
        7 => b'7',
        8 => b'8',
        _ => b'9',
    }
}

/// The ASCII digit for `digit`, which lies in 0..=9; the last arm is nine.
const fn i64_digit(digit: i64) -> u8 {
    match digit {
        0 => b'0',
        1 => b'1',
        2 => b'2',
        3 => b'3',
        4 => b'4',
        5 => b'5',
        6 => b'6',
        7 => b'7',
        8 => b'8',
        _ => b'9',
    }
}

fn unsupported(span: Span, detail: &str) -> Diagnostic {
    infer_diagnostic(DiagnosticKind::Unsupported, span, "unsupported", detail)
}

fn invalid(span: Span, detail: &str) -> Diagnostic {
    infer_diagnostic(DiagnosticKind::Invalid, span, "invalid-constraint", detail)
}

fn ill_scoped(span: Span, detail: &str) -> Diagnostic {
    infer_diagnostic(DiagnosticKind::Invalid, span, "ill-scoped", detail)
}

fn internal(span: Span, detail: &str) -> Diagnostic {
    infer_diagnostic(DiagnosticKind::Internal, span, "internal", detail)
}

/// Resolve every code metavariable in `ty`.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; resolution follows the finite metavariable table, whose bindings are occurs-checked into acyclic chains"
)]
fn resolve_ty(elaborator: &Elaborator<'_, '_>, ty: &Ty, span: Span) -> Result<Ty, Diagnostic> {
    match ty {
        Ty::Meta(value) => {
            let index = meta_index(*value, span)?;
            let meta = elaborator
                .metas
                .get(index)
                .ok_or_else(|| internal(span, "unknown code metavariable"))?;
            match &meta.code {
                Some(solution) => resolve_ty(elaborator, solution, span),
                None => Ok(Ty::Meta(*value)),
            }
        }
        Ty::Pair(left, right) => Ok(Ty::Pair(
            Box::new(resolve_ty(elaborator, left, span)?),
            Box::new(resolve_ty(elaborator, right, span)?),
        )),
        Ty::Sum(left, right) => Ok(Ty::Sum(
            Box::new(resolve_ty(elaborator, left, span)?),
            Box::new(resolve_ty(elaborator, right, span)?),
        )),
        Ty::List(item) => Ok(Ty::List(Box::new(resolve_ty(elaborator, item, span)?))),
        Ty::Unit => Ok(Ty::Unit),
        Ty::Bool => Ok(Ty::Bool),
        Ty::I64 => Ok(Ty::I64),
        Ty::Text => Ok(Ty::Text),
        Ty::Var(index) => Ok(Ty::Var(*index)),
    }
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the methods mutate only the elaborator's own metavariable table, deferred queue, binder context and meter for one bounded elaboration; the borrowed proof terms and dependency snapshot stay immutable."
)]
impl Elaborator<'_, '_> {
    fn resolve_ty(&self, ty: &Ty, span: Span) -> Result<Ty, Diagnostic> {
        resolve_ty(self, ty, span)
    }

    /// Resolve a value metavariable; bindings are acyclic because every
    /// binding is resolved first.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; resolution follows the finite value-metavariable chain, and binding always resolves both sides first, so every meta-to-meta link strictly lowers the index"
    )]
    fn resolve_val(&self, value: Val, span: Span) -> Result<Val, Diagnostic> {
        match value {
            Val::Level(_) => Ok(value),
            Val::Meta(meta) => {
                let index = meta_index(meta, span)?;
                let entry = self
                    .metas
                    .get(index)
                    .ok_or_else(|| internal(span, "unknown value metavariable"))?;
                match entry.value {
                    Some(next) => self.resolve_val(next, span),
                    None => Ok(value),
                }
            }
        }
    }

    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; resolution follows the finite proposition tree"
    )]
    fn resolve_prop(&self, prop: &PropTerm, span: Span) -> Result<PropTerm, Diagnostic> {
        match prop {
            PropTerm::Pi(index, sort, body) => Ok(PropTerm::Pi(
                *index,
                self.resolve_sort(sort, span)?,
                Box::new(self.resolve_prop(body, span)?),
            )),
            PropTerm::Eq(code, left, right) => Ok(PropTerm::Eq(
                self.resolve_ty(code, span)?,
                self.resolve_val(*left, span)?,
                self.resolve_val(*right, span)?,
            )),
        }
    }

    fn resolve_sort(&self, sort: &SortTerm, span: Span) -> Result<SortTerm, Diagnostic> {
        match sort {
            SortTerm::Codes => Ok(SortTerm::Codes),
            SortTerm::Value(ty) => Ok(SortTerm::Value(self.resolve_ty(ty, span)?)),
            SortTerm::Premise(prop) => {
                Ok(SortTerm::Premise(Box::new(self.resolve_prop(prop, span)?)))
            }
        }
    }

    /// Record `span` as an originating constraint of every metavariable the
    /// constraint mentions, in bounded walk order.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; the metavariable table is the elaborator's locally owned state and recording a constraint origin is the operation"
    )]
    #[expect(
        tigerstyle::unbounded_collection_growth,
        reason = "Owner: noble-contracts; the worklist is bounded by the 4,096-node budget the loop decrements before every push"
    )]
    fn note_ty(&mut self, ty: &Ty, span: Span) {
        let mut pending = Vec::new();
        pending.push(ty);
        let mut budget = 4096usize;
        let mut next = pending.pop();
        let mut is_running = next.is_some() && budget > 0;
        while is_running {
            if let Some(node) = next {
                budget -= 1;
                match node {
                    Ty::Meta(value) => {
                        if let Ok(index) = meta_index(*value, span) {
                            if let Some(meta) = self.metas.get_mut(index) {
                                if meta.constraints.last() != Some(&span) {
                                    meta.constraints.push(span);
                                }
                            }
                        }
                    }
                    Ty::Pair(left, right) | Ty::Sum(left, right) => {
                        pending.push(right);
                        pending.push(left);
                    }
                    Ty::List(item) => pending.push(item),
                    Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Var(_) => {}
                }
                next = pending.pop();
            }
            is_running = next.is_some() && budget > 0;
        }
    }

    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; the metavariable table is the elaborator's locally owned state and recording a constraint origin is the operation"
    )]
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; recording constraint origins follows the finite constraint term"
    )]
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; recording constraint origins pushes onto the growable metavariable table, so the const suggestion does not apply"
    )]
    fn note_prop(&mut self, prop: &PropTerm, span: Span) {
        match prop {
            PropTerm::Pi(_, sort, body) => {
                self.note_sort(sort, span);
                self.note_prop(body, span);
            }
            PropTerm::Eq(code, left, right) => {
                self.note_ty(code, span);
                self.note_val(*left, span);
                self.note_val(*right, span);
            }
        }
    }
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; recording constraint origins pushes onto the growable metavariable table, so the const suggestion does not apply"
    )]
    fn note_val(&mut self, value: Val, span: Span) {
        if let Val::Meta(meta) = value {
            if let Ok(index) = meta_index(meta, span) {
                if let Some(entry) = self.metas.get_mut(index) {
                    if entry.constraints.last() != Some(&span) {
                        entry.constraints.push(span);
                    }
                }
            }
        }
    }

    /// Add one code constraint, recording its origins and charging it.
    fn constrain_ty(&mut self, left: &Ty, right: &Ty, span: Span) -> Result<(), Diagnostic> {
        self.note_ty(left, span);
        self.note_ty(right, span);
        let nodes = ty_size(left).saturating_add(ty_size(right));
        self.meter_step(MeterStep::Charge(nodes), "constraint", span)?;
        self.unify_ty(left, right, span)
    }

    fn constrain_prop(
        &mut self,
        left: &PropTerm,
        right: &PropTerm,
        span: Span,
    ) -> Result<(), Diagnostic> {
        self.note_prop(left, span);
        self.note_prop(right, span);
        let nodes = prop_size_of(left).saturating_add(prop_size_of(right));
        self.meter_step(MeterStep::Charge(nodes), "constraint", span)?;
        self.unify_prop(left, right, span)
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; recording constraint origins pushes onto the growable metavariable table, so the const suggestion does not apply"
    )]
    fn note_sort(&mut self, sort: &SortTerm, span: Span) {
        match sort {
            SortTerm::Codes => {}
            SortTerm::Value(ty) => self.note_ty(ty, span),
            SortTerm::Premise(prop) => self.note_prop(prop, span),
        }
    }

    /// Unify two code terms, binding a metavariable to the ground side.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; unification follows the two finite term trees, and every binding is occurs-checked"
    )]
    fn unify_ty(&mut self, left: &Ty, right: &Ty, span: Span) -> Result<(), Diagnostic> {
        self.meter_step(MeterStep::Normalize(1), "unification", span)?;
        let left = self.resolve_ty(left, span)?;
        let right = self.resolve_ty(right, span)?;
        match (&left, &right) {
            (Ty::Meta(first), Ty::Meta(second)) => {
                // By value: `<` on the shared references would select
                // `PartialOrd for &u32`, whose `lt` the pinned library models
                // only through the unmodelled array instance.
                if *first == *second {
                    return Ok(());
                }
                let (early, late) = if *first < *second {
                    (*first, *second)
                } else {
                    (*second, *first)
                };
                self.bind_code(late, Ty::Meta(early), span)
            }
            (Ty::Meta(meta), other) | (other, Ty::Meta(meta)) => {
                self.bind_code(*meta, clone_ty(other), span)
            }
            (Ty::Unit, Ty::Unit)
            | (Ty::Bool, Ty::Bool)
            | (Ty::I64, Ty::I64)
            | (Ty::Text, Ty::Text) => Ok(()),
            (Ty::Var(first), Ty::Var(second)) if first == second => Ok(()),
            (Ty::Pair(left_a, right_a), Ty::Pair(left_b, right_b))
            | (Ty::Sum(left_a, right_a), Ty::Sum(left_b, right_b)) => {
                self.unify_ty(left_a, left_b, span)?;
                self.unify_ty(right_a, right_b, span)
            }
            (Ty::List(first), Ty::List(second)) => self.unify_ty(first, second, span),
            _ => Err(invalid(
                span,
                &text![
                    "code ",
                    &capped_term(describe_ty(&left)),
                    " does not match code ",
                    &capped_term(describe_ty(&right)),
                ],
            )),
        }
    }

    /// Unify two value positions.
    fn unify_val(&mut self, left: Val, right: Val, span: Span) -> Result<(), Diagnostic> {
        self.meter_step(MeterStep::Normalize(1), "unification", span)?;
        let left = self.resolve_val(left, span)?;
        let right = self.resolve_val(right, span)?;
        match (left, right) {
            (Val::Meta(first), Val::Meta(second)) => {
                if first == second {
                    return Ok(());
                }
                let (early, late) = if first < second {
                    (first, second)
                } else {
                    (second, first)
                };
                self.bind_value(late, Val::Meta(early), span)
            }
            (Val::Meta(meta), other) | (other, Val::Meta(meta)) => {
                self.bind_value(meta, other, span)
            }
            (Val::Level(first), Val::Level(second)) => {
                if first == second {
                    Ok(())
                } else {
                    Err(invalid(
                        span,
                        &text![
                            "value ",
                            &u32_text(first),
                            " does not match value ",
                            &u32_text(second),
                        ],
                    ))
                }
            }
        }
    }

    /// Unify two sorts by their constructors; the only definitional equality
    /// is structural identity.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; a sort clash builds its diagnostic text at run time, so the const suggestion does not apply"
    )]
    fn unify_sort(
        &mut self,
        left: &SortTerm,
        right: &SortTerm,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match (left, right) {
            (SortTerm::Codes, SortTerm::Codes) => Ok(()),
            (SortTerm::Value(first), SortTerm::Value(second)) => self.unify_ty(first, second, span),
            (SortTerm::Premise(first), SortTerm::Premise(second)) => {
                self.unify_prop(first, second, span)
            }
            _ => Err(invalid(
                span,
                &text![
                    "sort ",
                    &capped_term(describe_sort(left)),
                    " does not match sort ",
                    &capped_term(describe_sort(right)),
                ],
            )),
        }
    }

    /// Unify two propositions; `Pi` binders must agree on their level.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; unification follows the two finite proposition trees"
    )]
    fn unify_prop(
        &mut self,
        left: &PropTerm,
        right: &PropTerm,
        span: Span,
    ) -> Result<(), Diagnostic> {
        match (left, right) {
            (
                PropTerm::Pi(left_index, left_sort, left_body),
                PropTerm::Pi(right_index, right_sort, right_body),
            ) => {
                if left_index != right_index {
                    return Err(invalid(
                        span,
                        &text![
                            "dependent Pi binder ",
                            &u32_text(*left_index),
                            " does not match binder ",
                            &u32_text(*right_index),
                        ],
                    ));
                }
                self.unify_sort(left_sort, right_sort, span)?;
                self.unify_prop(left_body, right_body, span)
            }
            (
                PropTerm::Eq(left_code, left_from, left_to),
                PropTerm::Eq(right_code, right_from, right_to),
            ) => {
                self.unify_ty(left_code, right_code, span)?;
                self.unify_val(*left_from, *right_from, span)?;
                self.unify_val(*left_to, *right_to, span)
            }
            _ => Err(invalid(
                span,
                &text![
                    "proposition ",
                    &capped_term(describe_prop(left)),
                    " does not match ",
                    &capped_term(describe_prop(right)),
                ],
            )),
        }
    }

    /// Bind one code metavariable after an occurs check.
    fn bind_code(&mut self, meta: u32, solution: Ty, span: Span) -> Result<(), Diagnostic> {
        if occurs_code(meta, &solution) {
            return Err(invalid(
                span,
                &text![
                    "occurs check: ?",
                    &u32_text(meta),
                    " occurs in ",
                    &capped_term(describe_ty(&solution)),
                ],
            ));
        }
        let index = meta_index(meta, span)?;
        // A `let`-`else` rather than `ok_or_else` on the mutable borrow: the
        // pinned Aeneas models `Option::ok_or_else` without the backward
        // function a `&mut` payload needs.
        let Some(entry) = self.metas.get_mut(index) else {
            return Err(internal(span, "unknown code metavariable"));
        };
        if entry.code.is_some() {
            return Err(internal(span, "code metavariable bound twice"));
        }
        entry.code = Some(solution);
        Ok(())
    }

    /// Bind one value metavariable, checking its required code at the bound
    /// level and that the level lies in the proof context.
    fn bind_value(&mut self, meta: u32, solution: Val, span: Span) -> Result<(), Diagnostic> {
        let index = meta_index(meta, span)?;
        let (required, sorts) = match self.metas.get(index) {
            Some(entry) => {
                // Aeneas refuses function-definition types with regions, so
                // the map function item `Option::map(clone_ty)` is written as
                // a plain assignment here.
                let mut required = None;
                if let Some(code) = &entry.required {
                    required = Some(clone_ty(code));
                }
                (required, entry.sorts.clone())
            }
            None => return Err(internal(span, "unknown value metavariable")),
        };
        if let (Some(required), Val::Level(level)) = (required, solution) {
            let code = {
                let index = match usize::try_from(level) {
                    Ok(index) => index,
                    Err(_) => {
                        return Err(internal(span, "value level exceeds the host address space"))
                    }
                };
                let sort = match sorts.get(index) {
                    Some(sort) => sort,
                    None => {
                        return Err(internal(
                            span,
                            "value level outside the recorded proof context",
                        ))
                    }
                };
                match sort {
                    Sort::Value(code) => ty_of_code(code, span)?,
                    _ => {
                        return Err(invalid(
                            span,
                            "value binder at this level is not a total value",
                        ))
                    }
                }
            };
            self.constrain_ty(&required, &code, span)?;
        }
        // A `let`-`else` rather than `ok_or_else` on the mutable borrow: the
        // pinned Aeneas models `Option::ok_or_else` without the backward
        // function a `&mut` payload needs.
        let Some(entry) = self.metas.get_mut(index) else {
            return Err(internal(span, "unknown value metavariable"));
        };
        if entry.value.is_some() {
            return Err(internal(span, "value metavariable bound twice"));
        }
        entry.value = Some(solution);
        Ok(())
    }

    /// Whether every code variable in a ground solution lies below `depth`.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; the scope check follows the finite ground term"
    )]
    fn scope_ty(&self, ty: &Ty, depth: u32, span: Span) -> Result<(), Diagnostic> {
        match ty {
            Ty::Var(level) => {
                if *level >= depth {
                    return Err(ill_scoped(
                        span,
                        &text![
                            "solution names code level ",
                            &u32_text(*level),
                            " at depth ",
                            &u32_text(depth),
                        ],
                    ));
                }
                Ok(())
            }
            Ty::Pair(left, right) | Ty::Sum(left, right) => {
                self.scope_ty(left, depth, span)?;
                self.scope_ty(right, depth, span)
            }
            Ty::List(item) => self.scope_ty(item, depth, span),
            Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Meta(_) => Ok(()),
        }
    }

    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; grounding follows the finite resolved code tree"
    )]
    fn ground_ty(&self, ty: &Ty, span: Span) -> Result<Code, Diagnostic> {
        let resolved = self.resolve_ty(ty, span)?;
        match &resolved {
            Ty::Unit => Ok(Code::Unit),
            Ty::Bool => Ok(Code::Bool),
            Ty::I64 => Ok(Code::I64),
            Ty::Text => Ok(Code::Text),
            Ty::Var(level) => {
                Ok(Code::Variable(usize::try_from(*level).map_err(|_| {
                    internal(span, "code level exceeds the host address space")
                })?))
            }
            Ty::Pair(left, right) => Ok(Code::Pair(
                Box::new(self.ground_ty(left, span)?),
                Box::new(self.ground_ty(right, span)?),
            )),
            Ty::Sum(left, right) => Ok(Code::Sum(
                Box::new(self.ground_ty(left, span)?),
                Box::new(self.ground_ty(right, span)?),
            )),
            Ty::List(item) => Ok(Code::List(Box::new(self.ground_ty(item, span)?))),
            Ty::Meta(_) => Err(internal(
                span,
                "unresolved code metavariable in a ground term",
            )),
        }
    }

    fn ground_sort(&self, sort: &SortTerm, span: Span) -> Result<Sort, Diagnostic> {
        let resolved = self.resolve_sort(sort, span)?;
        match &resolved {
            SortTerm::Codes => Ok(Sort::Codes),
            SortTerm::Value(ty) => Ok(Sort::Value(self.ground_ty(ty, span)?)),
            SortTerm::Premise(prop) => Ok(Sort::Premise(Box::new(self.ground_prop(prop, span)?))),
        }
    }

    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; grounding follows the finite resolved proposition tree"
    )]
    fn ground_prop(&self, prop: &PropTerm, span: Span) -> Result<Proposition, Diagnostic> {
        let resolved = self.resolve_prop(prop, span)?;
        match &resolved {
            PropTerm::Pi(index, sort, body) => Ok(Proposition::Pi(
                usize::try_from(*index)
                    .map_err(|_| internal(span, "binder level exceeds the host"))?,
                self.ground_sort(sort, span)?,
                Box::new(self.ground_prop(body, span)?),
            )),
            PropTerm::Eq(code, left, right) => Ok(Proposition::Eq(
                self.ground_ty(code, span)?,
                self.ground_level(*left, span)?,
                self.ground_level(*right, span)?,
            )),
        }
    }

    fn ground_level(&self, value: Val, span: Span) -> Result<usize, Diagnostic> {
        match self.resolve_val(value, span)? {
            Val::Level(level) => usize::try_from(level)
                .map_err(|_| internal(span, "value level exceeds the host address space")),
            Val::Meta(meta) => {
                let index = meta_index(meta, span)?;
                Err(self.underconstrained(index))
            }
        }
    }
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the methods mutate only the elaborator's own metavariable table, deferred queue, binder context and meter for one bounded elaboration; the borrowed proof terms and dependency snapshot stay immutable."
)]
impl Elaborator<'_, '_> {
    /// Check `term` against `goal`, the checker's check-mode rule set.
    fn check(&mut self, form: &Form, goal: &PropTerm, depth: u32) -> Result<Out, Diagnostic> {
        self.meter_step(MeterStep::Visit(depth), "traversal", form.span)?;
        match goal {
            PropTerm::Pi(..) => {
                if let Ok(args) = operation(form, "intro", 2) {
                    return self.intro(form, args, goal, depth);
                }
            }
            PropTerm::Eq(..) => {
                if let Ok(parts) = operation(form, "subst", 3) {
                    return self.subst(form, parts, goal, depth);
                }
            }
        }
        let child = deeper(depth, form.span)?;
        let inferred = self.infer(form, child)?;
        if let Err(error) = self.constrain_prop(&inferred.prop, goal, form.span) {
            return Err(explain_constraint(
                &self.metas,
                error,
                form.span,
                "the expected claim",
            ));
        }
        Ok(inferred.out)
    }

    /// Check an `intro` against a dependent `Pi` goal.
    fn intro(
        &mut self,
        form: &Form,
        args: &[Form],
        goal: &PropTerm,
        depth: u32,
    ) -> Result<Out, Diagnostic> {
        let binder_form = list(&args[0])?;
        if binder_form.len() != 2 {
            return Err(super::reject(args[0].span, "intro needs a typed binder"));
        }
        let name = String::from(atom(&binder_form[0])?);
        let sort_depth = deeper(depth, binder_form[1].span)?;
        let (next, parsed) = sort(&binder_form[1], &self.context, self.meter, sort_depth);
        self.meter = next;
        let actual = parsed?;
        let (index, expected, rest) = match goal {
            PropTerm::Pi(index, expected, rest) => (*index, expected, rest),
            _ => {
                return Err(internal(
                    form.span,
                    "intro check without a dependent Pi goal",
                ))
            }
        };
        let expected_sort = self.ground_sort(expected, form.span)?;
        let current = u32_level(self.context.total, form.span)?;
        if !super::sort_equal(&actual, &expected_sort) || index != current {
            return Err(super::reject(
                args[0].span,
                "intro binder type differs from goal",
            ));
        }
        let body_depth = deeper(depth, args[1].span)?;
        // The binder form's span: candidate rows point at it.
        self.push_binder(Binder { name, sort: actual }, args[0].span)?;
        let body = self.check(&args[1], rest, body_depth);
        self.pop_binder();
        // Propagated before `vec!`: a `?` inside its elements lowers to an
        // uninitialised box the pinned Aeneas library does not model.
        let body = body?;
        let binder_out = Out::List(
            alloc::vec![
                Out::Done(binder_form[0].clone()),
                Out::Done(binder_form[1].clone()),
            ],
            args[0].span,
        );
        Ok(Out::List(
            alloc::vec![out_atom("intro", form.span), binder_out, body],
            form.span,
        ))
    }

    /// Check a `subst` against a goal: the motive is explicit, the evidence is
    /// inferred, and the premise is checked against the substituted motive.
    fn subst(
        &mut self,
        form: &Form,
        parts: &[Form],
        goal: &PropTerm,
        depth: u32,
    ) -> Result<Out, Diagnostic> {
        let motive = operation(&parts[0], "motive", 2)?;
        let binder_form = list(&motive[0])?;
        if binder_form.len() != 2 {
            return Err(super::reject(
                motive[0].span,
                "subst motive needs one typed value binder",
            ));
        }
        let name = String::from(atom(&binder_form[0])?);
        let sort_depth = deeper(depth, binder_form[1].span)?;
        let (next, parsed) = sort(&binder_form[1], &self.context, self.meter, sort_depth);
        self.meter = next;
        let Sort::Value(code) = parsed? else {
            return Err(super::reject(
                motive[0].span,
                "subst motive binder must be a total value",
            ));
        };
        let evidence_depth = deeper(depth, parts[1].span)?;
        let evidence = self.infer(&parts[1], evidence_depth)?;
        let (equation_code, left, right) = match evidence.prop {
            PropTerm::Eq(code, left, right) => (code, left, right),
            _ => {
                return Err(super::reject(
                    parts[1].span,
                    "subst requires equality evidence",
                ))
            }
        };
        let equation_code = self.ground_ty(&equation_code, parts[1].span)?;
        if equation_code != code {
            return Err(super::reject(
                binder_form[0].span,
                "subst motive has wrong equality type",
            ));
        }
        let left_level = self.ground_level(left, parts[1].span)?;
        let right_level = self.ground_level(right, parts[1].span)?;
        let index = self.context.total;
        let bound = self.meter.tree_bounds.frames;
        self.push_binder(
            Binder {
                name,
                sort: Sort::Value(code),
            },
            motive[0].span,
        )?;
        let motive_depth = deeper(depth, motive[1].span)?;
        let (next, parsed) = proposition(&motive[1], &self.context, self.meter, motive_depth);
        self.meter = next;
        self.pop_binder();
        let body = parsed?;
        let Proposition::Eq(..) = &body else {
            return Err(super::reject(
                motive[1].span,
                "subst motive must be a total Eq proposition",
            ));
        };
        let before = super::replace_prop(
            &body,
            index,
            &super::Argument::Value(left_level),
            form.span,
            bound,
        )?;
        let after = super::replace_prop(
            &body,
            index,
            &super::Argument::Value(right_level),
            form.span,
            bound,
        )?;
        let ground_goal = self.ground_prop(goal, form.span)?;
        if !super::proposition_equal(&after, &ground_goal) {
            return Err(super::reject(
                form.span,
                "subst result differs from expected goal",
            ));
        }
        let premise_goal = prop_term(&before, form.span)?;
        let premise_depth = deeper(depth, parts[2].span)?;
        let premise = self.check(&parts[2], &premise_goal, premise_depth)?;
        Ok(Out::List(
            alloc::vec![
                out_atom("subst", form.span),
                Out::List(
                    alloc::vec![
                        out_atom("motive", parts[0].span),
                        Out::List(
                            alloc::vec![
                                Out::Done(binder_form[0].clone()),
                                Out::Done(binder_form[1].clone()),
                            ],
                            motive[0].span,
                        ),
                        Out::Done(motive[1].clone()),
                    ],
                    parts[0].span,
                ),
                evidence.out,
                premise,
            ],
            form.span,
        ))
    }

    /// Infer the proposition a `use`, `apply`, `refl` or `app` term
    /// establishes.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; inference follows the finite application spine under the caller's depth budget"
    )]
    fn infer(&mut self, form: &Form, depth: u32) -> Result<Inferred, Diagnostic> {
        self.meter_step(MeterStep::Visit(depth), "traversal", form.span)?;
        if let Ok(args) = operation(form, "use", 1) {
            return self.use_term(form, &args[0]);
        }
        if let Ok(args) = operation(form, "apply", 2) {
            let child = deeper(depth, args[0].span)?;
            let function = self.infer(&args[0], child)?;
            return self.apply_one(form.span, function, &args[1], depth);
        }
        if let Ok(args) = operation(form, "refl", 1) {
            return self.refl_term(form, &args[0]);
        }
        if super::list_head_atom(form) == Some("app") {
            return self.app(form, depth);
        }
        Err(super::reject(
            form.span,
            "proof term has no inferable proposition",
        ))
    }

    /// Infer a `(use NAME)` reference: a proof premise or an available proof.
    fn use_term(&mut self, form: &Form, target: &Form) -> Result<Inferred, Diagnostic> {
        let name = atom(target)?;
        if let Some((_index, sort)) = stack_binder(&self.context, name) {
            let Sort::Premise(proof) = sort else {
                return Err(super::reject(
                    form.span,
                    "use does not name a proof premise",
                ));
            };
            return Ok(Inferred {
                prop: prop_term(proof, form.span)?,
                out: Out::Done(form.clone()),
                head: Head {
                    identity: String::from("local"),
                    names: Vec::new(),
                    next: 0,
                },
            });
        }
        let position = last_available(self.available, name).ok_or_else(|| {
            super::reject(
                form.span,
                "unresolved, private, forward or cyclic proof use",
            )
        })?;
        let dependency = &self.available[position];
        let (next, parsed) = proposition(&dependency.goal, &WorkStack::new(), self.meter, 0);
        self.meter = next;
        let goal = parsed?;
        let bound = self.meter.tree_bounds.frames;
        let shifted = shifted_prop(&goal, self.context.total, form.span, bound)?;
        Ok(Inferred {
            prop: prop_term(&shifted, form.span)?,
            out: Out::Done(form.clone()),
            head: Head {
                identity: dependency.identity.clone(),
                names: dependency.names.clone(),
                next: 0,
            },
        })
    }

    /// Infer `(refl x)`.
    fn refl_term(&mut self, form: &Form, operand: &Form) -> Result<Inferred, Diagnostic> {
        let name = atom(operand)?;
        let (index, sort) = stack_binder(&self.context, name)
            .ok_or_else(|| super::reject(operand.span, "unbound refl value"))?;
        let Sort::Value(code) = sort else {
            return Err(super::reject(
                form.span,
                "refl operand is not a total value",
            ));
        };
        let level = u32_level(index, form.span)?;
        Ok(Inferred {
            prop: PropTerm::Eq(
                ty_of_code(code, form.span)?,
                Val::Level(level),
                Val::Level(level),
            ),
            out: Out::Done(form.clone()),
            head: Head {
                identity: String::from("local"),
                names: Vec::new(),
                next: 0,
            },
        })
    }

    /// Apply one argument to an inferred dependent `Pi`.
    fn apply_one(
        &mut self,
        span: Span,
        inferred: Inferred,
        argument: &Form,
        depth: u32,
    ) -> Result<Inferred, Diagnostic> {
        let Inferred {
            prop,
            out,
            mut head,
        } = inferred;
        let (index, sort, body) = match prop {
            PropTerm::Pi(index, sort, body) => (index, sort, body),
            _ => return Err(super::reject(span, "apply requires dependent Pi proof")),
        };
        let current = u32_level(self.context.total, span)?;
        if index != current {
            return Err(super::reject(span, "apply binder scope mismatch"));
        }
        let child = deeper(depth, argument.span)?;
        let (arg_out, arg_term) = self.argument(&sort, argument, &mut head, child)?;
        let nodes = prop_size_of(&body);
        self.meter_step(MeterStep::Substitute(nodes), "substitution", span)?;
        let next = inst_prop(&body, index, &arg_term, span)?;
        Ok(Inferred {
            prop: next,
            out: Out::List(alloc::vec![out_atom("apply", span), out, arg_out], span),
            head,
        })
    }

    /// Elaborate one argument at a telescope binder.
    fn argument(
        &mut self,
        sort: &SortTerm,
        argument: &Form,
        head: &mut Head,
        depth: u32,
    ) -> Result<(Out, ArgTerm), Diagnostic> {
        // Every argument consumes one telescope binder, explicit or inferred.
        let (binder, position) = head.binder();
        head.next = position
            .checked_add(1)
            .ok_or_else(|| stage_exhausted(argument.span, "telescope"))?;
        let identity = head.identity.clone();
        match sort {
            SortTerm::Codes => {
                if is_infer_form(argument) {
                    let current = u32_level(self.context.total, argument.span)?;
                    let level =
                        self.alloc_code(&binder, position, &identity, argument.span, current)?;
                    return Ok((
                        Out::Infer(level, argument.span),
                        ArgTerm::Code(Ty::Meta(level)),
                    ));
                }
                let child = deeper(depth, argument.span)?;
                let (next, parsed) = code(argument, &self.context, self.meter, child);
                self.meter = next;
                let ty = ty_of_code(&parsed?, argument.span)?;
                Ok((Out::Done(argument.clone()), ArgTerm::Code(ty)))
            }
            SortTerm::Value(expected) => {
                let current = u32_level(self.context.total, argument.span)?;
                if is_infer_form(argument) {
                    let level = self.alloc_value(
                        &binder,
                        position,
                        &identity,
                        clone_ty(expected),
                        argument.span,
                        current,
                    )?;
                    return Ok((
                        Out::Infer(level, argument.span),
                        ArgTerm::Value(Val::Meta(level)),
                    ));
                }
                let name = atom(argument)?;
                let (index, actual) = stack_binder(&self.context, name)
                    .ok_or_else(|| super::reject(argument.span, "apply value is unbound"))?;
                let Sort::Value(actual) = actual else {
                    return Err(super::reject(argument.span, "apply value has wrong code"));
                };
                let actual = ty_of_code(actual, argument.span)?;
                if let Err(error) = self.constrain_ty(expected, &actual, argument.span) {
                    return Err(explain_constraint(
                        &self.metas,
                        error,
                        argument.span,
                        &binder_origin("the argument", &binder, position, &identity),
                    ));
                }
                let level = u32_level(index, argument.span)?;
                Ok((
                    Out::Done(argument.clone()),
                    ArgTerm::Value(Val::Level(level)),
                ))
            }
            SortTerm::Premise(expected) => {
                if is_infer_form(argument) {
                    let origin = binder_origin("premise inference", &binder, position, &identity);
                    return Err(self.premise_refusal(argument.span, expected, &origin));
                }
                if is_inferable(argument) {
                    let child = deeper(depth, argument.span)?;
                    let inferred = self.infer(argument, child)?;
                    if let Err(error) = self.constrain_prop(&inferred.prop, expected, argument.span)
                    {
                        return Err(explain_constraint(
                            &self.metas,
                            error,
                            argument.span,
                            &binder_origin("the witness", &binder, position, &identity),
                        ));
                    }
                    return Ok((inferred.out, ArgTerm::Premise));
                }
                let position = self.deferred.len();
                self.deferred.push(Some(Deferred {
                    form: argument.clone(),
                    expected: clone_prop(expected),
                    depth,
                }));
                Ok((Out::Hole(position, argument.span), ArgTerm::Premise))
            }
        }
    }

    /// Refuse a premise placeholder, listing candidates without choosing.
    /// The receiver is exclusive for the pinned Aeneas, as `verify_ground`'s.
    /// `origin` names the premise binder, its position and the head.
    fn premise_refusal(&mut self, span: Span, expected: &PropTerm, origin: &str) -> Diagnostic {
        const MAX_CANDIDATES: usize = 8;
        let mut detail = String::from(origin);
        detail.push_str("; write the explicit witness (witness identity is never chosen)");
        let mut listed = 0usize;
        let mut total = 0usize;
        let ground = self.ground_prop(expected, span).ok();
        if let Some(ground) = ground {
            let mut level = 0usize;
            while level < self.context.total {
                if let Some((name, Sort::Premise(prop))) = binder_parts(&self.context, level) {
                    if super::proposition_equal(&prop, &ground) {
                        total = total.saturating_add(1);
                        if listed < MAX_CANDIDATES {
                            detail.push_str("; local premise `");
                            detail.push_str(&name);
                            detail.push('`');
                            if let Some(binder_span) = span_at(&self.context_spans, level) {
                                detail.push_str(" at ");
                                detail.push_str(&span_text(binder_span));
                            }
                            listed = listed.saturating_add(1);
                        }
                    }
                }
                level = level.saturating_add(1);
            }
            let mut candidate = 0usize;
            while candidate < self.available.len() {
                if let Some(prop) = self.candidate_goal(candidate) {
                    if super::proposition_equal(&prop, &ground) {
                        total = total.saturating_add(1);
                        if listed < MAX_CANDIDATES {
                            detail.push_str("; accepted `");
                            detail.push_str(&self.candidate_identity(candidate));
                            detail.push('`');
                            listed = listed.saturating_add(1);
                        }
                    }
                }
                candidate = candidate.saturating_add(1);
            }
        }
        if total > listed {
            detail.push_str("; and ");
            detail.push_str(&super::decimal(total.saturating_sub(listed)));
            detail.push_str(" more candidates");
        }
        unsupported(span, &detail)
    }

    /// An owned copy of the identity of available proof `position`.
    fn candidate_identity(&self, position: usize) -> String {
        match self.available.get(position) {
            Some(candidate) => candidate.identity.clone(),
            None => String::new(),
        }
    }

    /// Parse one available proof's goal for candidate listing, under its own
    /// bounded meter so a diagnostic never consumes the elaboration budget.
    fn candidate_goal(&self, position: usize) -> Option<Proposition> {
        let goal = &self.available.get(position)?.goal;
        let meter = Meter {
            nodes: 0,
            work: 0,
            normalization: 0,
            substitution: 0,
            config: self.meter.config,
        };
        let (_, parsed) = proposition(goal, &WorkStack::new(), meter, 0);
        parsed.ok()
    }

    /// Elaborate the A2 explicit spine `(app H a1 … an)`.
    fn app(&mut self, form: &Form, depth: u32) -> Result<Inferred, Diagnostic> {
        let items = list(form)?;
        if items.len() < 3 {
            return Err(unsupported(
                form.span,
                "app requires a head and at least one argument",
            ));
        }
        let head_form = &items[1];
        if !is_app_head(head_form) {
            return Err(unsupported(
                head_form.span,
                "app head must be a use, app or apply spine",
            ));
        }
        let head_depth = deeper(depth, head_form.span)?;
        let mut inferred = self.infer(head_form, head_depth)?;
        self.app_spine(form, items, &mut inferred, 2, depth)?;
        Ok(inferred)
    }

    /// Fill the spine's binders from `argument_at` until it completes, one
    /// telescope step per call. A recursion, not a loop: the spine is part of
    /// the elaborator's own recursion (check, infer, app), and the pinned Lean
    /// cannot prove a loop inside that recursion monotone.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; one call per telescope binder of the finite, depth-bounded head proposition"
    )]
    fn app_spine(
        &mut self,
        form: &Form,
        items: &[Form],
        inferred: &mut Inferred,
        argument_at: usize,
        depth: u32,
    ) -> Result<(), Diagnostic> {
        let (next_argument, is_complete) =
            self.app_step(form, items, inferred, argument_at, depth)?;
        if is_complete {
            return Ok(());
        }
        self.app_spine(form, items, inferred, next_argument, depth)
    }

    /// One telescope step of an A2 spine: fill one binder, or report that the
    /// spine is complete. Returns the updated spine state and argument cursor.
    fn app_step(
        &mut self,
        form: &Form,
        items: &[Form],
        inferred: &mut Inferred,
        argument_at: usize,
        depth: u32,
    ) -> Result<(usize, bool), Diagnostic> {
        let prop = clone_prop(&inferred.prop);
        let (index, sort, body) = match prop {
            PropTerm::Pi(index, sort, body) => {
                let current = u32_level(self.context.total, form.span)?;
                if index != current {
                    return Err(super::reject(form.span, "apply binder scope mismatch"));
                }
                (index, sort, body)
            }
            _ => {
                if argument_at < items.len() {
                    return Err(unsupported(
                        form.span,
                        &text![
                            "app over-application of ",
                            &inferred.head.identity,
                            ": argument ",
                            &super::decimal(argument_at.saturating_sub(1)),
                            " has no binder",
                        ],
                    ));
                }
                return Ok((argument_at, true));
            }
        };
        let (arg_out, arg_term, next_argument) = if matches!(sort, SortTerm::Codes) {
            let current = u32_level(self.context.total, form.span)?;
            let (binder, position) = inferred.head.binder();
            inferred.head.next = position
                .checked_add(1)
                .ok_or_else(|| stage_exhausted(form.span, "telescope"))?;
            let identity = inferred.head.identity.clone();
            let level = self.alloc_code(&binder, position, &identity, form.span, current)?;
            (
                Out::Infer(level, form.span),
                ArgTerm::Code(Ty::Meta(level)),
                argument_at,
            )
        } else if argument_at < items.len() {
            let argument = &items[argument_at];
            let child = deeper(depth, argument.span)?;
            let (arg_out, arg_term) = self.argument(&sort, argument, &mut inferred.head, child)?;
            (arg_out, arg_term, argument_at.saturating_add(1))
        } else {
            let (binder, position) = inferred.head.binder();
            return Err(unsupported(
                form.span,
                &text![
                    "app under-application: ",
                    &binder_origin("no argument", &binder, position, &inferred.head.identity,),
                ],
            ));
        };
        let nodes = prop_size_of(&body);
        self.meter_step(MeterStep::Substitute(nodes), "substitution", form.span)?;
        let next = inst_prop(&body, index, &arg_term, form.span)?;
        let previous = core::mem::replace(&mut inferred.out, out_atom("apply", form.span));
        inferred.out = Out::List(
            alloc::vec![out_atom("apply", form.span), previous, arg_out],
            form.span,
        );
        inferred.prop = next;
        Ok((next_argument, false))
    }

    /// Resolve one constructed term after solving, mapping inserted nodes to
    /// their abbreviation spans. A frame walk with a value stack, as the
    /// checker assembles its terms: the pinned Aeneas refuses a recursion
    /// inside a loop. The receiver is exclusive for the pinned Aeneas, as
    /// `verify_ground`'s.
    ///
    /// The deferred premise forms move into one owned forest with the root
    /// last, and every frame borrows it only through the free step function's
    /// one forest parameter (the root too is a forest entry the step looks
    /// up): the pinned Aeneas refuses frames whose borrows come from different
    /// loans. A solved argument is handed back to this loop, which renders it.
    /// Finishing is the last use of the deferred forms.
    fn finish_out(&mut self, out: Out, depth: u32) -> Result<Form, Diagnostic> {
        let span = out_span(&out);
        let bound = self.meter.tree_bounds.frames;
        let solutions = self.solutions();
        let mut forest = self.deferred_forms.split_off(0);
        let root = forest.len();
        forest.push(out);
        finish_forest(&forest, root, &solutions, bound, depth, span)
    }

    /// The explicit form of every metavariable's solution, by index, or the
    /// refusal its rendering meets. Each inserted node carries its
    /// metavariable's allocation span, so the rendering is the one the node
    /// would get; the walk reports a refusal only when it reaches the node.
    /// The receiver is exclusive for the pinned Aeneas, as `verify_ground`'s.
    fn solutions(&mut self) -> Vec<Result<Form, Diagnostic>> {
        let mut solutions = Vec::with_capacity(self.metas.len());
        let mut index = 0usize;
        while index < self.metas.len() {
            solutions.push(self.solution_at(index));
            index = index.saturating_add(1);
        }
        solutions
    }

    /// The explicit form of metavariable `index`'s solution.
    fn solution_at(&self, index: usize) -> Result<Form, Diagnostic> {
        let span = self.metas[index].span;
        let meta = u32_level(index, span)?;
        self.finish_infer(meta, span)
    }

    /// The explicit form of one solved argument, rendered with the binder
    /// names at its allocation.
    fn finish_infer(&self, meta: u32, span: Span) -> Result<Form, Diagnostic> {
        let index = meta_index(meta, span)?;
        let Some(entry) = self.metas.get(index) else {
            return Err(internal(span, "unknown metavariable"));
        };
        let kind = entry.kind;
        let names = entry.names.clone();
        match kind {
            MetaKind::Code => {
                let solved = self.resolve_ty(&Ty::Meta(meta), span)?;
                self.ty_form(&solved, span, &names)
            }
            MetaKind::Value => {
                let solved = self.resolve_val(Val::Meta(meta), span)?;
                let name = self.render_val(solved, span, &names)?;
                Ok(Form {
                    kind: FormKind::Atom(name),
                    span,
                })
            }
        }
    }

    /// Render a ground code as its core source form.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; rendering follows the finite ground code"
    )]
    fn ty_form(&self, ty: &Ty, span: Span, names: &[String]) -> Result<Form, Diagnostic> {
        match ty {
            Ty::Unit => Ok(atom_form("Unit", span)),
            Ty::Bool => Ok(atom_form("Bool", span)),
            Ty::I64 => Ok(atom_form("I64", span)),
            Ty::Text => Ok(atom_form("Text", span)),
            Ty::Var(level) => {
                let name = level_name(names, *level, span)?;
                Ok(atom_form(&name, span))
            }
            // Each component is rendered before `vec!`: a `?` inside its
            // elements lowers to an uninitialised box the pinned Aeneas
            // library does not model.
            Ty::Pair(left, right) => {
                let left = self.ty_form(left, span, names)?;
                let right = self.ty_form(right, span, names)?;
                Ok(Form {
                    kind: FormKind::List(alloc::vec![atom_form("Pair", span), left, right]),
                    span,
                })
            }
            Ty::Sum(left, right) => {
                let left = self.ty_form(left, span, names)?;
                let right = self.ty_form(right, span, names)?;
                Ok(Form {
                    kind: FormKind::List(alloc::vec![atom_form("Sum", span), left, right]),
                    span,
                })
            }
            Ty::List(item) => {
                let item = self.ty_form(item, span, names)?;
                Ok(Form {
                    kind: FormKind::List(alloc::vec![atom_form("List", span), item]),
                    span,
                })
            }
            Ty::Meta(_) => Err(internal(
                span,
                "unresolved code metavariable in an expansion",
            )),
        }
    }

    /// Render a ground code as record text.
    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; rendering follows the finite ground code"
    )]
    fn render_ty(&self, ty: &Ty, span: Span, names: &[String]) -> Result<String, Diagnostic> {
        match ty {
            Ty::Unit => Ok(String::from("Unit")),
            Ty::Bool => Ok(String::from("Bool")),
            Ty::I64 => Ok(String::from("I64")),
            Ty::Text => Ok(String::from("Text")),
            Ty::Var(level) => level_name(names, *level, span),
            Ty::Pair(left, right) => {
                let left = self.render_ty(left, span, names)?;
                let right = self.render_ty(right, span, names)?;
                Ok(text!["(Pair ", &left, " ", &right, ")"])
            }
            Ty::Sum(left, right) => {
                let left = self.render_ty(left, span, names)?;
                let right = self.render_ty(right, span, names)?;
                Ok(text!["(Sum ", &left, " ", &right, ")"])
            }
            Ty::List(item) => {
                let item = self.render_ty(item, span, names)?;
                Ok(text!["(List ", &item, ")"])
            }
            Ty::Meta(_) => Err(internal(span, "unresolved code metavariable in a solution")),
        }
    }

    /// Render a solved value as its binder name.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper clones the binder name through level_name and builds a diagnostic, so the const suggestion does not apply"
    )]
    fn render_val(&self, value: Val, span: Span, names: &[String]) -> Result<String, Diagnostic> {
        match value {
            Val::Level(level) => level_name(names, level, span),
            Val::Meta(_) => Err(internal(
                span,
                "unresolved value metavariable in a solution",
            )),
        }
    }
}

/// The binder name at a solution level, from the metavariable's snapshot.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper clones the recorded String name, which is not a const operation"
)]
fn level_name(names: &[String], level: u32, span: Span) -> Result<String, Diagnostic> {
    let index = match usize::try_from(level) {
        Ok(index) => index,
        Err(_) => {
            return Err(internal(
                span,
                "solution level exceeds the host address space",
            ))
        }
    };
    match names.get(index) {
        Some(name) => Ok(name.clone()),
        None => Err(internal(
            span,
            "solution level outside the recorded proof context",
        )),
    }
}

/// The last available proof named `name`, as a reverse index walk.
fn last_available(available: &[Available], name: &str) -> Option<usize> {
    let mut remaining = available.len();
    let mut found = None;
    while found.is_none() && remaining > 0 {
        remaining -= 1;
        if available[remaining].name == name {
            found = Some(remaining);
        }
    }
    found
}

/// One atom form at `span`.
fn atom_form(text: &str, span: Span) -> Form {
    Form {
        kind: FormKind::Atom(String::from(text)),
        span,
    }
}

/// One constructor atom as a constructed item.
fn out_atom(text: &str, span: Span) -> Out {
    Out::Done(atom_form(text, span))
}

/// Whether `form` is the nullary `(infer)` placeholder. Binder names are
/// atoms, so the form never collides with one.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; comparing the head atom uses the non-const PartialEq of Option<&str>, so the const suggestion does not apply"
)]
fn is_infer_form(form: &Form) -> bool {
    match &form.kind {
        FormKind::List(items) => items.len() == 1 && head_atom(items) == Some("infer"),
        FormKind::Atom(_) => false,
    }
}

/// Whether `items` is `(exec-literal (infer))`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; comparing the head atom uses the non-const PartialEq of Option<&str>, so the const suggestion does not apply"
)]
fn is_exec_literal_infer(items: &[Form]) -> bool {
    items.len() == 2 && head_atom(items) == Some("exec-literal") && is_infer_form(&items[1])
}

/// Whether `form` is a proof form whose proposition the elaborator infers.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper calls the non-const operation recognizer and compares Option<&str> through PartialEq, so the const suggestion does not apply"
)]
fn is_inferable(form: &Form) -> bool {
    operation(form, "use", 1).is_ok()
        || operation(form, "apply", 2).is_ok()
        || operation(form, "refl", 1).is_ok()
        || super::list_head_atom(form) == Some("app")
}

/// Whether `form` can head an `app` spine: a `use`, an `apply` or an `app`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper calls the non-const operation recognizer and compares Option<&str> through PartialEq, so the const suggestion does not apply"
)]
fn is_app_head(form: &Form) -> bool {
    operation(form, "use", 1).is_ok()
        || operation(form, "apply", 2).is_ok()
        || super::list_head_atom(form) == Some("app")
}

/// Whether a term contains `(infer)` or an `app` spine.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn scan_abbreviation(form: &Form, limits: Limits) -> Result<Abbreviation, Diagnostic> {
    let bound = scan_bound(limits, form.span)?;
    let mut abbreviation = Abbreviation {
        infer: false,
        app: false,
    };
    let mut visited = 0u32;
    let mut pending = WorkStack::new();
    pending
        .push_bounded(ScanFrame::Enter(form, 0), bound)
        .map_err(|()| stage_exhausted(form.span, "tree frame"))?;
    let mut outcome = Ok(());
    while let Some(frame) = pending.pop_bounded() {
        outcome = abbreviation_step(
            frame,
            limits,
            bound,
            &mut abbreviation,
            &mut visited,
            &mut pending,
        );
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    Ok(abbreviation)
}

/// One frame of a read-only proof-term scan, in the checker's
/// `ValidationFrame` shape: a node to visit, or the children of a list still to
/// schedule, one per step. The pinned Aeneas extracts this shape; a loop that
/// pushes every borrowed child at once it refuses.
#[octet::sealed_enum]
enum ScanFrame<'a> {
    Enter(&'a Form, u32),
    /// A list's items, the index of the next item and the items' depth.
    Children(&'a [Form], usize, u32),
}

/// The frame bound of a scan over `limits`-bounded terms: the checker's.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper calls the non-const TreeBounds::new"
)]
fn scan_bound(limits: Limits, span: Span) -> Result<usize, Diagnostic> {
    match TreeBounds::new(limits) {
        Some(bounds) => Ok(bounds.frames),
        None => Err(stage_exhausted(span, "tree frame")),
    }
}

/// Visit one abbreviation-scan frame: record the abbreviation or schedule a
/// list's next child. Children are visited from the last, as the scan always
/// has; the loop stops at this helper's first refusal.
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
fn abbreviation_step<'a>(
    frame: ScanFrame<'a>,
    limits: Limits,
    bound: usize,
    abbreviation: &mut Abbreviation,
    visited: &mut u32,
    pending: &mut WorkStack<ScanFrame<'a>>,
) -> Result<(), Diagnostic> {
    match frame {
        ScanFrame::Enter(current, depth) => {
            if depth > limits.depth {
                return Err(stage_exhausted(current.span, "depth"));
            }
            *visited = visited.saturating_add(1);
            if *visited > limits.nodes {
                return Err(stage_exhausted(current.span, "node"));
            }
            match &current.kind {
                FormKind::Atom(_) => Ok(()),
                FormKind::List(items) => {
                    if is_infer_form(current) {
                        abbreviation.infer = true;
                        return Ok(());
                    }
                    if head_atom(items) == Some("app") {
                        abbreviation.app = true;
                    }
                    if items.is_empty() {
                        return Ok(());
                    }
                    let frame = ScanFrame::Children(items, items.len(), depth.saturating_add(1));
                    pending
                        .push_bounded(frame, bound)
                        .map_err(|()| stage_exhausted(current.span, "tree frame"))
                }
            }
        }
        // `remaining` items are left; the last of them is visited next.
        ScanFrame::Children(items, remaining, depth) => {
            let Some(index) = remaining.checked_sub(1) else {
                return Ok(());
            };
            let Some(item) = items.get(index) else {
                return Ok(());
            };
            if index > 0 {
                pending
                    .push_bounded(ScanFrame::Children(items, index, depth), bound)
                    .map_err(|()| stage_exhausted(item.span, "tree frame"))?;
            }
            pending
                .push_bounded(ScanFrame::Enter(item, depth), bound)
                .map_err(|()| stage_exhausted(item.span, "tree frame"))
        }
    }
}

/// The span of the first `(infer)` placeholder, in walk order.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn find_infer_span(form: &Form, limits: Limits) -> Result<Option<Span>, Diagnostic> {
    let bound = scan_bound(limits, form.span)?;
    let mut visited = 0u32;
    let mut found = None;
    let mut pending = WorkStack::new();
    pending
        .push_bounded(ScanFrame::Enter(form, 0), bound)
        .map_err(|()| stage_exhausted(form.span, "tree frame"))?;
    let mut outcome = Ok(());
    while let Some(frame) = pending.pop_bounded() {
        match infer_span_step(frame, limits, bound, &mut visited, &mut pending) {
            Ok(span) => found = span,
            Err(error) => outcome = Err(error),
        }
        if outcome.is_err() || found.is_some() {
            break;
        }
    }
    outcome?;
    Ok(found)
}

/// Visit one infer-span frame: report a placeholder or schedule a list's next
/// child, from the first, so the leftmost placeholder is found first.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
fn infer_span_step<'a>(
    frame: ScanFrame<'a>,
    limits: Limits,
    bound: usize,
    visited: &mut u32,
    pending: &mut WorkStack<ScanFrame<'a>>,
) -> Result<Option<Span>, Diagnostic> {
    match frame {
        ScanFrame::Enter(current, depth) => {
            if depth > limits.depth {
                return Err(stage_exhausted(current.span, "depth"));
            }
            *visited = visited.saturating_add(1);
            if *visited > limits.nodes {
                return Err(stage_exhausted(current.span, "node"));
            }
            match &current.kind {
                FormKind::Atom(_) => Ok(None),
                FormKind::List(items) => {
                    if is_infer_form(current) {
                        return Ok(Some(current.span));
                    }
                    if items.is_empty() {
                        return Ok(None);
                    }
                    let frame = ScanFrame::Children(items, 0, depth.saturating_add(1));
                    pending
                        .push_bounded(frame, bound)
                        .map_err(|()| stage_exhausted(current.span, "tree frame"))?;
                    Ok(None)
                }
            }
        }
        // `index` is the next item to visit, from the first.
        ScanFrame::Children(items, index, depth) => {
            let Some(item) = items.get(index) else {
                return Ok(None);
            };
            let next = index.saturating_add(1);
            if next < items.len() {
                pending
                    .push_bounded(ScanFrame::Children(items, next, depth), bound)
                    .map_err(|()| stage_exhausted(item.span, "tree frame"))?;
            }
            pending
                .push_bounded(ScanFrame::Enter(item, depth), bound)
                .map_err(|()| stage_exhausted(item.span, "tree frame"))?;
            Ok(None)
        }
    }
}

/// The syntactic position of one proof subterm.
#[octet::sealed_enum]
#[derive(Clone, Copy)]
enum Position {
    Root,
    UseTarget,
    ApplyHead,
    ApplyArgument,
    ReflOperand,
    IntroBinder,
    IntroName,
    IntroSort,
    IntroBody,
    SubstMotive,
    SubstBinder,
    SubstBinderName,
    SubstBinderSort,
    SubstProposition,
    SubstEvidence,
    SubstPremise,
    AppHead,
    AppArgument,
    ExecLiteral,
    RuleArgument,
}

impl Position {
    /// The refusal detail for `(infer)` at this position.
    const fn refusal(self) -> &'static str {
        match self {
            Self::Root => "infer is not a whole proof term",
            Self::UseTarget => "infer cannot name a proof",
            Self::ApplyHead | Self::AppHead => "infer cannot be an application head",
            Self::ReflOperand => "infer cannot be a refl operand",
            Self::IntroBinder => "infer cannot be an intro binder",
            Self::IntroName => "infer cannot be an intro binder name",
            Self::IntroSort => "infer cannot be an intro binder sort",
            Self::SubstMotive => "infer cannot be a subst motive",
            Self::SubstBinder => "infer cannot be a subst binder",
            Self::SubstBinderName => "infer cannot be a subst binder name",
            Self::SubstBinderSort => "infer cannot be a subst binder sort",
            Self::SubstProposition => "infer cannot be a subst motive or motive code",
            Self::SubstEvidence => "infer cannot be a subst evidence term",
            Self::SubstPremise => "infer cannot be a subst premise",
            Self::ApplyArgument | Self::AppArgument | Self::ExecLiteral => {
                "infer is admissible only at apply, app or exec-literal arguments"
            }
            Self::RuleArgument => "infer is not admissible at this rule position",
            Self::IntroBody => "infer is admissible only at apply, app or exec-literal arguments",
        }
    }
}

/// Refuse every `(infer)` outside the three specified abbreviation positions.
fn scan_positions(form: &Form, limits: Limits) -> Result<(), Diagnostic> {
    let bound = scan_bound(limits, form.span)?;
    let mut visited = 0u32;
    let mut pending = WorkStack::new();
    pending
        .push_bounded(PositionFrame::Enter(form, Position::Root, 0), bound)
        .map_err(|()| stage_exhausted(form.span, "tree frame"))?;
    let mut outcome = Ok(());
    while let Some(frame) = pending.pop_bounded() {
        outcome = position_step(frame, limits, bound, &mut visited, &mut pending);
        if outcome.is_err() {
            break;
        }
    }
    outcome
}

/// One frame of the position scan: a node to visit at its position, or the
/// children of a list still to schedule, from the first, one per step.
#[octet::sealed_enum]
enum PositionFrame<'a> {
    Enter(&'a Form, Position, u32),
    /// A list's items, the list's position, the next item and the items'
    /// depth.
    Children(&'a [Form], Position, usize, u32),
}

/// Visit one position-scan frame: refuse a misplaced placeholder or schedule
/// a list's next child. The loop stops at this helper's first refusal.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
fn position_step<'a>(
    frame: PositionFrame<'a>,
    limits: Limits,
    bound: usize,
    visited: &mut u32,
    pending: &mut WorkStack<PositionFrame<'a>>,
) -> Result<(), Diagnostic> {
    match frame {
        PositionFrame::Enter(current, position, depth) => {
            if depth > limits.depth {
                return Err(stage_exhausted(current.span, "depth"));
            }
            *visited = visited.saturating_add(1);
            if *visited > limits.nodes {
                return Err(stage_exhausted(current.span, "node"));
            }
            if is_infer_form(current) {
                return infer_admissible_at(position, current.span);
            }
            match &current.kind {
                FormKind::Atom(_) => Ok(()),
                FormKind::List(items) => {
                    if items.is_empty() {
                        return Ok(());
                    }
                    let frame =
                        PositionFrame::Children(items, position, 0, depth.saturating_add(1));
                    pending
                        .push_bounded(frame, bound)
                        .map_err(|()| stage_exhausted(current.span, "tree frame"))
                }
            }
        }
        // The list's head atom is no subterm, except a binder list's name.
        PositionFrame::Children(items, position, index, depth) => {
            let Some(item) = items.get(index) else {
                return Ok(());
            };
            let next = index.saturating_add(1);
            if next < items.len() {
                pending
                    .push_bounded(PositionFrame::Children(items, position, next, depth), bound)
                    .map_err(|()| stage_exhausted(item.span, "tree frame"))?;
            }
            let is_head =
                index == 0 && !matches!(position, Position::IntroBinder | Position::SubstBinder);
            if is_head {
                return Ok(());
            }
            let child = child_position(head_atom(items), position, index);
            pending
                .push_bounded(PositionFrame::Enter(item, child, depth), bound)
                .map_err(|()| stage_exhausted(item.span, "tree frame"))
        }
    }
}

/// Whether `(infer)` may stand at `position`: only the three selected
/// positions admit it, and every other position refuses with its own name.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; a refusal is built by the non-const unsupported constructor"
)]
fn infer_admissible_at(position: Position, span: Span) -> Result<(), Diagnostic> {
    match position {
        Position::ApplyArgument | Position::AppArgument | Position::ExecLiteral => Ok(()),
        other => Err(unsupported(span, other.refusal())),
    }
}

/// The position of item `index` of a list headed by `head`.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the grammar table is one total mapping from constructors to subterm positions; every input selects a position, so there is no invariant to assert"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; matching constructor names compares str through its non-const equality, so the const suggestion does not apply"
)]
fn child_position(head: Option<&str>, position: Position, index: usize) -> Position {
    match (position, index) {
        (Position::IntroBinder, 0) => return Position::IntroName,
        (Position::IntroBinder, 1) => return Position::IntroSort,
        (Position::SubstBinder, 0) => return Position::SubstBinderName,
        (Position::SubstBinder, 1) => return Position::SubstBinderSort,
        (Position::SubstMotive, 1) => return Position::SubstBinder,
        (Position::SubstMotive, 2) => return Position::SubstProposition,
        _ => {}
    }
    match head {
        Some("use") if index == 1 => Position::UseTarget,
        Some("apply") if index == 1 => Position::ApplyHead,
        Some("apply") if index == 2 => Position::ApplyArgument,
        Some("refl") if index == 1 => Position::ReflOperand,
        Some("intro") if index == 1 => Position::IntroBinder,
        Some("intro") if index == 2 => Position::IntroBody,
        Some("subst") if index == 1 => Position::SubstMotive,
        Some("subst") if index == 2 => Position::SubstEvidence,
        Some("subst") if index == 3 => Position::SubstPremise,
        Some("app") if index == 1 => Position::AppHead,
        Some("app") if index >= 2 => Position::AppArgument,
        Some("exec-literal") if index == 1 => Position::ExecLiteral,
        Some(_) | None => Position::RuleArgument,
    }
}

/// The binder names of a proposition's leading dependent `Pi` telescope.
fn pi_names(form: &Form) -> Vec<String> {
    let mut names = Vec::new();
    push_pi_names(form, 64, &mut names);
    names
}

/// Push the binder names of the `Pi` telescope at `form`, at most
/// `remaining`. A bounded recursion, one `Pi` per call: the pinned Aeneas
/// refuses a loop that re-borrows its cursor into the telescope.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; the walk descends one Pi binder per call and stops after 64 calls"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the walk appends each binder name with Vec::push, so it needs the caller's growable vector, not a slice."
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it appends to an exclusively borrowed, caller-owned name list for this bounded walk; the borrowed form stays immutable."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the walk clones names and pushes onto a Vec, neither of which is const"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; a form that is not a Pi telescope ends the walk without a name; the crate's elaboration tests pin the names it collects"
)]
fn push_pi_names(form: &Form, remaining: u32, names: &mut Vec<String>) {
    if remaining == 0 {
        return;
    }
    let FormKind::List(items) = &form.kind else {
        return;
    };
    if items.len() != 3 || head_atom(items) != Some("Pi") {
        return;
    }
    let FormKind::List(binding) = &items[1].kind else {
        return;
    };
    if binding.len() != 2 {
        return;
    }
    let FormKind::Atom(name) = &binding[0].kind else {
        return;
    };
    names.push(name.clone());
    push_pi_names(&items[2], remaining.saturating_sub(1), names);
}

/// Convert a parsed code into a metavariable term.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; conversion follows the finite parsed code"
)]
fn ty_of_code(code: &Code, span: Span) -> Result<Ty, Diagnostic> {
    match code {
        Code::Unit => Ok(Ty::Unit),
        Code::Bool => Ok(Ty::Bool),
        Code::I64 => Ok(Ty::I64),
        Code::Text => Ok(Ty::Text),
        Code::Variable(index) => Ok(Ty::Var(u32_level(*index, span)?)),
        Code::Pair(left, right) => Ok(Ty::Pair(
            Box::new(ty_of_code(left, span)?),
            Box::new(ty_of_code(right, span)?),
        )),
        Code::Sum(left, right) => Ok(Ty::Sum(
            Box::new(ty_of_code(left, span)?),
            Box::new(ty_of_code(right, span)?),
        )),
        Code::List(item) => Ok(Ty::List(Box::new(ty_of_code(item, span)?))),
    }
}

/// Convert a parsed sort into a metavariable term.
fn sort_term(sort: &Sort, span: Span) -> Result<SortTerm, Diagnostic> {
    match sort {
        Sort::Codes => Ok(SortTerm::Codes),
        Sort::Value(code) => Ok(SortTerm::Value(ty_of_code(code, span)?)),
        Sort::Premise(prop) => Ok(SortTerm::Premise(Box::new(prop_term(prop, span)?))),
    }
}

/// Convert a parsed proposition into a metavariable term.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; conversion follows the finite parsed proposition"
)]
fn prop_term(prop: &Proposition, span: Span) -> Result<PropTerm, Diagnostic> {
    match prop {
        Proposition::Pi(index, sort, body) => Ok(PropTerm::Pi(
            u32_level(*index, span)?,
            sort_term(sort, span)?,
            Box::new(prop_term(body, span)?),
        )),
        Proposition::Eq(code, left, right) => Ok(PropTerm::Eq(
            ty_of_code(code, span)?,
            Val::Level(u32_level(*left, span)?),
            Val::Level(u32_level(*right, span)?),
        )),
    }
}

/// Deep-copy one metavariable code.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; the copy follows the finite term tree"
)]
fn clone_ty(ty: &Ty) -> Ty {
    match ty {
        Ty::Unit => Ty::Unit,
        Ty::Bool => Ty::Bool,
        Ty::I64 => Ty::I64,
        Ty::Text => Ty::Text,
        Ty::Var(index) => Ty::Var(*index),
        Ty::Meta(meta) => Ty::Meta(*meta),
        Ty::Pair(left, right) => Ty::Pair(Box::new(clone_ty(left)), Box::new(clone_ty(right))),
        Ty::Sum(left, right) => Ty::Sum(Box::new(clone_ty(left)), Box::new(clone_ty(right))),
        Ty::List(item) => Ty::List(Box::new(clone_ty(item))),
    }
}

/// Deep-copy one metavariable proposition.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; the copy follows the finite term tree"
)]
fn clone_prop(prop: &PropTerm) -> PropTerm {
    match prop {
        PropTerm::Pi(index, sort, body) => PropTerm::Pi(
            *index,
            match sort {
                SortTerm::Codes => SortTerm::Codes,
                SortTerm::Value(ty) => SortTerm::Value(clone_ty(ty)),
                SortTerm::Premise(inner) => SortTerm::Premise(Box::new(clone_prop(inner))),
            },
            Box::new(clone_prop(body)),
        ),
        PropTerm::Eq(code, left, right) => PropTerm::Eq(clone_ty(code), *left, *right),
    }
}

/// Whether `meta` occurs in a resolved code.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; the worklist is bounded by the 4,096-node budget the loop decrements before every push"
)]
fn occurs_code(meta: u32, ty: &Ty) -> bool {
    let mut pending = Vec::new();
    pending.push(ty);
    let mut budget = 4096usize;
    let mut is_found = false;
    let mut next = pending.pop();
    while !is_found && next.is_some() {
        if let Some(node) = next {
            if budget == 0 {
                // The original walk treated a spent budget as conservative
                // membership; keep that exact outcome.
                is_found = true;
            } else {
                budget -= 1;
                match node {
                    Ty::Meta(other) => is_found = *other == meta,
                    Ty::Pair(left, right) | Ty::Sum(left, right) => {
                        pending.push(right);
                        pending.push(left);
                    }
                    Ty::List(item) => pending.push(item),
                    Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Var(_) => {}
                }
            }
            next = pending.pop();
        }
    }
    is_found
}

/// The node count of one metavariable code.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; the count follows the finite term tree"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the count dereferences Box children through the non-const Deref impl, so the const suggestion does not apply"
)]
fn ty_size(ty: &Ty) -> usize {
    match ty {
        Ty::Unit | Ty::Bool | Ty::I64 | Ty::Text | Ty::Var(_) | Ty::Meta(_) => 1,
        Ty::Pair(left, right) | Ty::Sum(left, right) => 1usize
            .saturating_add(ty_size(left))
            .saturating_add(ty_size(right)),
        Ty::List(item) => 1usize.saturating_add(ty_size(item)),
    }
}

/// The node count of one metavariable proposition.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; the count follows the finite term tree"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the count dereferences Box children through the non-const Deref impl, so the const suggestion does not apply"
)]
fn prop_size_of(prop: &PropTerm) -> usize {
    match prop {
        PropTerm::Pi(_, sort, body) => {
            let sort_nodes = match sort {
                SortTerm::Codes => 1,
                SortTerm::Value(ty) => ty_size(ty),
                SortTerm::Premise(inner) => prop_size_of(inner),
            };
            1usize
                .saturating_add(sort_nodes)
                .saturating_add(prop_size_of(body))
        }
        PropTerm::Eq(code, _, _) => 3usize.saturating_add(ty_size(code)),
    }
}

/// Render a code for a clash diagnostic, with metavariables as `?n`.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; the rendering follows the finite term tree"
)]
fn describe_ty(ty: &Ty) -> String {
    match ty {
        Ty::Unit => String::from("Unit"),
        Ty::Bool => String::from("Bool"),
        Ty::I64 => String::from("I64"),
        Ty::Text => String::from("Text"),
        Ty::Var(index) => text!["#", &u32_text(*index)],
        Ty::Meta(meta) => text!["?", &u32_text(*meta)],
        Ty::Pair(left, right) => {
            text!["(Pair ", &describe_ty(left), " ", &describe_ty(right), ")"]
        }
        Ty::Sum(left, right) => {
            text!["(Sum ", &describe_ty(left), " ", &describe_ty(right), ")"]
        }
        Ty::List(item) => text!["(List ", &describe_ty(item), ")"],
    }
}

/// Render a value for a clash diagnostic.
fn describe_val(value: Val) -> String {
    match value {
        Val::Level(level) => text!["#", &u32_text(level)],
        Val::Meta(meta) => text!["?", &u32_text(meta)],
    }
}

/// Render a sort for a clash diagnostic.
fn describe_sort(sort: &SortTerm) -> String {
    match sort {
        SortTerm::Codes => String::from("Type0"),
        SortTerm::Value(ty) => describe_ty(ty),
        SortTerm::Premise(prop) => describe_prop(prop),
    }
}

/// Render a proposition for a clash diagnostic.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; the rendering follows the finite term tree"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the rendering builds Strings, which is not const"
)]
fn describe_prop(prop: &PropTerm) -> String {
    match prop {
        PropTerm::Pi(index, sort, body) => text![
            "(Pi #",
            &u32_text(*index),
            " ",
            &describe_sort(sort),
            " ",
            &describe_prop(body),
            ")",
        ],
        PropTerm::Eq(code, left, right) => text![
            "(Eq ",
            &describe_ty(code),
            " ",
            &describe_val(*left),
            " ",
            &describe_val(*right),
            ")",
        ],
    }
}

/// Render one proof form as core source text.
fn render_form(form: &Form, limits: Limits) -> Result<String, Diagnostic> {
    let bound = scan_bound(limits, form.span)?;
    let mut text = String::new();
    let mut pending = WorkStack::new();
    pending
        .push_bounded(ScanFrame::Enter(form, 0), bound)
        .map_err(|()| stage_exhausted(form.span, "tree frame"))?;
    let mut outcome = Ok(());
    while let Some(frame) = pending.pop_bounded() {
        outcome = render_step(frame, limits, bound, &mut text, &mut pending);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    Ok(text)
}

/// Render one frame: append an atom or a list's opening parenthesis, or a
/// list's next item separator and child, or its closing parenthesis. A frame
/// walk, not a recursion inside a loop: the pinned Aeneas emits that loop
/// through its `loop` combinator, whose `partial_fixpoint` the pinned Lean
/// cannot prove monotone in the recursive call.
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending a type rendering requires the caller's growable String, not a fixed str slice."
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
fn render_step<'a>(
    frame: ScanFrame<'a>,
    limits: Limits,
    bound: usize,
    text: &mut String,
    pending: &mut WorkStack<ScanFrame<'a>>,
) -> Result<(), Diagnostic> {
    match frame {
        ScanFrame::Enter(form, depth) => {
            if depth > limits.depth {
                return Err(stage_exhausted(form.span, "depth"));
            }
            match &form.kind {
                FormKind::Atom(atom) => {
                    text.push_str(atom);
                    Ok(())
                }
                FormKind::List(items) => {
                    text.push('(');
                    let next = deeper(depth, form.span)?;
                    pending
                        .push_bounded(ScanFrame::Children(items, 0, next), bound)
                        .map_err(|()| stage_exhausted(form.span, "tree frame"))
                }
            }
        }
        // `index` is the next item; past the last, the list closes.
        ScanFrame::Children(items, index, depth) => {
            let Some(item) = items.get(index) else {
                text.push(')');
                return Ok(());
            };
            if index > 0 {
                text.push(' ');
            }
            pending
                .push_bounded(
                    ScanFrame::Children(items, index.saturating_add(1), depth),
                    bound,
                )
                .map_err(|()| stage_exhausted(item.span, "tree frame"))?;
            pending
                .push_bounded(ScanFrame::Enter(item, depth), bound)
                .map_err(|()| stage_exhausted(item.span, "tree frame"))
        }
    }
}

/// Substitute the eliminated binder `binder` with `argument` in a
/// proposition: the meta-aware analogue of the checker's `replace_prop`.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; substitution follows the finite proposition tree under the caller's budget"
)]
fn inst_prop(
    prop: &PropTerm,
    binder: u32,
    argument: &ArgTerm,
    span: Span,
) -> Result<PropTerm, Diagnostic> {
    match prop {
        PropTerm::Pi(index, sort, body) => {
            let next = index
                .checked_sub(1)
                .ok_or_else(|| internal(span, "dependent Pi binder underflow"))?;
            Ok(PropTerm::Pi(
                next,
                inst_sort(sort, binder, argument, span)?,
                Box::new(inst_prop(body, binder, argument, span)?),
            ))
        }
        PropTerm::Eq(code, left, right) => Ok(PropTerm::Eq(
            inst_ty(code, binder, argument, span)?,
            inst_val(*left, binder, argument),
            inst_val(*right, binder, argument),
        )),
    }
}

/// Substitute the eliminated binder in a code.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; substitution follows the finite code tree"
)]
fn inst_ty(ty: &Ty, binder: u32, argument: &ArgTerm, span: Span) -> Result<Ty, Diagnostic> {
    match ty {
        Ty::Var(index) => {
            if *index == binder {
                match argument {
                    ArgTerm::Code(replacement) => Ok(clone_ty(replacement)),
                    ArgTerm::Value(_) | ArgTerm::Premise => Ok(Ty::Var(*index)),
                }
            } else if *index > binder {
                Ok(Ty::Var(
                    index
                        .checked_sub(1)
                        .ok_or_else(|| internal(span, "code level underflow"))?,
                ))
            } else {
                Ok(Ty::Var(*index))
            }
        }
        Ty::Pair(left, right) => Ok(Ty::Pair(
            Box::new(inst_ty(left, binder, argument, span)?),
            Box::new(inst_ty(right, binder, argument, span)?),
        )),
        Ty::Sum(left, right) => Ok(Ty::Sum(
            Box::new(inst_ty(left, binder, argument, span)?),
            Box::new(inst_ty(right, binder, argument, span)?),
        )),
        Ty::List(item) => Ok(Ty::List(Box::new(inst_ty(item, binder, argument, span)?))),
        Ty::Meta(meta) => Ok(Ty::Meta(*meta)),
        Ty::Unit => Ok(Ty::Unit),
        Ty::Bool => Ok(Ty::Bool),
        Ty::I64 => Ok(Ty::I64),
        Ty::Text => Ok(Ty::Text),
    }
}

/// Substitute the eliminated binder in a value position.
const fn inst_val(value: Val, binder: u32, argument: &ArgTerm) -> Val {
    match value {
        Val::Level(level) => {
            if level == binder {
                match argument {
                    ArgTerm::Value(replacement) => *replacement,
                    ArgTerm::Code(_) | ArgTerm::Premise => Val::Level(level),
                }
            } else if level > binder {
                Val::Level(level.saturating_sub(1))
            } else {
                Val::Level(level)
            }
        }
        Val::Meta(_) => value,
    }
}

/// Substitute the eliminated binder in a sort.
fn inst_sort(
    sort: &SortTerm,
    binder: u32,
    argument: &ArgTerm,
    span: Span,
) -> Result<SortTerm, Diagnostic> {
    match sort {
        SortTerm::Codes => Ok(SortTerm::Codes),
        SortTerm::Value(ty) => Ok(SortTerm::Value(inst_ty(ty, binder, argument, span)?)),
        SortTerm::Premise(prop) => Ok(SortTerm::Premise(Box::new(inst_prop(
            prop, binder, argument, span,
        )?))),
    }
}

#[cfg(test)]
mod tests {
    //! Solver-level controls for outcomes a well-formed surface term cannot
    //! reach without an incorrect rebasing order or a forged solution: the
    //! occurs check, the depth restriction and the value-metacode rule.

    use super::*;

    fn one_span() -> Span {
        Span { start: 0, end: 1 }
    }

    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; the fixture builder only constructs an elaborator; every test that uses it asserts on the outcome"
    )]
    fn with_elaborator<R>(body: impl FnOnce(&mut Elaborator<'_, '_>) -> R) -> R {
        let limits = crate::Limits::default();
        let config = MeterConfig {
            budgets: ProofBudgets::from_limits(limits),
            limits,
            tree_bounds: TreeBounds::new(limits).expect("default limits bound a tree"),
        };
        let mut elaborator = Elaborator {
            meter: Meter {
                nodes: 0,
                work: 0,
                normalization: 0,
                substitution: 0,
                config: &config,
            },
            available: &[],
            context: WorkStack::new(),
            context_spans: WorkStack::new(),
            metas: Vec::new(),
            deferred: Vec::new(),
            deferred_forms: Vec::new(),
        };
        body(&mut elaborator)
    }

    #[test]
    fn meta_bound_only_to_meta_is_underconstrained() {
        with_elaborator(|elaborator| {
            let first = elaborator
                .alloc_code("A", 0, "m@1/h", one_span(), 0)
                .expect("metavariable");
            let second = elaborator
                .alloc_code("A", 0, "m@1/h", one_span(), 0)
                .expect("metavariable");
            elaborator
                .unify_ty(&Ty::Meta(first), &Ty::Meta(second), one_span())
                .expect("two metas unify by binding one to the other");
            let error = elaborator
                .verify_ground()
                .expect_err("an unassigned meta chain is not ground");
            assert!(
                error.message.starts_with("underconstrained:"),
                "{}",
                error.message
            );
        });
    }

    #[test]
    fn solution_above_its_depth_is_ill_scoped() {
        with_elaborator(|elaborator| {
            let meta = elaborator
                .alloc_code("A", 0, "m@1/h", one_span(), 0)
                .expect("metavariable");
            elaborator
                .unify_ty(&Ty::Meta(meta), &Ty::Var(0), one_span())
                .expect("binding succeeds; the depth check refuses it");
            let error = elaborator
                .verify_ground()
                .expect_err("level 0 is not below depth 0");
            assert!(
                error.message.starts_with("ill-scoped:"),
                "{}",
                error.message
            );
            assert!(error.message.contains("level 0"), "{}", error.message);
        });
    }

    #[test]
    fn occurs_check_refuses_a_repeating_solution() {
        with_elaborator(|elaborator| {
            let meta = elaborator
                .alloc_code("A", 0, "m@1/h", one_span(), 0)
                .expect("metavariable");
            let repeating = Ty::List(Box::new(Ty::Meta(meta)));
            let error = elaborator
                .unify_ty(&Ty::Meta(meta), &repeating, one_span())
                .expect_err("occurs check");
            assert!(
                error.message.starts_with("invalid-constraint:"),
                "{}",
                error.message
            );
            assert!(error.message.contains("occurs check"), "{}", error.message);
        });
    }

    #[test]
    fn value_meta_requires_its_own_code() {
        with_elaborator(|elaborator| {
            elaborator
                .push_binder(
                    Binder {
                        name: String::from("x"),
                        sort: Sort::Value(Code::I64),
                    },
                    one_span(),
                )
                .expect("binder");
            let wrong = elaborator
                .alloc_value("A", 0, "m@1/h", Ty::Bool, one_span(), 1)
                .expect("value metavariable");
            let error = elaborator
                .unify_val(Val::Meta(wrong), Val::Level(0), one_span())
                .expect_err("an I64 binder is not Bool");
            assert!(
                error.message.starts_with("invalid-constraint:"),
                "{}",
                error.message
            );
        });
        with_elaborator(|elaborator| {
            elaborator
                .push_binder(
                    Binder {
                        name: String::from("x"),
                        sort: Sort::Value(Code::I64),
                    },
                    one_span(),
                )
                .expect("binder");
            let right = elaborator
                .alloc_value("A", 0, "m@1/h", Ty::I64, one_span(), 1)
                .expect("value metavariable");
            elaborator
                .unify_val(Val::Meta(right), Val::Level(0), one_span())
                .expect("an I64 binder solves an I64 value meta");
            assert!(
                matches!(
                    elaborator.metas.first().and_then(|meta| meta.value),
                    Some(Val::Level(0))
                ),
                "the value metavariable is solved by the I64 binder at level 0"
            );
            elaborator.verify_ground().expect("every meta is ground");
        });
    }

    #[test]
    fn instantiation_rebases_like_the_checker() {
        let span = one_span();
        // The body under `(Pi (A Type0) ...)`: (Pi (x A) (Eq A 2 2)).
        let body = PropTerm::Pi(
            1,
            SortTerm::Value(Ty::Var(0)),
            Box::new(PropTerm::Eq(Ty::Var(0), Val::Level(2), Val::Level(2))),
        );
        // The checker's `replace_prop` expects the body below the eliminated
        // binder, exactly as `finish_apply` calls it.
        let instantiated =
            inst_prop(&body, 0, &ArgTerm::Code(Ty::I64), span).expect("substitution");
        assert_eq!(describe_prop(&instantiated), "(Pi #0 I64 (Eq I64 #1 #1))");
    }

    /// The digit helpers render exactly as `{value}` does, at every boundary.
    #[test]
    fn decimal_texts_match_the_display_rendering() {
        for value in [0u32, 1, 9, 10, 11, 99, 100, 4_096, 65_535, 4_294_967_295] {
            assert_eq!(u32_text(value), alloc::format!("{value}"));
        }
        for value in [
            0i64,
            1,
            -1,
            9,
            -9,
            10,
            -10,
            1_000_000,
            -1_000_000,
            i64::MAX,
            i64::MIN,
            i64::MIN + 1,
        ] {
            assert_eq!(signed_text(value), alloc::format!("{value}"));
        }
    }

    /// The format-free diagnostic renderings are byte for byte the `format!`
    /// texts they replaced.
    #[test]
    fn clash_renderings_match_the_format_texts() {
        let code = Ty::Pair(
            Box::new(Ty::List(Box::new(Ty::Var(12)))),
            Box::new(Ty::Sum(Box::new(Ty::Meta(3)), Box::new(Ty::I64))),
        );
        assert_eq!(describe_ty(&code), "(Pair (List #12) (Sum ?3 I64))");
        assert_eq!(describe_val(Val::Level(7)), "#7");
        assert_eq!(describe_val(Val::Meta(70)), "?70");
        let prop = PropTerm::Pi(
            4,
            SortTerm::Premise(Box::new(PropTerm::Eq(
                Ty::Text,
                Val::Meta(1),
                Val::Level(0),
            ))),
            Box::new(PropTerm::Eq(code, Val::Level(2), Val::Meta(9))),
        );
        assert_eq!(
            describe_prop(&prop),
            "(Pi #4 (Eq Text ?1 #0) (Eq (Pair (List #12) (Sum ?3 I64)) #2 ?9))"
        );
        assert_eq!(
            identity_text("logic", 12, "poly-refl"),
            "logic@12/poly-refl"
        );
        assert_eq!(
            stage_exhausted(one_span(), "work").message,
            "exhausted: elaboration work budget"
        );
        assert_eq!(invalid(one_span(), "x").message, "invalid-constraint: x");
    }

    /// A metered exhaustion names the budget its step exceeded, and a bounded
    /// rendering counts every byte it cuts.
    #[test]
    fn exhaustion_names_the_exceeded_budget_and_caps_count_cut_bytes() {
        let limits = crate::Limits::default();
        let config = MeterConfig {
            budgets: ProofBudgets {
                normalization_work: 1,
                substitution_work: 1,
            },
            limits,
            tree_bounds: TreeBounds::new(limits).expect("default limits bound a tree"),
        };
        let meter = |nodes: u32, work: u32, normalization: u32, substitution: u32| Meter {
            nodes,
            work,
            normalization,
            substitution,
            config: &config,
        };
        let over = limits.work.saturating_add(1);
        for (state, step, expected) in [
            (meter(0, 0, 2, 0), MeterStep::Normalize(1), "normalization"),
            (meter(0, over, 1, 0), MeterStep::Normalize(1), "work"),
            (meter(0, 0, 0, 2), MeterStep::Substitute(1), "substitution"),
            (meter(0, over, 0, 1), MeterStep::Substitute(1), "work"),
            (
                meter(limits.nodes.saturating_add(1), 0, 0, 0),
                MeterStep::Visit(0),
                "node",
            ),
            (meter(1, over, 0, 0), MeterStep::Visit(0), "work"),
            (
                meter(1, 1, 0, 0),
                MeterStep::Visit(limits.depth.saturating_add(1)),
                "depth",
            ),
            (meter(0, over, 0, 0), MeterStep::Charge(1), "work"),
        ] {
            assert_eq!(exceeded_budget(&state, step), expected);
        }
        assert_eq!(
            meter_exhausted(one_span(), "unification", "normalization").message,
            "exhausted: elaboration unification step exceeded the normalization budget"
        );
        assert_eq!(capped(String::from("abcd"), 4), "abcd");
        assert_eq!(capped(String::from("abcdef"), 4), "abcd...[2 more bytes]");
        // The cut never splits a character: it stops before the first
        // non-ASCII byte and still counts every byte not shown.
        assert_eq!(
            capped(String::from("ab\u{e9}cdef"), 4),
            "ab...[6 more bytes]"
        );
        let detail = "x".repeat(20_000);
        let long = infer_diagnostic(
            DiagnosticKind::Invalid,
            one_span(),
            "invalid-constraint",
            &detail,
        );
        let cut = "invalid-constraint: "
            .len()
            .saturating_add(detail.len())
            .saturating_sub(MAX_DIAGNOSTIC_BYTES);
        let notice = alloc::format!("...[{cut} more bytes]");
        assert_eq!(
            long.message.len(),
            MAX_DIAGNOSTIC_BYTES.saturating_add(notice.len())
        );
        assert!(long.message.ends_with(&notice));
    }
}
