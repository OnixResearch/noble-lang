#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; preparation's mutable parameters are only its request-local meter; draft trees and output buffers are newly owned, while source bytes, supplied types, and the committed session are borrowed unchanged until an explicit later commit."
)]

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use alloc::borrow::Cow;
pub(in crate::source) mod symbolic;

#[expect(
    tigerstyle::interior_mutability_in_core,
    reason = "Owner: noble-contracts; K-CHECK-07 request budget: one request-local Rc<Cell<u32>> counter a host opts into with ModuleSession::share_request_work; it is single-threaded (Rc is !Send), never outlives the session and its staged snapshots, and every meter debits it so nested checking cannot restart the request's budget. Explicit threading would turn the &self closure-walk and origin-replay APIs into &mut ones."
)]
impl super::Session {
    pub fn prepare(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<super::Prepared, super::Error> {
        self.prepare_signed(source_bytes, inputs, None, limits, limits.bytes)
    }

    pub(super) fn prepare_with_retention(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
        retained_bytes: u32,
    ) -> Result<super::Prepared, super::Error> {
        self.prepare_signed(source_bytes, inputs, None, limits, retained_bytes)
    }

    pub(super) fn prepare_signed(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        signature: Option<&noble_kernel::words::Scheme>,
        limits: crate::Limits,
        retained_bytes: u32,
    ) -> Result<super::Prepared, super::Error> {
        let mut meter = crate::Meter::pooled(limits, self.request_work());
        let mut result = self.prepare_signed_metered(
            source_bytes,
            inputs,
            signature,
            retained_bytes,
            &mut meter,
        );
        let work_used = meter.spent();
        match &mut result {
            Ok(prepared) => prepared.work_used = work_used,
            Err(failure) => failure.work_used = work_used,
        }
        result
    }

    /// Remaining work of the host-selected request this session shares
    /// across every checking stage, if the host opted in.
    pub(crate) fn request_work(&self) -> Option<&alloc::rc::Rc<core::cell::Cell<u32>>> {
        self.declared.as_ref()?.request_work.as_ref()
    }

    /// Session flags and pure scans are read before any branch and metered
    /// steps decide their own applicability, so every branch the pinned
    /// Aeneas joins differs only in plain values.
    pub(super) fn prepare_signed_metered(
        &self,
        source_bytes: &[u8],
        inputs: &[noble_kernel::types::Ty],
        signature: Option<&noble_kernel::words::Scheme>,
        retained_bytes: u32,
        meter: &mut crate::Meter,
    ) -> Result<super::Prepared, super::Error> {
        let parsed = if self.declared.is_some() {
            super::parsing::parse_declared(source_bytes, meter)
        } else {
            super::parsing::parse(source_bytes, meter)
        };
        let (mut tree, name) = match parsed {
            Ok(parsed) => parsed,
            Err(error) => return Err(super::Error::at(super::Stage::Parse, error)),
        };
        let is_named = name.is_some();
        let has_live_slots = self.live_slots.is_some();
        let is_declared_text = self
            .declared
            .as_ref()
            .is_some_and(|context| context.declared_text);
        let is_signed_self = self
            .declared
            .as_ref()
            .is_some_and(|context| context.signed_self);
        if has_live_slots || is_declared_text {
            attempt!(super::locals::lower(&mut tree, meter)
                .map_err(|error| super::Error::at(super::Stage::Resolve, error)));
        }
        let alias_candidates = attempt!(alias_candidates(&tree, self, is_named, meter)
            .map_err(|error| super::Error::at(super::Stage::Resolve, error)));
        let is_signed_owner = signed_owner(signature, is_signed_self);
        if let Err(error) =
            super::resolution::resolve(&mut tree, name.as_deref(), self, is_signed_owner, meter)
        {
            return Err(super::Error::at(super::Stage::Resolve, error));
        }
        let alias_uses = attempt!(checked_alias_uses(&tree, self, &alias_candidates, meter)
            .map_err(|error| super::Error::at(super::Stage::Check, error)));
        let has_borrowed_dispatch = crate::scan::find_first(&tree.nodes, |node| {
            matches!(node.kind, super::Kind::Call(super::Target::SlotInvoke(_)))
        })
        .is_some();
        if is_named && has_live_slots && has_borrowed_dispatch {
            return Err(super::Error::at(
                super::Stage::Check,
                crate::invalid(
                    tree.span,
                    "live-slot declarations cannot contain borrowed dispatch",
                ),
            ));
        }
        if let Err(error) = super::preflight::check(inputs, self, tree.span, meter) {
            return Err(super::Error::at(super::Stage::Check, error));
        }
        let selected_root = lone_named_root(&tree);
        let (extra, mode) = attempt!(retention(is_named, source_bytes.len(), tree.span));
        if let Err(error) = self.retained(extra, retained_bytes, tree.span, meter) {
            return Err(super::Error::at(super::Stage::Check, error));
        }
        let environment = match self.environment() {
            Ok(environment) => environment,
            Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
        };
        let (environment, prospective_owner) =
            attempt!(self.prospective_owner(environment, &tree, signature, is_signed_owner, meter));
        let (environment, inferred) = self.infer_signed(
            environment,
            Inference {
                tree: &tree,
                signature,
                prospective_owner,
                mode,
                inputs,
                source_bytes,
            },
            meter,
        );
        let (state, signed_relation) = attempt!(inferred);
        let Some(name) = name else {
            let environment = attempt!(emission_environment(environment, tree.span, meter));
            // The immutable namespace snapshot names the library word an
            // eligibility refusal instantiates; an undeclared session has none.
            let words = match &self.declared {
                Some(context) => context.words.as_slice(),
                None => &[],
            };
            let (submission, checked, output) = attempt!(super::emission::emit(
                state,
                environment,
                &self.definitions,
                words,
                source_bytes.len(),
                meter
            ));
            return Ok(self.prepared(
                Parts {
                    definition: None,
                    addition: alloc::vec::Vec::new(),
                    submission: Some(submission),
                    checked: Some(checked),
                    output,
                },
                selected_root,
                meter.limits,
            ));
        };
        let (definition, addition) = match declaration(
            tree,
            name,
            source_bytes,
            signature,
            signed_relation,
            alias_uses,
            self,
            meter,
        ) {
            Ok(declared) => declared,
            Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
        };
        Ok(self.prepared(
            Parts {
                definition: Some(definition),
                addition,
                submission: None,
                checked: None,
                output: inputs.to_vec(),
            },
            selected_root,
            meter.limits,
        ))
    }

    /// The prepared transaction: this session's identity with the checked parts.
    fn prepared(
        &self,
        parts: Parts,
        selected_root: Option<u32>,
        limits: crate::Limits,
    ) -> super::Prepared {
        super::Prepared {
            generation: self.generation,
            history: self.history.clone(),
            hosts: self.hosts,
            text_cursor: self.text_cursor,
            live_selected: self.live_selected.clone(),
            live_slots: self.live_slots.is_some(),
            live_slot_owner: self.live_slots.as_ref().map(|slots| slots.owner.clone()),
            limits,
            calculator: self.calculator,
            checked_i64: self.checked_i64,
            boundary: self.bindings.as_ref().map(|bindings| bindings.key.clone()),
            definition: parts.definition,
            selected_root,
            addition: parts.addition,
            submission: parts.submission,
            checked: parts.checked,
            work_used: 0,
            output: parts.output,
        }
    }

    pub(super) fn environment(
        &self,
    ) -> Result<Cow<'_, noble_kernel::contracts::Env>, crate::Diagnostic> {
        if let Some(declared) = &self.declared {
            if self.calculator {
                return Ok(Cow::Borrowed(&declared.environment));
            }
            return Ok(Cow::Owned(declared.environment.clone()));
        }
        if let Some(profile) = &self.live_slots {
            return Ok(Cow::Owned(profile.environment.clone()));
        }
        match &self.bindings {
            Some(bindings) => Ok(Cow::Owned(bindings.environment.clone())),
            None if self.calculator => owned_environment(
                noble_kernel::contracts::calculator_environment()
                    .map_err(|_| crate::internal(crate::Span { start: 0, end: 0 })),
            ),
            None if self.checked_i64 => owned_environment(super::checked_i64_environment()),
            None if self.text_cursor => owned_environment(
                noble_kernel::contracts::text_cursor_environment()
                    .map_err(|_| crate::internal(crate::Span { start: 0, end: 0 })),
            ),
            None if self.live_selected.is_some() => {
                owned_environment(super::live_environment(self.hosts))
            }
            None => owned_environment(super::environment()),
        }
    }

