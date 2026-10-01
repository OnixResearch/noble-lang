//! Editor-only syntax transport. No editor node is a kernel candidate or a
//! `.noble` token. An executable tree is re-parsed by normal source admission.

use alloc::{string::String, vec::Vec};

pub const FORMAT: u32 = 1;

#[derive(Clone, Debug)]
pub enum Node {
    Integer(i64),
    Boolean(bool),
    Word(String),
    Quotation(Vec<Node>),
    Hole,
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
}

impl Builder {
    fn append(
        &mut self,
        bytes: &[u8],
        meter: &mut crate::Meter,
    ) -> Result<crate::Span, crate::Diagnostic> {
        let start = attempt!(crate::index(self.source.len(), self.tree.span));
        let end = attempt!(crate::index(
            self.source.len().saturating_add(bytes.len()),
            self.tree.span
        ));
        if end >= meter.limits.bytes {
            return Err(super::exhausted(self.tree.span, "editor source byte limit exceeded"));
        }
        attempt!(meter.charge(attempt!(crate::index(bytes.len(), self.tree.span)), self.tree.span));
        self.source.extend_from_slice(bytes);
        self.source.push(b' ');
        Ok(crate::Span { start, end })
    }

    fn body(
        &mut self,
        nodes: &[Node],
        depth: u32,
        meter: &mut crate::Meter,
    ) -> Result<Vec<u32>, crate::Diagnostic> {
        attempt!(meter.depth(depth, self.tree.span));
        let mut body = Vec::new();
        for node in nodes {
            attempt!(meter.node(self.tree.span));
            let start = attempt!(crate::index(self.source.len(), self.tree.span));
            let kind = match node {
                Node::Integer(value) => {
                    let text = alloc::format!("{value}");
                    attempt!(self.append(text.as_bytes(), meter));
                    super::Kind::Literal(noble_kernel::untrusted::Lit::I64(*value))
                }
                Node::Boolean(value) => {
                    attempt!(self.append(if *value { b"true" } else { b"false" }, meter));
                    super::Kind::Literal(noble_kernel::untrusted::Lit::Bool(*value))
                }
                Node::Word(word) => {
                    let span = crate::Span { start, end: start };
                    if !matches!(
                        attempt!(super::lexer::tokens::classify(word.as_bytes(), span, meter, false)),
                        super::lexer::TokenKind::Word(_)
                    ) {
                        return Err(crate::invalid(span, "editor word is not source word syntax"));
                    }
                    attempt!(self.append(word.as_bytes(), meter));
                    super::Kind::Word(word.as_bytes().to_vec())
                }
                Node::Hole => {
                    self.holes += 1;
                    attempt!(self.append(b"@editor-hole", meter));
                    super::Kind::EditorHole
                }
                Node::Quotation(children) => {
                    attempt!(self.append(b"[", meter));
                    let children = attempt!(self.body(children, depth.saturating_add(1), meter));
                    attempt!(self.append(b"]", meter));
                    super::Kind::Quotation(children)
                }
            };
            let id = attempt!(crate::index(self.tree.nodes.len(), self.tree.span));
            let end = attempt!(crate::index(self.source.len(), self.tree.span));
            self.tree.nodes.push(super::Node {
                kind,
                span: crate::Span { start, end },
            });
            body.push(id);
        }
        Ok(body)
    }
}

impl Candidate {
    fn build(&self, limits: crate::Limits) -> Result<(Builder, crate::Meter), super::Error> {
        let span = crate::Span { start: 0, end: 0 };
        if self.format != FORMAT {
            return Err(super::Error::at(
                super::Stage::Parse,
                crate::Diagnostic::new(crate::DiagnosticKind::Unsupported, span, "unsupported editor format"),
            ));
        }
        let mut builder = Builder {
            tree: super::Tree { nodes: Vec::new(), body: Vec::new(), span },
            source: Vec::new(),
            holes: 0,
        };
        let mut meter = crate::Meter::new(limits);
        builder.tree.body = attempt!(builder.body(&self.nodes, 0, &mut meter)
            .map_err(|error| super::Error::at(super::Stage::Parse, error)));
        builder.tree.span.end = attempt!(crate::index(builder.source.len(), span)
            .map_err(|error| super::Error::at(super::Stage::Parse, error)));
        Ok((builder, meter))
    }

    /// Only a hole-free tree can become source bytes. The ordinary source
    /// parser, inference, kernel and compiler remain the sole admission route.
    pub fn admitted_source(&self, limits: crate::Limits) -> Result<Vec<u8>, super::Error> {
        let (builder, _) = attempt!(self.build(limits));
        if builder.holes != 0 {
            return Err(super::Error::at(
                super::Stage::Acceptance,
                crate::invalid(builder.tree.span, "editor hole cannot enter source admission"),
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
        let (mut builder, mut meter) = attempt!(candidate.build(limits));
        if let Err(error) = super::resolution::resolve(&mut builder.tree, None, self, &mut meter) {
            return Err(super::Error::at(super::Stage::Resolve, error));
        }
        if let Err(error) = super::preflight::check(inputs, self, builder.tree.span, &mut meter) {
            return Err(super::Error::at(super::Stage::Check, error));
        }
        let environment = attempt!(self.environment()
            .map_err(|error| super::Error::at(super::Stage::Check, error)));
        let scope = super::inference::Scope {
            root: &builder.tree,
            session: self,
            environment: &environment,
            root_signature: None,
        };
        let state = attempt!(super::inference::infer(
            scope, super::inference::Mode::EditorAnalysis, inputs, &mut meter
        ).map_err(|error| super::Error::at(super::Stage::Check, error)));
        let mut holes = Vec::with_capacity(state.holes.len());
        for hole in &state.holes {
            let input_stack = attempt!(state.arena.describe_stack(hole.input, hole.span, &mut meter)
                .map_err(|error| super::Error::at(super::Stage::Check, error)));
            let output_stack = attempt!(state.arena.describe_stack(hole.output, hole.span, &mut meter)
                .map_err(|error| super::Error::at(super::Stage::Check, error)));
            let incoming = attempt!(state.arena.editor_stack_suffix(hole.input, hole.span, &mut meter)
                .map_err(|error| super::Error::at(super::Stage::Check, error)));
            let outgoing = attempt!(state.arena.editor_stack_suffix(hole.output, hole.span, &mut meter)
                .map_err(|error| super::Error::at(super::Stage::Check, error)));
            let common = incoming.iter().zip(&outgoing)
                .take_while(|(left, right)| left == right).count();
            let required_output_suffix = outgoing[common..].to_vec();
            let (known_effects, unresolved_effect) =
                attempt!(state.arena.editor_effect(hole.effect, hole.span, &mut meter)
                    .map_err(|error| super::Error::at(super::Stage::Check, error)));
            holes.push(HoleConstraint {
                input_stack, output_stack, required_output_suffix,
                known_effects, unresolved_effect,
            });
        }
        let root = match state.bodies.first() {
            Some(root) => root,
            None => return Err(super::Error::at(super::Stage::Check, crate::internal(builder.tree.span))),
        };
        let (known_effects, unresolved_effect) =
            attempt!(state.arena.editor_effect(root.effect, root.span, &mut meter)
                .map_err(|error| super::Error::at(super::Stage::Check, error)));
        Ok(Analysis { holes, known_effects, unresolved_effect })
    }
}
