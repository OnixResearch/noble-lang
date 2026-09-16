//! Acceptance controls for the fragment checker.
// Names the fragment acceptance contract this crate implements.
// r[impl VT-M2-01]

fn ids(list: &[u32]) -> noble_kernel::types::EffSet {
    let effects: Vec<noble_kernel::types::EffId> = list
        .iter()
        .map(|id| noble_kernel::types::EffId(*id))
        .collect();
    noble_kernel::types::EffSet::from_ids(&effects)
}

fn segment(list: Vec<noble_kernel::types::Ty>) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Stack(list)
}

fn value(ty: noble_kernel::types::Ty) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Value(ty)
}

fn effect_binding(list: &[u32]) -> noble_kernel::words::Binding {
    noble_kernel::words::Binding::Effect(ids(list))
}

fn inst(list: Vec<noble_kernel::words::Binding>) -> noble_kernel::words::Inst {
    noble_kernel::words::Inst { bindings: list }
}

fn limits() -> noble_kernel::untrusted::Limits {
    noble_kernel::untrusted::Limits {
        bytes: 1 << 16,
        nodes: 256,
        depth: 32,
        type_size: 64,
        stack_height: 16,
        work: 10_000,
        diagnostics: 64,
    }
}

fn request(
    stack_in: Vec<noble_kernel::types::Ty>,
    stack_out: Vec<noble_kernel::types::Ty>,
    allowed: &[u32],
) -> noble_kernel::untrusted::Request {
    noble_kernel::untrusted::Request {
        input_bytes: 64,
        expected: noble_kernel::untrusted::Expected {
            stack_in,
            stack_out,
            allowed_effects: ids(allowed),
        },
        limits: limits(),
    }
}

fn lit_node(
    lit: noble_kernel::untrusted::Lit,
    stack: Vec<noble_kernel::types::Ty>,
) -> noble_kernel::untrusted::Node {
    noble_kernel::untrusted::Node::Literal {
        lit,
        inst: inst(vec![segment(stack)]),
    }
}

fn quote_node(
    body: Vec<u32>,
    surrounding: Vec<noble_kernel::types::Ty>,
    item_in: Vec<noble_kernel::types::Ty>,
    item_out: Vec<noble_kernel::types::Ty>,
    effects: &[u32],
) -> noble_kernel::untrusted::Node {
    noble_kernel::untrusted::Node::Quotation {
        body: body
            .into_iter()
            .map(noble_kernel::untrusted::NodeId)
            .collect(),
        inst: inst(vec![
            segment(surrounding),
            segment(item_in),
            segment(item_out),
            effect_binding(effects),
        ]),
    }
}

fn candidate(
    nodes: Vec<noble_kernel::untrusted::Node>,
    body: Vec<u32>,
) -> noble_kernel::untrusted::Candidate {
    noble_kernel::untrusted::Candidate {
        format: 0,
        revision: 0,
        nodes,
        body: body
            .into_iter()
            .map(noble_kernel::untrusted::NodeId)
            .collect(),
    }
}

fn env() -> Result<noble_kernel::contracts::Env, String> {
    match noble_kernel::contracts::environment() {
        Ok(env) => Ok(env),
        Err(defect) => Err(format!("environment defect: {defect:?}")),
    }
}

fn accept(
    env: &noble_kernel::contracts::Env,
    request: &noble_kernel::untrusted::Request,
    candidate: &noble_kernel::untrusted::Candidate,
) -> Result<noble_kernel::untrusted::Checked, String> {
    match noble_kernel::acceptance::check(env, request, candidate) {
        noble_kernel::untrusted::Outcome::Accepted(checked) => Ok(checked),
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => {
            Err(format!("invalid: {diagnostic:?}"))
        }
        noble_kernel::untrusted::Outcome::Unsupported(kind) => {
            Err(format!("unsupported: {kind:?}"))
        }
        noble_kernel::untrusted::Outcome::Exhausted(limit) => Err(format!("exhausted: {limit:?}")),
        noble_kernel::untrusted::Outcome::InternalFailure => Err("internal failure".to_string()),
    }
}

const ADD: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(4);
const DUP: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(0);
const RUN: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(9);
const TEST_EMIT: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(21);

fn forty_two() -> (
    noble_kernel::untrusted::Request,
    noble_kernel::untrusted::Candidate,
) {
    let program = candidate(
        vec![
            lit_node(noble_kernel::untrusted::Lit::I64(41), vec![]),
            lit_node(
                noble_kernel::untrusted::Lit::I64(1),
                vec![noble_kernel::types::Ty::I64],
            ),
            noble_kernel::untrusted::Node::Invocation {
                def: ADD,
                inst: inst(vec![segment(vec![])]),
            },
            quote_node(
                vec![1, 2],
                vec![noble_kernel::types::Ty::I64],
                vec![noble_kernel::types::Ty::I64],
                vec![noble_kernel::types::Ty::I64],
                &[],
            ),
            noble_kernel::untrusted::Node::Invocation {
                def: RUN,
                inst: inst(vec![
                    segment(vec![noble_kernel::types::Ty::I64]),
                    segment(vec![noble_kernel::types::Ty::I64]),
                    effect_binding(&[]),
                ]),
            },
        ],
        vec![0, 3, 4],
    );
    (
        request(vec![], vec![noble_kernel::types::Ty::I64], &[]),
        program,
    )
}

