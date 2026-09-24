//! The bounded synchronous WIT boundary. WIT is parsed, not replaced by a
//! parallel IDL. Unsupported shapes fail before source or Wasm emission.
mod bindings;
mod parser;
mod preparation;

pub const PROFILE: &str = "Component-Sync-Bootstrap";
pub const MAX_OPERATIONS: usize = 62;
pub const MAX_RESOURCES: usize = 32;
pub const MAX_PARAMETERS: usize = 16;

/// The adapter profile is closed: new types require an exact Noble mapping
/// and a separately reviewed Canonical ABI adapter.
#[octet::sealed_enum]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Type {
    Boolean,
    S64,
    String,
    Bytes,
    ResultS64String,
    Own(noble_kernel::types::ResourceKind),
    Borrow(noble_kernel::types::ResourceKind),
}

impl Type {
    /// Bytes use checked u8 conversions at the boundary, never I64 truncation.
    /// Borrow maps to its guest owner only; no borrow token enters guest types.
    pub fn noble(self) -> noble_kernel::types::Ty {
        match self {
            Self::Boolean => noble_kernel::types::Ty::Bool,
            Self::S64 => noble_kernel::types::Ty::I64,
            Self::String => noble_kernel::types::Ty::Text,
            Self::Bytes => {
                noble_kernel::types::Ty::List(alloc::boxed::Box::new(noble_kernel::types::Ty::I64))
            }
            Self::ResultS64String => noble_kernel::types::Ty::Sum(
                alloc::boxed::Box::new(noble_kernel::types::Ty::I64),
                alloc::boxed::Box::new(noble_kernel::types::Ty::Text),
            ),
            Self::Own(kind) | Self::Borrow(kind) => noble_kernel::types::Ty::Resource(kind),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Resource {
    pub identity: alloc::string::String,
    pub interface: alloc::string::String,
    pub name: alloc::string::String,
    pub kind: noble_kernel::types::ResourceKind,
}

#[derive(Clone, Debug)]
pub struct Operation {
    /// Exact package/interface/version/function semantic identity.
    pub identity: alloc::string::String,
    /// Generated Noble spelling, e.g. arithmetic.inc or counter.read.
    pub word: alloc::string::String,
    /// Standard core Canonical ABI module and function names.
    pub core_module: alloc::string::String,
    pub core_name: alloc::string::String,
    /// Direct world name, or versioned interface#function for interface exports.
    pub export_name: alloc::string::String,
    pub parameters: alloc::vec::Vec<Type>,
    pub results: alloc::vec::Vec<Type>,
    /// Import-only identities; export contracts do not invent host effects.
    pub effect: Option<noble_kernel::types::EffId>,
    pub definition: Option<noble_kernel::contracts::Definition>,
}

impl Operation {
    pub fn input_types(&self) -> alloc::vec::Vec<noble_kernel::types::Ty> {
        let mut result = alloc::vec::Vec::with_capacity(self.parameters.len());
        let mut at = 0usize;
        while at < self.parameters.len() {
            result.push(self.parameters[at].noble());
            at = at.saturating_add(1);
        }
        result
    }
    /// Thread borrowed owners before results, in original parameter order.
    /// Owned parameters are NOT returned unless WIT explicitly returns ownership.
    pub fn output_types(&self) -> alloc::vec::Vec<noble_kernel::types::Ty> {
        let mut result = alloc::vec::Vec::with_capacity(
            self.parameters.len().saturating_add(self.results.len()),
        );
        let mut at = 0usize;
        while let Some(ty) = self.parameters.get(at) {
            if let Type::Borrow(kind) = ty {
                result.push(noble_kernel::types::Ty::Resource(*kind));
            }
            at = at.saturating_add(1);
        }
        at = 0;
        while at < self.results.len() {
            result.push(self.results[at].noble());
            at = at.saturating_add(1);
        }
        result
    }
}

#[derive(Clone, Debug)]
pub struct World {
    identity: alloc::string::String,
    name: alloc::string::String,
    wit: alloc::vec::Vec<u8>,
    imports: alloc::vec::Vec<Operation>,
    exports: alloc::vec::Vec<Operation>,
    resources: alloc::vec::Vec<Resource>,
    limits: crate::Limits,
}

impl World {
    pub fn parse(wit: &[u8], world: &str, limits: crate::Limits) -> Result<Self, Error> {
        parser::parse(wit, world, limits)
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn wit(&self) -> &[u8] {
        &self.wit
    }
    pub fn imports(&self) -> &[Operation] {
        &self.imports
    }
    pub fn exports(&self) -> &[Operation] {
        &self.exports
    }
    pub fn resources(&self) -> &[Resource] {
        &self.resources
    }
    pub const fn limits(&self) -> crate::Limits {
        self.limits
    }
    /// Full build binding, not a digest offered as authority.
    pub fn build_context(&self) -> alloc::vec::Vec<u8> {
        let capacity_bytes = PROFILE
            .len()
            .saturating_add(self.identity.len())
            .saturating_add(self.wit.len())
            .saturating_add(2);
        let mut key = alloc::vec::Vec::with_capacity(capacity_bytes);
        key.extend_from_slice(PROFILE.as_bytes());
        key.push(0);
        key.extend_from_slice(self.identity.as_bytes());
        key.push(0);
        key.extend_from_slice(&self.wit);
        key
    }
    /// Independently reconstruct the exact generated import environment.
    pub fn environment(&self) -> Result<noble_kernel::contracts::Env, Error> {
        Ok(attempt!(bindings::make(self)).environment)
    }
    pub fn session(&self) -> Result<crate::source::Session, Error> {
        Ok(crate::source::Session::with_bindings(attempt!(
            bindings::make(self)
        )))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Bindings {
    pub(crate) environment: noble_kernel::contracts::Env,
    pub(crate) words: alloc::vec::Vec<(alloc::string::String, noble_kernel::contracts::Definition)>,
    pub(crate) resources: alloc::vec::Vec<noble_kernel::types::ResourceKind>,
    pub(crate) effects: u64,
    pub(crate) key: alloc::vec::Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Wit,
    Binding,
    Export,
    Acceptance,
}

#[derive(Clone, Debug)]
pub struct Error {
    pub stage: Stage,
    pub diagnostic: crate::Diagnostic,
}

fn error(stage: Stage, kind: crate::DiagnosticKind, message: &str) -> Error {
    Error {
        stage,
        diagnostic: crate::Diagnostic::new(kind, crate::Span { start: 0, end: 0 }, message),
    }
}
fn invalid(message: &str) -> Error {
    error(Stage::Wit, crate::DiagnosticKind::Invalid, message)
}
fn unsupported(message: &str) -> Error {
    error(Stage::Wit, crate::DiagnosticKind::Unsupported, message)
}
fn exhausted() -> Error {
    error(
        Stage::Wit,
        crate::DiagnosticKind::Exhausted,
        "bounded WIT compiler limit exceeded",
    )
}

/// Only actual source preparation can construct an export. All getters borrow.
pub struct CheckedExport {
    world: alloc::vec::Vec<u8>,
    index: usize,
    source_bytes: alloc::vec::Vec<u8>,
    prepared: crate::source::Prepared,
}
impl CheckedExport {
    pub fn world_context(&self) -> &[u8] {
        &self.world
    }
    #[expect(
        tigerstyle::usize_in_public_api,
        reason = "Owner: noble-maintainers; this retained index selects the same World's borrowed exports slice, is never serialized, and is bounded by MAX_OPERATIONS during parsing; preserve the existing slice-index API."
    )]
    pub const fn index(&self) -> usize {
        self.index
    }
    pub fn source(&self) -> &[u8] {
        &self.source_bytes
    }
    pub fn submission(&self) -> Option<&noble_kernel::execution::Submission> {
        self.prepared.submission()
    }
}
