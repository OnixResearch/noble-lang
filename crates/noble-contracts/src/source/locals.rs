//! Selected declared-text and live-slot lexical sugar. No local instruction reaches the kernel.
//!
//! Physical stacks place immutable bound values above the visible stack.
//! Checked `dip` executes ordinary words beneath them, while `dup` and
//! `swap` project a bound value into the visible stack. A quotation captures
//! its free values at construction with checked `quote` and `compose`; its
//! recipe therefore retains the original values, not source spellings.

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain items directly, and the qualified form at every use site would add noise without changing the dependency."
)]
use alloc::vec::Vec;

struct Bound {
    name: Vec<u8>,
    binder_node: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Generation {
    BindingSwap,
    ProjectionDup,
    ProjectionSwap,
    CaptureQuote,
    CaptureCompose,
    WrapQuotation,
    WrapDip,
    CleanupDrop,
}

impl Generation {
    #[expect(
        tigerstyle::fragile_exhaustive_enum_match,
        reason = "Owner: noble-contracts; the matched enum is deliberately sealed, so every exhaustive match over it is reviewed when a variant is added."
    )]
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::BindingSwap => "binding-swap",
            Self::ProjectionDup => "projection-dup",
            Self::ProjectionSwap => "projection-swap",
            Self::CaptureQuote => "capture-quote",
            Self::CaptureCompose => "capture-compose",
            Self::WrapQuotation => "wrap-quotation",
            Self::WrapDip => "wrap-dip",
            Self::CleanupDrop => "cleanup-drop",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct GeneratedWitness {
    pub node: u32,
    pub generation: Generation,
    pub occurrence_node: Option<u32>,
    pub binder_node: Option<u32>,
}

struct Lower<'a> {
    nodes: &'a mut Vec<super::Node>,
    meter: &'a mut crate::Meter,
    witnesses: Option<&'a mut Vec<GeneratedWitness>>,
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
pub(super) fn lower(
    tree: &mut super::Tree,
    meter: &mut crate::Meter,
) -> Result<(), crate::Diagnostic> {
    lower_inner(tree, meter, None)
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; it forwards the growable witness vector to lower_inner, which appends generated witnesses with Vec::push."
)]
pub(super) fn lower_with_witness(
    tree: &mut super::Tree,
    meter: &mut crate::Meter,
    witnesses: &mut Vec<GeneratedWitness>,
) -> Result<(), crate::Diagnostic> {
    lower_inner(tree, meter, Some(witnesses))
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
fn lower_inner(
    tree: &mut super::Tree,
    meter: &mut crate::Meter,
    witnesses: Option<&mut Vec<GeneratedWitness>>,
) -> Result<(), crate::Diagnostic> {
    if crate::scan::find_first(&tree.nodes, |node| is_let(&node.kind)).is_none() {
        return Ok(());
    }
    // A declaration's final, unreferenced quotation is its syntactic `def`
    // envelope, not an executable node. Keep it last when the lowering adds
    // checked operations, so signed owner certificates can distinguish it
    // from every generated executable node without trusting source spans.
    let has_envelope = match tree.nodes.len().checked_sub(1) {
        Some(last) => is_envelope(tree, last),
        None => false,
    };
    let envelope = if has_envelope { tree.nodes.pop() } else { None };
    let body = core::mem::take(&mut tree.body);
    let lowered = lower_body(&mut tree.nodes, meter, witnesses, &body);
    if let Some(envelope) = envelope {
        tree.nodes.push(envelope);
    }
    tree.body = attempt!(lowered);
    Ok(())
}

/// Lower one body through a `Lower` that borrows the node arena, meter and
/// witness log only for the duration of this call.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; lowering appends generated nodes to the growable node arena."
)]
fn lower_body(
    nodes: &mut Vec<super::Node>,
    meter: &mut crate::Meter,
    witnesses: Option<&mut Vec<GeneratedWitness>>,
    body: &[u32],
) -> Result<Vec<u32>, crate::Diagnostic> {
    let mut lower = Lower {
        nodes,
        meter,
        witnesses,
    };
    lower.body(body, Vec::new(), 0)
}

