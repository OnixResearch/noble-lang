//! Bounded Core-Bootstrap preparation. This module never executes guest code.
//!
//! Named bodies retain resolved dependency identities. Their type constraints
//! are instantiated afresh at each named use, while a first-class Program has
//! one shared inference term. Every executable specialization is subsequently
//! checked by the kernel against its exact concrete environment contract.

mod declared;
pub mod editor;
mod emission;
mod inference;
mod lexer;
mod locals;
mod parsing;
mod preflight;
mod preparation;
pub mod proof;
mod replacement;
mod resolution;

pub(crate) use declared::{
    verify_definition_origin, verify_named_call_origin, verify_named_export, DefinitionOrigin,
};
pub use declared::{
    BoundOperation, CheckedNominal, CheckedProgramInterface, CheckedProgramOwner,
    DeclaredTextSourceOwner, ModuleKind, ModulePrepared, ModuleSession,
    RebuiltImportDefinition, SignedSourceOwner,
};

/// Core vocabulary resolved by the same checker as a worker's pure source.
/// Test-host, live and selected cursor operations have different authority.
pub const fn pure_builtin_spelling(identity: u32) -> Option<&'static str> {
    if identity < 22 {
        crate::program::bootstrap_spelling(identity)
    } else {
        None
    }
}

/// Immutable source authority for the versioned Result interface. The same
/// declaration bytes are checked through ordinary module registration.
pub const RESULT_LIBRARY_SOURCE: &[u8] = include_bytes!("fixtures/result.noble");

/// Host-selected aggregate of original module bytes and outside definition history.
/// This does not raise the per-submission `Limits::bytes` parser bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedSourceLimit {
    bytes: u32,
}

impl RetainedSourceLimit {
    pub const MAX_BYTES: u32 = 262_144;

    pub const fn new(bytes: u32) -> Option<Self> {
        if bytes > Self::MAX_BYTES {
            None
        } else {
            Some(Self { bytes })
        }
    }

