//! The checking environment: one scheme per definition, its behavior kind,
//! its external dependency data, the user-declared schemas, and the
//! supplied effect-identity table.
//!
//! Contracts are rank-1 schemes; the bootstrap table lives in `bootstrap`.

pub mod bootstrap;
mod live;
mod nominal;

/// Identity of one environment definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Definition(pub u32);

/// The reserved identity of the resource-free `test.emit` effect.
pub const TEST_EMIT: crate::types::EffId = crate::types::EffId(0);
/// The reserved identity of the opt-in scripted `test.clock` effect.
pub const TEST_CLOCK: crate::types::EffId = crate::types::EffId(2);
/// The reserved, opt-in effect of selecting a preadmitted live program.
pub const LIVE_DISPATCH: crate::types::EffId = crate::types::EffId(5);

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
    /// Pure byte-offset inspection of an immutable Text in the opt-in cursor profile.
    TextByte,
    /// Non-callable slots preserving globally disjoint builtin identities.
    Reserved,
    /// The supplied `test.emit` operation.
    TestEmit,
    /// A required `test.emit` binding, retaining the immutable adapter slot.
    BoundEmit(u32),
    /// A required scripted `test.clock` binding, retaining its immutable slot.
    BoundClock(u32),
    /// Explicit opaque construction and conversion.
    NominalNew(crate::types::NominalTypeId),
    NominalInto(crate::types::NominalTypeId),
    /// Ordered variant constructors and exhaustive elimination.
    NominalLeft(crate::types::NominalTypeId),
    NominalRight(crate::types::NominalTypeId),
    NominalMatch(crate::types::NominalTypeId),
    /// Source-declared rank-1 variant family operations, shared by every
    /// concrete instantiation of the family.
    GenericLeft(crate::types::NominalTypeId),
    GenericRight(crate::types::NominalTypeId),
    GenericMatch(crate::types::NominalTypeId),
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

/// A source-declared family with exactly two type parameters. Each arm's
/// payload is the indicated parameter (0 or 1), in source-declared order.
#[derive(Clone, Debug)]
pub struct GenericVariantDecl {
    pub id: crate::types::NominalTypeId,
    pub payload_params: [u8; 2],
    pub exported: bool,
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

/// Pending scripted clock binding; not authority to consult a real clock.
pub struct BoundClockRegistration {
    pub adapter_identity: alloc::string::String,
    pub adapter_slot: u32,
    pub owner: u64,
    pub input: alloc::vec::Vec<crate::types::Ty>,
    pub output: alloc::vec::Vec<crate::types::Ty>,
    pub effects: crate::types::EffSet,
}

/// One precomputed, source-independent test-host dispatch decision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockDecision {
    Value(i64),
    Denied,
    ScriptExhausted,
    UnexpectedOperation,
}

/// A finite, explicitly typed test clock, not an ambient clock or authority.
pub struct ClockPlan<'a> {
    pub operation: &'a str,
    pub adapter_identity: &'a str,
    pub input: &'a [crate::types::Ty],
    pub output: &'a [crate::types::Ty],
    pub effects: &'a [crate::types::EffId],
    pub allowed: bool,
    pub script: &'a [i64],
}