/// Whether node `last` is an empty quotation the body never references.
fn is_envelope(tree: &super::Tree, last: usize) -> bool {
    if !is_empty_quotation(&tree.nodes[last].kind) {
        return false;
    }
    let mut at = 0;
    while at < tree.body.len() && !names_node(tree.body[at], last) {
        at += 1;
    }
    at == tree.body.len()
}

/// Whether a node is a quotation without children.
const fn is_empty_quotation(kind: &super::Kind) -> bool {
    match kind {
        super::Kind::Quotation(children) => children.is_empty(),
        _ => false,
    }
}

/// Whether body entry `id` names the arena node `node`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the checked u32-to-usize conversion is not const."
)]
fn names_node(id: u32, node: usize) -> bool {
    match usize::try_from(id) {
        Ok(id) => id == node,
        Err(_) => false,
    }
}

/// Whether a node is the `let` binding keyword.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; byte-vector comparison is not const."
)]
fn is_let(kind: &super::Kind) -> bool {
    matches!(kind, super::Kind::Word(word) if word == b"let")
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the byte scan helper and slice comparisons in the body are not const."
)]
fn name(bytes: &[u8]) -> bool {
    let Some((&first, rest)) = bytes.split_first() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == b'_')
        && super::every_byte(rest, |byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && bytes != b"let"
}

#[expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-contracts; the name matches the established vocabulary used by callers; renaming it would churn call sites without clarifying them."
)]
fn source_node(
    nodes: &[super::Node],
    id: u32,
    span: crate::Span,
) -> Result<&super::Node, crate::Diagnostic> {
    nodes
        .get(attempt!(crate::offset(id, span)))
        .ok_or_else(|| crate::internal(span))
}

/// Check lexical free occurrences before moving any quotation body. `shadow`
/// includes binders introduced inside the quoted body, not the outer locals.
#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; the lexical free-occurrence walk threads its node table, body, outer binders, shadow stack and meter explicitly; bundling them would hide which state each recursive step reads or extends"
)]
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::mutating_input_in_pure,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the shadow scope stack is pushed and popped while names are freed, so it needs the growable vector."
)]
fn free(
    nodes: &[super::Node],
    body: &[u32],
    outer: &[Bound],
    shadow: &mut Vec<Vec<u8>>,
    used: &mut [bool],
    meter: &mut crate::Meter,
    depth: u32,
    span: crate::Span,
) -> Result<(), crate::Diagnostic> {
    attempt!(meter.depth(depth, span));
    let level = FreeLevel {
        nodes,
        body,
        outer,
        scope: shadow.len(),
        depth,
        span,
    };
    let mut at = 0;
    let mut outcome = Ok(());
    while outcome.is_ok() && at < body.len() {
        match free_entry(level, at, shadow, used, meter) {
            Ok(true) => at += 2,
            Ok(false) => at += 1,
            Err(problem) => outcome = Err(problem),
        }
    }
    attempt!(outcome);
    shadow.truncate(level.scope);
    Ok(())
}

/// One quotation level of the `free` walk: its tables, its body and the
/// shadow height at which the level's own binders start.
#[derive(Clone, Copy)]
struct FreeLevel<'a> {
    nodes: &'a [super::Node],
    body: &'a [u32],
    outer: &'a [Bound],
    scope: usize,
    depth: u32,
    span: crate::Span,
}

