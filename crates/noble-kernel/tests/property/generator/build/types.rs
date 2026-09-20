//! Explicit continuations and leaf draws for generated types.

#[octet::sealed_enum]
pub(super) enum Frame {
    PairRight(u64),
    SumRight(u64),
    PairFinish(noble_kernel::types::Ty),
    SumFinish(noble_kernel::types::Ty),
    List,
}

pub(super) fn base(choice: u64) -> noble_kernel::types::Ty {
    match choice {
        0 => noble_kernel::types::Ty::Unit,
        1 => noble_kernel::types::Ty::Bool,
        2 => noble_kernel::types::Ty::I64,
        3 => noble_kernel::types::Ty::Text,
        _ => noble_kernel::types::Ty::Syntax,
    }
}

pub(super) fn empty_or_emit(rng: &mut crate::rng::Stream) -> noble_kernel::types::EffSet {
    if rng.bit() {
        noble_kernel::types::EffSet::from_ids(&[noble_kernel::types::EffId(0)])
    } else {
        noble_kernel::types::EffSet::empty()
    }
}

pub(super) fn literal(
    rng: &mut crate::rng::Stream,
) -> Result<noble_kernel::untrusted::Lit, String> {
    match rng.below(4) {
        0 => {
            let is_negative = rng.bit();
            let magnitude = i64::try_from(rng.below(1000))
                .map_err(|error| format!("literal magnitude exceeds i64: {error}"))?;
            Ok(noble_kernel::untrusted::Lit::I64(if is_negative {
                -magnitude
            } else {
                magnitude
            }))
        }
        1 => Ok(noble_kernel::untrusted::Lit::Bool(rng.bit())),
        2 => Ok(noble_kernel::untrusted::Lit::Text),
        _ => Ok(noble_kernel::untrusted::Lit::Unit),
    }
}
