//! The checking environment: one scheme per definition, its behavior kind,
//! its external dependency data, the user-declared schemas, and the
//! supplied effect-identity table.
//!
//! Contracts are rank-1 schemes; the bootstrap table lives in `bootstrap`.

pub mod bootstrap;
mod nominal;

/// Identity of one environment definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Definition(pub u32);

/// The reserved identity of the resource-free `test.emit` effect.
pub const TEST_EMIT: crate::types::EffId = crate::types::EffId(0);

/// The reserved resource kind used by negative eligibility fixtures.
pub const FIXTURE_RESOURCE: crate::types::ResourceKind = crate::types::ResourceKind(0);

/// Identity of one user-declared schema in external environment data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaId(pub u32);

/// What a definition's contract constrains beyond its scheme.
///
/// The extracted constructors carry a `Behavior` suffix: `Unit` would
/// otherwise shadow Lean's own `Unit` inside every declaration named
/// `contracts.Behavior.*`, where this type's instances live.
#[charon::variants_suffix("Behavior")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Behavior {
    /// `dup`: requires `Data` on its value variable.
    Dup,
    /// `drop`: requires `Data` on its value variable.
    Drop,
    /// `swap`.
    Swap,
    /// `dip`.
    Dip,
    /// `+`, `-`, or `*`; wrapping `I64`.
    Arith,
    /// `=`: `I64` equality yielding `Bool`.
    Equals,
    /// `quote`: requires `Data` on its captured variable.
    Quote,
    /// `compose`.
    Compose,
    /// `run`.
    Run,
    /// `reflect`.
    Reflect,
    /// `unit`.
    Unit,
    /// `pair`.
    Pair,
    /// `unpair`.
    Unpair,
    /// `inl`.
    Inl,
    /// `inr`.
    Inr,
    /// `case`.
    Case,
    /// `if`.
    If,
    /// `nil`.
    Nil,
    /// `cons`.
    Cons,
    /// `list.case`.
    ListCase,
    /// The supplied `test.emit` operation.
    TestEmit,
    /// A required `test.emit` binding, retaining the immutable adapter slot.
    BoundEmit(u32),
    /// Explicit opaque construction and conversion.
    NominalNew(crate::types::NominalTypeId),
    NominalInto(crate::types::NominalTypeId),
    /// Ordered variant constructors and exhaustive elimination.
    NominalLeft(crate::types::NominalTypeId),
    NominalRight(crate::types::NominalTypeId),
    NominalMatch(crate::types::NominalTypeId),
    /// An environment-supplied named definition with no extra constraint.
    Named,
}

/// One user-declared schema in external environment data.
///
/// A declaration that names itself — a recursive schema — is unsupported
/// (B-CHECK-02) and rejected during preflight; a non-recursive declaration
/// is still validated as a scheme before any candidate body is checked.
#[derive(Clone, Debug)]
pub struct SchemaDecl {
    /// The declared schema's identity.
    pub id: SchemaId,
    /// The declared contract.
    pub scheme: crate::words::Scheme,
    /// Whether the declaration names itself.
    pub recursive: bool,
}

/// Checked descriptor independently supplied as part of a module interface.
#[derive(Clone, Debug)]
pub struct NominalDecl {
    pub id: crate::types::NominalTypeId,
    pub shape: crate::types::NominalShape,
    /// Export of the type itself, distinct from constructor visibility.
    pub exported: bool,
    /// Opaque: `new`/`into`. Variant: left/right constructors, in order.
    pub public: [bool; 2],
}

/// Definitions assigned by `declare_nominal` in the environment.
#[derive(Clone, Copy, Debug)]
pub struct NominalOps {
    pub new: Option<Definition>,
    pub into: Option<Definition>,
    pub left: Option<Definition>,
    pub right: Option<Definition>,
    pub matcher: Option<Definition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NominalError {
    DuplicateIdentity,
    InvalidRepresentation,
    TooManyDefinitions,
}

/// Trusted manifest mapping for one required host operation.
#[derive(Clone, Debug)]
pub struct BoundAdapter {
    pub definition: Definition,
    pub adapter_identity: alloc::string::String,
    pub adapter_slot: u32,
    pub input: alloc::vec::Vec<crate::types::Ty>,
    pub output: alloc::vec::Vec<crate::types::Ty>,
    pub effects: crate::types::EffSet,
}

/// Pending required host operation, before a definition identity is assigned.
pub struct BoundEmitRegistration {
    pub adapter_identity: alloc::string::String,
    pub adapter_slot: u32,
    pub owner: u64,
    pub input: alloc::vec::Vec<crate::types::Ty>,
    pub output: alloc::vec::Vec<crate::types::Ty>,
    pub effects: crate::types::EffSet,
}

/// The checking environment. `Default` is an empty staging replacement;
/// checking uses the validated environment returned by `environment`.
#[derive(Clone, Debug, Default)]
pub struct Env {
    /// Schemes indexed by `Definition`.
    pub defs: alloc::vec::Vec<crate::words::Scheme>,
    /// Behavior kinds indexed by `Definition`.
    pub kinds: alloc::vec::Vec<Behavior>,
    /// Per-definition dependency lists from external environment data; the
    /// bootstrap table declares none, and a missing entry means none.
    pub deps: alloc::vec::Vec<alloc::vec::Vec<Definition>>,
    /// User-declared schemas from external environment data.
    pub schemas: alloc::vec::Vec<SchemaDecl>,
    /// Canonical nonrecursive descriptors, addressed by resolved identities.
    pub nominals: alloc::vec::Vec<NominalDecl>,
    /// Kinds actually admitted by the host; the fixture is explicit.
    pub resource_kinds: alloc::vec::Vec<crate::types::ResourceKind>,
    /// Caller selected by the host for this independent acceptance run.
    pub caller_module: Option<u64>,
    /// Explicit opt-in profile selected outside the untrusted candidate.
    /// Module registration also enables it; it is required for declarations
    /// with no nominal types or bound operations.
    pub declared_modules: bool,
    /// Owner of each definition, supplied independently of a candidate.
    pub definition_owners: alloc::vec::Vec<Option<u64>>,
    /// Host-confirmed immutable dispatch identities for required operations.
    pub bound_adapters: alloc::vec::Vec<BoundAdapter>,
    /// Effect identities this environment provides; `TEST_EMIT` is first.
    pub effects: alloc::vec::Vec<crate::types::EffId>,
}

impl Env {
    /// Find one resolved nominal descriptor, without treating an alias as an identity.
    pub fn nominal(&self, id: crate::types::NominalTypeId) -> Option<&NominalDecl> {
        let mut index = 0;
        while index < self.nominals.len() && self.nominals[index].id != id {
            index += 1;
        }
        self.nominals.get(index)
    }

