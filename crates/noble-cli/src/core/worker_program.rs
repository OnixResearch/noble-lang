//! Opt-in, source-bound Worker Program builders. The compiled named Program is
//! the only root: handles are indices into this invocation's private graph,
//! never package-supplied executable identities or host-operation grants.

use noble_kernel::types::{EffSet, Ty};
use noble_wasm::source::{NamedTargetMetadata, Prepared as Compiled};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) const SOURCE_LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 4_096,
    depth: 32,
    work: 3_000_000,
};
const GRAPH_LIMIT: usize = 4_096;
const WITNESS_LIMIT: usize = 4_096;
const RECIPE_DEPTH_LIMIT: u32 = 32;
const RECIPE_LEAF_LIMIT: u32 = 256;
pub(super) const PACKAGE_BYTES: usize = 262_144;
const PACKAGE_DEPTH: usize = 32;
/// format, semantic_revision, wit_world, source_utf8, root, graph,
/// selected_node, selected_graph_blake3, definitions, builtins, interface,
/// host_contracts, captures, capture_binding_blake3 and artifact.
const PACKAGE_FIELDS: usize = 15;
/// The pure builtin table is the checker's core vocabulary; each package row
/// binds one builtin spelling to this kernel semantic revision.
const BUILTIN_COUNT: u32 = 22;
pub(super) const RUNTIME_REQUEST_WORK: usize = 50_000_000;
const PACKAGE_REVISION: &str = "0.1.0-draft.11";
const WIT_WORLD: &str = "noble-test:worker-program/guest@2.0.0";
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Invalid,
    Unsupported,
    Exhausted,
}

/// Observations of one import attempt that remain meaningful when the import
/// is refused.
#[derive(Default)]
pub(crate) struct ImportTrace {
    /// Locally compiled source modules installed to reattest captured Text.
    pub(crate) installations: u32,
}

struct Node {
    recipe: Recipe,
    original: usize,
    depth: u32,
    leaves: u32,
    root_reachable: bool,
}

enum Recipe {
    Named { program_index: usize },
    Compose { first: usize, second: usize },
}

struct Witness {
    node: usize,
    input: Vec<Ty>,
    output: Vec<Ty>,
    effects: EffSet,
}

struct Owner {
    module_id: u64,
    version: u32,
    ordinal: u32,
    identity: u64,
    /// The definition is its own scoped recursive owner.
    recursive: bool,
}

pub(crate) struct CheckedTextAlias {
    pub(crate) owner_identity: u64,
    pub(crate) node: u32,
    pub(crate) bytes: Vec<u8>,
}

impl Owner {
    fn wire(&self) -> Value {
        json!({"module_id":self.module_id.to_string(),"version":self.version,
            "ordinal":self.ordinal,"identity":self.identity.to_string()})
    }
}

/// A host-selected, exact destination grant. Source/program/package bytes
/// cannot construct this value; code using it must authenticate the grant.
#[cfg(test)]
#[derive(Clone, Copy)]
struct ExportGrant<'a> {
    destination: &'a str,
    source_sha256: &'a str,
    capture_sha256: &'a str,
    allowed: bool,
}

/// An export denial is its own outcome: it never reads as a source-check
/// failure, and a refused package is never a successful no-op.
#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
enum ExportRefusal {
    Denied,
    Refused(Refusal),
}

/// A Program's graph index and its *adapted* interface witness are distinct.
/// The graph node retains its original, source-derived interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Handle {
    instance: u64,
    node: usize,
    witness: usize,
}

pub(crate) struct Store {
    instance: u64,
    host_policy: Option<super::worker_program_guest::WorkerHostPolicy>,
    compiled: Compiled,
    checked_interface: noble_contracts::source::CheckedProgramInterface,
    source: Vec<u8>,
    root_name: String,
    root_owner: Owner,
    root_index: usize,
    closure: Vec<Owner>,
    text_aliases: Vec<CheckedTextAlias>,
    attested_capture_hash: Option<[u8; 32]>,
    nodes: Vec<Node>,
    witnesses: Vec<Witness>,
    work_left: usize,
}

impl Store {
    /// Check the named root and independently emit the actual selected Wasm.
    /// No runtime builder below calls a source compiler or executes guest code.
    pub(crate) fn prepare(
        source: &[u8],
        name: &str,
        input: &[Ty],
        work: usize,
    ) -> Result<(Self, Handle), Refusal> {
        Self::prepare_selected(source, name, input, None, work)
    }

    /// Only a host embedder may opt in to a checked, bound operation. A typed
    /// source effect and caller-supplied package are never host policy.
    pub(crate) fn prepare_with_policy(
        source: &[u8],
        name: &str,
        input: &[Ty],
        policy: super::worker_program_guest::WorkerHostPolicy,
        work: usize,
    ) -> Result<(Self, Handle), Refusal> {
        Self::prepare_selected(source, name, input, Some(policy), work)
    }

