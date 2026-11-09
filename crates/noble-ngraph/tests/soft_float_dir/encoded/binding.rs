//! The two encoded certificates bound to the contexts they were checked in (issue #44,
//! NGRAPH-ERR-04). Each `Binary32Binding.Context` field is computed here from the real artifacts:
//! - the snapshot is the checked graph's `GraphLogicalId`, which folds in the IR, semantic and
//!   fact-rule revisions of the checker that produced it;
//! - the domain digest covers exactly the domain list a generated theorem equates with the
//!   certificate's declared domains;
//! - the order digest covers exactly the graph term, nodes in executed order and outputs, that
//!   `Binary32CheckerEncoded.lean` states;
//! - the model digest covers `Binary32Checker.lean`, where the certificates and the checker live.
//!
//! The digests use the tests' identity scheme, which is deterministic but not cryptographic: they
//! detect a stale or mismatched context, not a forged one. `Binary32CheckerBound.lean` is generated
//! from them, and `encoded::write_the_lean_file` regenerates it.

use noble_ngraph::IdentityScheme;

/// The generated binding module the foundation lane kernel-checks.
pub(super) const COMMITTED: &str =
    include_str!("../../../../../proofs/foundation/NobleFoundation/Binary32CheckerBound.lean");

/// The model the certificates and their checker are defined in. It is read when the binding is
/// generated rather than included, so the model is not compiled into this test target.
const MODEL_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../proofs/foundation/NobleFoundation/Binary32Checker.lean"
);

fn model_digest() -> String {
    digest(&std::fs::read(MODEL_PATH).expect("the certificate model is readable"))
}

/// One encoded certificate: its graph, the certificate term, its declared domains as Lean text, and
/// the theorem that certifies it on the encoded graph.
struct Bound {
    name: &'static str,
    encoding: &'static str,
    certificate: &'static str,
    domains: &'static str,
    certified: &'static str,
}

const BOUNDS: [Bound; 2] = [
    Bound {
        name: "cancellation",
        encoding: super::CANCELLATION,
        certificate: "cancellationCertificate",
        domains: "[{ lo := 1, hi := 2, inputError := 0 }]",
        certified: "cancellation_certified",
    },
    Bound {
        name: "square",
        encoding: super::SQUARE,
        certificate: "(squareCertificate (2 ^ 63))",
        domains: "[{ lo := 1, hi := 2 ^ 63, inputError := 0 }]",
        certified: "square_near_overflow.1",
    },
];

/// A 256-bit identity as a Lean natural-number literal.
fn natural(digest: &[u8; 32]) -> String {
    let hex: String = digest.iter().map(|byte| format!("{byte:02X}")).collect();
    format!("0x{hex}")
}

/// The test scheme's identity of some bytes.
fn digest(bytes: &[u8]) -> String {
    let mut hasher = crate::common::IDS.start();
    hasher.update(bytes);
    natural(&hasher.finish())
}

/// The revision number of the IR format both encodings declare.
fn ir_revision() -> u32 {
    let revision = noble_ngraph::ir::IR_VERSION
        .strip_prefix("ngraph-ir/")
        .and_then(|number| number.parse().ok())
        .expect("an ngraph-ir/N format revision");
    assert!(BOUNDS
        .iter()
        .all(|bound| bound.encoding.starts_with(noble_ngraph::ir::IR_VERSION)));
    revision
}

/// The context one encoded certificate was checked in, as a Lean `Context` term.
fn context(bound: &Bound, model: &str) -> String {
    let checked = super::checked(bound.encoding);
    format!(
        "{{ snapshot := {}, revision := {}, domainDigest := {}, orderDigest := {}, contraction := false, model := {model} }}",
        natural(&checked.graph().logical_id(&crate::common::IDS)),
        ir_revision(),
        digest(bound.domains.as_bytes()),
        digest(super::graph_term(bound.encoding).as_bytes()),
    )
}

fn bound_text(bound: &Bound) -> String {
    let own = context(bound, &model_digest());
    let name = bound.name;
    format!(
        "/-- The domains the `{name}` digest covers are exactly the certificate's declared domains. -/
theorem {name}_domains : {certificate}.domains = {domains} := rfl

set_option maxRecDepth 100000 in
/-- **The `{name}` certificate certifies under the encoded graph's own context.** -/
theorem {name}_bound_certified :
    checkBound fixtureBudget {name}
      {own}
      {own}
      {certificate} = .certified := by
  rw [checkBound_agrees_when_equal]
  exact {certified}
",
        certificate = bound.certificate,
        domains = bound.domains,
        certified = bound.certified,
    )
}

fn controls_text() -> String {
    let model = model_digest();
    let [cancellation, square] = &BOUNDS;
    format!(
        "set_option maxRecDepth 100000 in
/-- Control: the cancellation certificate, declared for its own graph's context, is not applicable
in the square's. -/
theorem cancellation_certificate_not_applicable_to_the_square :
    checkBound fixtureBudget square
      {cancellation_context}
      {square_context}
      cancellationCertificate = .notApplicable := by
  decide +kernel

set_option maxRecDepth 100000 in
/-- Control: declared against another model text, the square certificate is not applicable. -/
theorem square_certificate_not_applicable_under_another_model :
    checkBound fixtureBudget square
      {stale_context}
      {square_context}
      (squareCertificate (2 ^ 63)) = .notApplicable := by
  decide +kernel
",
        cancellation_context = context(cancellation, &model),
        square_context = context(square, &model),
        stale_context = context(square, &digest(b"")),
    )
}

/// The whole generated binding module.
pub(super) fn lean_file() -> String {
    let bounds: Vec<String> = BOUNDS.iter().map(bound_text).collect();
    format!(
        "import NobleFoundation.Binary32CheckerEncoded
import NobleFoundation.Binary32Binding

/-!
# The encoded certificates bound to their contexts (issue #44, NGRAPH-ERR-04)

Generated by `crates/noble-ngraph/tests/soft_float_dir/encoded/binding.rs`; do not edit by hand.
`encoded::binding::the_contexts_are_the_real_artifacts` fails when a graph, a declared domain list or
the model changes, until this file is regenerated and the foundation lane rerun.

Each context below is computed from the real artifacts. The snapshot is the checked graph's
`GraphLogicalId`, which folds in the IR, semantic and fact-rule revisions of the `noble-ngraph`
checker that produced it. The revision is the IR format revision. The domain digest covers exactly
the domain list each `*_domains` theorem equates with the certificate's declared domains. The order
digest covers exactly the graph term, nodes in executed order and outputs, that
`Binary32CheckerEncoded` states, and the model digest covers
`Binary32Checker.lean`, where the certificates and the checker are defined. `noble-ngraph` never
contracts `mul` and `add`, so contraction is `false`.

The digests use the tests' identity scheme, which is deterministic but not cryptographic: a context
mismatch is detected, a forged identity is not ruled out. How the identities are computed stays
outside the model, as `Binary32Binding` states.
-/

namespace NobleFoundation.Binary32.Checker.Encoded

open NobleFoundation.Binary32 NobleFoundation.Binary32.Checker

{}
{}
end NobleFoundation.Binary32.Checker.Encoded
",
        bounds.join("\n"),
        controls_text(),
    )
}

#[test]
fn the_contexts_are_the_real_artifacts() {
    assert!(
        lean_file() == COMMITTED,
        "a graph, a declared domain list or the model changed: regenerate Binary32CheckerBound.lean with `cargo test -p noble-ngraph --test soft_float -- --ignored write_the_lean_file` and rerun the foundation lane"
    );
}
