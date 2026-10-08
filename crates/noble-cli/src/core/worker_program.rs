//! Opt-in, source-bound Worker Program builders. The compiled named Program is
//! the only root: handles are indices into this invocation's private graph,
//! never package-supplied executable identities or host-operation grants.

use noble_kernel::types::{EffSet, Ty};
use noble_wasm::source::{NamedTargetMetadata, Prepared as Compiled};

const SOURCE_LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536, nodes: 4_096, depth: 32, work: 3_000_000,
};
const GRAPH_LIMIT: usize = 4_096;
const WITNESS_LIMIT: usize = 4_096;

/// The host, never package source, selects this exact operation contract.
#[derive(Clone, Copy)]
pub(crate) enum HostContract {
    TestEmitV1,
}

impl HostContract {
    fn effects(self) -> EffSet {
        match self {
            Self::TestEmitV1 => EffSet::from_ids(&[noble_kernel::contracts::TEST_EMIT]),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Refusal {
    Invalid,
    Unsupported,
    Exhausted,
}

struct Node {
    recipe: Recipe,
    original: usize,
}

enum Recipe {
    Named { program_index: usize },
    Compose { first: usize, second: usize },
}

struct Witness {
    input: Vec<Ty>,
    output: Vec<Ty>,
    effects: EffSet,
}

/// A Program's graph index and its *adapted* interface witness are distinct.
/// The graph node retains its original, source-derived interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Handle {
    node: usize,
    witness: usize,
}

pub(crate) struct Store {
    compiled: Compiled,
    selected_effects: EffSet,
    nodes: Vec<Node>,
    witnesses: Vec<Witness>,
    work_left: usize,
}

impl Store {
    /// Check the named root and independently emit the actual selected Wasm.
    /// No runtime builder below calls a source compiler or executes guest code.
    pub(crate) fn prepare(source: &[u8], name: &str, input: &[Ty],
                          contract: Option<HostContract>, work: usize)
        -> Result<(Self, Handle), Refusal> {
        if source.len() > SOURCE_LIMITS.bytes as usize || input.len() > 64 || name.len() > 128
            || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') {
            return Err(Refusal::Exhausted);
        }
        let session = noble_contracts::source::ModuleSession::new(&[])
            .map_err(|_| Refusal::Invalid)?;
        let module = session.prepare(source, &[], SOURCE_LIMITS).map_err(|_| Refusal::Invalid)?;
        if module.kind() != noble_contracts::source::ModuleKind::Module
            || module.proof_obligations().is_some() || !module.bindings().is_empty() {
            return Err(Refusal::Unsupported);
        }
        let owner = module.checked_program_owner(name).ok_or(Refusal::Invalid)?;
        let closure = module.checked_program_closure(name).ok_or(Refusal::Invalid)?;
        if closure.is_empty() { return Err(Refusal::Invalid) }
        let (_, module_name, version) = module.resolved_module().ok_or(Refusal::Invalid)?;
        let (session, committed) = session.commit(module);
        committed.map_err(|_| Refusal::Invalid)?;
        let invocation = format!("{module_name}@{version}.{name}");
        let prepared = session.prepare(invocation.as_bytes(), input, SOURCE_LIMITS)
            .map_err(|_| Refusal::Invalid)?;
        let submission = prepared.submission().ok_or(Refusal::Invalid)?;
        if submission.request.expected.stack_in != input
            || !submission.request.expected.allowed_effects.is_empty()
            || submission.definitions.iter().filter(|definition|
                definition.identity == owner.definition_identity()
                && submission.environment.definition_owners.get(definition.definition.0 as usize)
                    == Some(&Some(owner.module_identity()))).count() != 1 {
            return Err(Refusal::Invalid);
        }
        let compiled = noble_wasm::source::Compiler::new_worker().prepare(submission)
            .map_err(|_| Refusal::Invalid)?;
        let mut nodes = Vec::with_capacity(closure.len());
        let mut witnesses = Vec::with_capacity(closure.len());
        let mut root = None;
        for dependency in &closure {
            let named = compiled.named_targets().iter().filter(|target|
                target.definition.0 == dependency.definition_slot()
                && target.definition_identity == dependency.definition_identity()
                && submission.environment.definition_owners.get(target.definition.0 as usize)
                    == Some(&Some(owner.module_identity()))).collect::<Vec<_>>();
            let [target] = named.as_slice() else { return Err(Refusal::Invalid) };
            if target.stack_in.len() > 64 || target.stack_out.len() > 64
                || !target.effects.is_empty() || target.checked_recipe.is_empty() {
                return Err(Refusal::Invalid);
            }
            let index = nodes.len();
            nodes.push(Node { recipe: Recipe::Named { program_index: target.program_index },
                original: index });
            witnesses.push(Witness { input: target.stack_in.clone(),
                output: target.stack_out.clone(), effects: target.effects.clone() });
            if dependency.definition_slot() == owner.definition_slot() {
                if target.stack_in.as_slice() != input
                    || target.stack_out.as_slice() != prepared.output() {
                    return Err(Refusal::Invalid);
                }
                root = Some(Handle { node: index, witness: index });
            }
        }
        let root = root.ok_or(Refusal::Invalid)?;
        let store = Self { selected_effects: contract.map_or_else(EffSet::empty, HostContract::effects),
            nodes, witnesses, compiled, work_left: work };
        Ok((store, root))
    }

    pub(crate) fn wat(&self) -> &[u8] { self.compiled.wat() }

    /// A second candidate must be an independently checked reachable named
    /// Program in the same compiled source closure, not a claimed package row.
    pub(crate) fn checked_root(&self, index: usize) -> Option<Handle> {
        matches!(self.nodes.get(index)?.recipe, Recipe::Named { .. })
            .then_some(Handle { node: index, witness: self.nodes[index].original })
    }

    pub(crate) fn named(&self, handle: Handle) -> Option<&NamedTargetMetadata> {
        match &self.nodes.get(handle.node)?.recipe {
            Recipe::Named { program_index } => self.compiled.named_targets().iter()
                .find(|target| target.program_index == *program_index),
            Recipe::Compose { .. } => None,
        }
    }

    pub(crate) fn interface(&self, handle: Handle) -> Option<(&[Ty], &[Ty], &EffSet)> {
        self.nodes.get(handle.node)?;
        let witness = self.witnesses.get(handle.witness)?;
        Some((&witness.input, &witness.output, &witness.effects))
    }

    pub(crate) fn original(&self, handle: Handle) -> Option<(&[Ty], &[Ty], &EffSet)> {
        let original = self.nodes.get(handle.node)?.original;
        let witness = self.witnesses.get(original)?;
        Some((&witness.input, &witness.output, &witness.effects))
    }

    fn charge(&mut self, work: usize) -> Result<(), Refusal> {
        self.work_left = self.work_left.checked_sub(work).ok_or(Refusal::Exhausted)?;
        Ok(())
    }

    /// Explicit effect inclusion changes only a witness, never a recipe node.
    pub(crate) fn adapt(&mut self, handle: Handle, destination: &EffSet)
        -> Result<Handle, Refusal> {
        let current = self.witnesses.get(handle.witness).ok_or(Refusal::Invalid)?;
        self.nodes.get(handle.node).ok_or(Refusal::Invalid)?;
        if !current.effects.is_subset_of(destination)
            || !destination.is_subset_of(&self.selected_effects) {
            return Err(Refusal::Invalid)
        }
        if self.witnesses.len() >= WITNESS_LIMIT { return Err(Refusal::Exhausted) }
        self.charge(1)?;
        let current = &self.witnesses[handle.witness];
        let witness = Witness { input: current.input.clone(), output: current.output.clone(),
            effects: destination.clone() };
        let position = self.witnesses.len();
        self.witnesses.push(witness);
        Ok(Handle { node: handle.node, witness: position })
    }

    /// Compose already checked Programs in execution order, first then second.
    /// This is a bounded immutable recipe node, not invocation or authority.
    pub(crate) fn compose(&mut self, first: Handle, second: Handle)
        -> Result<Handle, Refusal> {
        let left = self.witnesses.get(first.witness).ok_or(Refusal::Invalid)?;
        let right = self.witnesses.get(second.witness).ok_or(Refusal::Invalid)?;
        self.nodes.get(first.node).ok_or(Refusal::Invalid)?;
        self.nodes.get(second.node).ok_or(Refusal::Invalid)?;
        if left.output != right.input { return Err(Refusal::Invalid) }
        if self.nodes.len() >= GRAPH_LIMIT || self.witnesses.len() >= WITNESS_LIMIT {
            return Err(Refusal::Exhausted);
        }
        self.charge(1)?;
        let left = &self.witnesses[first.witness];
        let right = &self.witnesses[second.witness];
        let witness = Witness { input: left.input.clone(), output: right.output.clone(),
            effects: left.effects.union(&right.effects) };
        let position = self.witnesses.len();
        let node = self.nodes.len();
        self.witnesses.push(witness);
        self.nodes.push(Node { recipe: Recipe::Compose { first: first.node,
            second: second.node }, original: position });
        Ok(Handle { node, witness: position })
    }

    /// Exact common witness, never a dynamic Any collection.
    pub(crate) fn select(&mut self, handles: &[Handle], index: usize)
        -> Result<Handle, Refusal> {
        if handles.is_empty() || handles.len() > GRAPH_LIMIT || index >= handles.len() {
            return Err(Refusal::Invalid);
        }
        let baseline = self.witnesses.get(handles[0].witness).ok_or(Refusal::Invalid)?;
        let (input, output, effects) = (&baseline.input, &baseline.output, &baseline.effects);
        for handle in handles {
            self.nodes.get(handle.node).ok_or(Refusal::Invalid)?;
            let witness = self.witnesses.get(handle.witness).ok_or(Refusal::Invalid)?;
            if &witness.input != input || &witness.output != output || &witness.effects != effects {
                return Err(Refusal::Invalid);
            }
        }
        self.charge(handles.len())?;
        Ok(handles[index])
    }

    pub(crate) fn recipe_nodes(&self) -> usize { self.nodes.len() }

    pub(crate) fn recipe_edges(&self, handle: Handle) -> Option<(usize, usize)> {
        match &self.nodes.get(handle.node)?.recipe {
            Recipe::Compose { first, second } => Some((*first, *second)),
            Recipe::Named { .. } => None,
        }
    }
}

/// Explicit source-selected local builder operation. This endpoint does not
/// claim guest execution, portable import, export authority or host effects.
pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    use serde_json::json;
    let result = (|| {
        if arguments.len() != 8 || arguments[1] != "program"
            || arguments[2] != "build" || arguments[4] != "--name"
            || arguments[6] != "--effect-ceiling" {
            return Err(("invalid-input", "usage: noble worker program build SOURCE --name NAME --effect-ceiling pure|test.emit@1.0.0"));
        }
        let name = arguments[5].to_str().ok_or(("invalid-input", "invalid named Program"))?;
        let contract = match arguments[7].to_str() {
            Some("pure") => None,
            Some("test.emit@1.0.0") => Some(HostContract::TestEmitV1),
            _ => return Err(("unsupported", "unsupported effect ceiling")),
        };
        let effects = contract.map_or_else(EffSet::empty, HostContract::effects);
        let source = super::worker_typed::read_bounded_regular_file(
            std::path::Path::new(&arguments[3]), SOURCE_LIMITS.bytes as usize)
            .map_err(|problem| (problem.context.outcome, "source file admission failed"))?;
        let (mut store, root) = Store::prepare(&source, name, &[Ty::I64], contract, 64)
            .map_err(|problem| (match problem {
                Refusal::Invalid => "invalid-input",
                Refusal::Unsupported => "unsupported",
                Refusal::Exhausted => "exhausted",
            }, "source-checked named Program rejected"))?;
        let source_sha256 = crate::workflow::intrinsic::sha256(&source);
        let wat_sha256 = crate::workflow::intrinsic::sha256(store.wat());
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
            "source_sha256":source_sha256,
            "wat_sha256":wat_sha256, "source_compiler_calls_after_admission":0,
            "guest_requests":0, "protected_operations":0, "recipe_nodes":store.recipe_nodes(),
            "host_contract":contract.map(|_| "test.emit@1.0.0"),
            "checked_programs":[store.named(root).map(|target| target.program_index),
                store.named(other).map(|target| target.program_index)],
            "original_effects":original.2.as_slice().iter().map(|effect| effect.0).collect::<Vec<_>>(),
            "adapted_effects":current.2.as_slice().iter().map(|effect| effect.0).collect::<Vec<_>>(),
            "executable_guest_installed":false,
        }))
    })();
    let (outcome, value) = match result {
        Ok(value) => ("normal", value),
        Err((outcome, diagnostic)) => (outcome, json!({
            "schema":"noble-worker-program-builder/v2", "stage":"admission",
            "outcome":outcome, "diagnostic":diagnostic, "guest_requests":0,
            "protected_operations":0, "executable_guest_installed":false,
        })),
    };
    let report = super::output::Report { outcome: outcome.to_owned(), json: value.to_string() };
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
    fn checked_named_program_adapts_selects_and_composes_without_changing_its_recipe() {
        let (mut store, original) = Store::prepare(
            b"module Worker@1 [ def minus [ 1 - ] export plus def plus [ minus 1 + ] ]",
            "plus", &[Ty::I64], Some(HostContract::TestEmitV1), 5,
        ).expect("source-checked named Program");
        let other = store.checked_root(0).expect("checked reachable second Program");
        assert_ne!(other.node, original.node);
        assert_ne!(store.named(other).unwrap().checked_recipe,
            store.named(original).unwrap().checked_recipe);
        assert_eq!(store.original(original).unwrap().0, [Ty::I64]);
        assert_eq!(store.interface(original).unwrap().1, [Ty::I64]);
        assert!(store.named(original).is_some());
        assert!(!store.wat().is_empty());
        let destination = EffSet::from_ids(&[TEST_EMIT]);
        let wider = store.adapt(original, &destination).expect("checked effect inclusion");
        assert_eq!(store.recipe_nodes(), 2, "widening must not add a recipe node");
        assert_eq!(store.original(wider).unwrap().2, &EffSet::empty());
        assert_eq!(store.interface(wider).unwrap().2, &destination);
        assert!(matches!(store.select(&[other, wider], 1), Err(Refusal::Invalid)),
            "heterogeneous interface must not enter a typed list");
        let second = store.adapt(other, &destination).expect("second source-checked Program");
        assert_eq!(store.select(&[wider, second], 1), Ok(second));
        let composed = store.compose(wider, second).expect("compatible checked stacks");
        assert_eq!(store.recipe_edges(composed), Some((original.node, other.node)));
        assert_eq!(store.interface(composed).unwrap().2, &destination);
        assert_eq!(store.original(original).unwrap().2, &EffSet::empty());
        assert_eq!(store.recipe_nodes(), 3);
        assert!(matches!(store.adapt(wider, &EffSet::empty()), Err(Refusal::Invalid)),
            "effect ceiling must not narrow");
        assert!(matches!(store.compose(original, composed), Err(Refusal::Exhausted)),
            "a consumed request budget must not silently reset");
    }

    #[test]
    fn forged_or_unchecked_named_program_cannot_become_a_builder_root() {
        assert!(matches!(Store::prepare(
            b"module Worker@1 [ def plus [ 1 + ] ]", "plus", &[Ty::I64], None, 10),
            Err(Refusal::Invalid)));
        assert!(matches!(Store::prepare(
            b"module Worker@1 [ export plus def plus [ 1 + ] ]",
            "plus", &[Ty::Text], None, 10), Err(Refusal::Invalid)));
    }

    #[test]
    fn selected_host_ceiling_and_request_work_are_independent_of_source_claims() {
        let source = b"module Worker@1 [ export plus def plus [ 1 + ] ]";
        let (mut pure, root) = Store::prepare(source, "plus", &[Ty::I64], None, 1)
            .expect("pure source");
        assert!(matches!(pure.adapt(root, &EffSet::from_ids(&[TEST_EMIT])),
            Err(Refusal::Invalid)), "source must not select an absent host contract");
        assert_eq!(pure.recipe_nodes(), 1);
        assert!(pure.adapt(root, &EffSet::empty()).is_ok());
        assert!(matches!(pure.adapt(root, &EffSet::empty()), Err(Refusal::Exhausted)));
        assert_eq!(pure.recipe_nodes(), 1);
    }
}