#[test]
fn sequence_and_literals_accept_arithmetic() -> Result<(), String> {
    let env = env()?;
    let program = candidate(
        vec![
            lit_node(noble_kernel::untrusted::Lit::I64(41), vec![]),
            lit_node(
                noble_kernel::untrusted::Lit::I64(1),
                vec![noble_kernel::types::Ty::I64],
            ),
            noble_kernel::untrusted::Node::Invocation {
                def: ADD,
                inst: inst(vec![segment(vec![])]),
            },
        ],
        vec![0, 1, 2],
    );
    let checked = accept(
        &env,
        &request(vec![], vec![noble_kernel::types::Ty::I64], &[]),
        &program,
    )?;
    assert_eq!(checked.derivations.len(), 3);
    assert_eq!(
        checked.interface.stack_out,
        vec![noble_kernel::types::Ty::I64]
    );
    Ok(())
}

#[test]
fn quotation_construction_checks_body_and_runs() -> Result<(), String> {
    let env = env()?;
    let (request, program) = forty_two();
    let checked = accept(&env, &request, &program)?;
    assert_eq!(checked.derivations.len(), 5);
    Ok(())
}

#[test]
fn hidden_emit_rejects_against_empty_bound() -> Result<(), String> {
    let env = env()?;
    let emit = || noble_kernel::untrusted::Node::Invocation {
        def: TEST_EMIT,
        inst: inst(vec![segment(vec![])]),
    };
    let hidden = candidate(
        vec![
            emit(),
            quote_node(
                vec![0],
                vec![],
                vec![noble_kernel::types::Ty::Text],
                vec![noble_kernel::types::Ty::Unit],
                &[],
            ),
        ],
        vec![1],
    );
    let pushed = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::Text],
        vec![noble_kernel::types::Ty::Unit],
        noble_kernel::types::EffSet::empty(),
    );
    match noble_kernel::acceptance::check(&env, &request(vec![], vec![pushed], &[0]), &hidden) {
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => assert_eq!(
            diagnostic.constraint,
            noble_kernel::untrusted::Constraint::EffectInclusion(noble_kernel::types::EffId(0))
        ),
        noble_kernel::untrusted::Outcome::Accepted(_)
        | noble_kernel::untrusted::Outcome::Unsupported(_)
        | noble_kernel::untrusted::Outcome::Exhausted(_)
        | noble_kernel::untrusted::Outcome::InternalFailure => {
            return Err("expected an effect-inclusion rejection".to_string())
        }
    }
    Ok(())
}

#[test]
fn limits_boundaries_and_exhaustion() -> Result<(), String> {
    let env = env()?;
    let (base, program) = forty_two();

    let mut exact = base.clone();
    exact.limits.nodes = 5;
    accept(&env, &exact, &program)?;
    let mut over = base.clone();
    over.limits.nodes = 4;
    match noble_kernel::acceptance::check(&env, &over, &program) {
        noble_kernel::untrusted::Outcome::Exhausted(noble_kernel::untrusted::LimitKind::Nodes) => {}
        noble_kernel::untrusted::Outcome::Accepted(_)
        | noble_kernel::untrusted::Outcome::Invalid(_)
        | noble_kernel::untrusted::Outcome::Unsupported(_)
        | noble_kernel::untrusted::Outcome::Exhausted(_)
        | noble_kernel::untrusted::Outcome::InternalFailure => {
            return Err("expected a nodes exhaustion".to_string())
        }
    }

    let mut deep = base.clone();
    deep.limits.depth = 1;
    accept(&env, &deep, &program)?;
    let mut shallow = base.clone();
    shallow.limits.depth = 0;
    match noble_kernel::acceptance::check(&env, &shallow, &program) {
        noble_kernel::untrusted::Outcome::Exhausted(noble_kernel::untrusted::LimitKind::Depth) => {}
        noble_kernel::untrusted::Outcome::Accepted(_)
        | noble_kernel::untrusted::Outcome::Invalid(_)
        | noble_kernel::untrusted::Outcome::Unsupported(_)
        | noble_kernel::untrusted::Outcome::Exhausted(_)
        | noble_kernel::untrusted::Outcome::InternalFailure => {
            return Err("expected a depth exhaustion".to_string())
        }
    }

    let single = candidate(
        vec![lit_node(noble_kernel::untrusted::Lit::Unit, vec![])],
        vec![0],
    );
    let mut work = request(vec![], vec![noble_kernel::types::Ty::Unit], &[]);
    work.limits.work = 8;
    accept(&env, &work, &single)?;
    work.limits.work = 4;
    match noble_kernel::acceptance::check(&env, &work, &single) {
        noble_kernel::untrusted::Outcome::Exhausted(noble_kernel::untrusted::LimitKind::Work) => {}
        noble_kernel::untrusted::Outcome::Accepted(_)
        | noble_kernel::untrusted::Outcome::Invalid(_)
        | noble_kernel::untrusted::Outcome::Unsupported(_)
        | noble_kernel::untrusted::Outcome::Exhausted(_)
        | noble_kernel::untrusted::Outcome::InternalFailure => {
            return Err("expected a work exhaustion".to_string())
        }
    }
    Ok(())
}

