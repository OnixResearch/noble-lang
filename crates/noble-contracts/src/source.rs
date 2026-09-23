//! Bounded Core-Bootstrap preparation. This module never executes guest code.
//!
//! Named bodies retain resolved dependency identities. Their type constraints
//! are instantiated afresh at each named use, while a first-class Program has
//! one shared inference term. Every executable specialization is subsequently
//! checked by the kernel against its exact concrete environment contract.

mod emission;
mod inference;
mod lexer;
mod parsing;
mod preflight;
mod preparation;
mod resolution;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Parse,
    Resolve,
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

#[derive(Clone, Copy, Debug)]
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

#[derive(Debug)]
struct Named {
    name: alloc::string::String,
    identity: u64,
    tree: Tree,
}

/// An immutable preparation. A declaration deliberately has no executable root.
#[derive(Debug)]
pub struct Prepared {
    generation: u64,
    history: alloc::vec::Vec<u8>,
    hosts: bool,
    definition: Option<Named>,
    addition: alloc::vec::Vec<u8>,
    submission: Option<noble_kernel::execution::Submission>,
    output: alloc::vec::Vec<noble_kernel::types::Ty>,
}

impl Prepared {
    pub const fn submission(&self) -> Option<&noble_kernel::execution::Submission> {
        self.submission.as_ref()
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
        }
    }
    pub const fn without_test_hosts() -> Self {
        Self {
            definitions: alloc::vec::Vec::new(),
            history: alloc::vec::Vec::new(),
            generation: 0,
            hosts: false,
        }
    }
    pub const fn generation(&self) -> u64 {
        self.generation
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
        if prepared.generation != self.generation
            || prepared.hosts != self.hosts
            || prepared.history != self.history
        {
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
fn environment() -> Result<noble_kernel::contracts::Env, crate::Diagnostic> {
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
    env.effects.push(noble_kernel::types::EffId(1));
    Ok(env)
}
