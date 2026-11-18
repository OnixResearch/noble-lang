pub(in crate::source) struct ParsedType {
    pub ty: noble_kernel::types::Ty,
    pub nodes: usize,
}

pub(in crate::source) fn parse_with_families(
    word: &str,
    types: &[(alloc::string::String, noble_kernel::types::Ty)],
    families: &[(alloc::string::String, noble_kernel::types::NominalTypeId)],
    environment: Option<&noble_kernel::contracts::Env>,
    span: crate::Span,
) -> Result<ParsedType, crate::Diagnostic> {
    let parser = attempt!(start_parser(alloc::string::String::from(word), span, 0));
    run(parser, types, families, environment)
}

/// A parser for one type word at nesting `depth`, after the depth and whitespace checks.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn start_parser(
    word: alloc::string::String,
    span: crate::Span,
    depth: u32,
) -> Result<Parser, crate::Diagnostic> {
    if depth >= 32 {
        return Err(crate::invalid(
            span,
            "type constructor nesting limit exceeded",
        ));
    }
    if contains_ascii_whitespace(word.as_bytes()) {
        return Err(crate::invalid(span, "type word contains whitespace"));
    }
    Ok(Parser {
        text: word.into_bytes(),
        at: 0,
        nodes: 0,
        span,
        depth,
        pending: alloc::vec::Vec::new(),
        value: None,
    })
}

/// The running type-word parser and the callable types suspended beneath it,
/// innermost last. This explicit stack replaces recursion through nested
/// `Program`/`LiveRef` components, so no loop lies on a recursive cycle.
struct Machine {
    running: Parser,
    suspended: alloc::vec::Vec<Callable>,
}

/// Drive `parser` and every nested callable component to the first failure
/// or the complete outermost type.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn run(
    parser: Parser,
    types: &[(alloc::string::String, noble_kernel::types::Ty)],
    families: &[(alloc::string::String, noble_kernel::types::NominalTypeId)],
    environment: Option<&noble_kernel::contracts::Env>,
) -> Result<ParsedType, crate::Diagnostic> {
    let span = parser.span;
    let mut machine = Machine {
        running: parser,
        suspended: alloc::vec::Vec::new(),
    };
    let mut completed = None;
    let mut failure = None;
    while completed.is_none() {
        match machine_step(machine, types, families, environment) {
            Ok((next, result)) => {
                machine = next;
                completed = result;
            }
            Err(problem) => {
                failure = Some(problem);
                break;
            }
        }
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    completed.ok_or_else(|| crate::internal(span))
}

/// Step the running parser; hand a finished component to the callable waiting
/// for it, or report the outermost type once no callable is waiting.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; Vec pop and the parser and callable steps it calls are not const on the pinned compilers, so the const suggestion does not apply"
)]
fn machine_step(
    machine: Machine,
    types: &[(alloc::string::String, noble_kernel::types::Ty)],
    families: &[(alloc::string::String, noble_kernel::types::NominalTypeId)],
    environment: Option<&noble_kernel::contracts::Env>,
) -> Result<(Machine, Option<ParsedType>), crate::Diagnostic> {
    let Machine {
        running,
        mut suspended,
    } = machine;
    match attempt!(running.parse_step(types, families, environment)) {
        Step::Continue(parser) => Ok((
            Machine {
                running: parser,
                suspended,
            },
            None,
        )),
        Step::Callable(callable) => resume(callable.advance(environment), suspended),
        Step::Complete(parser, parsed) => match suspended.pop() {
            Some(callable) => resume(callable.accept(parsed).advance(environment), suspended),
            None => Ok((
                Machine {
                    running: parser,
                    suspended,
                },
                Some(parsed),
            )),
        },
    }
}

/// Continue with the next component of a callable, or with its finished parent.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; Vec push and Result matching with owned Vec payloads are not const on the pinned compilers, so the const suggestion does not apply"
)]
fn resume(
    advanced: Result<Advance, crate::Diagnostic>,
    mut suspended: alloc::vec::Vec<Callable>,
) -> Result<(Machine, Option<ParsedType>), crate::Diagnostic> {
    match attempt!(advanced) {
        Advance::Component(callable, parser) => {
            suspended.push(callable);
            Ok((
                Machine {
                    running: parser,
                    suspended,
                },
                None,
            ))
        }
        Advance::Finished(parent) => Ok((
            Machine {
                running: parent,
                suspended,
            },
            None,
        )),
    }
}

