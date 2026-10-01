pub(super) struct ParsedType {
    pub ty: noble_kernel::types::Ty,
    pub nodes: usize,
}

pub(super) fn parse_with_families(
    word: &str,
    types: &[(alloc::string::String, noble_kernel::types::Ty)],
    families: &[(alloc::string::String, noble_kernel::types::NominalTypeId)],
    environment: Option<&noble_kernel::contracts::Env>,
    span: crate::Span,
) -> Result<ParsedType, crate::Diagnostic> {
    parse_with_depth(word, types, families, environment, span, 0)
}

fn parse_with_depth(
    word: &str,
    types: &[(alloc::string::String, noble_kernel::types::Ty)],
    families: &[(alloc::string::String, noble_kernel::types::NominalTypeId)],
    environment: Option<&noble_kernel::contracts::Env>,
    span: crate::Span,
    depth: u32,
) -> Result<ParsedType, crate::Diagnostic> {
    if depth >= 32 {
        return Err(crate::invalid(
            span,
            "type constructor nesting limit exceeded",
        ));
    }
    if contains_ascii_whitespace(word.as_bytes()) {
        return Err(crate::invalid(span, "type word contains whitespace"));
    }
    Parser {
        text: word.as_bytes(),
        at: 0,
        nodes: 0,
        span,
        names: types,
        families,
        environment,
        depth,
        pending: alloc::vec::Vec::new(),
        value: None,
    }
    .parse()
}

struct Parser<'a> {
    text: &'a [u8],
    at: usize,
    nodes: usize,
    span: crate::Span,
    names: &'a [(alloc::string::String, noble_kernel::types::Ty)],
    families: &'a [(alloc::string::String, noble_kernel::types::NominalTypeId)],
    environment: Option<&'a noble_kernel::contracts::Env>,
    depth: u32,
    pending: alloc::vec::Vec<Frame>,
    value: Option<noble_kernel::types::Ty>,
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

