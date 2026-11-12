//! Replayable certificates for a small set of pure `I64` graph rewrites.
//!
//! The transformation stays untrusted. A certificate names, per rewritten node, the
//! rule identity, the ordered operand identities, and the constant bits and claim each
//! rule requires in canonical form. [`check`] validates those premises against the
//! original graph without calling the emitter, replays the folds on a copy of the
//! original and requires the resulting graph to equal that replay, and endpoint digests
//! bind the exact original and resulting graphs. A digest alone binds the result the
//! producer named, not the result the steps produce, so the replay is what refuses a
//! substituted result. Refusals are typed rather than folded into one failure.
//!
//! This is the Rust side of the lane modelled in
//! `proofs/foundation/NobleFoundation/GraphRewrite.lean`; the model's rule,
//! congruence and checker-soundness theorems are stated over the Lean graph
//! denotation, and **no correspondence theorem binds these bytes to that model**
//! yet. Budgets, replay work, derivation depth and retained evidence sizes are
//! likewise still open, as is every `F32`, guard, effect or resource rewrite.

use crate::identity::IdentityScheme;

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
///
/// Every field is canonical for the rule and [`check`] refuses a step that is not:
///
/// - [`Rule::FoldAddConstants`]: `first_bits` and `second_bits` are the exact bits of
///   the two operand constants, and `claimed` is their wrapping sum.
/// - [`Rule::NeutralAddLeft`]: the first operand is the graph constant `0`, `first_bits`
///   is `0`, `second_bits` equals `first_bits`, and `claimed` is `0`.
/// - [`Rule::NeutralMulRight`]: the second operand is the graph constant `1`,
///   `second_bits` is `1`, `first_bits` equals `second_bits`, and `claimed` is `0`.
///
/// An identity step records the cancelled identity element in both bit fields and
/// claims the other identity, `0`; it never claims the rewritten node's value, which
/// the surviving operand carries at run time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// Rule identity.
    pub rule: Rule,
    /// The rewritten node.
    pub node: crate::ir::NodeId,
    /// Ordered operand identities, destination order preserved.
    pub operands: [crate::ir::NodeId; 2],
    /// The first recorded constant bits, canonical for the rule.
    pub first_bits: u64,
    /// The second recorded constant bits, canonical for the rule.
    pub second_bits: u64,
    /// The claimed value: the folded sum, or `0` for an identity step.
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
    pub left: crate::ir::Digest,
    /// Digest of the resulting graph.
    pub right: crate::ir::Digest,
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
    /// The resulting graph is not the original with the derivation's folds applied.
    ResultMismatch,
}

fn bits(lit: crate::ir::Literal) -> Option<u64> {
    match lit {
        crate::ir::Literal::I64(value) => Some(value.cast_unsigned()),
        crate::ir::Literal::Bool(_) | crate::ir::Literal::F32(_) => None,
    }
}

fn constant_bits(graph: &crate::ir::Graph, node: crate::ir::NodeId) -> Option<u64> {
    graph
        .nodes
        .get(usize::try_from(node.0).ok()?)
        .and_then(|n| match n.op {
            crate::ir::Op::Const(lit) => bits(lit),
            crate::ir::Op::Input(_)
            | crate::ir::Op::Param(_)
            | crate::ir::Op::I64Add(_, _)
            | crate::ir::Op::I64Sub(_, _)
            | crate::ir::Op::I64Mul(_, _)
            | crate::ir::Op::I64Eq(_, _)
            | crate::ir::Op::F32Add(_, _)
            | crate::ir::Op::F32Sub(_, _)
            | crate::ir::Op::F32Mul(_, _)
            | crate::ir::Op::F32Affine { .. }
            | crate::ir::Op::F32Relu(_)
            | crate::ir::Op::F32Reglu { .. }
            | crate::ir::Op::F32Step(_)
            | crate::ir::Op::F32Select { .. }
            | crate::ir::Op::F32Index { .. }
            | crate::ir::Op::Guard { .. } => None,
        })
}

