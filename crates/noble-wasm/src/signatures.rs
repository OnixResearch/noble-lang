mod walk;

pub(crate) const SOURCE_KEY_LIMIT: usize = 4096;
// Opt-in calculator retains at most 8,192 records (128 KiB of accounted
// front-coding metadata, plus bounded Vec records) and 4 MiB of expanded
// canonical bytes. The separate 16,384 table, 4 MiB complete WAT, and
// measured 2b-work gates can still refuse the prospective preparation.
// Both its key count and exact bytes are checked before publishing an ID.
pub(crate) const CALCULATOR_SOURCE_KEY_LIMIT: usize = 8192;
const PREFIX_BLOCK: usize = 32;
const PREFIX_RECORD_BYTES: usize = 16;
// Each calculator module emits only the expanded keys interned since its
// compiler's published base, and the trusted host retains every published key
// once. Bound that host table, and any single module's delta, by the actual
// canonical bytes rather than by their incidental front-coded representation.
const CALCULATOR_EXPANDED_KEY_LIMIT: usize = 4 * crate::OUTPUT_BYTE_LIMIT;

// Calculator keys retain exact canonical bytes as bounded front-coded blocks.
// The first key of each block is independent; decoding never follows more
// than 31 predecessors. Hashes only skip unequal keys: a collision must
// still compare every expanded byte before sharing an identity.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PrefixKey {
    prefix: u32,
    length_bytes: u32,
    hash: u64,
    suffix: alloc::vec::Vec<u8>,
}

fn fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    hash
}

/// Comparisons charged for one ordered-index probe over `entries` distinct
/// (fingerprint, length_bytes) classes: a bound on the search height, not the
/// number of records the session has retained.
const fn index_probe_cost(entries: usize) -> usize {
    (usize::BITS - entries.leading_zeros()) as usize + 1
}

/// The candidate index key of one (fingerprint, length_bytes) class: the
/// fingerprint fills the high 64 bits and the length the low 32, so integer
/// order is the lexicographic (fingerprint, length_bytes) order and equal
/// keys are exactly equal pairs.
const fn candidate_key(hash: u64, length_bytes: u32) -> u128 {
    ((hash as u128) << 32) | (length_bytes as u128)
}

/// The budget an interning probe charges, once per comparison or decoded
/// record it actually visits, in visit order.
pub(crate) trait Meter {
    /// Charge `count` visited entries, refusing once the budget is spent.
    fn visit(&mut self, count: usize) -> Result<(), crate::Diagnostic>;
}

/// Ordinary interning is unmetered: every visit is accepted.
pub(crate) struct Unmetered;

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-wasm-maintainers; the unmetered visit accepts every count and changes no state; the receiver is mutable only because the Meter interface charges stateful budgets."
)]
impl Meter for Unmetered {
    fn visit(&mut self, _count: usize) -> Result<(), crate::Diagnostic> {
        Ok(())
    }
}

/// IDs intern complete ordered structural stacks, never just stack heights.
/// The canonical grammar is injective and contains no caller-provided text.
/// Calculator uses compact printable tokens so a long-lived selected session
/// does not repeat verbose type names in every distinct structural key.
#[derive(Clone, Debug)]
pub(crate) struct Pool {
    pub(crate) keys: alloc::vec::Vec<alloc::vec::Vec<u8>>,
    prefix_keys: alloc::vec::Vec<PrefixKey>,
    // Compact records by (fingerprint, length_bytes) class, keyed by
    // `candidate_key`, each list in ascending ID order. A lookup visits only
    // records that could equal the key, so its metered cost does not grow
    // with the session's accumulated pool.
    candidates: alloc::collections::BTreeMap<u128, alloc::vec::Vec<u32>>,
    bytes: usize,
    expanded_bytes: usize,
    extended: bool,
    compact: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct Interface {
    pub(crate) input: u32,
    pub(crate) output: u32,
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; interning updates only a fresh compiler-private pool and emission only its fresh output sink; complete caller-owned type stacks and effects are immutable."
)]
impl Pool {
    pub(crate) fn new() -> Self {
        Self {
            keys: alloc::vec::Vec::with_capacity(16),
            prefix_keys: alloc::vec::Vec::new(),
            candidates: alloc::collections::BTreeMap::new(),
            bytes: 0,
            expanded_bytes: 0,
            extended: false,
            compact: false,
        }
    }

    pub(crate) fn source() -> Self {
        Self {
            keys: alloc::vec::Vec::with_capacity(16),
            prefix_keys: alloc::vec::Vec::new(),
            candidates: alloc::collections::BTreeMap::new(),
            bytes: 0,
            expanded_bytes: 0,
            extended: true,
            compact: false,
        }
    }

    pub(crate) fn select_calculator(&mut self) {
        self.compact = true;
    }

    /// A conservative heap accounting of this pool for byte-bounded caches:
    /// every stored byte plus a fixed per-record allowance for vector, map
    /// and allocation overhead. It never under-counts the stored key bytes.
    pub(crate) fn retained_bytes(&self) -> usize {
        const RECORD: usize = 64;
        let mut keys = 0usize;
        for key in &self.keys {
            keys = keys.saturating_add(key.len()).saturating_add(RECORD);
        }
        let mut prefixes = 0usize;
        for key in &self.prefix_keys {
            prefixes = prefixes
                .saturating_add(key.suffix.len())
                .saturating_add(RECORD);
        }
        let mut candidates = 0usize;
        for ids in self.candidates.values() {
            candidates = candidates
                .saturating_add(ids.len().saturating_mul(4))
                .saturating_add(RECORD);
        }
        keys.saturating_add(prefixes).saturating_add(candidates)
    }

