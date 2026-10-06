#![feature(register_tool)]
#![register_tool(tigerstyle)]

use noble_kernel::acceptance::check;
use noble_kernel::contracts::{environment, Env, NominalDecl, LIVE_DISPATCH, TEST_EMIT};
use noble_kernel::types::{EffId, EffSet, NominalShape, NominalTypeId, ResourceKind, Ty};
use noble_kernel::untrusted::{
    Candidate, Constraint, Expected, Interface, Limits, Node, NodeId, Outcome, Request,
    UnsupportedKind, CANDIDATE_FORMAT, SEMANTIC_REVISION,
};

fn effects(ids: &[EffId]) -> EffSet {
    EffSet::from_ids(ids)
}

fn account(module: u64, version: u32, kind: u32) -> NominalDecl {
    NominalDecl {
        id: NominalTypeId {
            module,
            ordinal: version,
        },
        shape: NominalShape::Opaque(Box::new(Ty::Resource(ResourceKind(kind)))),
        exported: true,
        public: [false, false],
    }
}

fn account_type(decl: &NominalDecl) -> Ty {
    Ty::Nominal(decl.id, Box::new(decl.shape.clone()))
}

fn live_environment() -> (Env, Ty, Ty) {
    let one = account(71, 1, 41);
    let two = account(71, 2, 42);
    let env = environment().expect("bootstrap")
        .enable_live_slots()
        .register_live_resource(one.clone()).expect("host registered @1")
        .register_live_resource(two.clone()).expect("host registered @2");
    (env, account_type(&one), account_type(&two))
}

fn request(input: Vec<Ty>, output: Vec<Ty>, allowed: &[EffId]) -> Request {
    Request {
        input_bytes: 40,
        expected: Expected {
            stack_in: input,
            stack_out: output,
            allowed_effects: effects(allowed),
        },
        limits: Limits {
            bytes: 512,
            nodes: 32,
            depth: 8,
            type_size: 128,
            stack_height: 16,
            work: 10_000,
            diagnostics: 64,
        },
    }
}

fn candidate(node: Node) -> Candidate {
    Candidate {
        format: CANDIDATE_FORMAT,
        revision: SEMANTIC_REVISION,
        nodes: vec![node],
        body: vec![NodeId(0)],
    }
}

fn slot(forwarded_ref_ordinals: Vec<u32>) -> Candidate {
    candidate(Node::SlotInvoke {
        ref_ordinal: 0,
        site_id: 0,
        forwarded_ref_ordinals,
    })
}

fn borrowed_request(account: Ty) -> (Request, Ty) {
    let forwarded = Ty::live_ref(vec![], vec![Ty::Bool], EffSet::empty());
    let primary = Ty::live_ref(
        vec![account.clone(), forwarded.clone(), Ty::Text],
        vec![account.clone()],
        effects(&[TEST_EMIT]),
    );
    (
        request(
            vec![primary, account.clone(), forwarded.clone(), Ty::Text],
            vec![account],
            &[TEST_EMIT, LIVE_DISPATCH],
        ),
        forwarded,
    )
}

fn rejected(outcome: Outcome, expected: Constraint) {
    match outcome {
        Outcome::Invalid(problem) => assert_eq!(problem.constraint, expected),
        other => panic!("expected {expected:?}, found {other:?}"),
    }
}

