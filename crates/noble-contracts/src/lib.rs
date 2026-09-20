#![no_std]
#![feature(register_tool)]
#![register_tool(charon, octet, tigerstyle)]

extern crate alloc;

macro_rules! attempt {
    ($step:expr) => {
        match $step {
            Ok(value) => value,
            Err(failure) => return Err(failure),
        }
    };
}

mod frontend;
mod inference;
mod predicate;
mod program;
mod rendering;
mod syntax;
pub mod wire;
pub use rendering::export_lean;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub bytes: u32,
    pub nodes: u32,
    pub depth: u32,
    pub work: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticKind {
    Invalid,
    Unsupported,
    Exhausted,
    Internal,
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub span: Span,
    pub message: alloc::string::String,
    ordinary_typing: Option<alloc::boxed::Box<noble_kernel::untrusted::Checked>>,
}

impl Diagnostic {
    pub fn ordinary_typing(&self) -> Option<&noble_kernel::untrusted::Checked> {
        self.ordinary_typing.as_deref()
    }

    pub(crate) fn new(kind: DiagnosticKind, span: Span, message: &str) -> Self {
        Self {
            kind,
            span,
            message: alloc::string::String::from(message),
            ordinary_typing: None,
        }
    }

    pub(crate) fn with_typing(mut self, checked: noble_kernel::untrusted::Checked) -> Self {
        self.ordinary_typing = Some(alloc::boxed::Box::new(checked));
        self
    }
}

#[derive(Clone, Debug)]
pub struct NamedType {
    pub name: alloc::string::String,
    pub ty: noble_kernel::types::Ty,
}

#[derive(Clone, Debug)]
pub struct LogicDef {
    pub name: alloc::string::String,
    pub ty: noble_kernel::types::Ty,
    pub body: u32,
}

#[derive(Clone, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: noble_kernel::types::Ty,
    pub span: Span,
    total: bool,
    uses_output: bool,
}

impl Expr {
    /// False means evaluation can be undefined (not a false Boolean value).
    pub fn is_total(&self) -> bool {
        self.total
    }

    pub fn uses_output(&self) -> bool {
        self.uses_output
    }
}

#[derive(Clone, Debug)]
#[charon::variants_suffix("Expr")]
#[octet::sealed_enum]
pub enum ExprKind {
    I64(i64),
    Bool(bool),
    Unit,
    Input(u32),
    Output(u32),
    Param(u32),
    Definition(u32),
    Not(u32),
    And(u32, u32),
    Or(u32, u32),
    Implies(u32, u32),
    Eq(u32, u32),
    Lt(u32, u32),
    Le(u32, u32),
    Add(u32, u32),
    Sub(u32, u32),
    Mul(u32, u32),
    Pair(u32, u32),
    First(u32),
    Second(u32),
    Inl(u32),
    Inr(u32),
    IsLeft(u32),
    Left(u32),
    Right(u32),
    Nil,
    Cons(u32, u32),
    IsNil(u32),
    Head(u32),
    Tail(u32),
    Length(u32),
    Maps(u32, u32, u32),
}

/// Only preparation can construct a value; every getter is immutable.
#[derive(Clone, Debug)]
pub struct Prepared {
    name: alloc::string::String,
    inputs: alloc::vec::Vec<NamedType>,
    outputs: alloc::vec::Vec<NamedType>,
    params: alloc::vec::Vec<NamedType>,
    definitions: alloc::vec::Vec<LogicDef>,
    expressions: alloc::vec::Vec<Expr>,
    requires: u32,
    ensures: u32,
    candidate: noble_kernel::untrusted::Candidate,
    request: noble_kernel::untrusted::Request,
    checked: noble_kernel::untrusted::Checked,
}

impl Prepared {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn inputs(&self) -> &[NamedType] {
        &self.inputs
    }
    pub fn outputs(&self) -> &[NamedType] {
        &self.outputs
    }
    pub fn params(&self) -> &[NamedType] {
        &self.params
    }
    pub fn definitions(&self) -> &[LogicDef] {
        &self.definitions
    }
    pub fn expressions(&self) -> &[Expr] {
        &self.expressions
    }
    pub fn requires(&self) -> u32 {
        self.requires
    }
    pub fn ensures(&self) -> u32 {
        self.ensures
    }
    pub fn candidate(&self) -> &noble_kernel::untrusted::Candidate {
        &self.candidate
    }
    pub fn request(&self) -> &noble_kernel::untrusted::Request {
        &self.request
    }
    pub fn checked(&self) -> &noble_kernel::untrusted::Checked {
        &self.checked
    }
}

pub fn prepare(source: &[u8], limits: Limits) -> Result<Prepared, Diagnostic> {
    frontend::prepare(source, limits)
}

pub(crate) struct Meter {
    pub limits: Limits,
    work: u32,
    nodes: u32,
}

impl Meter {
    pub fn new(limits: Limits) -> Self {
        Self {
            limits,
            work: limits.work,
            nodes: 0,
        }
    }

    pub fn charge(&mut self, amount: u32, span: Span) -> Result<(), Diagnostic> {
        match self.work.checked_sub(amount) {
            Some(remaining) => {
                self.work = remaining;
                Ok(())
            }
            None => Err(Diagnostic::new(
                DiagnosticKind::Exhausted,
                span,
                "frontend work limit exceeded",
            )),
        }
    }

    pub fn node(&mut self, span: Span) -> Result<(), Diagnostic> {
        attempt!(self.charge(1, span));
        if self.nodes >= self.limits.nodes {
            return Err(Diagnostic::new(
                DiagnosticKind::Exhausted,
                span,
                "frontend node limit exceeded",
            ));
        }
        self.nodes += 1;
        Ok(())
    }

    pub fn depth(&mut self, depth: u32, span: Span) -> Result<(), Diagnostic> {
        attempt!(self.charge(1, span));
        if depth > self.limits.depth {
            return Err(Diagnostic::new(
                DiagnosticKind::Exhausted,
                span,
                "frontend depth limit exceeded",
            ));
        }
        Ok(())
    }
}

pub(crate) fn invalid(span: Span, message: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::Invalid, span, message)
}

pub(crate) fn internal(span: Span) -> Diagnostic {
    Diagnostic::new(
        DiagnosticKind::Internal,
        span,
        "inconsistent frontend arena",
    )
}

pub(crate) fn index(value: usize, span: Span) -> Result<u32, Diagnostic> {
    match u32::try_from(value) {
        Ok(value) => Ok(value),
        Err(_) => Err(Diagnostic::new(
            DiagnosticKind::Exhausted,
            span,
            "index exceeds the finite format",
        )),
    }
}

pub(crate) fn offset(value: u32, span: Span) -> Result<usize, Diagnostic> {
    match usize::try_from(value) {
        Ok(value) => Ok(value),
        Err(_) => Err(Diagnostic::new(
            DiagnosticKind::Exhausted,
            span,
            "index exceeds the host address space",
        )),
    }
}
