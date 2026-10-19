#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the test-only family checker imports kernel type and effect domain types directly so the candidate rules stay readable"
)]
use noble_kernel::contracts::{TEST_CLOCK, TEST_EMIT};
use noble_kernel::types::{EffId, EffSet, Ty};

use super::relation::{interface, is_interface_subtype, Interface};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Candidate {
    pub(super) interface: Interface,
}

pub(super) fn family(base: &Interface, universe: &[EffId]) -> Vec<Candidate> {
    let mut extra = Vec::with_capacity(universe.len());
    let mut at = 0usize;
    while at < universe.len() {
        if !base.effects.contains(universe[at]) {
            extra.push(universe[at]);
        }
        at += 1;
    }
    let count = 1usize << extra.len();
    let mut candidates = Vec::with_capacity(count);
    let mut mask = 0usize;
    while mask < count {
        let mut ids = base.effects.as_slice().to_vec();
        let mut bit = 0usize;
        while bit < extra.len() {
            if mask & (1usize << bit) != 0 {
                ids.push(extra[bit]);
            }
            bit += 1;
        }
        candidates.push(Candidate {
            interface: interface(
                base.inputs.clone(),
                base.outputs.clone(),
                EffSet::from_ids(&ids),
            ),
        });
        mask += 1;
    }
    candidates
}

pub(super) fn principal(candidates: &[Candidate]) -> Option<&Candidate> {
    let mut at = 0usize;
    while at < candidates.len() {
        let candidate = &candidates[at];
        let mut other = 0usize;
        let mut is_principal = true;
        while other < candidates.len() {
            if !is_interface_subtype(&candidate.interface, &candidates[other].interface) {
                is_principal = false;
                break;
            }
            other += 1;
        }
        if is_principal {
            return Some(candidate);
        }
        at += 1;
    }
    None
}

pub(super) fn explicit_accepts(base: &Interface, target: &Interface, universe: &[EffId]) -> bool {
    let candidates = family(base, universe);
    let mut at = 0usize;
    while at < candidates.len() {
        if is_interface_subtype(&candidates[at].interface, target) {
            return true;
        }
        at += 1;
    }
    false
}

pub(super) fn backward_accepts(base: &Interface, target: &Interface) -> bool {
    is_interface_subtype(base, target)
}

pub(super) fn is_whole_candidate(candidate: &Candidate, base: &Interface) -> bool {
    is_interface_subtype(&candidate.interface, base)
        && is_interface_subtype(base, &candidate.interface)
}
pub(super) fn differential() -> (usize, usize, usize, usize, usize) {
    let universes: &[&[EffId]] = &[
        &[],
        &[TEST_EMIT],
        &[TEST_EMIT, TEST_CLOCK],
        &[EffId(0), EffId(1), EffId(2), EffId(3)],
    ];
    let mut cases = 0usize;
    let mut accepted = 0usize;
    let mut refused = 0usize;
    let mut principal_families = 0usize;
    let mut max_family = 0usize;
    let mut universe_at = 0usize;
    while universe_at < universes.len() {
        let universe = universes[universe_at];
        let limit = 1usize << universe.len();
        let mut base_mask = 0usize;
        while base_mask < limit {
            let mut target_mask = 0usize;
            while target_mask < limit {
                let base = interface(Vec::new(), vec![Ty::I64], subset(universe, base_mask));
                let target = interface(Vec::new(), vec![Ty::I64], subset(universe, target_mask));
                let candidates = family(&base, universe);
                if candidates.len() > max_family {
                    max_family = candidates.len();
                }
                if principal(&candidates).is_some() {
                    principal_families += 1;
                }
                let is_explicit = explicit_accepts(&base, &target, universe);
                let is_backward = backward_accepts(&base, &target);
                assert_eq!(is_explicit, is_backward, "explicit/backward disagreement");
                if is_explicit {
                    accepted += 1;
                } else {
                    refused += 1;
                }
                cases += 1;
                target_mask += 1;
            }
            base_mask += 1;
        }
        universe_at += 1;
    }
    (cases, accepted, refused, principal_families, max_family)
}

pub(super) fn subset(universe: &[EffId], mask: usize) -> EffSet {
    let mut ids = Vec::with_capacity(universe.len());
    let mut at = 0usize;
    while at < universe.len() {
        if mask & (1usize << at) != 0 {
            ids.push(universe[at]);
        }
        at += 1;
    }
    EffSet::from_ids(&ids)
}