/// Check one body entry of a `free` level: a `let` binder with its name, a
/// word occurrence or a nested quotation. Returns whether a binder consumed
/// two entries.
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the shadow scope stack is pushed and popped while names are freed, so it needs the growable vector."
)]
#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-contracts; at is an in-bounds body index, so at + 1 is at most the body length and cannot overflow usize."
)]
fn free_entry(
    level: FreeLevel<'_>,
    at: usize,
    shadow: &mut Vec<Vec<u8>>,
    used: &mut [bool],
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    attempt!(meter.charge(1, level.span));
    let node = attempt!(source_node(level.nodes, level.body[at], level.span));
    if is_let(&node.kind) {
        let next = level
            .body
            .get(at + 1)
            .copied()
            .ok_or_else(|| crate::invalid(node.span, "let requires a local name"))?;
        let bind = attempt!(source_node(level.nodes, next, node.span));
        let super::Kind::Word(name_bytes) = &bind.kind else {
            return Err(crate::invalid(bind.span, "let requires a local name"));
        };
        if !name(name_bytes)
            || crate::scan::find_first(&shadow[level.scope..], |known| known == name_bytes)
                .is_some()
        {
            return Err(crate::invalid(bind.span, "duplicate or invalid local name"));
        }
        attempt!(meter.node(bind.span));
        shadow.push(name_bytes.clone());
        return Ok(true);
    }
    match &node.kind {
        super::Kind::Word(word) => {
            if !attempt!(shadowed(shadow, word, meter, node.span)) {
                attempt!(mark_outer_use(level.outer, word, used, meter, node.span));
            }
            Ok(false)
        }
        super::Kind::Quotation(nested) => {
            let deeper = level
                .depth
                .checked_add(1)
                .ok_or_else(|| crate::invalid(node.span, "lexical quotation depth overflow"))?;
            attempt!(free(
                level.nodes,
                nested,
                level.outer,
                shadow,
                used,
                meter,
                deeper,
                node.span
            ));
            Ok(false)
        }
        _ => Ok(false),
    }
}

/// Whether a binder of the quoted body hides `word`, searching from the newest
/// and charging one unit per binder inspected.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it charges only the caller-owned work meter for this bounded pass."
)]
fn shadowed(
    shadow: &[Vec<u8>],
    word: &[u8],
    meter: &mut crate::Meter,
    span: crate::Span,
) -> Result<bool, crate::Diagnostic> {
    let mut at = shadow.len();
    let mut is_hidden = false;
    let mut failure = None;
    while !is_hidden && at > 0 {
        at -= 1;
        if let Err(problem) = meter.charge(1, span) {
            failure = Some(problem);
            break;
        }
        is_hidden = same_bytes(&shadow[at], word);
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(is_hidden),
    }
}

/// Mark the first outer binder named `word` as used, charging one unit per
/// binder inspected.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
fn mark_outer_use(
    outer: &[Bound],
    word: &[u8],
    used: &mut [bool],
    meter: &mut crate::Meter,
    span: crate::Span,
) -> Result<(), crate::Diagnostic> {
    let mut at = 0;
    let mut is_found = false;
    let mut failure = None;
    while !is_found && at < outer.len() {
        if let Err(problem) = meter.charge(1, span) {
            failure = Some(problem);
            break;
        }
        is_found = mark_if_named(outer, word, used, at);
        at += 1;
    }
    match failure {
        Some(problem) => Err(problem),
        None => Ok(()),
    }
}

/// Mark outer binder `at` as used when it is named `word`, reporting whether
/// it was.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it marks only the caller-owned capture set for this bounded pass."
)]
fn mark_if_named(outer: &[Bound], word: &[u8], used: &mut [bool], at: usize) -> bool {
    let is_named = same_bytes(&outer[at].name, word);
    if is_named {
        used[at] = true;
    }
    is_named
}

/// `Some(at)` when `is_match`, so a scan records its match without branching.
const fn matched_at(is_match: bool, at: usize) -> Option<usize> {
    if is_match {
        Some(at)
    } else {
        None
    }
}

/// Byte equality, called from the binder scans so that no loop compares a
/// field of an indexed element directly.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; slice comparison is not const."
)]
fn same_bytes(left: &[u8], right: &[u8]) -> bool {
    left == right
}

