//! Editor-only syntax transport. No editor node is a kernel candidate or a
//! `.noble` token. An executable tree is re-parsed by normal source admission.

#[expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-contracts; the module imports these domain types directly, and the qualified form at every use site would add noise without changing the dependency"
)]
use alloc::{string::String, vec::Vec};

pub const FORMAT: u32 = 1;

pub enum Node {
    Integer(i64),
    Boolean(bool),
    Word(String),
    Quotation(Vec<Node>),
    Hole,
}

// `Node` is recursive through `Vec<Node>`, so derived `Clone` and `Debug`
// impls refer to their own trait impl before the extracted Lean defines it.
// The copy is a plain function with exactly the derived result; the rendering
// is the one reviewed opaque formatting boundary (see `debug_node`).
impl Clone for Node {
    fn clone(&self) -> Self {
        clone_node(self)
    }
}

impl core::fmt::Debug for Node {
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; `Debug` renders by writing through the caller's formatter, which is the operation"
    )]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        debug_node(self, f)
    }
}

/// Deep-copy one node, variant by variant, as the derived `Clone` did.
fn clone_node(node: &Node) -> Node {
    match node {
        Node::Integer(value) => Node::Integer(*value),
        Node::Boolean(value) => Node::Boolean(*value),
        Node::Word(word) => Node::Word(word.clone()),
        Node::Quotation(children) => {
            let mut copy = Vec::with_capacity(children.len());
            copy_nodes(children, &mut copy);
            Node::Quotation(copy)
        }
        Node::Hole => Node::Hole,
    }
}

/// Append copies of `nodes`, in order, as `Vec::clone` did. The list is halved
/// instead of looped over, so no loop sits on the call cycle with
/// `clone_node`, and the added depth is logarithmic in the list length.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; each call halves the list, so the depth is logarithmic in its length on top of the quotation nesting the derived copy followed too."
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it appends only to the copy its caller is building."
)]
#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-contracts; the copy under construction grows with Vec::push, so it needs the growable vector."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; copying a node allocates and pushes, which is not const."
)]
fn copy_nodes(nodes: &[Node], copy: &mut Vec<Node>) {
    if nodes.is_empty() {
        return;
    }
    let half = nodes.len() / 2;
    if half == 0 {
        copy.push(clone_node(&nodes[0]));
        return;
    }
    copy_nodes(&nodes[..half], copy);
    copy_nodes(&nodes[half..], copy);
}