    fn prepare_selected(
        source: &[u8],
        name: &str,
        input: &[Ty],
        policy: Option<super::worker_program_guest::WorkerHostPolicy>,
        work: usize,
    ) -> Result<(Self, Handle), Refusal> {
        if source.len() > SOURCE_LIMITS.bytes as usize
            || input.len() > 64
            || name.len() > 128
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(Refusal::Exhausted);
        }
        // Reserve one slice of the request meter for each bounded checking
        // stage: the module check, every closure walk (the closure alone for
        // a pure root; the closure, owner and interface walks for a
        // host-selected binding), the selected invocation check and Wasm
        // compilation. Each walk runs under at most one slice, so no nested
        // stage receives an uncharged fresh allowance.
        let walks: usize = if policy.is_some() { 3 } else { 1 };
        let slices = walks.checked_add(3).ok_or(Refusal::Exhausted)?;
        let share = slices
            .checked_add(1)
            .and_then(|divisor| work.checked_div(divisor))
            .ok_or(Refusal::Exhausted)?;
        let stage_work = u32::try_from(share.min(SOURCE_LIMITS.work as usize))
            .map_err(|_| Refusal::Exhausted)?;
        if stage_work == 0 {
            return Err(Refusal::Exhausted);
        }
        let limits = noble_contracts::Limits {
            work: stage_work,
            ..SOURCE_LIMITS
        };
        let reserved = usize::try_from(stage_work)
            .map_err(|_| Refusal::Exhausted)?
            .checked_mul(slices)
            .ok_or(Refusal::Exhausted)?;
        let mut work_left = work.checked_sub(reserved).ok_or(Refusal::Exhausted)?;
        let bindings = policy.as_ref().map_or(&[][..], |selection| {
            std::slice::from_ref(selection.binding())
        });
        let retained = noble_contracts::source::RetainedSourceLimit::new(limits.bytes)
            .ok_or(Refusal::Exhausted)?;
        let session = match &policy {
            // A host-bound Program keeps the unsigned grammar.
            Some(_) => noble_contracts::source::ModuleSession::new(bindings, retained),
            // A pure Program may contain signed, scoped self recursion.
            None => noble_contracts::source::ModuleSession::new_signed_self(bindings, retained),
        }
        .map_err(source_refusal)?;
        let module = session
            .prepare(source, &[], limits)
            .map_err(source_refusal)?;
        if module.kind() != noble_contracts::source::ModuleKind::Module
            || module.proof_obligations().is_some()
            || (policy.is_none() && !module.bindings().is_empty())
            || policy.as_ref().is_some_and(|selected| {
                module.bindings() != std::slice::from_ref(selected.binding())
            })
        {
            return Err(Refusal::Unsupported);
        }
        // A host-selected binding is checked at module level; the closure walk
        // runs first so its exhaustion stays the primary outcome. A pure root
        // is checked from its exact invocation below, where signed recursive
        // owners become verified.
        let bound = match &policy {
            Some(selected) => {
                let closure = module
                    .checked_program_closure_for_binding(name, selected.binding())
                    .map_err(closure_refusal)?;
                let owner = module
                    .checked_program_owner_for_binding(name, selected.binding())
                    .ok_or(Refusal::Invalid)?;
                Some((owner, closure))
            }
            None => None,
        };
        let (module_identity, module_name, version) =
            module.resolved_module().ok_or(Refusal::Invalid)?;
        let (session, committed) = session.commit(module);
        committed.map_err(source_refusal)?;
        if let (Some(selected), Some((owner, _))) = (&policy, &bound) {
            if !selected.matches_source(source, owner.definition_identity(), session.generation()) {
                return Err(Refusal::Invalid);
            }
        }
        let invocation = format!("{module_name}@{version}.{name}");
        let prepared = session
            .prepare(invocation.as_bytes(), input, limits)
            .map_err(source_refusal)?;
        let (owner, closure) = match bound {
            Some(checked) => checked,
            None => prepared
                .checked_invocation_closure(module_identity, name)
                .map_err(closure_refusal)?,
        };
        if closure.is_empty() {
            return Err(Refusal::Invalid);
        }
        // Retaining the source and matching each dependency below.
        work_left = work_left
            .checked_sub(source.len())
            .and_then(|left| left.checked_sub(closure.len().checked_mul(8)?))
            .ok_or(Refusal::Exhausted)?;
        let checked_interface = match &policy {
            Some(selected) => {
                prepared.checked_program_interface_for_binding(&owner, selected.binding())
            }
            None => prepared.checked_program_interface(&owner),
        }
        .ok_or(Refusal::Invalid)?;
        let submission = prepared.submission().ok_or(Refusal::Invalid)?;
        if submission.request.expected.stack_in != input
            || match &policy {
                Some(selected) => {
                    submission.request.expected.allowed_effects.as_slice()
                        != selected.binding().effects.as_slice()
                }
                None => !submission.request.expected.allowed_effects.is_empty(),
            }
            || submission
                .definitions
                .iter()
                .filter(|definition| {
                    definition.owner_slot == Some(owner.definition_slot())
                        && definition.source_identity == owner.definition_identity()
                        && submission
                            .environment
                            .definition_owners
                            .get(definition.definition.0 as usize)
                            == Some(&Some(owner.module_identity()))
                })
                .count()
                != 1
        {
            return Err(Refusal::Invalid);
        }
        // Signed recursion compiles only through the sealed rederivation.
        let compiler = noble_wasm::source::Compiler::new_worker();
        let compiled = match prepared.signed_source_owner() {
            Some(signed) => compiler.prepare_worker_signed(&signed),
            None => compiler.prepare(submission),
        }
        .map_err(|problem| match problem {
            noble_wasm::Diagnostic::Exhausted => Refusal::Exhausted,
            _ => Refusal::Invalid,
        })?;
        let mut nodes = Vec::with_capacity(closure.len());
        let mut witnesses = Vec::with_capacity(closure.len());
        let mut owners = Vec::with_capacity(closure.len());
        let mut text_aliases = Vec::new();
        let mut root = None;
        for dependency in &closure {
            let mut source_definitions = submission.definitions.iter().filter(|definition| {
                definition.owner_slot == Some(dependency.definition_slot())
                    && definition.source_identity == dependency.definition_identity()
                    && submission
                        .environment
                        .definition_owners
                        .get(definition.definition.0 as usize)
                        == Some(&Some(owner.module_identity()))
            });
            let (Some(source_definition), None) =
                (source_definitions.next(), source_definitions.next())
            else {
                return Err(Refusal::Invalid);
            };
            let mut named = compiled.named_targets().iter().filter(|target| {
                target.definition == source_definition.definition
                    && target.definition_identity == source_definition.identity
                    && submission
                        .environment
                        .definition_owners
                        .get(target.definition.0 as usize)
                        == Some(&Some(owner.module_identity()))
            });
            let (Some(target), None) = (named.next(), named.next()) else {
                return Err(Refusal::Invalid);
            };
            if target.stack_in.len() > 64
                || target.stack_out.len() > 64
                || !target
                    .effects
                    .is_subset_of(&submission.request.expected.allowed_effects)
                || target.checked_recipe.is_empty()
            {
                return Err(Refusal::Invalid);
            }
            let index = nodes.len();
            let metadata = compiled
                .program_metadata()
                .get(target.program_index)
                .ok_or(Refusal::Invalid)?;
            if metadata.recipe_depth > RECIPE_DEPTH_LIMIT
                || metadata.recipe_leaves > RECIPE_LEAF_LIMIT
            {
                return Err(Refusal::Exhausted);
            }
            let root_reachable = dependency.definition_slot() == owner.definition_slot();
            nodes.push(Node {
                recipe: Recipe::Named {
                    program_index: target.program_index,
                },
                original: index,
                depth: metadata.recipe_depth,
                leaves: metadata.recipe_leaves,
                root_reachable,
            });
            owners.push(Owner {
                module_id: dependency.module_identity(),
                version: dependency.module_version(),
                ordinal: dependency.definition_ordinal(),
                identity: dependency.definition_identity(),
                recursive: dependency.is_recursive(),
            });
            for text in &source_definition.body.texts {
                let node = usize::try_from(text.node.0).map_err(|_| Refusal::Exhausted)?;
                if text_aliases.len() >= GRAPH_LIMIT
                    || text.bytes.len() > SOURCE_LIMITS.bytes as usize
                    || !matches!(
                        source_definition.body.candidate.nodes.get(node),
                        Some(noble_kernel::untrusted::Node::Literal {
                            lit: noble_kernel::untrusted::Lit::Text,
                            ..
                        })
                    )
                {
                    return Err(Refusal::Invalid);
                }
                text_aliases.push(CheckedTextAlias {
                    owner_identity: dependency.definition_identity(),
                    node: text.node.0,
                    bytes: text.bytes.clone(),
                });
            }
            witnesses.push(Witness {
                node: index,
                input: target.stack_in.clone(),
                output: target.stack_out.clone(),
                effects: target.effects.clone(),
            });
            if root_reachable {
                if target.stack_in.as_slice() != input
                    || target.stack_out.as_slice() != prepared.output()
                {
                    return Err(Refusal::Invalid);
                }
                root = Some(index);
            }
        }
        let root = root.ok_or(Refusal::Invalid)?;
        let instance = NEXT_INSTANCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .map_err(|_| Refusal::Exhausted)?;
        let store = Self {
            instance,
            host_policy: policy,
            checked_interface,
            source: source.to_vec(),
            root_name: name.to_owned(),
            root_owner: Owner {
                module_id: owner.module_identity(),
                version: owner.module_version(),
                ordinal: owner.definition_ordinal(),
                identity: owner.definition_identity(),
                recursive: owner.is_recursive(),
            },
            root_index: root,
            closure: owners,
            text_aliases,
            attested_capture_hash: None,
            nodes,
            witnesses,
            compiled,
            work_left,
        };
        Ok((
            store,
            Handle {
                instance,
                node: root,
                witness: root,
            },
        ))
    }

    pub(crate) fn wat(&self) -> &[u8] {
        self.compiled.wat()
    }
    pub(crate) fn compiled(&self) -> &Compiled {
        &self.compiled
    }
    pub(crate) fn instance_id(&self) -> u64 {
        self.instance
    }
    pub(crate) fn root_owner_identity(&self) -> u64 {
        self.root_owner.identity
    }
    pub(crate) fn source(&self) -> &[u8] {
        &self.source
    }
    pub(crate) fn host_policy(&self) -> Option<&super::worker_program_guest::WorkerHostPolicy> {
        self.host_policy.as_ref()
    }
    pub(crate) fn text_aliases(&self) -> &[CheckedTextAlias] {
        &self.text_aliases
    }

    fn capture_wire(&self) -> Result<Value, Refusal> {
        let mut values = Vec::with_capacity(self.text_aliases.len());
        for alias in &self.text_aliases {
            let text = std::str::from_utf8(&alias.bytes).map_err(|_| Refusal::Invalid)?;
            values.push(json!({"owner_identity":alias.owner_identity.to_string(),
                "source_node":alias.node,"schema":"Text","text_utf8":text}));
        }
        Ok(Value::Array(values))
    }

    /// Digest of the exact checked captures, computed without serializing
    /// them, so a host can bind an export grant to one capture set.
    #[cfg(test)]
    fn capture_digest(&self) -> Result<String, Refusal> {
        let mut preimage = Vec::new();
        for alias in &self.text_aliases {
            let length = u64::try_from(alias.bytes.len()).map_err(|_| Refusal::Exhausted)?;
            preimage.extend_from_slice(&alias.owner_identity.to_le_bytes());
            preimage.extend_from_slice(&alias.node.to_le_bytes());
            preimage.extend_from_slice(&length.to_le_bytes());
            preimage.extend_from_slice(&alias.bytes);
        }
        Ok(crate::workflow::intrinsic::sha256(&preimage))
    }

    fn selected_graph_blake3(&self, selected: usize) -> Result<String, Refusal> {
        let mut digest = blake3::Hasher::new();
        digest.update(b"noble-worker-checked-graph-v2\0");
        digest.update(&self.source);
        digest.update(
            &u64::try_from(selected)
                .map_err(|_| Refusal::Exhausted)?
                .to_le_bytes(),
        );
        for (index, node) in self.nodes.iter().enumerate() {
            match &node.recipe {
                Recipe::Named { program_index } => {
                    digest.update(b"named\0");
                    digest.update(
                        &self
                            .closure
                            .get(index)
                            .ok_or(Refusal::Invalid)?
                            .identity
                            .to_le_bytes(),
                    );
                    digest.update(
                        &self
                            .compiled
                            .named_targets()
                            .iter()
                            .find(|named| named.program_index == *program_index)
                            .ok_or(Refusal::Invalid)?
                            .checked_recipe,
                    );
                }
                Recipe::Compose { first, second } => {
                    digest.update(b"compose\0");
                    digest.update(
                        &u64::try_from(*first)
                            .map_err(|_| Refusal::Exhausted)?
                            .to_le_bytes(),
                    );
                    digest.update(
                        &u64::try_from(*second)
                            .map_err(|_| Refusal::Exhausted)?
                            .to_le_bytes(),
                    );
                }
            }
            digest.update(&node.depth.to_le_bytes());
            digest.update(&node.leaves.to_le_bytes());
        }
        Ok(crate::workflow::encoding::hex(digest.finalize().as_bytes()))
    }

    /// This source-authenticated WIT type map is inert. Actual WIT resource
    /// creation still requires source-to-installed-cell correspondence and a
    /// host owner table in the selected component instance.
    pub(crate) fn selected_world(&self) -> Result<noble_contracts::component::World, Refusal> {
        let bytes = include_bytes!("../../../noble-wasm/wit/worker-program.wit");
        let world = noble_contracts::component::World::parse(bytes, "guest", SOURCE_LIMITS)
            .map_err(|_| Refusal::Unsupported)?;
        match &self.host_policy {
            Some(policy) if policy.binding().operation == "test.emit" => {
                world.select_worker_program_for_test_emit(&self.checked_interface)
            }
            Some(policy) if policy.binding().operation == "test.code-export" => {
                world.select_worker_program_for_test_code_export(&self.checked_interface)
            }
            Some(_) => return Err(Refusal::Unsupported),
            None => world.select_worker_program(&self.checked_interface),
        }
        .map_err(|_| Refusal::Invalid)
    }

    pub(crate) fn require_selected_root(&self, handle: Handle) -> Result<(), Refusal> {
        if handle.instance != self.instance {
            return Err(Refusal::Invalid);
        }
        let node = self.nodes.get(handle.node).ok_or(Refusal::Invalid)?;
        if self
            .witnesses
            .get(handle.witness)
            .is_none_or(|witness| witness.node != handle.node)
        {
            return Err(Refusal::Invalid);
        }
        if !node.root_reachable {
            return Err(Refusal::Unsupported);
        }
        Ok(())
    }

    #[cfg(test)]
    fn package(&self, handle: Handle) -> Result<Value, Refusal> {
        let capture_binding = match self.attested_capture_hash {
            Some(hash) => Some(crate::workflow::encoding::hex(&hash)),
            None if self.text_aliases.is_empty() => None,
            None => return Err(Refusal::Invalid),
        };
        self.package_document(handle, capture_binding)
    }

    /// Every field except the capture binding derives from this store's
    /// checked source, closure, selected policy and graph. Import compares a
    /// claimed binding here before any Wasm is installed to reattest it.
    fn package_document(
        &self,
        handle: Handle,
        capture_binding: Option<String>,
    ) -> Result<Value, Refusal> {
        self.require_selected_root(handle)?;
        let root = self.node_handle(self.root_index).ok_or(Refusal::Invalid)?;
        let target = self.named(root).ok_or(Refusal::Invalid)?;
        if self
            .nodes
            .get(handle.node)
            .is_none_or(|node| node.original != handle.witness)
        {
            return Err(Refusal::Unsupported);
        }
        // The host-selected binding, if any, is the only effect a package
        // can carry; it is described here, never issued by import.
        let (allowed, effect_names, host_contracts) = match &self.host_policy {
            Some(policy) => {
                let binding = policy.binding();
                (
                    EffSet::from_ids(&binding.effects),
                    vec![binding.operation.clone()],
                    vec![contract_wire(binding)?],
                )
            }
            None => (EffSet::empty(), Vec::new(), Vec::new()),
        };
        let witness = self.witnesses.get(handle.witness).ok_or(Refusal::Invalid)?;
        if witness.input != [Ty::I64] || witness.output != [Ty::I64] || witness.effects != allowed {
            return Err(Refusal::Unsupported);
        }
        if self.nodes.iter().any(|node| {
            self.witnesses
                .get(node.original)
                .is_none_or(|original| !original.effects.is_subset_of(&allowed))
        }) {
            return Err(Refusal::Unsupported);
        }
        let source = std::str::from_utf8(&self.source).map_err(|_| Refusal::Unsupported)?;
        let mut definitions = Vec::with_capacity(self.closure.len());
        for (index, owner) in self.closure.iter().enumerate() {
            let named = self
                .named(Handle {
                    instance: self.instance,
                    node: index,
                    witness: index,
                })
                .ok_or(Refusal::Invalid)?;
            // A recursive definition's scoped self owner is itself; import
            // regenerates this row, so a foreign self owner cannot match.
            let self_owner = if owner.recursive {
                owner.wire()
            } else {
                Value::Null
            };
            definitions.push(json!({"owner":owner.wire(),"self_owner":self_owner,
                "recipe_sha256":crate::workflow::intrinsic::sha256(&named.checked_recipe)}));
        }
        let mut graph = Vec::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            graph.push(match &node.recipe {
                Recipe::Named { program_index } => json!({
                    "kind":"named","program_index":program_index,
                    "owner":self.closure.get(index).ok_or(Refusal::Invalid)?.wire(),
                    "depth":node.depth,"leaves":node.leaves}),
                Recipe::Compose { first, second } => json!({
                    "kind":"compose","first":first,"second":second,
                    "depth":node.depth,"leaves":node.leaves}),
            });
        }
        let captures = self.capture_wire()?;
        let mut builtins = Vec::with_capacity(BUILTIN_COUNT as usize);
        for ordinal in 0..BUILTIN_COUNT {
            let name =
                noble_contracts::source::pure_builtin_spelling(ordinal).ok_or(Refusal::Invalid)?;
            builtins.push(json!({"ordinal":ordinal,"name":name,
                "semantic_revision":super::worker_package::REVISION}));
        }
        Ok(json!({
            "format":"noble-worker-program-package/v2",
            "semantic_revision":PACKAGE_REVISION,
            "wit_world":WIT_WORLD,
            "source_utf8":source,
            "root":{"name":self.root_name,"owner":self.root_owner.wire(),
                "recipe_sha256":crate::workflow::intrinsic::sha256(&target.checked_recipe)},
            "graph":graph,"selected_node":handle.node,
            "selected_graph_blake3":self.selected_graph_blake3(handle.node)?,
            "definitions":definitions,
            "builtins":builtins,
            "interface":{"input":["I64"],"output":["I64"],"effects":effect_names},
            "host_contracts":host_contracts,
            "captures":captures,
            "capture_binding_blake3":capture_binding,
            "artifact":null
        }))
    }

    /// Authorization precedes package construction and serialization: only a
    /// host-built grant for this exact destination, source and capture digest
    /// admits an export, and a denial is reported as a denial. There is no
    /// CLI route: file or argument bytes cannot construct a grant.
    #[cfg(test)]
    fn export(
        &mut self,
        handle: Handle,
        destination: &str,
        grant: &ExportGrant<'_>,
    ) -> Result<Vec<u8>, ExportRefusal> {
        let captures = self.capture_digest().map_err(ExportRefusal::Refused)?;
        if !grant.allowed
            || destination != grant.destination
            || grant.source_sha256 != crate::workflow::intrinsic::sha256(&self.source)
            || grant.capture_sha256 != captures
        {
            return Err(ExportRefusal::Denied);
        }
        self.export_authorized(handle)
            .map_err(ExportRefusal::Refused)
    }

    #[cfg(test)]
    fn export_authorized(&mut self, handle: Handle) -> Result<Vec<u8>, Refusal> {
        self.require_selected_root(handle)?;
        let capture_bytes = self
            .text_aliases
            .iter()
            .try_fold(0usize, |total, alias| total.checked_add(alias.bytes.len()))
            .ok_or(Refusal::Exhausted)?;
        let package_work = self
            .nodes
            .len()
            .checked_mul(16)
            .and_then(|count| count.checked_add(self.source.len()))
            .and_then(|count| count.checked_add(capture_bytes))
            .ok_or(Refusal::Exhausted)?;
        self.charge(package_work)?;
        if !self.text_aliases.is_empty() {
            self.reserve_attestation()?;
            self.attested_capture_hash =
                Some(super::worker_program_guest::attest(self, handle).map_err(runtime_refusal)?);
        }
        let package = self.package(handle)?;
        let encoded = serde_json::to_vec(&package).map_err(|_| Refusal::Invalid)?;
        if encoded.len() > PACKAGE_BYTES {
            return Err(Refusal::Exhausted);
        }
        Ok(encoded)
    }

    /// Import never accepts a package's digest, flags or artifact as proof.
    /// Every bounded field is compared with a package regenerated from a
    /// fresh local check under the host-selected policy, if any, before any
    /// Wasm is installed; only then is a claimed Text binding reattested.
    /// A host contract row matches only the same independently selected
    /// binding, and importing it issues nothing.
    pub(crate) fn import(
        bytes: &[u8],
        policy: Option<super::worker_program_guest::WorkerHostPolicy>,
        work: usize,
        trace: &mut ImportTrace,
    ) -> Result<(Self, Handle), Refusal> {
        if bytes.len() > PACKAGE_BYTES {
            return Err(Refusal::Exhausted);
        }
        let mut depth = 0usize;
        let mut quoted = false;
        let mut escaped = false;
        let mut tokens = 0usize;
        for byte in bytes {
            if quoted {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == b'"' {
                    quoted = false;
                }
                continue;
            }
            match byte {
                b'"' => {
                    quoted = true;
                    tokens = tokens.checked_add(1).ok_or(Refusal::Exhausted)?;
                }
                b'{' | b'[' => {
                    depth = depth.checked_add(1).ok_or(Refusal::Exhausted)?;
                    if depth > PACKAGE_DEPTH {
                        return Err(Refusal::Exhausted);
                    }
                    tokens = tokens.checked_add(1).ok_or(Refusal::Exhausted)?;
                }
                b'}' | b']' => {
                    depth = depth.checked_sub(1).ok_or(Refusal::Invalid)?;
                }
                b',' => {
                    tokens = tokens.checked_add(1).ok_or(Refusal::Exhausted)?;
                }
                _ => {}
            }
            if tokens > GRAPH_LIMIT * 8 {
                return Err(Refusal::Exhausted);
            }
        }
        if depth != 0 || quoted {
            return Err(Refusal::Invalid);
        }
        let memory = bytes
            .len()
            .checked_mul(3)
            .and_then(|base| tokens.checked_mul(128)?.checked_add(base))
            .ok_or(Refusal::Exhausted)?;
        if memory > 1_048_576 {
            return Err(Refusal::Exhausted);
        }
        let parser_work = bytes
            .len()
            .checked_mul(3)
            .and_then(|base| tokens.checked_mul(8)?.checked_add(base))
            .ok_or(Refusal::Exhausted)?;
        let remaining = work.checked_sub(parser_work).ok_or(Refusal::Exhausted)?;
        let document: Value = serde_json::from_slice(bytes).map_err(|_| Refusal::Invalid)?;
        if serde_json::to_vec(&document).map_err(|_| Refusal::Invalid)? != bytes {
            return Err(Refusal::Invalid);
        }
        if document["format"] != "noble-worker-program-package/v2"
            || document["semantic_revision"] != PACKAGE_REVISION
            || document["wit_world"] != WIT_WORLD
        {
            return Err(Refusal::Unsupported);
        }
        // This revision fixes the field set and carries no executable
        // artifact: an artifact, accepted flag or any other extra claim is a
        // malformed package, not an unsupported feature.
        if document["artifact"] != Value::Null
            || document
                .as_object()
                .is_none_or(|fields| fields.len() != PACKAGE_FIELDS)
        {
            return Err(Refusal::Invalid);
        }
        let source = document["source_utf8"].as_str().ok_or(Refusal::Invalid)?;
        let name = document["root"]["name"].as_str().ok_or(Refusal::Invalid)?;
        let (mut store, _root) =
            Self::prepare_selected(source.as_bytes(), name, &[Ty::I64], policy, remaining)?;
        let graph = document["graph"].as_array().ok_or(Refusal::Invalid)?;
        if graph.len() < store.recipe_nodes() {
            return Err(Refusal::Invalid);
        }
        if graph.len() > GRAPH_LIMIT {
            return Err(Refusal::Exhausted);
        }
        for row in graph.iter().skip(store.recipe_nodes()) {
            if row["kind"] != "compose" {
                return Err(Refusal::Invalid);
            }
            let first = row["first"]
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| store.node_handle(index))
                .ok_or(Refusal::Invalid)?;
            let second = row["second"]
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| store.node_handle(index))
                .ok_or(Refusal::Invalid)?;
            store.compose(first, second)?;
        }
        let selected = document["selected_node"]
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| store.node_handle(index))
            .ok_or(Refusal::Invalid)?;
        // Refuse another closure member before any captured-Text guest
        // attestation, not merely during the final package comparison.
        store.require_selected_root(selected)?;
        let comparison_work = store
            .recipe_nodes()
            .checked_mul(16)
            .and_then(|count| count.checked_add(source.len()))
            .and_then(|count| count.checked_add(bytes.len()))
            .ok_or(Refusal::Exhausted)?;
        store.charge(comparison_work)?;
        let claimed = match &document["capture_binding_blake3"] {
            Value::Null => None,
            Value::String(binding) => Some(binding.clone()),
            _ => return Err(Refusal::Invalid),
        };
        if claimed.is_some() == store.text_aliases.is_empty()
            || store.package_document(selected, claimed.clone())? != document
        {
            return Err(Refusal::Invalid);
        }
        if let Some(claimed) = claimed {
            store.reserve_attestation()?;
            trace.installations = trace
                .installations
                .checked_add(1)
                .ok_or(Refusal::Exhausted)?;
            let attested =
                super::worker_program_guest::attest(&store, selected).map_err(runtime_refusal)?;
            if crate::workflow::encoding::hex(&attested) != claimed {
                return Err(Refusal::Invalid);
            }
            store.attested_capture_hash = Some(attested);
        }
        Ok((store, selected))
    }

    /// A second candidate must be an independently checked reachable named
    /// Program in the same compiled source closure, not a claimed package row.
    pub(crate) fn checked_root(&self, index: usize) -> Option<Handle> {
        matches!(&self.nodes.get(index)?.recipe, Recipe::Named { .. }).then_some(Handle {
            instance: self.instance,
            node: index,
            witness: self.nodes[index].original,
        })
    }

    pub(crate) fn node_handle(&self, index: usize) -> Option<Handle> {
        Some(Handle {
            instance: self.instance,
            node: index,
            witness: self.nodes.get(index)?.original,
        })
    }

    pub(crate) fn node_index(&self, handle: Handle) -> Option<usize> {
        self.interface(handle).map(|_| handle.node)
    }

    pub(crate) fn named(&self, handle: Handle) -> Option<&NamedTargetMetadata> {
        if handle.instance != self.instance {
            return None;
        }
        if self.witnesses.get(handle.witness)?.node != handle.node {
            return None;
        }
        match &self.nodes.get(handle.node)?.recipe {
            Recipe::Named { program_index } => self
                .compiled
                .named_targets()
                .iter()
                .find(|target| target.program_index == *program_index),
            Recipe::Compose { .. } => None,
        }
    }

    pub(crate) fn source_owner_identity(&self, handle: Handle) -> Option<u64> {
        self.named(handle)?;
        Some(self.closure.get(handle.node)?.identity)
    }

    pub(crate) fn interface(&self, handle: Handle) -> Option<(&[Ty], &[Ty], &EffSet)> {
        if handle.instance != self.instance {
            return None;
        }
        self.nodes.get(handle.node)?;
        let witness = self.witnesses.get(handle.witness)?;
        if witness.node != handle.node {
            return None;
        }
        Some((&witness.input, &witness.output, &witness.effects))
    }

    pub(crate) fn original(&self, handle: Handle) -> Option<(&[Ty], &[Ty], &EffSet)> {
        if handle.instance != self.instance {
            return None;
        }
        let original = self.nodes.get(handle.node)?.original;
        let witness = self.witnesses.get(original)?;
        if witness.node != handle.node {
            return None;
        }
        Some((&witness.input, &witness.output, &witness.effects))
    }

    fn charge(&mut self, work: usize) -> Result<(), Refusal> {
        self.work_left = self.work_left.checked_sub(work).ok_or(Refusal::Exhausted)?;
        Ok(())
    }

    pub(crate) fn reserve_attestation(&mut self) -> Result<(), Refusal> {
        self.charge(
            usize::try_from(super::worker_program_guest::FUEL).map_err(|_| Refusal::Exhausted)?,
        )
    }

    pub(crate) fn reserve_guest(&mut self) -> Result<(), Refusal> {
        let work = usize::try_from(super::worker_program_guest::FUEL)
            .map_err(|_| Refusal::Exhausted)?
            .checked_mul(2)
            .and_then(|fuel| fuel.checked_add(SOURCE_LIMITS.work as usize))
            .ok_or(Refusal::Exhausted)?;
        self.charge(work)
    }

    /// Explicit type-level effect inclusion changes only a witness, never a
    /// recipe node or host authority. Executable guest admission still
    /// requires a separate effect issuer; only pure cells are admitted here.
    pub(crate) fn adapt(
        &mut self,
        handle: Handle,
        destination: &EffSet,
    ) -> Result<Handle, Refusal> {
        let current = self.witnesses.get(handle.witness).ok_or(Refusal::Invalid)?;
        if handle.instance != self.instance {
            return Err(Refusal::Invalid);
        }
        self.nodes.get(handle.node).ok_or(Refusal::Invalid)?;
        if current.node != handle.node {
            return Err(Refusal::Invalid);
        }
        if !current.effects.is_subset_of(destination)
            || destination
                .as_slice()
                .iter()
                .any(|effect| *effect != noble_kernel::contracts::TEST_EMIT)
        {
            return Err(Refusal::Invalid);
        }
        if self.witnesses.len() >= WITNESS_LIMIT {
            return Err(Refusal::Exhausted);
        }
        self.charge(1)?;
        let current = &self.witnesses[handle.witness];
        let witness = Witness {
            node: handle.node,
            input: current.input.clone(),
            output: current.output.clone(),
            effects: destination.clone(),
        };
        let position = self.witnesses.len();
        self.witnesses.push(witness);
        Ok(Handle {
            instance: self.instance,
            node: handle.node,
            witness: position,
        })
    }

    /// Compose already checked Programs in execution order, first then second.
    /// This is a bounded immutable recipe node, not invocation or authority.
    pub(crate) fn compose(&mut self, first: Handle, second: Handle) -> Result<Handle, Refusal> {
        if first.instance != self.instance || second.instance != self.instance {
            return Err(Refusal::Invalid);
        }
        let left = self.witnesses.get(first.witness).ok_or(Refusal::Invalid)?;
        let right = self.witnesses.get(second.witness).ok_or(Refusal::Invalid)?;
        self.nodes.get(first.node).ok_or(Refusal::Invalid)?;
        self.nodes.get(second.node).ok_or(Refusal::Invalid)?;
        if left.node != first.node || right.node != second.node {
            return Err(Refusal::Invalid);
        }
        if left.output != right.input {
            return Err(Refusal::Invalid);
        }
        let depth = self.nodes[first.node]
            .depth
            .max(self.nodes[second.node].depth)
            .checked_add(1)
            .ok_or(Refusal::Exhausted)?;
        let leaves = self.nodes[first.node]
            .leaves
            .checked_add(self.nodes[second.node].leaves)
            .ok_or(Refusal::Exhausted)?;
        if depth > RECIPE_DEPTH_LIMIT || leaves > RECIPE_LEAF_LIMIT {
            return Err(Refusal::Exhausted);
        }
        if self.nodes.len() >= GRAPH_LIMIT || self.witnesses.len() >= WITNESS_LIMIT {
            return Err(Refusal::Exhausted);
        }
        self.charge(1)?;
        let left = &self.witnesses[first.witness];
        let right = &self.witnesses[second.witness];
        let root_reachable =
            self.nodes[first.node].root_reachable || self.nodes[second.node].root_reachable;
        let node = self.nodes.len();
        let witness = Witness {
            node,
            input: left.input.clone(),
            output: right.output.clone(),
            effects: left.effects.union(&right.effects),
        };
        let position = self.witnesses.len();
        self.witnesses.push(witness);
        self.nodes.push(Node {
            recipe: Recipe::Compose {
                first: first.node,
                second: second.node,
            },
            original: position,
            depth,
            leaves,
            root_reachable,
        });
        Ok(Handle {
            instance: self.instance,
            node,
            witness: position,
        })
    }

    /// Exact common witness, never a dynamic Any collection.
    pub(crate) fn select(&mut self, handles: &[Handle], index: usize) -> Result<Handle, Refusal> {
        if handles.is_empty() || handles.len() > GRAPH_LIMIT || index >= handles.len() {
            return Err(Refusal::Invalid);
        }
        let baseline = self
            .witnesses
            .get(handles[0].witness)
            .ok_or(Refusal::Invalid)?;
        if baseline.node != handles[0].node {
            return Err(Refusal::Invalid);
        }
        let (input, output, effects) = (&baseline.input, &baseline.output, &baseline.effects);
        for handle in handles {
            if handle.instance != self.instance {
                return Err(Refusal::Invalid);
            }
            self.nodes.get(handle.node).ok_or(Refusal::Invalid)?;
            let witness = self.witnesses.get(handle.witness).ok_or(Refusal::Invalid)?;
            if witness.node != handle.node {
                return Err(Refusal::Invalid);
            }
            if &witness.input != input || &witness.output != output || &witness.effects != effects {
                return Err(Refusal::Invalid);
            }
        }
        self.charge(handles.len())?;
        Ok(handles[index])
    }

    pub(crate) fn recipe_nodes(&self) -> usize {
        self.nodes.len()
    }

    pub(crate) fn recipe_edges(&self, handle: Handle) -> Option<(usize, usize)> {
        if handle.instance != self.instance {
            return None;
        }
        match &self.nodes.get(handle.node)?.recipe {
            Recipe::Compose { first, second } => Some((*first, *second)),
            Recipe::Named { .. } => None,
        }
    }
}

