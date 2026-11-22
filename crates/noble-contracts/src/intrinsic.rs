//! Bounded, non-executable source proof terms. No Lean source is read from a module.
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

mod named_v2;
pub use named_v2::prepare_named_contract;
mod elaborate;
pub use elaborate::{
    elaborate_obligations, Elaborated, ElaborationRecord, InsertedArgument, ELABORATION_REVISION,
};

#[derive(Debug, Eq)]
pub struct Form {
    pub kind: FormKind,
    pub span: Span,
}
#[derive(Eq)]
pub enum FormKind {
    Atom(String),
    List(Vec<Form>),
}

// `Form` and `FormKind` are mutually recursive through `Vec<Form>`. Derived
// `Clone`, `PartialEq` and `Debug` impls call each other through their trait
// impls, and the pinned Aeneas refuses such mixed function/trait-impl groups.
// The copy and equality walks below are plain functions instead, with exactly
// the derived results; nested lists are walked through explicit work lists, so
// no loop sits on a call cycle. `FormKind`'s rendering is the one reviewed
// opaque formatting boundary (see `debug_form_kind`).
impl Clone for Form {
    fn clone(&self) -> Self {
        clone_form(self)
    }
}
impl Clone for FormKind {
    fn clone(&self) -> Self {
        clone_form_kind(self)
    }
}
impl PartialEq for Form {
    fn eq(&self, other: &Self) -> bool {
        form_equal(self, other)
    }
}
impl PartialEq for FormKind {
    fn eq(&self, other: &Self) -> bool {
        form_kind_equal(self, other)
    }
}
impl core::fmt::Debug for FormKind {
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; `Debug` renders by writing through the caller's formatter, which is the operation"
    )]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        debug_form_kind(self, f)
    }
}

/// Deep-copy one form, field by field, as the derived `Clone` did.
fn clone_form(form: &Form) -> Form {
    Form {
        kind: clone_form_kind(&form.kind),
        span: form.span,
    }
}

/// Deep-copy one form kind, variant by variant, as the derived `Clone` did.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn clone_form_kind(kind: &FormKind) -> FormKind {
    match kind {
        FormKind::Atom(atom) => FormKind::Atom(atom.clone()),
        FormKind::List(forms) => FormKind::List(clone_forms(forms)),
    }
}

/// Copy a list of forms in order into exactly sized vectors, as `Vec::clone`
/// did. Nested lists are copied through an explicit work list rather than by
/// recursion; the list copies under construction stay open, innermost last.
fn clone_forms(forms: &[Form]) -> Vec<Form> {
    let mut copy = Vec::with_capacity(forms.len());
    let mut open_lists = Vec::new();
    let mut pending = Vec::new();
    pending.push(CopyFrame::Forms(forms));
    while let Some(frame) = pending.pop() {
        copy_step(frame, &mut pending, &mut open_lists, &mut copy);
    }
    copy
}

/// One pending piece of a list copy.
enum CopyFrame<'a> {
    /// Copy these forms, in order, into the innermost open list copy.
    Forms(&'a [Form]),
    /// Close the innermost open list copy into a list form with this span.
    Close(Span),
}

/// Copy the first pending form and schedule the rest, or close the innermost
/// open list copy. A list form opens a list copy whose forms are copied before
/// the forms that follow it.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the work list and the copies are the copy loop's locally owned state, borrowed only for one loop-free step"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the step grows the copy loop's owned vectors, which must stay the caller's Vecs"
)]
fn copy_step<'a>(
    frame: CopyFrame<'a>,
    pending: &mut Vec<CopyFrame<'a>>,
    open_lists: &mut Vec<Vec<Form>>,
    copy: &mut Vec<Form>,
) {
    match frame {
        CopyFrame::Forms(forms) => {
            if forms.is_empty() {
                return;
            }
            let form = &forms[0];
            pending.push(CopyFrame::Forms(&forms[1..]));
            match &form.kind {
                FormKind::Atom(atom) => push_copied(
                    open_lists,
                    copy,
                    Form {
                        kind: FormKind::Atom(atom.clone()),
                        span: form.span,
                    },
                ),
                FormKind::List(children) => {
                    open_lists.push(Vec::with_capacity(children.len()));
                    pending.push(CopyFrame::Close(form.span));
                    pending.push(CopyFrame::Forms(children));
                }
            }
        }
        CopyFrame::Close(span) => {
            if let Some(items) = open_lists.pop() {
                push_copied(
                    open_lists,
                    copy,
                    Form {
                        kind: FormKind::List(items),
                        span,
                    },
                );
            }
        }
    }
}

/// Append `item` to the innermost open list copy, else to the root `copy`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the copies are the copy loop's locally owned state, borrowed only to append one form"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; appending grows the copy loop's owned root copy, which must stay the caller's Vec"
)]
fn push_copied<T>(open_lists: &mut [Vec<T>], copy: &mut Vec<T>, item: T) {
    match open_lists.len().checked_sub(1) {
        Some(top) => open_lists[top].push(item),
        None => copy.push(item),
    }
}

/// Field-wise form equality in declaration order, as the derived `PartialEq` did.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn form_equal(left: &Form, right: &Form) -> bool {
    form_kind_equal(&left.kind, &right.kind) && left.span == right.span
}

/// Variant-wise form kind equality, as the derived `PartialEq` did.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn form_kind_equal(left: &FormKind, right: &FormKind) -> bool {
    match (left, right) {
        (FormKind::Atom(left), FormKind::Atom(right)) => left == right,
        (FormKind::List(left), FormKind::List(right)) => forms_equal(left, right),
        (FormKind::Atom(_), FormKind::List(_)) | (FormKind::List(_), FormKind::Atom(_)) => false,
    }
}

/// Slice equality: equal lengths and pairwise equal forms, compared through an
/// explicit work list of list pairs rather than by recursion. Equality has no
/// effects, so the comparison order cannot change the result.
fn forms_equal(left: &[Form], right: &[Form]) -> bool {
    let mut pending = Vec::new();
    pending.push(ListPair { left, right });
    let mut is_equal = true;
    while let Some(lists) = pending.pop() {
        is_equal = lists_step(lists, &mut pending);
        if !is_equal {
            break;
        }
    }
    is_equal
}

/// A pending pair of form lists to compare.
struct ListPair<'a> {
    left: &'a [Form],
    right: &'a [Form],
}

/// Compare the first forms of a pending list pair without descending: equal
/// lengths, spans and atoms. A pair of nested lists and the remaining forms
/// are queued.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the pending list pairs are the comparison loop's locally owned state, borrowed only for one loop-free step"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the step grows the comparison loop's owned work list, which must stay the caller's Vec"
)]
fn lists_step<'a>(lists: ListPair<'a>, pending: &mut Vec<ListPair<'a>>) -> bool {
    let ListPair { left, right } = lists;
    if left.len() != right.len() {
        return false;
    }
    if left.is_empty() {
        return true;
    }
    pending.push(ListPair {
        left: &left[1..],
        right: &right[1..],
    });
    let first = &left[0];
    let other = &right[0];
    if first.span != other.span {
        return false;
    }
    match (&first.kind, &other.kind) {
        (FormKind::Atom(first), FormKind::Atom(other)) => first == other,
        (FormKind::List(first), FormKind::List(other)) => {
            pending.push(ListPair {
                left: first,
                right: other,
            });
            true
        }
        (FormKind::Atom(_), FormKind::List(_)) | (FormKind::List(_), FormKind::Atom(_)) => false,
    }
}

/// Render a form kind exactly as `#[derive(Debug)]` did.
///
/// Reviewed opaque extraction boundary: any rendering of `List` must format the
/// nested forms through `Form`'s `Debug` impl, which in turn formats this kind,
/// so no transparent body can avoid the mixed trait-impl cycle the pinned Aeneas
/// refuses. Formatting layout is already an abstracted trust boundary.
#[charon::opaque]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper writes through the caller's formatter, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; rendering writes through the caller's formatter, which is the operation"
)]
fn debug_form_kind(kind: &FormKind, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match kind {
        FormKind::Atom(atom) => f.debug_tuple("Atom").field(atom).finish(),
        FormKind::List(forms) => f.debug_tuple("List").field(forms).finish(),
    }
}
#[derive(Clone, Debug)]
pub struct ResolvedSourceModule {
    pub module: String,
    pub version: u32,
    pub full_source: Vec<u8>,
    /// The retained namespace owner; never a portable module identity.
    pub owner: u64,
}
#[derive(Clone, Debug)]
pub struct NamedImportBinding {
    /// The import alias as resolved when the original caller was installed.
    pub alias: String,
    pub owner: u64,
    pub module_index: usize,
}
#[derive(Clone, Debug)]
pub struct NamedUseOrigin {
    pub definition: noble_kernel::contracts::Definition,
    pub definition_identity: u64,
    pub definition_owner: u64,
    pub definition_name: String,
    /// Lexical definition order in the original owner module, not a kernel slot.
    pub definition_ordinal: u32,
    pub definition_span: Span,
    pub body_span: Span,
    pub module_index: usize,
    /// None only for the enclosing, single-invocation subject root.
    pub caller_definition: Option<noble_kernel::contracts::Definition>,
    pub source_node: Option<u32>,
    pub source_span: Option<Span>,
    pub candidate_node: noble_kernel::untrusted::NodeId,
}
#[derive(Clone, Debug)]
pub struct SubjectBinding {
    pub module: String,
    pub version: u32,
    pub module_source: Vec<u8>,
    pub definition: String,
    pub definition_source: Vec<u8>,
    pub source_dependencies: Vec<ResolvedSourceModule>,
    /// Alias snapshot at source installation, not the current mutable alias map.
    pub named_imports: Vec<NamedImportBinding>,
    /// Source-resolved, per-use provenance; empty for historical contract 1.
    pub named_uses: Vec<NamedUseOrigin>,
    /// Session-local only; this number is never used as a portable artifact hash.
    pub definition_identity: u64,
    /// Declaration order within the original module (not a specialization slot).
    pub definition_ordinal: u32,
    /// Session-local owner recorded on the resolved definition.
    pub definition_owner: u64,
    pub accepted_submission: noble_kernel::execution::Submission,
    pub input_types: Vec<noble_kernel::types::Ty>,
    pub output_types: Vec<noble_kernel::types::Ty>,
    pub effects: noble_kernel::types::EffSet,
    pub source_span: Span,
}
#[derive(Clone, Debug)]
pub struct ContractGoal {
    pub contract_name: String,
    pub revision: u32,
    pub contract_span: Span,
    pub subject: SubjectBinding,
    pub inputs: Vec<(String, String)>,
    pub outputs: Vec<(String, String)>,
    pub requires: Form,
    pub ensures: Form,
}
#[derive(Clone, Debug)]
pub enum PendingGoal {
    Pure { proposition: Form },
    Contract { contract: Box<ContractGoal> },
}
#[derive(Clone, Debug)]
pub struct ProofObligation {
    pub name: String,
    pub revision: u32,
    pub goal: PendingGoal,
    pub term: Form,
    pub span: Span,
    /// The declaration opted into elaborator revision `implicit-args/v1`.
    pub elaboration: bool,
}
pub struct ProofDependency {
    pub name: String,
    pub module: String,
    pub version: u32,
    pub source: Vec<u8>,
    pub declaration: ProofObligation,
    pub exported: bool,
    pub dependencies: Vec<ProofDependency>,
    /// Exact host-reviewed model library identity at original acceptance.
    pub model_revision: String,
    /// Exact checker/lowering/consumer identity at original acceptance.
    pub checker_revision: String,
}
// `ProofDependency` is recursive through `dependencies`. Derived `Clone` and
// `Debug` impls would reach their own trait impls through
// `Vec<ProofDependency>`, which the extracted model cannot define before use.
// The copy below walks the dependency tree through plain functions and an
// explicit work list instead, with exactly the derived result; the rendering
// is a reviewed opaque formatting boundary (see `debug_proof_dependency`).
impl Clone for ProofDependency {
    fn clone(&self) -> Self {
        clone_dependency(self)
    }
}
impl core::fmt::Debug for ProofDependency {
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; `Debug` renders by writing through the caller's formatter, which is the operation"
    )]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        debug_proof_dependency(self, f)
    }
}

/// Deep-copy one dependency, field by field, as the derived `Clone` did.
fn clone_dependency(dependency: &ProofDependency) -> ProofDependency {
    copied_dependency(dependency, clone_dependencies(&dependency.dependencies))
}

/// A copy of `dependency`'s own fields around its already copied
/// `dependencies`.
fn copied_dependency(
    dependency: &ProofDependency,
    dependencies: Vec<ProofDependency>,
) -> ProofDependency {
    ProofDependency {
        name: dependency.name.clone(),
        module: dependency.module.clone(),
        version: dependency.version,
        source: dependency.source.clone(),
        declaration: dependency.declaration.clone(),
        exported: dependency.exported,
        dependencies,
        model_revision: dependency.model_revision.clone(),
        checker_revision: dependency.checker_revision.clone(),
    }
}

/// Copy a list of dependencies in order into exactly sized vectors, as
/// `Vec::clone` did, through an explicit work list rather than by recursion.
fn clone_dependencies(dependencies: &[ProofDependency]) -> Vec<ProofDependency> {
    let mut copy = Vec::with_capacity(dependencies.len());
    let mut open_lists = Vec::new();
    let mut pending = Vec::new();
    pending.push(DependencyCopy::Nodes(dependencies));
    while let Some(frame) = pending.pop() {
        dependency_copy_step(frame, &mut pending, &mut open_lists, &mut copy);
    }
    copy
}

/// One pending piece of a dependency-list copy.
enum DependencyCopy<'a> {
    /// Copy these dependencies, in order, into the innermost open list copy.
    Nodes(&'a [ProofDependency]),
    /// Close the innermost open list copy into a copy of this dependency.
    Close(&'a ProofDependency),
}

/// Open a list copy for the first pending dependency's own dependencies,
/// copied before the dependencies that follow it, or close the innermost open
/// list copy into a copy of its dependency.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the work list and the copies are the copy loop's locally owned state, borrowed only for one loop-free step"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the step grows the copy loop's owned vectors, which must stay the caller's Vecs"
)]
fn dependency_copy_step<'a>(
    frame: DependencyCopy<'a>,
    pending: &mut Vec<DependencyCopy<'a>>,
    open_lists: &mut Vec<Vec<ProofDependency>>,
    copy: &mut Vec<ProofDependency>,
) {
    match frame {
        DependencyCopy::Nodes(dependencies) => {
            if dependencies.is_empty() {
                return;
            }
            let dependency = &dependencies[0];
            pending.push(DependencyCopy::Nodes(&dependencies[1..]));
            open_lists.push(Vec::with_capacity(dependency.dependencies.len()));
            pending.push(DependencyCopy::Close(dependency));
            pending.push(DependencyCopy::Nodes(&dependency.dependencies));
        }
        DependencyCopy::Close(dependency) => {
            if let Some(dependencies) = open_lists.pop() {
                push_copied(
                    open_lists,
                    copy,
                    copied_dependency(dependency, dependencies),
                );
            }
        }
    }
}