    pub const fn bytes(self) -> u32 {
        self.bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Parse,
    Resolve,
    Link,
    Check,
    Acceptance,
}

#[derive(Clone, Debug)]
pub struct Error {
    stage: Stage,
    diagnostic: crate::Diagnostic,
}

impl Error {
    pub const fn stage(&self) -> Stage {
        self.stage
    }
    pub const fn diagnostic(&self) -> &crate::Diagnostic {
        &self.diagnostic
    }
    const fn at(stage: Stage, diagnostic: crate::Diagnostic) -> Self {
        Self { stage, diagnostic }
    }
}

/// An independently supplied, ordered Core stack interface, not a checked
/// candidate or permission to execute the indicated values.
#[derive(Clone, Debug, PartialEq)]
pub struct CoreInterface {
    pub stack_in: alloc::vec::Vec<noble_kernel::types::Ty>,
    pub stack_out: alloc::vec::Vec<noble_kernel::types::Ty>,
}

/// Parse host-selected Core interface words with the source type parser.
/// The C source-only caller separately checks this against kernel acceptance
/// and still refuses value forms its emitter cannot lower.
pub fn parse_core_interface(text: &str) -> Result<CoreInterface, Error> {
    let span = crate::Span {
        start: 0,
        end: u32::try_from(text.len()).unwrap_or(u32::MAX),
    };
    if text.len() > 4096 {
        return Err(Error::at(Stage::Check, crate::invalid(span, "Core interface bytes exhausted")));
    }
    let mut stack_in = alloc::vec::Vec::new();
    let mut stack_out = alloc::vec::Vec::new();
    let mut separator = false;
    for word in text.split_ascii_whitespace() {
        if word == "--" {
            if separator {
                return Err(Error::at(Stage::Check, crate::invalid(span, "duplicate stack separator")));
            }
            separator = true;
            continue;
        }
        let stack = if separator { &mut stack_out } else { &mut stack_in };
        if stack.len() >= 128 {
            return Err(Error::at(Stage::Check, crate::invalid(span, "Core stack height exhausted")));
        }
        let parsed = declared::types::parse_with_families(word, &[], &[], None, span)
            .map_err(|error| Error::at(Stage::Check, error))?;
        if !core_type(&parsed.ty) {
            return Err(Error::at(Stage::Check, crate::invalid(span, "unsupported Core interface type")));
        }
        stack.push(parsed.ty);
    }
    if !separator {
        return Err(Error::at(Stage::Check, crate::invalid(span, "missing stack separator")));
    }
    Ok(CoreInterface { stack_in, stack_out })
}

fn core_type(ty: &noble_kernel::types::Ty) -> bool {
    use noble_kernel::types::Ty;
    match ty {
        Ty::I64 | Ty::Bool | Ty::Unit | Ty::Text | Ty::Syntax => true,
        Ty::List(item) => core_type(item),
        Ty::Pair(left, right) | Ty::Sum(left, right) => core_type(left) && core_type(right),
        Ty::Program(input, output, effects) => {
            effects.as_slice().is_empty()
                && input.iter().all(core_type)
                && output.iter().all(core_type)
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Builtin(u32),
    Named(u32),
    /// Finite lexical self occurrence; never a namespace lookup or an
    /// installed-definition index until the entire owner is checked.
    SelfOwner,
    SlotInvoke(Option<u32>),
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Literal(noble_kernel::untrusted::Lit),
    Text(alloc::vec::Vec<u8>),
    Word(alloc::vec::Vec<u8>),
    Call(Target),
    Quotation(alloc::vec::Vec<u32>),
    EditorHole,
}

#[derive(Clone, Debug, PartialEq)]
struct Node {
    kind: Kind,
    span: crate::Span,
}

#[derive(Clone, Debug, PartialEq)]
struct Tree {
    nodes: alloc::vec::Vec<Node>,
    body: alloc::vec::Vec<u32>,
    span: crate::Span,
}

impl Tree {
    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; node lookup validates the arena offset and constructs an owned diagnostic for an invalid ID."
    )]
    fn node(&self, id: u32) -> Result<&Node, crate::Diagnostic> {
        match self.nodes.get(attempt!(crate::offset(id, self.span))) {
            Some(node) => Ok(node),
            None => Err(crate::internal(self.span)),
        }
    }
}

/// Follow immutable named targets, including calls inside quotations. A
/// replacement must not enter the old identity through any historical body:
/// rebuilding current dependents would otherwise publish an apparent cycle
/// whose new root still invokes the old root.
fn reaches_index(
    tree: &Tree,
    definitions: &[Named],
    target: usize,
    mut work: u32,
) -> Result<bool, Error> {
    let span = tree.span;
    let mut visited = alloc::vec![false; definitions.len()];
    let mut pending = alloc::vec::Vec::new();
    let mut current = Some(tree);
    while let Some(body) = current {
        for node in &body.nodes {
            work = work.checked_sub(1).ok_or_else(|| {
                Error::at(
                    Stage::Acceptance,
                    exhausted(span, "live replacement dependency walk exhausted"),
                )
            })?;
            if let Kind::Call(Target::Named(index)) = &node.kind {
                let index = *index as usize;
                if index == target {
                    return Ok(true);
                }
                let seen = visited.get_mut(index).ok_or_else(|| {
                    Error::at(
                        Stage::Acceptance,
                        crate::invalid(span, "invalid retained definition target"),
                    )
                })?;
                if !*seen {
                    *seen = true;
                    pending.push(index);
                }
            }
        }
        current = pending.pop().map(|index| &definitions[index].tree);
    }
    Ok(false)
}

/// Replay is allowed to change exactly the named edges rebuilt in this batch.
/// Source spelling alone is not authority to retarget a shadowed historical
/// call to today's same-name definition.
fn replay_preserves_targets(
    original: &Tree,
    replayed: &Tree,
    replacements: &[Option<usize>],
) -> bool {
    if original.body != replayed.body || original.nodes.len() != replayed.nodes.len() {
        return false;
    }
    original
        .nodes
        .iter()
        .zip(&replayed.nodes)
        .all(|(old, new)| match (&old.kind, &new.kind) {
            (Kind::Literal(a), Kind::Literal(b)) => a == b,
            (Kind::Text(a), Kind::Text(b)) => a == b,
            (Kind::Quotation(a), Kind::Quotation(b)) => a == b,
            (Kind::Call(Target::Builtin(a)), Kind::Call(Target::Builtin(b))) => a == b,
            (Kind::Call(Target::SlotInvoke(a)), Kind::Call(Target::SlotInvoke(b))) => a == b,
            (Kind::Call(Target::SelfOwner), Kind::Call(Target::SelfOwner)) => true,
            (Kind::Call(Target::Named(a)), Kind::Call(Target::Named(b))) => replacements
                .get(*a as usize)
                .is_some_and(|replacement| replacement.unwrap_or(*a as usize) == *b as usize),
            _ => false,
        })
}

#[derive(Clone, Debug, PartialEq)]
struct SymbolicNode {
    draft: u32,
    span: crate::Span,
    target: Option<Target>,
}

#[derive(Clone, Debug, PartialEq)]
struct SignedRelation {
    /// Source namespace slot reserved for the owner before it was installed.
    source_slot: u32,
    /// Every executable lexical node has exactly one rigid inferred draft;
    /// `None` slots are independently re-lowered, non-executable lexical
    /// binder/name syntax. The parser's final `def` envelope is omitted.
    /// Concrete environments may use different specialization slots, so this
    /// source-order relation, not a concrete Candidate, carries the rank-1
    /// owner check across publication and signed-only compiler ingress.
    symbolic_nodes: alloc::vec::Vec<Option<SymbolicNode>>,
    /// Original self occurrences, including those inside quotations.
    lexical_nodes: alloc::vec::Vec<u32>,
}

#[derive(Clone, Debug)]
struct Named {
    name: alloc::string::String,
    identity: u64,
    owner: Option<u64>,
    tree: Tree,
    signature: Option<noble_kernel::words::Scheme>,
    signed: Option<SignedRelation>,
    /// Original alias spellings and exact source nodes bound to their
    /// independently resolved module owners at definition admission.
    alias_uses: alloc::vec::Vec<AliasUse>,
    /// Exact length-prefixed retained source offset assigned at successful
    /// outside-definition publication. Module definitions instead retain
    /// their own immutable whole-module source and have no history offset.
    history_start: Option<usize>,
}

impl PartialEq for Named {
    fn eq(&self, other: &Self) -> bool {
        let same_signature = match (&self.signature, &other.signature) {
            (Some(a), Some(b)) => {
                a.var_kinds == b.var_kinds
                    && a.stack_in == b.stack_in
                    && a.stack_out == b.stack_out
                    && a.effects == b.effects
            }
            (None, None) => true,
            _ => false,
        };
        self.name == other.name
            && self.identity == other.identity
            && self.owner == other.owner
            && self.tree == other.tree
            && same_signature
            && self.signed == other.signed
            && self.alias_uses == other.alias_uses
            && self.history_start == other.history_start
    }
}

#[derive(Clone, Debug, PartialEq)]
struct AliasUse {
    spelling: alloc::string::String,
    owner: u64,
    nodes: alloc::vec::Vec<(u32, Target)>,
}

/// An immutable preparation. A declaration deliberately has no executable root.
#[derive(Clone, Debug)]
pub struct Prepared {
    generation: u64,
    history: alloc::vec::Vec<u8>,
    hosts: bool,
    text_cursor: bool,
    live_selected: Option<alloc::string::String>,
    live_slots: bool,
    limits: crate::Limits,
    boundary: Option<alloc::vec::Vec<u8>>,
    definition: Option<Named>,
    selected_root: Option<u32>,
    addition: alloc::vec::Vec<u8>,
    submission: Option<noble_kernel::execution::Submission>,
    checked: Option<noble_kernel::untrusted::Checked>,
    output: alloc::vec::Vec<noble_kernel::types::Ty>,
}

impl Prepared {
    pub const fn submission(&self) -> Option<&noble_kernel::execution::Submission> {
        self.submission.as_ref()
    }
    /// The kernel-accepted derivation, including immutable typed live-site
    /// descriptors. A candidate or source ordinal alone is never authority.
    pub const fn checked(&self) -> Option<&noble_kernel::untrusted::Checked> {
        self.checked.as_ref()
    }
    pub fn live_sites(&self) -> Option<&[noble_kernel::untrusted::LiveSite]> {
        self.checked
            .as_ref()
            .map(|checked| checked.live_sites.as_slice())
    }
    pub fn definition_name(&self) -> Option<&str> {
        self.definition
            .as_ref()
            .map(|definition| definition.name.as_str())
    }
    pub const fn output(&self) -> &[noble_kernel::types::Ty] {
        self.output.as_slice()
    }
    pub const fn is_definition(&self) -> bool {
        self.definition.is_some()
    }
}

/// A pure namespace, independent of the runtime value stack.
/// `new` explicitly configures resource-free `test.emit` and `test.abort` test
/// bindings. `without_test_hosts` is the bootstrap-only resolution environment.
#[derive(Debug)]
pub struct Session {
    definitions: alloc::vec::Vec<Named>,
    history: alloc::vec::Vec<u8>,
    generation: u64,
    hosts: bool,
    text_cursor: bool,
    live_selected: Option<alloc::string::String>,
    live_slots: Option<LiveSlots>,
    bindings: Option<crate::component::Bindings>,
    declared: Option<declared::Context>,
}

#[derive(Debug)]
struct LiveSlots {
    environment: noble_kernel::contracts::Env,
    resources: alloc::vec::Vec<(alloc::string::String, noble_kernel::types::Ty)>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub const fn new() -> Self {
        Self {
            definitions: alloc::vec::Vec::new(),
            history: alloc::vec::Vec::new(),
            generation: 0,
            hosts: true,
            text_cursor: false,
            live_selected: None,
            live_slots: None,
            bindings: None,
            declared: None,
        }
    }
    pub const fn without_test_hosts() -> Self {
        Self {
            definitions: alloc::vec::Vec::new(),
            history: alloc::vec::Vec::new(),
            generation: 0,
            hosts: false,
            text_cursor: false,
            live_selected: None,
            live_slots: None,
            bindings: None,
            declared: None,
        }
    }
    /// Separately selected pure byte cursor; Core and declared sessions remain unchanged.
    pub fn new_text_cursor() -> Self {
        Self {
            text_cursor: true,
            ..Self::without_test_hosts()
        }
    }
    /// Only this selected named body's lexical source can request a live edit.
    /// Ordinary sessions never install either live operation.
    pub fn new_live(selected_name: &str) -> Self {
        Self {
            live_selected: Some(alloc::string::String::from(selected_name)),
            ..Self::without_test_hosts()
        }
    }
    /// A separate, host-selected profile. The environment must already have
    /// been enabled and its versioned resource schemas registered by the host.
    /// Core and guarded Live-Wasm-Draft environments cannot gain slot syntax.
    pub fn new_live_slots(environment: noble_kernel::contracts::Env) -> Result<Self, Error> {
        if !environment.live_slots || environment.text_cursor {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(
                    crate::Span { start: 0, end: 0 },
                    "live slots require a host-enabled independent source environment",
                ),
            ));
        }
        Ok(Self {
            live_slots: Some(LiveSlots {
                environment,
                resources: alloc::vec::Vec::new(),
            }),
            ..Self::without_test_hosts()
        })
    }

    /// Only a trusted host which independently granted the bootstrap test
    /// effects may select this profile. Source text cannot enable them, and
    /// ordinary live-slot sessions remain unable to resolve `test.emit`.
    pub fn new_live_slots_with_test_hosts(
        environment: noble_kernel::contracts::Env,
    ) -> Result<Self, Error> {
        let mut session = Self::new_live_slots(environment.enable_live_test_hosts())?;
        session.hosts = true;
        Ok(session)
    }

    /// Resolve a source spelling such as `Account@1` only to an already
    /// registered exact host nominal resource schema. This does not mint a
    /// resource or a borrowed live reference.
    pub fn register_live_resource(
        &mut self,
        name: &str,
        id: noble_kernel::types::NominalTypeId,
    ) -> Result<(), Error> {
        let span = crate::Span { start: 0, end: 0 };
        let Some((base, version)) = name.rsplit_once('@') else {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(span, "live resource name requires a version suffix"),
            ));
        };
        if base.is_empty()
            || !base
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            || !base.as_bytes()[0].is_ascii_alphabetic()
            || version.is_empty()
            || (version.len() > 1 && version.starts_with('0'))
            || !version.bytes().all(|byte| byte.is_ascii_digit())
            || version.parse::<u32>().is_err()
        {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(span, "invalid versioned live resource name"),
            ));
        }
        let Some(profile) = self.live_slots.as_mut() else {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(
                    span,
                    "live resource registration requires the live-slot profile",
                ),
            ));
        };
        let Some(declaration) = profile.environment.nominal(id) else {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(span, "unregistered nominal resource schema"),
            ));
        };
        let noble_kernel::types::NominalShape::Opaque(payload) = &declaration.shape else {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(span, "live resource schema must be opaque"),
            ));
        };
        let noble_kernel::types::Ty::Resource(kind) = &**payload else {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(span, "live resource schema must own a registered resource"),
            ));
        };
        if !profile.environment.live_resource_nominals.contains(&id)
            || !profile.environment.resource_kinds.contains(kind)
            || !declaration.exported
            || declaration.public != [false, false]
            || profile.resources.iter().any(|(existing, ty)| {
                existing == name
                    || matches!(ty, noble_kernel::types::Ty::Nominal(other, _) if *other == id)
            })
        {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(span, "duplicate or constructible live resource schema"),
            ));
        }
        let next = self.generation.checked_add(1).ok_or_else(|| {
            Error::at(
                Stage::Acceptance,
                exhausted(span, "session generation limit exceeded"),
            )
        })?;
        profile.resources.push((
            alloc::string::String::from(name),
            noble_kernel::types::Ty::Nominal(id, alloc::boxed::Box::new(declaration.shape.clone())),
        ));
        self.generation = next;
        Ok(())
    }

    /// Parse a bounded type expression against this profile's registered
    /// versioned names; unregistered versions never alias a same-layout schema.
    pub fn parse_live_type(&self, text: &str) -> Result<noble_kernel::types::Ty, Error> {
        let span = crate::Span { start: 0, end: 0 };
        let Some(profile) = &self.live_slots else {
            return Err(Error::at(
                Stage::Check,
                crate::invalid(span, "live reference types require the live-slot profile"),
            ));
        };
        declared::types::parse_with_families(
            text,
            &profile.resources,
            &[],
            Some(&profile.environment),
            span,
        )
        .map(|parsed| parsed.ty)
        .map_err(|error| Error::at(Stage::Check, error))
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Immutable checked source identity, not the namespace generation or a
    /// caller-supplied name/identity pair.
    pub fn selected_definition_identity(&self, name: &str) -> Option<u64> {
        if self.live_selected.as_deref() != Some(name) {
            return None;
        }
        self.definitions
            .iter()
            .rev()
            .find(|definition| definition.name == name)
            .map(|definition| definition.identity)
    }

    /// Only current named bodies count as live dependents. Old Programs may
    /// retain historical definitions without acquiring the guest edit grant.
    pub fn live_definition_has_dependents(&self, name: &str) -> bool {
        let Some(index) = self
            .definitions
            .iter()
            .rposition(|definition| definition.name == name)
        else {
            return true;
        };
        !self.current_dependents(index).is_empty()
    }

    fn current_dependents(&self, original: usize) -> alloc::vec::Vec<usize> {
        // Every resolved edge points to an earlier immutable definition.
        // Current dependent bodies can therefore be visited in declaration
        // order, even though their later rebuilds are appended in a batch.
        let mut affected = alloc::vec![false; self.definitions.len()];
        affected[original] = true;
        let mut dependents = alloc::vec::Vec::new();
        for (index, definition) in self.definitions.iter().enumerate() {
            if index == original
                || self
                    .definitions
                    .iter()
                    .skip(index + 1)
                    .any(|later| later.name == definition.name)
            {
                continue;
            }
            if definition.tree.nodes.iter().any(|node| match &node.kind {
                Kind::Call(Target::Named(target)) => {
                    affected.get(*target as usize).copied().unwrap_or(false)
                }
                _ => false,
            }) {
                affected[index] = true;
                dependents.push(index);
            }
        }
        dependents
    }

    /// Build one private successor containing a replacement and every current
    /// definition whose immutable resolved calls reach its previous identity.
    /// Historical definitions stay in place for already captured Programs.
    pub fn preview_core_rebuild(&self, prepared: &Prepared) -> Result<Self, Error> {
        let span = crate::Span { start: 0, end: 0 };
        if !prepared.is_definition()
            || self.bindings.is_some()
            || self.declared.is_some()
            || !self.is_current_namespace(prepared)
        {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(span, "live Core preview requires a checked definition"),
            ));
        }
        let Some(replacement) = prepared.definition.as_ref() else {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(span, "live Core preview requires a definition"),
            ));
        };
        let name = &replacement.name;
        let original = self
            .definitions
            .iter()
            .rposition(|definition| definition.name == *name);
        if let Some(index) = original {
            let compatible = replacement::compatible(
                self,
                &self.definitions[index],
                replacement,
                prepared.limits,
            )
            .map_err(|diagnostic| Error::at(Stage::Check, diagnostic))?;
            if !compatible {
                return Err(Error::at(
                    Stage::Acceptance,
                    crate::invalid(
                        span,
                        "live replacement changes the checked stack interface or increases effects",
                    ),
                ));
            }
            if reaches_index(
                &replacement.tree,
                &self.definitions,
                index,
                prepared.limits.work,
            )? {
                return Err(Error::at(
                    Stage::Acceptance,
                    crate::invalid(
                        span,
                        "live replacement transitively depends on its previous identity",
                    ),
                ));
            }
        }
        let dependents =
            original.map_or_else(alloc::vec::Vec::new, |index| self.current_dependents(index));
        let mut successor = Self {
            definitions: self.definitions.clone(),
            history: self.history.clone(),
            generation: self.generation,
            hosts: self.hosts,
            text_cursor: self.text_cursor,
            live_selected: self.live_selected.clone(),
            live_slots: None,
            bindings: None,
            declared: None,
        };
        successor.commit(prepared.clone())?;
        let mut replacements = alloc::vec![None; self.definitions.len()];
        if let Some(index) = original {
            replacements[index] = Some(self.definitions.len());
        }
        let replay_work =
            prepared.limits.work / u32::try_from(dependents.len().max(1)).unwrap_or(u32::MAX);
        if replay_work == 0 {
            return Err(Error::at(
                Stage::Acceptance,
                exhausted(span, "live dependent rebuild work limit exceeded"),
            ));
        }
        for index in dependents {
            let bytes = self.definition_source(index)?;
            let limits = crate::Limits {
                work: replay_work,
                ..prepared.limits
            };
            let dependent = successor.prepare(bytes, &[], limits)?;
            let Some(rebuilt) = dependent.definition.as_ref() else {
                return Err(Error::at(
                    Stage::Acceptance,
                    crate::invalid(span, "retained dependent source is not a definition"),
                ));
            };
            if rebuilt.name != self.definitions[index].name
                || successor.core_definition_requires_host_effects(&dependent)
            {
                return Err(Error::at(
                    Stage::Acceptance,
                    crate::invalid(
                        span,
                        "retained dependent changed name or requires unavailable host effects",
                    ),
                ));
            }
            if !replay_preserves_targets(
                &self.definitions[index].tree,
                &rebuilt.tree,
                &replacements,
            ) {
                return Err(Error::at(
                    Stage::Acceptance,
                    crate::invalid(
                        span,
                        "retained dependent changed an unapproved resolved call target",
                    ),
                ));
            }
            let compatible =
                replacement::compatible(&successor, &self.definitions[index], rebuilt, limits)
                    .map_err(|diagnostic| Error::at(Stage::Check, diagnostic))?;
            if !compatible {
                return Err(Error::at(Stage::Acceptance, crate::invalid(
                    span, "rebuilt dependent changes the checked stack interface or increases effects",
                )));
            }
            replacements[index] = Some(successor.definitions.len());
            successor.commit(dependent)?;
        }
        Ok(successor)
    }

    /// Never reconstruct a dependent from its already-resolved tree: the
    /// source offset was fixed when its original checked definition committed.
    fn definition_source(&self, index: usize) -> Result<&[u8], Error> {
        let span = crate::Span { start: 0, end: 0 };
        let offset = self
            .definitions
            .get(index)
            .and_then(|definition| definition.history_start)
            .ok_or_else(|| {
                Error::at(
                    Stage::Acceptance,
                    crate::invalid(span, "retained dependent has no outside source provenance"),
                )
            })?;
        let header_end = offset.checked_add(4).ok_or_else(|| {
            Error::at(
                Stage::Acceptance,
                crate::invalid(span, "retained source header overflows"),
            )
        })?;
        let header = self.history.get(offset..header_end).ok_or_else(|| {
            Error::at(
                Stage::Acceptance,
                crate::invalid(span, "retained source header is missing"),
            )
        })?;
        let length = usize::try_from(u32::from_le_bytes(<[u8; 4]>::try_from(header).map_err(
            |_| {
                Error::at(
                    Stage::Acceptance,
                    crate::invalid(span, "invalid retained source length"),
                )
            },
        )?))
        .map_err(|_| {
            Error::at(
                Stage::Acceptance,
                crate::invalid(span, "retained source length exceeds address space"),
            )
        })?;
        let end = header_end.checked_add(length).ok_or_else(|| {
            Error::at(
                Stage::Acceptance,
                crate::invalid(span, "retained source length overflows"),
            )
        })?;
        self.history.get(header_end..end).ok_or_else(|| {
            Error::at(
                Stage::Acceptance,
                crate::invalid(span, "retained dependent source is incomplete"),
            )
        })
    }

    /// A definition declaration has no executable root; checking only an
    /// empty post-definition submission would miss latent host operations in
    /// its body. Walk resolved calls (including immutable named dependencies)
    /// and the kernel's effect schemes before allowing it into a live
    /// namespace. Only that session's scoped live effects are permitted;
    /// unavailable schemes and ambient test hosts fail closed.
    pub fn core_definition_requires_host_effects(&self, prepared: &Prepared) -> bool {
        let Some(definition) = prepared.definition.as_ref() else {
            return true;
        };
        let environment = match self.environment() {
            Ok(environment) => environment,
            Err(_) => return true,
        };
        let mut seen = alloc::vec![false; self.definitions.len()];
        let mut pending = alloc::vec![&definition.tree];
        while let Some(tree) = pending.pop() {
            for node in &tree.nodes {
                match &node.kind {
                    Kind::Call(Target::Builtin(index)) => {
                        if environment.defs.get(*index as usize).is_none_or(|scheme| {
                            scheme.effects.iter().any(|effect| {
                                !matches!(effect,
                                    noble_kernel::shapes::EffectSlot::Effect(
                                        noble_kernel::types::EffId(3 | 4)
                                    ) | noble_kernel::shapes::EffectSlot::Var(_)
                                        if self.live_selected.is_some())
                            })
                        }) {
                            return true;
                        }
                    }
                    Kind::Call(Target::Named(index)) => {
                        let Some(prior) = self.definitions.get(*index as usize) else {
                            return true;
                        };
                        let Some(visited) = seen.get_mut(*index as usize) else {
                            return true;
                        };
                        if !*visited {
                            *visited = true;
                            pending.push(&prior.tree);
                        }
                    }
                    _ => {}
                }
            }
        }
        false
    }

    pub(crate) fn with_bindings(bindings: crate::component::Bindings) -> Self {
        Self {
            bindings: Some(bindings),
            ..Self::without_test_hosts()
        }
    }

    fn effect_universe(&self) -> u64 {
        if let Some(profile) = &self.live_slots {
            return profile
                .environment
                .effects
                .iter()
                .fold(0u64, |mask, id| mask | 1u64.checked_shl(id.0).unwrap_or(0));
        }
        if let Some(context) = &self.declared {
            // Kernel validation binds non-bootstrap IDs to exact host-selected adapters.
            return context.environment.effects.iter().fold(0u64, |mask, id|
                mask | 1u64.checked_shl(id.0).unwrap_or(0));
        }
        match &self.bindings {
            Some(bindings) => bindings.effects,
            None => {
                if self.live_selected.is_some() {
                    (1 << 3) | (1 << 4)
                } else if self.hosts {
                    3
                } else {
                    0
                }
            }
        }
    }

    fn program_type_effect_universe(&self) -> u64 {
        self.effect_universe() | self.bindings.as_ref()
            .map_or(0, |bindings| bindings.program_type_effects)
    }

    /// Consume a preparation as an explicit deterministic namespace transition.
    ///
    /// Generation, host configuration, and complete declaration history must
    /// match; these checks and the next-generation check precede every mutation,
    /// so any returned error leaves the session unchanged. A successful
    /// declaration appends its owned definition and length-prefixed source
    /// history together. Every successful commit advances generation once,
    /// including executable submissions, which leave definitions/history intact.
    /// Host configuration is unchanged; no submission or host code is executed.
    #[expect(
        tigerstyle::mutating_input_in_pure,
        reason = "Owner: noble-maintainers; commit is the explicit owned namespace publication transition, not preparation scratch: snapshot and generation checks precede mutation, a declaration appends its definition/history, and every success advances generation once without executing guest or host code."
    )]
    pub fn commit(&mut self, prepared: Prepared) -> Result<(), Error> {
        let span = crate::Span { start: 0, end: 0 };
        if !self.is_current_namespace(&prepared) {
            return Err(Error::at(
                Stage::Acceptance,
                crate::invalid(
                    span,
                    "stale preparation belongs to a different namespace snapshot",
                ),
            ));
        }
        let next = match self.generation.checked_add(1) {
            Some(next) => next,
            None => {
                return Err(Error::at(
                    Stage::Acceptance,
                    exhausted(span, "session generation limit exceeded"),
                ))
            }
        };
        if let Some(mut definition) = prepared.definition {
            if definition.owner.is_none() {
                definition.history_start = Some(self.history.len());
            }
            self.definitions.push(definition);
            self.history.extend_from_slice(&prepared.addition);
        }
        self.generation = next;
        Ok(())
    }

    #[expect(
        tigerstyle::missing_const_fn,
        reason = "Owner: noble-maintainers; namespace comparison uses non-const exact Vec content equality over retained history and optional boundary identities."
    )]
    fn is_current_namespace(&self, prepared: &Prepared) -> bool {
        if prepared.generation != self.generation
            || prepared.hosts != self.hosts
            || prepared.text_cursor != self.text_cursor
            || prepared.live_selected != self.live_selected
            || prepared.live_slots != self.live_slots.is_some()
            || prepared.history != self.history
        {
            return false;
        }
        match (&self.bindings, &prepared.boundary) {
            (Some(bindings), Some(boundary)) => bindings.key == *boundary,
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        }
    }
}

