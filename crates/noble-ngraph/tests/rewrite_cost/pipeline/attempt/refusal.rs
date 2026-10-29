//! Stable labels for upstream refusal variants.

use noble_ngraph::{cert, layout};

pub(super) const fn certificate(value: &cert::Refusal) -> &'static str {
    match value {
        cert::Refusal::StaleRevision => "stale-revision",
        cert::Refusal::TooManySteps => "too-many-steps",
        cert::Refusal::Malformed => "malformed",
        cert::Refusal::ConstantMismatch => "constant-mismatch",
        cert::Refusal::ClaimMismatch => "claim-mismatch",
        cert::Refusal::Order => "order",
        cert::Refusal::EndpointMismatch => "endpoint-mismatch",
    }
}

pub(super) const fn layout(value: layout::Refusal) -> &'static str {
    match value {
        layout::Refusal::StaleRevision => "stale-revision",
        layout::Refusal::RegisterMapShape => "register-map-shape",
        layout::Refusal::RegisterOutOfRange => "register-out-of-range",
        layout::Refusal::AliasedRegisters => "aliased-registers",
        layout::Refusal::RegisterCount => "register-count",
        layout::Refusal::ConstantTable => "constant-table",
        layout::Refusal::SlotTable => "slot-table",
        layout::Refusal::GuardRegister => "guard-register",
        layout::Refusal::LayerShape => "layer-shape",
        layout::Refusal::LayerOrder => "layer-order",
        layout::Refusal::FailSuffixMismatch => "fail-suffix-mismatch",
        layout::Refusal::LayerCoverage => "layer-coverage",
    }
}