/// The type-word parser state. It owns its text so suspended parsers can be
/// kept on the machine's stack; the name tables and the optional environment
/// are passed beside it.
struct Parser {
    text: alloc::vec::Vec<u8>,
    at: usize,
    nodes: usize,
    span: crate::Span,
    depth: u32,
    pending: alloc::vec::Vec<Frame>,
    value: Option<noble_kernel::types::Ty>,
}

/// One parser step: keep going, suspend for a callable, or a complete type.
enum Step {
    Continue(Parser),
    Callable(Callable),
    Complete(Parser, ParsedType),
}

/// One read type: the parser continues, or a callable needs its stacks parsed.
enum Read {
    Parser(Parser),
    Callable(Callable),
}

/// A `Program`/`LiveRef` type whose input and output stacks are being parsed,
/// with the parser that read it suspended until both are complete.
struct Callable {
    parent: Parser,
    is_live_ref: bool,
    end: usize,
    fields: alloc::vec::Vec<alloc::string::String>,
    nested: u32,
    has_inputs: bool,
    inputs: alloc::vec::Vec<noble_kernel::types::Ty>,
    input_nodes: usize,
    stack: StackWork,
}

/// One `+`-separated program stack: its parts, the next part and the parsed values.
struct StackWork {
    parts: alloc::vec::Vec<alloc::string::String>,
    at: usize,
    values: alloc::vec::Vec<noble_kernel::types::Ty>,
    nodes: usize,
}

impl StackWork {
    /// The stack of an `empty` field: no parts and no values.
    const fn empty() -> Self {
        Self {
            parts: alloc::vec::Vec::new(),
            at: 0,
            values: alloc::vec::Vec::new(),
            nodes: 0,
        }
    }
}

/// A callable after one advance: a component to parse next, or its finished parent.
#[expect(
    clippy::large_enum_variant,
    reason = "Owner: noble-contracts; each advance result is matched immediately and its callable moved onto the machine stack, so boxing would only add an allocation per component"
)]
enum Advance {
    Component(Callable, Parser),
    Finished(Parser),
}

/// A stack after one advance: its next component parser, or its values and nodes.
enum Part {
    Parser(StackWork, Parser),
    Done(alloc::vec::Vec<noble_kernel::types::Ty>, usize),
}

/// The program stack spelled by one callable field, before any component is parsed.
fn start_stack(word: &str, span: crate::Span) -> Result<StackWork, crate::Diagnostic> {
    if word == "empty" {
        return Ok(StackWork::empty());
    }
    let parts = attempt!(split_top_level(word, b'+', span));
    if parts.len() > 256 {
        return Err(crate::invalid(span, "Program stack height limit exceeded"));
    }
    Ok(StackWork {
        values: alloc::vec::Vec::with_capacity(parts.len()),
        parts,
        at: 0,
        nodes: 0,
    })
}

/// The parser of the next stack component, or the finished stack.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; mem::take of a String and parser construction are not const on the pinned compilers, so the const suggestion does not apply"
)]
fn next_part(
    mut stack: StackWork,
    span: crate::Span,
    depth: u32,
) -> Result<Part, crate::Diagnostic> {
    if stack.at < stack.parts.len() {
        let at = stack.at;
        let word = core::mem::take(&mut stack.parts[at]);
        stack.at += 1;
        let parser = attempt!(start_parser(word, span, depth));
        return Ok(Part::Parser(stack, parser));
    }
    Ok(Part::Done(stack.values, stack.nodes))
}

impl Callable {
    /// Record one parsed stack component.
    fn accept(mut self, parsed: ParsedType) -> Self {
        self.stack.nodes = self.stack.nodes.saturating_add(parsed.nodes);
        self.stack.values.push(parsed.ty);
        self
    }