fn exhausted(span: crate::Span, message: &str) -> crate::Diagnostic {
    crate::Diagnostic::new(crate::DiagnosticKind::Exhausted, span, message)
}

/// The source test environment is explicit, resource-free, and fixed. The old
/// checker's Text--Unit host contract remains unchanged outside source.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; the fixed bootstrap environment and emit slot are checked fallibly before appending matching scheme, behavior, dependency, and effect entries."
)]
pub(crate) fn environment() -> Result<noble_kernel::contracts::Env, crate::Diagnostic> {
    let span = crate::Span { start: 0, end: 0 };
    let mut env = match noble_kernel::contracts::environment() {
        Ok(env) => env,
        Err(_) => return Err(crate::internal(span)),
    };
    let tail = noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0));
    match env.defs.get_mut(22) {
        Some(emit) => emit.stack_out = alloc::vec![tail.clone()],
        None => return Err(crate::internal(span)),
    }
    env.defs.push(noble_kernel::words::Scheme {
        var_kinds: alloc::vec![noble_kernel::words::VariableKind::Stack],
        stack_in: alloc::vec![tail.clone()],
        stack_out: alloc::vec![tail],
        effects: alloc::vec![noble_kernel::shapes::EffectSlot::Effect(
            noble_kernel::types::EffId(1)
        )],
    });
    env.kinds.push(noble_kernel::contracts::Behavior::Named);
    env.deps.push(alloc::vec::Vec::new());
    env.definition_owners.push(None);
    env.effects.push(noble_kernel::types::EffId(1));
    Ok(env)
}