/// Render a node exactly as `#[derive(Debug)]` did.
///
/// Reviewed opaque extraction boundary: rendering a `Quotation` must format
/// the nested nodes through `Node`'s own `Debug` impl, so no transparent body
/// can avoid referring to that impl before the extracted Lean defines it.
/// Formatting layout is already an abstracted trust boundary.
#[charon::opaque]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; the helper writes through the caller's formatter, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; rendering writes through the caller's formatter, which is the operation"
)]
fn debug_node(node: &Node, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match node {
        Node::Integer(value) => f.debug_tuple("Integer").field(value).finish(),
        Node::Boolean(value) => f.debug_tuple("Boolean").field(value).finish(),
        Node::Word(word) => f.debug_tuple("Word").field(word).finish(),
        Node::Quotation(children) => f.debug_tuple("Quotation").field(children).finish(),
        Node::Hole => f.write_str("Hole"),
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub format: u32,
    pub nodes: Vec<Node>,
}

#[derive(Debug)]
pub struct HoleConstraint {
    pub input_stack: String,
    pub output_stack: String,
    /// Additional guaranteed suffix if the known incoming suffix is retained;
    /// the editor hole may instead consume it, so no preservation is inferred.
    pub required_output_suffix: Vec<String>,
    pub known_effects: Vec<u32>,
    pub unresolved_effect: bool,
}

#[derive(Debug)]
pub struct Analysis {
    pub holes: Vec<HoleConstraint>,
    pub known_effects: Vec<u32>,
    pub unresolved_effect: bool,
}

struct Builder {
    tree: super::Tree,
    source: Vec<u8>,
    holes: usize,
    /// The generated span of the first hole. An admission refusal is located there rather than at
    /// the whole tree, so a client learns which node to fix and a nested hole maps to its own path.
    first_hole: Option<crate::Span>,
    /// One entry per generated node: its synthetic span and the bounded editor
    /// path that produced it. Paths are indices into the editor node tree, so
    /// they are stable within one input and never claim to be source offsets.
    paths: Vec<(crate::Span, Vec<u32>)>,
}

/// Owned build state: each step consumes and returns it, so no caller-owned
/// value is mutated through a borrowed capability.
struct Emitter {
    builder: Builder,
    meter: crate::Meter,
}

/// One open editor node list on the explicit work stack: its own index in the
/// enclosing list, the next node to emit, the generated offset of its opening
/// bracket, its depth and the first value it contributes. A frame holds no
/// borrow; its list is found again from the root through the frame indexes.
struct Frame {
    index: u32,
    next: usize,
    start: u32,
    depth: u32,
    value_start: usize,
}

/// The node list of the innermost of `path`: the root list followed down
/// through each enclosing quotation by its index. The first frame is the node
/// list itself rather than an editor node, so callers pass the frames below
/// it; the depth meter bounds their number before any frame is pushed.
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-contracts; the walk follows the frame path, whose length the configured depth limit bounds before each frame is pushed; the pinned Aeneas accepts no loop that carries a borrow into the editor tree, and a frame stack that holds borrows is refused too."
)]
#[expect(
    tigerstyle::platform_dependent_cast,
    reason = "Owner: noble-contracts; the operand is a u32 node index and the cast is a widening that cannot fail where usize is at least 32 bits."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; checked slice lookup is not const."
)]
fn frame_nodes<'a>(nodes: &'a [Node], path: &[Frame]) -> Option<&'a [Node]> {
    match path.split_first() {
        None => Some(nodes),
        Some((frame, rest)) => match nodes.get(frame.index as usize) {
            Some(Node::Quotation(children)) => frame_nodes(children, rest),
            Some(Node::Integer(_)) => None,
            Some(Node::Boolean(_)) => None,
            Some(Node::Word(_)) => None,
            Some(Node::Hole) => None,
            None => None,
        },
    }
}

/// The editor path of a node: the indexes of the frames below the root list,
/// then its own index.
fn frame_path(frames: &[Frame], index: u32) -> Vec<u32> {
    let mut path = Vec::with_capacity(frames.len());
    let mut at = 1;
    while at < frames.len() {
        path.push(frames[at].index);
        at += 1;
    }
    path.push(index);
    path
}

impl Emitter {
    fn append(mut self, bytes: &[u8]) -> Result<Self, crate::Diagnostic> {
        let builder = &mut self.builder;
        let end = attempt!(crate::index(
            builder.source.len().saturating_add(bytes.len()),
            builder.tree.span
        ));
        if end >= self.meter.limits.bytes {
            return Err(super::exhausted(
                builder.tree.span,
                "editor source byte limit exceeded",
            ));
        }
        attempt!(self.meter.charge(
            attempt!(crate::index(bytes.len(), builder.tree.span)),
            builder.tree.span
        ));
        builder.source.extend_from_slice(bytes);
        builder.source.push(b' ');
        Ok(self)
    }

    fn body(mut self, nodes: &[Node]) -> Result<(Self, Vec<u32>), crate::Diagnostic> {
        let span = self.builder.tree.span;
        attempt!(self.meter.depth(0, span));
        let mut frames = alloc::vec![Frame {
            index: 0,
            next: 0,
            start: 0,
            depth: 0,
            value_start: 0,
        }];
        let mut values = Vec::new();
        let mut current = Ok((self, false));
        while let Ok((emitter, false)) = current {
            current = emitter.step(nodes, &mut frames, &mut values);
        }
        match current {
            Ok((emitter, _)) => Ok((emitter, values)),
            Err(problem) => Err(problem),
        }
    }