/// Render a proof dependency exactly as `#[derive(Debug)]` did.
///
/// Reviewed Debug-only opaque extraction boundary: a transparent body must
/// format `Vec<ProofDependency>` through `dyn Debug`, which needs the very
/// instance this body defines. Formatting layout is already an abstracted
/// trust boundary.
#[charon::opaque]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; rendering writes through the caller's formatter, which is the operation"
)]
fn debug_proof_dependency(
    dependency: &ProofDependency,
    f: &mut core::fmt::Formatter<'_>,
) -> core::fmt::Result {
    f.debug_struct("ProofDependency")
        .field("name", &dependency.name)
        .field("module", &dependency.module)
        .field("version", &dependency.version)
        .field("source", &dependency.source)
        .field("declaration", &dependency.declaration)
        .field("exported", &dependency.exported)
        .field("dependencies", &dependency.dependencies)
        .field("model_revision", &dependency.model_revision)
        .field("checker_revision", &dependency.checker_revision)
        .finish()
}
#[derive(Clone, Debug)]
pub struct ProofBatch {
    pub module: String,
    pub version: u32,
    pub source: Vec<u8>,
    pub generation: u64,
    pub dependencies: Vec<ProofDependency>,
    pub obligations: Vec<ProofObligation>,
    /// Elaboration records for abbreviated proofs, in obligation order.
    pub elaborations: Vec<elaborate::ElaborationRecord>,
    /// Work charged to the shared elaboration meter.
    pub elaboration_work: ProofWork,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofKind {
    Pure,
    Contract,
    NamedContract,
}
#[derive(Clone, Debug)]
pub struct CheckedProof {
    pub name: String,
    pub kind: ProofKind,
    pub lean_term: String,
    pub claim: String,
    pub model_revision: String,
    pub checker_revision: String,
}

#[derive(Clone, Copy, Debug)]
pub struct ProofBudgets {
    pub normalization_work: u32,
    pub substitution_work: u32,
}
impl ProofBudgets {
    pub const fn from_limits(limits: Limits) -> Self {
        Self {
            normalization_work: limits.work,
            substitution_work: limits.work,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ProofWork {
    pub normalization: u32,
    pub substitution: u32,
    pub total: u32,
}
fn reject(span: Span, reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::Invalid, span, reason)
}
fn exhausted(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::Exhausted,
        span,
        "proof size, depth or work limit exhausted",
    )
}
fn deeper(depth: u32, span: Span) -> Result<u32, Diagnostic> {
    depth.checked_add(1).ok_or_else(|| exhausted(span))
}
#[derive(Clone, Copy)]
struct TreeBounds {
    frames: usize,
    bytes: usize,
}
impl TreeBounds {
    fn new(limits: Limits) -> Option<Self> {
        // Source Code nodes are visited; Eq contributes two unvisited index units,
        // and a Pi/Codes sort contributes one. Four units per visited node plus
        // four covers every freshly parsed proposition. Before a replacement,
        // Meter::substitute charges its entire body (at most limits.work); the
        // only expanding argument is a freshly parsed Code (at most limits.nodes).
        // Thus even repeated replacements cannot grow a surviving tree beyond
        // this bound: each replacement charges its input anew.
        let source_nodes = usize::try_from(limits.nodes).ok()?;
        let work = usize::try_from(limits.work).ok()?;
        let source = source_nodes.saturating_mul(4).saturating_add(4);
        let substituted = work.saturating_mul(source_nodes.max(1));
        let nodes = source.max(substituted);
        // Three pending frames per structural unit (plus the initial root)
        // cover deferred children and Lean punctuation, even for nested Pi.
        // Use saturating caps so 32-bit targets cannot reject a realizable tree
        // merely because user-supplied limits exceed addressable memory.
        // The longest leaf is a decimal usize (20 bytes on 64-bit); all
        // constructor punctuation is shorter than 128 bytes per size unit.
        Some(Self {
            frames: nodes.saturating_mul(3).saturating_add(2),
            bytes: nodes.saturating_mul(128),
        })
    }
}
#[derive(Clone, Copy)]
// Extraction-only name: the crate-root `Meter` is printed unqualified in Lean,
// where it would resolve to this type inside `intrinsic`.
#[charon::rename("IntrinsicMeter")]
struct Meter<'a> {
    nodes: u32,
    work: u32,
    normalization: u32,
    substitution: u32,
    config: &'a MeterConfig,
}
struct MeterConfig {
    budgets: ProofBudgets,
    limits: Limits,
    tree_bounds: TreeBounds,
}
impl core::ops::Deref for Meter<'_> {
    type Target = MeterConfig;
    fn deref(&self) -> &MeterConfig {
        self.config
    }
}
// A failed visit may already have incremented work. Intro continuations still
// charge their wrappers after that failure and can override its diagnostic, so
// every transition returns its owned meter on both paths.
type Transition<'a, T> = (Meter<'a>, Result<T, Diagnostic>);
macro_rules! advance {
    ($meter:expr,$transition:expr) => {{
        let (next, result) = $transition;
        $meter = next;
        result?
    }};
}
impl<'a> Meter<'a> {
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
    )]
    fn visit(mut self, depth: u32, span: Span) -> Transition<'a, ()> {
        let Some(nodes) = self.nodes.checked_add(1) else {
            return (self, Err(exhausted(span)));
        };
        self.nodes = nodes;
        let Some(work) = self.work.checked_add(1) else {
            return (self, Err(exhausted(span)));
        };
        self.work = work;
        if self.nodes > self.limits.nodes
            || self.work > self.limits.work
            || depth > self.limits.depth
        {
            return (self, Err(exhausted(span)));
        }
        (self, Ok(()))
    }
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
    )]
    fn charge(mut self, amount: usize, span: Span) -> Transition<'a, ()> {
        let Ok(units) = u32::try_from(amount) else {
            return (self, Err(exhausted(span)));
        };
        let Some(work) = self.work.checked_add(units) else {
            return (self, Err(exhausted(span)));
        };
        self.work = work;
        if self.work > self.limits.work {
            return (self, Err(exhausted(span)));
        }
        (self, Ok(()))
    }
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
    )]
    fn normalize(mut self, amount: usize, span: Span) -> Transition<'a, ()> {
        let Ok(units) = u32::try_from(amount) else {
            return (self, Err(exhausted(span)));
        };
        let Some(normalization) = self.normalization.checked_add(units) else {
            return (self, Err(exhausted(span)));
        };
        self.normalization = normalization;
        if self.normalization > self.budgets.normalization_work {
            return (
                self,
                Err(Diagnostic::new(
                    DiagnosticKind::Exhausted,
                    span,
                    "normalization work exhausted",
                )),
            );
        }
        self.charge(amount, span)
    }
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
    )]
    fn substitute(mut self, amount: usize, span: Span) -> Transition<'a, ()> {
        let Ok(units) = u32::try_from(amount) else {
            return (self, Err(exhausted(span)));
        };
        let Some(substitution) = self.substitution.checked_add(units) else {
            return (self, Err(exhausted(span)));
        };
        self.substitution = substitution;
        if self.substitution > self.budgets.substitution_work {
            return (
                self,
                Err(Diagnostic::new(
                    DiagnosticKind::Exhausted,
                    span,
                    "substitution work exhausted",
                )),
            );
        }
        self.charge(amount, span)
    }
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn atom(form: &Form) -> Result<&str, Diagnostic> {
    match &form.kind {
        FormKind::Atom(value) => Ok(value),
        _ => Err(reject(form.span, "expected logical atom")),
    }
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn list(form: &Form) -> Result<&[Form], Diagnostic> {
    match &form.kind {
        FormKind::List(items) => Ok(items),
        _ => Err(reject(form.span, "expected logical form")),
    }
}
fn operation<'a>(form: &'a Form, head: &str, count: usize) -> Result<&'a [Form], Diagnostic> {
    let items = list(form)?;
    if items.len().checked_sub(1) != Some(count) || head_atom(items) != Some(head) {
        return Err(reject(form.span, "unknown or ill-formed proof constructor"));
    }
    Ok(&items[1..])
}
// `Debug` is only derived for tests: no library code renders this private
// type, and a derived rendering would reach its own impl through `Box<Code>`.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(test, derive(Debug))]
// Extraction-only variant names: inside the derived impls' Lean namespace the
// constructors `Unit`, `Bool` and `List` would shadow the Lean types they name.
#[charon::variants_suffix("Code")]
enum Code {
    Unit,
    Bool,
    I64,
    Text,
    Variable(usize),
    Pair(Box<Code>, Box<Code>),
    Sum(Box<Code>, Box<Code>),
    List(Box<Code>),
}
// `Sort` and `Proposition` are mutually recursive (`Sort::Premise` and
// `Proposition::Pi`). Their copy and equality recurse through plain functions,
// with exactly the derived results, so the pinned Aeneas never sees a trait
// impl inside the recursion; `Debug` is only derived for tests because no
// library code renders these private types.
#[derive(Eq)]
#[cfg_attr(test, derive(Debug))]
enum Sort {
    Codes,
    Value(Code),
    Premise(Box<Proposition>),
}
#[derive(Eq)]
#[cfg_attr(test, derive(Debug))]
enum Proposition {
    Pi(usize, Sort, Box<Proposition>),
    Eq(Code, usize, usize),
}
impl Clone for Sort {
    fn clone(&self) -> Self {
        clone_sort(self)
    }
}
impl Clone for Proposition {
    fn clone(&self) -> Self {
        clone_proposition(self)
    }
}
impl PartialEq for Sort {
    fn eq(&self, other: &Self) -> bool {
        sort_equal(self, other)
    }
}
impl PartialEq for Proposition {
    fn eq(&self, other: &Self) -> bool {
        proposition_equal(self, other)
    }
}
/// Deep-copy one sort, variant by variant, as the derived `Clone` did.
fn clone_sort(sort: &Sort) -> Sort {
    match sort {
        Sort::Codes => Sort::Codes,
        Sort::Value(code) => Sort::Value(code.clone()),
        Sort::Premise(premise) => Sort::Premise(Box::new(clone_proposition(premise))),
    }
}
/// Deep-copy one proposition, field by field in order, as the derived `Clone` did.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; the copy follows the finite, already constructed proposition tree, and plain-function recursion keeps the Sort/Proposition cycle out of their trait impls for the pinned extraction"
)]
fn clone_proposition(prop: &Proposition) -> Proposition {
    match prop {
        Proposition::Pi(index, sort, body) => {
            Proposition::Pi(*index, clone_sort(sort), Box::new(clone_proposition(body)))
        }
        Proposition::Eq(code, left, right) => Proposition::Eq(code.clone(), *left, *right),
    }
}
/// Variant-wise sort equality, as the derived `PartialEq` did.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn sort_equal(left: &Sort, right: &Sort) -> bool {
    match (left, right) {
        (Sort::Codes, Sort::Codes) => true,
        (Sort::Value(left), Sort::Value(right)) => left == right,
        (Sort::Premise(left), Sort::Premise(right)) => proposition_equal(left, right),
        (Sort::Codes, Sort::Value(_) | Sort::Premise(_))
        | (Sort::Value(_), Sort::Codes | Sort::Premise(_))
        | (Sort::Premise(_), Sort::Codes | Sort::Value(_)) => false,
    }
}
/// Field-wise proposition equality in declaration order, as the derived `PartialEq` did.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; the comparison follows two finite, already constructed proposition trees, and plain-function recursion keeps the Sort/Proposition cycle out of their trait impls for the pinned extraction"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn proposition_equal(left: &Proposition, right: &Proposition) -> bool {
    match (left, right) {
        (
            Proposition::Pi(left_index, left_sort, left_body),
            Proposition::Pi(right_index, right_sort, right_body),
        ) => {
            left_index == right_index
                && sort_equal(left_sort, right_sort)
                && proposition_equal(left_body, right_body)
        }
        (
            Proposition::Eq(left_code, left_from, left_to),
            Proposition::Eq(right_code, right_from, right_to),
        ) => left_code == right_code && left_from == right_from && left_to == right_to,
        (Proposition::Pi(..), Proposition::Eq(..)) | (Proposition::Eq(..), Proposition::Pi(..)) => {
            false
        }
    }
}
const INLINE_TREE_FRAMES: usize = 32;
struct WorkStack<T> {
    inline: [Option<T>; INLINE_TREE_FRAMES],
    inline_len: usize,
    spill: Vec<T>,
    total: usize,
}
impl<T> WorkStack<T> {
    fn new() -> Self {
        Self {
            inline: core::array::from_fn(|_| None),
            inline_len: 0,
            spill: Vec::new(),
            total: 0,
        }
    }

    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; the traversal stack is locally owned state and its mutation is the operation"
    )]
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; the operation pushes into owned storage, which is not const"
    )]
    #[expect(
        tigerstyle::numeric_units,
        reason = "Owner: noble-contracts; the bound is the frame count supplied by the tree budget"
    )]
    /// Push one frame, spilling once the inline slots are exhausted.
    fn push_bounded(&mut self, value: T, limit: usize) -> Result<(), ()> {
        let Some(total) = self.total.checked_add(1).filter(|next| *next <= limit) else {
            return Err(());
        };
        if self.inline_len < INLINE_TREE_FRAMES && self.spill.is_empty() {
            self.inline[self.inline_len] = Some(value);
            self.inline_len = self.inline_len.saturating_add(1);
            self.total = total;
            return Ok(());
        }
        if self.spill.len() >= limit {
            return Err(());
        }
        // Spill only when a real walk exhausts its inline slots.
        self.spill.push(value);
        self.total = total;
        Ok(())
    }

    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; the traversal stack is locally owned state and its mutation is the operation"
    )]
    /// Pop one frame from the spill or the inline slots.
    fn pop_bounded(&mut self) -> Option<T> {
        if let Some(value) = self.spill.pop() {
            self.total = self.total.saturating_sub(1);
            return Some(value);
        }
        let next = self.inline_len.checked_sub(1)?;
        self.inline_len = next;
        let value = self.inline[next].take();
        self.total = self.total.saturating_sub(usize::from(value.is_some()));
        value
    }
}
// These operations mutate only a locally owned traversal stack. Keeping the
// mechanics at the callsite avoids passing a mutable proof-checking capability.
macro_rules! work_push {
    ($stack:ident,$value:expr,$bound:expr) => {
        $stack.push_bounded($value, $bound)
    };
}
macro_rules! work_pop {
    ($stack:ident) => {
        $stack.pop_bounded()
    };
}
enum ValidationFrame<'a> {
    Enter(&'a Form, u32),
    Children(&'a [Form], usize, u32),
}
fn validated<'m>(form: &Form, mut meter: Meter<'m>, depth: u32) -> Transition<'m, ()> {
    let result = validate_tree(form, &mut meter, depth);
    (meter, result)
}
/// Validate every node of `form` in walk order, stopping at the first refusal.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the meter is the caller's owned transition state, written back through this borrow on every path"
)]
fn validate_tree(form: &Form, meter: &mut Meter<'_>, depth: u32) -> Result<(), Diagnostic> {
    let mut pending = WorkStack::new();
    let bound = meter.tree_bounds.frames;
    work_push!(pending, ValidationFrame::Enter(form, depth), bound)
        .map_err(|()| exhausted(form.span))?;
    let mut outcome = Ok(());
    while let Some(frame) = work_pop!(pending) {
        outcome = validation_step(frame, &mut pending, bound, meter);
        if outcome.is_err() {
            break;
        }
    }
    outcome
}
/// Visit one validation frame: check an atom or schedule a list's next child.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the traversal stack and meter are the walk's locally owned state, borrowed only for one loop-free step"
)]
fn validation_step<'a>(
    frame: ValidationFrame<'a>,
    pending: &mut WorkStack<ValidationFrame<'a>>,
    bound: usize,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    match frame {
        ValidationFrame::Enter(node, depth) => {
            advance!(*meter, meter.visit(depth, node.span));
            match &node.kind {
                FormKind::Atom(text) => {
                    advance!(*meter, meter.charge(text.len(), node.span));
                    if text.is_empty() || !bytes_within(text, b"-_.@") {
                        return Err(reject(node.span, "invalid logical identifier"));
                    }
                }
                FormKind::List(items) => {
                    if !items.is_empty() {
                        work_push!(pending, ValidationFrame::Children(items, 0, depth), bound)
                            .map_err(|()| exhausted(node.span))?
                    }
                }
            }
        }
        ValidationFrame::Children(items, index, depth) => {
            let Some(item) = items.get(index) else {
                return Ok(());
            };
            let child_depth = deeper(depth, item.span)?;
            let next = index.checked_add(1).ok_or_else(|| exhausted(item.span))?;
            if next < items.len() {
                work_push!(
                    pending,
                    ValidationFrame::Children(items, next, depth),
                    bound
                )
                .map_err(|()| exhausted(item.span))?;
            }
            work_push!(pending, ValidationFrame::Enter(item, child_depth), bound)
                .map_err(|()| exhausted(item.span))?;
        }
    }
    Ok(())
}
/// Whether every byte of `text` is ASCII alphanumeric or one of `extra`, as
/// `text.bytes().all(..)` with that predicate: in order, up to the first miss.
fn bytes_within(text: &str, extra: &[u8]) -> bool {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut is_within = true;
    while is_within && index < bytes.len() {
        let byte = bytes[index];
        is_within = byte.is_ascii_alphanumeric() || extra.contains(&byte);
        index += 1;
    }
    is_within
}
enum SizeNode<'a> {
    Code(&'a Code),
    Sort(&'a Sort),
    Proposition(&'a Proposition),
}
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn tree_size(root: SizeNode<'_>, bound: usize) -> Option<usize> {
    let mut pending = WorkStack::new();
    work_push!(pending, root, bound).ok()?;
    #[expect(
        tigerstyle::numeric_units,
        reason = "Owner: noble-contracts; the constant is a count of source elements, and the name states its unit at every use site through the surrounding type and call"
    )]
    let mut size = 0usize;
    let mut is_complete = true;
    while let Some(node) = work_pop!(pending) {
        match size_step(node, &mut pending, &mut size, bound) {
            Some(()) => {}
            None => {
                is_complete = false;
                break;
            }
        }
    }
    if is_complete {
        Some(size)
    } else {
        None
    }
}
/// Count one popped node into `size` and schedule its children, in walk order.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the traversal stack and running count are the walk's locally owned state, borrowed only for one loop-free step"
)]
fn size_step<'a>(
    node: SizeNode<'a>,
    pending: &mut WorkStack<SizeNode<'a>>,
    size: &mut usize,
    bound: usize,
) -> Option<()> {
    match node {
        SizeNode::Code(Code::Pair(left, right) | Code::Sum(left, right)) => {
            *size = size.checked_add(1)?;
            work_push!(pending, SizeNode::Code(right), bound).ok()?;
            work_push!(pending, SizeNode::Code(left), bound).ok()?;
        }
        SizeNode::Code(Code::List(item)) => {
            *size = size.checked_add(1)?;
            work_push!(pending, SizeNode::Code(item), bound).ok()?;
        }
        SizeNode::Code(_) | SizeNode::Sort(Sort::Codes) => *size = size.checked_add(1)?,
        SizeNode::Sort(Sort::Value(code)) => {
            work_push!(pending, SizeNode::Code(code), bound).ok()?;
        }
        SizeNode::Sort(Sort::Premise(prop)) => {
            work_push!(pending, SizeNode::Proposition(prop), bound).ok()?;
        }
        SizeNode::Proposition(Proposition::Pi(_, sort, body)) => {
            *size = size.checked_add(1)?;
            work_push!(pending, SizeNode::Proposition(body), bound).ok()?;
            work_push!(pending, SizeNode::Sort(sort), bound).ok()?;
        }
        SizeNode::Proposition(Proposition::Eq(code, _, _)) => {
            *size = size.checked_add(3)?;
            work_push!(pending, SizeNode::Code(code), bound).ok()?;
        }
    }
    Some(())
}
fn code_size(code: &Code, bound: usize) -> Option<usize> {
    if !matches!(code, Code::Pair(..) | Code::Sum(..) | Code::List(..)) {
        return Some(1);
    }
    tree_size(SizeNode::Code(code), bound)
}
fn proposition_size(prop: &Proposition, bound: usize) -> Option<usize> {
    tree_size(SizeNode::Proposition(prop), bound)
}
#[derive(Clone)]
struct Binder {
    name: String,
    sort: Sort,
}
#[derive(Clone)]
struct Available {
    name: String,
    goal: Proposition,
    lean_term: String,
}
#[derive(Clone, Copy)]
enum Rewrite<'a> {
    Shift(usize),
    Replace(usize, &'a Argument),
}
enum RewriteFrame<'a> {
    Code(&'a Code, bool),
    Sort(&'a Sort),
    Proposition(&'a Proposition),
    Pair,
    Sum,
    List,
    Value,
    Premise,
    Pi(usize),
    Eq(usize, usize),
}
enum Rewritten {
    Code(Code),
    Sort(Sort),
    Proposition(Proposition),
}
macro_rules! push_rewrite_frame {
    ($stack:ident,$value:expr,$bound:expr,$span:expr) => {
        work_push!($stack, $value, $bound).map_err(|()| exhausted($span))
    };
}
macro_rules! push_rewritten {
    ($stack:ident,$value:expr,$bound:expr,$span:expr) => {
        work_push!($stack, $value, $bound).map_err(|()| exhausted($span))
    };
}
macro_rules! take_code {
    ($stack:ident,$span:expr) => {
        match work_pop!($stack) {
            Some(Rewritten::Code(code)) => Ok(code),
            Some(Rewritten::Sort(_) | Rewritten::Proposition(_)) | None => Err(exhausted($span)),
        }
    };
}
macro_rules! take_sort {
    ($stack:ident,$span:expr) => {
        match work_pop!($stack) {
            Some(Rewritten::Sort(sort)) => Ok(sort),
            Some(Rewritten::Code(_) | Rewritten::Proposition(_)) | None => Err(exhausted($span)),
        }
    };
}
macro_rules! take_prop {
    ($stack:ident,$span:expr) => {
        match work_pop!($stack) {
            Some(Rewritten::Proposition(prop)) => Ok(prop),
            Some(Rewritten::Code(_) | Rewritten::Sort(_)) | None => Err(exhausted($span)),
        }
    };
}
fn rewrite_prop(
    prop: &Proposition,
    mode: Rewrite<'_>,
    span: Span,
    bound: usize,
) -> Result<Proposition, Diagnostic> {
    let mut frames = WorkStack::new();
    let mut values = WorkStack::new();
    push_rewrite_frame!(frames, RewriteFrame::Proposition(prop), bound, span)?;
    let scope = RewriteScope { mode, span, bound };
    let mut outcome = Ok(());
    while let Some(frame) = work_pop!(frames) {
        outcome = rewrite_step(frame, scope, &mut frames, &mut values);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    take_prop!(values, span)
}
/// The fixed inputs of one `rewrite_prop` walk.
#[derive(Clone, Copy)]
struct RewriteScope<'a> {
    mode: Rewrite<'a>,
    span: Span,
    bound: usize,
}
/// Rewrite one frame: schedule a node's parts or rebuild it from its parts.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the frame and value stacks are the walk's locally owned state, borrowed only for one loop-free step"
)]
fn rewrite_step<'a>(
    frame: RewriteFrame<'a>,
    scope: RewriteScope<'a>,
    frames: &mut WorkStack<RewriteFrame<'a>>,
    values: &mut WorkStack<Rewritten>,
) -> Result<(), Diagnostic> {
    let RewriteScope { mode, span, bound } = scope;
    match frame {
        RewriteFrame::Code(code, copy) => match code {
            Code::Pair(left, right) | Code::Sum(left, right) => {
                push_rewrite_frame!(
                    frames,
                    if matches!(code, Code::Pair(..)) {
                        RewriteFrame::Pair
                    } else {
                        RewriteFrame::Sum
                    },
                    bound,
                    span
                )?;
                push_rewrite_frame!(frames, RewriteFrame::Code(right, copy), bound, span)?;
                push_rewrite_frame!(frames, RewriteFrame::Code(left, copy), bound, span)?;
            }
            Code::List(item) => {
                push_rewrite_frame!(frames, RewriteFrame::List, bound, span)?;
                push_rewrite_frame!(frames, RewriteFrame::Code(item, copy), bound, span)?;
            }
            Code::Variable(index) => {
                let rewritten = if copy {
                    *index
                } else {
                    match variable_rewrite(*index, mode, span)? {
                        VariableRewrite::Index(rewritten) => rewritten,
                        VariableRewrite::Argument(argument) => {
                            // Copy the argument without applying the same substitution again.
                            push_rewrite_frame!(
                                frames,
                                RewriteFrame::Code(argument, true),
                                bound,
                                span
                            )?;
                            return Ok(());
                        }
                    }
                };
                push_rewritten!(
                    values,
                    Rewritten::Code(Code::Variable(rewritten)),
                    bound,
                    span
                )?;
            }
            Code::Unit => push_rewritten!(values, Rewritten::Code(Code::Unit), bound, span)?,
            Code::Bool => push_rewritten!(values, Rewritten::Code(Code::Bool), bound, span)?,
            Code::I64 => push_rewritten!(values, Rewritten::Code(Code::I64), bound, span)?,
            Code::Text => push_rewritten!(values, Rewritten::Code(Code::Text), bound, span)?,
        },
        RewriteFrame::Sort(sort) => match sort {
            Sort::Codes => push_rewritten!(values, Rewritten::Sort(Sort::Codes), bound, span)?,
            Sort::Value(code) => {
                push_rewrite_frame!(frames, RewriteFrame::Value, bound, span)?;
                push_rewrite_frame!(frames, RewriteFrame::Code(code, false), bound, span)?;
            }
            Sort::Premise(prop) => {
                push_rewrite_frame!(frames, RewriteFrame::Premise, bound, span)?;
                push_rewrite_frame!(frames, RewriteFrame::Proposition(prop), bound, span)?;
            }
        },
        RewriteFrame::Proposition(prop) => match prop {
            Proposition::Pi(index, sort, body) => {
                let rewritten = match mode {
                    Rewrite::Shift(base) => {
                        index.checked_add(base).ok_or_else(|| exhausted(span))?
                    }
                    Rewrite::Replace(..) => index.checked_sub(1).ok_or_else(|| exhausted(span))?,
                };
                push_rewrite_frame!(frames, RewriteFrame::Pi(rewritten), bound, span)?;
                push_rewrite_frame!(frames, RewriteFrame::Proposition(body), bound, span)?;
                push_rewrite_frame!(frames, RewriteFrame::Sort(sort), bound, span)?;
            }
            Proposition::Eq(code, left, right) => {
                push_rewrite_frame!(frames, RewriteFrame::Eq(*left, *right), bound, span)?;
                push_rewrite_frame!(frames, RewriteFrame::Code(code, false), bound, span)?;
            }
        },
        RewriteFrame::Pair | RewriteFrame::Sum => {
            let right = take_code!(values, span)?;
            let left = take_code!(values, span)?;
            let code = if matches!(frame, RewriteFrame::Pair) {
                Code::Pair(Box::new(left), Box::new(right))
            } else {
                Code::Sum(Box::new(left), Box::new(right))
            };
            push_rewritten!(values, Rewritten::Code(code), bound, span)?;
        }
        RewriteFrame::List => {
            let item = take_code!(values, span)?;
            push_rewritten!(
                values,
                Rewritten::Code(Code::List(Box::new(item))),
                bound,
                span
            )?;
        }
        RewriteFrame::Value => {
            let code = take_code!(values, span)?;
            push_rewritten!(values, Rewritten::Sort(Sort::Value(code)), bound, span)?;
        }
        RewriteFrame::Premise => {
            let prop = take_prop!(values, span)?;
            push_rewritten!(
                values,
                Rewritten::Sort(Sort::Premise(Box::new(prop))),
                bound,
                span
            )?;
        }
        RewriteFrame::Pi(index) => {
            let body = take_prop!(values, span)?;
            let sort = take_sort!(values, span)?;
            push_rewritten!(
                values,
                Rewritten::Proposition(Proposition::Pi(index, sort, Box::new(body))),
                bound,
                span
            )?;
        }
        RewriteFrame::Eq(left, right) => {
            let code = take_code!(values, span)?;
            let (left, right) = match mode {
                Rewrite::Shift(base) => (
                    left.checked_add(base).ok_or_else(|| exhausted(span))?,
                    right.checked_add(base).ok_or_else(|| exhausted(span))?,
                ),
                Rewrite::Replace(binder, argument) => (
                    replace_index(left, binder, argument, span)?,
                    replace_index(right, binder, argument, span)?,
                ),
            };
            push_rewritten!(
                values,
                Rewritten::Proposition(Proposition::Eq(code, left, right)),
                bound,
                span
            )?;
        }
    }
    Ok(())
}
/// What a rewrite makes of one code variable.
enum VariableRewrite<'a> {
    /// The variable stays, at this index.
    Index(usize),
    /// The substituted code argument replaces it.
    Argument(&'a Code),
}
/// Rewrite the code variable `index` under `mode`.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn variable_rewrite(
    index: usize,
    mode: Rewrite<'_>,
    span: Span,
) -> Result<VariableRewrite<'_>, Diagnostic> {
    match mode {
        Rewrite::Shift(base) => match index.checked_add(base) {
            Some(shifted) => Ok(VariableRewrite::Index(shifted)),
            None => Err(exhausted(span)),
        },
        Rewrite::Replace(binder, Argument::Code(argument)) if index == binder => {
            Ok(VariableRewrite::Argument(argument))
        }
        // Every eliminated binder shifts later Type0 positions, not only Code binders.
        Rewrite::Replace(binder, _) if index > binder => match index.checked_sub(1) {
            Some(lowered) => Ok(VariableRewrite::Index(lowered)),
            None => Err(exhausted(span)),
        },
        Rewrite::Replace(..) => Ok(VariableRewrite::Index(index)),
    }
}
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
fn shifted_prop(
    prop: &Proposition,
    base: usize,
    span: Span,
    bound: usize,
) -> Result<Proposition, Diagnostic> {
    rewrite_prop(prop, Rewrite::Shift(base), span, bound)
}
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
fn replace_prop(
    prop: &Proposition,
    binder: usize,
    argument: &Argument,
    span: Span,
    bound: usize,
) -> Result<Proposition, Diagnostic> {
    rewrite_prop(prop, Rewrite::Replace(binder, argument), span, bound)
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
fn replace_index(
    index: usize,
    binder: usize,
    argument: &Argument,
    span: Span,
) -> Result<usize, Diagnostic> {
    if index == binder {
        Ok(if let Argument::Value(i) = argument {
            *i
        } else {
            index
        })
    } else if index > binder {
        index.checked_sub(1).ok_or_else(|| exhausted(span))
    } else {
        Ok(index)
    }
}
enum Argument {
    Code(Code),
    Value(usize),
    Premise,
}
enum ParseFrame<'a> {
    Code(&'a Form, u32),
    CodeSecond(&'a Form, u32, bool),
    CodePair(bool, Span),
    CodeList(Span),
    Sort(&'a Form, u32),
    SortValue(Span),
    SortPremise(Span),
    Proposition(&'a Form, u32),
    PiAfterSort {
        name: String,
        body: &'a Form,
        depth: u32,
    },
    PiFinish(usize, Span),
    EqAfterCode(&'a Form, &'a Form),
}
fn stack_binder<'a>(stack: &'a WorkStack<Binder>, name: &str) -> Option<(usize, &'a Sort)> {
    if let Some(index) = last_spilled_binder(stack, name) {
        let position = stack.inline_len.checked_add(index)?;
        return Some((position, &stack.spill[index].sort));
    }
    if let Some(index) = last_inline_binder(stack, name) {
        return match &stack.inline[index] {
            Some(binder) => Some((index, &binder.sort)),
            None => None,
        };
    }
    None
}
/// The last spilled binder named `name`, as `spill.iter().rposition(..)`.
fn last_spilled_binder(stack: &WorkStack<Binder>, name: &str) -> Option<usize> {
    let mut remaining = stack.spill.len();
    let mut found = None;
    while found.is_none() && remaining > 0 {
        remaining -= 1;
        if is_binder_named(&stack.spill[remaining], name) {
            found = Some(remaining);
        }
    }
    found
}
/// The last occupied inline slot holding a binder named `name`, as
/// `inline[..inline_len].iter().rposition(..)`.
fn last_inline_binder(stack: &WorkStack<Binder>, name: &str) -> Option<usize> {
    let mut remaining = stack.inline_len;
    let mut found = None;
    while found.is_none() && remaining > 0 {
        remaining -= 1;
        if is_slot_named(&stack.inline[remaining], name) {
            found = Some(remaining);
        }
    }
    found
}
/// Whether `binder` is named `name`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_binder_named(binder: &Binder, name: &str) -> bool {
    binder.name == name
}
/// Whether an occupied inline slot holds a binder named `name`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_slot_named(slot: &Option<Binder>, name: &str) -> bool {
    match slot {
        Some(binder) => is_binder_named(binder, name),
        None => false,
    }
}
/// The atom heading `items`, as `items.first().and_then(|item| atom(item).ok())`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn head_atom(items: &[Form]) -> Option<&str> {
    match items.first() {
        Some(first) => atom(first).ok(),
        None => None,
    }
}
fn binder<'a>(
    context: &'a WorkStack<Binder>,
    local: &'a WorkStack<Binder>,
    name: &str,
) -> Option<(usize, &'a Sort)> {
    if let Some((position, sort)) = stack_binder(local, name) {
        return Some((context.total.checked_add(position)?, sort));
    }
    stack_binder(context, name)
}
fn parsed<'m>(
    root: ParseFrame<'_>,
    context: &WorkStack<Binder>,
    mut meter: Meter<'m>,
    span: Span,
) -> Transition<'m, Rewritten> {
    let result = parse_tree(root, context, &mut meter, span);
    (meter, result)
}
/// The owned stacks of one `parsed` walk.
struct ParseStacks<'a> {
    frames: WorkStack<ParseFrame<'a>>,
    values: WorkStack<Rewritten>,
    local: WorkStack<Binder>,
}
/// Parse `root` in walk order, stopping at the first refusal.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the meter is the caller's owned transition state, written back through this borrow on every path"
)]
fn parse_tree<'a>(
    root: ParseFrame<'a>,
    context: &WorkStack<Binder>,
    meter: &mut Meter<'_>,
    span: Span,
) -> Result<Rewritten, Diagnostic> {
    let bound = meter.tree_bounds.frames;
    let mut frames = WorkStack::new();
    work_push!(frames, root, bound).map_err(|()| exhausted(span))?;
    // Only Pi binders enter `local`. An existing caller context is borrowed,
    // never copied at each nested Pi.
    let mut stacks = ParseStacks {
        frames,
        values: WorkStack::new(),
        local: WorkStack::new(),
    };
    let mut outcome = Ok(());
    while let Some(frame) = stacks.frames.pop_bounded() {
        outcome = parse_step(frame, &mut stacks, context, meter);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    stacks.values.pop_bounded().ok_or_else(|| exhausted(span))
}
// Schedule `$frame` on the walk's `$frames`, refusing at `$at` once `$bound`
// frames are pending.
macro_rules! schedule_frame {
    ($frames:ident,$bound:ident,$frame:expr,$at:expr) => {
        work_push!($frames, $frame, $bound).map_err(|()| exhausted($at))?
    };
}
/// Parse one frame: read a form or assemble a value from parsed parts.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the parse stacks and meter are the walk's locally owned state, borrowed only for one loop-free step"
)]
fn parse_step<'a>(
    frame: ParseFrame<'a>,
    stacks: &mut ParseStacks<'a>,
    context: &WorkStack<Binder>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    match frame {
        ParseFrame::Code(form, depth) => parse_code(form, depth, stacks, context, meter),
        ParseFrame::Proposition(form, depth) => parse_proposition(form, depth, stacks, meter),
        ParseFrame::EqAfterCode(left, right) => {
            parse_equality(left, right, stacks, context, meter.tree_bounds.frames)
        }
        frame => assemble_parsed(frame, stacks, context, meter.tree_bounds.frames),
    }
}
/// Read a Type0 code: a base or bound code atom, or a `Pair`, `Sum` or `List`
/// form whose parts are scheduled.
#[expect(
    tigerstyle::foreign_enum_catch_all,
    reason = "Owner: noble-contracts; the foreign enum is matched only to translate its known variants, and a new variant is a deliberate fail-closed default"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the parse stacks and meter are the walk's locally owned state, borrowed only for one loop-free step"
)]
fn parse_code<'a>(
    form: &'a Form,
    depth: u32,
    stacks: &mut ParseStacks<'a>,
    context: &WorkStack<Binder>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let bound = meter.tree_bounds.frames;
    let ParseStacks {
        frames,
        values,
        local,
    } = stacks;
    advance!(*meter, meter.visit(depth, form.span));
    if let Ok(value) = atom(form) {
        let ty = match value {
            "Unit" => Code::Unit,
            "Bool" => Code::Bool,
            "I64" => Code::I64,
            "Text" => Code::Text,
            _ => {
                let (position, sort) = binder(context, local, value)
                    .ok_or_else(|| reject(form.span, "not a bound Type0 code"))?;
                if *sort != Sort::Codes {
                    return Err(reject(form.span, "not a bound Type0 code"));
                }
                Code::Variable(position)
            }
        };
        push_rewritten!(values, Rewritten::Code(ty), bound, form.span)?;
        return Ok(());
    }
    let items = list(form)?;
    match head_atom(items) {
        Some("Pair") | Some("Sum") if items.len() == 3 => {
            let first_depth = deeper(depth, items[1].span)?;
            let is_pair = atom(&items[0])? == "Pair";
            schedule_frame!(
                frames,
                bound,
                ParseFrame::CodeSecond(&items[2], depth, is_pair),
                form.span
            );
            schedule_frame!(
                frames,
                bound,
                ParseFrame::Code(&items[1], first_depth),
                items[1].span
            );
        }
        Some("List") if items.len() == 2 => {
            let child_depth = deeper(depth, items[1].span)?;
            schedule_frame!(frames, bound, ParseFrame::CodeList(form.span), form.span);
            schedule_frame!(
                frames,
                bound,
                ParseFrame::Code(&items[1], child_depth),
                items[1].span
            );
        }
        _ => return Err(reject(form.span, "not a predicative Type0 code")),
    }
    Ok(())
}
/// Read a proposition: a `Pi` whose binder sort and body are scheduled, or an
/// `Eq` whose code and operands are scheduled.
#[expect(
    tigerstyle::foreign_enum_catch_all,
    reason = "Owner: noble-contracts; the foreign enum is matched only to translate its known variants, and a new variant is a deliberate fail-closed default"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the parse stacks and meter are the walk's locally owned state, borrowed only for one loop-free step"
)]
fn parse_proposition<'a>(
    form: &'a Form,
    depth: u32,
    stacks: &mut ParseStacks<'a>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let bound = meter.tree_bounds.frames;
    let ParseStacks { frames, .. } = stacks;
    advance!(*meter, meter.visit(depth, form.span));
    let items = list(form)?;
    match head_atom(items) {
        Some("Pi") if items.len() == 3 => {
            let binding = list(&items[1])?;
            if binding.len() != 2 {
                return Err(reject(items[1].span, "Pi needs one typed binder"));
            }
            let name = String::from(atom(&binding[0])?);
            let next_depth = deeper(depth, binding[1].span)?;
            schedule_frame!(
                frames,
                bound,
                ParseFrame::PiAfterSort {
                    name,
                    body: &items[2],
                    depth
                },
                form.span
            );
            schedule_frame!(
                frames,
                bound,
                ParseFrame::Sort(&binding[1], next_depth),
                binding[1].span
            );
        }
        Some("Eq") if items.len() == 4 => {
            let next_depth = deeper(depth, items[1].span)?;
            schedule_frame!(
                frames,
                bound,
                ParseFrame::EqAfterCode(&items[2], &items[3]),
                form.span
            );
            schedule_frame!(
                frames,
                bound,
                ParseFrame::Code(&items[1], next_depth),
                items[1].span
            );
        }
        _ => {
            return Err(reject(
                form.span,
                "expected dependent Pi or typed Eq proposition",
            ))
        }
    }
    Ok(())
}
/// Read an equality's operands once its code is parsed: each must be a bound
/// value of exactly that code.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the parse stacks and meter are the walk's locally owned state, borrowed only for one loop-free step"
)]
fn parse_equality(
    left: &Form,
    right: &Form,
    stacks: &mut ParseStacks<'_>,
    context: &WorkStack<Binder>,
    bound: usize,
) -> Result<(), Diagnostic> {
    let ParseStacks { values, local, .. } = stacks;
    let ty = take_code!(values, left.span)?;
    let left_name = atom(left)?;
    let (first, first_sort) = binder(context, local, left_name)
        .ok_or_else(|| reject(left.span, "unbound equality value"))?;
    if !is_value_of(first_sort, &ty) {
        return Err(reject(left.span, "equality operand has wrong code"));
    }
    let right_name = atom(right)?;
    let (second, second_sort) = binder(context, local, right_name)
        .ok_or_else(|| reject(right.span, "unbound equality value"))?;
    if !is_value_of(second_sort, &ty) {
        return Err(reject(right.span, "equality operand has wrong code"));
    }
    push_rewritten!(
        values,
        Rewritten::Proposition(Proposition::Eq(ty, first, second)),
        bound,
        right.span
    )?;
    Ok(())
}
/// Assemble a parsed value from its completed parts, or schedule the parts of
/// a sort or of a `Pi` body.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the parse stacks and meter are the walk's locally owned state, borrowed only for one loop-free step"
)]
fn assemble_parsed<'a>(
    frame: ParseFrame<'a>,
    stacks: &mut ParseStacks<'a>,
    context: &WorkStack<Binder>,
    bound: usize,
) -> Result<(), Diagnostic> {
    let ParseStacks {
        frames,
        values,
        local,
    } = stacks;
    match frame {
        ParseFrame::CodeSecond(second, depth, is_pair) => {
            let next_depth = deeper(depth, second.span)?;
            schedule_frame!(
                frames,
                bound,
                ParseFrame::CodePair(is_pair, second.span),
                second.span
            );
            schedule_frame!(
                frames,
                bound,
                ParseFrame::Code(second, next_depth),
                second.span
            );
        }
        ParseFrame::CodePair(is_pair, at) => {
            let right = take_code!(values, at)?;
            let left = take_code!(values, at)?;
            let ty = if is_pair {
                Code::Pair(Box::new(left), Box::new(right))
            } else {
                Code::Sum(Box::new(left), Box::new(right))
            };
            push_rewritten!(values, Rewritten::Code(ty), bound, at)?;
        }
        ParseFrame::CodeList(at) => {
            let item = take_code!(values, at)?;
            push_rewritten!(
                values,
                Rewritten::Code(Code::List(Box::new(item))),
                bound,
                at
            )?;
        }
        ParseFrame::Sort(form, depth) => {
            if atom(form).ok() == Some("Type0") {
                push_rewritten!(values, Rewritten::Sort(Sort::Codes), bound, form.span)?;
            } else if list_head_atom(form) == Some("Eq") {
                let next_depth = deeper(depth, form.span)?;
                schedule_frame!(frames, bound, ParseFrame::SortPremise(form.span), form.span);
                schedule_frame!(
                    frames,
                    bound,
                    ParseFrame::Proposition(form, next_depth),
                    form.span
                );
            } else {
                let next_depth = deeper(depth, form.span)?;
                schedule_frame!(frames, bound, ParseFrame::SortValue(form.span), form.span);
                schedule_frame!(frames, bound, ParseFrame::Code(form, next_depth), form.span);
            }
        }
        ParseFrame::SortValue(at) => {
            let ty = take_code!(values, at)?;
            push_rewritten!(values, Rewritten::Sort(Sort::Value(ty)), bound, at)?;
        }
        ParseFrame::SortPremise(at) => {
            let proof = take_prop!(values, at)?;
            push_rewritten!(
                values,
                Rewritten::Sort(Sort::Premise(Box::new(proof))),
                bound,
                at
            )?;
        }
        ParseFrame::PiAfterSort { name, body, depth } => {
            let ty = take_sort!(values, body.span)?;
            let index = context
                .total
                .checked_add(local.total)
                .ok_or_else(|| exhausted(body.span))?;
            // Each active binder belongs to a visited Pi; the derived frame
            // cap is strictly above the source's maximum visited node count.
            work_push!(local, Binder { name, sort: ty }, bound)
                .map_err(|()| exhausted(body.span))?;
            let next_depth = deeper(depth, body.span)?;
            schedule_frame!(
                frames,
                bound,
                ParseFrame::PiFinish(index, body.span),
                body.span
            );
            schedule_frame!(
                frames,
                bound,
                ParseFrame::Proposition(body, next_depth),
                body.span
            );
        }
        ParseFrame::PiFinish(index, at) => {
            let body = take_prop!(values, at)?;
            let ty = work_pop!(local).ok_or_else(|| exhausted(at))?.sort;
            push_rewritten!(
                values,
                Rewritten::Proposition(Proposition::Pi(index, ty, Box::new(body))),
                bound,
                at
            )?;
        }
        ParseFrame::Code(..) | ParseFrame::Proposition(..) | ParseFrame::EqAfterCode(..) => {
            unreachable!()
        }
    }
    Ok(())
}
/// Whether `sort` is a value of exactly `code`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_value_of(sort: &Sort, code: &Code) -> bool {
    matches!(sort, Sort::Value(value) if value == code)
}
/// Whether `prop` is a dependent Pi proposition.
fn is_pi(prop: &Proposition) -> bool {
    matches!(prop, Proposition::Pi(..))
}
/// Whether `prop` is a dependent Pi proposition, with its size when it is.
fn pi_goal_size(prop: &Proposition, bound: usize) -> (bool, Option<usize>) {
    if is_pi(prop) {
        (true, proposition_size(prop, bound))
    } else {
        (false, None)
    }
}
/// The atom heading a list form, as
/// `list(form).ok().and_then(|items| items.first()).and_then(|item| atom(item).ok())`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn list_head_atom(form: &Form) -> Option<&str> {
    match list(form) {
        Ok(items) => head_atom(items),
        Err(_) => None,
    }
}
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn code<'m>(
    form: &Form,
    context: &WorkStack<Binder>,
    meter: Meter<'m>,
    depth: u32,
) -> Transition<'m, Code> {
    let (meter, result) = parsed(ParseFrame::Code(form, depth), context, meter, form.span);
    (
        meter,
        result.and_then(|value| match value {
            Rewritten::Code(code) => Ok(code),
            Rewritten::Sort(_) | Rewritten::Proposition(_) => Err(exhausted(form.span)),
        }),
    )
}
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn sort<'m>(
    form: &Form,
    context: &WorkStack<Binder>,
    meter: Meter<'m>,
    depth: u32,
) -> Transition<'m, Sort> {
    let (meter, result) = parsed(ParseFrame::Sort(form, depth), context, meter, form.span);
    (
        meter,
        result.and_then(|value| match value {
            Rewritten::Sort(sort) => Ok(sort),
            Rewritten::Code(_) | Rewritten::Proposition(_) => Err(exhausted(form.span)),
        }),
    )
}
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn proposition<'m>(
    form: &Form,
    context: &WorkStack<Binder>,
    meter: Meter<'m>,
    depth: u32,
) -> Transition<'m, Proposition> {
    let (meter, result) = parsed(
        ParseFrame::Proposition(form, depth),
        context,
        meter,
        form.span,
    );
    (
        meter,
        result.and_then(|value| match value {
            Rewritten::Proposition(proposition) => Ok(proposition),
            Rewritten::Code(_) | Rewritten::Sort(_) => Err(exhausted(form.span)),
        }),
    )
}
enum LeanFrame<'a> {
    Code(&'a Code),
    Sort(&'a Sort),
    Proposition(&'a Proposition),
    Text(&'a str),
    Number(usize),
    /// The opening of a dependent Pi, `(` U+2200 ` (v`, as one text part.
    PiOpen,
}
fn lean_text(root: LeanFrame<'_>, span: Span, bounds: TreeBounds) -> Result<String, Diagnostic> {
    let mut pending = WorkStack::new();
    work_push!(pending, root, bounds.frames).map_err(|()| exhausted(span))?;
    // A small, constant initial allocation avoids reallocations for ordinary
    // composite terms; it never reserves from attacker-controlled limits.
    let mut output = String::with_capacity(64);
    let mut outcome = Ok(());
    while let Some(frame) = work_pop!(pending) {
        outcome = lean_step(frame, &mut pending, &mut output, span, bounds);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    Ok(output)
}
/// Render one frame: append its text or schedule its parts in print order.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the pending stack and output are the rendering's locally owned state, borrowed only for one loop-free step"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the step appends to the rendering's owned output buffer, which must stay the caller's String"
)]
fn lean_step<'a>(
    frame: LeanFrame<'a>,
    pending: &mut WorkStack<LeanFrame<'a>>,
    output: &mut String,
    span: Span,
    bounds: TreeBounds,
) -> Result<(), Diagnostic> {
    macro_rules! push {
        ($frame:expr) => {
            work_push!(pending, $frame, bounds.frames).map_err(|()| exhausted(span))?
        };
    }
    macro_rules! append {
        ($part:expr) => {{
            let part = $part;
            if output
                .len()
                .checked_add(part.len())
                .is_none_or(|len| len > bounds.bytes)
            {
                return Err(exhausted(span));
            }
            output.push_str(part);
        }};
    }
    match frame {
        LeanFrame::Text(part) => append!(part),
        LeanFrame::PiOpen => {
            let opening = pi_opening();
            append!(opening.as_str());
        }
        LeanFrame::Number(index) => {
            let digits = if index == 0 {
                1
            } else {
                usize::try_from(index.ilog10())
                    .map_err(|_| exhausted(span))?
                    .checked_add(1)
                    .ok_or_else(|| exhausted(span))?
            };
            if output
                .len()
                .checked_add(digits)
                .is_none_or(|len| len > bounds.bytes)
            {
                return Err(exhausted(span));
            }
            output.push_str(&decimal(index));
        }
        LeanFrame::Code(code) => match code {
            Code::Unit => append!(".unit"),
            Code::Bool => append!(".bool"),
            Code::I64 => append!(".i64"),
            Code::Text => append!(".text"),
            Code::Variable(index) => {
                push!(LeanFrame::Number(*index));
                push!(LeanFrame::Text("v"));
            }
            Code::Pair(left, right) | Code::Sum(left, right) => {
                push!(LeanFrame::Text(")"));
                push!(LeanFrame::Code(right));
                push!(LeanFrame::Text(" "));
                push!(LeanFrame::Code(left));
                push!(LeanFrame::Text(if matches!(code, Code::Pair(..)) {
                    "(.pair "
                } else {
                    "(.sum "
                }));
            }
            Code::List(item) => {
                push!(LeanFrame::Text(")"));
                push!(LeanFrame::Code(item));
                push!(LeanFrame::Text("(.list "));
            }
        },
        LeanFrame::Sort(sort) => match sort {
            Sort::Codes => append!("PureTyCode"),
            Sort::Value(code) => {
                push!(LeanFrame::Code(code));
                push!(LeanFrame::Text("El "));
            }
            Sort::Premise(prop) => push!(LeanFrame::Proposition(prop)),
        },
        LeanFrame::Proposition(prop) => match prop {
            Proposition::Pi(index, sort, body) => {
                push!(LeanFrame::Text(")"));
                push!(LeanFrame::Proposition(body));
                push!(LeanFrame::Text("), "));
                push!(LeanFrame::Sort(sort));
                push!(LeanFrame::Text(" : "));
                push!(LeanFrame::Number(*index));
                push!(LeanFrame::PiOpen);
            }
            Proposition::Eq(_, left, right) => {
                push!(LeanFrame::Number(*right));
                push!(LeanFrame::Text("v"));
                push!(LeanFrame::Text(" = "));
                push!(LeanFrame::Number(*left));
                push!(LeanFrame::Text("v"));
            }
        },
    }
    Ok(())
}
/// The opening `(` U+2200 ` (v` of a rendered dependent Pi.
fn pi_opening() -> String {
    let mut opening = push_code_point(String::from("("), 0x2200);
    opening.push_str(" (v");
    opening
}
/// The decimal digits of `value`, exactly as `{value}` renders them, pushed
/// as plain digits so rendering needs no formatting machinery.
fn decimal(value: usize) -> String {
    // The digits, least significant first; there is always at least one.
    let mut reversed = String::new();
    let mut rest = value;
    reversed.push(char::from(digit_byte(rest % 10)));
    rest /= 10;
    while rest > 0 {
        reversed.push(char::from(digit_byte(rest % 10)));
        rest /= 10;
    }
    let digits = reversed.as_bytes();
    let mut text = String::with_capacity(digits.len());
    let mut index = digits.len();
    while index > 0 {
        index -= 1;
        text.push(char::from(digits[index]));
    }
    text
}
/// The ASCII digit for `digit`, which is below ten; the last arm is nine.
const fn digit_byte(digit: usize) -> u8 {
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
/// The Lean name `v{index}` of the binder at de Bruijn level `index`, exactly
/// as `format!("v{index}")` renders it.
fn variable(index: usize) -> String {
    let mut name = String::from("v");
    name.push_str(&decimal(index));
    name
}
/// The lowered equation `v{left} = v{right}`.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the two binder levels are the equation's sides in print order, pinned by unit tests against the format! text this replaced"
)]
fn equation_text(left: usize, right: usize) -> String {
    let mut text = variable(left);
    text.push_str(" = ");
    text.push_str(&variable(right));
    text
}
/// The lowered intro `(fun (v{index} : {sort}) => {body})`.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the texts are the lowered intro's parts in print order, pinned by unit tests against the format! text this replaced"
)]
fn intro_text(index: usize, sort: &str, body: &str) -> String {
    let mut text = String::from("(fun (");
    text.push_str(&variable(index));
    text.push_str(" : ");
    text.push_str(sort);
    text.push_str(") => ");
    text.push_str(body);
    text.push(')');
    text
}
/// The lowered reflexivity proof `(Eq.refl v{index})`.
fn refl_text(index: usize) -> String {
    let mut text = String::from("(Eq.refl ");
    text.push_str(&variable(index));
    text.push(')');
    text
}
/// The lowered application `({function} {argument})`.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the texts are the lowered application's parts in print order, pinned by unit tests against the format! text this replaced"
)]
fn application_text(function: &str, argument: &str) -> String {
    let mut text = String::from("(");
    text.push_str(function);
    text.push(' ');
    text.push_str(argument);
    text.push(')');
    text
}
/// The lowered transport
/// `(Eq.ndrec (motive := fun (v{index} : El {code}) => {motive}) {premise} {equality})`.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the texts are the lowered transport's parts in print order, pinned by unit tests against the format! text this replaced"
)]
fn transport_text(index: usize, code: &str, motive: &str, premise: &str, equality: &str) -> String {
    let mut text = String::from("(Eq.ndrec (motive := fun (");
    text.push_str(&variable(index));
    text.push_str(" : El ");
    text.push_str(code);
    text.push_str(") => ");
    text.push_str(motive);
    text.push_str(") ");
    text.push_str(premise);
    text.push(' ');
    text.push_str(equality);
    text.push(')');
    text
}
/// `out` with the character `code` appended. Rendered Lean text keeps its
/// non-ASCII symbols out of string literals, which the extraction cannot print.
fn push_code_point(mut out: String, code: u32) -> String {
    if let Some(symbol) = char::from_u32(code) {
        out.push(symbol);
    }
    out
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn lean_code(code: &Code, span: Span, bounds: TreeBounds) -> Result<String, Diagnostic> {
    match code {
        Code::Unit => Ok(".unit".into()),
        Code::Bool => Ok(".bool".into()),
        Code::I64 => Ok(".i64".into()),
        Code::Text => Ok(".text".into()),
        Code::Variable(index) => Ok(variable(*index)),
        _ => lean_text(LeanFrame::Code(code), span, bounds),
    }
}
fn lean_sort(sort: &Sort, span: Span, bounds: TreeBounds) -> Result<String, Diagnostic> {
    match sort {
        Sort::Codes => Ok("PureTyCode".into()),
        Sort::Value(Code::Unit) => Ok("El .unit".into()),
        Sort::Value(Code::Bool) => Ok("El .bool".into()),
        Sort::Value(Code::I64) => Ok("El .i64".into()),
        Sort::Value(Code::Text) => Ok("El .text".into()),
        Sort::Value(Code::Variable(index)) => {
            let mut text = String::from("El ");
            text.push_str(&variable(*index));
            Ok(text)
        }
        Sort::Premise(prop) => lean_proposition(prop, span, bounds),
        _ => lean_text(LeanFrame::Sort(sort), span, bounds),
    }
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn lean_proposition(
    prop: &Proposition,
    span: Span,
    bounds: TreeBounds,
) -> Result<String, Diagnostic> {
    match prop {
        Proposition::Eq(_, left, right) => Ok(equation_text(*left, *right)),
        _ => lean_text(LeanFrame::Proposition(prop), span, bounds),
    }
}
enum ProofGoal<'a> {
    Borrowed(&'a Proposition),
    Owned(Proposition),
}
// Apply `$body` to the proposition a goal stands for, inside each arm. The
// pinned Aeneas refuses both a helper reading through `&ProofGoal` and a
// reference joined out of the two arms; an owned goal borrowed in place, with
// the use inside each arm, is accepted.
macro_rules! with_goal {
    ($goal:expr, |$prop:ident| $body:expr) => {
        match &$goal {
            ProofGoal::Borrowed($prop) => $body,
            ProofGoal::Owned($prop) => $body,
        }
    };
}
enum ProofSort<'a> {
    Borrowed(&'a Sort),
    Owned(Sort),
}
// Apply `$body` to the sort a proof sort stands for, as `with_goal!`.
macro_rules! with_sort {
    ($sort:expr, |$value:ident| $body:expr) => {
        match &$sort {
            ProofSort::Borrowed($value) => $body,
            ProofSort::Owned($value) => $body,
        }
    };
}
enum ProofFrame<'a> {
    Infer(&'a Form, u32),
    Check(&'a Form, ProofGoal<'a>, u32),
    ApplyFunction {
        term: &'a Form,
        argument: &'a Form,
        depth: u32,
    },
    ApplyPremise {
        term: &'a Form,
        body: Box<Proposition>,
        index: usize,
        lowered: String,
        depth: u32,
    },
    AfterInfer {
        term: &'a Form,
        goal: ProofGoal<'a>,
        pi: bool,
    },
    IntroFinish {
        term: &'a Form,
        index: usize,
        expected: ProofSort<'a>,
    },
    SubstEquation(SubstEquation<'a>),
    SubstPremise {
        term: &'a Form,
        code: Code,
        body: Proposition,
        index: usize,
        equality_lean: String,
    },
}
/// A `subst` proof waiting for its equality evidence.
struct SubstEquation<'a> {
    term: &'a Form,
    goal: ProofGoal<'a>,
    code: Code,
    name: String,
    motive: &'a Form,
    binder_span: Span,
    evidence: &'a Form,
    premise: &'a Form,
    depth: u32,
}
enum ProofValue {
    Inferred(Proposition, String),
    Checked(String),
}
fn proof_walk<'a, 'm>(
    root: ProofFrame<'a>,
    available: &[Available],
    mut meter: Meter<'m>,
    span: Span,
) -> Transition<'m, ProofValue> {
    let result = proof_tree(root, available, &mut meter, span);
    (meter, result)
}
/// The owned state of one `proof_walk`.
struct ProofWalk<'a> {
    frames: WorkStack<ProofFrame<'a>>,
    context: WorkStack<Binder>,
    completed: Option<ProofValue>,
    failure: Option<Diagnostic>,
}
/// Check `root` in walk order. After a refusal the walk still unwinds every
/// pending intro continuation, whose charge may replace that refusal.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the meter is the caller's owned transition state, written back through this borrow on every path"
)]
fn proof_tree<'a>(
    root: ProofFrame<'a>,
    available: &[Available],
    meter: &mut Meter<'_>,
    span: Span,
) -> Result<ProofValue, Diagnostic> {
    let bound = meter.tree_bounds.frames;
    let mut frames = WorkStack::new();
    work_push!(frames, root, bound).map_err(|()| exhausted(span))?;
    let mut walk = ProofWalk {
        frames,
        context: WorkStack::new(),
        completed: None,
        failure: None,
    };
    let mut escaped = None;
    while let Some(frame) = walk.frames.pop_bounded() {
        // An intro charges its wrapper even when its body failed. The charge
        // may itself fail and override that earlier diagnostic, so unwind all
        // pending intro continuations instead of returning immediately.
        if let ProofFrame::IntroFinish {
            term,
            index,
            expected,
        } = frame
        {
            if let Err(error) = finish_intro(term, index, expected, &mut walk, meter) {
                escaped = Some(error);
                break;
            }
            continue;
        }
        if walk.failure.is_some() {
            continue;
        }
        if let Err(error) = proof_step(frame, &mut walk, available, meter) {
            walk.failure = Some(error);
            walk.completed = None;
        }
    }
    if let Some(error) = escaped {
        return Err(error);
    }
    if let Some(error) = walk.failure {
        return Err(error);
    }
    walk.completed.ok_or_else(|| exhausted(span))
}
/// Close one intro: pop its binder, charge its wrapper even after a refusal,
/// and wrap the checked body. An `Err` ends the whole walk at once.
#[expect(
    tigerstyle::foreign_enum_catch_all,
    reason = "Owner: noble-contracts; the foreign enum is matched only to translate its known variants, and a new variant is a deliberate fail-closed default"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the walk state and meter are locally owned by the proof walk, borrowed only for one loop-free continuation"
)]
fn finish_intro(
    term: &Form,
    index: usize,
    expected: ProofSort<'_>,
    walk: &mut ProofWalk<'_>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let ProofWalk {
        context,
        completed,
        failure,
        ..
    } = walk;
    work_pop!(context);
    let body = if failure.is_none() {
        match completed.take() {
            Some(ProofValue::Checked(body)) => Some(body),
            _ => {
                *failure = Some(exhausted(term.span));
                None
            }
        }
    } else {
        None
    };
    let body_bytes = match &body {
        Some(body) => body.len(),
        None => 0,
    };
    match with_sort!(expected, |sort| charge_intro(
        sort, body_bytes, term.span, meter
    )) {
        Err(error) => *failure = Some(error),
        Ok(sort_text) => {
            if failure.is_none() {
                let body = body.ok_or_else(|| exhausted(term.span))?;
                *completed = Some(ProofValue::Checked(intro_text(index, &sort_text, &body)));
            }
        }
    }
    Ok(())
}
/// Charge one intro wrapper: its rendered binder sort plus its body text.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the meter is the caller's owned transition state, written back through this borrow on every path"
)]
fn charge_intro(
    expected: &Sort,
    body_bytes: usize,
    span: Span,
    meter: &mut Meter<'_>,
) -> Result<String, Diagnostic> {
    let sort_text = lean_sort(expected, span, meter.tree_bounds)?;
    #[expect(
        tigerstyle::numeric_units,
        reason = "Owner: noble-contracts; the constant is a count of source elements, and the name states its unit at every use site through the surrounding type and call"
    )]
    let size = body_bytes
        .checked_add(sort_text.len())
        .ok_or_else(|| exhausted(span))?;
    advance!(*meter, meter.charge(size, span));
    Ok(sort_text)
}
/// The last available proof named `name`, as `available.iter().rev().find(..)`.
fn last_available<'a>(available: &'a [Available], name: &str) -> Option<&'a Available> {
    let mut remaining = available.len();
    let mut found = None;
    while found.is_none() && remaining > 0 {
        remaining -= 1;
        if is_available_named(&available[remaining], name) {
            found = Some(remaining);
        }
    }
    match found {
        Some(index) => Some(&available[index]),
        None => None,
    }
}
/// Whether the available proof `item` is named `name`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_available_named(item: &Available, name: &str) -> bool {
    item.name == name
}
// Take the proposition and Lean text the last frame inferred into `$completed`.
macro_rules! take_inferred {
    ($completed:ident,$at:expr) => {
        match $completed.take() {
            Some(ProofValue::Inferred(goal, lean)) => Ok((goal, lean)),
            Some(ProofValue::Checked(_)) | None => Err(exhausted($at)),
        }
    };
}
// Take the Lean text the last frame checked into `$completed`.
macro_rules! take_checked {
    ($completed:ident,$at:expr) => {
        match $completed.take() {
            Some(ProofValue::Checked(lean)) => Ok(lean),
            Some(ProofValue::Inferred(..)) | None => Err(exhausted($at)),
        }
    };
}
/// Check one proof frame, scheduling its continuations in walk order.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the walk state and meter are locally owned by the proof walk, borrowed only for one loop-free step"
)]
fn proof_step<'a>(
    frame: ProofFrame<'a>,
    walk: &mut ProofWalk<'a>,
    available: &[Available],
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    match frame {
        ProofFrame::Infer(term, depth) => infer_step(term, depth, walk, available, meter),
        ProofFrame::Check(term, goal, depth) => check_step(term, goal, depth, walk, meter),
        ProofFrame::SubstEquation(equation) => subst_equation_step(equation, walk, meter),
        frame => continuation_step(frame, walk, meter),
    }
}
/// Infer the proposition a `use`, `apply` or `refl` proof term establishes.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the walk state and meter are locally owned by the proof walk, borrowed only for one loop-free step"
)]
fn infer_step<'a>(
    term: &'a Form,
    depth: u32,
    walk: &mut ProofWalk<'a>,
    available: &[Available],
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let bound = meter.tree_bounds.frames;
    let ProofWalk {
        frames,
        context,
        completed,
        ..
    } = walk;
    advance!(*meter, meter.visit(depth, term.span));
    if let Ok(args) = operation(term, "use", 1) {
        let name = atom(&args[0])?;
        if let Some((index, sort)) = stack_binder(context, name) {
            let Sort::Premise(proof) = sort else {
                return Err(reject(term.span, "use does not name a proof premise"));
            };
            *completed = Some(ProofValue::Inferred((**proof).clone(), variable(index)));
            return Ok(());
        }
        let dependency = last_available(available, name).ok_or_else(|| {
            reject(
                term.span,
                "unresolved, private, forward or cyclic proof use",
            )
        })?;
        advance!(*meter, meter.charge(dependency.lean_term.len(), term.span));
        advance!(
            *meter,
            meter.substitute(
                proposition_size(&dependency.goal, bound).ok_or_else(|| exhausted(term.span))?,
                term.span
            )
        );
        *completed = Some(ProofValue::Inferred(
            shifted_prop(&dependency.goal, context.total, term.span, bound)?,
            dependency.lean_term.clone(),
        ));
        return Ok(());
    }
    if let Ok(args) = operation(term, "apply", 2) {
        let child_depth = deeper(depth, args[0].span)?;
        schedule_frame!(
            frames,
            bound,
            ProofFrame::ApplyFunction {
                term,
                argument: &args[1],
                depth
            },
            term.span
        );
        schedule_frame!(
            frames,
            bound,
            ProofFrame::Infer(&args[0], child_depth),
            args[0].span
        );
        return Ok(());
    }
    if let Ok(args) = operation(term, "refl", 1) {
        let name = atom(&args[0])?;
        let (index, sort) = stack_binder(context, name)
            .ok_or_else(|| reject(args[0].span, "unbound refl value"))?;
        let Sort::Value(code) = sort else {
            return Err(reject(term.span, "refl operand is not a total value"));
        };
        *completed = Some(ProofValue::Inferred(
            Proposition::Eq(code.clone(), index, index),
            refl_text(index),
        ));
        return Ok(());
    }
    Err(reject(term.span, "proof term has no inferable proposition"))
}
/// Check `term` against `goal`: an `intro` for a Pi goal, a `subst`, or an
/// inferred proposition compared with the goal once it is inferred.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the walk state and meter are locally owned by the proof walk, borrowed only for one loop-free step"
)]
fn check_step<'a>(
    term: &'a Form,
    goal: ProofGoal<'a>,
    depth: u32,
    walk: &mut ProofWalk<'a>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let bound = meter.tree_bounds.frames;
    let ProofWalk {
        frames, context, ..
    } = walk;
    advance!(*meter, meter.visit(depth, term.span));
    // The size is only charged on the intro path below; it
    // is read here, once, because the goal is still whole.
    let (is_pi_goal, goal_size) = with_goal!(goal, |prop| pi_goal_size(prop, bound));
    if is_pi_goal {
        let args = match operation(term, "intro", 2) {
            Ok(args) => args,
            Err(_) => {
                let child_depth = deeper(depth, term.span)?;
                schedule_frame!(
                    frames,
                    bound,
                    ProofFrame::AfterInfer {
                        term,
                        goal,
                        pi: true
                    },
                    term.span
                );
                schedule_frame!(
                    frames,
                    bound,
                    ProofFrame::Infer(term, child_depth),
                    term.span
                );
                return Ok(());
            }
        };
        let binder_form = list(&args[0])?;
        if binder_form.len() != 2 {
            return Err(reject(args[0].span, "intro needs a typed binder"));
        }
        let name = String::from(atom(&binder_form[0])?);
        let actual = advance!(
            *meter,
            sort(
                &binder_form[1],
                context,
                *meter,
                deeper(depth, binder_form[1].span)?
            )
        );
        advance!(
            *meter,
            meter.normalize(
                goal_size.ok_or_else(|| exhausted(args[0].span))?,
                args[0].span
            )
        );
        let (index, expected, rest) = match goal {
            ProofGoal::Borrowed(Proposition::Pi(index, expected, rest)) => (
                *index,
                ProofSort::Borrowed(expected),
                ProofGoal::Borrowed(rest),
            ),
            ProofGoal::Owned(Proposition::Pi(index, expected, rest)) => {
                (index, ProofSort::Owned(expected), ProofGoal::Owned(*rest))
            }
            _ => return Err(exhausted(term.span)),
        };
        let is_expected_sort = with_sort!(expected, |sort| sort_equal(&actual, sort));
        if !is_expected_sort || index != context.total {
            return Err(reject(args[0].span, "intro binder type differs from goal"));
        }
        let body_depth = deeper(depth, args[1].span)?;
        work_push!(context, Binder { name, sort: actual }, bound)
            .map_err(|()| exhausted(args[1].span))?;
        schedule_frame!(
            frames,
            bound,
            ProofFrame::IntroFinish {
                term,
                index,
                expected
            },
            term.span
        );
        schedule_frame!(
            frames,
            bound,
            ProofFrame::Check(&args[1], rest, body_depth),
            args[1].span
        );
        return Ok(());
    }
    if let Ok(parts) = operation(term, "subst", 3) {
        let motive = operation(&parts[0], "motive", 2)?;
        let binder_form = list(&motive[0])?;
        if binder_form.len() != 2 {
            return Err(reject(
                motive[0].span,
                "subst motive needs one typed value binder",
            ));
        }
        let name = String::from(atom(&binder_form[0])?);
        let Sort::Value(code) = advance!(
            *meter,
            sort(
                &binder_form[1],
                context,
                *meter,
                deeper(depth, binder_form[1].span)?
            )
        ) else {
            return Err(reject(
                motive[0].span,
                "subst motive binder must be a total value",
            ));
        };
        let child_depth = deeper(depth, parts[1].span)?;
        schedule_frame!(
            frames,
            bound,
            ProofFrame::SubstEquation(SubstEquation {
                term,
                goal,
                code,
                name,
                motive: &motive[1],
                binder_span: motive[0].span,
                evidence: &parts[1],
                premise: &parts[2],
                depth,
            }),
            term.span
        );
        schedule_frame!(
            frames,
            bound,
            ProofFrame::Infer(&parts[1], child_depth),
            parts[1].span
        );
        return Ok(());
    }
    let child_depth = deeper(depth, term.span)?;
    schedule_frame!(
        frames,
        bound,
        ProofFrame::AfterInfer {
            term,
            goal,
            pi: false
        },
        term.span
    );
    schedule_frame!(
        frames,
        bound,
        ProofFrame::Infer(term, child_depth),
        term.span
    );
    Ok(())
}
/// Close a `subst` once its equality evidence is inferred: check the motive,
/// substitute both sides, compare with the goal and schedule the premise.
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the walk state and meter are locally owned by the proof walk, borrowed only for one loop-free step"
)]
fn subst_equation_step<'a>(
    equation: SubstEquation<'a>,
    walk: &mut ProofWalk<'a>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let bound = meter.tree_bounds.frames;
    let ProofWalk {
        frames,
        context,
        completed,
        ..
    } = walk;
    let SubstEquation {
        term,
        goal,
        code,
        name,
        motive,
        binder_span,
        evidence,
        premise,
        depth,
    } = equation;
    let (equation, equality_lean) = take_inferred!(completed, evidence.span)?;
    let Proposition::Eq(equation_code, left, right) = equation else {
        return Err(reject(evidence.span, "subst requires equality evidence"));
    };
    advance!(
        *meter,
        meter.normalize(
            code_size(&code, bound).ok_or_else(|| exhausted(evidence.span))?,
            evidence.span
        )
    );
    if equation_code != code {
        return Err(reject(binder_span, "subst motive has wrong equality type"));
    }
    let index = context.total;
    let body_depth = deeper(depth, motive.span)?;
    work_push!(
        context,
        Binder {
            name,
            sort: Sort::Value(code.clone())
        },
        bound
    )
    .map_err(|()| exhausted(motive.span))?;
    let (next, body) = proposition(motive, context, *meter, body_depth);
    *meter = next;
    work_pop!(context);
    let body = body?;
    if !matches!(body, Proposition::Eq(..)) {
        return Err(reject(
            motive.span,
            "subst motive must be a total Eq proposition",
        ));
    }
    #[expect(
        tigerstyle::numeric_units,
        reason = "Owner: noble-contracts; the constant is a count of source elements, and the name states its unit at every use site through the surrounding type and call"
    )]
    let size = proposition_size(&body, bound)
        .and_then(|size| size.checked_mul(2))
        .ok_or_else(|| exhausted(term.span))?;
    advance!(*meter, meter.substitute(size, term.span));
    let before = replace_prop(&body, index, &Argument::Value(left), term.span, bound)?;
    let after = replace_prop(&body, index, &Argument::Value(right), term.span, bound)?;
    advance!(
        *meter,
        meter.normalize(
            with_goal!(goal, |prop| proposition_size(prop, bound))
                .ok_or_else(|| exhausted(term.span))?,
            term.span
        )
    );
    let is_goal = with_goal!(goal, |prop| proposition_equal(&after, prop));
    if !is_goal {
        return Err(reject(term.span, "subst result differs from expected goal"));
    }
    let child_depth = deeper(depth, premise.span)?;
    schedule_frame!(
        frames,
        bound,
        ProofFrame::SubstPremise {
            term,
            code,
            body,
            index,
            equality_lean
        },
        term.span
    );
    schedule_frame!(
        frames,
        bound,
        ProofFrame::Check(premise, ProofGoal::Owned(before), child_depth),
        premise.span
    );
    Ok(())
}
/// Resume a frame waiting on its completed child: finish an `apply`, compare
/// an inferred proposition with its goal, or wrap a checked `subst` premise.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the walk state and meter are locally owned by the proof walk, borrowed only for one loop-free step"
)]
fn continuation_step<'a>(
    frame: ProofFrame<'a>,
    walk: &mut ProofWalk<'a>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let bound = meter.tree_bounds.frames;
    let ProofWalk {
        frames,
        context,
        completed,
        ..
    } = walk;
    macro_rules! finish_apply {
        ($term:expr,$depth:expr,$body:expr,$index:expr,$argument:expr,$lowered:expr,$expression:expr) => {{
            let (term, depth, body, index, argument, lowered, expression) = (
                $term,
                $depth,
                $body,
                $index,
                $argument,
                $lowered,
                $expression,
            );
            let step = usize::try_from(depth)
                .ok()
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| exhausted(term.span))?;
            advance!(*meter, meter.charge(step, term.span));
            advance!(
                *meter,
                meter.charge(
                    lowered
                        .len()
                        .checked_add(expression.len())
                        .ok_or_else(|| exhausted(term.span))?,
                    term.span
                )
            );
            advance!(
                *meter,
                meter.substitute(
                    proposition_size(&body, bound).ok_or_else(|| exhausted(term.span))?,
                    term.span
                )
            );
            *completed = Some(ProofValue::Inferred(
                replace_prop(&body, index, &argument, term.span, bound)?,
                application_text(&lowered, &expression),
            ));
        }};
    }
    match frame {
        ProofFrame::ApplyFunction {
            term,
            argument,
            depth,
        } => {
            let (function, lowered) = take_inferred!(completed, term.span)?;
            let Proposition::Pi(index, domain, body) = function else {
                return Err(reject(term.span, "apply requires dependent Pi proof"));
            };
            if index != context.total {
                return Err(reject(term.span, "apply binder scope mismatch"));
            }
            match domain {
                Sort::Codes => {
                    let ty = advance!(
                        *meter,
                        code(argument, context, *meter, deeper(depth, argument.span)?)
                    );
                    let expression = lean_code(&ty, argument.span, meter.tree_bounds)?;
                    finish_apply!(
                        term,
                        depth,
                        body,
                        index,
                        Argument::Code(ty),
                        lowered,
                        expression
                    );
                }
                Sort::Value(expected) => {
                    let name = atom(argument)?;
                    let (value, sort) = stack_binder(context, name)
                        .ok_or_else(|| reject(argument.span, "apply value is unbound"))?;
                    advance!(
                        *meter,
                        meter.normalize(
                            code_size(&expected, bound).ok_or_else(|| exhausted(argument.span))?,
                            argument.span
                        )
                    );
                    if sort != &Sort::Value(expected) {
                        return Err(reject(argument.span, "apply value has wrong code"));
                    }
                    finish_apply!(
                        term,
                        depth,
                        body,
                        index,
                        Argument::Value(value),
                        lowered,
                        variable(value)
                    );
                }
                Sort::Premise(expected) => {
                    let child_depth = deeper(depth, argument.span)?;
                    schedule_frame!(
                        frames,
                        bound,
                        ProofFrame::ApplyPremise {
                            term,
                            body,
                            index,
                            lowered,
                            depth
                        },
                        term.span
                    );
                    schedule_frame!(
                        frames,
                        bound,
                        ProofFrame::Check(argument, ProofGoal::Owned(*expected), child_depth),
                        argument.span
                    );
                }
            }
        }
        ProofFrame::ApplyPremise {
            term,
            body,
            index,
            lowered,
            depth,
        } => {
            let expression = take_checked!(completed, term.span)?;
            finish_apply!(
                term,
                depth,
                body,
                index,
                Argument::Premise,
                lowered,
                expression
            );
        }
        ProofFrame::AfterInfer { term, goal, pi } => {
            let (inferred, lowered) = take_inferred!(completed, term.span)?;
            advance!(
                *meter,
                meter.normalize(
                    with_goal!(goal, |prop| proposition_size(prop, bound))
                        .ok_or_else(|| exhausted(term.span))?,
                    term.span
                )
            );
            let is_goal = with_goal!(goal, |prop| proposition_equal(&inferred, prop));
            if !is_goal {
                let reason = if pi {
                    "inferred proof does not match dependent Pi goal"
                } else {
                    "proof does not establish exact expected equality"
                };
                return Err(reject(term.span, reason));
            }
            *completed = Some(ProofValue::Checked(lowered));
        }
        ProofFrame::SubstPremise {
            term,
            code,
            body,
            index,
            equality_lean,
        } => {
            let premise = take_checked!(completed, term.span)?;
            advance!(
                *meter,
                meter.charge(
                    premise
                        .len()
                        .checked_add(equality_lean.len())
                        .ok_or_else(|| exhausted(term.span))?,
                    term.span
                )
            );
            let code_text = lean_code(&code, term.span, meter.tree_bounds)?;
            let motive_text = lean_proposition(&body, term.span, meter.tree_bounds)?;
            *completed = Some(ProofValue::Checked(transport_text(
                index,
                &code_text,
                &motive_text,
                &premise,
                &equality_lean,
            )));
        }
        ProofFrame::Infer(..)
        | ProofFrame::Check(..)
        | ProofFrame::SubstEquation(..)
        | ProofFrame::IntroFinish { .. } => unreachable!(),
    }
    Ok(())
}
/// Checks only the closed predicative fragment; contract proofs have a separate
/// finite reviewed MC1 rule checker below. An unchecked term is never lowered.
pub fn check_batch(batch: &ProofBatch, limits: Limits) -> Result<Vec<CheckedProof>, Diagnostic> {
    check_batch_with_budgets(batch, limits, ProofBudgets::from_limits(limits))
        .map(|(checked, _)| checked)
}
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
pub fn check_batch_with_budgets(
    batch: &ProofBatch,
    limits: Limits,
    budgets: ProofBudgets,
) -> Result<(Vec<CheckedProof>, ProofWork), Diagnostic> {
    if u32::try_from(batch.source.len()).map_or(true, |len| len > limits.bytes) {
        return Err(exhausted(Span { start: 0, end: 0 }));
    }
    let config = MeterConfig {
        budgets,
        limits,
        tree_bounds: TreeBounds::new(limits).ok_or_else(|| exhausted(Span { start: 0, end: 0 }))?,
    };
    // Elaboration and checking share one request budget (r[IMPL-TRUST-01]):
    // the checker resumes from the work staging already charged to the
    // `implicit-args/v1` elaborator, which is zero for a fully explicit batch.
    let mut meter = Meter {
        nodes: 0,
        work: batch.elaboration_work.total,
        normalization: batch.elaboration_work.normalization,
        substitution: batch.elaboration_work.substitution,
        config: &config,
    };
    // Each accepted obligation has already consumed a visited node. Do not
    // reserve caller-supplied fan-out before charging it.
    let result_bound =
        usize::try_from(limits.nodes).map_err(|_| exhausted(Span { start: 0, end: 0 }))?;
    let available_bound = batch
        .dependencies
        .len()
        .checked_add(batch.obligations.len())
        .ok_or_else(|| exhausted(Span { start: 0, end: 0 }))?;
    let mut admitted = Admitted {
        available: Vec::new(),
        available_bound,
        result: Vec::new(),
        result_bound,
    };
    let mut index = 0;
    let mut dependencies = Ok(());
    while dependencies.is_ok() && index < batch.dependencies.len() {
        dependencies = admit_dependency(&batch.dependencies[index], &mut admitted, &mut meter);
        index += 1;
    }
    dependencies?;
    let mut index = 0;
    let mut obligations = Ok(());
    while obligations.is_ok() && index < batch.obligations.len() {
        obligations = admit_obligation(&batch.obligations[index], &mut admitted, &mut meter);
        index += 1;
    }
    obligations?;
    Ok((
        admitted.result,
        ProofWork {
            normalization: meter.normalization,
            substitution: meter.substitution,
            total: meter.work,
        },
    ))
}
/// The proofs one batch check has admitted so far, with their bounds.
struct Admitted {
    available: Vec<Available>,
    available_bound: usize,
    result: Vec<CheckedProof>,
    result_bound: usize,
}
/// Check one imported dependency and make it available to later proofs.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the admitted proofs and meter are the batch check's locally owned state, borrowed only for one loop-free step"
)]
fn admit_dependency(
    dependency: &ProofDependency,
    admitted: &mut Admitted,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    if !dependency.exported {
        return Err(reject(
            dependency.declaration.span,
            "private proof cannot be imported",
        ));
    }
    let checked = advance!(*meter, check_dependency(dependency, *meter));
    if admitted.available.len() >= admitted.available_bound {
        return Err(exhausted(dependency.declaration.span));
    }
    admitted.available.push(checked);
    Ok(())
}
/// Check one obligation, recording its proof and any goal it exports.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the admitted proofs and meter are the batch check's locally owned state, borrowed only for one loop-free step"
)]
fn admit_obligation(
    obligation: &ProofObligation,
    admitted: &mut Admitted,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let (checked, exported) = advance!(
        *meter,
        check_obligation(obligation, &admitted.available, *meter)
    );
    if let Some(goal) = exported {
        if admitted.available.len() >= admitted.available_bound {
            return Err(exhausted(obligation.span));
        }
        admitted.available.push(goal);
    }
    if admitted.result.len() >= admitted.result_bound {
        return Err(exhausted(obligation.span));
    }
    admitted.result.push(checked);
    Ok(())
}
fn check_obligation<'m>(
    obligation: &ProofObligation,
    available: &[Available],
    mut meter: Meter<'m>,
) -> Transition<'m, (CheckedProof, Option<Available>)> {
    let result = obligation_proof(obligation, available, &mut meter);
    (meter, result)
}
/// Check one obligation against the proofs available before it.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the meter is the caller's owned transition state, written back through this borrow on every path"
)]
fn obligation_proof(
    obligation: &ProofObligation,
    available: &[Available],
    meter: &mut Meter<'_>,
) -> Result<(CheckedProof, Option<Available>), Diagnostic> {
    advance!(*meter, meter.visit(0, obligation.span));
    advance!(*meter, validated(&obligation.term, *meter, 0));
    match &obligation.goal {
        PendingGoal::Pure {
            proposition: statement,
        } => {
            if obligation.revision != 1 {
                return Err(reject(obligation.span, "pure proof requires revision 1"));
            }
            advance!(*meter, validated(statement, *meter, 0));
            let goal = advance!(*meter, proposition(statement, &WorkStack::new(), *meter, 0));
            let lowered = match advance!(
                *meter,
                proof_walk(
                    ProofFrame::Check(&obligation.term, ProofGoal::Borrowed(&goal), 0),
                    available,
                    *meter,
                    obligation.term.span
                )
            ) {
                ProofValue::Checked(lowered) => lowered,
                ProofValue::Inferred(..) => return Err(exhausted(obligation.term.span)),
            };
            let checked = CheckedProof {
                name: obligation.name.clone(),
                kind: ProofKind::Pure,
                lean_term: lowered.clone(),
                claim: lean_proposition(&goal, obligation.span, meter.tree_bounds)?,
                model_revision: String::new(),
                checker_revision: String::new(),
            };
            Ok((
                checked,
                Some(Available {
                    name: obligation.name.clone(),
                    goal,
                    lean_term: lowered,
                }),
            ))
        }
        PendingGoal::Contract { contract } => {
            if obligation.revision != contract.revision {
                return Err(reject(
                    obligation.span,
                    "proof and contract revisions differ",
                ));
            }
            let checked = match contract.revision {
                1 => advance!(*meter, check_contract(obligation, contract, *meter)),
                2 => advance!(
                    *meter,
                    named_v2::check_contract(obligation, contract, *meter)
                ),
                _ => return Err(reject(obligation.span, "unknown contract revision")),
            };
            Ok((checked, None))
        }
    }
}
enum DependencyFrame<'a> {
    Enter {
        dependency: &'a ProofDependency,
        depth: u32,
        parent: Option<&'a ProofDependency>,
    },
    Children {
        dependency: &'a ProofDependency,
        depth: u32,
        next: usize,
        start: usize,
    },
}
fn check_dependency<'m>(
    dependency: &ProofDependency,
    mut meter: Meter<'m>,
) -> Transition<'m, Available> {
    let result = dependency_proof(dependency, &mut meter);
    (meter, result)
}
/// The identity by which an active dependency detects a cycle.
struct DependencyIdentity {
    module: String,
    version: u32,
    name: String,
}
/// The owned state of one `check_dependency` walk.
struct DependencyWalk<'a> {
    frames: WorkStack<DependencyFrame<'a>>,
    active: WorkStack<DependencyIdentity>,
    completed: Option<Available>,
    // Contiguous siblings share one scratch buffer; a frame owns only its
    // starting offset. No per-dependency vector or eager fan-out reservation.
    available: Vec<Available>,
    available_bound: usize,
}
/// Check a dependency after all of its prerequisites, depth first.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the meter is the caller's owned transition state, written back through this borrow on every path"
)]
fn dependency_proof(
    dependency: &ProofDependency,
    meter: &mut Meter<'_>,
) -> Result<Available, Diagnostic> {
    let bound = meter.tree_bounds.frames;
    let available_bound =
        usize::try_from(meter.limits.nodes).map_err(|_| exhausted(dependency.declaration.span))?;
    let mut frames = WorkStack::new();
    work_push!(
        frames,
        DependencyFrame::Enter {
            dependency,
            depth: 0,
            parent: None
        },
        bound
    )
    .map_err(|()| exhausted(dependency.declaration.span))?;
    let mut walk = DependencyWalk {
        frames,
        active: WorkStack::new(),
        completed: None,
        available: Vec::new(),
        available_bound,
    };
    let mut outcome = Ok(());
    while let Some(frame) = walk.frames.pop_bounded() {
        outcome = dependency_step(frame, &mut walk, meter);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    walk.completed
        .ok_or_else(|| exhausted(dependency.declaration.span))
}
/// Visit one dependency frame: enter a dependency or advance through its
/// prerequisites, closing it once all of them are checked.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the walk state and meter are locally owned by the dependency walk, borrowed only for one loop-free step"
)]
fn dependency_step<'a>(
    frame: DependencyFrame<'a>,
    walk: &mut DependencyWalk<'a>,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let bound = meter.tree_bounds.frames;
    let DependencyWalk {
        frames,
        active,
        completed,
        available,
        available_bound,
    } = walk;
    match frame {
        DependencyFrame::Enter {
            dependency,
            depth,
            parent,
        } => {
            advance!(*meter, meter.visit(depth, dependency.declaration.span));
            advance!(
                *meter,
                meter.charge(dependency.source.len(), dependency.declaration.span)
            );
            let is_repeats = repeats_active(active, dependency);
            if is_repeats {
                return Err(reject(
                    dependency.declaration.span,
                    "cyclic proof dependency",
                ));
            }
            let is_private_local = match parent {
                Some(parent) => is_private_local_of(dependency, parent),
                None => false,
            };
            if (!dependency.exported && !is_private_local)
                || dependency.source.is_empty()
                || dependency.declaration.name.is_empty()
            {
                return Err(reject(
                    dependency.declaration.span,
                    "unpublished or private proof dependency",
                ));
            }
            work_push!(
                active,
                DependencyIdentity {
                    module: dependency.module.clone(),
                    version: dependency.version,
                    name: dependency.declaration.name.clone(),
                },
                bound
            )
            .map_err(|()| exhausted(dependency.declaration.span))?;
            work_push!(
                frames,
                DependencyFrame::Children {
                    dependency,
                    depth,
                    next: 0,
                    start: available.len()
                },
                bound
            )
            .map_err(|()| exhausted(dependency.declaration.span))?;
        }
        DependencyFrame::Children {
            dependency,
            depth,
            next,
            start,
        } => {
            if let Some(child) = completed.take() {
                let sibling_count = available
                    .len()
                    .checked_sub(start)
                    .ok_or_else(|| exhausted(dependency.declaration.span))?;
                if sibling_count >= dependency.dependencies.len() {
                    return Err(exhausted(dependency.declaration.span));
                }
                if available.len() >= *available_bound {
                    return Err(exhausted(dependency.declaration.span));
                }
                available.push(child);
            }
            if let Some(prerequisite) = dependency.dependencies.get(next) {
                let child_depth = deeper(depth, prerequisite.declaration.span)?;
                let following = next
                    .checked_add(1)
                    .ok_or_else(|| exhausted(prerequisite.declaration.span))?;
                work_push!(
                    frames,
                    DependencyFrame::Children {
                        dependency,
                        depth,
                        next: following,
                        start
                    },
                    bound
                )
                .map_err(|()| exhausted(prerequisite.declaration.span))?;
                work_push!(
                    frames,
                    DependencyFrame::Enter {
                        dependency: prerequisite,
                        depth: child_depth,
                        parent: Some(dependency),
                    },
                    bound
                )
                .map_err(|()| exhausted(prerequisite.declaration.span))?;
                return Ok(());
            }
            let siblings = available
                .get(start..)
                .ok_or_else(|| exhausted(dependency.declaration.span))?;
            let (_, closed) = advance!(
                *meter,
                check_obligation(&dependency.declaration, siblings, *meter)
            );
            available.truncate(start);
            work_pop!(active);
            let mut closed = closed.ok_or_else(|| {
                reject(
                    dependency.declaration.span,
                    "contract theorem dependency is not in the pure fragment",
                )
            })?;
            closed.name = dependency.name.clone();
            *completed = Some(closed);
        }
    }
    Ok(())
}
/// Whether `dependency` is a private declaration earlier in its parent's own
/// module source.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_private_local_of(dependency: &ProofDependency, parent: &ProofDependency) -> bool {
    dependency.module == parent.module
        && dependency.version == parent.version
        && dependency.source == parent.source
        && dependency.declaration.span.start < parent.declaration.span.start
}
/// Whether an active ancestor has the identity of `dependency`: the inline
/// slots, then the spill, in the order `iter().flatten().chain(..).any(..)` scanned.
fn repeats_active(active: &WorkStack<DependencyIdentity>, dependency: &ProofDependency) -> bool {
    let mut is_repeat = false;
    let mut index = 0;
    while !is_repeat && index < active.inline_len {
        is_repeat = slot_is_identity_of(&active.inline[index], dependency);
        index += 1;
    }
    let mut index = 0;
    while !is_repeat && index < active.spill.len() {
        is_repeat = is_identity_of(&active.spill[index], dependency);
        index += 1;
    }
    is_repeat
}
/// Whether an occupied inline slot holds the identity of `dependency`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn slot_is_identity_of(slot: &Option<DependencyIdentity>, dependency: &ProofDependency) -> bool {
    match slot {
        Some(ancestor) => is_identity_of(ancestor, dependency),
        None => false,
    }
}
/// Whether `ancestor` names the same module, version and declaration.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_identity_of(ancestor: &DependencyIdentity, dependency: &ProofDependency) -> bool {
    ancestor.module == dependency.module
        && ancestor.version == dependency.version
        && ancestor.name == dependency.declaration.name
}
fn check_contract<'m>(
    obligation: &ProofObligation,
    contract: &ContractGoal,
    mut meter: Meter<'m>,
) -> Transition<'m, CheckedProof> {
    let result = contract_proof(obligation, contract, &mut meter);
    (meter, result)
}
/// Check the selected unary I64 rule of a source proof against its MC1 contract.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the meter is the caller's owned transition state, written back through this borrow on every path"
)]
fn contract_proof(
    obligation: &ProofObligation,
    contract: &ContractGoal,
    meter: &mut Meter<'_>,
) -> Result<CheckedProof, Diagnostic> {
    advance!(*meter, meter.visit(0, obligation.span));
    let (_, literal) = prepare_checked_contract(contract, meter.limits)?;
    check_unary_true_rule(obligation, contract)?;
    let literal = literal.ok_or_else(|| {
        reject(
            obligation.span,
            "selected proof rules require exact literal then wrapping-add subject slices",
        )
    })?;
    if !matches_eq_add(
        &contract.ensures,
        &contract.inputs[0].0,
        &contract.outputs[0].0,
        literal,
    ) {
        return Err(reject(
            contract.ensures.span,
            "proof consequence does not match exact typed MC1 postcondition",
        ));
    }
    let root = operation(&obligation.term, "export-unary-I64", 2)?;
    let outer = operation(&root[0], "intro", 2)?;
    let tail_binder = list(&outer[0])?;
    if typed_binder_differs(tail_binder, "Stack")? {
        return Err(reject(outer[0].span, "expected explicit Stack tail binder"));
    }
    let tail_name = atom(&tail_binder[0])?;
    let inner = operation(&outer[1], "intro", 2)?;
    let x = list(&inner[0])?;
    if typed_binder_differs(x, "I64")? || atom(&x[0])? == tail_name {
        return Err(reject(
            inner[0].span,
            "expected distinct explicit I64 input binder",
        ));
    }
    let sequence = operation(&inner[1], "pc-sequence", 4)?;
    let exact1 = operation(&sequence[0], "pc-exact", 1)?;
    let lit = operation(&exact1[0], "exec-literal", 1)?;
    if atom(&lit[0])?.parse::<i64>().ok() != Some(literal) {
        return Err(reject(
            lit[0].span,
            "literal rule does not match first subject slice",
        ));
    }
    let exact2 = operation(&sequence[1], "pc-exact", 1)?;
    operation(&exact2[0], "exec-add", 0)?;
    let bridge = operation(&sequence[2], "bridge", 1)?;
    operation(&bridge[0], "by-exact-append-assoc", 0)?;
    let join = operation(&sequence[3], "join", 1)?;
    operation(&join[0], "by-exact-result", 0)?;
    operation(&root[1], "mc1-true-eq-wrap", 0)?;
    let lean_term = lower_unary_add(literal);
    advance!(*meter, meter.charge(lean_term.len(), obligation.span));
    Ok(CheckedProof {
        name: obligation.name.clone(),
        kind: ProofKind::Contract,
        lean_term,
        claim: "MC1Obligation.claim".into(),
        model_revision: String::new(),
        checker_revision: String::new(),
    })
}
/// Refuse a contract outside the selected unary I64 rule. An interface other
/// than one I64 input and one I64 output is an unsupported goal; a
/// precondition other than the atom `true` is a rejected proof.
fn check_unary_true_rule(
    obligation: &ProofObligation,
    contract: &ContractGoal,
) -> Result<(), Diagnostic> {
    let is_outside_rule = contract.inputs.len() != 1
        || contract.outputs.len() != 1
        || contract.inputs[0].1 != "I64"
        || contract.outputs[0].1 != "I64";
    if is_outside_rule {
        return Err(Diagnostic::new(
            DiagnosticKind::Unsupported,
            contract.contract_span,
            "selected unary I64 rule takes exactly one I64 input and one I64 output",
        ));
    }
    if atom(&contract.requires)? != "true" {
        return Err(reject(
            obligation.span,
            "selected unary I64 rule requires typed True precondition",
        ));
    }
    Ok(())
}
/// Whether `binder` is not a two-part binder whose sort is the atom `sort`;
/// refuses a sort that is not an atom.
fn typed_binder_differs(binder: &[Form], sort: &str) -> Result<bool, Diagnostic> {
    Ok(binder.len() != 2 || atom(&binder[1])? != sort)
}
enum Mc1Frame<'a> {
    Enter(&'a Form, u32),
    Children(&'a [Form], usize, u32),
}
fn mc1_expression<'m>(form: &Form, mut meter: Meter<'m>, depth: u32) -> Transition<'m, String> {
    let result = mc1_text(form, &mut meter, depth);
    (meter, result)
}
/// Render `form` as MC1 text in walk order, stopping at the first refusal.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the meter is the caller's owned transition state, written back through this borrow on every path"
)]
fn mc1_text(form: &Form, meter: &mut Meter<'_>, depth: u32) -> Result<String, Diagnostic> {
    let mut pending = WorkStack::new();
    let bound = meter.tree_bounds.frames;
    work_push!(pending, Mc1Frame::Enter(form, depth), bound).map_err(|()| exhausted(form.span))?;
    let mut text = String::new();
    let mut outcome = Ok(());
    while let Some(frame) = work_pop!(pending) {
        outcome = mc1_step(frame, &mut pending, &mut text, meter);
        if outcome.is_err() {
            break;
        }
    }
    outcome?;
    Ok(text)
}
/// Render one MC1 frame: a word, a list opening, or a list's next part.
#[expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; the pending stack, text and meter are the rendering's locally owned state, borrowed only for one loop-free step"
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the step appends to the rendering's owned text buffer, which must stay the caller's String"
)]
fn mc1_step<'a>(
    frame: Mc1Frame<'a>,
    pending: &mut WorkStack<Mc1Frame<'a>>,
    text: &mut String,
    meter: &mut Meter<'_>,
) -> Result<(), Diagnostic> {
    let bound = meter.tree_bounds.frames;
    match frame {
        Mc1Frame::Enter(node, depth) => {
            advance!(*meter, meter.visit(depth, node.span));
            match &node.kind {
                FormKind::Atom(word) => {
                    if word.is_empty() || !bytes_within(word, b"-_") {
                        return Err(reject(node.span, "invalid MC1 logical word"));
                    }
                    text.push_str(word);
                }
                FormKind::List(items) => {
                    text.push('(');
                    if items.is_empty() {
                        text.push(')');
                    } else {
                        work_push!(pending, Mc1Frame::Children(items, 0, depth), bound)
                            .map_err(|()| exhausted(node.span))?;
                    }
                }
            }
        }
        Mc1Frame::Children(items, index, depth) => {
            let Some(part) = items.get(index) else {
                text.push(')');
                return Ok(());
            };
            if index != 0 {
                text.push(' ');
            }
            let child_depth = deeper(depth, part.span)?;
            let next = index.checked_add(1).ok_or_else(|| exhausted(part.span))?;
            work_push!(pending, Mc1Frame::Children(items, next, depth), bound)
                .map_err(|()| exhausted(part.span))?;
            work_push!(pending, Mc1Frame::Enter(part, child_depth), bound)
                .map_err(|()| exhausted(part.span))?;
        }
    }
    Ok(())
}
/// Cross-checks the asserted source binding, reaccepted complete definition
/// recipe and MC1-v1 frontend. Host authority over these public fields must
/// come from the retained `ModuleSession`, not this consistency check.
pub fn prepare_contract(
    contract: &ContractGoal,
    limits: Limits,
) -> Result<crate::Prepared, Diagnostic> {
    prepare_checked_contract(contract, limits).map(|(prepared, _)| prepared)
}
#[expect(
    tigerstyle::function_length,
    reason = "Owner: noble-contracts; the single-pass dispatcher is kept linear so its stage order stays reviewable"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn prepare_checked_contract(
    contract: &ContractGoal,
    limits: Limits,
) -> Result<(crate::Prepared, Option<i64>), Diagnostic> {
    let subject = &contract.subject;
    let config = MeterConfig {
        budgets: ProofBudgets::from_limits(limits),
        limits,
        tree_bounds: TreeBounds::new(limits).ok_or_else(|| exhausted(contract.contract_span))?,
    };
    let mut meter = Meter {
        nodes: 0,
        work: 0,
        normalization: 0,
        substitution: 0,
        config: &config,
    };
    advance!(
        meter,
        meter.charge(
            subject
                .module_source
                .len()
                .saturating_add(subject.definition_source.len()),
            contract.contract_span
        )
    );
    if !is_own_module_origin(subject) {
        return Err(reject(
            subject.source_span,
            "subject source dependency differs from original module",
        ));
    }
    if subject.effects != noble_kernel::types::EffSet::empty() {
        return Err(reject(
            subject.source_span,
            "MC1 contract subject has resource effects",
        ));
    }
    let accepted = &subject.accepted_submission;
    let noble_kernel::untrusted::Outcome::Accepted(interface) = noble_kernel::acceptance::check(
        &accepted.environment,
        &accepted.request,
        &accepted.body.candidate,
    ) else {
        return Err(reject(
            subject.source_span,
            "subject candidate is not independently accepted",
        ));
    };
    if interface_differs(&interface.interface, subject) {
        return Err(reject(
            subject.source_span,
            "accepted subject interface differs from contract",
        ));
    }
    let Some(position) = identity_position(&accepted.definitions, subject.definition_identity, 0)
    else {
        return Err(reject(
            subject.source_span,
            "resolved subject identity absent from accepted recipe",
        ));
    };
    let definition = &accepted.definitions[position];
    if identity_position(
        &accepted.definitions,
        subject.definition_identity,
        position.saturating_add(1),
    )
    .is_some()
        || accepted.definitions.len() != 1
    {
        return Err(reject(
            subject.source_span,
            "ambiguous resolved subject identity",
        ));
    }
    let slot = usize::try_from(definition.definition.0).map_err(|_| {
        reject(
            subject.source_span,
            "subject definition slot exceeds address space",
        )
    })?;
    if invocation_differs(subject, definition, slot) {
        return Err(reject(
            subject.source_span,
            "resolved subject owner or invocation differs from accepted recipe",
        ));
    }
    if expected_differs(&definition.expected, subject) {
        return Err(reject(
            subject.source_span,
            "subject definition has different stack or effect bound",
        ));
    }
    let body_span = crate::source::verify_definition_origin(
        crate::source::DefinitionOrigin {
            module_source: &subject.module_source,
            module: &subject.module,
            version: subject.version,
            definition: &subject.definition,
            ordinal: subject.definition_ordinal,
            span: subject.source_span,
        },
        &subject.definition_source,
        limits,
    )?;
    let start = usize::try_from(body_span.start)
        .map_err(|_| reject(body_span, "subject body exceeds address space"))?;
    let end = usize::try_from(body_span.end)
        .map_err(|_| reject(body_span, "subject body exceeds address space"))?;
    let body = subject
        .module_source
        .get(start..end)
        .ok_or_else(|| reject(body_span, "subject body is outside original module source"))?;
    let body = core::str::from_utf8(body)
        .map_err(|_| reject(subject.source_span, "subject body must be UTF-8 for MC1"))?;
    let requires = advance!(meter, mc1_expression(&contract.requires, meter, 0));
    let ensures = advance!(meter, mc1_expression(&contract.ensures, meter, 0));
    let text = contract_text(contract, body, &requires, &ensures);
    let (_, charged) = meter.charge(text.len(), contract.contract_span);
    charged?;
    let result = crate::prepare(text.as_bytes(), limits)?;
    if typed_contract_differs(&result, contract) {
        return Err(reject(
            subject.source_span,
            "MC1 typed contract differs from accepted subject interface",
        ));
    }
    // The standalone MC1 program must be the accepted definition's complete
    // operation list. Any unresolved named call or different recipe is refused.
    let expected = &definition.body.candidate;
    let actual = result.candidate();
    if recipe_shape_differs(definition, actual, limits) {
        return Err(reject(
            subject.source_span,
            "MC1 program differs from resolved subject recipe",
        ));
    }
    let mut index = 0;
    let mut operations = Ok(());
    while operations.is_ok() && index < expected.body.len() && index < actual.body.len() {
        operations = same_subject_operation(expected, actual, index, subject.source_span);
        index += 1;
    }
    operations?;
    let request = noble_kernel::untrusted::Request {
        input_bytes: accepted.request.input_bytes,
        expected: definition.expected.clone(),
        limits: accepted.request.limits,
    };
    let noble_kernel::untrusted::Outcome::Accepted(checked_definition) =
        noble_kernel::acceptance::check(&accepted.environment, &request, expected)
    else {
        return Err(reject(
            subject.source_span,
            "subject definition body is not independently accepted",
        ));
    };
    if interface_differs(&checked_definition.interface, subject) {
        return Err(reject(
            subject.source_span,
            "accepted definition body differs from contract interface",
        ));
    }
    if checked_definition.derivations.len() != result.checked().derivations.len()
        || !derivations_agree(
            &checked_definition.derivations,
            &result.checked().derivations,
        )
    {
        return Err(reject(
            subject.source_span,
            "MC1 node derivations differ from accepted definition body",
        ));
    }
    let literal = unary_literal(expected, subject);
    if literal.is_some() && builtin_add_differs(&accepted.environment, slot) {
        return Err(reject(
            subject.source_span,
            "subject addition is not the fixed MC1 builtin",
        ));
    }
    Ok((result, literal))
}
/// The standalone MC1 contract text of `contract`: its name, its input and
/// output bindings, then the rendered body, precondition and postcondition.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the texts are the contract's rendered clauses in print order, pinned by unit tests against the format! text this replaced"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; this total text builder only concatenates already checked parts and has no precondition to assert"
)]
fn contract_text(
    contract: &ContractGoal,
    body: &str,
    requires_text: &str,
    ensures_text: &str,
) -> String {
    let mut text = String::from("(contract 1 ");
    text.push_str(&contract.contract_name);
    text.push_str(" (input");
    for (name, ty) in &contract.inputs {
        text = contract_binding(text, name, ty);
    }
    text.push_str(") (output");
    for (name, ty) in &contract.outputs {
        text = contract_binding(text, name, ty);
    }
    text.push_str(") (program ");
    text.push_str(body);
    text.push_str(") (requires ");
    text.push_str(requires_text);
    text.push_str(") (ensures ");
    text.push_str(ensures_text);
    text.push_str("))");
    text
}
/// `text` with one MC1 contract binding ` ({name} {ty})` appended.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the binding's name and type are appended in print order, pinned by unit tests against the format! text this replaced"
)]
fn contract_binding(mut text: String, name: &str, ty: &str) -> String {
    text.push_str(" (");
    text.push_str(name);
    text.push(' ');
    text.push_str(ty);
    text.push(')');
    text
}
/// Whether an accepted interface differs from the subject's stacks or effects.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn interface_differs(
    interface: &noble_kernel::untrusted::Interface,
    subject: &SubjectBinding,
) -> bool {
    interface.stack_in != subject.input_types
        || interface.stack_out != subject.output_types
        || interface.effects != subject.effects
}
/// Whether a definition's expected bound differs from the subject's stacks or
/// effects.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn expected_differs(
    expected: &noble_kernel::untrusted::Expected,
    subject: &SubjectBinding,
) -> bool {
    expected.stack_in != subject.input_types
        || expected.stack_out != subject.output_types
        || expected.allowed_effects != subject.effects
}
/// Whether the accepted recipe is not one plain invocation of `definition`,
/// whose slot `slot` the subject's owner holds as a named definition.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn invocation_differs(
    subject: &SubjectBinding,
    definition: &noble_kernel::execution::Definition,
    slot: usize,
) -> bool {
    let accepted = &subject.accepted_submission;
    accepted.environment.definition_owners.get(slot) != Some(&Some(subject.definition_owner))
        || accepted.environment.kind(definition.definition)
            != Some(noble_kernel::contracts::Behavior::Named)
        || !is_single_invocation(&accepted.body.candidate, definition.definition)
        || accepted.body.candidate.nodes.len() != 1
        || !accepted.body.texts.is_empty()
}
/// Whether the prepared MC1 contract's arity or checked interface differs
/// from the contract and its subject.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn typed_contract_differs(result: &crate::Prepared, contract: &ContractGoal) -> bool {
    result.inputs().len() != contract.inputs.len()
        || result.outputs().len() != contract.outputs.len()
        || interface_differs(&result.checked().interface, &contract.subject)
}
/// Whether the MC1 program `actual` differs in shape from the resolved
/// subject definition's recipe, or that recipe exceeds the node limit.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn recipe_shape_differs(
    definition: &noble_kernel::execution::Definition,
    actual: &noble_kernel::untrusted::Candidate,
    limits: Limits,
) -> bool {
    let expected = &definition.body.candidate;
    !definition.body.texts.is_empty()
        || expected.format != actual.format
        || expected.revision != actual.revision
        || expected.body.len() != actual.body.len()
        || expected.nodes.len() != actual.nodes.len()
        || expected.nodes.len() != expected.body.len()
        || u32::try_from(expected.body.len()).map_or(true, |count| count > limits.nodes)
}
/// Whether the environment's addition at `slot` is not the fixed MC1 builtin.
fn builtin_add_differs(environment: &noble_kernel::contracts::Env, slot: usize) -> bool {
    !canonical_wrapping_add(environment)
        || !deps_are(
            &environment.deps,
            slot,
            &[noble_kernel::contracts::Definition(4)],
        )
}
/// The literal of a literal-then-add recipe that visits its two nodes in
/// order for a unary I64 subject; the guarded slice pattern this replaces.
fn unary_literal(
    expected: &noble_kernel::untrusted::Candidate,
    subject: &SubjectBinding,
) -> Option<i64> {
    let value = literal_then_add(&expected.nodes)?;
    if is_unary_literal_add(expected, subject) {
        Some(value)
    } else {
        None
    }
}
/// Whether a literal-then-add recipe visits its two nodes in order for a
/// unary I64 subject.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_unary_literal_add(
    expected: &noble_kernel::untrusted::Candidate,
    subject: &SubjectBinding,
) -> bool {
    expected.body.as_slice()
        == [
            noble_kernel::untrusted::NodeId(0),
            noble_kernel::untrusted::NodeId(1),
        ]
        && subject.input_types == [noble_kernel::types::Ty::I64]
        && subject.output_types == [noble_kernel::types::Ty::I64]
}
/// Whether `subject` records exactly one source dependency, its own original
/// module, as the slice pattern `[origin]` with that guard.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_own_module_origin(subject: &SubjectBinding) -> bool {
    if subject.source_dependencies.len() != 1 {
        return false;
    }
    let origin = &subject.source_dependencies[0];
    origin.module == subject.module
        && origin.version == subject.version
        && origin.full_source == subject.module_source
}
/// The first definition at or after `from` with `identity`, as the next item
/// of `definitions.iter().filter(..)`.
fn identity_position(
    definitions: &[noble_kernel::execution::Definition],
    identity: u64,
    from: usize,
) -> Option<usize> {
    let mut index = from;
    let mut found = None;
    while found.is_none() && index < definitions.len() {
        if definitions[index].identity == identity {
            found = Some(index);
        }
        index += 1;
    }
    found
}
/// The node an occurrence names, as
/// `usize::try_from(id.0).ok().and_then(|index| nodes.get(index))`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn candidate_node(
    nodes: &[noble_kernel::untrusted::Node],
    id: noble_kernel::untrusted::NodeId,
) -> Option<&noble_kernel::untrusted::Node> {
    match usize::try_from(id.0) {
        Ok(index) => nodes.get(index),
        Err(_) => None,
    }
}
/// Whether `candidate`'s body is one occurrence of a node invoking `definition`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn is_single_invocation(
    candidate: &noble_kernel::untrusted::Candidate,
    definition: noble_kernel::contracts::Definition,
) -> bool {
    if candidate.body.len() != 1 {
        return false;
    }
    matches!(
        candidate_node(&candidate.nodes, candidate.body[0]),
        Some(noble_kernel::untrusted::Node::Invocation { def, .. }) if *def == definition
    )
}
/// Refuse the `index`-th body occurrence unless the accepted subject and the
/// MC1 program hold the same literal or invocation with the same `Inst` there.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn same_subject_operation(
    expected: &noble_kernel::untrusted::Candidate,
    actual: &noble_kernel::untrusted::Candidate,
    index: usize,
    span: Span,
) -> Result<(), Diagnostic> {
    let Some(left) = candidate_node(&expected.nodes, expected.body[index]) else {
        return Err(reject(span, "invalid accepted subject node"));
    };
    let Some(right) = candidate_node(&actual.nodes, actual.body[index]) else {
        return Err(reject(span, "invalid MC1 subject node"));
    };
    match (left, right) {
        (
            noble_kernel::untrusted::Node::Literal { lit: a, inst: ia },
            noble_kernel::untrusted::Node::Literal { lit: b, inst: ib },
        ) if a == b && same_inst(ia, ib) => Ok(()),
        (
            noble_kernel::untrusted::Node::Invocation { def: a, inst: ia },
            noble_kernel::untrusted::Node::Invocation { def: b, inst: ib },
        ) if a == b && same_inst(ia, ib) => Ok(()),
        _ => Err(reject(
            span,
            "MC1 program differs from resolved subject operation",
        )),
    }
}
/// Pairwise agreement of two derivation lists in order, as
/// `left.iter().zip(right).all(..)` over `derivation_agrees`.
fn derivations_agree(
    left: &[noble_kernel::untrusted::Derivation],
    right: &[noble_kernel::untrusted::Derivation],
) -> bool {
    let mut index = 0;
    let mut is_agreed = true;
    while is_agreed && index < left.len() && index < right.len() {
        is_agreed = derivation_agrees(&left[index], &right[index]);
        index += 1;
    }
    is_agreed
}
/// Whether two derivations name the same node with the same interface.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn derivation_agrees(
    left: &noble_kernel::untrusted::Derivation,
    right: &noble_kernel::untrusted::Derivation,
) -> bool {
    left.node == right.node
        && left.interface.stack_in == right.interface.stack_in
        && left.interface.stack_out == right.interface.stack_out
        && left.interface.effects == right.interface.effects
}
/// The literal of a `[literal, builtin 4]` node list, as the slice pattern
/// `[Literal { lit: I64(value), .. }, Invocation { def: Definition(4), .. }]`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn literal_then_add(nodes: &[noble_kernel::untrusted::Node]) -> Option<i64> {
    if nodes.len() != 2 {
        return None;
    }
    match (&nodes[0], &nodes[1]) {
        (
            noble_kernel::untrusted::Node::Literal {
                lit: noble_kernel::untrusted::Lit::I64(value),
                ..
            },
            noble_kernel::untrusted::Node::Invocation {
                def: noble_kernel::contracts::Definition(4),
                ..
            },
        ) => Some(*value),
        _ => None,
    }
}
/// Whether the definition at `slot` depends on exactly `expected`, as
/// `deps.get(slot).map(Vec::as_slice) == Some(expected)`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn deps_are(
    deps: &[Vec<noble_kernel::contracts::Definition>],
    slot: usize,
    expected: &[noble_kernel::contracts::Definition],
) -> bool {
    match deps.get(slot) {
        Some(found) => found.as_slice() == expected,
        None => false,
    }
}

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; this stage imports the domain types it operates on directly, matching the module vocabulary"
)]
pub(crate) fn canonical_wrapping_add(env: &noble_kernel::contracts::Env) -> bool {
    use noble_kernel::{
        contracts::{Behavior, Definition},
        words::VariableKind,
    };
    let Some(scheme) = env.scheme(Definition(4)) else {
        return false;
    };
    env.kind(Definition(4)) == Some(Behavior::Arith)
        && env.definition_owners.get(4) == Some(&None)
        && deps_are(&env.deps, 4, &[])
        && scheme.var_kinds == [VariableKind::Stack]
        && is_stack_then_i64s(&scheme.stack_in, 2)
        && is_stack_then_i64s(&scheme.stack_out, 1)
        && scheme.effects.is_empty()
}
/// Whether `patterns` is `StackVar(Variable(0))` followed by exactly `i64s`
/// `I64` patterns, as the slice patterns of `canonical_wrapping_add` did.
fn is_stack_then_i64s(patterns: &[noble_kernel::shapes::Pattern], i64s: usize) -> bool {
    let Some((first, rest)) = patterns.split_first() else {
        return false;
    };
    let mut is_shape = rest.len() == i64s && is_stack_zero(first);
    let mut index = 0;
    while is_shape && index < rest.len() {
        is_shape = is_i64_pattern(&rest[index]);
        index += 1;
    }
    is_shape
}
/// Whether `pattern` is the stack variable 0.
fn is_stack_zero(pattern: &noble_kernel::shapes::Pattern) -> bool {
    matches!(
        pattern,
        noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0))
    )
}
/// Whether `pattern` is `I64`.
fn is_i64_pattern(pattern: &noble_kernel::shapes::Pattern) -> bool {
    matches!(pattern, noble_kernel::shapes::Pattern::I64)
}