    /// Start the next component of the current stack, or move past a finished one.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; it drops and moves owned Vec state and calls non-const stack helpers, so the const suggestion does not apply"
    )]
    fn advance(
        mut self,
        environment: Option<&noble_kernel::contracts::Env>,
    ) -> Result<Advance, crate::Diagnostic> {
        let stack = core::mem::replace(&mut self.stack, StackWork::empty());
        match attempt!(next_part(stack, self.parent.span, self.nested)) {
            Part::Parser(stack, parser) => {
                self.stack = stack;
                Ok(Advance::Component(self, parser))
            }
            Part::Done(values, nodes) => {
                if self.has_inputs {
                    self.finish(values, nodes, environment)
                } else {
                    self.has_inputs = true;
                    self.inputs = values;
                    self.input_nodes = nodes;
                    self.start_outputs(environment)
                }
            }
        }
    }

    /// Begin the output stack once the input stack is complete.
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; it splits the output field into owned Strings and calls non-const stack helpers, so the const suggestion does not apply"
    )]
    fn start_outputs(
        mut self,
        environment: Option<&noble_kernel::contracts::Env>,
    ) -> Result<Advance, crate::Diagnostic> {
        let stack = attempt!(start_stack(&self.fields[1], self.parent.span));
        match attempt!(next_part(stack, self.parent.span, self.nested)) {
            Part::Parser(stack, parser) => {
                self.stack = stack;
                Ok(Advance::Component(self, parser))
            }
            Part::Done(values, nodes) => self.finish(values, nodes, environment),
        }
    }

    /// Parse the effect bound and resume the parent with the complete callable type.
    fn finish(
        self,
        outputs: alloc::vec::Vec<noble_kernel::types::Ty>,
        output_nodes: usize,
        environment: Option<&noble_kernel::contracts::Env>,
    ) -> Result<Advance, crate::Diagnostic> {
        let Callable {
            mut parent,
            is_live_ref,
            end,
            fields,
            inputs,
            input_nodes,
            ..
        } = self;
        let effects = attempt!(parse_callable_effects(&fields[2], environment, parent.span));
        parent.nodes = parent
            .nodes
            .saturating_add(input_nodes)
            .saturating_add(output_nodes);
        if parent.nodes > 256 {
            return Err(crate::invalid(
                parent.span,
                "type constructor limit exceeded",
            ));
        }
        parent.at = attempt!(end
            .checked_add(1)
            .ok_or_else(|| crate::invalid(parent.span, "unclosed Program type")));
        parent.value = Some(if is_live_ref {
            noble_kernel::types::Ty::LiveRef(
                alloc::boxed::Box::new(inputs),
                alloc::boxed::Box::new(outputs),
                effects,
            )
        } else {
            noble_kernel::types::Ty::program(inputs, outputs, effects)
        });
        Ok(Advance::Finished(parent))
    }
}

enum Frame {
    List,
    PairLeft,
    SumLeft,
    PairRight(noble_kernel::types::Ty),
    SumRight(noble_kernel::types::Ty),
    GenericLeft(noble_kernel::types::NominalTypeId),
    GenericRight(noble_kernel::types::NominalTypeId, noble_kernel::types::Ty),
    Unknown,
}