    /// An actual unpublished owner interface occupies this transaction's
    /// prospective Env row while its body is checked. It is never installed
    /// into the committed namespace or exposed as a raw graph self edge.
    fn prospective_owner<'e>(
        &self,
        environment: Cow<'e, noble_kernel::contracts::Env>,
        tree: &super::Tree,
        signature: Option<&noble_kernel::words::Scheme>,
        is_signed_owner: bool,
        meter: &mut crate::Meter,
    ) -> Result<
        (
            Cow<'e, noble_kernel::contracts::Env>,
            Option<noble_kernel::contracts::Definition>,
        ),
        super::Error,
    > {
        if !is_signed_owner
            || crate::scan::find_first(&tree.nodes, |node| {
                matches!(node.kind, super::Kind::Call(super::Target::SelfOwner))
            })
            .is_none()
        {
            return Ok((environment, None));
        }
        let Some(scheme) = signature else {
            return Err(super::Error::at(
                super::Stage::Check,
                crate::invalid(tree.span, "signed owner has no signature"),
            ));
        };
        let Some(owner) = self.declared.as_ref().and_then(|context| context.owner) else {
            return Err(super::Error::at(
                super::Stage::Check,
                crate::invalid(tree.span, "signed owner has no module"),
            ));
        };
        let (environment, rows) = environment_rows(environment);
        let def = noble_kernel::contracts::Definition(attempt!(crate::index(rows, tree.span)
            .map_err(|problem| super::Error::at(super::Stage::Check, problem))));
        attempt!(meter
            .node(tree.span)
            .map_err(|problem| super::Error::at(super::Stage::Check, problem)));
        // Copy-on-write: only a signed declaration pays for an owned
        // prospective environment row. The environment is taken by value (a
        // borrowed one is cloned) rather than through `Cow::to_mut`, whose
        // mutable borrow the pinned Aeneas cannot give back.
        let mut owned = match environment {
            Cow::Borrowed(borrowed) => borrowed.clone(),
            Cow::Owned(owned) => owned,
        };
        owned.defs.push(scheme.clone());
        owned.kinds.push(noble_kernel::contracts::Behavior::Named);
        owned.deps.push(alloc::vec::Vec::new());
        owned.definition_owners.push(Some(owner));
        Ok((Cow::Owned(owned), Some(def)))
    }

    /// Infer the parsed root and, for a signed owner, relate its body to its
    /// lexical source; the copy-on-write environment is returned unchanged.
    ///
    /// The environment is consumed by value and each copy-on-write state
    /// returns from its own arm; the pinned Aeneas cannot translate `Cow`'s
    /// Deref nor join borrows of the two states.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; inference charges the shared meter and builds owned state through non-const calls."
    )]
    fn infer_signed<'e>(
        &self,
        environment: Cow<'e, noble_kernel::contracts::Env>,
        request: Inference<'_>,
        meter: &mut crate::Meter,
    ) -> (Cow<'e, noble_kernel::contracts::Env>, Inferred) {
        match environment {
            Cow::Borrowed(borrowed) => {
                let inferred = self.infer_with(borrowed, request, meter);
                (Cow::Borrowed(borrowed), inferred)
            }
            Cow::Owned(owned) => {
                let inferred = self.infer_with(&owned, request, meter);
                (Cow::Owned(owned), inferred)
            }
        }
    }

    /// Infer under one environment state. The request binder is not named
    /// after the `inference` module, which generated Lean would shadow.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; inference charges the shared meter and builds owned state through non-const calls."
    )]
    fn infer_with(
        &self,
        environment: &noble_kernel::contracts::Env,
        request: Inference<'_>,
        meter: &mut crate::Meter,
    ) -> Inferred {
        let state = match super::inference::infer(
            super::inference::Scope {
                root: request.tree,
                session: self,
                environment,
                root_signature: request.signature,
                prospective_owner: request.prospective_owner,
            },
            request.mode,
            request.inputs,
            meter,
        ) {
            Ok(state) => state,
            Err(error) => return Err(super::Error::at(super::Stage::Check, error)),
        };
        let Some(owner) = request.prospective_owner else {
            return Ok((state, None));
        };
        let Some(scheme) = request.signature else {
            return Err(super::Error::at(
                super::Stage::Check,
                crate::invalid(request.tree.span, "signed owner has no signature"),
            ));
        };
        match symbolic::check(
            request.tree,
            &state,
            scheme,
            request.source_bytes,
            owner,
            self,
            environment,
            meter,
        ) {
            Ok(relation) => Ok((state, Some(relation))),
            Err(problem) => Err(super::Error::at(super::Stage::Check, problem)),
        }
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; retained-byte admission uses checked conversions and metering whose rejection paths construct owned diagnostics."
    )]
    fn retained(
        &self,
        extra: usize,
        retained_bytes: u32,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        let history_bytes = attempt!(crate::index(self.history.len(), span));
        let limit_bytes = attempt!(crate::offset(retained_bytes, span));
        let total = attempt!(self.history.len().checked_add(extra).ok_or_else(|| {
            super::exhausted(span, "retained definition history byte count overflows")
        }));
        if total > limit_bytes {
            return Err(super::exhausted(
                span,
                "retained definition history byte limit exceeded",
            ));
        }
        meter.charge(history_bytes, span)
    }
}