    /// One step of the explicit work stack: emit the next node of the
    /// innermost list, open a nested quotation, or close a finished list.
    /// Returns whether the root list is complete.
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-contracts; the frame stack and the emitted value list grow and shrink with Vec push, pop and split_off, so they need growable vectors."
    )]
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it advances only the frame stack and value list owned by Emitter::body for this bounded pass."
    )]
    fn step(
        mut self,
        nodes: &[Node],
        frames: &mut Vec<Frame>,
        values: &mut Vec<u32>,
    ) -> Result<(Self, bool), crate::Diagnostic> {
        let span = self.builder.tree.span;
        let Some(frame) = frames.last() else {
            return Err(crate::internal(span));
        };
        let Some(list) = frame_nodes(nodes, &frames[1..]) else {
            return Err(crate::internal(span));
        };
        if frame.next == list.len() {
            return self.close(nodes, frames, values);
        }
        let at = frame.next;
        let frame_depth = frame.depth;
        let value_start = frame.value_start;
        let node = &list[at];
        if let Some(top) = frames.last_mut() {
            top.next += 1;
        }
        attempt!(self.meter.node(self.builder.tree.span));
        let start = attempt!(crate::index(
            self.builder.source.len(),
            self.builder.tree.span
        ));
        let kind = match node {
            Node::Integer(value) => {
                let text = alloc::format!("{value}");
                self = attempt!(self.append(text.as_bytes()));
                super::Kind::Literal(noble_kernel::untrusted::Lit::I64(*value))
            }
            Node::Boolean(value) => {
                self = attempt!(self.append(if *value { b"true" } else { b"false" }));
                super::Kind::Literal(noble_kernel::untrusted::Lit::Bool(*value))
            }
            Node::Word(word) => {
                let span = crate::Span { start, end: start };
                if !matches!(
                    attempt!(super::lexer::tokens::classify(
                        word.as_bytes(),
                        span,
                        &mut self.meter,
                        false
                    )),
                    super::lexer::TokenKind::Word(_)
                ) {
                    return Err(crate::invalid(
                        span,
                        "editor word is not source word syntax",
                    ));
                }
                self = attempt!(self.append(word.as_bytes()));
                super::Kind::Word(word.as_bytes().to_vec())
            }
            Node::Hole => {
                self.builder.holes += 1;
                self = attempt!(self.append(b"@editor-hole"));
                super::Kind::EditorHole
            }
            Node::Quotation(_) => {
                let depth = frame_depth.saturating_add(1);
                let index = at as u32;
                self = attempt!(self.append(b"["));
                attempt!(self.meter.depth(depth, span));
                frames.push(Frame {
                    index,
                    next: 0,
                    start,
                    depth,
                    value_start: values.len(),
                });
                return Ok((self, false));
            }
        };
        let id = attempt!(crate::index(
            self.builder.tree.nodes.len(),
            self.builder.tree.span
        ));
        let end = attempt!(crate::index(
            self.builder.source.len(),
            self.builder.tree.span
        ));
        // The node budget is charged above, and each input contributes at
        // most one body entry; do not reserve for rejected candidates.
        let Some(body_items) = values.len().checked_sub(value_start) else {
            return Err(crate::internal(span));
        };
        if body_items >= list.len() {
            return Err(super::exhausted(
                self.builder.tree.span,
                "editor node limit exceeded",
            ));
        }
        if matches!(kind, super::Kind::EditorHole) && self.builder.first_hole.is_none() {
            self.builder.first_hole = Some(crate::Span { start, end });
        }
        let leaf_path = frame_path(frames, at as u32);
        self.builder
            .paths
            .push((crate::Span { start, end }, leaf_path));
        self.builder.tree.nodes.push(super::Node {
            kind,
            span: crate::Span { start, end },
        });
        if values.len() >= self.builder.tree.nodes.len() {
            return Err(crate::internal(span));
        }
        values.push(id);
        Ok((self, false))
    }

    /// Close the innermost finished list: pop its frame and, unless it is the
    /// root list, emit its closing bracket, record its node and path, and count
    /// it as one value of the enclosing list. Returns whether the root list
    /// is complete.
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-contracts; the frame stack and the emitted value list grow and shrink with Vec push, pop and split_off, so they need growable vectors."
    )]
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it advances only the frame stack and value list owned by Emitter::body for this bounded pass."
    )]
    fn close(
        mut self,
        nodes: &[Node],
        frames: &mut Vec<Frame>,
        values: &mut Vec<u32>,
    ) -> Result<(Self, bool), crate::Diagnostic> {
        let span = self.builder.tree.span;
        let Some(frame) = frames.pop() else {
            return Err(crate::internal(span));
        };
        let quotation_path = frame_path(frames, frame.index);
        let Some(parent) = frames.last() else {
            return Ok((self, true));
        };
        let parent_value_start = parent.value_start;
        let Some(parent_list) = frame_nodes(nodes, &frames[1..]) else {
            return Err(crate::internal(span));
        };
        self = attempt!(self.append(b"]"));
        let id = attempt!(crate::index(self.builder.tree.nodes.len(), span));
        let end = attempt!(crate::index(self.builder.source.len(), span));
        let quotation_span = crate::Span {
            start: frame.start,
            end,
        };
        self.builder.paths.push((quotation_span, quotation_path));
        self.builder.tree.nodes.push(super::Node {
            kind: super::Kind::Quotation(values.split_off(frame.value_start)),
            span: quotation_span,
        });
        let Some(parent_items) = values.len().checked_sub(parent_value_start) else {
            return Err(crate::internal(span));
        };
        if parent_items >= parent_list.len() {
            return Err(super::exhausted(span, "editor node limit exceeded"));
        }
        if values.len() >= self.builder.tree.nodes.len() {
            return Err(crate::internal(span));
        }
        values.push(id);
        Ok((self, false))
    }
}

