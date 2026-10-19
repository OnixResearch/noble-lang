//! Exact reuse of calculator Program-free preparations (canonical epochs).
//!
//! At a Program-free boundary `prepare_program_free` restarts the function
//! allocation at the fixed base, starts an empty identity ledger, an empty
//! signature pool and an empty descriptor table, and publishes signatures
//! from ID 0. Its emitted bytes, prospective state, issued Work and refusal
//! therefore depend on the committed compiler only through the profile flags,
//! whether the compiler is still at generation 0 (the module that installs the
//! fixed core table entries), the static literal Text cursor `text_end` and
//! the content-addressed static literal table, neither of which any
//! preparation rewinds (a calculator session restore returns it to
//! the last commit together with the arena bytes and the generation, so the
//! restored compiler's epoch is exactly the committed one). Calculator
//! modules do not bake the preparation
//! generation into their bytes (the engine binds it per instance), so a
//! preparation's generation affects only the prospective generation, which
//! reuse recomputes. `CanonicalEpoch` is exactly that dependency; reuse is
//! refused unless the committed compiler's epoch equals the recorded one.

/// The exact committed-compiler component of a calculator reuse key. Only a
/// calculator compiler with no live, slot or text-cursor state and no
/// reusable or installed code spans has one.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CanonicalEpoch {
    first: bool,
    text_end: u32,
    // Content-addressed static literals decide whether a literal reuses an
    // address or advances `text_end`; equal cursors do not imply equal tables.
    static_literals: alloc::collections::BTreeMap<alloc::vec::Vec<u8>, u32>,
}

impl CanonicalEpoch {
    /// The next module installs the fixed core table entries.
    pub const fn first(&self) -> bool {
        self.first
    }

    /// The static literal Text cursor the module's literals start at.
    pub const fn text_end(&self) -> u32 {
        self.text_end
    }
}

/// A checked calculator Program-free preparation retained for exact reuse:
/// its WAT, its prospective compiler state and the Work its preparation
/// issued. It can only be produced by `Prepared::reusable` from a real
/// preparation and is only accepted by `Compiler::reuse_program_free` at an
/// identical canonical epoch.
pub struct Reusable {
    epoch: CanonicalEpoch,
    next: super::Compiler,
    code_start: u32,
    code_slots: u32,
    work_issued: u64,
    metrics: super::CompileMetrics,
    wat: alloc::vec::Vec<u8>,
}

impl Reusable {
    pub const fn epoch(&self) -> &CanonicalEpoch {
        &self.epoch
    }

    pub fn wat(&self) -> &[u8] {
        &self.wat
    }

    /// Cumulative Work issued by the recorded preparation.
    pub const fn work_issued(&self) -> u64 {
        self.work_issued
    }

    /// Conservative heap accounting for byte-bounded caches.
    pub fn retained_bytes(&self) -> usize {
        const RECORD: usize = 64;
        let identities = self.next.identities.iter().fold(0usize, |total, identity| {
            total
                .saturating_add(identity.recipe.len())
                .saturating_add(RECORD)
        });
        core::mem::size_of::<Self>()
            .saturating_add(self.wat.len())
            .saturating_add(self.next.signatures.retained_bytes())
            .saturating_add(self.next.descriptors.len().saturating_mul(8))
            .saturating_add(self.next.static_literals.keys().fold(0usize, |total, key| {
                total.saturating_add(key.len() + RECORD)
            }))
            .saturating_add(identities)
    }
}

impl super::Compiler {
    /// This compiler's canonical epoch, if it has one. The caller must still
    /// establish that the next submission starts at a Program-free boundary.
    pub fn canonical_epoch(&self) -> Option<CanonicalEpoch> {
        (self.calculator
            && !self.live
            && !self.live_slots
            && !self.text_cursor
            && self.free_code.is_empty()
            && self.installed_code.is_empty())
        .then(|| CanonicalEpoch {
            first: self.generation == 0,
            text_end: self.text_end,
            static_literals: self.static_literals.clone(),
        })
    }

    /// Calculator-only reuse of a recorded Program-free preparation, at the
    /// same boundary at which `prepare_program_free` may be used. The recorded
    /// artifact was prepared from an identical submission at an identical
    /// canonical epoch, so this returns exactly what `prepare_program_free`
    /// would: the same WAT and prospective state, with only the generation
    /// (not part of the bytes) and retention revision taken from this
    /// compiler. Any other committed state is refused without mutation.
    pub fn reuse_program_free(
        &self,
        artifact: &Reusable,
    ) -> Result<super::Prepared, crate::Diagnostic> {
        if self.canonical_epoch().as_ref() != Some(&artifact.epoch) {
            return Err(crate::Diagnostic::Invalid);
        }
        let generation = match self.generation.checked_add(1) {
            Some(value) => value,
            None => return Err(crate::Diagnostic::Exhausted),
        };
        let mut next = artifact.next.clone();
        next.generation = generation;
        next.retention_revision = self.retention_revision;
        Ok(super::Prepared {
            base: super::transaction::Base::capture(self, 0, 0, 0),
            base_live_slots: self.live_slots,
            base_retention_revision: self.retention_revision,
            next,
            code_span: super::CodeSpan {
                start: artifact.code_start,
                slots: artifact.code_slots,
                generation,
            },
            live_sites: alloc::vec::Vec::new(),
            target_metadata: None,
            program_metadata: alloc::vec::Vec::new(),
            named_targets: alloc::vec::Vec::new(),
            selected_target: None,
            selected_origin: None,
            resource_catalog: alloc::vec::Vec::new(),
            wat: artifact.wat.clone(),
            epoch: Some(artifact.epoch.clone()),
            work_issued: artifact.work_issued,
            metrics: artifact.metrics,
        })
    }
}

impl super::Prepared {
    /// The compiler generation this module must be submitted at: the engine
    /// binds it as the module's immutable `submit_generation` import.
    pub const fn submit_generation(&self) -> u32 {
        self.base.generation()
    }

    /// Cumulative Work issued by this preparation (the recorded preparation
    /// for a reused artifact).
    pub const fn work_issued(&self) -> u64 {
        self.work_issued
    }

    /// The canonical epoch this preparation was made at, if it is a
    /// calculator Program-free preparation that may be reused.
    pub const fn epoch(&self) -> Option<&CanonicalEpoch> {
        self.epoch.as_ref()
    }

    /// Retain this checked Program-free preparation for exact reuse. Only a
    /// calculator Program-free preparation at a canonical epoch, carrying no
    /// live, selected, origin or resource metadata, is reusable.
    pub fn reusable(&self) -> Option<Reusable> {
        let epoch = self.epoch.clone()?;
        let has_live_metadata = !self.live_sites.is_empty()
            || self.target_metadata.is_some()
            || !self.program_metadata.is_empty()
            || !self.named_targets.is_empty()
            || self.selected_target.is_some()
            || self.selected_origin.is_some()
            || !self.resource_catalog.is_empty();
        if has_live_metadata {
            return None;
        }
        Some(Reusable {
            epoch,
            next: self.next.clone(),
            code_start: self.code_span.start,
            code_slots: self.code_span.slots,
            work_issued: self.work_issued,
            metrics: self.metrics,
            wat: self.wat.clone(),
        })
    }
}