impl ClockPlan<'_> {
    /// Check the exact binding anew and choose one dispatch result. The shell
    /// receives a finite table of these decisions; it never consults a real clock.
    pub fn decide(&self, requested: &str, index: usize) -> Result<ClockDecision, NominalError> {
        if self.operation != "test.clock"
            || self.adapter_identity.is_empty()
            || !self.input.is_empty()
            || self.output != [crate::types::Ty::I64]
            || self.effects != [TEST_CLOCK]
            || self.script.len() > 16
        {
            return Err(NominalError::InvalidRepresentation);
        }
        if requested != self.operation {
            return Ok(ClockDecision::UnexpectedOperation);
        }
        if !self.allowed {
            return Ok(ClockDecision::Denied);
        }
        Ok(match self.script.get(index) {
            Some(value) => ClockDecision::Value(*value),
            None => ClockDecision::ScriptExhausted,
        })
    }
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
    /// Source-declared two-parameter variants; never expanded per instance.
    pub generic_variants: alloc::vec::Vec<GenericVariantDecl>,
    /// Kinds actually admitted by the host; the fixture is explicit.
    pub resource_kinds: alloc::vec::Vec<crate::types::ResourceKind>,
    /// Host-registered, versioned opaque resource identities without guest
    /// constructors. Each has a distinct kind in `resource_kinds`.
    pub live_resource_nominals: alloc::vec::Vec<crate::types::NominalTypeId>,
    /// Independently selected live-slot profile; absent in Core and draft.
    pub live_slots: bool,
    /// Caller selected by the host for this independent acceptance run.
    pub caller_module: Option<u64>,
    /// Explicit opt-in profile selected outside the untrusted candidate.
    /// Module registration also enables it; it is required for declarations
    /// with no nominal types or bound operations.
    pub declared_modules: bool,
    /// Separately selected pure Text byte cursor profile; never part of Core or declared modules.
    pub text_cursor: bool,
    /// Owner of each definition, supplied independently of a candidate.
    pub definition_owners: alloc::vec::Vec<Option<u64>>,
    /// Host-confirmed immutable dispatch identities for required operations.
    pub bound_adapters: alloc::vec::Vec<BoundAdapter>,
    /// Effect identities this environment provides; `TEST_EMIT` is first.
    pub effects: alloc::vec::Vec<crate::types::EffId>,
}

impl Env {
    /// Select live-slot checking outside the untrusted candidate.
    pub fn enable_live_slots(mut self) -> Self {
        if !self.live_slots {
            self.live_slots = true;
            self.declared_modules = true;
            self.effects.push(LIVE_DISPATCH);
        }
        self
    }

    /// Register a host-owned versioned nominal backed by a unique opaque
    /// resource kind. No guest-accessible `new` or `into` operation is made.
    pub fn register_live_resource(self, decl: NominalDecl) -> Result<Self, NominalError> {
        live::register_resource(self, decl)
    }

    /// Validate a live reference's full ordered interface, including nested
    /// borrowed inputs but never borrowed outputs or captured values.
    pub fn valid_live_ref(&self, ty: &crate::types::Ty) -> bool {
        live::valid_ref(self, ty)
    }

    /// Find one resolved nominal descriptor, without treating an alias as an identity.
    pub fn nominal(&self, id: crate::types::NominalTypeId) -> Option<&NominalDecl> {
        let mut index = 0;
        while index < self.nominals.len() && self.nominals[index].id != id {
            index += 1;
        }
        self.nominals.get(index)
    }

    /// Find a generic family by its resolved declaration identity.
    pub fn generic_variant(&self, id: crate::types::NominalTypeId) -> Option<&GenericVariantDecl> {
        let mut index = 0;
        while index < self.generic_variants.len() && self.generic_variants[index].id != id {
            index += 1;
        }
        self.generic_variants.get(index)
    }

    /// Construct a checked canonical instance, retaining argument order and
    /// the exact descriptor derived from its family declaration.
    pub fn generic_instance(
        &self,
        id: crate::types::NominalTypeId,
        args: [crate::types::Ty; 2],
    ) -> Option<crate::types::Ty> {
        let decl = self.generic_variant(id)?;
        if (!decl.exported && self.caller_module != Some(id.module))
            || !args[0].valid_generic_argument()
            || !args[1].valid_generic_argument()
            || !self.valid_nominal_payload(&args[0], 512)
            || !self.valid_nominal_payload(&args[1], 512)
            || !self.valid_type(&args[0], 512)
            || !self.valid_type(&args[1], 512)
        {
            return None;
        }
        let shape = nominal::generic_shape(decl, &args)?;
        Some(crate::types::Ty::GenericNominal(
            id,
            alloc::boxed::Box::new(args),
            alloc::boxed::Box::new(shape),
        ))
    }

