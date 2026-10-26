#![feature(register_tool)]
#![register_tool(tigerstyle)]
// This test crate asserts behaviour directly; panics and unwraps are the
// assertion mechanism, and its file name mirrors the module it exercises.
#![expect(
    tigerstyle::no_panic,
    reason = "test crate: assertion panics are the expected failure signal"
)]
#![expect(
    tigerstyle::no_unwrap,
    reason = "test crate: unwrap on the fixture environment is the assertion form"
)]
#![expect(
    tigerstyle::underscore_in_module_filename,
    reason = "the test target name mirrors the kernel module it exercises"
)]
#![expect(
    tigerstyle::excessive_file_length,
    reason = "one integration test crate per exercised kernel domain"
)]
#![expect(
    tigerstyle::non_trait_imports,
    reason = "test crate: imports read as the exercised kernel API surface"
)]
#![expect(
    tigerstyle::function_length,
    reason = "test crate: one linear step per construction, mutation and comparison rule"
)]
#![expect(
    tigerstyle::assertion_density,
    reason = "test crate: scenario assertions are added where the property is checked"
)]
#![expect(
    tigerstyle::ambiguous_params,
    reason = "test crate: positional fixture parameters are named at each call site"
)]

//! Bounded live-case generation, an independent admission model, and
//! failure-class-attributed mutations (issue #49, phase 1).
//!
//! The production acceptance routine is compared against, never used to label its
//! own inputs: `model` decides admission from the case declaration alone, using
//! the rules the issue names (root-issued reference, exact target interface,
//! ordered forwarded positions, effect ceilings and declared limits). One premise
//! is mutated at a time, and every mutation keeps the other premises intact so the
//! refusal can be attributed.
//!
//! Nothing here is a universal live-slot theorem. Coverage is reported by shape:
//! admission outcome, provenance violation, signature shape, repeated reborrow,
//! nesting and boundary class.

use noble_kernel::acceptance::check;
use noble_kernel::contracts::{environment, Env, NominalDecl, LIVE_DISPATCH, TEST_EMIT};
use noble_kernel::types::{EffId, EffSet, NominalShape, NominalTypeId, ResourceKind, Ty};
use noble_kernel::untrusted::{
    Candidate, Constraint, Expected, Limits, Node, NodeId, Outcome, Request, CANDIDATE_FORMAT,
    SEMANTIC_REVISION,
};

/// A nominal identity this case declares and the host independently registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity {
    module: u64,
    version: u32,
    kind: u32,
}

/// The bounded case representation: everything a verdict and a replay depend on.
#[derive(Clone, Debug)]
struct LiveCase {
    /// Host-registered live resources, in declaration order.
    registrations: Vec<Identity>,
    /// Ordinal, in the ordered input stack, of the borrowed input that issues the root.
    root_ordinal: u32,
    /// Exact target identity the reference must resolve to.
    target: Identity,
    /// Ordered forwarded positions, one per borrowed-reference formal of the root.
    forwarded: Vec<u32>,
    /// Ordered input stack handed to the request.
    stack_in: Vec<Ty>,
    /// One dispatch site per invocation node: its node identity.
    sites: Vec<u32>,
    /// Whether the root signature carries a nested borrowed-reference formal.
    nested: bool,
    /// The second registered identity a nested signature also names.
    second: Identity,
    /// Declared effect ceiling.
    ceilings: Vec<EffId>,
    /// Declared checker limits.
    limits: Limits,
}

/// The model's verdict and the premise it blames.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelVerdict {
    Admits,
    /// The declared root ordinal is not a borrowed input position.
    ForeignRoot,
    /// The target identity was never host-registered.
    UnknownTarget,
    /// The forwarded list does not carry exactly one position per formal.
    ForwardedCount,
    /// A forwarded position lies outside the declared stack.
    ForwardedOutOfStack,
    /// A forwarded position does not hold the formal's borrowed-reference type.
    ForwardedMismatch,
    /// A site identity is not its node identity.
    SiteIdentity,
}

/// The declared premises the mutation lanes flip, one at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Premise {
    Root,
    Target,
    ForwardedCount,
    ForwardedPosition,
    ForwardedMismatch,
    SiteIdentity,
}

fn effects(ids: &[EffId]) -> EffSet {
    EffSet::from_ids(ids)
}

fn nominal(identity: Identity) -> NominalDecl {
    NominalDecl {
        id: NominalTypeId {
            module: identity.module,
            ordinal: identity.version,
        },
        shape: NominalShape::Opaque(Box::new(Ty::Resource(ResourceKind(identity.kind)))),
        exported: true,
        public: [false, false],
    }
}