/// The borrowed inputs of one inference over a parsed root.
#[derive(Clone, Copy)]
struct Inference<'a> {
    tree: &'a super::Tree,
    signature: Option<&'a noble_kernel::words::Scheme>,
    prospective_owner: Option<noble_kernel::contracts::Definition>,
    mode: super::inference::Mode,
    inputs: &'a [noble_kernel::types::Ty],
    source_bytes: &'a [u8],
}

/// One inference's checked state and, for a signed owner, its symbolic relation.
type Inferred = Result<(super::inference::State, Option<super::SignedRelation>), super::Error>;

/// What preparing one source produced: a declaration or a submission.
struct Parts {
    definition: Option<super::Named>,
    addition: alloc::vec::Vec<u8>,
    submission: Option<noble_kernel::execution::Submission>,
    checked: Option<noble_kernel::untrusted::Checked>,
    output: alloc::vec::Vec<noble_kernel::types::Ty>,
}

/// Whether a signed-self session checks this source as a signed owner: it
/// carries its declared signature.
const fn signed_owner(
    signature: Option<&noble_kernel::words::Scheme>,
    is_signed_self: bool,
) -> bool {
    match signature {
        Some(_) => is_signed_self,
        None => false,
    }
}

/// The definition a root consisting of one named call selects.
#[expect(
    tigerstyle::platform_dependent_cast,
    reason = "Owner: noble-contracts; the operand is a u32 table index and the cast is a widening that cannot fail where usize is at least 32 bits; the portable form would add fallible plumbing to infallible lookups"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the node lookup goes through Vec's non-const Deref."
)]
fn lone_named_root(tree: &super::Tree) -> Option<u32> {
    if tree.body.len() != 1 {
        return None;
    }
    match tree.nodes.get(tree.body[0] as usize) {
        Some(node) => match &node.kind {
            super::Kind::Call(super::Target::Named(index)) => Some(*index),
            _ => None,
        },
        None => None,
    }
}