    pub(crate) const fn len(&self) -> usize {
        if self.compact {
            self.prefix_keys.len()
        } else {
            self.keys.len()
        }
    }

    pub(crate) fn key_len(&self, index: usize) -> usize {
        if self.compact {
            self.prefix_keys[index].length_bytes as usize
        } else {
            self.keys[index].len()
        }
    }

    pub(crate) fn same_key(&self, other: &Self, index: usize) -> bool {
        if self.compact && other.compact {
            self.prefix_keys[index] == other.prefix_keys[index]
        } else {
            self.keys[index] == other.keys[index]
        }
    }

    /// Decode the canonical keys with IDs `first..len()` for selected WAT
    /// emission. Earlier IDs were published by previously committed modules,
    /// so the decode visits at most 31 predecessors plus the requested delta,
    /// never the whole retained session. Both the decode and the owned output
    /// copy charge the live preparation.
    pub(crate) fn expanded_keys(
        &self,
        first: usize,
        meter: &mut impl Meter,
    ) -> Result<alloc::vec::Vec<alloc::vec::Vec<u8>>, crate::Diagnostic> {
        let Some(delta) = self.prefix_keys.get(first..) else {
            return Err(crate::Diagnostic::Defective);
        };
        // These exact strings each appear in the selected WAT. Refuse an
        // impossible module before decompressing an adversarially shared
        // prefix into a large transient allocation.
        let mut expanded_bytes = 0usize;
        let mut index = 0usize;
        let mut failure = None;
        while index < delta.len() {
            match expanded_total(expanded_bytes, &delta[index], meter) {
                Ok(total) => expanded_bytes = total,
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
            index += 1;
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        if expanded_bytes > self.expanded_bytes {
            return Err(crate::Diagnostic::Defective);
        }
        let start = first / PREFIX_BLOCK * PREFIX_BLOCK;
        let mut output = alloc::vec::Vec::with_capacity(delta.len());
        let mut scratch = alloc::vec::Vec::new();
        index = start;
        while index < self.prefix_keys.len() {
            let record = &self.prefix_keys[index];
            match expand_record(record, index, first, &mut scratch, meter) {
                Ok(Some(key)) => output.push(key),
                Ok(None) => {}
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
            index += 1;
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(output),
        }
    }

    pub(crate) fn intern(
        &mut self,
        stack: &[noble_kernel::types::Ty],
    ) -> Result<u32, crate::Diagnostic> {
        self.intern_charged(stack, &mut Unmetered)
    }

    /// Charge each comparison actually visited, including a matching key.
    /// The selected source compiler meters its own pool; ordinary callers
    /// retain the exact previous unmetered interning path.
    pub(crate) fn intern_charged(
        &mut self,
        stack: &[noble_kernel::types::Ty],
        meter: &mut impl Meter,
    ) -> Result<u32, crate::Diagnostic> {
        let key = attempt!(walk::encode(stack, self.extended, self.compact));
        if self.compact {
            attempt!(meter.visit(key.len()));
            return self.intern_prefix(key, meter);
        }
        let mut index = 0usize;
        let mut found = None;
        let mut failure = None;
        while index < self.keys.len() {
            match compare_key(&self.keys[index], &key, meter) {
                Ok(true) => {
                    found = Some(index);
                    break;
                }
                Ok(false) => index += 1,
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        if let Some(problem) = failure {
            return Err(problem);
        }
        if let Some(index) = found {
            return number(index);
        }
        if self.extended && self.keys.len() >= SOURCE_KEY_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        let bytes = match self.bytes.checked_add(key.len()) {
            Some(bytes) => bytes,
            None => return Err(crate::Diagnostic::Exhausted),
        };
        if bytes > crate::OUTPUT_BYTE_LIMIT {
            return Err(crate::Diagnostic::Exhausted);
        }
        let id = attempt!(number(self.keys.len()));
        self.keys.push(key);
        self.bytes = bytes;
        Ok(id)
    }

    fn intern_prefix(
        &mut self,
        key: alloc::vec::Vec<u8>,
        meter: &mut impl Meter,
    ) -> Result<u32, crate::Diagnostic> {
        let hash = fingerprint(&key);
        // Lengths above u32 cannot be stored and are refused below; they
        // also cannot match an existing record.
        let Ok(length_bytes) = u32::try_from(key.len()) else {
            return Err(crate::Diagnostic::Exhausted);
        };
        let index_key = candidate_key(hash, length_bytes);
        let mut scratch = alloc::vec::Vec::new();
        // One ordered-index probe, then one unit per candidate actually
        // examined, plus every candidate block record actually decoded.
        attempt!(meter.visit(index_probe_cost(self.candidates.len())));
        if let Some(candidates) = self.candidates.get(&index_key) {
            let existing =
                attempt!(self.existing_record(candidates, hash, &key, &mut scratch, meter));
            if let Some(index) = existing {
                return number(index);
            }
        }
        let index = self.prefix_keys.len();
        if index >= CALCULATOR_SOURCE_KEY_LIMIT || key.len() > 65_536 {
            return Err(crate::Diagnostic::Exhausted);
        }
        let expanded_bytes = match self.expanded_bytes.checked_add(key.len()) {
            Some(bytes) if bytes <= CALCULATOR_EXPANDED_KEY_LIMIT => bytes,
            Some(_) | None => return Err(crate::Diagnostic::Exhausted),
        };
        let mut prefix = 0usize;
        if !index.is_multiple_of(PREFIX_BLOCK) {
            scratch.clear();
            attempt!(decode_records(
                &self.prefix_keys[index / PREFIX_BLOCK * PREFIX_BLOCK..index],
                &mut scratch,
                meter
            ));
            while prefix < key.len() && prefix < scratch.len() && key[prefix] == scratch[prefix] {
                prefix += 1;
            }
            attempt!(meter.visit(prefix.saturating_add(1)));
        }
        let suffix = &key[prefix..];
        let size_bytes = match self
            .bytes
            .checked_add(suffix.len().saturating_add(PREFIX_RECORD_BYTES))
        {
            Some(size) => size,
            None => return Err(crate::Diagnostic::Exhausted),
        };
        let length_bytes = match u32::try_from(key.len()) {
            Ok(value) => value,
            Err(_) => return Err(crate::Diagnostic::Exhausted),
        };
        let prefix = match u32::try_from(prefix) {
            Ok(value) => value,
            Err(_) => return Err(crate::Diagnostic::Exhausted),
        };
        attempt!(meter.visit(suffix.len().saturating_add(PREFIX_RECORD_BYTES)));
        attempt!(meter.visit(index_probe_cost(self.candidates.len())));
        let id = attempt!(number(index));
        self.prefix_keys.push(PrefixKey {
            prefix,
            length_bytes,
            hash,
            suffix: suffix.to_vec(),
        });
        self.candidates
            .entry(candidate_key(hash, length_bytes))
            .or_default()
            .push(id);
        self.bytes = size_bytes;
        self.expanded_bytes = expanded_bytes;
        Ok(id)
    }

    /// The first indexed candidate whose decoded bytes equal `key`, charging
    /// one unit per candidate examined plus every block record decoded.
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-maintainers; candidate decoding reuses the caller's private front-coding scratch, which requires Vec growth rather than a slice."
    )]
    fn existing_record(
        &self,
        candidates: &[u32],
        hash: u64,
        key: &[u8],
        scratch: &mut alloc::vec::Vec<u8>,
        meter: &mut impl Meter,
    ) -> Result<Option<usize>, crate::Diagnostic> {
        let mut position = 0usize;
        let mut verdict = Ok(None);
        while matches!(verdict, Ok(None)) && position < candidates.len() {
            verdict = self.candidate_record(candidates[position], hash, key, scratch, meter);
            position += 1;
        }
        verdict
    }

    /// `Some(index)` when stored record `candidate` decodes to exactly `key`.
    #[expect(
        tigerstyle::borrowed_argument_types,
        reason = "Owner: noble-maintainers; candidate decoding reuses the caller's private front-coding scratch, which requires Vec growth rather than a slice."
    )]
    fn candidate_record(
        &self,
        candidate: u32,
        hash: u64,
        key: &[u8],
        scratch: &mut alloc::vec::Vec<u8>,
        meter: &mut impl Meter,
    ) -> Result<Option<usize>, crate::Diagnostic> {
        attempt!(meter.visit(1));
        let index = candidate as usize;
        let Some(existing) = self.prefix_keys.get(index) else {
            return Err(crate::Diagnostic::Defective);
        };
        if existing.hash != hash || existing.length_bytes as usize != key.len() {
            return Ok(None);
        }
        scratch.clear();
        attempt!(decode_records(
            &self.prefix_keys[index / PREFIX_BLOCK * PREFIX_BLOCK..index + 1],
            scratch,
            meter
        ));
        if scratch.as_slice() == key {
            Ok(Some(index))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn interface(
        &mut self,
        interface: &noble_kernel::untrusted::Interface,
    ) -> Result<Interface, crate::Diagnostic> {
        if !interface.effects.is_empty() {
            return Err(crate::Diagnostic::Unsupported);
        }
        let input = attempt!(self.intern(&interface.stack_in));
        let output = attempt!(self.intern(&interface.stack_out));
        Ok(Interface { input, output })
    }

    pub(crate) fn produced_program(
        &mut self,
        interface: &noble_kernel::untrusted::Interface,
    ) -> Result<Interface, crate::Diagnostic> {
        match interface.stack_out.last() {
            Some(noble_kernel::types::Ty::Program(input, output, effects)) => {
                if !effects.is_empty() {
                    return Err(crate::Diagnostic::Unsupported);
                }
                let input = attempt!(self.intern(input));
                let output = attempt!(self.intern(output));
                Ok(Interface { input, output })
            }
            Some(_) | None => Err(crate::Diagnostic::Defective),
        }
    }

    pub(crate) fn emit(&self, out: &mut crate::output::Buffer) -> Result<(), crate::Diagnostic> {
        let mut index = 0usize;
        let mut failure = None;
        while index < self.keys.len() {
            match signature_line(out, index, &self.keys[index]) {
                Ok(()) => index += 1,
                Err(problem) => {
                    failure = Some(problem);
                    break;
                }
            }
        }
        match failure {
            Some(problem) => Err(problem),
            None => Ok(()),
        }
    }
}

/// Charge one comparison of `key` with a retained key, then compare them.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the only mutable input is the caller's charge meter, invoked once before the comparison result is returned."
)]
fn compare_key(
    stored: &[u8],
    key: &[u8],
    meter: &mut impl Meter,
) -> Result<bool, crate::Diagnostic> {
    attempt!(meter.visit(stored.len().min(key.len()).saturating_add(1)));
    Ok(stored == key)
}

/// Charge one delta record and add its expanded length to `total`, refusing
/// a total above the calculator expansion ceiling.
#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the only mutable input is the caller's charge meter, invoked once before the checked total is returned."
)]
fn expanded_total(
    total: usize,
    record: &PrefixKey,
    meter: &mut impl Meter,
) -> Result<usize, crate::Diagnostic> {
    attempt!(meter.visit(1));
    let total = match total.checked_add(record.length_bytes as usize) {
        Some(value) => value,
        None => return Err(crate::Diagnostic::Exhausted),
    };
    if total > CALCULATOR_EXPANDED_KEY_LIMIT {
        return Err(crate::Diagnostic::Exhausted);
    }
    Ok(total)
}