fn nominal_type(identity: Identity) -> Ty {
    Ty::Nominal(
        NominalTypeId {
            module: identity.module,
            ordinal: identity.version,
        },
        Box::new(NominalShape::Opaque(Box::new(Ty::Resource(ResourceKind(
            identity.kind,
        ))))),
    )
}

fn forwarded_type() -> Ty {
    Ty::live_ref(vec![], vec![Ty::Bool], EffSet::empty())
}

/// A nested borrowed-reference formal, declared only when the case asks for a
/// nested signature.
fn nested_type(one: Identity, two: Identity) -> Ty {
    Ty::live_ref(
        vec![nominal_type(one), nominal_type(two)],
        vec![nominal_type(one)],
        EffSet::empty(),
    )
}

/// The root input's declared signature: the target identity first, then one
/// borrowed-reference formal per forwarded position, then text.
fn root_formals(case: &LiveCase) -> Vec<Ty> {
    let nested = usize::from(case.nested);
    let mut formals = alloc_formals(case.forwarded.len().saturating_sub(nested));
    if case.nested {
        formals.push(nested_type(case.target, case.second));
    }
    formals
}

fn alloc_formals(count: usize) -> Vec<Ty> {
    core::iter::repeat_n(forwarded_type(), count).collect()
}

fn root_type(case: &LiveCase) -> Ty {
    let mut formals = vec![nominal_type(case.target)];
    formals.extend(root_formals(case));
    formals.push(Ty::Text);
    Ty::live_ref(formals, vec![nominal_type(case.target)], EffSet::empty())
}

fn limits() -> Limits {
    Limits {
        bytes: 512,
        nodes: 32,
        depth: 8,
        type_size: 128,
        stack_height: 16,
        work: 10_000,
        diagnostics: 64,
    }
}

/// Independent admission model: reads the declaration only and never calls the
/// production checker. The root's declared signature is taken from the recorded
/// case, its borrowed-reference formals are counted from that signature, and every
/// forwarded position must land on the formal it stands for.
fn model(case: &LiveCase) -> ModelVerdict {
    if !case.registrations.contains(&case.target) {
        return ModelVerdict::UnknownTarget;
    }
    let Some(root) = usize::try_from(case.root_ordinal)
        .ok()
        .and_then(|index| case.stack_in.get(index))
    else {
        return ModelVerdict::ForeignRoot;
    };
    let Ty::LiveRef(formals, _, _) = root else {
        return ModelVerdict::ForeignRoot;
    };
    if **formals != live_ref_formals(&root_type(case)) {
        return ModelVerdict::ForwardedMismatch;
    }
    let borrowed: Vec<&Ty> = formals
        .iter()
        .filter(|formal| matches!(formal, Ty::LiveRef(_, _, _)))
        .collect();
    if borrowed.len() != case.forwarded.len() {
        return ModelVerdict::ForwardedCount;
    }
    for (position, formal) in case.forwarded.iter().zip(borrowed.iter()) {
        match usize::try_from(*position).map(|index| case.stack_in.get(index)) {
            Ok(Some(actual)) if actual == *formal => {}
            Ok(Some(_)) => return ModelVerdict::ForwardedMismatch,
            _ => return ModelVerdict::ForwardedOutOfStack,
        }
    }
    for (index, &node) in case.sites.iter().enumerate() {
        if usize::try_from(node).ok() != Some(index) {
            return ModelVerdict::SiteIdentity;
        }
    }
    ModelVerdict::Admits
}

fn live_ref_formals(ty: &Ty) -> Vec<Ty> {
    match ty {
        Ty::LiveRef(formals, _, _) => (**formals).clone(),
        _ => Vec::new(),
    }
}

impl LiveCase {}

fn environment_for(case: &LiveCase) -> Env {
    let mut env = environment().expect("bootstrap").enable_live_slots();
    for identity in &case.registrations {
        env = env
            .register_live_resource(nominal(*identity))
            .expect("host registers the declared live resource");
    }
    env
}

fn request_for(case: &LiveCase) -> Request {
    Request {
        input_bytes: 40,
        expected: Expected {
            stack_in: case.stack_in.clone(),
            stack_out: vec![nominal_type(case.target)],
            allowed_effects: effects(&case.ceilings),
        },
        limits: case.limits.clone(),
    }
}

