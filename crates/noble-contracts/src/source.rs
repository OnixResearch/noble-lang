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
mod parsing;
mod preflight;
mod preparation;
mod replacement;
mod resolution;

pub use declared::{BoundOperation, ModuleKind, ModulePrepared, ModuleSession};
pub(crate) use declared::{DefinitionOrigin, verify_definition_origin, verify_named_call_origin, verify_named_export};

/// Immutable source authority for the versioned Result interface. The same
/// declaration bytes are checked through ordinary module registration.
pub const RESULT_LIBRARY_SOURCE: &[u8] = include_bytes!("fixtures/result.noble");

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Builtin(u32),
    Named(u32),
}

#[derive(Clone, Debug)]
enum Kind {
    Literal(noble_kernel::untrusted::Lit),
    Text(alloc::vec::Vec<u8>),
    Word(alloc::vec::Vec<u8>),
    Call(Target),
    Quotation(alloc::vec::Vec<u32>),
    EditorHole,
}

#[derive(Clone, Debug)]
struct Node {
    kind: Kind,
    span: crate::Span,
}

#[derive(Clone, Debug)]
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

#[derive(Clone, Debug)]
struct Named {
    name: alloc::string::String,
    identity: u64,
    owner: Option<u64>,
    tree: Tree,
    signature: Option<noble_kernel::words::Scheme>,
}

/// An immutable preparation. A declaration deliberately has no executable root.
#[derive(Clone, Debug)]
pub struct Prepared {
    generation: u64,
    history: alloc::vec::Vec<u8>,
    hosts: bool,
    live_selected: Option<alloc::string::String>,
    limits: crate::Limits,
    boundary: Option<alloc::vec::Vec<u8>>,
    definition: Option<Named>,
    addition: alloc::vec::Vec<u8>,
    submission: Option<noble_kernel::execution::Submission>,
    output: alloc::vec::Vec<noble_kernel::types::Ty>,
}

impl Prepared {
    pub const fn submission(&self) -> Option<&noble_kernel::execution::Submission> {
        self.submission.as_ref()
    }
    pub fn definition_name(&self) -> Option<&str> {
        self.definition.as_ref().map(|definition| definition.name.as_str())
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
    live_selected: Option<alloc::string::String>,
    bindings: Option<crate::component::Bindings>,
    declared: Option<declared::Context>,
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
            live_selected: None,
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
            live_selected: None,
            bindings: None,
            declared: None,
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
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Immutable checked source identity, not the namespace generation or a
    /// caller-supplied name/identity pair.
    pub fn selected_definition_identity(&self, name: &str) -> Option<u64> {
        if self.live_selected.as_deref() != Some(name) {
            return None;
        }
        self.definitions.iter().rev().find(|definition| definition.name == name)
            .map(|definition| definition.identity)
    }

    /// Check a Core definition against this namespace and build its private
    /// successor without publishing the definition or changing this session.
    /// A live reload can compile and stage from the successor before swapping
    /// it into the selected namespace.
    pub fn preview_core_definition(&self, prepared: &Prepared) -> Result<Self, Error> {
        let span = crate::Span { start: 0, end: 0 };
        if !prepared.is_definition() || self.bindings.is_some() || self.declared.is_some()
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
        if let Some(previous) = self.definitions.iter().rev().find(|definition| definition.name == *name) {
            let compatible = replacement::compatible(self, previous, replacement, prepared.limits)
                .map_err(|diagnostic| Error::at(Stage::Check, diagnostic))?;
            if !compatible {
                return Err(Error::at(
                    Stage::Acceptance,
                    crate::invalid(span, "live replacement changes the checked stack interface or increases effects"),
                ));
            }
        }
        // Existing named bodies contain resolved immutable definition indexes.
        // Until the entire transitive dependent graph can be rebuilt, accepting
        // a replacement with even one affected body would give a fresh direct
        // lookup but a stale dependent after the success acknowledgement.
        for definition in self.definitions.iter().chain(core::iter::once(replacement)) {
            if definition.tree.nodes.iter().any(|node| match &node.kind {
                Kind::Call(Target::Named(index)) => self.definitions
                    .get(*index as usize)
                    .is_some_and(|referenced| referenced.name == *name),
                _ => false,
            }) {
                return Err(Error::at(
                    Stage::Acceptance,
                    crate::invalid(span, "live reload requires unsupported dependent rebuild"),
                ));
            }
        }
        let mut successor = Self {
            definitions: self.definitions.clone(),
            history: self.history.clone(),
            generation: self.generation,
            hosts: self.hosts,
            live_selected: self.live_selected.clone(),
            bindings: None,
            declared: None,
        };
        successor.commit(prepared.clone())?;
        Ok(successor)
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
                        if environment
                            .defs
                            .get(*index as usize)
                            .is_none_or(|scheme| scheme.effects.iter().any(|effect| {
                                !matches!(effect,
                                    noble_kernel::shapes::EffectSlot::Effect(
                                        noble_kernel::types::EffId(3 | 4)
                                    ) | noble_kernel::shapes::EffectSlot::Var(_)
                                        if self.live_selected.is_some())
                            }))
                        {
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
        if let Some(context) = &self.declared {
            return if context.environment.effects.contains(&noble_kernel::contracts::TEST_CLOCK) {
                1 | (1 << noble_kernel::contracts::TEST_CLOCK.0)
            } else {
                1
            };
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
        if let Some(definition) = prepared.definition {
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
            || prepared.live_selected != self.live_selected
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
        stack_in: alloc::vec![stack.clone(), Pattern::I64,
            Pattern::program(alloc::vec![Pattern::I64], alloc::vec![Pattern::I64],
                alloc::vec::Vec::new())],
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