#[test]
fn ordered_borrowed_sidecar_checks_effect_and_admits_exact_target() {
    let (env, account_one, account_two) = live_environment();
    let (request, forwarded) = borrowed_request(account_one.clone());
    let Outcome::Accepted(checked) = check(&env, &request, &slot(vec![2])) else {
        panic!("expected live-slot acceptance");
    };
    assert_eq!(checked.interface.stack_in, request.expected.stack_in);
    assert_eq!(checked.interface.stack_out, vec![account_one.clone()]);
    assert_eq!(checked.interface.effects, effects(&[TEST_EMIT, LIVE_DISPATCH]));
    assert_eq!(
        effects(&[TEST_EMIT, LIVE_DISPATCH]).with_id(LIVE_DISPATCH),
        effects(&[TEST_EMIT, LIVE_DISPATCH]),
        "an existing dispatch effect must not be duplicated",
    );
    assert_eq!(checked.derivations[0].interface.stack_in, vec![account_one.clone(), Ty::Text]);
    assert_eq!(checked.derivations[0].interface.stack_out, vec![account_one.clone()]);
    let [site] = checked.live_sites.as_slice() else {
        panic!("expected one dispatch site");
    };
    assert_eq!((site.site_id, site.ref_ordinal, site.root_logical_input_position), (0, 0, 0));
    assert_eq!(site.forwarded_ref_ordinals, vec![2]);
    assert_eq!(site.stack_in, vec![account_one.clone(), forwarded, Ty::Text]);
    assert_eq!(site.stack_out, vec![account_one.clone()]);
    assert_eq!(site.allowed_effects, effects(&[TEST_EMIT]));

    let target = Interface {
        stack_in: site.stack_in.clone(),
        stack_out: site.stack_out.clone(),
        effects: EffSet::empty(),
    };
    assert!(site.admits_target(&target));
    let version_mismatch = Interface {
        stack_in: vec![account_two.clone(), site.stack_in[1].clone(), Ty::Text],
        ..target.clone()
    };
    assert!(!site.admits_target(&version_mismatch), "nominal @2 is not @1");
    let resource_mismatch = Interface {
        stack_in: vec![Ty::Resource(ResourceKind(41)), site.stack_in[1].clone(), Ty::Text],
        ..target.clone()
    };
    assert!(!site.admits_target(&resource_mismatch), "bare resource is not Account@1");
    let order_mismatch = Interface {
        stack_in: vec![Ty::Text, site.stack_in[1].clone(), account_one],
        ..target.clone()
    };
    assert!(!site.admits_target(&order_mismatch));
    let effect_mismatch = Interface {
        effects: effects(&[LIVE_DISPATCH]),
        ..target
    };
    assert!(!site.admits_target(&effect_mismatch));
    assert!(!account_two.is_data());
}

#[test]
fn borrowed_refs_cannot_be_forged_forwarded_incorrectly_or_leaked() {
    let (env, account, other_version) = live_environment();
    let (base, _) = borrowed_request(account.clone());
    let mut forged = slot(vec![2]);
    forged.nodes[0] = Node::SlotInvoke {
        ref_ordinal: 2,
        site_id: 0,
        forwarded_ref_ordinals: vec![2],
    };
    rejected(check(&env, &base, &forged), Constraint::BorrowProvenance);
    let without_root = request(
        vec![account.clone(), Ty::Text],
        vec![account.clone()],
        &[TEST_EMIT, LIVE_DISPATCH],
    );
    rejected(check(&env, &without_root, &slot(vec![])), Constraint::BorrowProvenance);
    rejected(check(&env, &base, &slot(vec![1])), Constraint::BorrowProvenance);
    rejected(check(&env, &base, &slot(vec![2, 2])), Constraint::BorrowProvenance);
    rejected(check(&env, &base, &slot(vec![])), Constraint::BorrowProvenance);
    forged.nodes[0] = Node::SlotInvoke {
        ref_ordinal: 0,
        site_id: 9,
        forwarded_ref_ordinals: vec![2],
    };
    rejected(check(&env, &base, &forged), Constraint::SlotSite);

    let mut wrong_version = base.clone();
    wrong_version.expected.stack_in[1] = other_version;
    rejected(check(&env, &wrong_version, &slot(vec![2])), Constraint::StackJoin);
    let mut wrong_forwarded_schema = base.clone();
    wrong_forwarded_schema.expected.stack_in[2] =
        Ty::live_ref(vec![], vec![Ty::I64], EffSet::empty());
    rejected(
        check(&env, &wrong_forwarded_schema, &slot(vec![2])),
        Constraint::StackJoin,
    );

    let mut escaped = base.clone();
    escaped.expected.stack_out = vec![base.expected.stack_in[0].clone()];
    rejected(check(&env, &escaped, &slot(vec![2])), Constraint::InvalidType);
    let mut ref_return = base.clone();
    ref_return.expected.stack_in[0] = Ty::live_ref(
        vec![account.clone(), Ty::Text],
        vec![base.expected.stack_in[2].clone()],
        effects(&[TEST_EMIT]),
    );
    rejected(check(&env, &ref_return, &slot(vec![])), Constraint::InvalidType);
    let mut captured = base.clone();
    captured.expected.stack_out = vec![Ty::program(
        vec![],
        vec![base.expected.stack_in[0].clone()],
        EffSet::empty(),
    )];
    rejected(check(&env, &captured, &slot(vec![2])), Constraint::InvalidType);
    let mut aggregate = base.clone();
    aggregate.expected.stack_in[0] = Ty::Pair(
        Box::new(base.expected.stack_in[0].clone()),
        Box::new(Ty::Unit),
    );
    rejected(check(&env, &aggregate, &slot(vec![2])), Constraint::InvalidType);
    assert!(!base.expected.stack_in[0].is_data());
    assert!(!Ty::List(Box::new(base.expected.stack_in[0].clone())).is_data());
    let mut raw_resource = base.clone();
    raw_resource.expected.stack_in[1] = Ty::Resource(ResourceKind(41));
    rejected(check(&env, &raw_resource, &slot(vec![2])), Constraint::InvalidType);
}