impl Parser<'_> {
    fn parse(mut self) -> Result<ParsedType, crate::Diagnostic> {
        let span = self.span;
        let mut completed = None;
        let mut failure = None;
        while completed.is_none() {
            match self.parse_step() {
                Ok((next, result)) => {
                    self = next;
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

    fn parse_step(mut self) -> Result<(Self, Option<ParsedType>), crate::Diagnostic> {
        if self.value.is_none() {
            self = attempt!(self.read_type());
        }
        if let Some(ty) = self.value.take() {
            return self.fold(ty);
        }
        Ok((self, None))
    }

    fn read_type(mut self) -> Result<Self, crate::Diagnostic> {
        if self.pending.len() >= 64 || self.nodes >= 256 {
            return Err(crate::invalid(self.span, "type constructor limit exceeded"));
        }
        self.nodes += 1;
        let start = self.at;
        while type_name_byte(self.text, self.at) {
            self.at += 1;
        }
        let name = attempt!(core::str::from_utf8(&self.text[start..self.at])
            .map_err(|_| crate::invalid(self.span, "invalid type name")));
        if name.is_empty() {
            return Err(crate::invalid(self.span, "type constructor missing name"));
        }
        if self.text.get(self.at) == Some(&b'<') {
            self.at += 1;
            if name == "Program" {
                self = attempt!(self.read_program());
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
                let family = self
                    .families
                    .iter()
                    .find_map(|(known, id)| (known == name).then_some(*id));
                self.pending.push(match name {
                    "List" => Frame::List,
                    "Pair" => Frame::PairLeft,
                    "Sum" => Frame::SumLeft,
                    _ => family.map_or(Frame::Unknown, Frame::GenericLeft),
                });
            }
        } else {
            self.value = Some(match name {
                "I64" => noble_kernel::types::Ty::I64,
                "Bool" => noble_kernel::types::Ty::Bool,
                "Text" => noble_kernel::types::Ty::Text,
                "Unit" => noble_kernel::types::Ty::Unit,
                "Syntax" => noble_kernel::types::Ty::Syntax,
                _ => {
                    let mut found = None;
                    let mut at = self.names.len();
                    while at != 0 {
                        at -= 1;
                        let (known, ty) = &self.names[at];
                        if known == name {
                            found = Some(ty.clone());
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
        Ok(self)
    }

    fn read_program(mut self) -> Result<Self, crate::Diagnostic> {
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
        let (inputs, input_nodes) = attempt!(parse_program_stack(
            fields[0],
            self.names,
            self.families,
            self.environment,
            self.span,
            self.depth + 1,
        ));
        let (outputs, output_nodes) = attempt!(parse_program_stack(
            fields[1],
            self.names,
            self.families,
            self.environment,
            self.span,
            self.depth + 1,
        ));
        let effects = match fields[2] {
            "pure" => noble_kernel::types::EffSet::empty(),
            "test.emit"
                if self.environment.is_some_and(|environment| {
                    environment
                        .effects
                        .contains(&noble_kernel::contracts::TEST_EMIT)
                }) =>
            {
                noble_kernel::types::EffSet::from_ids(&[noble_kernel::contracts::TEST_EMIT])
            }
            "test.clock"
                if self.environment.is_some_and(|environment| {
                    environment.effects.contains(&noble_kernel::contracts::TEST_CLOCK)
                }) =>
            {
                noble_kernel::types::EffSet::from_ids(&[noble_kernel::contracts::TEST_CLOCK])
            }
            _ => return Err(crate::invalid(self.span, "unknown Program effect bound")),
        };
        self.nodes = self
            .nodes
            .saturating_add(input_nodes)
            .saturating_add(output_nodes);
        if self.nodes > 256 {
            return Err(crate::invalid(self.span, "type constructor limit exceeded"));
        }
        self.at = end + 1;
        self.value = Some(noble_kernel::types::Ty::program(inputs, outputs, effects));
        Ok(self)
    }

    fn fold(
        mut self,
        ty: noble_kernel::types::Ty,
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
                self.value = Some(attempt!(self
                    .environment
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

fn parse_program_stack(
    word: &str,
    types: &[(alloc::string::String, noble_kernel::types::Ty)],
    families: &[(alloc::string::String, noble_kernel::types::NominalTypeId)],
    environment: Option<&noble_kernel::contracts::Env>,
    span: crate::Span,
    depth: u32,
) -> Result<(alloc::vec::Vec<noble_kernel::types::Ty>, usize), crate::Diagnostic> {
    if word == "empty" {
        return Ok((alloc::vec::Vec::new(), 0));
    }
    let parts = attempt!(split_top_level(word, b'+', span));
    if parts.len() > 256 {
        return Err(crate::invalid(span, "Program stack height limit exceeded"));
    }
    let mut stack = alloc::vec::Vec::with_capacity(parts.len());
    let mut nodes = 0usize;
    for part in parts {
        let parsed = attempt!(parse_with_depth(
            part,
            types,
            families,
            environment,
            span,
            depth,
        ));
        nodes = nodes.saturating_add(parsed.nodes);
        stack.push(parsed.ty);
    }
    Ok((stack, nodes))
}

fn split_top_level(
    word: &str,
    separator: u8,
    span: crate::Span,
) -> Result<alloc::vec::Vec<&str>, crate::Diagnostic> {
    let mut parts = alloc::vec::Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    for (at, byte) in word.bytes().enumerate() {
        match byte {
            b'<' => depth = depth.saturating_add(1),
            b'>' => {
                depth = attempt!(depth
                    .checked_sub(1)
                    .ok_or_else(|| crate::invalid(span, "unbalanced type argument")));
            }
            byte if byte == separator && depth == 0 => {
                parts.push(&word[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    if depth != 0 || parts.len() > 256 {
        return Err(crate::invalid(
            span,
            "unbalanced or oversized type argument",
        ));
    }
    parts.push(&word[start..]);
    if parts.iter().any(|part| part.is_empty()) {
        return Err(crate::invalid(span, "empty type argument"));
    }
    Ok(parts)
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

fn contains_ascii_whitespace(bytes: &[u8]) -> bool {
    let mut at = 0usize;
    let mut is_found = false;
    while at < bytes.len() && !is_found {
        is_found = bytes[at].is_ascii_whitespace();
        at += 1;
    }
    is_found
}