impl Candidate {
    fn build(&self, limits: crate::Limits) -> Result<(Builder, crate::Meter), super::Error> {
        let span = crate::Span { start: 0, end: 0 };
        if self.format != FORMAT {
            return Err(super::Error::at(
                super::Stage::Parse,
                crate::Diagnostic::new(
                    crate::DiagnosticKind::Unsupported,
                    span,
                    "unsupported editor format",
                ),
            ));
        }
        let emitter = Emitter {
            builder: Builder {
                tree: super::Tree {
                    nodes: Vec::new(),
                    body: Vec::new(),
                    span,
                },
                source: Vec::new(),
                holes: 0,
                first_hole: None,
                paths: Vec::new(),
            },
            meter: crate::Meter::new(limits),
        };
        let (Emitter { mut builder, meter }, body) = attempt!(emitter
            .body(&self.nodes)
            .map_err(|error| super::Error::at(super::Stage::Parse, error)));
        builder.tree.body = body;
        builder.tree.span.end = attempt!(crate::index(builder.source.len(), span)
            .map_err(|error| super::Error::at(super::Stage::Parse, error)));
        Ok((builder, meter))
    }

    /// The generated-span to editor-node-path index for this candidate.
    ///
    /// These spans are offsets into the synthetic editor source, never into a
    /// `.noble` file or the caller's JSON. The mapping is rebuilt from the same
    /// emitter that produced the source, so a span the emitter never produced
    /// has no path and is reported as unmapped rather than guessed.
    pub fn editor_paths(
        &self,
        limits: crate::Limits,
    ) -> Result<Vec<(crate::Span, Vec<u32>)>, super::Error> {
        let (builder, _) = attempt!(self.build(limits));
        Ok(builder.paths)
    }

    /// Only a hole-free tree can become source bytes. The ordinary source
    /// parser, inference, kernel and compiler remain the sole admission route.
    pub fn admitted_source(&self, limits: crate::Limits) -> Result<Vec<u8>, super::Error> {
        let (builder, _) = attempt!(self.build(limits));
        if builder.holes != 0 {
            // Located at the first hole's own generated span, not at the whole tree.
            let span = builder.first_hole.unwrap_or(builder.tree.span);
            return Err(super::Error::at(
                super::Stage::Acceptance,
                crate::invalid(span, "editor hole cannot enter source admission"),
            ));
        }
        Ok(builder.source)
    }
}