/// The retained bytes a declaration adds (its source and a four-byte
/// header) and the inference mode of the source.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the overflow refusal constructs an owned diagnostic, which a const fn cannot."
)]
fn retention(
    is_named: bool,
    source_len: usize,
    span: crate::Span,
) -> Result<(usize, super::inference::Mode), super::Error> {
    if !is_named {
        return Ok((0, super::inference::Mode::Submission));
    }
    match source_len.checked_add(4) {
        Some(extra) => Ok((extra, super::inference::Mode::Declaration)),
        None => Err(super::Error::at(
            super::Stage::Check,
            super::exhausted(span, "retained definition source length overflows"),
        )),
    }
}

/// The owned environment a submission emits. Cloning a borrowed calculator
/// environment is first bounded and charged; an owned one is moved.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the clone admission charges the shared meter and clones the environment through non-const calls."
)]
fn emission_environment(
    environment: Cow<'_, noble_kernel::contracts::Env>,
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<noble_kernel::contracts::Env, super::Error> {
    let borrowed = match environment {
        Cow::Borrowed(borrowed) => borrowed,
        Cow::Owned(owned) => return Ok(owned),
    };
    if borrowed.defs.len() > noble_kernel::contracts::CALCULATOR_ENVIRONMENT_ROW_LIMIT {
        return Err(super::Error::at(
            super::Stage::Check,
            super::exhausted(span, "calculator environment row limit exceeded"),
        ));
    }
    let Some(budget) = borrowed.calculator_clone_budget() else {
        return Err(super::Error::at(
            super::Stage::Check,
            super::exhausted(span, "calculator environment clone bound exceeded"),
        ));
    };
    if budget.reserved_bytes > u64::from(meter.limits.work) {
        return Err(super::Error::at(
            super::Stage::Check,
            super::exhausted(span, "calculator environment clone work exceeded"),
        ));
    }
    let Ok(bytes) = u32::try_from(budget.bytes) else {
        return Err(super::Error::at(
            super::Stage::Check,
            super::exhausted(span, "calculator environment clone work exceeded"),
        ));
    };
    if let Err(error) = meter.charge(bytes, span) {
        return Err(super::Error::at(super::Stage::Check, error));
    }
    Ok(borrowed.clone())
}

/// An owned environment as the copy-on-write environment.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the owned environment Result has a destructor, which a const fn cannot evaluate."
)]
fn owned_environment<'e>(
    environment: Result<noble_kernel::contracts::Env, crate::Diagnostic>,
) -> Result<Cow<'e, noble_kernel::contracts::Env>, crate::Diagnostic> {
    match environment {
        Ok(environment) => Ok(Cow::Owned(environment)),
        Err(error) => Err(error),
    }
}