#[test]
fn forwarding_can_reborrow_one_root_reference_twice_in_formal_order() {
    let (env, account, _) = live_environment();
    let (mut request, forwarded) = borrowed_request(account.clone());
    request.expected.stack_in[0] = Ty::live_ref(
        vec![account.clone(), forwarded.clone(), forwarded.clone(), Ty::Text],
        vec![account.clone()],
        effects(&[TEST_EMIT]),
    );
    let Outcome::Accepted(checked) = check(&env, &request, &slot(vec![2, 2])) else {
        panic!("the same root reference may be borrowed twice");
    };
    let [site] = checked.live_sites.as_slice() else {
        panic!("one site");
    };
    assert_eq!(site.forwarded_ref_ordinals, vec![2, 2]);
    assert_eq!(
        checked.derivations[0].interface.stack_in,
        vec![account, Ty::Text],
    );
}

#[test]
fn profile_gate_quotation_capture_and_effect_ceiling_reject() {
    let (env, account, _) = live_environment();
    let (base, _) = borrowed_request(account.clone());
    let old = environment().expect("bootstrap");
    assert!(matches!(
        check(&old, &base, &slot(vec![2])),
        Outcome::Unsupported(UnsupportedKind::NodeForm)
    ));
    let mut without_dispatch = base.clone();
    without_dispatch.expected.allowed_effects = effects(&[TEST_EMIT]);
    rejected(
        check(&env, &without_dispatch, &slot(vec![2])),
        Constraint::EffectInclusion(LIVE_DISPATCH),
    );
    let quoted = Candidate {
        format: CANDIDATE_FORMAT,
        revision: SEMANTIC_REVISION,
        nodes: vec![
            Node::Quotation {
                body: vec![NodeId(1)],
                inst: noble_kernel::words::Inst {
                    bindings: vec![
                        noble_kernel::words::Binding::Stack(vec![account.clone(), Ty::Text]),
                        noble_kernel::words::Binding::Stack(vec![account.clone(), Ty::Text]),
                        noble_kernel::words::Binding::Stack(vec![account.clone()]),
                        noble_kernel::words::Binding::Effect(effects(&[TEST_EMIT, LIVE_DISPATCH])),
                    ],
                },
            },
            Node::SlotInvoke {
                ref_ordinal: 0,
                site_id: 1,
                forwarded_ref_ordinals: vec![2],
            },
        ],
        body: vec![NodeId(0)],
    };
    let mut quote_request = base.clone();
    quote_request.expected.stack_out = vec![account, Ty::Text, Ty::program(
        vec![base.expected.stack_in[1].clone(), Ty::Text],
        vec![base.expected.stack_in[1].clone()],
        effects(&[TEST_EMIT, LIVE_DISPATCH]),
    )];
    rejected(check(&env, &quote_request, &quoted), Constraint::BorrowProvenance);
}

#[test]
fn host_registration_rejects_alias_kinds_and_guest_resource_constructors() {
    let (env, _, _) = live_environment();
    assert!(env.clone().register_live_resource(account(80, 3, 41)).is_err());
    assert!(env.clone().register_live_resource(account(71, 1, 45)).is_err());
    assert!(env.clone().declare_nominal(account(80, 1, 45)).is_err());
    let mut forged = env.clone();
    forged.effects.clear();
    let request = request(vec![], vec![], &[LIVE_DISPATCH]);
    rejected(check(&forged, &request, &candidate(Node::SlotInvoke {
        ref_ordinal: 0,
        site_id: 0,
        forwarded_ref_ordinals: vec![],
    })), Constraint::InvalidContract);
}

#[test]
fn core_retains_existing_non_live_resource_admission() {
    let mut env = environment().expect("bootstrap");
    let extra = ResourceKind(99);
    env.resource_kinds.push(extra);
    let interface = request(
        vec![Ty::Resource(extra)],
        vec![Ty::Resource(extra)],
        &[],
    );
    let empty = Candidate {
        format: CANDIDATE_FORMAT,
        revision: SEMANTIC_REVISION,
        nodes: vec![],
        body: vec![],
    };
    assert!(matches!(check(&env, &interface, &empty), Outcome::Accepted(_)));
}