/// Equal binding counts and pairwise equal bindings in order, as
/// `a.bindings.iter().zip(&b.bindings).all(..)` after the length check.
pub(crate) fn same_inst(a: &noble_kernel::words::Inst, b: &noble_kernel::words::Inst) -> bool {
    let mut is_same = a.bindings.len() == b.bindings.len();
    let mut index = 0;
    while is_same && index < a.bindings.len() {
        is_same = same_binding(&a.bindings[index], &b.bindings[index]);
        index += 1;
    }
    is_same
}
/// Whether two bindings are the same variant with equal contents.
#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
fn same_binding(a: &noble_kernel::words::Binding, b: &noble_kernel::words::Binding) -> bool {
    use noble_kernel::words::Binding;
    match (a, b) {
        (Binding::Stack(a), Binding::Stack(b)) => a == b,
        (Binding::Value(a), Binding::Value(b)) => a == b,
        (Binding::Effect(a), Binding::Effect(b)) => a == b,
        (Binding::Ref(a), Binding::Ref(b)) => a == b,
        _ => false,
    }
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper reads runtime tables or calls non-const helpers, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
fn matches_eq_add(form: &Form, input_name: &str, output_name: &str, literal: i64) -> bool {
    let Ok(eq) = operation(form, "eq", 2) else {
        return false;
    };
    let Ok(out) = operation(&eq[0], "out", 1) else {
        return false;
    };
    let Ok(add) = operation(&eq[1], "add", 2) else {
        return false;
    };
    let Ok(input) = operation(&add[0], "in", 1) else {
        return false;
    };
    atom(&out[0]).ok() == Some(output_name)
        && atom(&input[0]).ok() == Some(input_name)
        && atom(&add[1]).ok().and_then(|word| word.parse::<i64>().ok()) == Some(literal)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
/// Expansion of the checked constructors in `check_contract`. Every instance
/// of `__N__` is sourced from the independently accepted literal node. These
/// are actual PC introduction/elimination terms, not a selected theorem name.
/// The term's non-ASCII Lean symbols are spelled as ASCII markers in the
/// template and substituted afterwards: `__AND__` is U+2227, `__LANGLE__` and
/// `__RANGLE__` are U+27E8 and U+27E9, and `__REWRITE__` is U+25B8.
fn lower_unary_add(literal: i64) -> String {
    const TERM: &str = r#"
(exportedClaim_consequence
  (exportedClaim_unaryI64
    (code := [.lit (.i64 __N__)] ++ [.word 4])
    (f := fun x => x + (__N__ : BitVec 64))
    (fun tail x final execution =>
      let first : PC [.lit (.i64 __N__)]
          (fun before => before = tail ++ [.i64 x])
          (fun _ middle => middle = (tail ++ [.i64 x]) ++ [.i64 __N__]) :=
        pc_exact (code := [.lit (.i64 __N__)]) (s := tail ++ [.i64 x])
          (expected := (tail ++ [.i64 x]) ++ [.i64 __N__])
          (fun actual => exec_lit_iff (v := .i64 __N__)
            (s := tail ++ [.i64 x]) (t := actual))
      let second : PC [.word 4]
          (fun middle => middle = tail ++ [.i64 x, .i64 __N__])
          (fun _ actual => actual = tail ++ [.i64 (x + (__N__ : BitVec 64))]) :=
        pc_exact (code := [.word 4]) (s := tail ++ [.i64 x, .i64 __N__])
          (expected := tail ++ [.i64 (x + (__N__ : BitVec 64))])
          (fun actual => exec_add_iff (a := x) (b := __N__)
            (s := tail) (t := actual))
      let composed : PC ([.lit (.i64 __N__)] ++ [.word 4])
          (fun before => before = tail ++ [.i64 x])
          (fun before actual => before = tail ++ [.i64 x] __AND__
            actual = tail ++ [.i64 (x + (__N__ : BitVec 64))]) :=
        @pc_sequence
          [.lit (.i64 __N__)] [.word 4]
          (fun (before : Stack) => before = tail ++ [.i64 x])
          (fun (_ middle : Stack) => middle = (tail ++ [.i64 x]) ++ [.i64 __N__])
          (fun (middle : Stack) => middle = tail ++ [.i64 x, .i64 __N__])
          (fun (_ actual : Stack) => actual = tail ++ [.i64 (x + (__N__ : BitVec 64))])
          (fun (before actual : Stack) => before = tail ++ [.i64 x] __AND__
            actual = tail ++ [.i64 (x + (__N__ : BitVec 64))])
          first second
          (fun _ middle _ exactMiddle =>
            exactMiddle.trans (List.append_assoc tail [.i64 x] [.i64 __N__]))
          (fun _ _ _ initial _ exactFinal => __LANGLE__initial, exactFinal__RANGLE__)
      ((composed (tail ++ [.i64 x]) final rfl execution).2)))
  (fun _ _ _ => True.intro)
  (fun before after params _ exactPost =>
    match exactPost with
    | __LANGLE__x, hbefore, hafter__RANGLE__ =>
      hbefore.symm __REWRITE__ hafter.symm __REWRITE__
      (holds_eq (.output 0) (.add (.input 0) (.i64 __N__))
        __LANGLE__[.i64 x], [.i64 (x + (__N__ : BitVec 64))], params__RANGLE__
        (.i64 (x + (__N__ : BitVec 64))) (.i64 (x + (__N__ : BitVec 64)))
        rfl rfl).mpr rfl))
"#;
    let mut digits = String::new();
    crate::rendering::integer(&mut digits, literal);
    let term = replace_all(TERM, "__N__", &digits);
    let term = replace_all(&term, "__AND__", &push_code_point(String::new(), 0x2227));
    let term = replace_all(&term, "__LANGLE__", &push_code_point(String::new(), 0x27E8));
    let term = replace_all(&term, "__RANGLE__", &push_code_point(String::new(), 0x27E9));
    replace_all(
        &term,
        "__REWRITE__",
        &push_code_point(String::new(), 0x25B8),
    )
}
/// `text.replace(marker, value)` for a non-empty `marker`: each match, found
/// left to right without overlap, becomes `value`.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; the same-typed parameters are positional by contract and named at every call site"
)]
fn replace_all(text: &str, marker: &str, value: &str) -> String {
    debug_assert!(!marker.is_empty());
    let haystack = text.as_bytes();
    let needle = marker.as_bytes();
    let mut replaced = String::new();
    let mut copied = 0;
    let mut index = 0;
    while index < haystack.len() {
        if haystack[index..].starts_with(needle) {
            // A match of whole UTF-8 sequences starts and ends on character
            // boundaries, so both slices below exist.
            replaced.push_str(&text[copied..index]);
            replaced.push_str(value);
            index += needle.len();
            copied = index;
        } else {
            index += 1;
        }
    }
    replaced.push_str(&text[copied..]);
    replaced
}