fn refusal(problem: Refusal) -> (&'static str, &'static str) {
    match problem {
        Refusal::Invalid => (
            "invalid-input",
            "worker Program or package differs from checked source",
        ),
        Refusal::Unsupported => ("unsupported", "worker Program package shape is unsupported"),
        Refusal::Exhausted => ("exhausted", "worker Program request budget exhausted"),
    }
}

fn source_refusal(problem: noble_contracts::source::Error) -> Refusal {
    if problem.diagnostic().kind == noble_contracts::DiagnosticKind::Exhausted {
        Refusal::Exhausted
    } else {
        Refusal::Invalid
    }
}

fn closure_refusal(problem: noble_contracts::source::ProgramClosureError) -> Refusal {
    match problem {
        noble_contracts::source::ProgramClosureError::Exhausted => Refusal::Exhausted,
        noble_contracts::source::ProgramClosureError::Unsupported => Refusal::Unsupported,
        noble_contracts::source::ProgramClosureError::Invalid => Refusal::Invalid,
    }
}

fn type_names(types: &[Ty]) -> Result<Vec<&'static str>, Refusal> {
    types
        .iter()
        .map(|ty| match ty {
            Ty::I64 => Ok("I64"),
            Ty::Text => Ok("Text"),
            _ => Err(Refusal::Unsupported),
        })
        .collect()
}