impl super::Session {
    /// Analyze incomplete editor syntax against the *same* namespace and word
    /// schemes as source preparation, without emitting or admitting a candidate.
    pub fn analyze_editor(
        &self,
        candidate: &Candidate,
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<Analysis, super::Error> {
        self.analyze_editor_metered(candidate, inputs, limits)
            .map(|(analysis, _)| analysis)
    }

    /// Analyze an editor tree and report its consumed frontend work so a
    /// caller-owned bounded service can debit the same cumulative budget.
    pub(crate) fn analyze_editor_metered(
        &self,
        candidate: &Candidate,
        inputs: &[noble_kernel::types::Ty],
        limits: crate::Limits,
    ) -> Result<(Analysis, u32), super::Error> {
        let (mut builder, mut meter) = attempt!(candidate.build(limits));
        let result = self.analyze_built(&mut builder, inputs, &mut meter);
        let work_used = meter.spent();
        match result {
            Ok(analysis) => Ok((analysis, work_used)),
            Err(mut error) => {
                error.work_used = work_used;
                Err(error)
            }
        }
    }

    /// Resolve, preflight and infer one built editor tree, then describe each
    /// hole and the root effect in source order.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it resolves only the freshly built editor tree and charges the analysis-owned meter."
    )]
    fn analyze_built(
        &self,
        builder: &mut Builder,
        inputs: &[noble_kernel::types::Ty],
        meter: &mut crate::Meter,
    ) -> Result<Analysis, super::Error> {
        if let Err(error) = super::resolution::resolve(&mut builder.tree, None, self, false, meter)
        {
            return Err(super::Error::at(super::Stage::Resolve, error));
        }
        if let Err(error) = super::preflight::check(inputs, self, builder.tree.span, meter) {
            return Err(super::Error::at(super::Stage::Check, error));
        }
        let environment = attempt!(self
            .environment()
            .map_err(|error| super::Error::at(super::Stage::Check, error)));
        let inferred = match &environment {
            alloc::borrow::Cow::Borrowed(environment) => {
                self.infer_editor(&builder.tree, environment, inputs, meter)
            }
            alloc::borrow::Cow::Owned(environment) => {
                self.infer_editor(&builder.tree, environment, inputs, meter)
            }
        };
        let state = attempt!(inferred);
        let mut holes = Vec::with_capacity(state.holes.len());
        let mut at = 0;
        let mut failure = None;
        while at < state.holes.len() {
            match hole_constraint(&state, &state.holes[at], meter) {
                Ok(constraint) => holes.push(constraint),
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
            at += 1;
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        let root = match state.bodies.first() {
            Some(root) => root,
            None => {
                return Err(super::Error::at(
                    super::Stage::Check,
                    crate::internal(builder.tree.span),
                ))
            }
        };
        let (known_effects, unresolved_effect) = attempt!(state
            .arena
            .editor_effect(root.effect, root.span, meter)
            .map_err(|error| super::Error::at(super::Stage::Check, error)));
        Ok(Analysis {
            holes,
            known_effects,
            unresolved_effect,
        })
    }

    /// Infer the editor tree in the session environment.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-contracts; it charges only the analysis-owned meter."
    )]
    fn infer_editor(
        &self,
        root: &super::Tree,
        environment: &noble_kernel::contracts::Env,
        inputs: &[noble_kernel::types::Ty],
        meter: &mut crate::Meter,
    ) -> Result<super::inference::State, super::Error> {
        let scope = super::inference::Scope {
            root,
            session: self,
            environment,
            root_signature: None,
            prospective_owner: None,
        };
        super::inference::infer(scope, super::inference::Mode::EditorAnalysis, inputs, meter)
            .map_err(|error| super::Error::at(super::Stage::Check, error))
    }
}

/// Describe one hole: both stacks, the output suffix beyond the incoming
/// suffix it shares, and its effect.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; each description step returns its typed checker diagnostic rather than asserting."
)]
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-contracts; it charges only the analysis-owned meter."
)]
fn hole_constraint(
    state: &super::inference::State,
    hole: &super::inference::Hole,
    meter: &mut crate::Meter,
) -> Result<HoleConstraint, super::Error> {
    let input_stack = attempt!(state
        .arena
        .describe_stack(hole.input, hole.span, meter)
        .map_err(|error| super::Error::at(super::Stage::Check, error)));
    let output_stack = attempt!(state
        .arena
        .describe_stack(hole.output, hole.span, meter)
        .map_err(|error| super::Error::at(super::Stage::Check, error)));
    let incoming = attempt!(state
        .arena
        .editor_stack_suffix(hole.input, hole.span, meter)
        .map_err(|error| super::Error::at(super::Stage::Check, error)));
    let outgoing = attempt!(state
        .arena
        .editor_stack_suffix(hole.output, hole.span, meter)
        .map_err(|error| super::Error::at(super::Stage::Check, error)));
    let common = common_prefix(&incoming, &outgoing);
    let required_output_suffix = outgoing[common..].to_vec();
    let (known_effects, unresolved_effect) = attempt!(state
        .arena
        .editor_effect(hole.effect, hole.span, meter)
        .map_err(|error| super::Error::at(super::Stage::Check, error)));
    Ok(HoleConstraint {
        input_stack,
        output_stack,
        required_output_suffix,
        known_effects,
        unresolved_effect,
    })
}