/// The environment's row count, returned with the unchanged environment.
const fn environment_rows(
    environment: Cow<'_, noble_kernel::contracts::Env>,
) -> (Cow<'_, noble_kernel::contracts::Env>, usize) {
    let rows = match &environment {
        Cow::Borrowed(borrowed) => borrowed.defs.len(),
        Cow::Owned(owned) => owned.defs.len(),
    };
    (environment, rows)
}

/// Preserve exact source node ordinals before resolution erases spellings.
/// Only current outside aliases on a top-level named body are mutable;
/// module-local imports belong to that module's immutable source snapshot.
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; growth is bounded by the checked input or admission limits that drive the loop."
)]
fn alias_candidates(
    tree: &super::Tree,
    session: &super::Session,
    is_named: bool,
    meter: &mut crate::Meter,
) -> Result<alloc::vec::Vec<(u32, usize)>, crate::Diagnostic> {
    if !is_named {
        return Ok(alloc::vec::Vec::new());
    }
    let Some(context) = session.declared.as_ref() else {
        return Ok(alloc::vec::Vec::new());
    };
    if context.owner.is_some() || context.aliases.is_empty() {
        return Ok(alloc::vec::Vec::new());
    }
    let mut candidates = alloc::vec::Vec::new();
    let mut node_index = 0;
    let mut failure = None;
    while failure.is_none() && node_index < tree.nodes.len() {
        match alias_candidate(&tree.nodes[node_index], node_index, &context.aliases, meter) {
            Ok(Some(candidate)) => candidates.push(candidate),
            Ok(None) => {}
            Err(problem) => failure = Some(problem),
        }
        node_index += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    Ok(candidates)
}

/// Charge one source node and return the first alias its word qualifies.
fn alias_candidate(
    node: &super::Node,
    node_index: usize,
    aliases: &[(alloc::string::String, u64)],
    meter: &mut crate::Meter,
) -> Result<Option<(u32, usize)>, crate::Diagnostic> {
    attempt!(meter.charge(1, node.span));
    let super::Kind::Word(word) = &node.kind else {
        return Ok(None);
    };
    if let Some(alias_index) = attempt!(first_alias(word, aliases, node.span, meter)) {
        return Ok(Some((
            attempt!(crate::index(node_index, node.span)),
            alias_index,
        )));
    }
    Ok(None)
}

/// The first alias `word` qualifies as `alias.name`, charging one unit per alias tried.
fn first_alias(
    word: &[u8],
    aliases: &[(alloc::string::String, u64)],
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<Option<usize>, crate::Diagnostic> {
    let mut alias_index = 0;
    let mut found = None;
    let mut charged = Ok(());
    while charged.is_ok() && found.is_none() && alias_index < aliases.len() {
        charged = meter.charge(1, span);
        // The pure test runs unconditionally; a failed charge ends the walk.
        let is_qualified = qualifies(word, &aliases[alias_index]);
        if charged.is_ok() && is_qualified {
            found = Some(alias_index);
        }
        alias_index += 1;
    }
    attempt!(charged);
    Ok(found)
}

/// Whether `word` is a member qualified by `alias`: the alias, a dot, and a nonempty member.
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-contracts; an alias length is a String length below isize::MAX, so alias.len() + 1 cannot overflow usize."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; byte-slice prefix comparison is not const."
)]
fn qualifies(word: &[u8], alias: &(alloc::string::String, u64)) -> bool {
    let (alias, _) = alias;
    word.len() > alias.len() + 1 && word.starts_with(alias.as_bytes()) && word[alias.len()] == b'.'
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite."
)]
fn checked_alias_uses(
    tree: &super::Tree,
    session: &super::Session,
    candidates: &[(u32, usize)],
    meter: &mut crate::Meter,
) -> Result<alloc::vec::Vec<super::AliasUse>, crate::Diagnostic> {
    if candidates.is_empty() {
        return Ok(alloc::vec::Vec::new());
    }
    let context = session
        .declared
        .as_ref()
        .ok_or_else(|| crate::internal(tree.span))?;
    let mut uses: alloc::vec::Vec<super::AliasUse> = alloc::vec::Vec::new();
    let mut at = 0;
    let mut outcome = Ok(());
    while outcome.is_ok() && at < candidates.len() {
        outcome = checked_alias_use(
            tree,
            session,
            &context.aliases,
            candidates[at],
            &mut uses,
            meter,
        );
        at += 1;
    }
    attempt!(outcome);
    Ok(uses)
}

