//! Exact host-side reuse of checked calculator modules (canonical epochs).
//!
//! A calculator guest module is a pure function of its key: the source check
//! (`ModuleSession::prepare`) depends only on the namespace content, the exact
//! source bytes, the exact input stack types and the fixed `LIMITS`; a
//! Program-free backend preparation depends on its submission and the
//! committed compiler only through `noble_wasm::source::CanonicalEpoch`. The
//! key is therefore (namespace recipe, source bytes, input stack types,
//! canonical epoch), every component compared in full; the hash map only
//! selects a bucket and never decides equality. A hit replays the recorded
//! checked output types and the recorded backend artifact through
//! `Compiler::reuse_program_free`, which refuses any other compiler state.
//! The guest still executes every module.
//!
//! The namespace component is the process-constant calculator module recipe:
//! every `Session::new` builds its namespace from exactly that recipe, and a
//! calculator session commits only checked Expression modules afterwards,
//! which never change namespace content (only its staleness generations).

use noble_kernel::types::Ty;

/// Upper bound on the accounted bytes this cache retains (source, types,
/// WAT and prospective compiler state of every entry).
pub(super) const HOST_CACHE_BYTES: usize = 96 * 1024 * 1024;

/// Conservative accounting for one stored type node, its box and slack.
const TYPE_NODE_BYTES: usize = 2 * core::mem::size_of::<Ty>() + 16;
const ENTRY_BYTES: usize = 128;

pub(super) type Namespace = &'static [(&'static str, &'static [u8])];

struct Entry {
    epoch: noble_wasm::source::CanonicalEpoch,
    input: Vec<Ty>,
    output: Vec<Ty>,
    artifact: noble_wasm::source::Reusable,
    bytes: usize,
    used: u64,
}

pub(super) struct ModuleCache {
    namespace: Namespace,
    entries: std::collections::HashMap<Vec<u8>, Vec<Entry>>,
    bytes: usize,
    limit: usize,
    tick: u64,
}

/// A recorded module: its checked output stack and backend artifact.
pub(super) struct Hit<'a> {
    pub(super) output: &'a [Ty],
    pub(super) artifact: &'a noble_wasm::source::Reusable,
}

impl ModuleCache {
    pub(super) fn new(namespace: Namespace, limit: usize) -> Self {
        Self {
            namespace,
            entries: std::collections::HashMap::new(),
            bytes: 0,
            limit,
            tick: 0,
        }
    }

    /// Accounted bytes currently retained (never above the bound).
    pub(super) const fn bytes(&self) -> usize {
        self.bytes
    }

    pub(super) fn get(
        &mut self,
        namespace: Namespace,
        source: &[u8],
        input: &[Ty],
        epoch: noble_wasm::source::CanonicalEpoch,
    ) -> Option<Hit<'_>> {
        if !same_namespace(self.namespace, namespace) {
            return None;
        }
        self.tick += 1;
        let tick = self.tick;
        let entry = self
            .entries
            .get_mut(source)?
            .iter_mut()
            .find(|entry| entry.epoch == epoch && entry.input.as_slice() == input)?;
        entry.used = tick;
        Some(Hit {
            output: &entry.output,
            artifact: &entry.artifact,
        })
    }

    /// Record one checked, successfully prepared Program-free module. An
    /// entry larger than the bound is not retained; otherwise the least
    /// recently used entries are evicted until the bound holds.
    pub(super) fn insert(
        &mut self,
        namespace: Namespace,
        source: &[u8],
        input: &[Ty],
        output: &[Ty],
        artifact: noble_wasm::source::Reusable,
    ) {
        if !same_namespace(self.namespace, namespace) {
            return;
        }
        let epoch = artifact.epoch();
        let bytes = ENTRY_BYTES
            .saturating_add(source.len().saturating_mul(2))
            .saturating_add(stack_bytes(input))
            .saturating_add(stack_bytes(output))
            .saturating_add(artifact.retained_bytes());
        if bytes > self.limit {
            return;
        }
        let bucket = self.entries.entry(source.to_vec()).or_default();
        if let Some(position) = bucket
            .iter()
            .position(|entry| entry.epoch == epoch && entry.input.as_slice() == input)
        {
            let replaced = bucket.swap_remove(position);
            self.bytes -= replaced.bytes;
        }
        self.tick += 1;
        bucket.push(Entry {
            epoch,
            input: input.to_vec(),
            output: output.to_vec(),
            artifact,
            bytes,
            used: self.tick,
        });
        self.bytes += bytes;
        while self.bytes > self.limit {
            self.evict_oldest();
        }
    }

    fn evict_oldest(&mut self) {
        let Some((source, position)) = self
            .entries
            .iter()
            .flat_map(|(source, bucket)| {
                bucket
                    .iter()
                    .enumerate()
                    .map(move |(position, entry)| (entry.used, source, position))
            })
            .min_by_key(|(used, _, _)| *used)
            .map(|(_, source, position)| (source.clone(), position))
        else {
            return;
        };
        if let Some(bucket) = self.entries.get_mut(&source) {
            let evicted = bucket.swap_remove(position);
            self.bytes -= evicted.bytes;
            if bucket.is_empty() {
                self.entries.remove(&source);
            }
        }
    }
}