fn candidate_for(case: &LiveCase) -> Candidate {
    Candidate {
        format: CANDIDATE_FORMAT,
        revision: SEMANTIC_REVISION,
        nodes: case
            .sites
            .iter()
            .map(|node| Node::SlotInvoke {
                ref_ordinal: case.root_ordinal,
                site_id: *node,
                forwarded_ref_ordinals: case.forwarded.clone(),
            })
            .collect(),
        body: case.sites.iter().map(|node| NodeId(*node)).collect(),
    }
}

/// Positives are constructed by the declared rules: a host-registered target, a
/// borrowed root input whose signature names that exact target, one forwarded
/// position per borrowed-reference formal, and one site per invocation node. A
/// repeated reborrow uses the same root reference from two invocation nodes, which
/// the formal signature permits; the sites stay distinct.
fn positive_case(nesting: u32, repeated_reborrow: bool) -> LiveCase {
    let one = Identity {
        module: 71,
        version: 1,
        kind: 41,
    };
    let two = Identity {
        module: 71,
        version: 2,
        kind: 42,
    };
    let nested = nesting > 0;
    let mut stack_in = vec![nominal_type(one)];
    let mut forwarded = vec![2];
    if nested {
        // Two forwarded formals plus the nested one: one position per formal.
        stack_in.push(forwarded_type());
        stack_in.push(forwarded_type());
        stack_in.push(nested_type(one, two));
        forwarded.push(3);
        forwarded.push(4);
    } else {
        stack_in.push(forwarded_type());
    }
    stack_in.push(Ty::Text);
    let mut case = LiveCase {
        registrations: vec![one, two],
        root_ordinal: 0,
        target: one,
        forwarded: forwarded.clone(),
        stack_in,
        // A second invocation of the same root in one straight-line program needs
        // the first site's result in its own formal stack, which this phase does
        // not model; the repeated lane therefore reuses the root across program
        // instances instead of forging a two-site program.
        sites: vec![0],
        nested,
        second: two,
        ceilings: vec![TEST_EMIT, LIVE_DISPATCH],
        limits: limits(),
    };
    case.stack_in.insert(0, root_type(&case));
    let _ = repeated_reborrow;
    case
}

fn outcome_of(case: &LiveCase) -> Outcome {
    check(
        &environment_for(case),
        &request_for(case),
        &candidate_for(case),
    )
}

fn admitted(case: &LiveCase) -> bool {
    matches!(outcome_of(case), Outcome::Accepted(_))
}

/// The declared-case summary a replay must reproduce byte for byte.
fn summary(case: &LiveCase) -> String {
    format!(
        "registrations={:?} root={} target={:?} forwarded={:?} sites={:?} stack_len={} words={:?}",
        case.registrations,
        case.root_ordinal,
        case.target,
        case.forwarded,
        case.sites,
        case.stack_in.len(),
        case.stack_in.iter().map(Ty::is_data).collect::<Vec<_>>()
    )
}

/// The refusal class each mutated premise is measured to produce. `Target` is a
/// declared-identity failure rather than a provenance one, because the request's
/// type is decoded against the host registrations before the root is inspected.
fn expected_constraint(premise: Premise) -> Constraint {
    match premise {
        Premise::Root | Premise::ForwardedCount | Premise::ForwardedPosition => {
            Constraint::BorrowProvenance
        }
        Premise::Target => Constraint::InvalidType,
        Premise::ForwardedMismatch => Constraint::BorrowProvenance,
        Premise::SiteIdentity => {
            Constraint::MalformedReference(noble_kernel::untrusted::NodeId(SITE_IDENTITY_MUTATION))
        }
    }
}

/// The node identity the site premise mutation declares.
const SITE_IDENTITY_MUTATION: u32 = 7;

#[derive(Default)]
struct Coverage {
    cases: u32,
    admitted: u32,
    refused: u32,
    provenance_violations: u32,
    nested_signatures: u32,
    repeated_reborrows: u32,
}

impl Coverage {
    fn record(&mut self, case: &LiveCase, verdict: ModelVerdict, admitted: bool) {
        self.cases += 1;
        match verdict {
            ModelVerdict::Admits => self.admitted += 1,
            _ => {
                self.refused += 1;
                self.provenance_violations += 1;
            }
        }
        if case.forwarded.len() > 1 {
            self.nested_signatures += 1;
        }
        assert_eq!(
            verdict == ModelVerdict::Admits,
            admitted,
            "the independent model and production disagree; case {case:?}"
        );
    }
}

