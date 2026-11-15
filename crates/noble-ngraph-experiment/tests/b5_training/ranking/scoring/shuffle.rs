use super::*;

pub(crate) struct SplitMix64 {
    pub(crate) state: u64,
}

impl SplitMix64 {
    pub(crate) fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    pub(crate) fn index(&mut self, upper: usize) -> usize {
        let upper = u64::try_from(upper)
            .expect("the fixed candidate set fits into a u64 shuffle bound")
            .max(1);
        assert!(upper > 0, "the normalized shuffle bound must be nonzero");
        let threshold = upper.wrapping_neg() % upper;
        loop {
            let value = self.next();
            if value >= threshold {
                return usize::try_from(value % upper)
                    .expect("a reduced shuffle index fits into usize");
            }
        }
    }
}

pub(crate) fn shuffled_model_identity(
    seed: u64,
    model_id: &[u8; chunks::REVISION_BYTES],
) -> [u8; chunks::REVISION_BYTES] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"noble.tchunk-ranking-shuffle/1\0");
    hasher.update(model_id);
    hasher.update(&seed.to_le_bytes());
    *hasher.finalize().as_bytes()
}

pub(crate) fn shuffled_scores(
    context: &CandidateContext,
    seed: u64,
) -> std::collections::BTreeMap<u32, f32> {
    shuffled_ordinals(context, seed)
        .into_iter()
        .enumerate()
        .map(|(position, ordinal)| {
            let score = match position {
                0 => 4.0,
                1 => 3.0,
                2 => 2.0,
                _ => 1.0,
            };
            (ordinal, score)
        })
        .collect()
}

fn shuffled_ordinals(context: &CandidateContext, seed: u64) -> Vec<u32> {
    let mut ordinals: Vec<u32> = context
        .allowed_candidates
        .iter()
        .map(|candidate| candidate.ordinal)
        .collect();
    let mut rng = SplitMix64 { state: seed };
    for upper in (2..=ordinals.len()).rev() {
        let index = rng.index(upper);
        ordinals.swap(upper.saturating_sub(1), index);
    }
    ordinals
}