#[cfg(test)]
mod form_impl_tests {
    //! The hand-written `Form`/`FormKind` and `ProofDependency` impls must agree
    //! exactly with the derived impls they replace; `mirror` keeps those derives
    //! for comparison.
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
    )]
    use super::{Form, FormKind};
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
    )]
    use alloc::{format, string::String, vec, vec::Vec};

    mod mirror {
        #[derive(Clone, Debug, PartialEq, Eq)]
        #[expect(
            tigerstyle::path_segment_repetition,
            reason = "Owner: noble-contracts; the mirror keeps the replaced type's own name, so its derived Debug output is the reference rendering"
        )]
        pub struct Form {
            pub kind: FormKind,
            pub span: crate::Span,
        }
        #[derive(Clone, Debug, PartialEq, Eq)]
        #[expect(
            tigerstyle::path_segment_repetition,
            reason = "Owner: noble-contracts; the mirror keeps the replaced type's own name, so its derived Debug output is the reference rendering"
        )]
        pub enum FormKind {
            Atom(alloc::string::String),
            List(alloc::vec::Vec<Form>),
        }
        #[derive(Clone, Debug)]
        #[expect(
            dead_code,
            reason = "Owner: noble-contracts; the mirror's fields are read only by its derived Debug, which is the reference rendering"
        )]
        pub struct ProofDependency {
            pub(super) name: alloc::string::String,
            pub(super) module: alloc::string::String,
            pub(super) version: u32,
            pub(super) source: alloc::vec::Vec<u8>,
            pub(super) declaration: crate::intrinsic::ProofObligation,
            pub(super) exported: bool,
            pub(super) dependencies: alloc::vec::Vec<ProofDependency>,
            pub(super) model_revision: alloc::string::String,
            pub(super) checker_revision: alloc::string::String,
        }
    }

    fn atom(text: &str, start: u32) -> Form {
        Form {
            kind: FormKind::Atom(String::from(text)),
            span: crate::Span {
                start,
                end: start.saturating_add(1),
            },
        }
    }

    fn list(forms: Vec<Form>, start: u32) -> Form {
        Form {
            kind: FormKind::List(forms),
            span: crate::Span {
                start,
                end: start.saturating_add(10),
            },
        }
    }

    fn mirrored(form: &Form) -> mirror::Form {
        mirror::Form {
            kind: match &form.kind {
                FormKind::Atom(text) => mirror::FormKind::Atom(text.clone()),
                FormKind::List(forms) => {
                    mirror::FormKind::List(forms.iter().map(mirrored).collect())
                }
            },
            span: form.span,
        }
    }

    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; the sample table is data, checked by the tests that consume it"
    )]
    fn samples() -> Vec<Form> {
        let mut forms = vec![
            atom("a", 0),
            atom("a\"\n", 0),
            atom("a", 1),
            list(Vec::new(), 0),
            list(vec![atom("a", 0)], 0),
            list(vec![atom("a", 0), list(vec![atom("b", 2)], 1)], 0),
            list(vec![atom("a", 0), list(vec![atom("b", 2)], 1)], 3),
            list(vec![atom("a", 0), list(vec![atom("c", 2)], 1)], 0),
        ];
        // Deeper lists, with forms after nested lists and an empty nested
        // list, differing only deep inside: in an atom, a span or a length.
        for (deep, deep_start) in [("d", 4), ("x", 4), ("d", 5)] {
            forms.push(list(
                vec![
                    list(
                        vec![
                            list(vec![atom(deep, deep_start)], 3),
                            atom("e", 5),
                            list(Vec::new(), 6),
                        ],
                        2,
                    ),
                    atom("f", 7),
                ],
                1,
            ));
        }
        forms.push(list(
            vec![
                list(
                    vec![
                        list(vec![atom("d", 4), atom("d", 4)], 3),
                        atom("e", 5),
                        list(Vec::new(), 6),
                    ],
                    2,
                ),
                atom("f", 7),
            ],
            1,
        ));
        let mut chain = atom("g", 0);
        for start in 0..64 {
            chain = list(vec![chain, atom("h", start)], start);
        }
        forms.push(chain);
        forms
    }

    #[test]
    fn debug_matches_the_derived_rendering() {
        for form in samples() {
            let expected = mirrored(&form);
            assert_eq!(format!("{form:?}"), format!("{expected:?}"));
            assert_eq!(format!("{form:#?}"), format!("{expected:#?}"));
            assert_eq!(format!("{form:4?}"), format!("{expected:4?}"));
            assert_eq!(format!("{:?}", form.kind), format!("{:?}", expected.kind));
            assert_eq!(format!("{:#?}", form.kind), format!("{:#?}", expected.kind));
        }
    }

    #[test]
    fn equality_matches_the_derived_relation() {
        let forms = samples();
        for left in &forms {
            for right in &forms {
                let is_expected = mirrored(left) == mirrored(right);
                assert_eq!(left == right, is_expected);
                assert_eq!(left != right, !is_expected);
                assert_eq!(
                    left.kind == right.kind,
                    mirrored(left).kind == mirrored(right).kind
                );
            }
        }
    }

    #[test]
    fn clone_is_an_equal_independent_deep_copy() {
        for form in samples() {
            let mut copy = form.clone();
            assert_eq!(mirrored(&copy), mirrored(&form));
            assert_eq!(copy.kind.clone(), form.kind);
            if let FormKind::List(items) = &mut copy.kind {
                items.push(atom("z", 9));
                assert_ne!(copy, form);
            }
        }
    }

    fn mirrored_dependency(dependency: &super::ProofDependency) -> mirror::ProofDependency {
        mirror::ProofDependency {
            name: dependency.name.clone(),
            module: dependency.module.clone(),
            version: dependency.version,
            source: dependency.source.clone(),
            declaration: dependency.declaration.clone(),
            exported: dependency.exported,
            dependencies: dependency
                .dependencies
                .iter()
                .map(mirrored_dependency)
                .collect(),
            model_revision: dependency.model_revision.clone(),
            checker_revision: dependency.checker_revision.clone(),
        }
    }

    fn dependency(name: &str, dependencies: Vec<super::ProofDependency>) -> super::ProofDependency {
        super::ProofDependency {
            name: String::from(name),
            module: String::from("Module"),
            version: 3,
            source: name.as_bytes().to_vec(),
            declaration: super::ProofObligation {
                name: String::from(name),
                revision: 1,
                goal: super::PendingGoal::Pure {
                    proposition: atom("p", 1),
                },
                term: list(vec![atom("t", 2)], 2),
                span: crate::Span { start: 0, end: 4 },
                elaboration: false,
            },
            exported: name.len().is_multiple_of(2),
            dependencies,
            model_revision: String::from("model \"r\"\n"),
            checker_revision: String::from("checker"),
        }
    }

    fn dependency_samples() -> Vec<super::ProofDependency> {
        vec![
            dependency("leaf", Vec::new()),
            dependency(
                "a",
                vec![
                    dependency("b", Vec::new()),
                    dependency("c", vec![dependency("d", Vec::new())]),
                    dependency("e", Vec::new()),
                ],
            ),
            dependency(
                "deep",
                vec![dependency(
                    "x",
                    vec![dependency("y", vec![dependency("z", Vec::new())])],
                )],
            ),
        ]
    }

    #[test]
    fn dependency_debug_matches_the_derived_rendering() {
        for dependency in dependency_samples() {
            let expected = mirrored_dependency(&dependency);
            assert_eq!(format!("{dependency:?}"), format!("{expected:?}"));
            assert_eq!(format!("{dependency:#?}"), format!("{expected:#?}"));
        }
    }

    #[test]
    fn dependency_clone_is_an_equal_independent_deep_copy() {
        for dependency in dependency_samples() {
            let mut copy = dependency.clone();
            assert_eq!(
                format!("{:?}", mirrored_dependency(&copy)),
                format!("{:?}", mirrored_dependency(&dependency))
            );
            copy.dependencies.push(dependency.clone());
            assert_ne!(format!("{copy:?}"), format!("{dependency:?}"));
        }
    }
}

