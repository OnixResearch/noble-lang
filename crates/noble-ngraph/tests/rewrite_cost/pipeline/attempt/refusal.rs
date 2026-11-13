//! Stable labels for upstream refusal variants.

use noble_ngraph::{cert, cert_scoped, layout};

pub(super) const fn certificate(value: &cert::Refusal) -> &'static str {
    match value {
        cert::Refusal::StaleRevision => "stale-revision",
        cert::Refusal::TooManySteps => "too-many-steps",
        cert::Refusal::Malformed => "malformed",
        cert::Refusal::ConstantMismatch => "constant-mismatch",
        cert::Refusal::ClaimMismatch => "claim-mismatch",
        cert::Refusal::Order => "order",
        cert::Refusal::EndpointMismatch => "endpoint-mismatch",
        cert::Refusal::ResultMismatch => "result-mismatch",
    }
}

pub(super) const fn scoped_certificate(value: &cert_scoped::Refusal) -> &'static str {
    match value {
        cert_scoped::Refusal::StaleRevision => "stale-revision",
        cert_scoped::Refusal::Order => "order",
        cert_scoped::Refusal::Malformed => "malformed",
        cert_scoped::Refusal::ScopeUnavailable => "scope-unavailable",
        cert_scoped::Refusal::ScopeValueMismatch => "scope-value-mismatch",
        cert_scoped::Refusal::ScopeConflict => "scope-conflict",
        cert_scoped::Refusal::ScopeKindMismatch => "scope-kind-mismatch",
        cert_scoped::Refusal::ClaimMismatch => "claim-mismatch",
        cert_scoped::Refusal::InterfaceChanged => "interface-changed",
        cert_scoped::Refusal::GuardChanged => "guard-changed",
        cert_scoped::Refusal::EndpointMismatch => "endpoint-mismatch",
        cert_scoped::Refusal::ResultMismatch => "result-mismatch",
    }
}

pub(super) const fn layout(value: layout::Refusal) -> &'static str {
    match value {
        layout::Refusal::StaleRevision => "stale-revision",
        layout::Refusal::StaleCompilerRevision => "stale-compiler-revision",
        layout::Refusal::LayoutKey => "layout-key",
        layout::Refusal::RegisterMapShape => "register-map-shape",
        layout::Refusal::RegisterOutOfRange => "register-out-of-range",
        layout::Refusal::AliasedRegisters => "aliased-registers",
        layout::Refusal::RegisterCount => "register-count",
        layout::Refusal::OneRegisterHeld => "one-register-held",
        layout::Refusal::ConstantTable => "constant-table",
        layout::Refusal::SlotTable => "slot-table",
        layout::Refusal::GuardRegister => "guard-register",
        layout::Refusal::LayerShape => "layer-shape",
        layout::Refusal::LayerOrder => "layer-order",
        layout::Refusal::FailSuffixMismatch => "fail-suffix-mismatch",
        layout::Refusal::LayerCoverage => "layer-coverage",
    }
}

#[test]
fn certificate_labels_are_stable() {
    use cert_scoped::Refusal;

    let refusals = [
        (Refusal::StaleRevision, "stale-revision"),
        (Refusal::Order, "order"),
        (Refusal::Malformed, "malformed"),
        (Refusal::ScopeUnavailable, "scope-unavailable"),
        (Refusal::ScopeValueMismatch, "scope-value-mismatch"),
        (Refusal::ScopeConflict, "scope-conflict"),
        (Refusal::ScopeKindMismatch, "scope-kind-mismatch"),
        (Refusal::ClaimMismatch, "claim-mismatch"),
        (Refusal::InterfaceChanged, "interface-changed"),
        (Refusal::GuardChanged, "guard-changed"),
        (Refusal::EndpointMismatch, "endpoint-mismatch"),
        (Refusal::ResultMismatch, "result-mismatch"),
    ];
    for (refusal, expected) in refusals {
        assert_eq!(scoped_certificate(&refusal), expected);
    }
}
