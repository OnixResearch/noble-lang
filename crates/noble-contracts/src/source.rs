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
            work = work.checked_sub(1).ok_or_else(|| Error::at(
                Stage::Acceptance,
                exhausted(span, "live replacement dependency walk exhausted"),
            ))?;
            if let Kind::Call(Target::Named(index)) = &node.kind {
                let index = *index as usize;
                if index == target {
                    return Ok(true);
                }
                let seen = visited.get_mut(index).ok_or_else(|| Error::at(
                    Stage::Acceptance,
                    crate::invalid(span, "invalid retained definition target"),
                ))?;
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
    original.nodes.iter().zip(&replayed.nodes).all(|(old, new)| {
        match (&old.kind, &new.kind) {
            (Kind::Literal(a), Kind::Literal(b)) => a == b,
            (Kind::Text(a), Kind::Text(b)) => a == b,
            (Kind::Quotation(a), Kind::Quotation(b)) => a == b,
            (Kind::Call(Target::Builtin(a)), Kind::Call(Target::Builtin(b))) => a == b,
            (Kind::Call(Target::Named(a)), Kind::Call(Target::Named(b))) => {
                replacements.get(*a as usize).is_some_and(|replacement| {
                    replacement.unwrap_or(*a as usize) == *b as usize
                })
            }
            _ => false,
        }
    })
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
    text_cursor: bool,
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
    text_cursor: bool,
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
            text_cursor: false,
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
            text_cursor: false,
            live_selected: None,
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

    /// Only current named bodies count as live dependents. Old Programs may
    /// retain historical definitions without acquiring the guest edit grant.
    pub fn live_definition_has_dependents(&self, name: &str) -> bool {
        let Some(index) = self.definitions.iter().rposition(|definition| definition.name == name)
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
                || self.definitions.iter().skip(index + 1).any(|later| later.name == definition.name)
            {
                continue;
            }
            if definition.tree.nodes.iter().any(|node| match &node.kind {
                Kind::Call(Target::Named(target)) => affected
                    .get(*target as usize).copied().unwrap_or(false),
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
        let original = self.definitions.iter().rposition(|definition| definition.name == *name);
        if let Some(index) = original {
            let compatible = replacement::compatible(
                self, &self.definitions[index], replacement, prepared.limits,
            ).map_err(|diagnostic| Error::at(Stage::Check, diagnostic))?;
            if !compatible {
                return Err(Error::at(Stage::Acceptance, crate::invalid(
                    span, "live replacement changes the checked stack interface or increases effects",
                )));
            }
            if reaches_index(
                &replacement.tree, &self.definitions, index, prepared.limits.work,
            )? {
                return Err(Error::at(Stage::Acceptance, crate::invalid(
                    span, "live replacement transitively depends on its previous identity",
                )));
            }
        }
        let dependents = original.map_or_else(alloc::vec::Vec::new, |index| {
            self.current_dependents(index)
        });
        let mut successor = Self {
            definitions: self.definitions.clone(),
            history: self.history.clone(),
            generation: self.generation,
            hosts: self.hosts,
            text_cursor: self.text_cursor,
            live_selected: self.live_selected.clone(),
            bindings: None,
            declared: None,
        };
        successor.commit(prepared.clone())?;
        let mut replacements = alloc::vec![None; self.definitions.len()];
        if let Some(index) = original {
            replacements[index] = Some(self.definitions.len());
        }
        let replay_work = prepared.limits.work
            / u32::try_from(dependents.len().max(1)).unwrap_or(u32::MAX);
        if replay_work == 0 {
            return Err(Error::at(Stage::Acceptance, exhausted(
                span, "live dependent rebuild work limit exceeded",
            )));
        }
        for index in dependents {
            let bytes = self.definition_source(index)?;
            let limits = crate::Limits { work: replay_work, ..prepared.limits };
            let dependent = successor.prepare(bytes, &[], limits)?;
            let Some(rebuilt) = dependent.definition.as_ref() else {
                return Err(Error::at(Stage::Acceptance, crate::invalid(
                    span, "retained dependent source is not a definition",
                )));
            };
            if rebuilt.name != self.definitions[index].name
                || successor.core_definition_requires_host_effects(&dependent)
            {
                return Err(Error::at(Stage::Acceptance, crate::invalid(
                    span, "retained dependent changed name or requires unavailable host effects",
                )));
            }
            if !replay_preserves_targets(
                &self.definitions[index].tree, &rebuilt.tree, &replacements,
            ) {
                return Err(Error::at(Stage::Acceptance, crate::invalid(
                    span, "retained dependent changed an unapproved resolved call target",
                )));
            }
            let compatible = replacement::compatible(
                &successor, &self.definitions[index], rebuilt, limits,
            ).map_err(|diagnostic| Error::at(Stage::Check, diagnostic))?;
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

    /// Definition history is length-prefixed in precisely the append order of
    /// `definitions`. Never reconstruct a dependent from its already-resolved
    /// tree: that would silently keep calls to the prior generation.
    fn definition_source(&self, index: usize) -> Result<&[u8], Error> {
        let span = crate::Span { start: 0, end: 0 };
        if index >= self.definitions.len() {
            return Err(Error::at(Stage::Acceptance, crate::invalid(
                span, "retained dependent has no source provenance",
            )));
        }
        let mut at = 0usize;
        for position in 0..=index {
            let header = self.history.get(at..at.saturating_add(4)).ok_or_else(|| {
                Error::at(Stage::Acceptance, crate::invalid(
                    span, "retained dependent has no source provenance",
                ))
            })?;
            let length = u32::from_le_bytes(
                <[u8; 4]>::try_from(header).map_err(|_| Error::at(
                    Stage::Acceptance, crate::invalid(span, "invalid retained source length"),
                ))?,
            ) as usize;
            at += 4;
            let bytes = self.history.get(at..at.saturating_add(length)).ok_or_else(|| {
                Error::at(Stage::Acceptance, crate::invalid(
                    span, "retained dependent source is incomplete",
                ))
            })?;
            if position == index {
                return Ok(bytes);
            }
            at += length;
        }
        Err(Error::at(Stage::Acceptance, crate::invalid(
            span, "retained dependent has no source provenance",
        )))
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
            || prepared.text_cursor != self.text_cursor
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

#[cfg(test)]
mod tests {
    use super::Session;

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
        let original = session.definitions.iter().map(|definition| definition.identity)
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
        assert_eq!(session.definitions.iter().map(|definition| definition.identity)
            .collect::<alloc::vec::Vec<_>>(), original);
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
                failure.diagnostic().message.contains("unapproved resolved call target"),
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
        for source in [
            b"def f [ 1 + ]".as_slice(),
            b"def g [ f ]",
            b"def h [ g ]",
        ] {
            let prepared = session.prepare(source, &[], limits).unwrap();
            session.commit(prepared).unwrap();
        }
        for source in [
            b"def f [ h ]".as_slice(),
            b"def f [ [ h ] run ]",
        ] {
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