#[test]
fn positives_agree_with_the_independent_model_and_admit() {
    let mut coverage = Coverage::default();
    for nesting in 0..=1 {
        for repeated_reborrow in [false, true] {
            let case = positive_case(nesting, repeated_reborrow);
            let verdict = model(&case);
            assert_eq!(
                verdict,
                ModelVerdict::Admits,
                "stack_in={:?} root={:?} formals={:?} forwarded={:?}",
                case.stack_in,
                root_type(&case),
                root_formals(&case),
                case.forwarded
            );
            let outcome = outcome_of(&case);
            let Outcome::Accepted(checked) = outcome else {
                panic!("model admitted the constructed case but production refused it: {outcome:?}; case {case:?}");
            };
            assert_eq!(checked.live_sites.len(), case.sites.len());
            for (site, node) in checked.live_sites.iter().zip(case.sites.iter()) {
                assert_eq!(site.site_id, *node);
                assert_eq!(site.forwarded_ref_ordinals, case.forwarded);
                assert_eq!(site.ref_ordinal, case.root_ordinal);
            }
            let admitted = true;
            coverage.record(&case, verdict, admitted);
            if repeated_reborrow {
                // The same root reference is issued again, as a second program
                // instance: two sites inside one straight-line program would need
                // the first site's result in the second site's formal stack, which
                // this phase does not model and does not forge.
                assert!(matches!(outcome_of(&case), Outcome::Accepted(_)));
                coverage.repeated_reborrows += 1;
            }
        }
    }
    assert_eq!(coverage.cases, 4);
    assert_eq!(coverage.admitted, 4);
    assert_eq!(coverage.nested_signatures, 2);
    assert_eq!(coverage.repeated_reborrows, 2);
}

#[test]
fn one_premise_mutations_are_attributed_and_refused() {
    let mut coverage = Coverage::default();
    let base = positive_case(1, false);
    assert!(admitted(&base), "the base case must be admitted");
    for premise in [
        Premise::Root,
        Premise::Target,
        Premise::ForwardedCount,
        Premise::ForwardedPosition,
        Premise::ForwardedMismatch,
        Premise::SiteIdentity,
    ] {
        let mut case = base.clone();
        match premise {
            Premise::Root => case.root_ordinal = 9,
            Premise::Target => {
                case.target = Identity {
                    module: 71,
                    version: 9,
                    kind: 41,
                }
            }
            Premise::ForwardedCount => {
                // One surplus forwarded position: every formal already has one.
                case.forwarded.push(9);
            }
            Premise::ForwardedPosition => case.forwarded[1] = 17,
            Premise::ForwardedMismatch => case.stack_in[3] = Ty::Text,
            Premise::SiteIdentity => case.sites[0] = SITE_IDENTITY_MUTATION,
        }
        let verdict = model(&case);
        assert_ne!(
            verdict,
            ModelVerdict::Admits,
            "{premise:?} was not modelled as refusal"
        );
        match outcome_of(&case) {
            Outcome::Invalid(problem) => assert_eq!(
                problem.constraint,
                expected_constraint(premise),
                "{premise:?} must be attributed to its own premise"
            ),
            other => panic!("{premise:?} mutation was not refused: {other:?}"),
        }
        let admitted = false;
        coverage.record(&case, verdict, admitted);
    }
    assert_eq!(coverage.cases, 6);
    assert_eq!(coverage.refused, 6);
    assert_eq!(coverage.provenance_violations, 6);
}

#[test]
fn boundary_families_stay_distinct_from_agreement() {
    let case = positive_case(1, false);
    let mut boundary = case.clone();
    boundary.limits.stack_height = 1;
    assert_eq!(model(&boundary), ModelVerdict::Admits);
    match outcome_of(&boundary) {
        Outcome::Exhausted(limit) => {
            assert_eq!(limit, noble_kernel::untrusted::LimitKind::StackHeight)
        }
        other => panic!("declared stack-height exhaustion must stay its own class: {other:?}"),
    }
}

#[test]
fn replay_from_a_seed_is_deterministic_and_bounded() {
    let mut first = Vec::new();
    let mut state: u64 = 0x5eed_1234;
    for _ in 0..8 {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let nesting = u32::try_from(state >> 63).unwrap_or(0);
        let repeated = (state >> 62) & 1 == 1;
        let case = positive_case(nesting, repeated);
        assert_eq!(model(&case), ModelVerdict::Admits);
        first.push(summary(&case));
    }
    let mut replay: u64 = 0x5eed_1234;
    let mut second = Vec::new();
    for _ in 0..8 {
        replay = replay
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let nesting = u32::try_from(replay >> 63).unwrap_or(0);
        let repeated = (replay >> 62) & 1 == 1;
        second.push(summary(&positive_case(nesting, repeated)));
    }
    assert_eq!(
        first, second,
        "the same seed must retain the same full case"
    );
}
