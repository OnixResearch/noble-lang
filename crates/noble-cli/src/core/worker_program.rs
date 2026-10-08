//! Opt-in, source-bound Worker Program builders. The compiled named Program is
//! the only root: handles are indices into this invocation's private graph,
//! never package-supplied executable identities or host-operation grants.

use noble_kernel::types::{EffSet, Ty};
use noble_wasm::source::{NamedTargetMetadata, Prepared as Compiled};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) const SOURCE_LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536, nodes: 4_096, depth: 32, work: 3_000_000,
};
const GRAPH_LIMIT: usize = 4_096;
const WITNESS_LIMIT: usize = 4_096;
const RECIPE_DEPTH_LIMIT: u32 = 32;
const RECIPE_LEAF_LIMIT: u32 = 256;
const PACKAGE_BYTES: usize = 262_144;
const PACKAGE_DEPTH: usize = 32;
const RUNTIME_REQUEST_WORK: usize = 50_000_000;
const PACKAGE_REVISION: &str = "0.1.0-draft.9";
const WIT_WORLD: &str = "noble-test:worker-program/guest@2.0.0";
static NEXT_INSTANCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Invalid,
    Unsupported,
    Exhausted,
}

struct Node {
    recipe: Recipe,
    original: usize,
    depth: u32,
    leaves: u32,
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
    pub(crate) fn prepare(source: &[u8], name: &str, input: &[Ty], work: usize)
        -> Result<(Self, Handle), Refusal> {
        Self::prepare_selected(source, name, input, None, work)
    }

    /// Only a host embedder may opt in to a checked, bound operation. A typed
    /// source effect and caller-supplied package are never host policy.
    pub(crate) fn prepare_with_policy(source: &[u8], name: &str, input: &[Ty],
        policy: super::worker_program_guest::WorkerHostPolicy, work: usize)
        -> Result<(Self, Handle), Refusal> {
        Self::prepare_selected(source, name, input, Some(policy), work)
    }

