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
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; it scans with a closure, pops and pushes the node arena and runs the lowering, none of which is const."
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
    let lowered = lower_body(&mut tree.nodes, meter, witnesses, body);
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
    body: Vec<u32>,
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
/// Nested quotations are walked with an explicit stack of levels rather than
/// by recursion, so no loop sits on a recursive call; the depth meter bounds
/// the stack before each level is pushed.
#[expect(
    clippy::too_many_arguments,
    reason = "Owner: noble-contracts; the lexical free-occurrence walk threads its node table, body, outer binders, shadow stack and meter explicitly; bundling them would hide which state each step reads or extends"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; the parameters are independent checked inputs and shared budget state threaded explicitly; an options struct would only rename the same borrowed fields."
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
    let walk = FreeWalk { nodes, body, outer };
    // An explicit owned push: the pinned Aeneas fails on the erased regions
    // of a `vec!` array conversion.
    let mut frames = Vec::new();
    frames.push(FreeFrame {
        quotation: None,
        cursor: 0,
        scope: shadow.len(),
        depth,
        span,
    });
    let mut outcome = Ok(false);
    while matches!(outcome, Ok(false)) {
        outcome = free_step(walk, &mut frames, shadow, used, meter);
    }
    attempt!(outcome);
    Ok(())
}

/// The tables shared by every level of one `free` walk.
#[derive(Clone, Copy)]
struct FreeWalk<'a> {
    nodes: &'a [super::Node],
    body: &'a [u32],
    outer: &'a [Bound],
}

impl<'a> FreeWalk<'a> {
    /// The entries of a level: the walk's own body, or the children of the
    /// nested quotation node `quotation`.
    fn level_body(
        self,
        quotation: Option<u32>,
        span: crate::Span,
    ) -> Result<&'a [u32], crate::Diagnostic> {
        let Some(id) = quotation else {
            return Ok(self.body);
        };
        let node = attempt!(source_node(self.nodes, id, span));
        let super::Kind::Quotation(children) = &node.kind else {
            return Err(crate::internal(span));
        };
        Ok(children)
    }
}

/// One pending quotation level of the `free` walk, without borrows: the
/// quotation node whose body it walks (`None` for the walk's own body), its
/// next entry, the shadow height at which its own binders start, and its
/// depth and span.
#[derive(Clone, Copy)]
struct FreeFrame {
    quotation: Option<u32>,
    cursor: usize,
    scope: usize,
    depth: u32,
    span: crate::Span,
}

/// What one checked entry of a `free` level asks the walk to do next.
enum FreeEntry {
    /// Continue with the following entry.
    Next,
    /// A `let` binder consumed its name too: skip both entries.
    Binder,
    /// Walk the body of the nested quotation node first, at `depth`.
    Nested {
        quotation: u32,
        depth: u32,
        span: crate::Span,
    },
}

/// Advance the `free` walk by one entry of the innermost level, or finish
/// that level and drop its binders. Returns whether the walk is complete.
/// Entries, metering and errors follow the recursive walk's order exactly.
#[expect(
    tigerstyle::assertion_density,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; invalid input is rejected through typed outcomes rather than assertions, and the invariants are enforced by callers and the crate test suite; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the level stack and the shadow scope stack are pushed and popped by the walk, so they need growable vectors."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; it pushes and pops the walk's vectors and charges the meter, none of which is const."
)]
fn free_step(
    walk: FreeWalk<'_>,
    frames: &mut Vec<FreeFrame>,
    shadow: &mut Vec<Vec<u8>>,
    used: &mut [bool],
    meter: &mut crate::Meter,
) -> Result<bool, crate::Diagnostic> {
    let Some(mut frame) = frames.pop() else {
        return Ok(true);
    };
    let entries = attempt!(walk.level_body(frame.quotation, frame.span));
    if frame.cursor >= entries.len() {
        shadow.truncate(frame.scope);
        return Ok(frames.is_empty());
    }
    let level = FreeLevel {
        nodes: walk.nodes,
        body: entries,
        outer: walk.outer,
        scope: frame.scope,
        depth: frame.depth,
        span: frame.span,
    };
    match attempt!(free_entry(level, frame.cursor, shadow, used, meter)) {
        FreeEntry::Next => {
            frame.cursor += 1;
            frames.push(frame);
        }
        FreeEntry::Binder => {
            frame.cursor += 2;
            frames.push(frame);
        }
        FreeEntry::Nested {
            quotation,
            depth,
            span,
        } => {
            frame.cursor += 1;
            frames.push(frame);
            attempt!(meter.depth(depth, span));
            frames.push(FreeFrame {
                quotation: Some(quotation),
                cursor: 0,
                scope: shadow.len(),
                depth,
                span,
            });
        }
    }
    Ok(false)
}