impl Parser {
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-contracts; it calls the non-const read and fold steps over owned Vec state, so the const suggestion does not apply"
    )]
    fn parse_step(
        mut self,
        types: &[(alloc::string::String, noble_kernel::types::Ty)],
        families: &[(alloc::string::String, noble_kernel::types::NominalTypeId)],
        environment: Option<&noble_kernel::contracts::Env>,
    ) -> Result<Step, crate::Diagnostic> {
        if self.value.is_none() {
            match attempt!(self.read_type(types, families, environment)) {
                Read::Parser(parser) => self = parser,
                Read::Callable(callable) => return Ok(Step::Callable(callable)),
            }
        }
        match self.value.take() {
            Some(ty) => match attempt!(self.fold(ty, environment)) {
                (parser, Some(parsed)) => Ok(Step::Complete(parser, parsed)),
                (parser, None) => Ok(Step::Continue(parser)),
            },
            None => Ok(Step::Continue(self)),
        }
    }

    fn read_type(
        mut self,
        types: &[(alloc::string::String, noble_kernel::types::Ty)],
        families: &[(alloc::string::String, noble_kernel::types::NominalTypeId)],
        environment: Option<&noble_kernel::contracts::Env>,
    ) -> Result<Read, crate::Diagnostic> {
        if self.pending.len() >= 64 || self.nodes >= 256 {
            return Err(crate::invalid(self.span, "type constructor limit exceeded"));
        }
        self.nodes += 1;
        let start = self.at;
        while type_name_byte(&self.text, self.at) {
            self.at += 1;
        }
        // An owned copy, so the name outlives moves of the parser.
        let name = attempt!(
            alloc::string::String::from_utf8(self.text[start..self.at].to_vec())
                .map_err(|_| crate::invalid(self.span, "invalid type name"))
        );
        if name.is_empty() {
            return Err(crate::invalid(self.span, "type constructor missing name"));
        }
        if self.text.get(self.at) == Some(&b'<') {
            self.at += 1;
            if name == "Program" || name == "LiveRef" {
                let is_live_ref = name == "LiveRef";
                return Ok(Read::Callable(attempt!(
                    self.read_callable(is_live_ref, environment)
                )));
            } else if name == "Resource" {
                let kind = b"test.counter";
                if self.text.get(self.at..self.at.saturating_add(kind.len()))
                    != Some(kind.as_slice())
                {
                    return Err(crate::invalid(self.span, "unregistered resource kind"));
                }
                self.at += kind.len();
                attempt!(self.expect(b'>'));
                self.at += 1;
                self.value = Some(noble_kernel::types::Ty::Resource(
                    noble_kernel::types::ResourceKind(0),
                ));
            } else {
                let family = crate::scan::find_first(families, |(known, _)| *known == name)
                    .map(|(_, id)| *id);
                self.pending.push(match name.as_str() {
                    "List" => Frame::List,
                    "Pair" => Frame::PairLeft,
                    "Sum" => Frame::SumLeft,
                    _ => family.map_or(Frame::Unknown, Frame::GenericLeft),
                });
            }
        } else {
            self.value = Some(match name.as_str() {
                "I64" => noble_kernel::types::Ty::I64,
                "Bool" => noble_kernel::types::Ty::Bool,
                "Text" => noble_kernel::types::Ty::Text,
                "Unit" => noble_kernel::types::Ty::Unit,
                "Syntax" => noble_kernel::types::Ty::Syntax,
                _ => {
                    let mut found = None;
                    let mut at = types.len();
                    while at != 0 {
                        at -= 1;
                        if names_type(&types[at], &name) {
                            found = Some(types[at].1.clone());
                            break;
                        }
                    }
                    match found {
                        Some(ty) => ty,
                        None => {
                            return Err(crate::invalid(self.span, "unknown or cyclic nominal type"))
                        }
                    }
                }
            });
        }
        Ok(Read::Parser(self))
    }

    fn read_callable(
        mut self,
        is_live_ref: bool,
        environment: Option<&noble_kernel::contracts::Env>,
    ) -> Result<Callable, crate::Diagnostic> {
        if is_live_ref && !environment.is_some_and(|environment| environment.live_slots) {
            return Err(crate::invalid(
                self.span,
                "LiveRef requires the live-slot profile",
            ));
        }
        let start = self.at;
        let mut depth = 0usize;
        let mut end = None;
        while self.at < self.text.len() {
            match self.text[self.at] {
                b'<' => depth += 1,
                b'>' if depth == 0 => {
                    end = Some(self.at);
                    break;
                }
                b'>' => depth -= 1,
                _ => {}
            }
            self.at += 1;
        }
        let Some(end) = end else {
            return Err(crate::invalid(self.span, "unclosed Program type"));
        };
        let inner = attempt!(core::str::from_utf8(&self.text[start..end])
            .map_err(|_| crate::invalid(self.span, "invalid Program type encoding")));
        let fields = attempt!(split_top_level(inner, b',', self.span));
        if fields.len() != 3 {
            return Err(crate::invalid(
                self.span,
                "Program requires input, output and effects",
            ));
        }
        let nested = attempt!(self.depth.checked_add(1).ok_or_else(|| {
            crate::invalid(self.span, "type constructor nesting limit exceeded")
        }));
        let stack = attempt!(start_stack(&fields[0], self.span));
        Ok(Callable {
            parent: self,
            is_live_ref,
            end,
            fields,
            nested,
            has_inputs: false,
            inputs: alloc::vec::Vec::new(),
            input_nodes: 0,
            stack,
        })
    }

    fn fold(
        mut self,
        ty: noble_kernel::types::Ty,
        environment: Option<&noble_kernel::contracts::Env>,
    ) -> Result<(Self, Option<ParsedType>), crate::Diagnostic> {
        match self.pending.pop() {
            Some(Frame::List) => {
                attempt!(self.expect(b'>'));
                self.at += 1;
                self.value = Some(noble_kernel::types::Ty::List(alloc::boxed::Box::new(ty)));
            }
            Some(Frame::PairLeft) => {
                attempt!(self.expect(b','));
                self.at += 1;
                self.pending.push(Frame::PairRight(ty));
            }
            Some(Frame::SumLeft) => {
                attempt!(self.expect(b','));
                self.at += 1;
                self.pending.push(Frame::SumRight(ty));
            }
            Some(Frame::GenericLeft(id)) => {
                attempt!(self.expect(b','));
                self.at += 1;
                self.pending.push(Frame::GenericRight(id, ty));
            }
            Some(Frame::GenericRight(id, left)) => {
                attempt!(self.expect(b'>'));
                self.at += 1;
                self.value = Some(attempt!(environment
                    .and_then(|environment| environment.generic_instance(id, [left, ty]))
                    .ok_or_else(|| crate::invalid(
                        self.span,
                        "invalid or unsupported generic variant instance",
                    ))));
            }
            Some(Frame::PairRight(left)) => {
                attempt!(self.expect(b'>'));
                self.at += 1;
                self.value = Some(noble_kernel::types::Ty::Pair(
                    alloc::boxed::Box::new(left),
                    alloc::boxed::Box::new(ty),
                ));
            }
            Some(Frame::SumRight(left)) => {
                attempt!(self.expect(b'>'));
                self.at += 1;
                self.value = Some(noble_kernel::types::Ty::Sum(
                    alloc::boxed::Box::new(left),
                    alloc::boxed::Box::new(ty),
                ));
            }
            Some(Frame::Unknown) => {
                return Err(crate::invalid(
                    self.span,
                    "unknown generic type constructor",
                ));
            }
            None => {
                if self.at != self.text.len() {
                    return Err(crate::invalid(
                        self.span,
                        "trailing characters in type word",
                    ));
                }
                let nodes = self.nodes;
                return Ok((self, Some(ParsedType { ty, nodes })));
            }
        }
        Ok((self, None))
    }

    fn expect(&self, byte: u8) -> Result<(), crate::Diagnostic> {
        if self.text.get(self.at).eq(&Some(&byte)) {
            Ok(())
        } else {
            Err(crate::invalid(
                self.span,
                "malformed generic type argument list",
            ))
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
#[expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-contracts; the collection is bounded by the surrounding admission limits, which are checked before this loop runs"
)]
fn parse_callable_effects(
    text: &str,
    environment: Option<&noble_kernel::contracts::Env>,
    span: crate::Span,
) -> Result<noble_kernel::types::EffSet, crate::Diagnostic> {
    if text == "pure" {
        return Ok(noble_kernel::types::EffSet::empty());
    }
    let words = attempt!(split_top_level(text, b'+', span));
    let mut effects = alloc::vec::Vec::new();
    let mut failure = None;
    let mut index = 0usize;
    while failure.is_none() && index < words.len() {
        match callable_effect(&words[index], environment, span, &effects) {
            Ok(id) => effects.push(id),
            Err(problem) => failure = Some(problem),
        }
        index += 1;
    }
    if let Some(problem) = failure {
        return Err(problem);
    }
    Ok(noble_kernel::types::EffSet::from_ids(&effects))
}