    fn prepare_selected(source: &[u8], name: &str, input: &[Ty],
        policy: Option<super::worker_program_guest::WorkerHostPolicy>, work: usize)
        -> Result<(Self, Handle), Refusal> {
        if source.len() > SOURCE_LIMITS.bytes as usize || input.len() > 64 || name.len() > 128
            || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
            return Err(Refusal::Exhausted);
        }
        // Reserve from one request meter: module check, selected invocation
        // check and Wasm compilation cannot each refresh a full 3M allowance.
        let stage_work = u32::try_from((work / 4).min(SOURCE_LIMITS.work as usize))
            .map_err(|_| Refusal::Exhausted)?;
        if stage_work == 0 { return Err(Refusal::Exhausted) }
        let limits = noble_contracts::Limits { work: stage_work, ..SOURCE_LIMITS };
        let reserved = usize::try_from(stage_work).map_err(|_| Refusal::Exhausted)?
            .checked_mul(3).ok_or(Refusal::Exhausted)?;
        let mut work_left = work.checked_sub(reserved).ok_or(Refusal::Exhausted)?;
        let bindings = policy.as_ref().map_or(&[][..], |selection|
            std::slice::from_ref(selection.binding()));
        let retained = noble_contracts::source::RetainedSourceLimit::new(limits.bytes)
            .ok_or(Refusal::Exhausted)?;
        let session = noble_contracts::source::ModuleSession::new(bindings, retained)
            .map_err(source_refusal)?;
        let module = session.prepare(source, &[], limits).map_err(source_refusal)?;
        if module.kind() != noble_contracts::source::ModuleKind::Module
            || module.proof_obligations().is_some()
            || (policy.is_none() && !module.bindings().is_empty())
            || policy.as_ref().is_some_and(|selected|
                module.bindings() != std::slice::from_ref(selected.binding())) {
            return Err(Refusal::Unsupported);
        }
        let owner = match &policy {
            Some(selected) => module.checked_program_owner_for_binding(name, selected.binding()),
            None => module.checked_program_owner(name),
        }.ok_or(Refusal::Invalid)?;
        let closure = match &policy {
            Some(selected) => module.checked_program_closure_for_binding(name, selected.binding()),
            None => module.checked_program_closure(name),
        }.ok_or(Refusal::Invalid)?;
        if closure.is_empty() { return Err(Refusal::Invalid) }
        work_left = work_left.checked_sub(source.len())
            .and_then(|left| left.checked_sub(closure.len().checked_mul(8)?))
            .ok_or(Refusal::Exhausted)?;
        let (_, module_name, version) = module.resolved_module().ok_or(Refusal::Invalid)?;
        let (session, committed) = session.commit(module);
        committed.map_err(source_refusal)?;
        if policy.as_ref().is_some_and(|selected| !selected.matches_source(
            source, owner.definition_identity(), session.generation())) {
            return Err(Refusal::Invalid);
        }
        let invocation = format!("{module_name}@{version}.{name}");
        let prepared = session.prepare(invocation.as_bytes(), input, limits)
            .map_err(source_refusal)?;
        let checked_interface = match &policy {
            Some(selected) => prepared.checked_program_interface_for_binding(
                &owner, selected.binding()),
            None => prepared.checked_program_interface(&owner),
        }.ok_or(Refusal::Invalid)?;
        let submission = prepared.submission().ok_or(Refusal::Invalid)?;
        if submission.request.expected.stack_in != input
            || match &policy {
                Some(selected) => submission.request.expected.allowed_effects.as_slice()
                    != selected.binding().effects.as_slice(),
                None => !submission.request.expected.allowed_effects.is_empty(),
            }
            || submission.definitions.iter().filter(|definition|
                definition.owner_slot == Some(owner.definition_slot())
                && definition.source_identity == owner.definition_identity()
                && submission.environment.definition_owners.get(definition.definition.0 as usize)
                    == Some(&Some(owner.module_identity()))).count() != 1 {
            return Err(Refusal::Invalid);
        }
        let compiled = noble_wasm::source::Compiler::new_worker().prepare(submission)
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
            let mut source_definitions = submission.definitions.iter().filter(|definition|
                definition.owner_slot == Some(dependency.definition_slot())
                && definition.source_identity == dependency.definition_identity()
                && submission.environment.definition_owners.get(definition.definition.0 as usize)
                    == Some(&Some(owner.module_identity())));
            let (Some(source_definition), None) =
                (source_definitions.next(), source_definitions.next())
                else { return Err(Refusal::Invalid) };
            let mut named = compiled.named_targets().iter().filter(|target|
                target.definition == source_definition.definition
                && target.definition_identity == source_definition.identity
                && submission.environment.definition_owners.get(target.definition.0 as usize)
                    == Some(&Some(owner.module_identity())));
            let (Some(target), None) = (named.next(), named.next())
                else { return Err(Refusal::Invalid) };
            if target.stack_in.len() > 64 || target.stack_out.len() > 64
                || !target.effects.is_subset_of(&submission.request.expected.allowed_effects)
                || target.checked_recipe.is_empty() {
                return Err(Refusal::Invalid);
            }
            let index = nodes.len();
            let metadata = compiled.program_metadata().get(target.program_index)
                .ok_or(Refusal::Invalid)?;
            if metadata.recipe_depth > RECIPE_DEPTH_LIMIT
                || metadata.recipe_leaves > RECIPE_LEAF_LIMIT {
                return Err(Refusal::Exhausted);
            }
            nodes.push(Node { recipe: Recipe::Named { program_index: target.program_index },
                original: index, depth: metadata.recipe_depth,
                leaves: metadata.recipe_leaves });
            owners.push(Owner { module_id: dependency.module_identity(),
                version: dependency.module_version(), ordinal: dependency.definition_ordinal(),
                identity: dependency.definition_identity() });
            for text in &source_definition.body.texts {
                let node = usize::try_from(text.node.0).map_err(|_| Refusal::Exhausted)?;
                if text_aliases.len() >= GRAPH_LIMIT || text.bytes.len() > SOURCE_LIMITS.bytes as usize
                    || !matches!(source_definition.body.candidate.nodes
                        .get(node),
                        Some(noble_kernel::untrusted::Node::Literal {
                            lit: noble_kernel::untrusted::Lit::Text, ..
                        })) {
                    return Err(Refusal::Invalid);
                }
                text_aliases.push(CheckedTextAlias { owner_identity: dependency.definition_identity(),
                    node: text.node.0, bytes: text.bytes.clone() });
            }
            witnesses.push(Witness { node: index, input: target.stack_in.clone(),
                output: target.stack_out.clone(), effects: target.effects.clone() });
            if dependency.definition_slot() == owner.definition_slot() {
                if target.stack_in.as_slice() != input
                    || target.stack_out.as_slice() != prepared.output() {
                    return Err(Refusal::Invalid);
                }
                root = Some(index);
            }
        }
        let root = root.ok_or(Refusal::Invalid)?;
        let instance = NEXT_INSTANCE.fetch_update(Ordering::Relaxed, Ordering::Relaxed,
            |next| next.checked_add(1)).map_err(|_| Refusal::Exhausted)?;
        let store = Self { instance, host_policy: policy,
            checked_interface,
            source: source.to_vec(), root_name: name.to_owned(),
            root_owner: Owner { module_id: owner.module_identity(),
                version: owner.module_version(), ordinal: owner.definition_ordinal(),
                identity: owner.definition_identity() },
            root_index: root,
            closure: owners, text_aliases, attested_capture_hash: None,
            nodes, witnesses, compiled, work_left };
        Ok((store, Handle { instance, node: root, witness: root }))
    }

    pub(crate) fn wat(&self) -> &[u8] { self.compiled.wat() }
    pub(crate) fn compiled(&self) -> &Compiled { &self.compiled }
    pub(crate) fn instance_id(&self) -> u64 { self.instance }
    pub(crate) fn root_owner_identity(&self) -> u64 { self.root_owner.identity }
    pub(crate) fn source(&self) -> &[u8] { &self.source }
    pub(crate) fn host_policy(&self) -> Option<&super::worker_program_guest::WorkerHostPolicy> {
        self.host_policy.as_ref()
    }
    pub(crate) fn text_aliases(&self) -> &[CheckedTextAlias] { &self.text_aliases }

    fn capture_wire(&self) -> Result<Value, Refusal> {
        let mut values = Vec::with_capacity(self.text_aliases.len());
        for alias in &self.text_aliases {
            let text = std::str::from_utf8(&alias.bytes).map_err(|_| Refusal::Invalid)?;
            values.push(json!({"owner_identity":alias.owner_identity.to_string(),
                "source_node":alias.node,"text_utf8":text}));
        }
        Ok(Value::Array(values))
    }

    fn selected_graph_blake3(&self, selected: usize) -> Result<String, Refusal> {
        let mut digest = blake3::Hasher::new();
        digest.update(b"noble-worker-checked-graph-v2\0");
        digest.update(&self.source);
        digest.update(&u64::try_from(selected).map_err(|_| Refusal::Exhausted)?.to_le_bytes());
        for (index, node) in self.nodes.iter().enumerate() {
            match &node.recipe {
                Recipe::Named { program_index } => {
                    digest.update(b"named\0");
                    digest.update(&self.closure.get(index).ok_or(Refusal::Invalid)?
                        .identity.to_le_bytes());
                    digest.update(&self.compiled.named_targets().iter()
                        .find(|named| named.program_index == *program_index)
                        .ok_or(Refusal::Invalid)?.checked_recipe);
                }
                Recipe::Compose { first, second } => {
                    digest.update(b"compose\0");
                    digest.update(&u64::try_from(*first).map_err(|_| Refusal::Exhausted)?
                        .to_le_bytes());
                    digest.update(&u64::try_from(*second).map_err(|_| Refusal::Exhausted)?
                        .to_le_bytes());
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
            Some(policy) if policy.binding().operation == "test.emit" =>
                world.select_worker_program_for_test_emit(&self.checked_interface),
            Some(policy) if policy.binding().operation == "test.code-export" =>
                world.select_worker_program_for_test_code_export(&self.checked_interface),
            Some(_) => return Err(Refusal::Unsupported),
            None => world.select_worker_program(&self.checked_interface),
        }
            .map_err(|_| Refusal::Invalid)
    }

    fn package(&self, handle: Handle) -> Result<Value, Refusal> {
        if handle.instance != self.instance { return Err(Refusal::Invalid) }
        let root = self.node_handle(self.root_index).ok_or(Refusal::Invalid)?;
        let target = self.named(root).ok_or(Refusal::Invalid)?;
        if self.nodes.get(handle.node).is_none_or(|node| node.original != handle.witness) {
            return Err(Refusal::Unsupported);
        }
        let witness = self.witnesses.get(handle.witness).ok_or(Refusal::Invalid)?;
        if witness.input != [Ty::I64] || witness.output != [Ty::I64]
            || !witness.effects.is_empty() {
            return Err(Refusal::Unsupported);
        }
        if self.nodes.iter().any(|node| self.witnesses.get(node.original)
            .is_none_or(|original| !original.effects.is_empty())) {
            return Err(Refusal::Unsupported);
        }
        let source = std::str::from_utf8(&self.source).map_err(|_| Refusal::Unsupported)?;
        let mut definitions = Vec::with_capacity(self.closure.len());
        for (index, owner) in self.closure.iter().enumerate() {
            let named = self.named(Handle { instance: self.instance, node: index, witness: index })
                .ok_or(Refusal::Invalid)?;
            definitions.push(json!({"owner":owner.wire(),
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
        let capture_binding_blake3 = match self.attested_capture_hash {
            Some(hash) => Some(crate::workflow::encoding::hex(&hash)),
            None if self.text_aliases.is_empty() => None,
            None => return Err(Refusal::Invalid),
        };
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
            "interface":{"input":["I64"],"output":["I64"],"effects":[]},
            "host_contracts":[],
            "captures":captures,
            "capture_binding_blake3":capture_binding_blake3,
            "artifact":null
        }))
    }

    /// Authorization precedes even package construction/serialization.
    /// A grant to another destination or capture digest never transfers.
    #[cfg(test)]
    fn export(&mut self, handle: Handle, destination: &str,
                         grant: &ExportGrant<'_>) -> Result<Vec<u8>, Refusal> {
        let captures = serde_json::to_vec(&self.capture_wire()?)
            .map_err(|_| Refusal::Invalid)?;
        if !grant.allowed || destination != grant.destination
            || grant.source_sha256 != crate::workflow::intrinsic::sha256(&self.source)
            || grant.capture_sha256 != crate::workflow::intrinsic::sha256(&captures) {
            return Err(Refusal::Invalid);
        }
        let package_work = self.nodes.len().checked_mul(16)
            .and_then(|count| count.checked_add(self.source.len()))
            .and_then(|count| count.checked_add(captures.len()))
            .ok_or(Refusal::Exhausted)?;
        self.charge(package_work)?;
        if !self.text_aliases.is_empty() {
            self.reserve_attestation()?;
            self.attested_capture_hash = Some(super::worker_program_guest::attest(self, handle)
                .map_err(runtime_refusal)?);
        }
        let package = self.package(handle)?;
        let encoded = serde_json::to_vec(&package).map_err(|_| Refusal::Invalid)?;
        if encoded.len() > PACKAGE_BYTES { return Err(Refusal::Exhausted) }
        Ok(encoded)
    }

    /// Import never accepts a package's digest, flags or artifact as proof:
    /// compare every bounded field with a newly checked, emitted pure source
    /// package. Nonempty host contracts have no issuer and do not import.
    pub(crate) fn import(bytes: &[u8], work: usize)
        -> Result<(Self, Handle), Refusal> {
        if bytes.len() > PACKAGE_BYTES { return Err(Refusal::Exhausted) }
        let mut depth = 0usize;
        let mut quoted = false;
        let mut escaped = false;
        let mut tokens = 0usize;
        for byte in bytes {
            if quoted {
                if escaped { escaped = false; }
                else if *byte == b'\\' { escaped = true; }
                else if *byte == b'"' { quoted = false; }
                continue;
            }
            match byte {
                b'"' => {
                    quoted = true;
                    tokens = tokens.checked_add(1).ok_or(Refusal::Exhausted)?;
                }
                b'{' | b'[' => {
                    depth = depth.checked_add(1).ok_or(Refusal::Exhausted)?;
                    if depth > PACKAGE_DEPTH { return Err(Refusal::Exhausted) }
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
            if tokens > GRAPH_LIMIT * 8 { return Err(Refusal::Exhausted) }
        }
        if depth != 0 || quoted { return Err(Refusal::Invalid) }
        let memory = bytes.len().checked_mul(3)
            .and_then(|base| tokens.checked_mul(128)?.checked_add(base))
            .ok_or(Refusal::Exhausted)?;
        if memory > 1_048_576 { return Err(Refusal::Exhausted) }
        let parser_work = bytes.len().checked_mul(3)
            .and_then(|base| tokens.checked_mul(8)?.checked_add(base))
            .ok_or(Refusal::Exhausted)?;
        let remaining = work.checked_sub(parser_work).ok_or(Refusal::Exhausted)?;
        let document: Value = serde_json::from_slice(bytes).map_err(|_| Refusal::Invalid)?;
        if serde_json::to_vec(&document).map_err(|_| Refusal::Invalid)? != bytes {
            return Err(Refusal::Invalid)
        }
        if document["format"] != "noble-worker-program-package/v2"
            || document["semantic_revision"] != PACKAGE_REVISION {
            return Err(Refusal::Unsupported);
        }
        if document["artifact"] != Value::Null
            || document["wit_world"] != WIT_WORLD
            || document.as_object().is_none_or(|fields| fields.len() != 14) {
            return Err(Refusal::Unsupported);
        }
        let source = document["source_utf8"].as_str().ok_or(Refusal::Invalid)?;
        let name = document["root"]["name"].as_str().ok_or(Refusal::Invalid)?;
        let (mut store, _root) = Self::prepare(source.as_bytes(), name, &[Ty::I64], remaining)?;
        let graph = document["graph"].as_array().ok_or(Refusal::Invalid)?;
        if graph.len() < store.recipe_nodes() { return Err(Refusal::Invalid) }
        if graph.len() > GRAPH_LIMIT { return Err(Refusal::Exhausted) }
        for row in graph.iter().skip(store.recipe_nodes()) {
            if row["kind"] != "compose" { return Err(Refusal::Invalid) }
            let first = row["first"].as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| store.node_handle(index)).ok_or(Refusal::Invalid)?;
            let second = row["second"].as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| store.node_handle(index)).ok_or(Refusal::Invalid)?;
            store.compose(first, second)?;
        }
        let selected = document["selected_node"].as_u64()
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| store.node_handle(index)).ok_or(Refusal::Invalid)?;
        let comparison_work = store.recipe_nodes().checked_mul(16)
            .and_then(|count| count.checked_add(source.len()))
            .and_then(|count| count.checked_add(bytes.len()))
            .ok_or(Refusal::Exhausted)?;
        store.charge(comparison_work)?;
        if !store.text_aliases.is_empty() {
            store.reserve_attestation()?;
            let attested = super::worker_program_guest::attest(&store, selected)
                .map_err(runtime_refusal)?;
            store.attested_capture_hash = Some(attested);
        }
        if store.package(selected)? != document { return Err(Refusal::Invalid) }
        Ok((store, selected))
    }

    /// A second candidate must be an independently checked reachable named
    /// Program in the same compiled source closure, not a claimed package row.
    pub(crate) fn checked_root(&self, index: usize) -> Option<Handle> {
        matches!(&self.nodes.get(index)?.recipe, Recipe::Named { .. })
            .then_some(Handle { instance: self.instance, node: index,
                witness: self.nodes[index].original })
    }

    pub(crate) fn node_handle(&self, index: usize) -> Option<Handle> {
        Some(Handle { instance: self.instance, node: index,
            witness: self.nodes.get(index)?.original })
    }

    pub(crate) fn node_index(&self, handle: Handle) -> Option<usize> {
        self.interface(handle).map(|_| handle.node)
    }

    pub(crate) fn named(&self, handle: Handle) -> Option<&NamedTargetMetadata> {
        if handle.instance != self.instance { return None }
        if self.witnesses.get(handle.witness)?.node != handle.node { return None }
        match &self.nodes.get(handle.node)?.recipe {
            Recipe::Named { program_index } => self.compiled.named_targets().iter()
                .find(|target| target.program_index == *program_index),
            Recipe::Compose { .. } => None,
        }
    }

    pub(crate) fn source_owner_identity(&self, handle: Handle) -> Option<u64> {
        self.named(handle)?;
        Some(self.closure.get(handle.node)?.identity)
    }

    pub(crate) fn interface(&self, handle: Handle) -> Option<(&[Ty], &[Ty], &EffSet)> {
        if handle.instance != self.instance { return None }
        self.nodes.get(handle.node)?;
        let witness = self.witnesses.get(handle.witness)?;
        if witness.node != handle.node { return None }
        Some((&witness.input, &witness.output, &witness.effects))
    }

    pub(crate) fn original(&self, handle: Handle) -> Option<(&[Ty], &[Ty], &EffSet)> {
        if handle.instance != self.instance { return None }
        let original = self.nodes.get(handle.node)?.original;
        let witness = self.witnesses.get(original)?;
        if witness.node != handle.node { return None }
        Some((&witness.input, &witness.output, &witness.effects))
    }

    fn charge(&mut self, work: usize) -> Result<(), Refusal> {
        self.work_left = self.work_left.checked_sub(work).ok_or(Refusal::Exhausted)?;
        Ok(())
    }

    pub(crate) fn reserve_attestation(&mut self) -> Result<(), Refusal> {
        self.charge(usize::try_from(super::worker_program_guest::FUEL)
            .map_err(|_| Refusal::Exhausted)?)
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
    pub(crate) fn adapt(&mut self, handle: Handle, destination: &EffSet)
        -> Result<Handle, Refusal> {
        let current = self.witnesses.get(handle.witness).ok_or(Refusal::Invalid)?;
        if handle.instance != self.instance { return Err(Refusal::Invalid) }
        self.nodes.get(handle.node).ok_or(Refusal::Invalid)?;
        if current.node != handle.node { return Err(Refusal::Invalid) }
        if !current.effects.is_subset_of(destination)
            || destination.as_slice().iter()
                .any(|effect| *effect != noble_kernel::contracts::TEST_EMIT) {
            return Err(Refusal::Invalid)
        }
        if self.witnesses.len() >= WITNESS_LIMIT { return Err(Refusal::Exhausted) }
        self.charge(1)?;
        let current = &self.witnesses[handle.witness];
        let witness = Witness { node: handle.node,
            input: current.input.clone(), output: current.output.clone(),
            effects: destination.clone() };
        let position = self.witnesses.len();
        self.witnesses.push(witness);
        Ok(Handle { instance: self.instance, node: handle.node, witness: position })
    }

    /// Compose already checked Programs in execution order, first then second.
    /// This is a bounded immutable recipe node, not invocation or authority.
    pub(crate) fn compose(&mut self, first: Handle, second: Handle)
        -> Result<Handle, Refusal> {
        if first.instance != self.instance || second.instance != self.instance {
            return Err(Refusal::Invalid)
        }
        let left = self.witnesses.get(first.witness).ok_or(Refusal::Invalid)?;
        let right = self.witnesses.get(second.witness).ok_or(Refusal::Invalid)?;
        self.nodes.get(first.node).ok_or(Refusal::Invalid)?;
        self.nodes.get(second.node).ok_or(Refusal::Invalid)?;
        if left.node != first.node || right.node != second.node {
            return Err(Refusal::Invalid)
        }
        if left.output != right.input { return Err(Refusal::Invalid) }
        let depth = self.nodes[first.node].depth.max(self.nodes[second.node].depth)
            .checked_add(1).ok_or(Refusal::Exhausted)?;
        let leaves = self.nodes[first.node].leaves
            .checked_add(self.nodes[second.node].leaves).ok_or(Refusal::Exhausted)?;
        if depth > RECIPE_DEPTH_LIMIT || leaves > RECIPE_LEAF_LIMIT {
            return Err(Refusal::Exhausted);
        }
        if self.nodes.len() >= GRAPH_LIMIT || self.witnesses.len() >= WITNESS_LIMIT {
            return Err(Refusal::Exhausted);
        }
        self.charge(1)?;
        let left = &self.witnesses[first.witness];
        let right = &self.witnesses[second.witness];
        let node = self.nodes.len();
        let witness = Witness { node, input: left.input.clone(), output: right.output.clone(),
            effects: left.effects.union(&right.effects) };
        let position = self.witnesses.len();
        self.witnesses.push(witness);
        self.nodes.push(Node { recipe: Recipe::Compose { first: first.node,
            second: second.node }, original: position, depth, leaves });
        Ok(Handle { instance: self.instance, node, witness: position })
    }

    /// Exact common witness, never a dynamic Any collection.
    pub(crate) fn select(&mut self, handles: &[Handle], index: usize)
        -> Result<Handle, Refusal> {
        if handles.is_empty() || handles.len() > GRAPH_LIMIT || index >= handles.len() {
            return Err(Refusal::Invalid);
        }
        let baseline = self.witnesses.get(handles[0].witness).ok_or(Refusal::Invalid)?;
        if baseline.node != handles[0].node { return Err(Refusal::Invalid) }
        let (input, output, effects) = (&baseline.input, &baseline.output, &baseline.effects);
        for handle in handles {
            if handle.instance != self.instance { return Err(Refusal::Invalid) }
            self.nodes.get(handle.node).ok_or(Refusal::Invalid)?;
            let witness = self.witnesses.get(handle.witness).ok_or(Refusal::Invalid)?;
            if witness.node != handle.node { return Err(Refusal::Invalid) }
            if &witness.input != input || &witness.output != output || &witness.effects != effects {
                return Err(Refusal::Invalid);
            }
        }
        self.charge(handles.len())?;
        Ok(handles[index])
    }

    pub(crate) fn recipe_nodes(&self) -> usize { self.nodes.len() }

    pub(crate) fn recipe_edges(&self, handle: Handle) -> Option<(usize, usize)> {
        if handle.instance != self.instance { return None }
        match &self.nodes.get(handle.node)?.recipe {
            Recipe::Compose { first, second } => Some((*first, *second)),
            Recipe::Named { .. } => None,
        }
    }
}

fn refusal(problem: Refusal) -> (&'static str, &'static str) {
    match problem {
        Refusal::Invalid => ("invalid-input", "worker Program or package differs from checked source"),
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

fn runtime_refusal(problem: anyhow::Error) -> Refusal {
    if problem.downcast_ref::<wasmtime::Trap>() == Some(&wasmtime::Trap::OutOfFuel) {
        Refusal::Exhausted
    } else {
        Refusal::Invalid
    }
}

fn import(arguments: &[std::ffi::OsString]) -> Result<Value, (&'static str, &'static str)> {
    if arguments.len() != 6 || arguments[4] != "--effect-ceiling" {
        return Err(("invalid-input", "usage: noble worker program import PACKAGE --effect-ceiling pure"));
    }
    if arguments[5] != "pure" {
        return Err(("unsupported", "Worker host effect policy unavailable to CLI"));
    }
    let package = super::worker_typed::read_bounded_regular_file(
        std::path::Path::new(&arguments[3]), PACKAGE_BYTES)
        .map_err(|_| ("invalid-input", "package file admission failed"))?;
    let (store, root) = Store::import(&package, RUNTIME_REQUEST_WORK)
        .map_err(refusal)?;
    Ok(json!({"schema":"noble-worker-program-import/v2","stage":"import-admission",
        "outcome":"ready","source_sha256":crate::workflow::intrinsic::sha256(&store.source),
        "selected_graph_blake3":store.selected_graph_blake3(root.node).map_err(refusal)?,
        "dependencies":store.closure.len(),"guest_requests":0,
        "protected_operations":0,"executable_guest_installed":false}))
}

fn guest(arguments: &[std::ffi::OsString]) -> Result<Value, (&'static str, &'static str)> {
    if arguments.len() != 10 || arguments[4] != "--name"
        || arguments[6] != "--operand" || arguments[8] != "--effect-ceiling"
        || arguments[9] != "pure" {
        return Err(("invalid-input",
            "usage: noble worker program guest SOURCE --name NAME --operand I64 --effect-ceiling pure"));
    }
    let name = arguments[5].to_str().ok_or(("invalid-input", "invalid named Program"))?;
    let operand = arguments[7].to_str().ok_or(("invalid-input", "invalid signed operand"))?
        .parse::<i64>().map_err(|_| ("invalid-input", "invalid signed operand"))?;
    let source = super::worker_typed::read_bounded_regular_file(
        std::path::Path::new(&arguments[3]), SOURCE_LIMITS.bytes as usize)
        .map_err(|_| ("invalid-input", "source file admission failed"))?;
    let (mut store, root) = Store::prepare(&source, name, &[Ty::I64], RUNTIME_REQUEST_WORK)
        .map_err(refusal)?;
    guest_report(&mut store, root, operand, "source")
}

fn guest_package(arguments: &[std::ffi::OsString])
    -> Result<Value, (&'static str, &'static str)> {
    if arguments.len() != 8 || arguments[4] != "--operand"
        || arguments[6] != "--effect-ceiling" || arguments[7] != "pure" {
        return Err(("invalid-input",
            "usage: noble worker program guest-package PACKAGE --operand I64 --effect-ceiling pure"));
    }
    let operand = arguments[5].to_str().ok_or(("invalid-input", "invalid signed operand"))?
        .parse::<i64>().map_err(|_| ("invalid-input", "invalid signed operand"))?;
    let package = super::worker_typed::read_bounded_regular_file(
        std::path::Path::new(&arguments[3]), PACKAGE_BYTES)
        .map_err(|_| ("invalid-input", "package file admission failed"))?;
    let (mut store, root) = Store::import(&package, RUNTIME_REQUEST_WORK)
        .map_err(refusal)?;
    guest_report(&mut store, root, operand, "fresh-package")
}

fn guest_report(store: &mut Store, root: Handle, operand: i64, origin: &'static str)
    -> Result<Value, (&'static str, &'static str)> {
    let observed = super::worker_program_guest::execute(store, root, operand, 1)
        .map_err(|failure| match failure {
            super::worker_program_guest::GuestFailure::Exhausted => refusal(Refusal::Exhausted),
            super::worker_program_guest::GuestFailure::Invalid(problem) =>
                match runtime_refusal(problem) {
                    Refusal::Exhausted => refusal(Refusal::Exhausted),
                    _ => ("invalid-input",
                        "source-bound Worker guest installation or execution refused"),
                },
        })?;
    let guest_requests = observed.admits.checked_add(observed.invokes)
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
        if arguments.len() != 8 || arguments[1] != "program"
            || arguments[2] != "build" || arguments[4] != "--name"
            || arguments[6] != "--effect-ceiling" {
            return Err(("invalid-input", "usage: noble worker program build SOURCE --name NAME --effect-ceiling pure"));
        }
        let name = arguments[5].to_str().ok_or(("invalid-input", "invalid named Program"))?;
        if arguments[7] != "pure" {
            return Err(("unsupported", "Worker host effect policy unavailable to CLI"));
        }
        let effects = EffSet::empty();
        let source = super::worker_typed::read_bounded_regular_file(
            std::path::Path::new(&arguments[3]), SOURCE_LIMITS.bytes as usize)
            .map_err(|problem| (problem.context.outcome, "source file admission failed"))?;
        let (mut store, root) = Store::prepare(&source, name, &[Ty::I64], 1_000_000)
            .map_err(|problem| (match problem {
                Refusal::Invalid => "invalid-input",
                Refusal::Unsupported => "unsupported",
                Refusal::Exhausted => "exhausted",
            }, "source-checked named Program rejected"))?;
        let source_sha256 = crate::workflow::intrinsic::sha256(&source);
        let wat_sha256 = crate::workflow::intrinsic::sha256(store.wat());
        let world = store.selected_world().map_err(refusal)?;
        let other = (0..store.recipe_nodes()).filter_map(|index| store.checked_root(index))
            .find(|candidate| *candidate != root && {
                match (store.interface(root), store.interface(*candidate)) {
                    (Some((first_input, first_output, _)),
                     Some((other_input, other_output, _))) =>
                        first_input == other_input && first_output == other_output,
                    _ => false,
                }
            }).ok_or(("unsupported", "source has no second reachable compatible checked Program"))?;
        let adapted = store.adapt(root, &effects)
            .map_err(|_| ("invalid-input", "effect inclusion rejected"))?;
        let second = store.adapt(other, &effects)
            .map_err(|_| ("invalid-input", "second effect inclusion rejected"))?;
        let selected = store.select(&[adapted, second], 1)
            .map_err(|_| ("invalid-input", "typed runtime selection rejected"))?;
        let composed = store.compose(adapted, selected)
            .map_err(|_| ("exhausted", "bounded composition rejected"))?;
        let original = store.original(root).ok_or(("internal-failure", "original witness lost"))?;
        let current = store.interface(composed).ok_or(("internal-failure", "adapted witness lost"))?;
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
        Ok(value) => (value["outcome"].as_str().unwrap_or("internal-failure").to_owned(), value),
        Err((outcome, diagnostic)) => (outcome.to_owned(), json!({
            "schema":"noble-worker-program-builder/v2",
            "stage":"admission",
            "outcome":outcome, "diagnostic":diagnostic, "guest_requests":0,
            "protected_operations":0, "executable_guest_installed":false,
        })),
    };
    let report = super::output::Report { outcome, json: value.to_string() };
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
        let module = session.prepare(
            b"module Worker@1 [ def plus [ 1 + ] export plus ]",
            &[], SOURCE_LIMITS).unwrap();
        let owner = module.checked_program_owner("plus").unwrap();
        let (session, committed) = session.commit(module);
        committed.unwrap();
        let exact = session.prepare(b"Worker@1.plus", &[Ty::I64], SOURCE_LIMITS).unwrap();
        assert!(exact.checked_program_interface(&owner).is_some());
        let extended = session.prepare(b"Worker@1.plus 1 drop",
            &[Ty::I64], SOURCE_LIMITS).unwrap();
        assert!(extended.checked_program_interface(&owner).is_none(),
            "a source expression calling a named Program is not that Program");
    }

    #[test]
    fn text_capture_is_runtime_bound_before_portable_export_or_import() {
        let source = b"module Worker@1 [ def plus [ \"tag\" drop 1 + ] export plus ]";
        let (mut store, root) = Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK)
            .expect("checked Text source");
        let captures = serde_json::to_vec(&store.capture_wire().unwrap()).unwrap();
        let source_sha256 = crate::workflow::intrinsic::sha256(source);
        let capture_sha256 = crate::workflow::intrinsic::sha256(&captures);
        let denied = ExportGrant { destination: "private", source_sha256: &source_sha256,
            capture_sha256: &crate::workflow::intrinsic::sha256(b"[]"), allowed: true };
        assert_eq!(store.export(root, "private", &denied), Err(Refusal::Invalid));
        let grant = ExportGrant { destination: "private", source_sha256: &source_sha256,
            capture_sha256: &capture_sha256, allowed: true };
        let encoded = store.export(root, "private", &grant)
            .expect("authorized runtime Text rebind");
        let mut document: Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(document["captures"][0]["text_utf8"], "tag");
        assert!(document["capture_binding_blake3"].as_str().is_some());
        let (mut imported, selected) = Store::import(&encoded, RUNTIME_REQUEST_WORK)
            .expect("freshly attested Text package");
        assert_eq!(crate::core::worker_program_guest::execute(&mut imported, selected, 41, 1)
            .unwrap().value, 42, "imported Text must rebind before guest invocation");
        document["captures"][0]["text_utf8"] = json!("forged");
        let forged = serde_json::to_vec(&document).unwrap();
        assert!(matches!(Store::import(&forged, RUNTIME_REQUEST_WORK),
            Err(Refusal::Invalid)));
    }

    #[test]
    fn composed_program_package_reconstructs_checked_graph_not_claimed_edges() {
        let source = b"module Worker@1 [ def double [ 2 * ] export plus def plus [ double 1 + ] ]";
        let (mut store, root) = Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK)
            .unwrap();
        let double = store.checked_root(0).unwrap();
        let composed = store.compose(root, double).unwrap();
        let source_sha256 = crate::workflow::intrinsic::sha256(source);
        let capture_sha256 = crate::workflow::intrinsic::sha256(b"[]");
        let grant = ExportGrant { destination: "private", source_sha256: &source_sha256,
            capture_sha256: &capture_sha256, allowed: true };
        let package = store.export(composed, "private", &grant).unwrap();
        let (mut imported, selected) = Store::import(&package, RUNTIME_REQUEST_WORK).unwrap();
        assert_eq!(imported.recipe_edges(selected), Some((root.node,double.node)));
        assert_eq!(crate::core::worker_program_guest::execute(&mut imported, selected, 3, 1)
            .unwrap().value, 14, "freshly imported graph must execute its selected composition");
        let mut claim: Value = serde_json::from_slice(&package).unwrap();
        claim["graph"][2]["first"] = json!(0);
        let forged = serde_json::to_vec(&claim).unwrap();
        assert!(matches!(Store::import(&forged, RUNTIME_REQUEST_WORK),
            Err(Refusal::Invalid)));
    }

    #[test]
    fn checked_named_program_adapts_selects_and_composes_without_changing_its_recipe() {
        let (mut store, original) = Store::prepare(
            b"module Worker@1 [ def minus [ 1 - ] export plus def plus [ minus 1 + ] ]",
            "plus", &[Ty::I64], RUNTIME_REQUEST_WORK,
        ).expect("source-checked named Program");
        store.work_left = 5;
        let other = store.checked_root(0).expect("checked reachable second Program");
        assert_ne!(other.node, original.node);
        assert_ne!(store.named(other).unwrap().checked_recipe,
            store.named(original).unwrap().checked_recipe);
        assert_eq!(store.original(original).unwrap().0, [Ty::I64]);
        assert_eq!(store.interface(original).unwrap().1, [Ty::I64]);
        assert!(store.named(original).is_some());
        assert!(!store.wat().is_empty());
        let selected = store.selected_world().expect("source-bound Program WIT resource");
        let kind = selected.worker_program_kind().expect("selected resource kind");
        assert_eq!(selected.noble_type(noble_contracts::component::Type::Own(kind)),
            Some(Ty::program(vec![Ty::I64],vec![Ty::I64],EffSet::empty())));
        assert!(selected.noble_type(noble_contracts::component::Type::Borrow(kind)).is_none());
        let ordinary = noble_contracts::component::World::parse(
            include_bytes!("../../../noble-wasm/wit/worker-program.wit"),
            "guest", SOURCE_LIMITS).expect("ordinary unselected WIT");
        assert_eq!(ordinary.noble_type(noble_contracts::component::Type::Own(kind)),
            Some(Ty::Resource(kind)), "ordinary WIT resource must stay untrusted");
        assert!(selected.prepare_export("compute",
            b"programs.admit swap programs.invoke", SOURCE_LIMITS).is_ok(),
            "selected source can type the host-admitted Program");
        let destination = EffSet::from_ids(&[TEST_EMIT]);
        let wider = store.adapt(original, &destination).expect("checked effect inclusion");
        assert_eq!(store.recipe_nodes(), 2, "widening must not add a recipe node");
        assert_eq!(store.original(wider).unwrap().2, &EffSet::empty());
        assert_eq!(store.interface(wider).unwrap().2, &destination);
        let mismatched = Handle { node: other.node, ..wider };
        assert!(store.interface(mismatched).is_none());
        assert!(matches!(store.adapt(mismatched, &destination), Err(Refusal::Invalid)));
        assert!(matches!(store.select(&[other, wider], 1), Err(Refusal::Invalid)),
            "heterogeneous effect witnesses cannot enter a typed list");
        let second = store.adapt(other, &destination).expect("second source-checked Program");
        assert_eq!(store.select(&[wider, second], 1), Ok(second));
        let composed = store.compose(wider, second).expect("compatible checked stacks");
        assert_eq!(store.recipe_edges(composed), Some((original.node, other.node)));
        assert_eq!(store.interface(composed).unwrap().2, &destination);
        assert_eq!(store.original(original).unwrap().2, &EffSet::empty());
        assert_eq!(store.recipe_nodes(), 3);
        assert!(matches!(store.adapt(wider, &EffSet::empty()), Err(Refusal::Invalid)),
            "effect ceiling must not narrow");
        assert!(matches!(store.package(wider), Err(Refusal::Unsupported)),
            "widening alone cannot authorize executable export");
        assert!(matches!(store.compose(original, composed), Err(Refusal::Exhausted)),
            "a consumed request budget must not silently reset");
    }

    #[test]
    fn forged_or_unchecked_named_program_cannot_become_a_builder_root() {
        assert!(matches!(Store::prepare(
            b"module Worker@1 [ def plus [ 1 + ] ]", "plus", &[Ty::I64], 1_000_000),
            Err(Refusal::Invalid)));
        assert!(matches!(Store::prepare(
            b"module Worker@1 [ export plus def plus [ 1 + ] ]",
            "plus", &[Ty::Text], 1_000_000), Err(Refusal::Invalid)));
    }

    #[test]
    fn unissued_effect_witness_cannot_grant_package_export() {
        let source = b"module Worker@1 [ export plus def plus [ 1 + ] ]";
        assert!(matches!(Store::prepare(source, "plus", &[Ty::I64], 3),
            Err(Refusal::Exhausted)), "source checks cannot refresh an empty request meter");
        let (mut pure, root) = Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK)
            .expect("pure source");
        pure.work_left = 2;
        let widened = pure.adapt(root, &EffSet::from_ids(&[TEST_EMIT]))
            .expect("type-level effect inclusion does not perform a host request");
        assert_eq!(pure.recipe_nodes(), 1);
        assert_eq!(pure.original(widened).unwrap().2, &EffSet::empty());
        assert!(matches!(pure.package(widened), Err(Refusal::Unsupported)));
        assert!(pure.adapt(root, &EffSet::empty()).is_ok());
        assert!(matches!(pure.adapt(root, &EffSet::empty()), Err(Refusal::Exhausted)));
        assert_eq!(pure.recipe_nodes(), 1);
    }

    #[test]
    fn handle_from_retired_or_other_source_session_cannot_alias_a_new_instance() {
        let source = b"module Worker@1 [ export plus def plus [ 1 + ] ]";
        let (old, stale) = Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK)
            .expect("old source instance");
        assert!(old.interface(stale).is_some());
        drop(old);
        let (mut current, active) = Store::prepare(source, "plus", &[Ty::I64], RUNTIME_REQUEST_WORK)
            .expect("new source instance with same source identity and index");
        assert_ne!(stale, active);
        assert!(current.interface(stale).is_none());
        assert!(current.named(stale).is_none());
        assert!(matches!(current.adapt(stale, &EffSet::empty()), Err(Refusal::Invalid)));
        assert!(matches!(current.compose(stale, active), Err(Refusal::Invalid)));
        assert!(matches!(current.select(&[active, stale], 0), Err(Refusal::Invalid)));
    }

    #[test]
    fn source_bound_package_rechecks_closure_contracts_and_export_authority() {
        let source = b"module Worker@1 [ def minus [ 1 - ] export plus def plus [ minus 1 + ] ]";
        let (mut store, root) = Store::prepare(source, "plus", &[Ty::I64],
            RUNTIME_REQUEST_WORK).expect("checked package source");
        let source_sha256 = crate::workflow::intrinsic::sha256(source);
        let capture_sha256 = crate::workflow::intrinsic::sha256(b"[]");
        let grant = ExportGrant { destination: "local-test", source_sha256: &source_sha256,
            capture_sha256: &capture_sha256, allowed: true };
        let denied = ExportGrant { allowed: false, ..grant };
        assert!(matches!(store.export(root, "local-test", &denied), Err(Refusal::Invalid)));
        assert!(matches!(store.export(root, "other", &grant), Err(Refusal::Invalid)));
        let alternate = (0..store.recipe_nodes()).filter_map(|index| store.checked_root(index))
            .find(|candidate| candidate.node != root.node).expect("checked dependency");
        assert!(matches!(store.export(alternate, "local-test", &grant),
            Err(Refusal::Unsupported)), "a different named root cannot inherit export authority");
        let encoded = store.export(root, "local-test", &grant).expect("explicit export grant");
        let (imported, restored) = Store::import(&encoded, RUNTIME_REQUEST_WORK)
            .expect("fresh locally checked package import");
        assert_eq!(imported.named(restored).unwrap().checked_recipe,
            store.named(root).unwrap().checked_recipe);
        assert!(imported.interface(restored).unwrap().2.is_empty());
        let parsed: Value = serde_json::from_slice(&encoded).expect("test package");
        for (label, altered, expected) in [
            ("missing transitive dependency", {
                let mut value = parsed.clone();
                value["definitions"].as_array_mut().unwrap().remove(0);
                value
            }, Refusal::Invalid),
            ("unissued host contract", {
                let mut value = parsed.clone();
                value["host_contracts"] = json!(["test.emit@1.0.0"]);
                value
            }, Refusal::Invalid),
            ("forged root identity", {
                let mut value = parsed.clone();
                value["root"]["owner"]["identity"] = json!("0");
                value
            }, Refusal::Invalid),
            ("unrelated executable artifact", {
                let mut value = parsed.clone();
                value["artifact"] = json!({"wasm":"claimed-accepted"});
                value
            }, Refusal::Unsupported),
            ("unbound captured Text", {
                let mut value = parsed.clone();
                value["captures"] = json!([{"type":"Text","value":"synthetic-private-text"}]);
                value
            }, Refusal::Invalid),
            ("unsupported revision", {
                let mut value = parsed.clone();
                value["semantic_revision"] = json!("future");
                value
            }, Refusal::Unsupported),
        ] {
            let bytes = serde_json::to_vec(&altered).expect(label);
            assert!(matches!(Store::import(&bytes, RUNTIME_REQUEST_WORK),
                Err(refusal) if refusal == expected), "{label}");
        }
    }
}