    /// Borrowed exact-instance check for consumers of an independently
    /// checked family; no per-instance declaration is registered.
    pub fn generic_instance_matches(
        &self,
        ty: &crate::types::Ty,
        id: crate::types::NominalTypeId,
    ) -> bool {
        let crate::types::Ty::GenericNominal(actual_id, _, _) = ty else {
            return false;
        };
        if *actual_id != id {
            return false;
        }
        self.generic_variant(id).is_some() && self.valid_type(ty, 512)
    }

    /// Check a borrowed instance without a separately supplied family id.
    pub fn valid_generic_instance(&self, ty: &crate::types::Ty) -> bool {
        match ty {
            crate::types::Ty::GenericNominal(id, _, _) => self.generic_instance_matches(ty, *id),
            _ => false,
        }
    }

    /// Register a validated nonrecursive type and its typed operations.
    /// Ownership transfers into the updated environment on success.
    pub fn declare_nominal(self, decl: NominalDecl) -> Result<(Self, NominalOps), NominalError> {
        let resource_payload = match &decl.shape {
            crate::types::NominalShape::Opaque(inner) => !inner.is_data(),
            crate::types::NominalShape::Variant(left, right) => {
                !left.is_data() || !right.is_data()
            }
        };
        if self.live_slots && resource_payload {
            return Err(NominalError::InvalidRepresentation);
        }
        nominal::register(self, decl)
    }

    /// Register one validated generic variant and its three rank-1
    /// operations, without generating definitions per concrete instance.
    pub fn declare_generic_variant(
        self,
        decl: GenericVariantDecl,
    ) -> Result<(Self, NominalOps), NominalError> {
        nominal::register_generic_variant(self, decl)
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

    /// Register only the exact `[] -- I64 ! test.clock` scripted host contract.
    /// The separately supplied host policy and finite script grant no authority here.
    pub fn declare_bound_clock(
        self,
        registration: BoundClockRegistration,
    ) -> Result<(Self, Definition), NominalError> {
        nominal::register_bound_clock(self, registration)
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
        generic_variants: alloc::vec::Vec::new(),
        resource_kinds: alloc::vec![FIXTURE_RESOURCE],
        live_resource_nominals: alloc::vec::Vec::new(),
        live_slots: false,
        caller_module: None,
        declared_modules: false,
        text_cursor: false,
        definition_owners: alloc::vec![None; table.len()],
        bound_adapters: alloc::vec::Vec::new(),
        effects: alloc::vec![TEST_EMIT],
    })
}

/// Add the single pure Text cursor contract without changing the bootstrap table.
pub fn text_cursor_environment() -> Result<Env, crate::shapes::Defect> {
    use crate::shapes::Pattern;
    use crate::words::{Scheme, Variable, VariableKind};

    let mut env = environment()?;
    let reserved = Scheme {
        var_kinds: alloc::vec![],
        stack_in: alloc::vec![],
        stack_out: alloc::vec![],
        effects: alloc::vec![],
    };
    reserved.validate()?;
    for _ in 23..26 {
        env.defs.push(reserved.clone());
        env.kinds.push(Behavior::Reserved);
        env.deps.push(alloc::vec::Vec::new());
        env.definition_owners.push(None);
    }
    let tail = Pattern::StackVar(Variable(0));
    let scheme = Scheme {
        var_kinds: alloc::vec![VariableKind::Stack],
        stack_in: alloc::vec![tail.clone(), Pattern::Text, Pattern::I64],
        stack_out: alloc::vec![tail, Pattern::Text, Pattern::I64],
        effects: alloc::vec![],
    };
    scheme.validate()?;
    env.defs.push(scheme);
    env.kinds.push(Behavior::TextByte);
    env.deps.push(alloc::vec::Vec::new());
    env.definition_owners.push(None);
    env.text_cursor = true;
    Ok(env)
}