/// The number of leading entries two stack descriptions share, as
/// `zip(..).take_while(equal).count()`.
fn common_prefix(left: &[String], right: &[String]) -> usize {
    let mut common = 0;
    while common < left.len()
        && common < right.len()
        && same_text(left[common].as_str(), right[common].as_str())
    {
        common += 1;
    }
    common
}

/// Text equality, called from `common_prefix` so that its loop compares no
/// indexed element directly.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; str comparison is not const."
)]
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-contracts; equality is symmetric, so the two descriptions cannot be swapped by accident."
)]
fn same_text(left: &str, right: &str) -> bool {
    left == right
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape of `Node` with the derived `Debug` that its hand-written
    /// rendering must reproduce byte for byte.
    #[derive(Debug)]
    #[expect(
        dead_code,
        reason = "the payloads exist only to be rendered by the derived Debug under comparison"
    )]
    enum Derived {
        Integer(i64),
        Boolean(bool),
        Word(String),
        Quotation(Vec<Derived>),
        Hole,
    }

    fn derived(node: &Node) -> Derived {
        match node {
            Node::Integer(value) => Derived::Integer(*value),
            Node::Boolean(value) => Derived::Boolean(*value),
            Node::Word(word) => Derived::Word(word.clone()),
            Node::Quotation(children) => Derived::Quotation(children.iter().map(derived).collect()),
            Node::Hole => Derived::Hole,
        }
    }

    /// Every variant, an empty and a nested quotation, and a word that needs
    /// escaping.
    fn sample() -> Vec<Node> {
        alloc::vec![
            Node::Integer(-7),
            Node::Boolean(true),
            Node::Word(String::from("dup \"x\"\n")),
            Node::Quotation(alloc::vec![
                Node::Hole,
                Node::Quotation(Vec::new()),
                Node::Quotation(alloc::vec![
                    Node::Boolean(false),
                    Node::Integer(i64::MIN),
                    Node::Hole,
                ]),
            ]),
            Node::Hole,
        ]
    }

    /// Whether every list of `node` holds exactly its length, as `Vec::clone`
    /// leaves it.
    fn exactly_sized(node: &Node) -> bool {
        match node {
            Node::Quotation(children) => {
                children.capacity() == children.len() && children.iter().all(exactly_sized)
            }
            Node::Integer(_) | Node::Boolean(_) | Node::Word(_) | Node::Hole => true,
        }
    }

    #[test]
    fn node_rendering_is_the_derived_rendering() {
        for node in sample() {
            let mirror = derived(&node);
            assert_eq!(alloc::format!("{node:?}"), alloc::format!("{mirror:?}"));
            assert_eq!(alloc::format!("{node:#?}"), alloc::format!("{mirror:#?}"));
        }
        let candidate = Candidate {
            format: FORMAT,
            nodes: sample(),
        };
        let mirrors: Vec<Derived> = candidate.nodes.iter().map(derived).collect();
        assert_eq!(
            alloc::format!("{:#?}", candidate.nodes),
            alloc::format!("{mirrors:#?}")
        );
    }

    #[test]
    fn node_copies_are_deep_and_exactly_sized() {
        let original = Node::Quotation(sample());
        let mut copy = original.clone();
        assert_eq!(alloc::format!("{copy:#?}"), alloc::format!("{original:#?}"));
        assert!(exactly_sized(&copy));
        if let Node::Quotation(children) = &mut copy {
            children[3] = Node::Word(String::from("changed"));
        }
        let untouched = Node::Quotation(sample());
        assert_eq!(
            alloc::format!("{original:?}"),
            alloc::format!("{untouched:?}")
        );
        // Lists of every small length, so each halving position is copied.
        for count in 0..9 {
            let list = Node::Quotation((0..count).map(Node::Integer).collect());
            let copy = list.clone();
            assert_eq!(alloc::format!("{copy:?}"), alloc::format!("{list:?}"));
            assert!(exactly_sized(&copy));
        }
    }

    #[test]
    fn generated_spans_map_to_bounded_node_paths() {
        let direct = Candidate {
            format: FORMAT,
            nodes: alloc::vec![Node::Integer(1), Node::Hole, Node::Word(String::from("+"))],
        };
        let paths = direct
            .editor_paths(crate::Limits::default())
            .expect("direct mapping");
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0].1, alloc::vec![0]);
        assert_eq!(paths[1].1, alloc::vec![1]);
        assert_eq!(paths[2].1, alloc::vec![2]);

        let nested = Candidate {
            format: FORMAT,
            nodes: alloc::vec![Node::Quotation(alloc::vec![
                Node::Hole,
                Node::Quotation(alloc::vec![Node::Hole]),
            ])],
        };
        let paths = nested
            .editor_paths(crate::Limits::default())
            .expect("nested mapping");
        assert_eq!(paths.len(), 4);
        // Entries are recorded when each node is emitted, so a quotation appears
        // after its children. Repeated indices name the outer and inner
        // quotation levels rather than collapsing them.
        assert_eq!(paths[0].1, alloc::vec![0, 0]);
        assert_eq!(paths[1].1, alloc::vec![0, 1, 0]);
        assert_eq!(paths[2].1, alloc::vec![0, 1]);
        assert_eq!(paths[3].1, alloc::vec![0]);
        for (span, path) in &paths {
            assert!(span.start <= span.end);
            assert!(!path.is_empty());
        }
    }

    #[test]
    fn repeated_and_unicode_words_keep_their_own_generated_spans() {
        // Issue #63 span mapping: every emitted node keeps its own generated span and its own bounded
        // path, so two identical words never share an address and a repeated word is addressed by
        // position rather than by text.
        let repeated = Candidate {
            format: FORMAT,
            nodes: alloc::vec![
                Node::Word(String::from("+")),
                Node::Word(String::from("+")),
                Node::Quotation(alloc::vec![Node::Word(String::from("+"))]),
                Node::Word(String::from("+")),
            ],
        };
        let paths = repeated
            .editor_paths(crate::Limits::default())
            .expect("repeated mapping");
        assert_eq!(paths.len(), 5, "four words and one quotation");
        let direct_paths: alloc::vec::Vec<_> =
            paths.iter().take(3).map(|(_, path)| path.clone()).collect();
        assert_eq!(
            direct_paths,
            alloc::vec![alloc::vec![0], alloc::vec![1], alloc::vec![2, 0]],
            "repeated words keep distinct paths rather than collapsing into one"
        );
        assert_eq!(paths[3].1, alloc::vec![2], "the quotation's own path");
        assert_eq!(paths[4].1, alloc::vec![3], "the trailing word's own path");
        let mut spans: alloc::vec::Vec<_> = paths.iter().map(|(span, _)| *span).collect();
        spans.sort_by_key(|span| span.start);
        spans.dedup();
        assert_eq!(spans.len(), paths.len(), "no two nodes share a span");

        // Spans are byte ranges of the generated source rather than character counts, and each
        // word's span is exactly its own bytes: a hole-free tree lets the same emitter's bytes be
        // read back at a word's span.
        let source = Candidate {
            format: FORMAT,
            nodes: alloc::vec![Node::Integer(10), Node::Word(String::from("+"))],
        };
        let bytes = source
            .admitted_source(crate::Limits::default())
            .expect("a hole-free tree becomes source");
        let paths = source
            .editor_paths(crate::Limits::default())
            .expect("word mapping");
        let word_span = paths[1].0;
        let start = usize::try_from(word_span.start).expect("bounded span");
        let end = usize::try_from(word_span.end).expect("bounded span");
        // The emitted span covers the node and the separator the emitter writes after it, so the
        // bytes it names start with the word itself rather than being an assumed character count.
        assert!(
            bytes[start..end].starts_with("+".as_bytes()),
            "the span names the word's own bytes first"
        );
        assert_eq!(
            start, 3,
            "the two-byte integer and its separator precede the word"
        );

        // Unicode coverage is a refusal rather than a silent transliteration: the editor word
        // grammar admits ASCII words only, so a non-ASCII word is refused at the parse stage at the
        // refused node's own bounded span instead of being mapped or rewritten.
        let unicode = Candidate {
            format: FORMAT,
            nodes: alloc::vec![Node::Integer(10), Node::Word(String::from("λ"))],
        };
        let refusal = unicode
            .editor_paths(crate::Limits::default())
            .expect_err("a non-ASCII word is refused, never transliterated");
        assert_eq!(refusal.stage(), super::super::Stage::Parse);
        assert_eq!(refusal.diagnostic().kind.category(), "invalid");
        assert_eq!(
            refusal.diagnostic().span.start,
            refusal.diagnostic().span.end
        );
    }

    #[test]
    fn a_hole_admission_refusal_is_located_at_the_first_hole() {
        // Issue #63: the refusal used to carry the whole tree's span, so a nested hole reported the
        // outer quotation as its address. It now carries the first hole's own generated span, and the
        // later hole is not silently chosen instead.
        let candidate = Candidate {
            format: FORMAT,
            nodes: alloc::vec![Node::Quotation(alloc::vec![
                Node::Integer(1),
                Node::Quotation(alloc::vec![Node::Hole]),
            ])],
        };
        let paths = candidate
            .editor_paths(crate::Limits::default())
            .expect("nested paths");
        let error = candidate
            .admitted_source(crate::Limits::default())
            .expect_err("a hole cannot enter admission");
        assert_eq!(error.stage(), super::super::Stage::Acceptance);
        assert_eq!(error.diagnostic().kind.category(), "invalid");
        let (span, path) = paths
            .iter()
            .find(|(span, _)| *span == error.diagnostic().span)
            .expect("the refusal names a node the emitter produced");
        assert_eq!(*span, error.diagnostic().span);
        assert_eq!(*path, alloc::vec![0, 1, 0], "the inner hole's own path");

        let trailing = Candidate {
            format: FORMAT,
            nodes: alloc::vec![
                Node::Integer(1),
                Node::Quotation(alloc::vec![Node::Hole]),
                Node::Hole,
            ],
        };
        let paths = trailing
            .editor_paths(crate::Limits::default())
            .expect("first-hole paths");
        let error = trailing
            .admitted_source(crate::Limits::default())
            .expect_err("a hole cannot enter admission");
        let (_, path) = paths
            .iter()
            .find(|(span, _)| *span == error.diagnostic().span)
            .expect("the first hole is the one named");
        assert_eq!(*path, alloc::vec![1, 0], "the first hole, not the last");
    }

    #[test]
    fn deep_quotation_emission_respects_configured_depth_without_call_stack_recursion() {
        let mut node = Node::Integer(7);
        for _ in 0..512 {
            node = Node::Quotation(alloc::vec![node]);
        }
        let candidate = Candidate {
            format: FORMAT,
            nodes: alloc::vec![node],
        };
        let mut limits = crate::Limits {
            depth: 512,
            ..crate::Limits::default()
        };
        let source = candidate
            .admitted_source(limits)
            .expect("iterative quotation emission");
        assert_eq!(source.iter().filter(|&&byte| byte == b'[').count(), 512);
        assert_eq!(source.iter().filter(|&&byte| byte == b']').count(), 512);
        assert!(source.starts_with(b"[ [ "));
        assert!(source.ends_with(b"] ] "));

        limits.depth = 511;
        let error = candidate.admitted_source(limits).expect_err("depth limit");
        assert_eq!(error.stage(), super::super::Stage::Parse);
        assert_eq!(error.diagnostic().message, "frontend depth limit exceeded");
        // A budget refusal stays an exhaustion rather than collapsing into a
        // generic invalid input.
        assert_eq!(error.diagnostic().kind.category(), "exhausted");
    }
}