impl Lower<'_> {
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn node(
        &mut self,
        kind: super::Kind,
        span: crate::Span,
        generation: Generation,
        occurrence_node: Option<u32>,
        binder_node: Option<u32>,
    ) -> Result<u32, crate::Diagnostic> {
        attempt!(self.meter.node(span));
        let id = attempt!(crate::index(self.nodes.len(), span));
        self.nodes.push(super::Node { kind, span });
        if let Some(witnesses) = &mut self.witnesses {
            witnesses.push(GeneratedWitness {
                node: id,
                generation,
                occurrence_node,
                binder_node,
            });
        }
        Ok(id)
    }

    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn builtin(
        &mut self,
        definition: u32,
        span: crate::Span,
        generation: Generation,
        occurrence_node: Option<u32>,
        binder_node: Option<u32>,
    ) -> Result<u32, crate::Diagnostic> {
        self.node(
            super::Kind::Call(super::Target::Builtin(definition)),
            span,
            generation,
            occurrence_node,
            binder_node,
        )
    }

    #[expect(
        tigerstyle::ambiguous_params,
        tigerstyle::mutating_input_in_pure,
        tigerstyle::too_many_parameters,
        reason = "Owner: noble-contracts; the same-typed parameters are positional coordinates at each checked call site; a newtype would not change the call shape; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
    )]
    fn wrapped(
        &mut self,
        id: u32,
        locals: &[Bound],
        hidden: usize,
        depth: u32,
        span: crate::Span,
        occurrence_node: u32,
    ) -> Result<Vec<u32>, crate::Diagnostic> {
        let mut code = Ok(alloc::vec![id]);
        let mut layer = 0;
        while code.is_ok() && layer < hidden {
            code = match code {
                Ok(current) => {
                    self.wrap_layer(current, layer, locals, depth, span, occurrence_node)
                }
                Err(problem) => Err(problem),
            };
            layer += 1;
        }
        code
    }

    /// Wrap `code` beneath one more hidden binder: quote it and run it under
    /// `dip`, as one layer of `wrapped`.
    #[expect(
        tigerstyle::ambiguous_params,
        tigerstyle::mutating_input_in_pure,
        tigerstyle::too_many_parameters,
        reason = "Owner: noble-contracts; the same-typed parameters are positional coordinates at each checked call site; a newtype would not change the call shape; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
    )]
    #[expect(
        tigerstyle::sentinel_fallback,
        reason = "Owner: noble-contracts; an unrepresentable depth maps to usize::MAX, whose following checked_add fails closed."
    )]
    fn wrap_layer(
        &mut self,
        code: Vec<u32>,
        layer: usize,
        locals: &[Bound],
        depth: u32,
        span: crate::Span,
        occurrence_node: u32,
    ) -> Result<Vec<u32>, crate::Diagnostic> {
        let count = usize::try_from(depth)
            .unwrap_or(usize::MAX)
            .checked_add(layer)
            .and_then(|value| value.checked_add(1))
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| crate::invalid(span, "lexical quotation depth overflow"))?;
        attempt!(self.meter.depth(count, span));
        let binder_node = locals
            .get(layer)
            .map(|bound| bound.binder_node)
            .ok_or_else(|| crate::internal(span))?;
        let quotation = attempt!(self.node(
            super::Kind::Quotation(code),
            span,
            Generation::WrapQuotation,
            Some(occurrence_node),
            Some(binder_node)
        ));
        let dip = attempt!(self.builtin(
            3,
            span,
            Generation::WrapDip,
            Some(occurrence_node),
            Some(binder_node)
        ));
        Ok(alloc::vec![quotation, dip])
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Owner: noble-contracts; one lexical lowering step names its output code, owner, locals and witness state separately; bundling them would change the extracted subject"
    )]
    #[expect(
        tigerstyle::ambiguous_params,
        tigerstyle::mutating_input_in_pure,
        tigerstyle::too_many_parameters,
        reason = "Owner: noble-contracts; the same-typed parameters are positional coordinates at each checked call site; a newtype would not change the call shape; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
    )]
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-contracts; the emitted code buffer grows with Vec::push, so it needs the growable vector."
    )]
    fn operation(
        &mut self,
        code: &mut Vec<u32>,
        definition: u32,
        locals: &[Bound],
        hidden: usize,
        depth: u32,
        span: crate::Span,
        generation: Generation,
        occurrence_node: u32,
        binder_node: Option<u32>,
    ) -> Result<(), crate::Diagnostic> {
        let id = attempt!(self.builtin(
            definition,
            span,
            generation,
            Some(occurrence_node),
            binder_node
        ));
        code.extend(attempt!(self.wrapped(
            id,
            locals,
            hidden,
            depth,
            span,
            occurrence_node
        )));
        Ok(())
    }

    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn lookup(
        meter: &mut crate::Meter,
        locals: &[Bound],
        word: &[u8],
        span: crate::Span,
    ) -> Result<Option<usize>, crate::Diagnostic> {
        let mut at = 0;
        let mut found = None;
        let mut failure = None;
        while found.is_none() && at < locals.len() {
            if let Err(problem) = meter.charge(1, span) {
                failure = Some(problem);
                break;
            }
            found = matched_at(same_bytes(&locals[at].name, word), at);
            at += 1;
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(found),
        }
    }

    /// Project a local at physical index `which` below *all* hidden values.
    /// Each `dup` or `swap` is itself an ordinary kernel-checked builtin.
    #[expect(
        tigerstyle::ambiguous_params,
        tigerstyle::mutating_input_in_pure,
        tigerstyle::too_many_parameters,
        reason = "Owner: noble-contracts; the same-typed parameters are positional coordinates at each checked call site; a newtype would not change the call shape; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
    )]
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-contracts; it forwards the growable code buffer to operation, which appends with Vec::push."
    )]
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-contracts; the preceding which >= locals.len() refusal guarantees locals.len() - which - 1 cannot underflow."
    )]
    fn read(
        &mut self,
        code: &mut Vec<u32>,
        which: usize,
        locals: &[Bound],
        depth: u32,
        span: crate::Span,
        occurrence_node: u32,
    ) -> Result<(), crate::Diagnostic> {
        if which >= locals.len() {
            return Err(crate::internal(span));
        }
        let binder_node = Some(locals[which].binder_node);
        attempt!(self.operation(
            code,
            0,
            locals,
            which,
            depth,
            span,
            Generation::ProjectionDup,
            occurrence_node,
            binder_node
        ));
        let older_count = locals.len() - which - 1;
        let mut older = 0;
        let mut outcome = Ok(());
        while outcome.is_ok() && older < older_count {
            outcome = match which
                .checked_add(1)
                .and_then(|count| count.checked_add(older))
            {
                Some(hidden) => self.operation(
                    code,
                    2,
                    locals,
                    hidden,
                    depth,
                    span,
                    Generation::ProjectionSwap,
                    occurrence_node,
                    binder_node,
                ),
                None => Err(crate::internal(span)),
            };
            older += 1;
        }
        outcome
    }

    #[expect(
        tigerstyle::ambiguous_params,
        tigerstyle::mutating_input_in_pure,
        tigerstyle::unbounded_collection_growth,
        reason = "Owner: noble-contracts; the same-typed parameters are positional coordinates at each checked call site; a newtype would not change the call shape; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; growth is bounded by the checked input or admission limits that drive the loop."
    )]
    fn quote(
        &mut self,
        id: u32,
        outer: &[Bound],
        depth: u32,
        span: crate::Span,
    ) -> Result<Vec<u32>, crate::Diagnostic> {
        let nested = {
            let node = self
                .nodes
                .get_mut(attempt!(crate::offset(id, span)))
                .ok_or_else(|| crate::internal(span))?;
            let super::Kind::Quotation(body) = &mut node.kind else {
                return Err(crate::internal(span));
            };
            core::mem::take(body)
        };
        let mut used = alloc::vec![false; outer.len()];
        let deeper = depth
            .checked_add(1)
            .ok_or_else(|| crate::invalid(span, "lexical quotation depth overflow"))?;
        attempt!(free(
            self.nodes,
            &nested,
            outer,
            &mut Vec::new(),
            &mut used,
            self.meter,
            deeper,
            span
        ));
        let mut captured = Vec::new();
        let mut index = 0;
        let mut failure = None;
        while index < outer.len() {
            if used[index] {
                if let Err(problem) = self.meter.node(span) {
                    failure = Some(problem);
                    break;
                }
                captured.push(copy_bound(&outer[index]));
            }
            index += 1;
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        let mut lowered = Vec::new();
        let mut is_first = true;
        // `outer[0]` is the newest/top binder. Capture oldest first, so
        // replaying the composed programs reconstructs that same stack order.
        let mut index = used.len();
        let mut failure = None;
        while index > 0 {
            index -= 1;
            if used[index] {
                if let Err(problem) =
                    self.capture(&mut lowered, index, outer, depth, span, id, is_first)
                {
                    failure = Some(problem);
                    break;
                }
                is_first = false;
            }
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        let body = attempt!(self.body(&nested, captured, deeper));
        let node = self
            .nodes
            .get_mut(attempt!(crate::offset(id, span)))
            .ok_or_else(|| crate::internal(span))?;
        node.kind = super::Kind::Quotation(body);
        lowered.extend(attempt!(self.wrapped(
            id,
            outer,
            outer.len(),
            depth,
            span,
            id
        )));
        if !is_first {
            attempt!(self.operation(
                &mut lowered,
                9,
                outer,
                outer.len(),
                depth,
                span,
                Generation::CaptureCompose,
                id,
                None
            ));
        }
        Ok(lowered)
    }

    /// Capture outer binder `index` into a quotation's prefix: read it, quote
    /// it, and compose it after the previous capture unless it is the first.
    #[expect(
        clippy::too_many_arguments,
        reason = "Owner: noble-contracts; one capture step names its output code, binder, locals and owner coordinates separately; bundling them would hide which state each lowering call reads"
    )]
    #[expect(
        tigerstyle::ambiguous_params,
        tigerstyle::mutating_input_in_pure,
        tigerstyle::too_many_parameters,
        reason = "Owner: noble-contracts; the same-typed parameters are positional coordinates at each checked call site; a newtype would not change the call shape; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
    )]
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-contracts; the emitted code buffer grows with Vec::push, so it needs the growable vector."
    )]
    fn capture(
        &mut self,
        lowered: &mut Vec<u32>,
        index: usize,
        outer: &[Bound],
        depth: u32,
        span: crate::Span,
        id: u32,
        is_first: bool,
    ) -> Result<(), crate::Diagnostic> {
        let binder_node = Some(outer[index].binder_node);
        attempt!(self.read(lowered, index, outer, depth, span, id));
        attempt!(self.operation(
            lowered,
            8,
            outer,
            outer.len(),
            depth,
            span,
            Generation::CaptureQuote,
            id,
            binder_node
        ));
        if !is_first {
            attempt!(self.operation(
                lowered,
                9,
                outer,
                outer.len(),
                depth,
                span,
                Generation::CaptureCompose,
                id,
                binder_node
            ));
        }
        Ok(())
    }

    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn body(
        &mut self,
        source: &[u32],
        locals: Vec<Bound>,
        depth: u32,
    ) -> Result<Vec<u32>, crate::Diagnostic> {
        let mut state = BodyState {
            code: Vec::new(),
            locals,
            introduced: 0,
        };
        let mut at = 0usize;
        let mut outcome = Ok(());
        while outcome.is_ok() && at < source.len() {
            match self.body_entry(source, at, &mut state, depth) {
                Ok(true) => at += 2,
                Ok(false) => at += 1,
                Err(problem) => outcome = Err(problem),
            }
        }
        attempt!(outcome);
        let mut index = 0;
        let mut failure = None;
        while index < state.locals.len() {
            match self.builtin(
                1,
                crate::Span { start: 0, end: 0 },
                Generation::CleanupDrop,
                None,
                Some(state.locals[index].binder_node),
            ) {
                Ok(drop) => state.code.push(drop),
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
            index += 1;
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(state.code),
        }
    }

    /// Lower one source entry: a `let` binding (with its name) or a local
    /// read, quotation or other word. Returns whether a binding consumed two
    /// entries.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn body_entry(
        &mut self,
        source: &[u32],
        at: usize,
        state: &mut BodyState,
        depth: u32,
    ) -> Result<bool, crate::Diagnostic> {
        let id = source[at];
        let node = attempt!(source_node(
            self.nodes,
            id,
            crate::Span { start: 0, end: 0 }
        ));
        let span = node.span;
        if is_let(&node.kind) {
            attempt!(self.binding(source, at, state, depth, span));
            return Ok(true);
        }
        let local = match &node.kind {
            super::Kind::Word(word) => {
                attempt!(Self::lookup(self.meter, &state.locals, word, span))
            }
            _ => None,
        };
        if let Some(which) = local {
            self.nodes[attempt!(crate::offset(id, span))].kind = super::Kind::Quotation(Vec::new());
            attempt!(self.read(&mut state.code, which, &state.locals, depth, span, id));
        } else if matches!(&node.kind, super::Kind::Quotation(_)) {
            state
                .code
                .extend(attempt!(self.quote(id, &state.locals, depth, span)));
        } else {
            state.code.extend(attempt!(self.wrapped(
                id,
                &state.locals,
                state.locals.len(),
                depth,
                span,
                id
            )));
        }
        Ok(false)
    }

    /// Install the `let` binder at `at`: check its name, move the bound value
    /// above the hidden locals and record it as the newest binder.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-contracts; at is an in-bounds source index, so at + 1 is at most the source length and cannot overflow usize; a Vec length is below isize::MAX, so locals.len() + 1 cannot overflow usize."
    )]
    fn binding(
        &mut self,
        source: &[u32],
        at: usize,
        state: &mut BodyState,
        depth: u32,
        span: crate::Span,
    ) -> Result<(), crate::Diagnostic> {
        let id = source[at];
        let next = source
            .get(at + 1)
            .copied()
            .ok_or_else(|| crate::invalid(span, "let requires a local name"))?;
        let target = attempt!(source_node(self.nodes, next, span));
        let super::Kind::Word(name_bytes) = &target.kind else {
            return Err(crate::invalid(target.span, "let requires a local name"));
        };
        if !name(name_bytes)
            || crate::scan::find_first(&state.locals[..state.introduced], |prior| {
                prior.name == *name_bytes
            })
            .is_some()
        {
            return Err(crate::invalid(
                target.span,
                "duplicate or invalid local name",
            ));
        }
        attempt!(self.meter.depth(
            u32::try_from(state.locals.len() + 1)
                .map_err(|_| crate::invalid(span, "lexical binding depth overflow"))?,
            span
        ));
        // The new value is just below the previously hidden locals.
        // Move it above them before installing it as the newest
        // binder; `swap` moves ownership without copying or dropping.
        attempt!(self.swap_above_hidden(&mut state.code, &state.locals, (id, next), depth, span));
        let bound = core::mem::replace(
            &mut self.nodes[attempt!(crate::offset(next, span))].kind,
            super::Kind::Quotation(Vec::new()),
        );
        let super::Kind::Word(name_bytes) = bound else {
            return Err(crate::internal(span));
        };
        self.nodes[attempt!(crate::offset(id, span))].kind = super::Kind::Quotation(Vec::new());
        state.locals.insert(
            0,
            Bound {
                name: name_bytes,
                binder_node: next,
            },
        );
        state.introduced += 1;
        Ok(())
    }

    /// Move a just-bound value above every hidden local with `swap`s, from the
    /// oldest hidden local down, stopping at the first refusal. `binding` is
    /// the `let` node and its binder node.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-contracts; the emitted code buffer grows with Vec::push, so it needs the growable vector."
    )]
    fn swap_above_hidden(
        &mut self,
        code: &mut Vec<u32>,
        locals: &[Bound],
        binding: (u32, u32),
        depth: u32,
        span: crate::Span,
    ) -> Result<(), crate::Diagnostic> {
        let mut hidden = locals.len();
        let mut failure = None;
        while hidden > 0 {
            hidden -= 1;
            if let Err(problem) = self.operation(
                code,
                2,
                locals,
                hidden,
                depth,
                span,
                Generation::BindingSwap,
                binding.0,
                Some(binding.1),
            ) {
                failure = Some(problem);
                break;
            }
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }
}

/// The code, binders and binder count of one `Lower::body` walk.
struct BodyState {
    code: Vec<u32>,
    locals: Vec<Bound>,
    introduced: usize,
}

/// A copy of one binder, as captured by a quotation.
fn copy_bound(bound: &Bound) -> Bound {
    Bound {
        name: bound.name.clone(),
        binder_node: bound.binder_node,
    }
}
