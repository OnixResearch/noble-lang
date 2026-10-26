#![expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; the module path names the domain and the item name keeps its canonical operation spelling"
)]

//! Replayable certificates for a small set of pure `I64` graph rewrites.
//!
//! The transformation stays untrusted. A certificate names, per rewritten node, the
//! rule identity, the ordered operand identities, the exact constant bits the rule
//! consumed and the value it claims. [`check`] validates those premises against the
//! original graph without calling the emitter, and endpoint digests bind the exact
//! original and resulting graphs. Refusals are typed rather than folded into one
//! failure.
//!
//! This is the Rust side of the lane modelled in
//! `proofs/foundation/NobleFoundation/GraphRewrite.lean`; the model's rule,
//! congruence and checker-soundness theorems are stated over the Lean graph
//! denotation, and **no correspondence theorem binds these bytes to that model**
//! yet. Budgets, replay work, derivation depth and retained evidence sizes are
//! likewise still open, as is every `F32`, guard, effect or resource rewrite.

use crate::diag::{self, Diagnostic};
use crate::identity::IdentityScheme;
use crate::ir::{Digest, Graph, Literal, NodeId, Op};

/// Certificate format revision bound into every accepted derivation.
pub const CERT_REVISION: &str = "ngraph-cert/0";

/// Declared bound on derivation steps: a certificate larger than this is refused
/// before any node is inspected.
pub const MAX_STEPS: usize = 64;

/// One accepted rewrite rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    /// An addition of two exact `I64` constants denotes their wrapping sum.
    FoldAddConstants,
    /// `0 + x` denotes `x`.
    NeutralAddLeft,
    /// `x * 1` denotes `x`.
    NeutralMulRight,
}

impl Rule {
    /// Stable name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::FoldAddConstants => "fold-add-constants",
            Self::NeutralAddLeft => "neutral-add-left",
            Self::NeutralMulRight => "neutral-mul-right",
        }
    }
}

/// One derivation step: the whole premise, in ordered form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// Rule identity.
    pub rule: Rule,
    /// The rewritten node.
    pub node: NodeId,
    /// Ordered operand identities, destination order preserved.
    pub operands: [NodeId; 2],
    /// The exact bits of the first operand's constant.
    pub first_bits: u64,
    /// The exact bits of the second operand's constant.
    pub second_bits: u64,
    /// The claimed value of the rewritten node.
    pub claimed: u64,
}

/// A replayable derivation over exact endpoints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Derivation {
    /// Format revision.
    pub revision: alloc::string::String,
    /// Ordered steps; node identities strictly increase.
    pub steps: alloc::vec::Vec<Step>,
    /// Digest of the original graph.
    pub left: Digest,
    /// Digest of the resulting graph.
    pub right: Digest,
}

/// Why a certificate was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The format revision is not the current one.
    StaleRevision,
    /// The derivation exceeds the declared step bound.
    TooManySteps,
    /// A step is malformed, unsupported or names a node of another kind.
    Malformed,
    /// A declared constant does not match the graph's own bits.
    ConstantMismatch,
    /// The claimed value is not what the rule computes for those bits.
    ClaimMismatch,
    /// The steps are not in strictly increasing node order.
    Order,
    /// The certificate does not name the endpoints it was checked against.
    EndpointMismatch,
}

fn bits(lit: Literal) -> Option<u64> {
    match lit {
        Literal::I64(value) => Some(value.cast_unsigned()),
        Literal::Bool(_) | Literal::F32(_) => None,
    }
}

fn constant_bits(graph: &Graph, node: NodeId) -> Option<u64> {
    graph
        .nodes
        .get(usize::try_from(node.0).ok()?)
        .and_then(|n| match n.op {
            Op::Const(lit) => bits(lit),
            _ => None,
        })
}

fn wrapping_add(first: u64, second: u64) -> u64 {
    first.wrapping_add(second)
}