/// Decode record `index` into its block's `scratch`, returning an owned copy
/// when the record belongs to the requested delta `first..`.
#[expect(
    tigerstyle::borrowed_argument_types,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the decode truncates and extends the caller's private front-coding scratch, which requires Vec growth rather than a slice."
)]
fn expand_record(
    record: &PrefixKey,
    index: usize,
    first: usize,
    scratch: &mut alloc::vec::Vec<u8>,
    meter: &mut impl Meter,
) -> Result<Option<alloc::vec::Vec<u8>>, crate::Diagnostic> {
    if index.is_multiple_of(PREFIX_BLOCK) {
        scratch.clear();
    }
    if record.prefix as usize > scratch.len() {
        return Err(crate::Diagnostic::Defective);
    }
    scratch.truncate(record.prefix as usize);
    attempt!(meter.visit(record.suffix.len().saturating_add(1)));
    scratch.extend_from_slice(&record.suffix);
    attempt!(meter.visit(scratch.len()));
    if scratch.len() != record.length_bytes as usize || fingerprint(scratch) != record.hash {
        return Err(crate::Diagnostic::Defective);
    }
    if index < first {
        return Ok(None);
    }
    attempt!(meter.visit(scratch.len()));
    Ok(Some(scratch.clone()))
}

/// Decode the front-coded `records` in order into `scratch`, charging each
/// record's suffix copy and stopping at the first malformed prefix.
#[expect(
    tigerstyle::borrowed_argument_types,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the decode truncates and extends the caller's private front-coding scratch, which requires Vec growth rather than a slice."
)]
fn decode_records(
    records: &[PrefixKey],
    scratch: &mut alloc::vec::Vec<u8>,
    meter: &mut impl Meter,
) -> Result<(), crate::Diagnostic> {
    let mut index = 0usize;
    let mut outcome = Ok(());
    while outcome.is_ok() && index < records.len() {
        outcome = decode_record(&records[index], scratch, meter);
        index += 1;
    }
    outcome
}