/// The exact host-selected bound adapter a packaged Program was checked
/// against. A row describes the contract; it never issues the effect.
fn contract_wire(binding: &noble_contracts::source::BoundOperation) -> Result<Value, Refusal> {
    Ok(
        json!({"module_name":binding.module_name,"module_version":binding.module_version,
        "operation":binding.operation,"adapter_identity":binding.adapter_identity,
        "adapter_slot":binding.adapter_slot,"input":type_names(&binding.input)?,
        "output":type_names(&binding.output)?,"effects":[binding.operation]}),
    )
}

fn runtime_refusal(problem: anyhow::Error) -> Refusal {
    if problem.downcast_ref::<wasmtime::Trap>() == Some(&wasmtime::Trap::OutOfFuel) {
        Refusal::Exhausted
    } else {
        Refusal::Invalid
    }
}

fn import(arguments: &[std::ffi::OsString]) -> Result<Value, (&'static str, &'static str)> {
    if arguments.len() != 6 || arguments[4] != "--effect-ceiling" {
        return Err((
            "invalid-input",
            "usage: noble worker program import PACKAGE --effect-ceiling pure",
        ));
    }
    if arguments[5] != "pure" {
        return Err((
            "unsupported",
            "Worker host effect policy unavailable to CLI",
        ));
    }
    let package = super::worker_typed::read_bounded_regular_file(
        std::path::Path::new(&arguments[3]),
        PACKAGE_BYTES,
    )
    .map_err(|_| ("invalid-input", "package file admission failed"))?;
    let mut trace = ImportTrace::default();
    let (store, root) = match Store::import(&package, None, RUNTIME_REQUEST_WORK, &mut trace) {
        Ok(imported) => imported,
        // A refusal still reports how many source modules were installed
        // before it, so a hostile package cannot hide a partial admission.
        Err(problem) => {
            let (outcome, diagnostic) = refusal(problem);
            return Ok(
                json!({"schema":"noble-worker-program-import/v2","stage":"import-admission",
                "outcome":outcome,"diagnostic":diagnostic,
                "source_installations":trace.installations,"guest_requests":0,
                "protected_operations":0,"executable_guest_installed":false}),
            );
        }
    };
    Ok(
        json!({"schema":"noble-worker-program-import/v2","stage":"import-admission",
        "outcome":"ready","source_sha256":crate::workflow::intrinsic::sha256(&store.source),
        "selected_graph_blake3":store.selected_graph_blake3(root.node).map_err(refusal)?,
        "dependencies":store.closure.len(),"guest_requests":0,
        "protected_operations":0,"executable_guest_installed":false,
        "source_installations":trace.installations}),
    )
}

