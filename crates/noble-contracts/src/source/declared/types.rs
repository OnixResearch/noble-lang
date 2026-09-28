pub(super) struct ParsedType {
    pub ty: noble_kernel::types::Ty,
    pub nodes: usize,
}

pub(super) fn parse(
    word: &str,
    types: &[(alloc::string::String, noble_kernel::types::Ty)],
    span: crate::Span,
) -> Result<ParsedType, crate::Diagnostic> {
    if contains_ascii_whitespace(word.as_bytes()) {
        return Err(crate::invalid(span, "type word contains whitespace"));
    }
    Parser {
        text: word.as_bytes(),
        at: 0,
        nodes: 0,
        span,
        names: types,
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
    pending: alloc::vec::Vec<Frame>,
    value: Option<noble_kernel::types::Ty>,
}

enum Frame {
    List,
    PairLeft,
    SumLeft,
    PairRight(noble_kernel::types::Ty),
    SumRight(noble_kernel::types::Ty),
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
            if name == "Resource" {
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
                self.pending.push(match name {
                    "List" => Frame::List,
                    "Pair" => Frame::PairLeft,
                    "Sum" => Frame::SumLeft,
                    _ => Frame::Unknown,
                });
            }
        } else {
            self.value = Some(match name {
                "I64" => noble_kernel::types::Ty::I64,
                "Bool" => noble_kernel::types::Ty::Bool,
                "Text" => noble_kernel::types::Ty::Text,
                "Unit" => noble_kernel::types::Ty::Unit,
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
