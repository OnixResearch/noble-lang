//! A module transaction's bounded index of its exact declared word namespace.
//! Only source preparation borrows it; the committed namespace remains the authority.

use alloc::{string::String, vec::Vec};

use crate::source::Target;

pub(in crate::source) struct NameIndex {
    /// Indices into Context.words, sorted by spelling then original ordinal.
    /// The rightmost equal spelling is the newest, matching the reverse scan.
    sorted: Vec<usize>,
    words_len: usize,
    owner: Option<u64>,
    generation: u64,
}

impl NameIndex {
    pub fn fits_original_work(
        words: &[(String, Target)],
        incoming: usize,
        longest_incoming: usize,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<bool, crate::Diagnostic> {
        let mut longest = longest_incoming;
        for (name, _) in words {
            attempt!(meter.charge(1, span));
            longest = longest.max(name.len());
        }
        let n = words.len() as u128;
        let new = incoming as u128;
        let slot = core::mem::size_of::<usize>() as u128;
        // Each heap sift inspects at most two keys and moves at most four
        // machine words per level. Duplicate keys additionally compare two
        // ordinals. The height is a conservative bound for every sift.
        let height = u128::from(usize::BITS - words.len().leading_zeros());
        let max_compare = (longest as u128).saturating_add(2 * slot + 2);
        let heap = (2 * n).saturating_mul(height)
            .saturating_mul(2 * max_compare + 4 * slot)
            .saturating_add(n.saturating_mul(4 * slot));
        let after = n.saturating_add(new);
        let insertion_height = u128::from(usize::BITS - after.min(usize::MAX as u128)
            .try_into().unwrap_or(usize::MAX).leading_zeros());
        let inserts = new.saturating_mul(
            after.saturating_mul(slot)
                .saturating_add(insertion_height.saturating_mul(max_compare))
                .saturating_add(1),
        );
        let storage = after.saturating_mul(slot).saturating_add(n.saturating_mul(slot));
        Ok(heap.saturating_add(inserts).saturating_add(storage) <= u128::from(meter.work))
    }

    pub fn build(
        words: &[(String, Target)],
        owner: Option<u64>,
        generation: u64,
        incoming: usize,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<Self, crate::Diagnostic> {
        let capacity = words.len().checked_add(incoming)
            .ok_or_else(|| crate::source::exhausted(span, "name index capacity exceeded"))?;
        let storage = capacity.checked_mul(core::mem::size_of::<usize>())
            .ok_or_else(|| crate::source::exhausted(span, "name index storage exceeded"))?;
        attempt!(meter.charge(attempt!(crate::index(storage, span)), span));
        let mut index = Self {
            sorted: Vec::with_capacity(capacity), words_len: words.len(), owner, generation,
        };
        let slot_work = attempt!(crate::index(core::mem::size_of::<usize>(), span));
        for ordinal in 0..words.len() {
            attempt!(meter.charge(slot_work, span));
            index.sorted.push(ordinal);
        }
        // Build without repeated Vec::insert shifts: even a reverse-ordered
        // outside alias namespace must admit unrelated source modules under
        // its original 2m Work ceiling. Every heap comparison and swap is
        // metered before reading or moving an index.
        for root in (0..index.sorted.len() / 2).rev() {
            attempt!(index.sift_down(words, root, index.sorted.len(), span, meter));
        }
        for end in (1..index.sorted.len()).rev() {
            attempt!(index.swap(0, end, span, meter));
            attempt!(index.sift_down(words, 0, end, span, meter));
        }
        Ok(index)
    }

    fn slot_order(
        &self,
        words: &[(String, Target)],
        left: usize,
        right: usize,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<core::cmp::Ordering, crate::Diagnostic> {
        let Some(a) = self.sorted.get(left).and_then(|slot| words.get(*slot)) else {
            return Err(crate::invalid(span, "indexed source word is absent"));
        };
        let Some(b) = self.sorted.get(right).and_then(|slot| words.get(*slot)) else {
            return Err(crate::invalid(span, "indexed source word is absent"));
        };
        let order = attempt!(compare(a.0.as_bytes(), b.0.as_bytes(), span, meter));
        if order != core::cmp::Ordering::Equal {
            return Ok(order);
        }
        attempt!(meter.charge(attempt!(crate::index(
            2 * core::mem::size_of::<usize>() + 1, span,
        )), span));
        Ok(self.sorted[left].cmp(&self.sorted[right]))
    }

    fn swap(
        &mut self,
        left: usize,
        right: usize,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        // Two machine-word reads and writes; the bounded heap is the only
        // index storage and never reallocates during the transaction.
        let moved = 4usize * core::mem::size_of::<usize>();
        attempt!(meter.charge(attempt!(crate::index(moved, span)), span));
        self.sorted.swap(left, right);
        Ok(())
    }

    fn sift_down(
        &mut self,
        words: &[(String, Target)],
        start: usize,
        end: usize,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        let mut root = start;
        loop {
            let Some(child) = root.checked_mul(2).and_then(|value| value.checked_add(1)) else {
                return Err(crate::source::exhausted(span, "name index heap bound exceeded"));
            };
            if child >= end { return Ok(()); }
            let mut greatest = child;
            if child + 1 < end
                && attempt!(self.slot_order(words, child, child + 1, span, meter))
                    == core::cmp::Ordering::Less
            {
                greatest += 1;
            }
            if attempt!(self.slot_order(words, root, greatest, span, meter))
                != core::cmp::Ordering::Less
            {
                return Ok(());
            }
            attempt!(self.swap(root, greatest, span, meter));
            root = greatest;
        }
    }

    pub fn insert(
        &mut self,
        words: &[(String, Target)],
        owner: Option<u64>,
        generation: u64,
        ordinal: usize,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        if owner != self.owner || self.generation.checked_add(1) != Some(generation) {
            return Err(crate::invalid(span, "indexed source owner or generation changed"));
        }
        attempt!(self.put(words, ordinal, span, meter));
        self.generation = generation;
        Ok(())
    }

    fn put(
        &mut self,
        words: &[(String, Target)],
        ordinal: usize,
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<(), crate::Diagnostic> {
        if words.len() != self.words_len.saturating_add(1) || ordinal != self.words_len {
            return Err(crate::invalid(span, "indexed source namespace changed outside its transaction"));
        }
        let name = &words[ordinal].0;
        let mut left = 0usize;
        let mut right = self.sorted.len();
        while left < right {
            let middle = left + (right - left) / 2;
            let Some(prior) = self.sorted.get(middle).and_then(|slot| words.get(*slot)) else {
                return Err(crate::invalid(span, "indexed source word is absent"));
            };
            match attempt!(compare(name.as_bytes(), prior.0.as_bytes(), span, meter)) {
                core::cmp::Ordering::Less => right = middle,
                core::cmp::Ordering::Greater | core::cmp::Ordering::Equal => left = middle + 1,
            }
        }
        // Vec::insert moves exactly this many machine-word indices; capacity
        // was reserved once for the whole bounded transaction before mutation.
        let moved = (self.sorted.len() - left)
            .checked_mul(core::mem::size_of::<usize>())
            .ok_or_else(|| crate::source::exhausted(span, "name index shift exceeded"))?;
        attempt!(meter.charge(attempt!(crate::index(moved.saturating_add(1), span)), span));
        self.sorted.insert(left, ordinal);
        self.words_len += 1;
        Ok(())
    }

    pub fn lookup(
        &self,
        words: &[(String, Target)],
        owner: Option<u64>,
        generation: u64,
        name: &[u8],
        span: crate::Span,
        meter: &mut crate::Meter,
    ) -> Result<Option<Target>, crate::Diagnostic> {
        if words.len() != self.words_len || owner != self.owner || generation != self.generation {
            return Err(crate::invalid(span, "indexed source namespace changed outside its transaction"));
        }
        let mut left = 0usize;
        let mut right = self.sorted.len();
        let mut found = None;
        while left < right {
            let middle = left + (right - left) / 2;
            let Some((spelling, target)) = words.get(self.sorted[middle]) else {
                return Err(crate::invalid(span, "indexed source word is absent"));
            };
            match attempt!(compare(name, spelling.as_bytes(), span, meter)) {
                core::cmp::Ordering::Less => right = middle,
                core::cmp::Ordering::Greater => left = middle + 1,
                core::cmp::Ordering::Equal => {
                    found = Some(*target);
                    left = middle + 1;
                }
            }
        }
        Ok(found)
    }
}

pub(in crate::source) fn compare(
    left: &[u8],
    right: &[u8],
    span: crate::Span,
    meter: &mut crate::Meter,
) -> Result<core::cmp::Ordering, crate::Diagnostic> {
    let mut offset = 0usize;
    let common_len = left.len().min(right.len());
    while offset < common_len {
        attempt!(meter.charge(1, span));
        match left[offset].cmp(&right[offset]) {
            core::cmp::Ordering::Equal => offset += 1,
            difference => return Ok(difference),
        }
    }
    attempt!(meter.charge(1, span));
    Ok(left.len().cmp(&right.len()))
}