/// Emits a derivation for every foldable and identity node, and the resulting
/// graph in which each foldable addition is replaced, in place, by its constant.
/// Identity rewrites are recorded but not applied here: replacing `0 + x` by `x`
/// in place would change the node's identity, which is a separate contract.
#[must_use]
pub fn emit(ids: &dyn IdentityScheme, graph: &Graph) -> (Derivation, Graph) {
    let mut result = graph.clone();
    let mut steps = alloc::vec::Vec::new();
    for (index, node) in graph.nodes.iter().enumerate() {
        let Ok(index) = u32::try_from(index) else {
            continue;
        };
        match node.op {
            Op::I64Add(first, second) => {
                let (Some(first_bits), Some(second_bits)) =
                    (constant_bits(graph, first), constant_bits(graph, second))
                else {
                    continue;
                };
                steps.push(Step {
                    rule: Rule::FoldAddConstants,
                    node: NodeId(index),
                    operands: [first, second],
                    first_bits,
                    second_bits,
                    claimed: wrapping_add(first_bits, second_bits),
                });
                if let Some(target) = result.nodes.get_mut(usize::try_from(index).unwrap_or(0)) {
                    target.op = Op::Const(Literal::I64(
                        wrapping_add(first_bits, second_bits).cast_signed(),
                    ));
                }
            }
            Op::I64Mul(first, second) if constant_bits(graph, second) == Some(1) => {
                let one = constant_bits(graph, second).unwrap_or(0);
                steps.push(Step {
                    rule: Rule::NeutralMulRight,
                    node: NodeId(index),
                    operands: [first, second],
                    first_bits: one,
                    second_bits: one,
                    claimed: 0,
                });
            }
            _ => {}
        }
    }
    let derivation = Derivation {
        revision: alloc::string::String::from(CERT_REVISION),
        steps,
        left: graph.logical_id(ids),
        right: result.logical_id(ids),
    };
    (derivation, result)
}

fn checked_bits(graph: &Graph, node: NodeId, declared: u64) -> Result<u64, Refusal> {
    match constant_bits(graph, node) {
        Some(actual) if actual == declared => Ok(actual),
        Some(_) => Err(Refusal::ConstantMismatch),
        None => Err(Refusal::Malformed),
    }
}

/// Validates a derivation against the original and the resulting graph. The
/// operand order is compared, never re-derived from the claimed value, and every
/// declared constant is compared against the graph's own bits.
pub fn check(
    ids: &dyn IdentityScheme,
    graph: &Graph,
    result: &Graph,
    derivation: &Derivation,
) -> Result<(), Refusal> {
    if derivation.revision != CERT_REVISION {
        return Err(Refusal::StaleRevision);
    }
    if derivation.steps.len() > MAX_STEPS {
        return Err(Refusal::TooManySteps);
    }
    if graph.logical_id(ids) != derivation.left || result.logical_id(ids) != derivation.right {
        return Err(Refusal::EndpointMismatch);
    }
    let mut previous: Option<u32> = None;
    for step in &derivation.steps {
        if previous.is_some_and(|earlier| step.node.0 <= earlier) {
            return Err(Refusal::Order);
        }
        previous = Some(step.node.0);
        let op = &graph
            .nodes
            .get(usize::try_from(step.node.0).map_err(|_| Refusal::Malformed)?)
            .ok_or(Refusal::Malformed)?
            .op;
        match (step.rule, op) {
            (Rule::FoldAddConstants, &Op::I64Add(first, second)) => {
                if step.operands != [first, second] {
                    return Err(Refusal::Malformed);
                }
                let first_bits = checked_bits(graph, first, step.first_bits)?;
                let second_bits = checked_bits(graph, second, step.second_bits)?;
                if step.claimed != wrapping_add(first_bits, second_bits) {
                    return Err(Refusal::ClaimMismatch);
                }
            }
            (Rule::NeutralAddLeft, &Op::I64Add(first, second)) => {
                if step.operands != [first, second]
                    || checked_bits(graph, first, step.first_bits)? != 0
                {
                    return Err(Refusal::Malformed);
                }
            }
            (Rule::NeutralMulRight, &Op::I64Mul(first, second)) => {
                if step.operands != [first, second]
                    || checked_bits(graph, second, step.second_bits)? != 1
                {
                    return Err(Refusal::Malformed);
                }
            }
            (_, _) => return Err(Refusal::Malformed),
        }
    }
    Ok(())
}

/// Maps a refusal to the graph-malformed diagnostic form for callers that report it.
#[must_use]
pub fn refusal_diagnostic(refusal: &Refusal) -> Diagnostic {
    diag::Diagnostic::malformed(
        diag::Malformed::UnknownNode,
        diag::Site::Graph,
        alloc::format!("ngraph certificate refused: {refusal:?}"),
    )
}