fn live_environment() -> Result<noble_kernel::contracts::Env, crate::Diagnostic> {
    use noble_kernel::shapes::{EffectSlot, Pattern};
    use noble_kernel::types::EffId;
    use noble_kernel::words::{Scheme, Variable, VariableKind};
    let mut env = environment()?;
    let stack = Pattern::StackVar(Variable(0));
    let propose = Scheme {
        var_kinds: alloc::vec![VariableKind::Stack],
        stack_in: alloc::vec![
            stack.clone(),
            Pattern::I64,
            Pattern::program(
                alloc::vec![Pattern::I64],
                alloc::vec![Pattern::I64],
                alloc::vec::Vec::new()
            )
        ],
        stack_out: alloc::vec![stack.clone(), Pattern::Unit],
        effects: alloc::vec![EffectSlot::Effect(EffId(3))],
    };
    let generation = Scheme {
        var_kinds: alloc::vec![VariableKind::Stack],
        stack_in: alloc::vec![stack.clone()],
        stack_out: alloc::vec![stack, Pattern::I64],
        effects: alloc::vec![EffectSlot::Effect(EffId(4))],
    };
    for scheme in [propose, generation] {
        env.defs.push(scheme);
        env.kinds.push(noble_kernel::contracts::Behavior::Named);
        env.deps.push(alloc::vec::Vec::new());
        env.definition_owners.push(None);
    }
    env.effects.push(EffId(3));
    env.effects.push(EffId(4));
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::Session;

    fn live_limits() -> crate::Limits {
        crate::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        }
    }

    fn borrowed(
        input: alloc::vec::Vec<noble_kernel::types::Ty>,
        output: alloc::vec::Vec<noble_kernel::types::Ty>,
        effects: &[noble_kernel::types::EffId],
    ) -> noble_kernel::types::Ty {
        noble_kernel::types::Ty::LiveRef(
            alloc::boxed::Box::new(input),
            alloc::boxed::Box::new(output),
            noble_kernel::types::EffSet::from_ids(effects),
        )
    }

    #[test]
    fn live_slots_forward_ordered_borrows_and_reuse_the_same_input() {
        use noble_kernel::types::Ty;
        let session = Session::new_live_slots(
            noble_kernel::contracts::environment()
                .unwrap()
                .enable_live_slots(),
        )
        .unwrap();
        let c = borrowed(alloc::vec![Ty::I64], alloc::vec![Ty::I64], &[]);
        let b = borrowed(
            alloc::vec![Ty::I64, c.clone()],
            alloc::vec![Ty::I64],
            &[noble_kernel::contracts::LIVE_DISPATCH],
        );
        let a = borrowed(
            alloc::vec![Ty::I64, c.clone(), b.clone()],
            alloc::vec![Ty::I64],
            &[noble_kernel::contracts::LIVE_DISPATCH],
        );
        let root = session
            .prepare(
                b"slot.invoke",
                &[Ty::I64, c.clone(), b.clone(), a],
                live_limits(),
            )
            .unwrap();
        let root_submission = root.submission().unwrap();
        assert!(matches!(
            crate::wire::lower_subject(&root_submission.body.candidate),
            Err(crate::wire::ProjectionError::LiveSlot)
        ));
        assert_eq!(root_submission.request.expected.stack_in.len(), 4);
        assert_eq!(root_submission.request.expected.stack_out, [Ty::I64]);
        assert_eq!(root.live_sites().unwrap().len(), 1);
        let site = &root.live_sites().unwrap()[0];
        assert_eq!(site.ref_ordinal, 2);
        assert_eq!(site.root_logical_input_position, 3);
        assert_eq!(site.forwarded_ref_ordinals, [1, 2]);
        assert_eq!(site.stack_in, [Ty::I64, c.clone(), b.clone()]);
        assert_eq!(
            site.allowed_effects,
            noble_kernel::types::EffSet::from_ids(&[noble_kernel::contracts::LIVE_DISPATCH])
        );
        assert_eq!(
            root.checked().unwrap().interface.effects,
            noble_kernel::types::EffSet::from_ids(&[noble_kernel::contracts::LIVE_DISPATCH])
        );

        let reversed = session
            .prepare(
                b"slot.invoke.0",
                &[Ty::I64, b.clone(), c.clone()],
                live_limits(),
            )
            .unwrap();
        assert!(!site.admits_target(&reversed.checked().unwrap().interface));
        let nested = session
            .prepare(
                b"slot.invoke slot.invoke",
                &[Ty::I64, c.clone(), b],
                live_limits(),
            )
            .unwrap();
        assert!(site.admits_target(&nested.checked().unwrap().interface));
        let sites = nested.live_sites().unwrap();
        assert_eq!(sites.len(), 2);
        assert_ne!(sites[0].site_id, sites[1].site_id);
        assert_eq!(sites[0].ref_ordinal, 1);
        assert_eq!(sites[1].ref_ordinal, 1);
        assert_eq!(sites[0].root_logical_input_position, 2);
        assert_eq!(sites[1].root_logical_input_position, 2);
        assert_eq!(sites[0].forwarded_ref_ordinals, [1]);
        assert_eq!(sites[1].forwarded_ref_ordinals, [1]);
        let leaf = session
            .prepare(b"slot.invoke", &[Ty::I64, c], live_limits())
            .unwrap();
        assert!(sites[0].admits_target(&leaf.checked().unwrap().interface));
        assert_eq!(leaf.live_sites().unwrap()[0].forwarded_ref_ordinals, []);
    }

    #[test]
    fn live_slot_rejects_capture_forgery_ambiguity_and_legacy_profiles() {
        use noble_kernel::types::Ty;
        let session = Session::new_live_slots(
            noble_kernel::contracts::environment()
                .unwrap()
                .enable_live_slots(),
        )
        .unwrap();
        let reference = borrowed(alloc::vec![Ty::I64], alloc::vec![Ty::I64], &[]);
        let inputs = [Ty::I64, reference.clone()];
        for source in [b"[ slot.invoke ]".as_slice(), b"def f [ slot.invoke ]"] {
            assert!(session.prepare(source, &inputs, live_limits()).is_err());
        }
        assert!(session
            .prepare(
                b"slot.invoke",
                &[
                    Ty::I64,
                    Ty::Pair(
                        alloc::boxed::Box::new(Ty::I64),
                        alloc::boxed::Box::new(reference.clone()),
                    )
                ],
                live_limits(),
            )
            .is_err());
        assert!(session
            .prepare(
                b"slot.invoke",
                &[Ty::I64, reference.clone(), reference.clone()],
                live_limits(),
            )
            .is_err());
        let selected = session
            .prepare(
                b"slot.invoke.1",
                &[Ty::I64, reference.clone(), reference.clone()],
                live_limits(),
            )
            .unwrap();
        assert_eq!(selected.live_sites().unwrap()[0].ref_ordinal, 1);
        for core in [Session::new(), Session::new_live("f")] {
            assert!(core
                .prepare(b"slot.invoke", &[Ty::I64], live_limits())
                .is_err());
            assert!(core
                .prepare(b"1", &[reference.clone()], live_limits())
                .is_err());
        }
        let escaping = borrowed(alloc::vec![Ty::I64], alloc::vec![reference.clone()], &[]);
        assert!(session
            .prepare(b"slot.invoke", &[Ty::I64, escaping], live_limits())
            .is_err());
    }

    #[test]
    fn versioned_resource_names_resolve_only_registered_exact_schema() {
        use noble_kernel::types::{NominalShape, NominalTypeId, ResourceKind, Ty};
        let mut environment = noble_kernel::contracts::environment()
            .unwrap()
            .enable_live_slots();
        let make = |module, kind| noble_kernel::contracts::NominalDecl {
            id: NominalTypeId { module, ordinal: 0 },
            shape: NominalShape::Opaque(alloc::boxed::Box::new(Ty::Resource(ResourceKind(kind)))),
            exported: true,
            public: [false, false],
        };
        let one = make(110, 41);
        let two = make(111, 42);
        environment = environment.register_live_resource(one.clone()).unwrap();
        environment = environment.register_live_resource(two.clone()).unwrap();
        let mut session = Session::new_live_slots(environment).unwrap();
        session.register_live_resource("Account@1", one.id).unwrap();
        session.register_live_resource("Account@2", two.id).unwrap();
        let account_one = session.parse_live_type("Account@1").unwrap();
        let account_two = session.parse_live_type("Account@2").unwrap();
        assert_ne!(account_one, account_two);
        assert!(session.parse_live_type("Account@3").is_err());
        assert!(session.register_live_resource("Account@3", one.id).is_err());
        let reference = borrowed(
            alloc::vec![account_one.clone(), Ty::I64],
            alloc::vec![account_one.clone(), Ty::I64],
            &[noble_kernel::contracts::TEST_EMIT],
        );
        assert_eq!(
            session
                .parse_live_type("LiveRef<Account@1+I64,Account@1+I64,test.emit>")
                .unwrap(),
            reference,
        );
        let prepared = session
            .prepare(
                b"slot.invoke",
                &[account_one.clone(), Ty::I64, reference.clone()],
                live_limits(),
            )
            .unwrap();
        assert_eq!(prepared.output(), &[account_one.clone(), Ty::I64]);
        assert_eq!(
            prepared.checked().unwrap().interface.effects,
            noble_kernel::types::EffSet::from_ids(&[
                noble_kernel::contracts::TEST_EMIT,
                noble_kernel::contracts::LIVE_DISPATCH,
            ])
        );
        let exact_target = session
            .prepare(b"1 +", &[account_one.clone(), Ty::I64], live_limits())
            .unwrap();
        assert!(prepared.live_sites().unwrap()[0]
            .admits_target(&exact_target.checked().unwrap().interface));
        let wrong_schema = session
            .prepare(b"1 +", &[account_two.clone(), Ty::I64], live_limits())
            .unwrap();
        assert!(!prepared.live_sites().unwrap()[0]
            .admits_target(&wrong_schema.checked().unwrap().interface));
        let wrong_order = session
            .prepare(b"", &[Ty::I64, account_one], live_limits())
            .unwrap();
        assert!(!prepared.live_sites().unwrap()[0]
            .admits_target(&wrong_order.checked().unwrap().interface));
        assert!(session
            .prepare(
                b"slot.invoke",
                &[account_two, Ty::I64, reference],
                live_limits()
            )
            .is_err());
    }

    #[test]
    fn live_batch_rebinds_only_effective_transitive_dependents() {
        let limits = crate::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        };
        let mut session = Session::new();
        for source in [
            b"def addone [ 1 + ]".as_slice(),
            b"def twice [ addone addone ]",
            b"def four [ twice twice ]",
            b"def unrelated [ 10 + ]",
        ] {
            let prepared = session.prepare(source, &[], limits).unwrap();
            session.commit(prepared).unwrap();
        }
        let original = session
            .definitions
            .iter()
            .map(|definition| definition.identity)
            .collect::<alloc::vec::Vec<_>>();
        let replacement = session.prepare(b"def addone [ 2 + ]", &[], limits).unwrap();
        let preview = session.preview_core_rebuild(&replacement).unwrap();
        assert_eq!(session.definitions.len(), 4);
        assert_eq!(preview.definitions.len(), 7);
        assert_eq!(preview.definitions[3].identity, original[3]);
        assert_eq!(preview.definitions[4].name, "addone");
        assert_eq!(preview.definitions[5].name, "twice");
        assert_eq!(preview.definitions[6].name, "four");
        assert_ne!(preview.definitions[4].identity, original[0]);
        assert_ne!(preview.definitions[5].identity, original[1]);
        assert_ne!(preview.definitions[6].identity, original[2]);
        assert_eq!(
            session
                .definitions
                .iter()
                .map(|definition| definition.identity)
                .collect::<alloc::vec::Vec<_>>(),
            original
        );
    }

    #[test]
    fn replay_refuses_drift_to_a_shadowed_unchanged_target() {
        let limits = crate::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        };
        // Ordinary preparation can retain a historical target when another
        // name is shadowed. The live batch must not silently retarget it.
        for source in [
            b"def use [ f offset ]".as_slice(),
            b"def use [ [ f offset ] run ]",
        ] {
            let mut session = Session::new_live("f");
            for definition in [
                b"def f [ 1 + ]".as_slice(),
                b"def offset [ 10 + ]",
                source,
                b"def offset [ 100 + ]",
            ] {
                let prepared = session.prepare(definition, &[], limits).unwrap();
                session.commit(prepared).unwrap();
            }
            let replacement = session.prepare(b"def f [ 2 + ]", &[], limits).unwrap();
            let failure = session.preview_core_rebuild(&replacement).unwrap_err();
            assert!(
                failure
                    .diagnostic()
                    .message
                    .contains("unapproved resolved call target"),
                "{failure:?}",
            );
            assert_eq!(session.definitions.len(), 4);
        }
    }

    #[test]
    fn replacement_rejects_indirect_old_identity_even_in_quotation() {
        let limits = crate::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        };
        let mut session = Session::new();
        for source in [b"def f [ 1 + ]".as_slice(), b"def g [ f ]", b"def h [ g ]"] {
            let prepared = session.prepare(source, &[], limits).unwrap();
            session.commit(prepared).unwrap();
        }
        for source in [b"def f [ h ]".as_slice(), b"def f [ [ h ] run ]"] {
            let replacement = session.prepare(source, &[], limits).unwrap();
            let failure = session.preview_core_rebuild(&replacement).unwrap_err();
            assert!(
                failure.diagnostic().message.contains("previous identity"),
                "{failure:?}",
            );
            assert_eq!(session.definitions.len(), 3);
        }
    }
}