#[test]
fn diagnostics_report_order_shape_and_truncation() -> Result<(), String> {
    let env = env()?;
    let program = candidate(
        vec![
            lit_node(noble_kernel::untrusted::Lit::I64(7), vec![]),
            lit_node(
                noble_kernel::untrusted::Lit::Bool(true),
                vec![noble_kernel::types::Ty::I64],
            ),
        ],
        vec![0, 1],
    );
    let ordered = request(
        vec![],
        vec![noble_kernel::types::Ty::Bool, noble_kernel::types::Ty::I64],
        &[],
    );
    match noble_kernel::acceptance::check(&env, &ordered, &program) {
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => {
            assert_eq!(
                diagnostic.constraint,
                noble_kernel::untrusted::Constraint::StackOrder
            );
            assert!(!diagnostic.provenance_available);
        }
        noble_kernel::untrusted::Outcome::Accepted(_)
        | noble_kernel::untrusted::Outcome::Unsupported(_)
        | noble_kernel::untrusted::Outcome::Exhausted(_)
        | noble_kernel::untrusted::Outcome::InternalFailure => {
            return Err("expected an order diagnostic".to_string())
        }
    }
    let mut tiny = ordered.clone();
    tiny.limits.diagnostics = 1;
    match noble_kernel::acceptance::check(&env, &tiny, &program) {
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => {
            assert!(diagnostic.truncated);
            assert!(diagnostic.expected.len() + diagnostic.actual.len() <= 1);
        }
        noble_kernel::untrusted::Outcome::Accepted(_)
        | noble_kernel::untrusted::Outcome::Unsupported(_)
        | noble_kernel::untrusted::Outcome::Exhausted(_)
        | noble_kernel::untrusted::Outcome::InternalFailure => {
            return Err("expected a truncated diagnostic".to_string())
        }
    }
    Ok(())
}

#[test]
fn word_naming_duplicate_and_resource_eligibility() -> Result<(), String> {
    let env = env()?;
    // `dup` requires Data; the fixture resource kind is not Data.
    let mut with_resource = env.clone();
    let maker = noble_kernel::contracts::Definition(u32::try_from(env.len()).unwrap_or(u32::MAX));
    with_resource.defs.push(noble_kernel::words::Scheme {
        var_kinds: vec![noble_kernel::words::VariableKind::Stack],
        stack_in: vec![noble_kernel::shapes::StackPart::Stack(
            noble_kernel::words::Variable(0),
        )],
        stack_out: vec![
            noble_kernel::shapes::StackPart::Stack(noble_kernel::words::Variable(0)),
            noble_kernel::shapes::StackPart::Pattern(noble_kernel::shapes::Pattern::Resource(
                noble_kernel::contracts::FIXTURE_RESOURCE,
            )),
        ],
        effects: vec![],
    });
    with_resource
        .kinds
        .push(noble_kernel::contracts::Behavior::Named);
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let program = candidate(
        vec![
            noble_kernel::untrusted::Node::Invocation {
                def: maker,
                inst: inst(vec![segment(vec![])]),
            },
            noble_kernel::untrusted::Node::Invocation {
                def: DUP,
                inst: inst(vec![segment(vec![]), value(resource.clone())]),
            },
        ],
        vec![0, 1],
    );
    match noble_kernel::acceptance::check(&env, &request(vec![], vec![resource], &[]), &program) {
        noble_kernel::untrusted::Outcome::Invalid(_)
        | noble_kernel::untrusted::Outcome::Unsupported(_) => {}
        noble_kernel::untrusted::Outcome::Accepted(_)
        | noble_kernel::untrusted::Outcome::Exhausted(_)
        | noble_kernel::untrusted::Outcome::InternalFailure => {
            return Err("expected a rejection without the resource definition".to_string())
        }
    }
    match noble_kernel::acceptance::check(
        &with_resource,
        &request(
            vec![],
            vec![noble_kernel::types::Ty::Resource(
                noble_kernel::contracts::FIXTURE_RESOURCE,
            )],
            &[],
        ),
        &program,
    ) {
        noble_kernel::untrusted::Outcome::Invalid(diagnostic) => assert_eq!(
            diagnostic.constraint,
            noble_kernel::untrusted::Constraint::Eligibility(noble_kernel::types::Ty::Resource(
                noble_kernel::contracts::FIXTURE_RESOURCE
            ))
        ),
        noble_kernel::untrusted::Outcome::Accepted(_)
        | noble_kernel::untrusted::Outcome::Unsupported(_)
        | noble_kernel::untrusted::Outcome::Exhausted(_)
        | noble_kernel::untrusted::Outcome::InternalFailure => {
            return Err("expected an eligibility rejection".to_string())
        }
    }
    Ok(())
}