fn wrapping_add(first: u64, second: u64) -> u64 {
    first.wrapping_add(second)
}

/// Emits a derivation for every `I64` addition of two exact constants and every
/// `x * 1` multiplication, with the resulting graph in which each such addition is
/// replaced, in place, by its constant. The `x * 1` steps are recorded canonical but
/// not applied: replacing the node would change its identity, which is a separate
/// contract. `0 + x` is not recorded at all.
#[must_use]
pub fn emit(ids: &dyn IdentityScheme, graph: &crate::ir::Graph) -> (Derivation, crate::ir::Graph) {
    let mut result = graph.clone();
    let mut steps = alloc::vec::Vec::with_capacity(graph.nodes.len());
    for (index, node) in graph.nodes.iter().enumerate() {
        let Ok(index) = u32::try_from(index) else {
            continue;
        };
        match node.op {
            crate::ir::Op::I64Add(first, second) => {
                let (Some(first_bits), Some(second_bits)) =
                    (constant_bits(graph, first), constant_bits(graph, second))
                else {
                    continue;
                };
                steps.push(Step {
                    rule: Rule::FoldAddConstants,
                    node: crate::ir::NodeId(index),
                    operands: [first, second],
                    first_bits,
                    second_bits,
                    claimed: wrapping_add(first_bits, second_bits),
                });
                if let Some(target) = result.nodes.get_mut(usize::try_from(index).unwrap_or(0)) {
                    target.op = crate::ir::Op::Const(crate::ir::Literal::I64(
                        wrapping_add(first_bits, second_bits).cast_signed(),
                    ));
                }
            }
            crate::ir::Op::I64Mul(first, second) if constant_bits(graph, second) == Some(1) => {
                steps.push(Step {
                    rule: Rule::NeutralMulRight,
                    node: crate::ir::NodeId(index),
                    operands: [first, second],
                    // Canonical identity premise: both recorded bit fields hold the
                    // cancelled identity `1`, and the claim is the other identity `0`.
                    first_bits: 1,
                    second_bits: 1,
                    claimed: 0,
                });
            }
            crate::ir::Op::Input(_)
            | crate::ir::Op::Param(_)
            | crate::ir::Op::Const(_)
            | crate::ir::Op::I64Sub(_, _)
            | crate::ir::Op::I64Eq(_, _)
            | crate::ir::Op::F32Add(_, _)
            | crate::ir::Op::F32Sub(_, _)
            | crate::ir::Op::F32Mul(_, _)
            | crate::ir::Op::F32Affine { .. }
            | crate::ir::Op::F32Relu(_)
            | crate::ir::Op::F32Reglu { .. }
            | crate::ir::Op::F32Step(_)
            | crate::ir::Op::F32Select { .. }
            | crate::ir::Op::F32Index { .. }
            | crate::ir::Op::Guard { .. }
            | crate::ir::Op::I64Mul(_, _) => {}
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

/// Which rewrites a proposer offers.
///
/// A weaker proposer can still emit a **valid** certificate, so certificate validity alone never
/// says the transformation did useful work. Issue #64 measures the two separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proposer {
    /// Emit no step at all: the empty derivation over identical endpoints.
    Identity,
    /// Emit exactly one step, the first eligible fold in node order.
    FirstEligible,
    /// One ascending pass over the nodes: a node is offered a rewrite once and never revisited.
    SinglePass,
    /// Repeat passes until no fold is applicable.
    FixedPoint,
}

/// The fold step for `index`, if the node is an `I64` addition of two exact constants.
fn fold_site(graph: &crate::ir::Graph, index: usize) -> Option<Step> {
    let node = graph.nodes.get(index)?;
    let crate::ir::Op::I64Add(first, second) = node.op else {
        return None;
    };
    let (Some(first_bits), Some(second_bits)) =
        (constant_bits(graph, first), constant_bits(graph, second))
    else {
        return None;
    };
    Some(Step {
        rule: Rule::FoldAddConstants,
        node: crate::ir::NodeId(u32::try_from(index).ok()?),
        operands: [first, second],
        first_bits,
        second_bits,
        claimed: wrapping_add(first_bits, second_bits),
    })
}

/// Replaces the step's node by its folded constant.
fn apply_fold(graph: &mut crate::ir::Graph, step: &Step) {
    let Ok(index) = usize::try_from(step.node.0) else {
        return;
    };
    if let Some(target) = graph.nodes.get_mut(index) {
        target.op = crate::ir::Op::Const(crate::ir::Literal::I64(step.claimed.cast_signed()));
    }
}

/// The number of nodes where `fold-add-constants` still applies: the residual property a quality
/// target names, counted separately from certificate validity.
#[must_use]
pub fn residual_fold_sites(graph: &crate::ir::Graph) -> usize {
    (0..graph.nodes.len())
        .filter(|index| fold_site(graph, *index).is_some())
        .count()
}

/// Proposes rewrites under `proposer` and returns the derivation with the graph it produces.
///
/// Steps stay in strictly increasing node order in every case, so each proposer's certificate is
/// checked by the same validator; the proposers differ only in how much work they do.
#[must_use]
pub fn propose(
    ids: &dyn IdentityScheme,
    proposer: Proposer,
    graph: &crate::ir::Graph,
) -> (Derivation, crate::ir::Graph) {
    let mut result = graph.clone();
    let mut steps = alloc::vec::Vec::new();
    match proposer {
        Proposer::Identity => {}
        Proposer::FirstEligible => {
            if let Some(step) = (0..graph.nodes.len()).find_map(|index| fold_site(&result, index)) {
                apply_fold(&mut result, &step);
                steps.push(step);
            }
        }
        Proposer::SinglePass => {
            // Recorded from the original graph so a step's premises stay the ones the validator
            // compares, then applied to the result in ascending order.
            let sites: alloc::vec::Vec<Step> = (0..graph.nodes.len())
                .filter_map(|index| fold_site(graph, index))
                .collect();
            for step in sites {
                apply_fold(&mut result, &step);
                steps.push(step);
            }
        }
        Proposer::FixedPoint => {
            while let Some(step) =
                (0..result.nodes.len()).find_map(|index| fold_site(&result, index))
            {
                apply_fold(&mut result, &step);
                steps.push(step);
            }
        }
    }
    steps.sort_by_key(|step| step.node.0);
    let derivation = Derivation {
        revision: alloc::string::String::from(CERT_REVISION),
        steps,
        left: graph.logical_id(ids),
        right: result.logical_id(ids),
    };
    (derivation, result)
}

fn checked_bits(
    graph: &crate::ir::Graph,
    node: crate::ir::NodeId,
    declared: u64,
) -> Result<u64, Refusal> {
    match constant_bits(graph, node) {
        Some(actual) if actual == declared => Ok(actual),
        Some(_) => Err(Refusal::ConstantMismatch),
        None => Err(Refusal::Malformed),
    }
}

/// Whether an identity step's recorded bits are the canonical ones: the operand the rule
/// cancels is the graph's own constant holding `identity_bits`, and both recorded fields
/// hold that same constant.
fn identity_bits_are_canonical(
    graph: &crate::ir::Graph,
    operand: crate::ir::NodeId,
    declared_bits: [u64; 2],
    identity_bits: u64,
) -> bool {
    let [first_bits, second_bits] = declared_bits;
    constant_bits(graph, operand) == Some(identity_bits)
        && first_bits == identity_bits
        && second_bits == identity_bits
}

/// The graph the steps denote: each fold replaces its node, in place, by the claimed
/// constant, and identity steps are recorded but not applied, as the proposers record
/// them. Written beside the checker rather than shared with the proposers, so the checker
/// never runs producer code.
fn replay(graph: &crate::ir::Graph, derivation: &Derivation) -> Result<crate::ir::Graph, Refusal> {
    let mut replayed = graph.clone();
    for step in &derivation.steps {
        match step.rule {
            Rule::FoldAddConstants => {
                let index = usize::try_from(step.node.0).map_err(|_| Refusal::Malformed)?;
                let node = replayed.nodes.get_mut(index).ok_or(Refusal::Malformed)?;
                node.op = crate::ir::Op::Const(crate::ir::Literal::I64(step.claimed.cast_signed()));
            }
            Rule::NeutralAddLeft | Rule::NeutralMulRight => {}
        }
    }
    Ok(replayed)
}

/// Validates a derivation against the original and the resulting graph. The
/// operand order is compared, never re-derived from the claimed value, every
/// recorded bit field and claim must be the canonical one for its rule (see
/// [`Step`]), and the resulting graph must equal the original with the folds
/// replayed.
pub fn check(
    ids: &dyn IdentityScheme,
    graph: &crate::ir::Graph,
    result: &crate::ir::Graph,
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
            (Rule::FoldAddConstants, &crate::ir::Op::I64Add(first, second)) => {
                if step.operands != [first, second] {
                    return Err(Refusal::Malformed);
                }
                let first_bits = checked_bits(graph, first, step.first_bits)?;
                let second_bits = checked_bits(graph, second, step.second_bits)?;
                if step.claimed != wrapping_add(first_bits, second_bits) {
                    return Err(Refusal::ClaimMismatch);
                }
            }
            (Rule::NeutralAddLeft, &crate::ir::Op::I64Add(first, second)) => {
                if step.operands != [first, second] {
                    return Err(Refusal::Malformed);
                }
                // Canonical identity step: the graph's own constant zero is the operand the
                // rule cancels, both recorded bit fields hold it, and the claim is the
                // cancelled identity.
                if !identity_bits_are_canonical(
                    graph,
                    first,
                    [step.first_bits, step.second_bits],
                    0,
                ) {
                    return Err(Refusal::Malformed);
                }
                if step.claimed != 0 {
                    return Err(Refusal::Malformed);
                }
            }
            (Rule::NeutralMulRight, &crate::ir::Op::I64Mul(first, second)) => {
                if step.operands != [first, second] {
                    return Err(Refusal::Malformed);
                }
                // Canonical identity step: the graph's own constant one is the operand the
                // rule cancels, both recorded bit fields hold it, and the claim is the
                // cancelled identity.
                if !identity_bits_are_canonical(
                    graph,
                    second,
                    [step.first_bits, step.second_bits],
                    1,
                ) {
                    return Err(Refusal::Malformed);
                }
                if step.claimed != 0 {
                    return Err(Refusal::Malformed);
                }
            }
            (_, _) => return Err(Refusal::Malformed),
        }
    }
    if replay(graph, derivation)? != *result {
        return Err(Refusal::ResultMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::identity::IdentityScheme;

    /// A deterministic test scheme: digests every part in order under a fixed tag.
    struct TestIds;

    impl crate::identity::IdentityHasher for TestHasher {
        fn update(&mut self, bytes: &[u8]) {
            for &byte in bytes {
                self.0 = self.0.wrapping_mul(31).wrapping_add(u64::from(byte));
            }
        }

        fn finish(self: alloc::boxed::Box<Self>) -> crate::ir::Digest {
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
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
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
        let (derivation, result) = super::emit(&scheme, &graph);
        assert_eq!(derivation.revision, super::CERT_REVISION);
        assert!(derivation.steps.len() <= super::MAX_STEPS);
        assert!(super::check(&scheme, &graph, &result, &derivation).is_ok());
        assert_ne!(
            derivation.left, derivation.right,
            "the fold changes the graph digest"
        );
    }

    #[test]
    fn mutated_certificates_are_refused() {
        let graph = sample();
        let scheme = TestIds;
        let (derivation, result) = super::emit(&scheme, &graph);
        let fold = derivation
            .steps
            .iter()
            .copied()
            .find(|step| step.rule == super::Rule::FoldAddConstants)
            .expect("the sample folds one addition");

        let mut swapped = derivation.clone();
        swapped.steps[0].operands = [fold.operands[1], fold.operands[0]];
        assert_eq!(
            super::check(&scheme, &graph, &result, &swapped),
            Err(super::Refusal::Malformed)
        );

        let mut claimed = derivation.clone();
        claimed.steps[0].claimed = fold.claimed.wrapping_add(1);
        assert_eq!(
            super::check(&scheme, &graph, &result, &claimed),
            Err(super::Refusal::ClaimMismatch)
        );

        let mut bits = derivation.clone();
        bits.steps[0].first_bits = fold.first_bits.wrapping_add(1);
        assert_eq!(
            super::check(&scheme, &graph, &result, &bits),
            Err(super::Refusal::ConstantMismatch)
        );

        let mut renamed = derivation.clone();
        renamed.steps[0].rule = super::Rule::NeutralAddLeft;
        assert_eq!(
            super::check(&scheme, &graph, &result, &renamed),
            Err(super::Refusal::Malformed)
        );

        let mut stale = derivation.clone();
        stale.revision = alloc::string::String::from("ngraph-cert/1");
        assert_eq!(
            super::check(&scheme, &graph, &result, &stale),
            Err(super::Refusal::StaleRevision)
        );

        let mut unrelated = derivation.clone();
        unrelated.right = derivation.left;
        assert_eq!(
            super::check(&scheme, &graph, &result, &unrelated),
            Err(super::Refusal::EndpointMismatch)
        );

        let mut exhausted = derivation.clone();
        exhausted.steps = core::iter::repeat_n(fold, super::MAX_STEPS + 1).collect();
        assert_eq!(
            super::check(&scheme, &graph, &result, &exhausted),
            Err(super::Refusal::TooManySteps)
        );

        let mut reordered = derivation.clone();
        reordered.steps = alloc::vec![fold, fold];
        assert_eq!(
            super::check(&scheme, &graph, &result, &reordered),
            Err(super::Refusal::Order)
        );
    }

    /// The emitted certificate with its `right` digest renamed to a substituted result, as a
    /// dishonest producer would re-digest it: the steps stay valid for the original graph.
    fn redigested(derivation: &super::Derivation, result: &crate::ir::Graph) -> super::Derivation {
        let mut renamed = derivation.clone();
        renamed.right = result.logical_id(&TestIds);
        renamed
    }

    #[test]
    fn a_redigested_substituted_result_is_refused() {
        let graph = sample();
        let scheme = TestIds;
        let (derivation, result) = super::emit(&scheme, &graph);
        let fold = derivation
            .steps
            .iter()
            .copied()
            .find(|step| step.rule == super::Rule::FoldAddConstants)
            .expect("the sample folds one addition");
        let folded = usize::try_from(fold.node.0).expect("node index fits");
        let consumer = result
            .nodes
            .iter()
            .position(|node| matches!(node.op, crate::ir::Op::I64Mul(_, _)))
            .expect("the sample scales the sum");

        let mut wrong_constant = result.clone();
        wrong_constant.nodes[folded].op = crate::ir::Op::Const(crate::ir::Literal::I64(
            fold.claimed.wrapping_add(1).cast_signed(),
        ));
        let mut broken_sharing = result.clone();
        broken_sharing.nodes[consumer].op =
            crate::ir::Op::I64Mul(fold.operands[0], crate::ir::NodeId(3));
        let mut swapped_operands = result.clone();
        swapped_operands.nodes[consumer].op =
            crate::ir::Op::I64Mul(crate::ir::NodeId(3), fold.node);
        let mut changed_output = result.clone();
        changed_output.outputs = alloc::vec![fold.node];
        let mut repeated_output = result.clone();
        repeated_output.outputs.push(result.outputs[0]);
        let unrelated = {
            let mut builder = crate::ir::GraphBuilder::new(
                crate::ir::SubProfile::I64,
                crate::ir::Subject::authored(&scheme, "cert-sample"),
            );
            let eight = builder.i64_const(8);
            builder.output(eight);
            builder.finish()
        };

        for substituted in [
            &wrong_constant,
            &broken_sharing,
            &swapped_operands,
            &changed_output,
            &repeated_output,
            &unrelated,
        ] {
            assert_ne!(
                substituted, &result,
                "each control substitutes a different result"
            );
            assert_eq!(
                super::check(
                    &scheme,
                    &graph,
                    substituted,
                    &redigested(&derivation, substituted)
                ),
                Err(super::Refusal::ResultMismatch)
            );
        }
        assert_eq!(
            super::check(&scheme, &graph, &result, &redigested(&derivation, &result)),
            Ok(())
        );
    }

    /// A derivation over `steps` whose endpoints are both `graph`: identity steps are
    /// recorded without changing it.
    fn identity_derivation(
        graph: &crate::ir::Graph,
        steps: alloc::vec::Vec<super::Step>,
    ) -> super::Derivation {
        let scheme = TestIds;
        super::Derivation {
            revision: alloc::string::String::from(super::CERT_REVISION),
            steps,
            left: graph.logical_id(&scheme),
            right: graph.logical_id(&scheme),
        }
    }

    /// `zero + x` with `x` a program input: the canonical `neutral-add-left` premise.
    fn neutral_add_left() -> (
        crate::ir::Graph,
        crate::ir::NodeId,
        crate::ir::NodeId,
        crate::ir::NodeId,
    ) {
        let scheme = TestIds;
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            crate::ir::Subject::authored(&scheme, "cert-neutral-add-left"),
        );
        let zero = builder.i64_const(0);
        let input = builder.input(crate::ir::ScalarType::I64);
        let sum = builder.op(crate::ir::Op::I64Add(zero, input));
        builder.output(sum);
        (builder.finish(), zero, input, sum)
    }

    /// `x * 1` with `x` a program input: the canonical `neutral-mul-right` premise.
    fn neutral_mul_right() -> (
        crate::ir::Graph,
        crate::ir::NodeId,
        crate::ir::NodeId,
        crate::ir::NodeId,
    ) {
        let scheme = TestIds;
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            crate::ir::Subject::authored(&scheme, "cert-neutral-mul-right"),
        );
        let input = builder.input(crate::ir::ScalarType::I64);
        let one = builder.i64_const(1);
        let product = builder.op(crate::ir::Op::I64Mul(input, one));
        builder.output(product);
        (builder.finish(), one, input, product)
    }

    /// `x + 0` with `x` a program input: the operands of `neutral-add-left` in the
    /// other order, so its premise does not hold. Returns the graph and a premise-shaped
    /// step over it.
    fn swapped_sum() -> (crate::ir::Graph, super::Step) {
        let scheme = TestIds;
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            crate::ir::Subject::authored(&scheme, "cert-neutral-add-left-swapped"),
        );
        let input = builder.input(crate::ir::ScalarType::I64);
        let zero = builder.i64_const(0);
        let sum = builder.op(crate::ir::Op::I64Add(input, zero));
        builder.output(sum);
        let step = super::Step {
            rule: super::Rule::NeutralAddLeft,
            node: sum,
            operands: [input, zero],
            first_bits: 0,
            second_bits: 0,
            claimed: 0,
        };
        (builder.finish(), step)
    }

    /// `1 * x` with `x` a program input: the operands of `neutral-mul-right` in the
    /// other order, so its premise does not hold. Returns the graph and a premise-shaped
    /// step over it.
    fn swapped_mul() -> (crate::ir::Graph, super::Step) {
        let scheme = TestIds;
        let mut builder = crate::ir::GraphBuilder::new(
            crate::ir::SubProfile::I64,
            crate::ir::Subject::authored(&scheme, "cert-neutral-mul-right-swapped"),
        );
        let input = builder.input(crate::ir::ScalarType::I64);
        let one = builder.i64_const(1);
        let product = builder.op(crate::ir::Op::I64Mul(one, input));
        builder.output(product);
        let step = super::Step {
            rule: super::Rule::NeutralMulRight,
            node: product,
            operands: [one, input],
            first_bits: 1,
            second_bits: 1,
            claimed: 0,
        };
        (builder.finish(), step)
    }

    #[test]
    fn a_canonical_neutral_add_left_step_is_accepted_and_every_field_is_bound() {
        let (graph, zero, input, sum) = neutral_add_left();
        let scheme = TestIds;
        let canonical = super::Step {
            rule: super::Rule::NeutralAddLeft,
            node: sum,
            operands: [zero, input],
            first_bits: 0,
            second_bits: 0,
            claimed: 0,
        };
        assert_eq!(
            super::check(
                &scheme,
                &graph,
                &graph,
                &identity_derivation(&graph, alloc::vec![canonical])
            ),
            Ok(()),
            "the canonical `0 + x` step is accepted"
        );
        for (field, mutant) in [
            (
                "first_bits",
                super::Step {
                    first_bits: 1,
                    ..canonical
                },
            ),
            (
                "second_bits",
                super::Step {
                    second_bits: 1,
                    ..canonical
                },
            ),
            (
                "claimed",
                super::Step {
                    claimed: 1,
                    ..canonical
                },
            ),
        ] {
            assert_eq!(
                super::check(
                    &scheme,
                    &graph,
                    &graph,
                    &identity_derivation(&graph, alloc::vec![mutant])
                ),
                Err(super::Refusal::Malformed),
                "a mutated {field} is not a canonical identity premise"
            );
        }
        // The rule consumes its first operand only when the graph spells it as constant zero.
        let (swapped, swapped_step) = swapped_sum();
        assert_eq!(
            super::check(
                &scheme,
                &swapped,
                &swapped,
                &identity_derivation(&swapped, alloc::vec![swapped_step])
            ),
            Err(super::Refusal::Malformed),
            "`x + 0` is not the `neutral-add-left` premise"
        );
    }

    #[test]
    fn a_canonical_neutral_mul_right_step_is_accepted_and_every_field_is_bound() {
        let (graph, one, input, product) = neutral_mul_right();
        let scheme = TestIds;
        let canonical = super::Step {
            rule: super::Rule::NeutralMulRight,
            node: product,
            operands: [input, one],
            first_bits: 1,
            second_bits: 1,
            claimed: 0,
        };
        assert_eq!(
            super::check(
                &scheme,
                &graph,
                &graph,
                &identity_derivation(&graph, alloc::vec![canonical])
            ),
            Ok(()),
            "the canonical `x * 1` step is accepted"
        );
        for (field, mutant) in [
            (
                "first_bits",
                super::Step {
                    first_bits: 0,
                    ..canonical
                },
            ),
            (
                "second_bits",
                super::Step {
                    second_bits: 0,
                    ..canonical
                },
            ),
            (
                "claimed",
                super::Step {
                    claimed: u64::MAX,
                    ..canonical
                },
            ),
        ] {
            assert_eq!(
                super::check(
                    &scheme,
                    &graph,
                    &graph,
                    &identity_derivation(&graph, alloc::vec![mutant])
                ),
                Err(super::Refusal::Malformed),
                "a mutated {field} is not a canonical identity premise"
            );
        }
        // The rule consumes its second operand only when the graph spells it as constant one.
        let (swapped, swapped_step) = swapped_mul();
        assert_eq!(
            super::check(
                &scheme,
                &swapped,
                &swapped,
                &identity_derivation(&swapped, alloc::vec![swapped_step])
            ),
            Err(super::Refusal::Malformed),
            "`1 * x` is not the `neutral-mul-right` premise"
        );
    }

    #[test]
    fn the_emitted_identity_step_is_canonical_and_accepted() {
        let (graph, one, input, product) = neutral_mul_right();
        let scheme = TestIds;
        let (derivation, result) = super::emit(&scheme, &graph);
        assert_eq!(
            derivation.steps,
            alloc::vec![super::Step {
                rule: super::Rule::NeutralMulRight,
                node: product,
                operands: [input, one],
                first_bits: 1,
                second_bits: 1,
                claimed: 0,
            }],
            "the emitter records the canonical `x * 1` premise"
        );
        assert_eq!(
            result, graph,
            "a recorded identity does not change the graph"
        );
        assert_eq!(super::check(&scheme, &graph, &result, &derivation), Ok(()));
    }
}