fn guest(arguments: &[std::ffi::OsString]) -> Result<Value, (&'static str, &'static str)> {
    if arguments.len() != 10
        || arguments[4] != "--name"
        || arguments[6] != "--operand"
        || arguments[8] != "--effect-ceiling"
        || arguments[9] != "pure"
    {
        return Err(("invalid-input",
            "usage: noble worker program guest SOURCE --name NAME --operand I64 --effect-ceiling pure"));
    }
    let name = arguments[5]
        .to_str()
        .ok_or(("invalid-input", "invalid named Program"))?;
    let operand = arguments[7]
        .to_str()
        .ok_or(("invalid-input", "invalid signed operand"))?
        .parse::<i64>()
        .map_err(|_| ("invalid-input", "invalid signed operand"))?;
    let source = super::worker_typed::read_bounded_regular_file(
        std::path::Path::new(&arguments[3]),
        SOURCE_LIMITS.bytes as usize,
    )
    .map_err(|_| ("invalid-input", "source file admission failed"))?;
    let (mut store, root) =
        Store::prepare(&source, name, &[Ty::I64], RUNTIME_REQUEST_WORK).map_err(refusal)?;
    guest_report(&mut store, root, operand, "source")
}

fn guest_package(arguments: &[std::ffi::OsString]) -> Result<Value, (&'static str, &'static str)> {
    if arguments.len() != 8
        || arguments[4] != "--operand"
        || arguments[6] != "--effect-ceiling"
        || arguments[7] != "pure"
    {
        return Err((
            "invalid-input",
            "usage: noble worker program guest-package PACKAGE --operand I64 --effect-ceiling pure",
        ));
    }
    let operand = arguments[5]
        .to_str()
        .ok_or(("invalid-input", "invalid signed operand"))?
        .parse::<i64>()
        .map_err(|_| ("invalid-input", "invalid signed operand"))?;
    let package = super::worker_typed::read_bounded_regular_file(
        std::path::Path::new(&arguments[3]),
        PACKAGE_BYTES,
    )
    .map_err(|_| ("invalid-input", "package file admission failed"))?;
    let mut trace = ImportTrace::default();
    let (mut store, root) =
        Store::import(&package, None, RUNTIME_REQUEST_WORK, &mut trace).map_err(refusal)?;
    guest_report(&mut store, root, operand, "fresh-package")
}

fn guest_report(
    store: &mut Store,
    root: Handle,
    operand: i64,
    origin: &'static str,
) -> Result<Value, (&'static str, &'static str)> {
    let observed =
        super::worker_program_guest::execute(store, root, operand, 1).map_err(|failure| {
            match failure {
                super::worker_program_guest::GuestFailure::Exhausted => refusal(Refusal::Exhausted),
                super::worker_program_guest::GuestFailure::Invalid(problem) => {
                    match runtime_refusal(problem) {
                        Refusal::Exhausted => refusal(Refusal::Exhausted),
                        _ => (
                            "invalid-input",
                            "source-bound Worker guest installation or execution refused",
                        ),
                    }
                }
            }
        })?;
    let guest_requests = observed
        .admits
        .checked_add(observed.invokes)
        .and_then(|count| count.checked_add(observed.effect_requests))
        .and_then(|count| count.checked_add(observed.code_export_requests))
        .ok_or(("internal-failure", "guest request counter overflow"))?;
    Ok(json!({"schema":"noble-worker-program-guest/v2",
        "stage":"guest-execution", "outcome":"normal",
        "origin":origin,"source_sha256":crate::workflow::intrinsic::sha256(store.source()),
        "operand":operand,"value":observed.value,"guest_requests":guest_requests,
        "program_admissions":observed.admits,"program_invocations":observed.invokes,
        "effect_requests":observed.effect_requests,
        "code_export_requests":observed.code_export_requests,
        "code_export_denials":observed.code_export_denials,
        "protected_operations":observed.protected_operations,
        "live_owners":observed.live_owners,
        "executable_guest_installed":true}))
}