/// Apply one front-coded record to `scratch`.
#[expect(
    tigerstyle::borrowed_argument_types,
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; the decode truncates and extends the caller's private front-coding scratch, which requires Vec growth rather than a slice."
)]
fn decode_record(
    record: &PrefixKey,
    scratch: &mut alloc::vec::Vec<u8>,
    meter: &mut impl Meter,
) -> Result<(), crate::Diagnostic> {
    if record.prefix as usize > scratch.len() {
        return Err(crate::Diagnostic::Defective);
    }
    scratch.truncate(record.prefix as usize);
    attempt!(meter.visit(record.suffix.len().saturating_add(1)));
    scratch.extend_from_slice(&record.suffix);
    Ok(())
}

#[expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; signature serialization mutates only the fresh bounded compiler sink, not the borrowed interned key."
)]
fn signature_line(
    out: &mut crate::output::Buffer,
    index: usize,
    key: &[u8],
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b";; signature "));
    attempt!(out.index(index));
    attempt!(out.append(b" = "));
    attempt!(out.append(key));
    out.append(b"\n")
}

#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; this guarded conversion calls the non-const TryFrom trait, retaining its explicit overflow diagnostic instead of narrowing."
)]
fn number(index: usize) -> Result<u32, crate::Diagnostic> {
    match u32::try_from(index) {
        Ok(index) => Ok(index),
        Err(_) => Err(crate::Diagnostic::Exhausted),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        tigerstyle::bool_naming,
        tigerstyle::function_length,
        tigerstyle::unbounded_collection_growth,
        tigerstyle::unchecked_narrowing,
        reason = "Owner: noble-wasm-maintainers; these pool fixtures build the whole boundary scenario in one function and assert its retention outcome, so terse flags, long setups and bounded test-only pushes are the readable form."
    )]

    /// Test meters are closures that record or refuse each visit.
    impl<F: FnMut(usize) -> Result<(), crate::Diagnostic>> super::Meter for F {
        fn visit(&mut self, count: usize) -> Result<(), crate::Diagnostic> {
            self(count)
        }
    }

    #[test]
    fn candidate_keys_keep_pair_order_and_lookups() {
        let pairs = [
            (0u64, 0u32),
            (0, 1),
            (0, u32::MAX),
            (1, 0),
            (1, 7),
            (0xff, 3),
            (0x7fff_ffff_ffff_ffff, u32::MAX),
            (0x8000_0000_0000_0000, 5),
            (u64::MAX, 0),
            (u64::MAX, u32::MAX),
        ];
        // Insert out of order so both maps must sort.
        let mut by_pair = alloc::collections::BTreeMap::new();
        let mut by_key = alloc::collections::BTreeMap::new();
        for (index, &(hash, length_bytes)) in pairs.iter().enumerate().rev() {
            by_pair.insert((hash, length_bytes), index);
            by_key.insert(super::candidate_key(hash, length_bytes), index);
        }
        // Iteration visits the classes in the same (lexicographic) order.
        let pair_order: alloc::vec::Vec<usize> = by_pair.values().copied().collect();
        let key_order: alloc::vec::Vec<usize> = by_key.values().copied().collect();
        assert_eq!(pair_order, key_order);
        assert_eq!(key_order, (0..pairs.len()).collect::<alloc::vec::Vec<_>>());
        // Every class finds its own entry, and neighbouring classes miss.
        for &(hash, length_bytes) in &pairs {
            assert_eq!(
                by_key.get(&super::candidate_key(hash, length_bytes)),
                by_pair.get(&(hash, length_bytes))
            );
        }
        for (hash, length_bytes) in [(0u64, 2u32), (2, 0), (1, 6), (u64::MAX - 1, u32::MAX)] {
            assert_eq!(by_pair.get(&(hash, length_bytes)), None);
            assert_eq!(by_key.get(&super::candidate_key(hash, length_bytes)), None);
        }
    }

    #[test]
    fn ordinary_pool_charges_each_compared_key_in_order_and_stops_at_a_refusal() {
        let mut pool = super::Pool::source();
        let first = [noble_kernel::types::Ty::I64];
        let second = [noble_kernel::types::Ty::Bool, noble_kernel::types::Ty::I64];
        let third = [noble_kernel::types::Ty::Unit];
        assert_eq!(pool.intern(&first).ok(), Some(0));
        assert_eq!(pool.intern(&second).ok(), Some(1));
        assert_eq!(pool.intern(&third).ok(), Some(2));
        // Keys "[I64]", "[Bool I64]" and "[Unit]": each visited key is
        // charged its shorter compared length plus one, and a match ends
        // the scan before any later key is charged.
        let mut charges = alloc::vec::Vec::new();
        let hit = pool.intern_charged(&second, &mut |amount| {
            charges.push(amount);
            Ok(())
        });
        assert_eq!(hit.ok(), Some(1));
        assert_eq!(charges, [6, 11]);
        // A new key ("[Text]") is charged against every retained key.
        charges.clear();
        let fresh = pool.intern_charged(&[noble_kernel::types::Ty::Text], &mut |amount| {
            charges.push(amount);
            Ok(())
        });
        assert_eq!(fresh.ok(), Some(3));
        assert_eq!(charges, [6, 7, 7]);
        // The first refused comparison stops the scan and interns nothing.
        charges.clear();
        let refused = pool.intern_charged(&[noble_kernel::types::Ty::Syntax], &mut |amount| {
            charges.push(amount);
            if charges.len() == 2 {
                Err(crate::Diagnostic::Exhausted)
            } else {
                Ok(())
            }
        });
        assert!(matches!(refused, Err(crate::Diagnostic::Exhausted)));
        assert_eq!(charges, [6, 9]);
        assert_eq!(pool.len(), 4);
    }

    #[test]
    fn calculator_pool_charges_decoded_records_in_order_and_stops_at_a_refusal() {
        let mut pool = super::Pool::source();
        pool.select_calculator();
        assert_eq!(
            pool.intern_prefix(alloc::vec![1, 2, 3], &mut |_| Ok(()))
                .ok(),
            Some(0)
        );
        assert_eq!(
            pool.intern_prefix(alloc::vec![9, 9], &mut |_| Ok(())).ok(),
            Some(1)
        );
        // One unit per delta record, then per decoded record its suffix copy
        // plus one and its decoded bytes, plus an owned copy for each
        // requested key.
        let mut charges = alloc::vec::Vec::new();
        let keys = pool.expanded_keys(0, &mut |amount| {
            charges.push(amount);
            Ok(())
        });
        assert_eq!(
            keys.ok(),
            Some(alloc::vec![alloc::vec![1, 2, 3], alloc::vec![9, 9]])
        );
        assert_eq!(charges, [1, 1, 4, 3, 3, 3, 2, 2]);
        // A delta starting at ID 1 decodes its block predecessor uncopied.
        charges.clear();
        let delta = pool.expanded_keys(1, &mut |amount| {
            charges.push(amount);
            Ok(())
        });
        assert_eq!(delta.ok(), Some(alloc::vec![alloc::vec![9, 9]]));
        assert_eq!(charges, [1, 4, 3, 3, 2, 2]);
        charges.clear();
        let refused = pool.expanded_keys(0, &mut |amount| {
            charges.push(amount);
            if charges.len() == 5 {
                Err(crate::Diagnostic::Exhausted)
            } else {
                Ok(())
            }
        });
        assert!(matches!(refused, Err(crate::Diagnostic::Exhausted)));
        assert_eq!(charges, [1, 1, 4, 3, 3]);
        // A hit charges the index probe, the candidate, and each block
        // record it decodes; a refusal inside that decode interns nothing.
        charges.clear();
        let hit = pool.intern_prefix(alloc::vec![9, 9], &mut |amount| {
            charges.push(amount);
            Ok(())
        });
        assert_eq!(hit.ok(), Some(1));
        assert_eq!(charges, [3, 1, 4, 3]);
        charges.clear();
        let bytes = pool.bytes;
        let refused = pool.intern_prefix(alloc::vec![9, 9], &mut |amount| {
            charges.push(amount);
            if charges.len() == 3 {
                Err(crate::Diagnostic::Exhausted)
            } else {
                Ok(())
            }
        });
        assert!(matches!(refused, Err(crate::Diagnostic::Exhausted)));
        assert_eq!(charges, [3, 1, 4]);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.bytes, bytes);
    }

    #[test]
    fn calculator_pool_crosses_front_coded_mib_but_refuses_expanded_overflow_atomically() {
        let mut pool = super::Pool::source();
        pool.select_calculator();
        // Different first bytes make each key independent of its predecessor.
        // Sixty-four maximum-length_bytes keys reach exactly the preexisting WAT
        // expansion ceiling, while front-coded storage exceeds one MiB.
        for first in 0..64u8 {
            let key = alloc::vec![first; 65_536];
            assert_eq!(
                pool.intern_prefix(key, &mut |_| Ok(())).ok(),
                Some(u32::from(first))
            );
        }
        assert_eq!(pool.expanded_bytes, super::CALCULATOR_EXPANDED_KEY_LIMIT);
        assert!(pool.bytes > crate::OUTPUT_BYTE_LIMIT);
        let previous_bytes = pool.bytes;
        assert_eq!(
            pool.intern_prefix(alloc::vec![0; 65_536], &mut |_| Ok(()))
                .ok(),
            Some(0)
        );
        assert_eq!(pool.expanded_bytes, super::CALCULATOR_EXPANDED_KEY_LIMIT);
        assert_eq!(pool.bytes, previous_bytes);
        for _ in 0..128 {
            assert_eq!(
                pool.intern_prefix(alloc::vec![0; 65_536], &mut |_| Ok(()))
                    .ok(),
                Some(0)
            );
        }
        assert_eq!(pool.expanded_bytes, super::CALCULATOR_EXPANDED_KEY_LIMIT);
        assert_eq!(pool.bytes, previous_bytes);
        assert!(matches!(
            pool.intern_prefix(alloc::vec![64], &mut |_| Ok(())),
            Err(crate::Diagnostic::Exhausted)
        ));
        assert!(matches!(
            pool.intern_prefix(alloc::vec![64; 65_537], &mut |_| Ok(())),
            Err(crate::Diagnostic::Exhausted)
        ));
        assert_eq!(pool.len(), 64);
        assert_eq!(pool.bytes, previous_bytes);
        let expanded = pool
            .expanded_keys(0, &mut super::Unmetered)
            .unwrap_or_else(|_| panic!("bounded keys did not decode"));
        for (first, key) in expanded.iter().enumerate() {
            assert_eq!(key.len(), 65_536);
            assert!(key.iter().all(|&byte| byte == first as u8));
        }
    }

    #[test]
    fn calculator_pool_never_reuses_colliding_fingerprint_or_exceeds_key_count() {
        let mut pool = super::Pool::source();
        pool.select_calculator();
        let original = alloc::vec![1, 2, 3, 4];
        let different = alloc::vec![4, 3, 2, 1];
        assert_eq!(
            pool.intern_prefix(original.clone(), &mut |_| Ok(())).ok(),
            Some(0)
        );
        // Force a genuine fingerprint collision: record 0 is also indexed
        // under the different key's fingerprint, so lookup must decode and
        // compare its bytes rather than trust the matching hash.
        pool.prefix_keys[0].hash = super::fingerprint(&different);
        pool.candidates
            .entry(super::candidate_key(super::fingerprint(&different), 4))
            .or_default()
            .push(0);
        assert_eq!(
            pool.intern_prefix(different.clone(), &mut |_| Ok(())).ok(),
            Some(1)
        );
        pool.prefix_keys[0].hash = super::fingerprint(&original);
        assert_eq!(pool.intern_prefix(original, &mut |_| Ok(())).ok(), Some(0));
        assert_eq!(pool.intern_prefix(different, &mut |_| Ok(())).ok(), Some(1));
        for value in 2..super::CALCULATOR_SOURCE_KEY_LIMIT {
            let key = (value as u32).to_le_bytes().to_vec();
            assert_eq!(
                pool.intern_prefix(key, &mut |_| Ok(())).ok(),
                Some(value as u32)
            );
        }
        let before = pool.bytes;
        assert!(matches!(
            pool.intern_prefix(alloc::vec![9; 5], &mut |_| Ok(())),
            Err(crate::Diagnostic::Exhausted)
        ));
        assert_eq!(pool.bytes, before);
        assert_eq!(pool.len(), super::CALCULATOR_SOURCE_KEY_LIMIT);
        assert_eq!(
            pool.intern_prefix(alloc::vec![4, 3, 2, 1], &mut |_| Ok(()))
                .ok(),
            Some(1)
        );
        let keys = pool
            .expanded_keys(0, &mut super::Unmetered)
            .unwrap_or_else(|_| panic!("bounded source keys did not decode"));
        assert_eq!(
            keys[super::SOURCE_KEY_LIMIT],
            (super::SOURCE_KEY_LIMIT as u32).to_le_bytes()
        );
        assert_eq!(
            keys[super::CALCULATOR_SOURCE_KEY_LIMIT - 1],
            ((super::CALCULATOR_SOURCE_KEY_LIMIT - 1) as u32).to_le_bytes()
        );
        // A delta that starts mid-block still decodes its front-coded
        // predecessors, but returns and charges output for only its own IDs.
        let mut copied = 0usize;
        let delta = pool
            .expanded_keys(super::CALCULATOR_SOURCE_KEY_LIMIT - 1, &mut |bytes| {
                copied += bytes;
                Ok(())
            })
            .unwrap_or_else(|_| panic!("bounded source delta did not decode"));
        assert_eq!(
            delta,
            [keys[super::CALCULATOR_SOURCE_KEY_LIMIT - 1].clone()]
        );
        assert!(
            copied < 32 * 16,
            "delta decode charged {copied} for one four-byte key"
        );
        assert_eq!(
            pool.expanded_keys(super::CALCULATOR_SOURCE_KEY_LIMIT, &mut super::Unmetered)
                .ok(),
            Some(alloc::vec::Vec::new())
        );
        assert!(matches!(
            pool.expanded_keys(
                super::CALCULATOR_SOURCE_KEY_LIMIT + 1,
                &mut super::Unmetered
            ),
            Err(crate::Diagnostic::Defective)
        ));
    }

    #[test]
    fn calculator_pool_lookup_charge_does_not_grow_with_retained_keys() {
        fn charged(pool: &mut super::Pool, key: alloc::vec::Vec<u8>) -> (Option<u32>, usize) {
            let mut total = 0usize;
            let id = pool
                .intern_prefix(key, &mut |amount| {
                    total += amount;
                    Ok(())
                })
                .ok();
            (id, total)
        }
        let key = |value: u32| {
            alloc::vec![b'k', b'0' + (value % 10) as u8]
                .into_iter()
                .chain(value.to_le_bytes())
                .collect::<alloc::vec::Vec<u8>>()
        };
        let mut pool = super::Pool::source();
        pool.select_calculator();
        for value in 0..64 {
            assert_eq!(charged(&mut pool, key(value)).0, Some(value));
        }
        let (small_hit_id, small_hit) = charged(&mut pool, key(40));
        let (small_new_id, small_new) = charged(&mut pool, key(100_000));
        assert_eq!(small_hit_id, Some(40));
        assert_eq!(small_new_id, Some(64));
        for value in 65..8_000 {
            assert_eq!(charged(&mut pool, key(value)).0, Some(value));
        }
        // IDs 40 and 7,016 share block offset 8, so both decode nine records.
        let (large_hit_id, large_hit) = charged(&mut pool, key(7_016));
        let (large_new_id, large_new) = charged(&mut pool, key(200_000));
        assert_eq!(large_hit_id, Some(7_016));
        assert_eq!(large_new_id, Some(8_000));
        // A 125x larger pool may deepen the ordered probe, but a linear scan
        // would charge thousands of comparisons for the same lookups.
        assert!(
            large_hit <= small_hit + 16,
            "hit charge grew from {small_hit} to {large_hit}"
        );
        assert!(
            large_new <= small_new + 32,
            "insert charge grew from {small_new} to {large_new}"
        );
    }

    #[test]
    fn calculator_pool_keeps_ordinary_count_ceiling_and_stable_cumulative_ids() {
        let mut ordinary = super::Pool::source();
        let mut selected = super::Pool::source();
        selected.select_calculator();
        for index in 0..super::CALCULATOR_SOURCE_KEY_LIMIT {
            let key = (index as u32).to_le_bytes().to_vec();
            if index < super::SOURCE_KEY_LIMIT {
                assert_eq!(
                    ordinary
                        .intern(&[noble_kernel::types::Ty::Resource(
                            noble_kernel::types::ResourceKind(index as u32)
                        )])
                        .ok(),
                    Some(index as u32)
                );
            }
            assert_eq!(
                selected.intern_prefix(key, &mut |_| Ok(())).ok(),
                Some(index as u32)
            );
            if index == super::SOURCE_KEY_LIMIT {
                assert_eq!(selected.len(), super::SOURCE_KEY_LIMIT + 1);
            }
        }
        let previous = selected.clone();
        assert_eq!(
            selected
                .intern_prefix(
                    (super::SOURCE_KEY_LIMIT as u32).to_le_bytes().to_vec(),
                    &mut |_| Ok(())
                )
                .ok(),
            Some(super::SOURCE_KEY_LIMIT as u32)
        );
        assert!(selected.prefix_keys == previous.prefix_keys);
        assert_eq!(selected.bytes, previous.bytes);
        assert_eq!(selected.expanded_bytes, previous.expanded_bytes);
        let count = ordinary.len();
        assert_eq!(count, super::SOURCE_KEY_LIMIT);
        // Ordinary source always uses complete, uncompressed stack keys.
        let ordinary_bytes = ordinary.bytes;
        assert!(matches!(
            ordinary.intern(&[noble_kernel::types::Ty::Resource(
                noble_kernel::types::ResourceKind(4096)
            )]),
            Err(crate::Diagnostic::Exhausted)
        ));
        assert_eq!(ordinary.len(), count);
        assert_eq!(ordinary.bytes, ordinary_bytes);
    }

    #[test]
    fn calculator_pool_preserves_ordinary_source_one_mib_boundary() {
        let mut pool = super::Pool::source();
        // Reserve the walker's six-slot worklist headroom as well as its
        // scalar-work limit, so only the aggregate byte ceiling can refuse.
        let mut stack = alloc::vec![noble_kernel::types::Ty::I64; 2044];
        let mut refused = false;
        for kind in 0..256u32 {
            stack[0] = noble_kernel::types::Ty::Resource(noble_kernel::types::ResourceKind(kind));
            if matches!(pool.intern(&stack), Err(crate::Diagnostic::Exhausted)) {
                refused = true;
                break;
            }
        }
        assert!(
            refused,
            "ordinary source keys unexpectedly crossed their byte ceiling"
        );
        assert!(
            pool.len() < 256,
            "key count, rather than bytes, refused first"
        );
        assert!(pool.bytes <= crate::OUTPUT_BYTE_LIMIT);
        assert!(pool.bytes > crate::OUTPUT_BYTE_LIMIT - 8_192);
    }

    #[test]
    fn calculator_generic_signature_identity_includes_concrete_payload_mapping() {
        let limits = noble_contracts::Limits {
            bytes: 65_536,
            nodes: 16_384,
            depth: 64,
            work: 2_000_000,
        };
        let mut compiler = crate::source::Compiler::new_calculator();
        let mut pool = super::Pool::source();
        pool.select_calculator();
        let mut ids = alloc::vec::Vec::new();
        let mut types = alloc::vec::Vec::new();
        let mut wats = alloc::vec::Vec::new();
        for source in [
            &b"module choice@1 [ variant Choice<A,B> Left A public Right B public export Choice ]"
                [..],
            &b"module choice@1 [ variant Choice<A,B> Left B public Right A public export Choice ]"
                [..],
        ] {
            let session = noble_contracts::source::ModuleSession::new_calculator()
                .unwrap_or_else(|_| panic!("selected source session refused"));
            let module = session
                .prepare(source, &[], limits)
                .unwrap_or_else(|_| panic!("generic module refused"));
            let (session, installed) = session.commit(module);
            assert!(installed.is_ok(), "generic module did not commit");
            let ty = session
                .resolve_type("choice@1.Choice<I64,Bool>")
                .unwrap_or_else(|_| panic!("generic concrete type refused"));
            let input = session
                .prepare(b"", core::slice::from_ref(&ty), limits)
                .unwrap_or_else(|_| panic!("source refused checked empty-body input"));
            let prepared = compiler
                .prepare(
                    input
                        .submission()
                        .unwrap_or_else(|| panic!("missing checked input submission")),
                )
                .unwrap_or_else(|_| panic!("selected compiler refused checked input"));
            wats.push(prepared.wat().to_vec());
            compiler
                .commit(prepared)
                .unwrap_or_else(|_| panic!("selected compiler did not commit checked input"));
            ids.push(
                pool.intern(core::slice::from_ref(&ty))
                    .unwrap_or_else(|_| panic!("selected signature refused checked input")),
            );
            types.push(ty);
        }
        let [first_id, second_id] = [ids[0], ids[1]];
        assert!(
            types[0] != types[1],
            "independent module declarations must have different canonical types"
        );
        assert!(
            matches!((&types[0], &types[1]),
            (noble_kernel::types::Ty::GenericNominal(first, first_args, _), noble_kernel::types::Ty::GenericNominal(second, second_args, _))
            if first == second && first_args.as_ref() == second_args.as_ref()),
            "test must isolate payload mapping from nominal identity and generic arguments"
        );
        assert_ne!(
            first_id, second_id,
            "different payload mappings cannot share a selected signature"
        );
        let keys = pool
            .expanded_keys(0, &mut super::Unmetered)
            .unwrap_or_else(|_| panic!("selected signature prefix did not decode"));
        // Each module publishes only signatures new since the committed base,
        // so every distinct identity appears in exactly one compiled module.
        for key in &keys {
            let mut suffix = alloc::vec![b' ', b'"'];
            suffix.extend_from_slice(key);
            suffix.extend_from_slice(b"\")");
            let published = wats
                .iter()
                .filter(|wat| {
                    wat.split(|byte| *byte == b'\n')
                        .any(|line| line.starts_with(b"(data $sig") && line.ends_with(&suffix))
                })
                .count();
            assert_eq!(
                published, 1,
                "each distinct passive signature identity must be published exactly once"
            );
        }
    }
}