#[cfg(test)]
mod tests {
    use super::{check, emit, Refusal, CERT_REVISION, MAX_STEPS};
    use crate::identity::IdentityScheme;
    use crate::ir::{Digest, GraphBuilder, SubProfile};

    /// A deterministic test scheme: digests every part in order under a fixed tag.
    struct TestIds;

    impl crate::identity::IdentityHasher for TestHasher {
        fn update(&mut self, bytes: &[u8]) {
            for &byte in bytes {
                self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(byte));
            }
        }

        fn finish(self: alloc::boxed::Box<Self>) -> Digest {
            let mut digest = [0u8; 32];
            digest[..8].copy_from_slice(&self.0.to_le_bytes());
            digest
        }
    }

    struct TestHasher(u64);

    impl IdentityScheme for TestIds {
        fn name(&self) -> &'static str {
            "test"
        }

        fn start(&self) -> alloc::boxed::Box<dyn crate::identity::IdentityHasher + '_> {
            alloc::boxed::Box::new(TestHasher(0))
        }
    }

    fn sample() -> crate::ir::Graph {
        let scheme = TestIds;
        let mut builder = GraphBuilder::new(
            SubProfile::I64,
            crate::ir::Subject::authored(&scheme, "cert-sample"),
        );
        let three = builder.i64_const(3);
        let five = builder.i64_const(5);
        let sum = builder.op(crate::ir::Op::I64Add(three, five));
        let one = builder.i64_const(1);
        let scaled = builder.op(crate::ir::Op::I64Mul(sum, one));
        builder.output(scaled);
        builder.finish()
    }

    #[test]
    fn emitted_fold_is_checked_and_endpoints_bind() {
        let graph = sample();
        let scheme = TestIds;
        let (derivation, result) = emit(&scheme, &graph);
        assert_eq!(derivation.revision, CERT_REVISION);
        assert!(derivation.steps.len() <= MAX_STEPS);
        assert!(check(&scheme, &graph, &result, &derivation).is_ok());
        assert_ne!(
            derivation.left, derivation.right,
            "the fold changes the graph digest"
        );
    }

    #[test]
    fn mutated_certificates_are_refused() {
        let graph = sample();
        let scheme = TestIds;
        let (derivation, result) = emit(&scheme, &graph);
        let fold = derivation
            .steps
            .iter()
            .copied()
            .find(|step| step.rule == super::Rule::FoldAddConstants)
            .expect("the sample folds one addition");

        let mut swapped = derivation.clone();
        swapped.steps[0].operands = [fold.operands[1], fold.operands[0]];
        assert_eq!(
            check(&scheme, &graph, &result, &swapped),
            Err(Refusal::Malformed)
        );

        let mut claimed = derivation.clone();
        claimed.steps[0].claimed = fold.claimed.wrapping_add(1);
        assert_eq!(
            check(&scheme, &graph, &result, &claimed),
            Err(Refusal::ClaimMismatch)
        );

        let mut bits = derivation.clone();
        bits.steps[0].first_bits = fold.first_bits.wrapping_add(1);
        assert_eq!(
            check(&scheme, &graph, &result, &bits),
            Err(Refusal::ConstantMismatch)
        );

        let mut renamed = derivation.clone();
        renamed.steps[0].rule = super::Rule::NeutralAddLeft;
        assert_eq!(
            check(&scheme, &graph, &result, &renamed),
            Err(Refusal::Malformed)
        );

        let mut stale = derivation.clone();
        stale.revision = alloc::string::String::from("ngraph-cert/1");
        assert_eq!(
            check(&scheme, &graph, &result, &stale),
            Err(Refusal::StaleRevision)
        );

        let mut unrelated = derivation.clone();
        unrelated.right = derivation.left;
        assert_eq!(
            check(&scheme, &graph, &result, &unrelated),
            Err(Refusal::EndpointMismatch)
        );

        let mut exhausted = derivation.clone();
        exhausted.steps = core::iter::repeat_n(fold, MAX_STEPS + 1).collect();
        assert_eq!(
            check(&scheme, &graph, &result, &exhausted),
            Err(Refusal::TooManySteps)
        );

        #[expect(
            tigerstyle::unused_mut,
            reason = "Owner: noble-maintainers; the clone is mutated below"
        )]
        let mut reordered = derivation.clone();
        reordered.steps = alloc::vec![fold, fold];
        assert_eq!(
            check(&scheme, &graph, &result, &reordered),
            Err(Refusal::Order)
        );
    }
}