    /// Register a validated nonrecursive type and its typed operations.
    /// Ownership transfers into the updated environment on success.
    pub fn declare_nominal(self, decl: NominalDecl) -> Result<(Self, NominalOps), NominalError> {
        nominal::register(self, decl)
    }

    /// Register an immutable required operation, exactly `Text -- ! test.emit`.
    /// Its operation name and requirement are resolved by the module frontend.
    /// Ownership transfers into the updated environment on success.
    pub fn declare_bound_emit(
        self,
        registration: BoundEmitRegistration,
    ) -> Result<(Self, Definition), NominalError> {
        nominal::register_bound_emit(self, registration)
    }

    /// The scheme of a definition.
    pub fn scheme(&self, def: Definition) -> Option<&crate::words::Scheme> {
        match index(def) {
            Some(index) => self.defs.get(index),
            None => None,
        }
    }

    /// The behavior of a definition.
    pub fn kind(&self, def: Definition) -> Option<Behavior> {
        match index(def) {
            Some(index) => self.kinds.get(index).copied(),
            None => None,
        }
    }

    /// Whether the environment provides this effect identity.
    pub fn knows_effect(&self, id: crate::types::EffId) -> bool {
        let mut index = 0;
        while index < self.effects.len() {
            if self.effects[index] == id {
                return true;
            }
            index += 1;
        }
        false
    }

    /// Number of definitions.
    pub fn len(&self) -> Option<u64> {
        u64::try_from(self.defs.len()).ok()
    }

    /// Whether the environment has no definitions.
    pub fn is_empty(&self) -> bool {
        self.defs.len() == 0
    }
}

fn index(def: Definition) -> Option<usize> {
    usize::try_from(def.0).ok()
}

/// Build the bootstrap environment with the fixed v1 contract table.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; environment validates every bootstrap scheme before constructing its parallel definition/kind/dependency tables and returns the first Defect; assertions would change this fallible construction contract."
)]
pub fn environment() -> Result<Env, crate::shapes::Defect> {
    let table = bootstrap::data::table();
    let mut defs: alloc::vec::Vec<crate::words::Scheme> = alloc::vec::Vec::with_capacity(32);
    let mut kinds: alloc::vec::Vec<Behavior> = alloc::vec::Vec::with_capacity(32);
    let mut deps: alloc::vec::Vec<alloc::vec::Vec<Definition>> = alloc::vec::Vec::with_capacity(32);
    let mut index = 0;
    while index < table.len() {
        match table[index].1.validate() {
            Ok(()) => {
                kinds.push(table[index].0);
                index += 1;
            }
            Err(problem) => return Err(problem),
        }
    }
    let mut table_index = 0;
    while table_index < table.len() {
        defs.push(table[table_index].1.clone());
        #[expect(
            tigerstyle::allocation_in_loop,
            reason = "Owner: noble-maintainers; Vec::new allocates no storage and records the bootstrap definition's empty dependency list; reassess if this changes to a nonzero-capacity constructor."
        )]
        deps.push(alloc::vec::Vec::new());
        table_index += 1;
    }
    Ok(Env {
        defs,
        kinds,
        deps,
        schemas: alloc::vec::Vec::new(),
        nominals: alloc::vec::Vec::new(),
        resource_kinds: alloc::vec![FIXTURE_RESOURCE],
        caller_module: None,
        declared_modules: false,
        definition_owners: alloc::vec![None; table.len()],
        bound_adapters: alloc::vec::Vec::new(),
        effects: alloc::vec![TEST_EMIT],
    })
}