/// One quotation level of the `free` walk as seen by a single entry: its
/// tables, its body and the shadow height at which the level's own binders
/// start.
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
/// word occurrence or a nested quotation, which the caller walks next.
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
) -> Result<FreeEntry, crate::Diagnostic> {
    attempt!(meter.charge(1, level.span));
    let id = level.body[at];
    let node = attempt!(source_node(level.nodes, id, level.span));
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
        return Ok(FreeEntry::Binder);
    }
    match &node.kind {
        super::Kind::Word(word) => {
            if !attempt!(shadowed(shadow, word, meter, node.span)) {
                attempt!(mark_outer_use(level.outer, word, used, meter, node.span));
            }
            Ok(FreeEntry::Next)
        }
        super::Kind::Quotation(_) => {
            let deeper = level
                .depth
                .checked_add(1)
                .ok_or_else(|| crate::invalid(node.span, "lexical quotation depth overflow"))?;
            Ok(FreeEntry::Nested {
                quotation: id,
                depth: deeper,
                span: node.span,
            })
        }
        _ => Ok(FreeEntry::Next),
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
        // Swap the copy above each older hidden local, `which + 1` up to the
        // oldest. Every iteration makes the same single call, so no branch
        // in the loop decides whether `self` is borrowed.
        let Some(mut older) = which.checked_add(1) else {
            return Err(crate::internal(span));
        };
        let mut failure = None;
        while older < locals.len() {
            if let Err(problem) = self.operation(
                code,
                2,
                locals,
                older,
                depth,
                span,
                Generation::ProjectionSwap,
                occurrence_node,
                binder_node,
            ) {
                failure = Some(problem);
                break;
            }
            older += 1;
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }

    /// Open quotation `id`: take its body out of the arena, find the outer
    /// binders it uses, charge and copy them, and emit the code capturing
    /// them oldest first. The returned level lowers the body with those
    /// binders; `close_quote` finishes the quotation once it is lowered.
    #[expect(
        tigerstyle::ambiguous_params,
        tigerstyle::mutating_input_in_pure,
        tigerstyle::unbounded_collection_growth,
        reason = "Owner: noble-contracts; the same-typed parameters are positional coordinates at each checked call site; a newtype would not change the call shape; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable; growth is bounded by the checked input or admission limits that drive the loop."
    )]
    fn open_quote(
        &mut self,
        id: u32,
        outer: &[Bound],
        depth: u32,
        span: crate::Span,
    ) -> Result<BodyLevel, crate::Diagnostic> {
        // Explicit matches: a mutable node borrow must not pass through a std
        // combinator for the extraction.
        let arena_index = attempt!(crate::offset(id, span));
        let nested = {
            let node = match self.nodes.get_mut(arena_index) {
                Some(node) => node,
                None => return Err(crate::internal(span)),
            };
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
        Ok(BodyLevel {
            entries: nested,
            state: BodyState {
                code: Vec::new(),
                locals: captured,
                introduced: 0,
            },
            cursor: 0,
            depth: deeper,
            pending: Some(PendingQuote {
                id,
                arena_index,
                depth,
                span,
                lowered,
                is_first,
            }),
        })
    }

    /// Finish a quotation whose body was lowered to `code`: store the body
    /// back in the arena, wrap the quotation for the enclosing binders
    /// `outer`, and compose it after its captures.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; it writes the node arena and emits checked code through non-const helpers."
    )]
    fn close_quote(
        &mut self,
        pending: PendingQuote,
        code: Vec<u32>,
        outer: &[Bound],
    ) -> Result<Vec<u32>, crate::Diagnostic> {
        let PendingQuote {
            id,
            arena_index,
            depth,
            span,
            mut lowered,
            is_first,
        } = pending;
        let node = match self.nodes.get_mut(arena_index) {
            Some(node) => node,
            None => return Err(crate::internal(span)),
        };
        node.kind = super::Kind::Quotation(code);
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

    /// Lower a body and every quotation nested in it. Nested bodies are kept
    /// on an explicit stack of levels instead of a recursive call, so no loop
    /// sits on a recursion; entries, metering and errors keep the order of
    /// the recursive lowering exactly.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn body(
        &mut self,
        entries: Vec<u32>,
        locals: Vec<Bound>,
        depth: u32,
    ) -> Result<Vec<u32>, crate::Diagnostic> {
        // An explicit owned push: the pinned Aeneas fails on the erased
        // regions of a `vec!` array conversion.
        let mut levels = Vec::new();
        levels.push(BodyLevel {
            entries,
            state: BodyState {
                code: Vec::new(),
                locals,
                introduced: 0,
            },
            cursor: 0,
            depth,
            pending: None,
        });
        let mut outcome = Ok(None);
        while matches!(outcome, Ok(None)) {
            outcome = self.lower_step(&mut levels);
        }
        match outcome {
            Ok(Some(code)) => Ok(code),
            Ok(None) => Err(crate::internal(crate::Span { start: 0, end: 0 })),
            Err(problem) => Err(problem),
        }
    }

    /// One step of `body`: lower the next entry of the innermost level, or
    /// open a nested quotation as a new level; once a level's entries are
    /// done, drop its binders and hand its code to the quotation it belongs
    /// to, or return it when it is the outermost body.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-contracts; levels are pushed and popped as quotation bodies open and close, so the stack needs the growable vector."
    )]
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; it moves levels through the stack vector and emits checked code through non-const helpers."
    )]
    fn lower_step(
        &mut self,
        levels: &mut Vec<BodyLevel>,
    ) -> Result<Option<Vec<u32>>, crate::Diagnostic> {
        let Some(mut level) = levels.pop() else {
            return Err(crate::internal(crate::Span { start: 0, end: 0 }));
        };
        if level.cursor < level.entries.len() {
            match attempt!(self.body_entry(
                &level.entries,
                level.cursor,
                &mut level.state,
                level.depth
            )) {
                EntryStep::Next => level.cursor += 1,
                EntryStep::Binder => level.cursor += 2,
                EntryStep::Quotation { id, span } => {
                    level.cursor += 1;
                    let nested =
                        attempt!(self.open_quote(id, &level.state.locals, level.depth, span));
                    levels.push(level);
                    levels.push(nested);
                    return Ok(None);
                }
            }
            levels.push(level);
            return Ok(None);
        }
        let code = attempt!(self.cleanup(level.state));
        let Some(pending) = level.pending else {
            return Ok(Some(code));
        };
        let Some(mut parent) = levels.pop() else {
            return Err(crate::internal(pending.span));
        };
        let lowered = attempt!(self.close_quote(pending, code, &parent.state.locals));
        parent.state.code.extend(lowered);
        levels.push(parent);
        Ok(None)
    }

    /// Drop every binder of a finished body, newest first, after its code.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn cleanup(&mut self, mut state: BodyState) -> Result<Vec<u32>, crate::Diagnostic> {
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
    /// read or other word. A quotation is left to the caller, which opens it
    /// as a nested level.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    fn body_entry(
        &mut self,
        entries: &[u32],
        at: usize,
        state: &mut BodyState,
        depth: u32,
    ) -> Result<EntryStep, crate::Diagnostic> {
        let id = entries[at];
        let node = attempt!(source_node(
            self.nodes,
            id,
            crate::Span { start: 0, end: 0 }
        ));
        let span = node.span;
        if is_let(&node.kind) {
            attempt!(self.binding(entries, at, state, depth, span));
            return Ok(EntryStep::Binder);
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
            return Ok(EntryStep::Quotation { id, span });
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
        Ok(EntryStep::Next)
    }

    /// Install the `let` binder at `at`: check its name, move the bound value
    /// above the hidden locals and record it as the newest binder.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it mutates only exclusively borrowed, caller-owned work state for this bounded pass; checked inputs stay immutable."
    )]
    #[expect(
        tigerstyle::raw_arithmetic_overflow,
        reason = "Owner: noble-contracts; at is an in-bounds entry index, so at + 1 is at most the entry count and cannot overflow usize; a Vec length is below isize::MAX, so locals.len() + 1 cannot overflow usize."
    )]
    fn binding(
        &mut self,
        entries: &[u32],
        at: usize,
        state: &mut BodyState,
        depth: u32,
        span: crate::Span,
    ) -> Result<(), crate::Diagnostic> {
        let id = entries[at];
        let next = entries
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

/// The code, binders and binder count of one body being lowered.
struct BodyState {
    code: Vec<u32>,
    locals: Vec<Bound>,
    introduced: usize,
}

/// One body on the `Lower::body` stack: its entries, lowering state, next
/// entry and depth, and for a nested quotation body the quotation to finish
/// once the body is lowered.
struct BodyLevel {
    entries: Vec<u32>,
    state: BodyState,
    cursor: usize,
    depth: u32,
    pending: Option<PendingQuote>,
}

/// A quotation whose body is being lowered: its node and arena slot, its own
/// depth and span, the code capturing its free binders, and whether that
/// code captured nothing.
struct PendingQuote {
    id: u32,
    arena_index: usize,
    depth: u32,
    span: crate::Span,
    lowered: Vec<u32>,
    is_first: bool,
}

/// How the lowering continues after one body entry.
enum EntryStep {
    /// Continue with the following entry.
    Next,
    /// A `let` binding consumed its name too: skip both entries.
    Binder,
    /// Open the quotation node `id` as a nested level.
    Quotation { id: u32, span: crate::Span },
}

/// A copy of one binder, as captured by a quotation.
fn copy_bound(bound: &Bound) -> Bound {
    Bound {
        name: bound.name.clone(),
        binder_node: bound.binder_node,
    }
}

#[cfg(test)]
mod tests {
    /// Binders and captures at several quotation depths, shadowing, a root
    /// body without `def`, and malformed binders.
    const PROGRAMS: [&[u8]; 12] = [
        b"def a [ let x x ]",
        b"def b [ let x [ x ] ]",
        b"def c [ let x let y [ y [ x y ] ] x ]",
        b"def d [ let x [ let y [ x y ] y ] ]",
        b"def e [ let x [ let x x ] x ]",
        b"def f [ 1 let x [ [ [ x ] ] ] run ]",
        b"def g [ let a let b let c [ c b a ] [ a ] ]",
        b"def h [ let ]",
        b"def i [ let 1 ]",
        b"def j [ let x let x x ]",
        b"def k [ [ let y let y ] ]",
        b"2 let x [ x x + ] x",
    ];

    /// Lower `text` with a fresh meter under `limits` and render the outcome,
    /// the lowered tree, the witnesses and the work spent.
    fn render(text: &[u8], limits: crate::Limits) -> alloc::string::String {
        let mut parse_meter = crate::Meter::new(crate::Limits::default());
        let (mut tree, _) = super::super::parsing::parse(text, &mut parse_meter).unwrap();
        let mut meter = crate::Meter::new(limits);
        let mut witnesses = alloc::vec::Vec::new();
        let outcome = super::lower_with_witness(&mut tree, &mut meter, &mut witnesses);
        alloc::format!("{outcome:?} {tree:?} {witnesses:?} {}\n", meter.spent())
    }

    /// FNV-1a of `text`, continuing from `hash`.
    fn fnv(hash: u64, text: &str) -> u64 {
        let mut next = hash;
        for byte in text.bytes() {
            next = (next ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
        }
        next
    }

    /// The lowering keeps nested bodies and free-occurrence levels on explicit
    /// stacks. Under every work, node and depth budget it reproduces the
    /// recursive lowering it replaced: the digest was computed from that
    /// implementation (commit fc2eb6c6) on the same programs and budgets.
    #[test]
    fn explicit_level_lowering_matches_the_recursive_lowering_under_every_budget() {
        let full = crate::Limits::default();
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for text in PROGRAMS {
            hash = fnv(hash, &render(text, full));
            for work in 0..=400 {
                hash = fnv(hash, &render(text, crate::Limits { work, ..full }));
            }
            for nodes in 0..=64 {
                hash = fnv(hash, &render(text, crate::Limits { nodes, ..full }));
            }
            for depth in 0..=6 {
                hash = fnv(hash, &render(text, crate::Limits { depth, ..full }));
            }
        }
        assert_eq!(hash, 18_307_575_468_579_584_411);
    }
}