#[cfg(test)]
mod proposition_impl_tests {
    //! The hand-written `Sort`/`Proposition` copy and equality must agree
    //! exactly with the derived impls they replace; `mirror` keeps the derives.
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
    )]
    use super::{Code, Proposition, Sort};
    #[expect(
        tigerstyle::non_trait_imports,
        reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
    )]
    use alloc::{boxed::Box, vec, vec::Vec};

    mod mirror {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum Sort {
            Codes,
            Value(super::Code),
            Premise(alloc::boxed::Box<Proposition>),
        }
        #[derive(Clone, Debug, PartialEq, Eq)]
        #[expect(
            tigerstyle::path_segment_repetition,
            reason = "Owner: noble-contracts; the mirror keeps the replaced type's own name, so it reads as the derived reference"
        )]
        pub enum Proposition {
            Pi(usize, Sort, alloc::boxed::Box<Proposition>),
            Eq(super::Code, usize, usize),
        }
    }

    fn mirrored_sort(sort: &Sort) -> mirror::Sort {
        match sort {
            Sort::Codes => mirror::Sort::Codes,
            Sort::Value(code) => mirror::Sort::Value(code.clone()),
            Sort::Premise(prop) => mirror::Sort::Premise(Box::new(mirrored(prop))),
        }
    }

    #[expect(
        tigerstyle::no_recursion,
        reason = "Owner: noble-contracts; the test mirror follows the finite sample propositions"
    )]
    fn mirrored(prop: &Proposition) -> mirror::Proposition {
        match prop {
            Proposition::Pi(index, sort, body) => {
                mirror::Proposition::Pi(*index, mirrored_sort(sort), Box::new(mirrored(body)))
            }
            Proposition::Eq(code, left, right) => {
                mirror::Proposition::Eq(code.clone(), *left, *right)
            }
        }
    }

    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; the sample table is data, checked by the tests that consume it"
    )]
    fn samples() -> Vec<Proposition> {
        let eq = |code: Code, left, right| Proposition::Eq(code, left, right);
        vec![
            eq(Code::I64, 0, 0),
            eq(Code::I64, 0, 1),
            eq(Code::Bool, 0, 0),
            eq(
                Code::Pair(Box::new(Code::I64), Box::new(Code::Variable(2))),
                1,
                0,
            ),
            Proposition::Pi(0, Sort::Codes, Box::new(eq(Code::Unit, 0, 0))),
            Proposition::Pi(1, Sort::Codes, Box::new(eq(Code::Unit, 0, 0))),
            Proposition::Pi(0, Sort::Value(Code::Text), Box::new(eq(Code::Unit, 0, 0))),
            Proposition::Pi(
                0,
                Sort::Premise(Box::new(eq(Code::I64, 1, 1))),
                Box::new(eq(Code::Unit, 0, 0)),
            ),
            Proposition::Pi(
                0,
                Sort::Premise(Box::new(eq(Code::I64, 1, 2))),
                Box::new(eq(Code::Unit, 0, 0)),
            ),
            Proposition::Pi(
                0,
                Sort::Premise(Box::new(Proposition::Pi(
                    3,
                    Sort::Codes,
                    Box::new(eq(Code::List(Box::new(Code::I64)), 0, 0)),
                ))),
                Box::new(eq(Code::Unit, 0, 0)),
            ),
        ]
    }

    #[test]
    fn equality_matches_the_derived_relation() {
        let props = samples();
        for left in &props {
            for right in &props {
                let is_expected = mirrored(left) == mirrored(right);
                assert_eq!(left == right, is_expected);
                assert_eq!(left != right, !is_expected);
            }
        }
        let sorts = [
            Sort::Codes,
            Sort::Value(Code::I64),
            Sort::Value(Code::Bool),
            Sort::Premise(Box::new(Proposition::Eq(Code::I64, 0, 0))),
        ];
        for left in &sorts {
            for right in &sorts {
                assert_eq!(left == right, mirrored_sort(left) == mirrored_sort(right));
            }
        }
    }

    #[test]
    fn clone_is_an_equal_deep_copy() {
        for prop in samples() {
            let copy = prop.clone();
            assert_eq!(mirrored(&copy), mirrored(&prop));
            if let Proposition::Pi(_, sort, _) = &prop {
                let sort_copy = sort.clone();
                assert_eq!(mirrored_sort(&sort_copy), mirrored_sort(sort));
            }
        }
    }

    /// An atom spanning the one pre-order position `start`.
    fn word(text: &str, start: u32) -> super::Form {
        super::Form {
            kind: super::FormKind::Atom(alloc::string::String::from(text)),
            span: crate::Span {
                start,
                end: start.saturating_add(1),
            },
        }
    }

    /// A list spanning the pre-order positions `start..end`.
    fn forms(items: Vec<super::Form>, (start, end): (u32, u32)) -> super::Form {
        super::Form {
            kind: super::FormKind::List(items),
            span: crate::Span { start, end },
        }
    }

    /// `(Pi (x I64) (Eq I64 x x))`, numbered 0..10 in pre-order.
    fn reflexive_goal() -> super::Form {
        let binder = forms(vec![word("x", 3), word("I64", 4)], (2, 5));
        let body = vec![word("Eq", 6), word("I64", 7), word("x", 8), word("x", 9)];
        forms(vec![word("Pi", 1), binder, forms(body, (5, 10))], (0, 10))
    }

    /// `(intro (x I64) (refl <name>))`, numbered 10..18 in pre-order.
    fn reflexive_term(name: &str) -> super::Form {
        let binder = forms(vec![word("x", 13), word("I64", 14)], (12, 15));
        let refl = forms(vec![word("refl", 16), word(name, 17)], (15, 18));
        forms(vec![word("intro", 11), binder, refl], (10, 18))
    }

    /// One pure obligation `p` proving `reflexive_goal` by `term`.
    #[expect(
        tigerstyle::assertion_density,
        reason = "Owner: noble-contracts; the fixture builder only constructs a proof batch; the tests that consume it assert on the checker's outcome"
    )]
    fn pure_batch(term: super::Form) -> super::ProofBatch {
        let proposition = reflexive_goal();
        super::ProofBatch {
            module: alloc::string::String::from("m"),
            version: 1,
            source: vec![b's'; 10],
            generation: 0,
            dependencies: Vec::new(),
            obligations: vec![super::ProofObligation {
                name: alloc::string::String::from("p"),
                revision: 1,
                goal: super::PendingGoal::Pure { proposition },
                term,
                span: crate::Span { start: 0, end: 18 },
                elaboration: false,
            }],
            elaborations: Vec::new(),
            elaboration_work: super::ProofWork {
                normalization: 0,
                substitution: 0,
                total: 0,
            },
        }
    }

    fn limits(work: u32) -> crate::Limits {
        crate::Limits {
            bytes: 1000,
            nodes: 100,
            depth: 20,
            work,
        }
    }

    /// The proof walk keeps unwinding after a refused intro body: the intro
    /// still charges its wrapper, and a failing charge replaces the refusal.
    /// Expected values are those of the closure-based walk this replaces.
    #[test]
    fn intro_charges_its_wrapper_after_a_refused_body() {
        let batch = pure_batch(reflexive_term("y"));
        let refusal = |work| {
            super::check_batch(&batch, limits(work))
                .err()
                .map(|error| (error.kind, error.span, error.message))
        };
        let exhausted = |start, end| {
            Some((
                crate::DiagnosticKind::Exhausted,
                crate::Span { start, end },
                alloc::string::String::from("proof size, depth or work limit exhausted"),
            ))
        };
        assert_eq!(refusal(57), exhausted(12, 15));
        assert_eq!(refusal(58), exhausted(10, 18));
        assert_eq!(refusal(66), exhausted(10, 18));
        assert_eq!(
            refusal(67),
            Some((
                crate::DiagnosticKind::Invalid,
                crate::Span { start: 17, end: 18 },
                alloc::string::String::from("unbound refl value"),
            ))
        );
    }

    /// An accepted pure proof reports exactly the work the original walk charged.
    #[test]
    fn accepted_pure_proof_reports_its_exact_work() {
        let batch = pure_batch(reflexive_term("x"));
        let budgets = super::ProofBudgets::from_limits(limits(1000));
        let checked = super::check_batch_with_budgets(&batch, limits(1000), budgets);
        let summary = checked.map(|(proofs, work)| {
            let terms: Vec<_> = proofs
                .iter()
                .map(|proof| (proof.lean_term.clone(), proof.claim.clone()))
                .collect();
            (terms, work.normalization, work.substitution, work.total)
        });
        assert_eq!(
            summary.ok(),
            Some((
                vec![(
                    alloc::string::String::from("(fun (v0 : El .i64) => (Eq.refl v0))"),
                    alloc::string::String::from("(∀ (v0 : El .i64), v0 = v0)"),
                )],
                10,
                0,
                83,
            ))
        );
        assert!(super::check_batch(&batch, limits(82)).is_err());
    }

    #[test]
    fn replace_all_matches_str_replace() {
        for text in [
            "",
            "__N__",
            "a__N__b__N__c",
            "____N__",
            "__N_",
            "__N____N__",
            "\u{e9}__N__\u{e9}",
            "no marker",
        ] {
            assert_eq!(
                super::replace_all(text, "__N__", "-7"),
                text.replace("__N__", "-7")
            );
        }
        let term = super::lower_unary_add(-7);
        assert!(term.contains("(.i64 -7)"));
        assert!(!term.contains("__N__"));
    }

    #[test]
    fn decimal_matches_the_display_rendering() {
        for value in [
            0,
            1,
            9,
            10,
            11,
            99,
            100,
            101,
            4_096,
            65_535,
            1_000_000,
            4_294_967_295,
            usize::MAX,
        ] {
            assert_eq!(super::decimal(value), alloc::format!("{value}"));
        }
    }

    /// The format-free lowered texts are byte for byte the `format!` texts
    /// they replaced, up to the widest binder index.
    #[test]
    fn lowered_texts_match_the_formatted_renderings() {
        let indices = [0, 1, 9, 10, 4_096, usize::MAX];
        for index in indices {
            assert_eq!(super::variable(index), alloc::format!("v{index}"));
            assert_eq!(
                super::refl_text(index),
                alloc::format!("(Eq.refl v{index})")
            );
            for other in indices {
                assert_eq!(
                    super::equation_text(index, other),
                    alloc::format!("v{index} = v{other}")
                );
            }
            for (sort_text, body) in [("El .i64", "(Eq.refl v0)"), ("", ""), ("PureTyCode", "{}")] {
                assert_eq!(
                    super::intro_text(index, sort_text, body),
                    alloc::format!("(fun (v{index} : {sort_text}) => {body})")
                );
            }
            let (premise, equality_lean) = ("(h v0)", "(Eq.refl v1)");
            for (code, motive) in [(".i64", "v1 = v2"), ("", ""), ("v3", "{x}")] {
                assert_eq!(
                    super::transport_text(index, code, motive, premise, equality_lean),
                    alloc::format!(
                        "(Eq.ndrec (motive := fun (v{index} : El {}) => {}) {premise} {equality_lean})",
                        code, motive
                    )
                );
            }
        }
        for (lowered, expression) in [
            ("f", "x"),
            ("", ""),
            ("(g v0)", "(\u{2200} (v1 : El .i64), v1 = v1)"),
        ] {
            assert_eq!(
                super::application_text(lowered, expression),
                alloc::format!("({lowered} {expression})")
            );
        }
    }

    /// Variable codes and sorts and equations lower to the `format!` texts
    /// they replaced.
    #[test]
    fn lean_variable_leaves_match_the_formatted_renderings() {
        let span = crate::Span { start: 0, end: 1 };
        let bounds = super::TreeBounds::new(limits(1000));
        assert!(bounds.is_some());
        let Some(bounds) = bounds else { return };
        for index in [0, 1, 10, usize::MAX] {
            assert_eq!(
                super::lean_code(&Code::Variable(index), span, bounds).ok(),
                Some(alloc::format!("v{index}"))
            );
            assert_eq!(
                super::lean_sort(&Sort::Value(Code::Variable(index)), span, bounds).ok(),
                Some(alloc::format!("El v{index}"))
            );
            for other in [0, 7, usize::MAX] {
                assert_eq!(
                    super::lean_proposition(
                        &Proposition::Eq(Code::I64, index, other),
                        span,
                        bounds
                    )
                    .ok(),
                    Some(alloc::format!("v{index} = v{other}"))
                );
            }
        }
    }

    #[test]
    fn bytes_within_matches_bytes_all() {
        for text in ["", "a-b_c", "a.b@c", "a b", "\u{e9}", "-", "A9"] {
            for extra in [&b"-_"[..], &b"-_.@"[..]] {
                let is_expected = text
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || extra.contains(&b));
                assert_eq!(super::bytes_within(text, extra), is_expected);
            }
        }
    }

    /// `stack_binder` finds the innermost binder of a name: the last spilled
    /// one, else the last inline one, as the `rposition` scans it replaces.
    #[test]
    fn stack_binder_finds_the_innermost_binder() {
        let mut stack = super::WorkStack::new();
        let mut index = 0usize;
        for _ in 0..10 {
            for name in ["x", "y", "x", "z"] {
                let binder = super::Binder {
                    name: alloc::string::String::from(name),
                    sort: Sort::Value(Code::Variable(index)),
                };
                assert!(stack.push_bounded(binder, 100).is_ok());
                index = index.saturating_add(1);
            }
        }
        for name in ["x", "y", "z", "w"] {
            let expected = match stack.spill.iter().rposition(|item| item.name == name) {
                Some(index) => stack.inline_len.checked_add(index),
                None => stack.inline[..stack.inline_len]
                    .iter()
                    .rposition(|item| item.as_ref().is_some_and(|binder| binder.name == name)),
            };
            let found = super::stack_binder(&stack, name).map(|(position, _)| position);
            assert_eq!(found, expected);
        }
        let mut inline_only = super::WorkStack::new();
        for name in ["a", "b", "a"] {
            let binder = super::Binder {
                name: alloc::string::String::from(name),
                sort: Sort::Codes,
            };
            assert!(inline_only.push_bounded(binder, 100).is_ok());
        }
        assert_eq!(
            super::stack_binder(&inline_only, "a").map(|(at, _)| at),
            Some(2)
        );
        assert_eq!(
            super::stack_binder(&inline_only, "b").map(|(at, _)| at),
            Some(1)
        );
        assert!(super::stack_binder(&inline_only, "c").is_none());
    }

    #[test]
    fn stack_shape_matches_the_slice_patterns() {
        let stack_zero =
            || noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0));
        let i64_pattern = || noble_kernel::shapes::Pattern::I64;
        let samples = [
            vec![stack_zero(), i64_pattern(), i64_pattern()],
            vec![stack_zero(), i64_pattern()],
            vec![
                noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(1)),
                i64_pattern(),
                i64_pattern(),
            ],
            vec![i64_pattern(), i64_pattern(), i64_pattern()],
            vec![
                stack_zero(),
                i64_pattern(),
                noble_kernel::shapes::Pattern::Bool,
            ],
            vec![stack_zero()],
            vec![stack_zero(), i64_pattern(), i64_pattern(), i64_pattern()],
            Vec::new(),
        ];
        for sample in &samples {
            assert_eq!(
                super::is_stack_then_i64s(sample, 2),
                matches!(
                    sample.as_slice(),
                    [
                        noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0)),
                        noble_kernel::shapes::Pattern::I64,
                        noble_kernel::shapes::Pattern::I64
                    ]
                )
            );
            assert_eq!(
                super::is_stack_then_i64s(sample, 1),
                matches!(
                    sample.as_slice(),
                    [
                        noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0)),
                        noble_kernel::shapes::Pattern::I64
                    ]
                )
            );
        }
    }
}