/// Source-checked builders/packages remain inert; only the explicitly selected
/// pure guest path installs a source-bound Program cell and links WIT ownership.
pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    if arguments.get(2).is_some_and(|value| value == "test-host") {
        return super::worker_program_guest::run_test_host(arguments);
    }
    let result = (|| {
        match arguments.get(2).and_then(|value| value.to_str()) {
            Some("import") => return import(arguments),
            Some("guest") => return guest(arguments),
            Some("guest-package") => return guest_package(arguments),
            _ => {}
        }
        if arguments.len() != 8
            || arguments[1] != "program"
            || arguments[2] != "build"
            || arguments[4] != "--name"
            || arguments[6] != "--effect-ceiling"
        {
            return Err((
                "invalid-input",
                "usage: noble worker program build SOURCE --name NAME --effect-ceiling pure",
            ));
        }
        let name = arguments[5]
            .to_str()
            .ok_or(("invalid-input", "invalid named Program"))?;
        if arguments[7] != "pure" {
            return Err((
                "unsupported",
                "Worker host effect policy unavailable to CLI",
            ));
        }
        let effects = EffSet::empty();
        let source = super::worker_typed::read_bounded_regular_file(
            std::path::Path::new(&arguments[3]),
            SOURCE_LIMITS.bytes as usize,
        )
        .map_err(|problem| (problem.context.outcome, "source file admission failed"))?;
        let (mut store, root) =
            Store::prepare(&source, name, &[Ty::I64], 1_000_000).map_err(|problem| {
                (
                    match problem {
                        Refusal::Invalid => "invalid-input",
                        Refusal::Unsupported => "unsupported",
                        Refusal::Exhausted => "exhausted",
                    },
                    "source-checked named Program rejected",
                )
            })?;
        let source_sha256 = crate::workflow::intrinsic::sha256(&source);
        let wat_sha256 = crate::workflow::intrinsic::sha256(store.wat());
        let world = store.selected_world().map_err(refusal)?;
        let other = (0..store.recipe_nodes())
            .filter_map(|index| store.checked_root(index))
            .find(|candidate| {
                *candidate != root && {
                    match (store.interface(root), store.interface(*candidate)) {
                        (
                            Some((first_input, first_output, _)),
                            Some((other_input, other_output, _)),
                        ) => first_input == other_input && first_output == other_output,
                        _ => false,
                    }
                }
            })
            .ok_or((
                "unsupported",
                "source has no second reachable compatible checked Program",
            ))?;
        let adapted = store
            .adapt(root, &effects)
            .map_err(|_| ("invalid-input", "effect inclusion rejected"))?;
        let second = store
            .adapt(other, &effects)
            .map_err(|_| ("invalid-input", "second effect inclusion rejected"))?;
        let selected = store
            .select(&[adapted, second], 1)
            .map_err(|_| ("invalid-input", "typed runtime selection rejected"))?;
        let composed = store
            .compose(adapted, selected)
            .map_err(|_| ("exhausted", "bounded composition rejected"))?;
        let original = store
            .original(root)
            .ok_or(("internal-failure", "original witness lost"))?;
        let current = store
            .interface(composed)
            .ok_or(("internal-failure", "adapted witness lost"))?;
        Ok(json!({
            "schema":"noble-worker-program-builder/v2", "stage":"runtime-builders",
            "outcome":"normal", "builder_result":"checked-common-interface",
            "wit_world":world.identity(),
            "source_sha256":source_sha256,
            "wat_sha256":wat_sha256, "source_compiler_calls_after_admission":0,
            "guest_requests":0, "protected_operations":0, "recipe_nodes":store.recipe_nodes(),
            "host_contract":null,
            "checked_programs":[store.named(root).map(|target| target.program_index),
                store.named(other).map(|target| target.program_index)],
            "original_effects":original.2.as_slice().iter().map(|effect| effect.0).collect::<Vec<_>>(),
            "adapted_effects":current.2.as_slice().iter().map(|effect| effect.0).collect::<Vec<_>>(),
            "executable_guest_installed":false,
        }))
    })();
    let (outcome, value) = match result {
        Ok(value) => (
            value["outcome"]
                .as_str()
                .unwrap_or("internal-failure")
                .to_owned(),
            value,
        ),
        Err((outcome, diagnostic)) => (
            outcome.to_owned(),
            json!({
                "schema":"noble-worker-program-builder/v2",
                "stage":"admission",
                "outcome":outcome, "diagnostic":diagnostic, "guest_requests":0,
                "protected_operations":0, "executable_guest_installed":false,
            }),
        ),
    };
    let report = super::output::Report {
        outcome,
        json: value.to_string(),
    };
    if let Err(error) = super::output::print(&report) {
        eprintln!("noble: {}", error.message);
        return std::process::ExitCode::from(error.exit());
    }
    std::process::ExitCode::from(report.exit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use noble_kernel::contracts::TEST_EMIT;

    #[test]
    fn checked_wit_token_requires_exact_named_program_root() {
        let retained = noble_contracts::source::RetainedSourceLimit::new(SOURCE_LIMITS.bytes)
            .expect("selected Worker source limit");
        let session = noble_contracts::source::ModuleSession::new(&[], retained).unwrap();
        let module = session
            .prepare(
                b"module Worker@1 [ def plus [ 1 + ] export plus ]",
                &[],
                SOURCE_LIMITS,
            )
            .unwrap();
        let owner = module.checked_program_owner("plus").unwrap();
        let (session, committed) = session.commit(module);
        committed.unwrap();
        let exact = session
            .prepare(b"Worker@1.plus", &[Ty::I64], SOURCE_LIMITS)
            .unwrap();
        assert!(exact.checked_program_interface(&owner).is_some());
        let extended = session
            .prepare(b"Worker@1.plus 1 drop", &[Ty::I64], SOURCE_LIMITS)
            .unwrap();
        assert!(
            extended.checked_program_interface(&owner).is_none(),
            "a source expression calling a named Program is not that Program"
        );
    }

    #[test]
    fn text_capture_is_runtime_bound_before_portable_export_or_import() {
        let source = b"module Worker@1 [ def plus [ \"tag\" drop 1 + ] export plus ]";
        let (mut store, root) = Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK)
            .expect("checked Text source");
        let source_sha256 = crate::workflow::intrinsic::sha256(source);
        let capture_sha256 = store.capture_digest().unwrap();
        let denied = ExportGrant {
            destination: "private",
            source_sha256: &source_sha256,
            capture_sha256: &crate::workflow::intrinsic::sha256(b"[]"),
            allowed: true,
        };
        assert_eq!(
            store.export(root, "private", &denied),
            Err(ExportRefusal::Denied)
        );
        let grant = ExportGrant {
            destination: "private",
            source_sha256: &source_sha256,
            capture_sha256: &capture_sha256,
            allowed: true,
        };
        let encoded = store
            .export(root, "private", &grant)
            .expect("authorized runtime Text rebind");
        let mut document: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(document["captures"][0]["text_utf8"], "tag");
        assert!(document["capture_binding_blake3"].as_str().is_some());
        let (mut imported, selected) = Store::import(
            &encoded,
            None,
            RUNTIME_REQUEST_WORK,
            &mut ImportTrace::default(),
        )
        .expect("freshly attested Text package");
        assert_eq!(
            crate::core::worker_program_guest::execute(&mut imported, selected, 41, 1)
                .unwrap()
                .value,
            42,
            "imported Text must rebind before guest invocation"
        );
        document["captures"][0]["text_utf8"] = json!("forged");
        let forged = serde_json::to_vec(&document).unwrap();
        assert!(matches!(
            Store::import(
                &forged,
                None,
                RUNTIME_REQUEST_WORK,
                &mut ImportTrace::default()
            ),
            Err(Refusal::Invalid)
        ));
    }

    #[test]
    fn composed_program_package_reconstructs_checked_graph_not_claimed_edges() {
        let source = b"module Worker@1 [ def double [ \"nested\" drop 2 * ] export plus def plus [ double 1 + ] ]";
        let (mut store, root) =
            Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK).unwrap();
        let double = store.checked_root(0).unwrap();
        let composed = store.compose(root, double).unwrap();
        let source_sha256 = crate::workflow::intrinsic::sha256(source);
        let capture_sha256 = store.capture_digest().unwrap();
        let grant = ExportGrant {
            destination: "private",
            source_sha256: &source_sha256,
            capture_sha256: &capture_sha256,
            allowed: true,
        };
        let package = store.export(composed, "private", &grant).unwrap();
        let (mut imported, selected) = Store::import(
            &package,
            None,
            RUNTIME_REQUEST_WORK,
            &mut ImportTrace::default(),
        )
        .unwrap();
        assert_eq!(imported.capture_wire().unwrap()[0]["text_utf8"], "nested");
        assert_eq!(
            imported.recipe_edges(selected),
            Some((root.node, double.node))
        );
        assert_eq!(
            crate::core::worker_program_guest::execute(&mut imported, selected, 3, 1)
                .unwrap()
                .value,
            14,
            "freshly imported graph must execute its selected composition"
        );
        let mut claim: Value = serde_json::from_slice(&package).unwrap();
        claim["graph"][2]["second"] = json!(root.node);
        let forged = serde_json::to_vec(&claim).unwrap();
        assert!(
            matches!(
                Store::import(
                    &forged,
                    None,
                    RUNTIME_REQUEST_WORK,
                    &mut ImportTrace::default()
                ),
                Err(Refusal::Invalid)
            ),
            "a still-rooted but forged edge cannot pass graph attestation"
        );
    }

    #[test]
    fn checked_named_program_adapts_selects_and_composes_without_changing_its_recipe() {
        let (mut store, original) = Store::prepare(
            b"module Worker@1 [ def minus [ 1 - ] export plus def plus [ minus 1 + ] ]",
            "plus",
            &[Ty::I64],
            RUNTIME_REQUEST_WORK,
        )
        .expect("source-checked named Program");
        store.work_left = 5;
        let other = store
            .checked_root(0)
            .expect("checked reachable second Program");
        assert_ne!(other.node, original.node);
        assert_ne!(
            store.named(other).unwrap().checked_recipe,
            store.named(original).unwrap().checked_recipe
        );
        assert_eq!(store.original(original).unwrap().0, [Ty::I64]);
        assert_eq!(store.interface(original).unwrap().1, [Ty::I64]);
        assert!(store.named(original).is_some());
        assert!(!store.wat().is_empty());
        let selected = store
            .selected_world()
            .expect("source-bound Program WIT resource");
        let kind = selected
            .worker_program_kind()
            .expect("selected resource kind");
        assert_eq!(
            selected.noble_type(noble_contracts::component::Type::Own(kind)),
            Some(Ty::program(vec![Ty::I64], vec![Ty::I64], EffSet::empty()))
        );
        assert!(selected
            .noble_type(noble_contracts::component::Type::Borrow(kind))
            .is_none());
        let ordinary = noble_contracts::component::World::parse(
            include_bytes!("../../../noble-wasm/wit/worker-program.wit"),
            "guest",
            SOURCE_LIMITS,
        )
        .expect("ordinary unselected WIT");
        assert_eq!(
            ordinary.noble_type(noble_contracts::component::Type::Own(kind)),
            Some(Ty::Resource(kind)),
            "ordinary WIT resource must stay untrusted"
        );
        assert!(
            selected
                .prepare_export(
                    "compute",
                    b"programs.admit swap programs.invoke",
                    SOURCE_LIMITS
                )
                .is_ok(),
            "selected source can type the host-admitted Program"
        );
        let destination = EffSet::from_ids(&[TEST_EMIT]);
        let wider = store
            .adapt(original, &destination)
            .expect("checked effect inclusion");
        assert_eq!(
            store.recipe_nodes(),
            2,
            "widening must not add a recipe node"
        );
        assert_eq!(store.original(wider).unwrap().2, &EffSet::empty());
        assert_eq!(store.interface(wider).unwrap().2, &destination);
        let mismatched = Handle {
            node: other.node,
            ..wider
        };
        assert!(store.interface(mismatched).is_none());
        assert!(matches!(
            store.adapt(mismatched, &destination),
            Err(Refusal::Invalid)
        ));
        assert!(
            matches!(store.select(&[other, wider], 1), Err(Refusal::Invalid)),
            "heterogeneous effect witnesses cannot enter a typed list"
        );
        let second = store
            .adapt(other, &destination)
            .expect("second source-checked Program");
        assert_eq!(store.select(&[wider, second], 1), Ok(second));
        let composed = store
            .compose(wider, second)
            .expect("compatible checked stacks");
        assert_eq!(
            store.recipe_edges(composed),
            Some((original.node, other.node))
        );
        assert_eq!(store.interface(composed).unwrap().2, &destination);
        assert_eq!(store.original(original).unwrap().2, &EffSet::empty());
        assert_eq!(store.recipe_nodes(), 3);
        assert!(
            matches!(store.adapt(wider, &EffSet::empty()), Err(Refusal::Invalid)),
            "effect ceiling must not narrow"
        );
        assert!(
            matches!(store.package(wider), Err(Refusal::Unsupported)),
            "widening alone cannot authorize executable export"
        );
        assert!(
            matches!(store.compose(original, composed), Err(Refusal::Exhausted)),
            "a consumed request budget must not silently reset"
        );
    }

    #[test]
    fn forged_or_unchecked_named_program_cannot_become_a_builder_root() {
        assert!(matches!(
            Store::prepare(
                b"module Worker@1 [ def plus [ 1 + ] ]",
                "plus",
                &[Ty::I64],
                1_000_000
            ),
            Err(Refusal::Invalid)
        ));
        assert!(matches!(
            Store::prepare(
                b"module Worker@1 [ export plus def plus [ 1 + ] ]",
                "plus",
                &[Ty::Text],
                1_000_000
            ),
            Err(Refusal::Invalid)
        ));
    }

    #[test]
    fn unissued_effect_witness_cannot_grant_package_export() {
        let source = b"module Worker@1 [ export plus def plus [ 1 + ] ]";
        assert!(
            matches!(
                Store::prepare(source, "plus", &[Ty::I64], 3),
                Err(Refusal::Exhausted)
            ),
            "source checks cannot refresh an empty request meter"
        );
        let (mut pure, root) =
            Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK).expect("pure source");
        pure.work_left = 2;
        let widened = pure
            .adapt(root, &EffSet::from_ids(&[TEST_EMIT]))
            .expect("type-level effect inclusion does not perform a host request");
        assert_eq!(pure.recipe_nodes(), 1);
        assert_eq!(pure.original(widened).unwrap().2, &EffSet::empty());
        assert!(matches!(pure.package(widened), Err(Refusal::Unsupported)));
        assert!(pure.adapt(root, &EffSet::empty()).is_ok());
        assert!(matches!(
            pure.adapt(root, &EffSet::empty()),
            Err(Refusal::Exhausted)
        ));
        assert_eq!(pure.recipe_nodes(), 1);
    }

    #[test]
    fn handle_from_retired_or_other_source_session_cannot_alias_a_new_instance() {
        let source = b"module Worker@1 [ export plus def plus [ 1 + ] ]";
        let (old, stale) = Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK)
            .expect("old source instance");
        assert!(old.interface(stale).is_some());
        drop(old);
        let (mut current, active) =
            Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK)
                .expect("new source instance with same source identity and index");
        assert_ne!(stale, active);
        assert!(current.interface(stale).is_none());
        assert!(current.named(stale).is_none());
        assert!(matches!(
            current.adapt(stale, &EffSet::empty()),
            Err(Refusal::Invalid)
        ));
        assert!(matches!(
            current.compose(stale, active),
            Err(Refusal::Invalid)
        ));
        assert!(matches!(
            current.select(&[active, stale], 0),
            Err(Refusal::Invalid)
        ));
        let source_sha256 = crate::workflow::intrinsic::sha256(source);
        let capture_sha256 = current.capture_digest().unwrap();
        let grant = ExportGrant {
            destination: "local-test",
            source_sha256: &source_sha256,
            capture_sha256: &capture_sha256,
            allowed: true,
        };
        assert!(
            matches!(
                current.export(stale, "local-test", &grant),
                Err(ExportRefusal::Refused(Refusal::Invalid))
            ),
            "a same-source retired handle cannot borrow a new grant"
        );
        assert!(
            current.export(active, "local-test", &grant).is_ok(),
            "the fresh exact-instance root retains its own export authority"
        );
    }

    #[test]
    fn source_bound_package_rechecks_closure_contracts_and_export_authority() {
        let source = b"module Worker@1 [ def minus [ 1 - ] export plus def plus [ minus 1 + ] ]";
        let (mut store, root) = Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK)
            .expect("checked package source");
        let source_sha256 = crate::workflow::intrinsic::sha256(source);
        let capture_sha256 = store.capture_digest().unwrap();
        let grant = ExportGrant {
            destination: "local-test",
            source_sha256: &source_sha256,
            capture_sha256: &capture_sha256,
            allowed: true,
        };
        let denied = ExportGrant {
            allowed: false,
            ..grant
        };
        assert_eq!(
            store.export(root, "local-test", &denied),
            Err(ExportRefusal::Denied)
        );
        assert_eq!(
            store.export(root, "other", &grant),
            Err(ExportRefusal::Denied),
            "a grant for another destination never transfers"
        );
        let alternate = (0..store.recipe_nodes())
            .filter_map(|index| store.checked_root(index))
            .find(|candidate| candidate.node != root.node)
            .expect("checked dependency");
        assert!(
            matches!(
                store.export(alternate, "local-test", &grant),
                Err(ExportRefusal::Refused(Refusal::Unsupported))
            ),
            "a different named root cannot inherit export authority"
        );
        let dependency_only = store
            .compose(alternate, alternate)
            .expect("two checked dependencies compose without the selected root");
        assert!(
            matches!(
                store.export(dependency_only, "local-test", &grant),
                Err(ExportRefusal::Refused(Refusal::Unsupported))
            ),
            "composition without the selected root cannot inherit export authority"
        );
        let encoded = store
            .export(root, "local-test", &grant)
            .expect("explicit export grant");
        let (imported, restored) = Store::import(
            &encoded,
            None,
            RUNTIME_REQUEST_WORK,
            &mut ImportTrace::default(),
        )
        .expect("fresh locally checked package import");
        assert_eq!(
            imported.named(restored).unwrap().checked_recipe,
            store.named(root).unwrap().checked_recipe
        );
        assert!(imported.interface(restored).unwrap().2.is_empty());
        let parsed: Value = serde_json::from_slice(&encoded).expect("test package");
        let mut forged_selection = parsed.clone();
        forged_selection["selected_node"] = json!(alternate.node);
        forged_selection["selected_graph_blake3"] = json!(store
            .selected_graph_blake3(alternate.node)
            .expect("checked dependency digest"));
        assert!(
            matches!(
                Store::import(
                    &serde_json::to_vec(&forged_selection).unwrap(),
                    None,
                    RUNTIME_REQUEST_WORK,
                    &mut ImportTrace::default()
                ),
                Err(Refusal::Unsupported)
            ),
            "a recomputed digest cannot authorize importing another selected root"
        );
        for (label, altered, expected) in [
            (
                "missing transitive dependency",
                {
                    let mut value = parsed.clone();
                    value["definitions"].as_array_mut().unwrap().remove(0);
                    value
                },
                Refusal::Invalid,
            ),
            (
                "unissued host contract",
                {
                    let mut value = parsed.clone();
                    value["host_contracts"] = json!(["test.emit@1.0.0"]);
                    value
                },
                Refusal::Invalid,
            ),
            (
                "forged root identity",
                {
                    let mut value = parsed.clone();
                    value["root"]["owner"]["identity"] = json!("0");
                    value
                },
                Refusal::Invalid,
            ),
            (
                "unrelated executable artifact",
                {
                    let mut value = parsed.clone();
                    value["artifact"] = json!({"wasm":"claimed-accepted"});
                    value
                },
                Refusal::Invalid,
            ),
            (
                "unbound captured Text",
                {
                    let mut value = parsed.clone();
                    value["captures"] = json!([{"type":"Text","value":"synthetic-private-text"}]);
                    value
                },
                Refusal::Invalid,
            ),
            (
                "unsupported revision",
                {
                    let mut value = parsed.clone();
                    value["semantic_revision"] = json!("future");
                    value
                },
                Refusal::Unsupported,
            ),
        ] {
            let bytes = serde_json::to_vec(&altered).expect(label);
            assert!(
                matches!(Store::import(&bytes, None, RUNTIME_REQUEST_WORK, &mut ImportTrace::default()),
                Err(refusal) if refusal == expected),
                "{label}"
            );
        }
    }

    /// Host-selected exact `test.emit` binding for the `Ledger@1.plus` fixture.
    fn emit_policy(source: &[u8]) -> super::super::worker_program_guest::WorkerHostPolicy {
        let binding = noble_contracts::source::BoundOperation {
            module_name: "Ledger".into(),
            module_version: 1,
            operation: "test.emit".into(),
            adapter_identity: "worker-test-emit@1.0.0".into(),
            adapter_slot: 4,
            input: vec![Ty::Text],
            output: vec![],
            effects: vec![TEST_EMIT],
        };
        let retained = noble_contracts::source::RetainedSourceLimit::new(SOURCE_LIMITS.bytes)
            .expect("selected Worker source limit");
        let session =
            noble_contracts::source::ModuleSession::new(std::slice::from_ref(&binding), retained)
                .unwrap();
        let module = session.prepare(source, &[], SOURCE_LIMITS).unwrap();
        let owner = module
            .checked_program_owner_for_binding("plus", &binding)
            .expect("host-selected exact checked owner");
        let (session, committed) = session.commit(module);
        committed.unwrap();
        super::super::worker_program_guest::WorkerHostPolicy::select(
            crate::workflow::intrinsic::sha256(source),
            owner.definition_identity(),
            session.generation(),
            "local-test-destination".into(),
            1,
            binding,
        )
        .unwrap()
    }

    /// Export `handle` under a host grant for exactly this store's source and
    /// captures.
    fn granted_export(store: &mut Store, handle: Handle) -> Vec<u8> {
        let source_sha256 = crate::workflow::intrinsic::sha256(&store.source);
        let capture_sha256 = store.capture_digest().unwrap();
        let grant = ExportGrant {
            destination: "local-test",
            source_sha256: &source_sha256,
            capture_sha256: &capture_sha256,
            allowed: true,
        };
        store.export(handle, "local-test", &grant).unwrap()
    }

    #[test]
    fn bound_package_records_exact_versions_and_imports_only_under_its_binding() {
        let source = b"module Ledger@1 [ require emit Text -- ! test.emit export plus def plus [ \"audit\" emit 1 + ] ]";
        let policy = emit_policy(source);
        let (mut store, root) = Store::prepare_with_policy(
            source,
            "plus",
            &[Ty::I64],
            policy.clone(),
            RUNTIME_REQUEST_WORK,
        )
        .unwrap();
        let encoded = granted_export(&mut store, root);
        let package: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(package["interface"]["effects"], json!(["test.emit"]));
        assert_eq!(
            package["host_contracts"],
            json!([{"module_name":"Ledger","module_version":1,"operation":"test.emit",
                "adapter_identity":"worker-test-emit@1.0.0","adapter_slot":4,
                "input":["Text"],"output":[],"effects":["test.emit"]}])
        );
        let builtins = package["builtins"].as_array().unwrap();
        assert_eq!(builtins.len(), 22);
        assert!(builtins.iter().enumerate().all(|(ordinal, row)| {
            row["ordinal"] == ordinal
                && row["semantic_revision"] == "0.1.0-draft.5"
                && row["name"]
                    == noble_contracts::source::pure_builtin_spelling(
                        u32::try_from(ordinal).unwrap(),
                    )
                    .unwrap()
        }));
        assert_eq!(package["captures"][0]["schema"], "Text");

        let mut trace = ImportTrace::default();
        let (imported, selected) = Store::import(
            &encoded,
            Some(policy.clone()),
            RUNTIME_REQUEST_WORK,
            &mut trace,
        )
        .expect("the same host binding rechecks the package");
        assert_eq!(trace.installations, 1, "only Text reattestation installs");
        assert_eq!(
            imported.interface(selected).unwrap().2,
            &EffSet::from_ids(&[TEST_EMIT])
        );

        let mut trace = ImportTrace::default();
        assert!(
            Store::import(&encoded, None, RUNTIME_REQUEST_WORK, &mut trace).is_err(),
            "the pure import path cannot admit a host-bound package"
        );
        assert_eq!(trace.installations, 0);
        for (label, path, substitute) in [
            (
                "substituted host-contract version",
                "/host_contracts/0/adapter_identity",
                json!("worker-test-emit@1.0.1"),
            ),
            (
                "substituted adapter slot",
                "/host_contracts/0/adapter_slot",
                json!(5),
            ),
            (
                "substituted builtin revision",
                "/builtins/4/semantic_revision",
                json!("0.1.0-draft.6"),
            ),
            (
                "substituted capture schema",
                "/captures/0/schema",
                json!("Bytes"),
            ),
        ] {
            let mut altered = package.clone();
            *altered.pointer_mut(path).expect(label) = substitute;
            let mut trace = ImportTrace::default();
            assert_eq!(
                Store::import(
                    &serde_json::to_vec(&altered).unwrap(),
                    Some(policy.clone()),
                    RUNTIME_REQUEST_WORK,
                    &mut trace,
                )
                .err(),
                Some(Refusal::Invalid),
                "{label}"
            );
            assert_eq!(
                trace.installations, 0,
                "{label} installed Wasm before refusal"
            );
        }
    }

    #[test]
    fn hostile_text_package_alternatives_reject_before_any_installation() {
        let source = b"module Worker@1 [ def minus [ \"tag\" drop 1 - ] export plus def plus [ minus 1 + ] ]";
        let (mut store, root) =
            Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK).unwrap();
        let encoded = granted_export(&mut store, root);
        let package: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(package["captures"][0]["text_utf8"], "tag");
        let mut trace = ImportTrace::default();
        Store::import(&encoded, None, RUNTIME_REQUEST_WORK, &mut trace)
            .expect("the exact locally checked closure and witness import");
        assert_eq!(trace.installations, 1);

        let foreign_owner = package["definitions"][0]["owner"].clone();
        assert_ne!(foreign_owner, package["root"]["owner"]);
        for (label, altered, expected) in [
            (
                "missing transitive definition",
                {
                    let mut value = package.clone();
                    value["definitions"].as_array_mut().unwrap().remove(0);
                    value
                },
                Refusal::Invalid,
            ),
            (
                "foreign self owner",
                {
                    let mut value = package.clone();
                    value["root"]["owner"] = foreign_owner.clone();
                    value
                },
                Refusal::Invalid,
            ),
            (
                "unrelated artifact",
                {
                    let mut value = package.clone();
                    value["artifact"] = json!({"wasm":"claimed-accepted"});
                    value
                },
                Refusal::Invalid,
            ),
            (
                "accepted flag",
                {
                    let mut value = package.clone();
                    value["accepted"] = json!(true);
                    value
                },
                Refusal::Invalid,
            ),
            (
                "invalid body keeping the valid package's digest",
                {
                    let mut value = package.clone();
                    value["source_utf8"] =
                        json!(String::from_utf8_lossy(source).replace("1 - ]", "1 - - ]"));
                    value
                },
                Refusal::Invalid,
            ),
            (
                "invalid body with digest and accepted flag",
                {
                    let mut value = package.clone();
                    value["source_utf8"] =
                        json!(String::from_utf8_lossy(source).replace("1 - ]", "1 - - ]"));
                    value["accepted"] = json!(true);
                    value
                },
                Refusal::Invalid,
            ),
            (
                "forged captured Text",
                {
                    let mut value = package.clone();
                    value["captures"][0]["text_utf8"] = json!("forged");
                    value
                },
                Refusal::Invalid,
            ),
            (
                "Text without a claimed binding",
                {
                    let mut value = package.clone();
                    value["capture_binding_blake3"] = Value::Null;
                    value
                },
                Refusal::Invalid,
            ),
        ] {
            let mut trace = ImportTrace::default();
            assert_eq!(
                Store::import(
                    &serde_json::to_vec(&altered).unwrap(),
                    None,
                    RUNTIME_REQUEST_WORK,
                    &mut trace,
                )
                .err(),
                Some(expected),
                "{label}"
            );
            assert_eq!(
                trace.installations, 0,
                "{label} installed Wasm before refusal"
            );
        }

        // A wrong binding claim matches every derived field, so only the
        // reattestation of the installed Text can refuse it.
        let mut misbound = package.clone();
        misbound["capture_binding_blake3"] = json!("0".repeat(64));
        let mut trace = ImportTrace::default();
        assert_eq!(
            Store::import(
                &serde_json::to_vec(&misbound).unwrap(),
                None,
                RUNTIME_REQUEST_WORK,
                &mut trace,
            )
            .err(),
            Some(Refusal::Invalid)
        );
        assert_eq!(trace.installations, 1);
    }

    #[test]
    fn import_admits_an_exact_shared_budget_and_exhausts_one_charged_node_over() {
        let source = b"module Worker@1 [ def double [ \"nested\" drop 2 * ] export plus def plus [ double 1 + ] ]";
        let package = |extra_compositions: usize| -> Vec<u8> {
            let (mut store, root) =
                Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK).unwrap();
            let double = store.checked_root(0).unwrap();
            let selected = store.compose(root, double).unwrap();
            for _ in 0..extra_compositions {
                store.compose(root, double).unwrap();
            }
            granted_export(&mut store, selected)
        };
        let exact = package(0);
        let one_more = package(1);
        let import = |bytes: &[u8], work: usize| {
            let mut trace = ImportTrace::default();
            Store::import(bytes, None, work, &mut trace).map(|_| ())
        };
        assert!(import(&one_more, RUNTIME_REQUEST_WORK).is_ok());

        // Every budget below the minimum must refuse as exhausted: no stage
        // reports a smaller budget as a different primary outcome.
        let (mut short, mut enough) = (0usize, RUNTIME_REQUEST_WORK);
        while enough - short > 1 {
            let middle = short + (enough - short) / 2;
            match import(&exact, middle) {
                Ok(()) => enough = middle,
                Err(Refusal::Exhausted) => short = middle,
                Err(other) => panic!("budget {middle} refused as {other:?}"),
            }
        }
        assert!(import(&exact, enough).is_ok());
        assert_eq!(import(&exact, enough - 1).err(), Some(Refusal::Exhausted));
        assert_eq!(
            import(&one_more, enough).err(),
            Some(Refusal::Exhausted),
            "one more decoded graph node must not fit the exact budget"
        );
    }

    const RECURSIVE: &[u8] = b"module Worker@1 [ def dec [ \"tag\" drop 1 - ] signature count forall<S:stack> [ S I64 -- S I64 ! pure ] export count def count [ dup 0 = [ 1 + ] [ dec count 1 + ] if ] ]";

    #[test]
    fn recursive_package_retains_its_scoped_self_owner_and_rejects_a_foreign_one() {
        let (mut store, root) =
            Store::prepare(RECURSIVE, "count", &[Ty::I64], RUNTIME_REQUEST_WORK)
                .expect("signed recursive Program");
        let encoded = granted_export(&mut store, root);
        let package: Value = serde_json::from_slice(&encoded).unwrap();
        let dec = package["definitions"][0].clone();
        let count = package["definitions"][1].clone();
        assert_eq!(count["owner"], package["root"]["owner"]);
        assert_eq!(count["self_owner"], count["owner"]);
        assert_eq!(dec["self_owner"], Value::Null);
        let mut trace = ImportTrace::default();
        let (mut imported, selected) =
            Store::import(&encoded, None, RUNTIME_REQUEST_WORK, &mut trace)
                .expect("the exact recursive package imports");
        assert_eq!(trace.installations, 1);
        assert_eq!(
            crate::core::worker_program_guest::execute(&mut imported, selected, 20, 1)
                .unwrap()
                .value,
            21,
            "the imported recursive Program must run through its own owner"
        );
        for (label, path, value) in [
            (
                "self owner of another definition",
                "/definitions/1/self_owner",
                dec["owner"].clone(),
            ),
            (
                "self owner claimed by a non-recursive definition",
                "/definitions/0/self_owner",
                dec["owner"].clone(),
            ),
            (
                "recursive owner dropped",
                "/definitions/1/self_owner",
                Value::Null,
            ),
            (
                "self owner from a foreign module",
                "/definitions/1/self_owner/module_id",
                json!("999"),
            ),
        ] {
            let mut altered = package.clone();
            *altered.pointer_mut(path).expect(label) = value;
            let mut trace = ImportTrace::default();
            assert_eq!(
                Store::import(
                    &serde_json::to_vec(&altered).unwrap(),
                    None,
                    RUNTIME_REQUEST_WORK,
                    &mut trace,
                )
                .err(),
                Some(Refusal::Invalid),
                "{label}"
            );
            assert_eq!(
                trace.installations, 0,
                "{label} installed Wasm before refusal"
            );
        }
        for (label, source, name) in [
            (
                "unsigned self recursion",
                b"module Worker@1 [ export spin def spin [ 1 + spin ] ]".as_slice(),
                "spin",
            ),
            (
                "polymorphic recursion",
                b"module Worker@1 [ signature recur forall<S:stack> [ S -- S ! pure ] export recur def recur [ 1 recur drop ] ]".as_slice(),
                "recur",
            ),
            (
                "mutual recursion",
                b"module Worker@1 [ signature even forall<S:stack> [ S I64 -- S I64 ! pure ] signature odd forall<S:stack> [ S I64 -- S I64 ! pure ] def odd [ even ] export even def even [ odd ] ]".as_slice(),
                "even",
            ),
        ] {
            assert!(
                Store::prepare(source, name, &[Ty::I64], RUNTIME_REQUEST_WORK).is_err(),
                "{label} was admitted"
            );
        }
    }
}