/// Check one alias candidate's resolved call and record it under its spelling.
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::borrowed_argument_types,
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; the step appends to the owned alias-use list, so it needs the Vec rather than a slice; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added."
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameter list is the internal call shape; bundling it would change the extracted subject"
)]
fn checked_alias_use(
    tree: &super::Tree,
    session: &super::Session,
    aliases: &[(alloc::string::String, u64)],
    candidate: (u32, usize),
    uses: &mut alloc::vec::Vec<super::AliasUse>,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    let (node_index, alias_index) = candidate;
    let node = tree
        .nodes
        .get(attempt!(crate::offset(node_index, tree.span)))
        .ok_or_else(|| crate::internal(tree.span))?;
    let (alias, owner) = aliases
        .get(alias_index)
        .ok_or_else(|| crate::internal(node.span))?;
    let super::Kind::Call(target) = &node.kind else {
        return Err(crate::invalid(
            node.span,
            "checked import alias lost its source word",
        ));
    };
    match *target {
        super::Target::Named(index) => {
            let definition = session
                .definitions
                .get(attempt!(crate::offset(index, node.span)))
                .ok_or_else(|| crate::internal(node.span))?;
            if definition.owner != Some(*owner) {
                return Err(crate::invalid(
                    node.span,
                    "checked alias call has a foreign module owner",
                ));
            }
        }
        super::Target::Builtin(_) => {}
        super::Target::SelfOwner | super::Target::SlotInvoke(_) => {
            return Err(crate::invalid(
                node.span,
                "checked alias call has an unsupported target",
            ));
        }
    }
    attempt!(meter.charge(1, node.span));
    if let Some(existing) = crate::scan::find_first_mut(uses, |entry| {
        entry.spelling == alias.as_str() && entry.owner == *owner
    }) {
        existing.nodes.push((node_index, *target));
    } else {
        attempt!(meter.node(node.span));
        uses.push(super::AliasUse {
            spelling: alias.clone(),
            owner: *owner,
            nodes: alloc::vec![(node_index, *target)],
        });
    }
    Ok(())
}
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; the parameter list is the internal call shape; bundling it would change the extracted subject"
)]
#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; the parameter list is the internal call shape; bundling it would change the extracted subject"
)]
fn declaration(
    tree: super::Tree,
    name: alloc::string::String,
    source_bytes: &[u8],
    signature: Option<&noble_kernel::words::Scheme>,
    signed: Option<super::SignedRelation>,
    alias_uses: alloc::vec::Vec<super::AliasUse>,
    session: &super::Session,
    meter: &mut crate::Meter,
) -> Result<(super::Named, alloc::vec::Vec<u8>), crate::Diagnostic> {
    // Inference has solved the open graph, including every dependency body.
    // No Unit instance decides generic validity.
    let identity = attempt!(super::resolution::comparison::identity(
        &tree, session, signature, meter
    ));
    let count = attempt!(crate::index(source_bytes.len(), tree.span));
    attempt!(meter.charge(count.saturating_add(4), tree.span));
    let mut addition = alloc::vec::Vec::with_capacity(source_bytes.len().saturating_add(4));
    addition.extend_from_slice(&count.to_le_bytes());
    addition.extend_from_slice(source_bytes);
    Ok((
        super::Named {
            name,
            identity,
            owner: session.declared.as_ref().and_then(|context| context.owner),
            tree,
            signature: signature.cloned(),
            signed,
            alias_uses,
            history_start: None,
        },
        addition,
    ))
}