fn same_namespace(left: Namespace, right: Namespace) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|((left_name, left_source), (right_name, right_source))| {
                left_name == right_name && left_source == right_source
            })
}

fn stack_bytes(stack: &[Ty]) -> usize {
    stack.iter().fold(0usize, |total, ty| {
        total.saturating_add(nodes(ty).saturating_mul(TYPE_NODE_BYTES))
    })
}

fn nodes(ty: &Ty) -> usize {
    let shape = |shape: &noble_kernel::types::NominalShape| match shape {
        noble_kernel::types::NominalShape::Opaque(value) => nodes(value),
        noble_kernel::types::NominalShape::Variant(left, right) => {
            nodes(left).saturating_add(nodes(right))
        }
    };
    let children = match ty {
        Ty::Pair(left, right) | Ty::Sum(left, right) => nodes(left).saturating_add(nodes(right)),
        Ty::List(item) => nodes(item),
        Ty::Program(input, output, _) | Ty::LiveRef(input, output, _) => input
            .iter()
            .chain(output.iter())
            .fold(0usize, |total, ty| total.saturating_add(nodes(ty))),
        Ty::Nominal(_, descriptor) => shape(descriptor),
        Ty::GenericNominal(_, arguments, descriptor) => nodes(&arguments[0])
            .saturating_add(nodes(&arguments[1]))
            .saturating_add(shape(descriptor)),
        Ty::Unit
        | Ty::Bool
        | Ty::I64
        | Ty::Text
        | Ty::Syntax
        | Ty::Contract
        | Ty::Evidence
        | Ty::Certified
        | Ty::Resource(_) => 0,
    };
    children.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::{ModuleCache, Namespace};
    use noble_kernel::types::Ty;

    const NAMESPACE: Namespace = &[("calc@1", b"module calc@1 [ ]")];
    const OTHER: Namespace = &[("calc@1", b"module calc@1 [ export x def x [ 1 ] ]")];

    fn limits() -> noble_contracts::Limits {
        noble_contracts::Limits {
            bytes: 65_536,
            nodes: 65_536,
            depth: 64,
            work: 200_000_000,
        }
    }

    /// A real checked Program-free preparation of `source` at `compiler`.
    fn artifact(
        compiler: &noble_wasm::source::Compiler,
        source: &str,
        inputs: &[Ty],
    ) -> (Vec<Ty>, noble_wasm::source::Reusable) {
        let session = noble_contracts::source::Session::new_calculator();
        let checked = session
            .prepare(source.as_bytes(), inputs, limits())
            .unwrap_or_else(|error| panic!("{source}: {error:?}"));
        let prepared = compiler
            .prepare_program_free(checked.submission().expect("submission"))
            .unwrap_or_else(|_| panic!("{source}: compiler refused"));
        (
            checked.output().to_vec(),
            prepared
                .reusable()
                .expect("Program-free calculator preparation"),
        )
    }

    #[test]
    fn exact_key_hits_and_every_component_change_misses() {
        let compiler = noble_wasm::source::Compiler::new_calculator();
        let epoch = compiler.canonical_epoch().expect("calculator epoch");
        let (output, recorded) = artifact(&compiler, "1 drop", &[Ty::I64]);
        let wat = recorded.wat().to_vec();
        let mut cache = ModuleCache::new(NAMESPACE, usize::MAX);
        cache.insert(NAMESPACE, b"1 drop", &[Ty::I64], &output, recorded);
        let hit = cache
            .get(NAMESPACE, b"1 drop", &[Ty::I64], epoch)
            .expect("exact key hits");
        assert_eq!(hit.output, output.as_slice());
        assert_eq!(hit.artifact.wat(), wat.as_slice());
        // Source bytes, input types, namespace and canonical epoch each miss.
        assert!(cache
            .get(NAMESPACE, b"1  drop", &[Ty::I64], epoch)
            .is_none());
        assert!(cache
            .get(NAMESPACE, b"1 drop", &[Ty::Bool], epoch)
            .is_none());
        assert!(cache.get(NAMESPACE, b"1 drop", &[], epoch).is_none());
        assert!(cache.get(OTHER, b"1 drop", &[Ty::I64], epoch).is_none());
        let mut advanced = noble_wasm::source::Compiler::new_calculator();
        let session = noble_contracts::source::Session::new_calculator();
        let literal = session
            .prepare(b"\"a\" drop", &[], limits())
            .expect("literal source");
        let prepared = advanced
            .prepare_program_free(literal.submission().expect("submission"))
            .unwrap_or_else(|_| panic!("literal refused"));
        assert!(advanced.commit(prepared).is_ok());
        let later = advanced.canonical_epoch().expect("calculator epoch");
        assert_ne!(later, epoch);
        assert!(cache.get(NAMESPACE, b"1 drop", &[Ty::I64], later).is_none());
        // Inserting under a different namespace retains nothing.
        let (output, other) = artifact(&compiler, "2 drop", &[Ty::I64]);
        cache.insert(OTHER, b"2 drop", &[Ty::I64], &output, other);
        assert!(cache.get(OTHER, b"2 drop", &[Ty::I64], epoch).is_none());
    }

    #[test]
    fn retained_bytes_stay_within_the_bound_evicting_least_recent() {
        let compiler = noble_wasm::source::Compiler::new_calculator();
        let epoch = compiler.canonical_epoch().expect("calculator epoch");
        let sources = ["1 drop", "2 drop", "3 drop"];
        let mut sizes = Vec::new();
        for source in sources {
            let mut probe = ModuleCache::new(NAMESPACE, usize::MAX);
            let (output, recorded) = artifact(&compiler, source, &[Ty::I64]);
            probe.insert(NAMESPACE, source.as_bytes(), &[Ty::I64], &output, recorded);
            sizes.push(probe.bytes());
        }
        // Equal-sized entries; room for exactly two of them.
        assert!(sizes.iter().all(|size| *size == sizes[0]));
        let limit = 2 * sizes[0];
        let mut cache = ModuleCache::new(NAMESPACE, limit);
        for source in &sources[..2] {
            let (output, recorded) = artifact(&compiler, source, &[Ty::I64]);
            cache.insert(NAMESPACE, source.as_bytes(), &[Ty::I64], &output, recorded);
        }
        assert_eq!(cache.bytes(), limit);
        // Touch the first entry so the second is least recently used.
        assert!(cache.get(NAMESPACE, b"1 drop", &[Ty::I64], epoch).is_some());
        let (output, recorded) = artifact(&compiler, sources[2], &[Ty::I64]);
        cache.insert(NAMESPACE, b"3 drop", &[Ty::I64], &output, recorded);
        assert!(cache.bytes() <= limit);
        assert!(cache.get(NAMESPACE, b"2 drop", &[Ty::I64], epoch).is_none());
        assert!(cache.get(NAMESPACE, b"1 drop", &[Ty::I64], epoch).is_some());
        assert!(cache.get(NAMESPACE, b"3 drop", &[Ty::I64], epoch).is_some());
        // An entry larger than the whole bound is never retained.
        let mut tiny = ModuleCache::new(NAMESPACE, sizes[0] - 1);
        let (output, recorded) = artifact(&compiler, sources[0], &[Ty::I64]);
        tiny.insert(NAMESPACE, b"1 drop", &[Ty::I64], &output, recorded);
        assert_eq!(tiny.bytes(), 0);
        assert!(tiny.get(NAMESPACE, b"1 drop", &[Ty::I64], epoch).is_none());
    }
}