/// One registered, not yet bound callable effect named by `word`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; str pattern matching and diagnostic construction are not const on the pinned compilers, so the const suggestion does not apply"
)]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn callable_effect(
    word: &str,
    environment: Option<&noble_kernel::contracts::Env>,
    span: crate::Span,
    effects: &[noble_kernel::types::EffId],
) -> Result<noble_kernel::types::EffId, crate::Diagnostic> {
    let id = match word {
        "test.emit" => noble_kernel::contracts::TEST_EMIT,
        "test.clock" => noble_kernel::contracts::TEST_CLOCK,
        "test.code-export" => noble_kernel::contracts::TEST_CODE_EXPORT,
        "test.send" => noble_kernel::contracts::TEST_SEND,
        "live.dispatch" => noble_kernel::contracts::LIVE_DISPATCH,
        _ => return Err(crate::invalid(span, "unknown callable effect bound")),
    };
    if !environment.is_some_and(|environment| {
        environment.effects.contains(&id)
            && (id != noble_kernel::contracts::LIVE_DISPATCH || environment.live_slots)
    }) || effects.contains(&id)
    {
        return Err(crate::invalid(
            span,
            "unregistered or duplicate callable effect",
        ));
    }
    Ok(id)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn split_top_level(
    word: &str,
    separator: u8,
    span: crate::Span,
) -> Result<alloc::vec::Vec<alloc::string::String>, crate::Diagnostic> {
    let mut split = Ok(TopLevel {
        parts: alloc::vec::Vec::new(),
        start: 0,
        depth: 0,
    });
    let mut at = 0usize;
    while split.is_ok() && at < word.len() {
        split = match split {
            Ok(current) => top_level_step(word, separator, at, span, current),
            Err(problem) => Err(problem),
        };
        at += 1;
    }
    let TopLevel {
        mut parts,
        start,
        depth,
    } = attempt!(split);
    if depth != 0 || parts.len() > 256 {
        return Err(crate::invalid(
            span,
            "unbalanced or oversized type argument",
        ));
    }
    parts.push(alloc::string::String::from(&word[start..]));
    if super::has_empty_part(&parts) {
        return Err(crate::invalid(span, "empty type argument"));
    }
    Ok(parts)
}

/// The owned parts, current part start and nesting depth of a top-level split.
struct TopLevel {
    parts: alloc::vec::Vec<alloc::string::String>,
    start: usize,
    depth: usize,
}

/// Classify the byte of `word` at `at`: track nesting or cut a top-level part.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-contracts; the invariants of this linear dispatcher are enforced by its callers and the contracts test suite"
)]
fn top_level_step(
    word: &str,
    separator: u8,
    at: usize,
    span: crate::Span,
    mut split: TopLevel,
) -> Result<TopLevel, crate::Diagnostic> {
    match word.as_bytes()[at] {
        b'<' => split.depth = split.depth.saturating_add(1),
        b'>' => {
            split.depth = attempt!(split
                .depth
                .checked_sub(1)
                .ok_or_else(|| crate::invalid(span, "unbalanced type argument")));
        }
        byte if byte == separator && split.depth == 0 => {
            // Parts end at distinct separator bytes, so this bound is never reached.
            if split.parts.len() >= word.len() {
                return Err(crate::invalid(
                    span,
                    "unbalanced or oversized type argument",
                ));
            }
            split
                .parts
                .push(alloc::string::String::from(&word[split.start..at]));
            split.start = attempt!(at
                .checked_add(1)
                .ok_or_else(|| crate::invalid(span, "unbalanced or oversized type argument")));
        }
        _ => {}
    }
    Ok(split)
}
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; slice.get requires unavailable const_index on both pinned Rust compilers; out-of-range type bytes reject; reassess const indexing."
)]
fn type_name_byte(text: &[u8], at: usize) -> bool {
    if let Some(byte) = text.get(at) {
        byte.is_ascii_alphanumeric() || *byte == b'.' || *byte == b'_' || *byte == b'@'
    } else {
        false
    }
}

/// Whether the named-type entry is spelled `name`.
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-contracts; String-to-str equality is not const on the pinned compilers, so the const suggestion does not apply"
)]
fn names_type(entry: &(alloc::string::String, noble_kernel::types::Ty), name: &str) -> bool {
    entry.0 == name
}

fn contains_ascii_whitespace(bytes: &[u8]) -> bool {
    let mut at = 0usize;
    let mut is_found = false;
    while at < bytes.len() && !is_found {
        is_found = bytes[at].is_ascii_whitespace();
        at += 1;
    }
    is_found
}
