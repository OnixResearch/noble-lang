//! Acceptance controls for the fragment checker.
// Names the fragment acceptance contract this crate implements.
// r[impl VT-M2-01]

mod support;

const DUP: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(0);
const ADD: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(4);
const RUN: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(10);
const TEST_EMIT: noble_kernel::contracts::Definition = noble_kernel::contracts::Definition(22);

fn forty_two() -> (
    noble_kernel::untrusted::Request,
    noble_kernel::untrusted::Candidate,
) {
    let program = support::candidate(
        vec![
            support::lit_node(noble_kernel::untrusted::Lit::I64(41), vec![]),
            support::lit_node(
                noble_kernel::untrusted::Lit::I64(1),
                vec![support::i64_ty()],
            ),
            support::invocation(ADD, vec![support::segment(vec![])]),
            support::quote_node(
                vec![1, 2],
                vec![support::i64_ty()],
                vec![support::i64_ty()],
                vec![support::i64_ty()],
                &[],
            ),
            support::invocation(
                RUN,
                vec![
                    support::segment(vec![support::i64_ty()]),
                    support::segment(vec![support::i64_ty()]),
                    support::effect_binding(&[]),
                ],
            ),
        ],
        vec![0, 3, 4],
    );
    (
        support::request(vec![], vec![support::i64_ty()], &[]),
        program,
    )
}

#[test]
fn sequence_and_literals_accept_arithmetic() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let program = support::candidate(
        vec![
            support::lit_node(noble_kernel::untrusted::Lit::I64(41), vec![]),
            support::lit_node(
                noble_kernel::untrusted::Lit::I64(1),
                vec![support::i64_ty()],
            ),
            support::invocation(ADD, vec![support::segment(vec![])]),
        ],
        vec![0, 1, 2],
    );
    let support::Seen::Accepted(checked) = support::seen(
        &env,
        &support::request(vec![], vec![support::i64_ty()], &[]),
        &program,
    )?
    else {
        return Err("expected acceptance".to_string());
    };
    assert_eq!(checked.derivations.len(), 3);
    assert_eq!(checked.interface.stack_out, vec![support::i64_ty()]);
    Ok(())
}

#[test]
fn quotation_construction_checks_body_and_runs() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let (request, program) = forty_two();
    let support::Seen::Accepted(checked) = support::seen(&env, &request, &program)? else {
        return Err("expected acceptance".to_string());
    };
    assert_eq!(checked.derivations.len(), 5);
    Ok(())
}

#[test]
fn hidden_emit_rejects_against_empty_bound() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let program = support::candidate(
        vec![
            support::invocation(TEST_EMIT, vec![support::segment(vec![])]),
            support::quote_node(
                vec![0],
                vec![],
                vec![support::text_ty()],
                vec![support::unit_ty()],
                &[],
            ),
        ],
        vec![1],
    );
    let pushed = noble_kernel::types::Ty::program(
        vec![support::text_ty()],
        vec![support::unit_ty()],
        support::ids(&[]),
    );
    let support::Seen::Bad(diagnostic) = support::seen(
        &env,
        &support::request(vec![], vec![pushed], &[0]),
        &program,
    )?
    else {
        return Err("expected a rejection".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::EffectInclusion(noble_kernel::types::EffId(0))
    );
    Ok(())
}

#[test]
fn limits_boundaries_and_exhaustion() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let (base, program) = forty_two();

    let mut exact = base.clone();
    exact.limits.nodes = 5;
    let support::Seen::Accepted(_) = support::seen(&env, &exact, &program)? else {
        return Err("expected acceptance at the node boundary".to_string());
    };
    let mut over = base.clone();
    over.limits.nodes = 4;
    let support::Seen::Exhausted(limit) = support::seen(&env, &over, &program)? else {
        return Err("expected node exhaustion".to_string());
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Nodes);

    let mut deep = base.clone();
    deep.limits.depth = 1;
    let support::Seen::Accepted(_) = support::seen(&env, &deep, &program)? else {
        return Err("expected acceptance at the depth boundary".to_string());
    };
    let mut shallow = base.clone();
    shallow.limits.depth = 0;
    let support::Seen::Exhausted(limit) = support::seen(&env, &shallow, &program)? else {
        return Err("expected depth exhaustion".to_string());
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Depth);

    let single = support::candidate(
        vec![support::lit_node(
            noble_kernel::untrusted::Lit::Unit,
            vec![],
        )],
        vec![0],
    );
    let mut work = support::request(vec![], vec![support::unit_ty()], &[]);
    work.limits.work = 8;
    let support::Seen::Accepted(_) = support::seen(&env, &work, &single)? else {
        return Err("expected acceptance at the work boundary".to_string());
    };
    work.limits.work = 4;
    let support::Seen::Exhausted(limit) = support::seen(&env, &work, &single)? else {
        return Err("expected work exhaustion".to_string());
    };
    assert_eq!(limit, noble_kernel::untrusted::LimitKind::Work);
    Ok(())
}

#[test]
fn diagnostics_report_order_shape_and_truncation() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let program = support::candidate(
        vec![
            support::lit_node(noble_kernel::untrusted::Lit::I64(7), vec![]),
            support::lit_node(
                noble_kernel::untrusted::Lit::Bool(true),
                vec![support::i64_ty()],
            ),
        ],
        vec![0, 1],
    );
    let ordered = support::request(vec![], vec![support::bool_ty(), support::i64_ty()], &[]);
    let support::Seen::Bad(diagnostic) = support::seen(&env, &ordered, &program)? else {
        return Err("expected an order diagnostic".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::StackOrder
    );
    assert_eq!(
        diagnostic.expected,
        vec![support::bool_ty(), support::i64_ty()]
    );
    assert_eq!(
        diagnostic.actual,
        vec![support::i64_ty(), support::bool_ty()]
    );

    let mut tiny = ordered.clone();
    tiny.limits.diagnostics = 1;
    let support::Seen::Bad(diagnostic) = support::seen(&env, &tiny, &program)? else {
        return Err("expected a truncated diagnostic".to_string());
    };
    assert!(diagnostic.truncated);
    assert!(diagnostic.expected.len() + diagnostic.actual.len() <= 1);
    Ok(())
}

#[test]
fn resource_eligibility_rejects_duplication() -> Result<(), String> {
    let mut env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    assert_eq!(env.defs.len(), 23);
    let maker = noble_kernel::contracts::Definition(23);
    env.defs.push(noble_kernel::words::Scheme {
        var_kinds: vec![noble_kernel::words::VariableKind::Stack],
        stack_in: vec![noble_kernel::shapes::Pattern::StackVar(
            noble_kernel::words::Variable(0),
        )],
        stack_out: vec![
            noble_kernel::shapes::Pattern::StackVar(noble_kernel::words::Variable(0)),
            noble_kernel::shapes::Pattern::Resource(noble_kernel::contracts::FIXTURE_RESOURCE),
        ],
        effects: vec![],
    });
    env.kinds.push(noble_kernel::contracts::Behavior::Named);
    env.deps.push(Vec::new());
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let program = support::candidate(
        vec![
            support::invocation(maker, vec![support::segment(vec![])]),
            support::invocation(
                DUP,
                vec![support::segment(vec![]), support::value(resource.clone())],
            ),
        ],
        vec![0, 1],
    );
    let support::Seen::Bad(diagnostic) = support::seen(
        &env,
        &support::request(vec![], vec![resource.clone()], &[]),
        &program,
    )?
    else {
        return Err("expected an eligibility rejection".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::Eligibility(resource)
    );
    Ok(())
}

/// Helpers for the per-word control table (fragment v1: the complete
/// 23-entry bootstrap table).
mod words {
    use super::support;

    pub fn prog(
        input: Vec<noble_kernel::types::Ty>,
        output: Vec<noble_kernel::types::Ty>,
        effects: &[u32],
    ) -> noble_kernel::types::Ty {
        noble_kernel::types::Ty::program(input, output, support::ids(effects))
    }

    pub fn pair_of(
        left: noble_kernel::types::Ty,
        right: noble_kernel::types::Ty,
    ) -> noble_kernel::types::Ty {
        noble_kernel::types::Ty::Pair(Box::new(left), Box::new(right))
    }

    pub fn sum_of(
        left: noble_kernel::types::Ty,
        right: noble_kernel::types::Ty,
    ) -> noble_kernel::types::Ty {
        noble_kernel::types::Ty::Sum(Box::new(left), Box::new(right))
    }

    pub fn list_of(item: noble_kernel::types::Ty) -> noble_kernel::types::Ty {
        noble_kernel::types::Ty::List(Box::new(item))
    }
}

/// One word's controls: its definition index, one accepting witness at a
/// concrete interface, and one rejecting witness with the constraint the
/// rejection must name.
struct WordControl {
    def: u32,
    /// The entry stack the positive fixture provides.
    stack_in: Vec<noble_kernel::types::Ty>,
    /// The positive witness.
    positive: Vec<noble_kernel::words::Binding>,
    /// The positive result stack.
    stack_out: Vec<noble_kernel::types::Ty>,
    /// The positive allowed effects.
    allowed: &'static [u32],
    /// The rejection's entry stack.
    reject_stack_in: Vec<noble_kernel::types::Ty>,
    /// The rejecting witness.
    reject: Vec<noble_kernel::words::Binding>,
    /// The constraint the rejection names.
    constraint: noble_kernel::untrusted::Constraint,
}

/// The per-word control table: every entry of the 23-word bootstrap table
/// with one positive and one rejection fixture (task 2.3, B-SCOPE-01).
fn word_controls() -> Vec<WordControl> {
    use noble_kernel::types::Ty::{Bool, Text, Unit, I64};
    use noble_kernel::untrusted::Constraint::{EffectInclusion, Eligibility, StackJoin};
    use noble_kernel::words::Binding::Value as V;
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let eff = support::effect_binding;
    let seg = support::segment;
    vec![
        WordControl {
            def: 0,
            stack_in: vec![I64],
            positive: vec![seg(vec![]), V(I64)],
            stack_out: vec![I64, I64],
            allowed: &[],
            reject_stack_in: vec![I64],
            reject: vec![seg(vec![]), V(resource.clone())],
            constraint: Eligibility(resource.clone()),
        },
        WordControl {
            def: 1,
            stack_in: vec![I64],
            positive: vec![seg(vec![]), V(I64)],
            stack_out: vec![],
            allowed: &[],
            reject_stack_in: vec![resource.clone()],
            reject: vec![seg(vec![]), V(resource.clone())],
            constraint: Eligibility(resource.clone()),
        },
        WordControl {
            def: 2,
            stack_in: vec![I64, Bool],
            positive: vec![seg(vec![]), V(I64), V(Bool)],
            stack_out: vec![Bool, I64],
            allowed: &[],
            reject_stack_in: vec![I64],
            reject: vec![seg(vec![]), V(I64), V(Bool)],
            constraint: StackJoin,
        },
        WordControl {
            def: 3,
            stack_in: vec![I64, words::prog(vec![], vec![Unit], &[])],
            // `dip`'s branch consumes the same stack tail `S` it runs
            // under: with `S = []` the program reads `[] -- [Unit]`.
            positive: vec![seg(vec![]), V(I64), seg(vec![Unit]), eff(&[])],
            stack_out: vec![Unit, I64],
            allowed: &[],
            reject_stack_in: vec![I64, words::prog(vec![I64], vec![Unit], &[])],
            reject: vec![seg(vec![]), V(resource.clone()), seg(vec![Unit]), eff(&[])],
            constraint: StackJoin,
        },
        WordControl {
            def: 4,
            stack_in: vec![I64, I64],
            positive: vec![seg(vec![])],
            stack_out: vec![I64],
            allowed: &[],
            reject_stack_in: vec![I64, Bool],
            reject: vec![seg(vec![])],
            constraint: StackJoin,
        },
        WordControl {
            def: 5,
            stack_in: vec![I64, I64],
            positive: vec![seg(vec![])],
            stack_out: vec![I64],
            allowed: &[],
            reject_stack_in: vec![I64, Bool],
            reject: vec![seg(vec![])],
            constraint: StackJoin,
        },
        WordControl {
            def: 6,
            stack_in: vec![I64, I64],
            positive: vec![seg(vec![])],
            stack_out: vec![I64],
            allowed: &[],
            reject_stack_in: vec![I64, Bool],
            reject: vec![seg(vec![])],
            constraint: StackJoin,
        },
        WordControl {
            def: 7,
            stack_in: vec![I64, I64],
            positive: vec![seg(vec![])],
            stack_out: vec![Bool],
            allowed: &[],
            reject_stack_in: vec![I64, Bool],
            reject: vec![seg(vec![])],
            constraint: StackJoin,
        },
        WordControl {
            def: 8,
            stack_in: vec![I64],
            positive: vec![seg(vec![]), V(I64), seg(vec![I64])],
            stack_out: vec![words::prog(vec![I64], vec![I64, I64], &[])],
            allowed: &[],
            reject_stack_in: vec![resource.clone()],
            reject: vec![
                seg(vec![]),
                V(resource.clone()),
                seg(vec![resource.clone()]),
            ],
            constraint: Eligibility(resource.clone()),
        },
        WordControl {
            def: 9,
            stack_in: vec![
                words::prog(vec![], vec![I64], &[]),
                words::prog(vec![I64], vec![Unit], &[]),
            ],
            positive: vec![
                seg(vec![]),
                seg(vec![]),
                seg(vec![I64]),
                seg(vec![Unit]),
                eff(&[]),
                eff(&[]),
            ],
            stack_out: vec![words::prog(vec![], vec![Unit], &[])],
            allowed: &[],
            reject_stack_in: vec![
                words::prog(vec![], vec![I64], &[]),
                words::prog(vec![Bool], vec![Unit], &[]),
            ],
            reject: vec![
                seg(vec![]),
                seg(vec![]),
                seg(vec![I64]),
                seg(vec![Unit]),
                eff(&[]),
                eff(&[]),
            ],
            constraint: StackJoin,
        },
        WordControl {
            def: 10,
            stack_in: vec![I64, words::prog(vec![I64], vec![Unit], &[])],
            positive: vec![seg(vec![I64]), seg(vec![Unit]), eff(&[])],
            stack_out: vec![Unit],
            allowed: &[],
            reject_stack_in: vec![I64, words::prog(vec![I64], vec![Unit], &[])],
            reject: vec![seg(vec![I64]), seg(vec![Unit]), eff(&[0])],
            constraint: StackJoin,
        },
        WordControl {
            def: 11,
            stack_in: vec![words::prog(vec![], vec![], &[])],
            positive: vec![seg(vec![]), seg(vec![]), seg(vec![]), eff(&[])],
            stack_out: vec![noble_kernel::types::Ty::Syntax],
            allowed: &[],
            reject_stack_in: vec![I64],
            reject: vec![seg(vec![]), seg(vec![]), seg(vec![]), eff(&[])],
            constraint: StackJoin,
        },
        WordControl {
            def: 12,
            stack_in: vec![],
            positive: vec![seg(vec![])],
            stack_out: vec![Unit],
            allowed: &[],
            reject_stack_in: vec![Unit],
            reject: vec![seg(vec![Unit])],
            constraint: StackJoin,
        },
        WordControl {
            def: 13,
            stack_in: vec![I64, Bool],
            positive: vec![seg(vec![]), V(I64), V(Bool)],
            stack_out: vec![words::pair_of(I64, Bool)],
            allowed: &[],
            reject_stack_in: vec![I64],
            reject: vec![seg(vec![]), V(I64), V(Bool)],
            constraint: StackJoin,
        },
        WordControl {
            def: 14,
            stack_in: vec![words::pair_of(I64, Bool)],
            positive: vec![seg(vec![]), V(I64), V(Bool)],
            stack_out: vec![I64, Bool],
            allowed: &[],
            reject_stack_in: vec![I64],
            reject: vec![seg(vec![]), V(I64), V(Bool)],
            constraint: StackJoin,
        },
        WordControl {
            def: 15,
            stack_in: vec![I64],
            positive: vec![seg(vec![]), V(I64), V(Bool)],
            stack_out: vec![words::sum_of(I64, Bool)],
            allowed: &[],
            reject_stack_in: vec![Text],
            reject: vec![seg(vec![]), V(I64), V(Bool)],
            constraint: StackJoin,
        },
        WordControl {
            def: 16,
            stack_in: vec![Bool],
            positive: vec![seg(vec![]), V(I64), V(Bool)],
            stack_out: vec![words::sum_of(I64, Bool)],
            allowed: &[],
            reject_stack_in: vec![Text],
            reject: vec![seg(vec![]), V(I64), V(Bool)],
            constraint: StackJoin,
        },
        WordControl {
            def: 17,
            stack_in: vec![
                words::sum_of(I64, Bool),
                words::prog(vec![I64], vec![Unit], &[]),
                words::prog(vec![Bool], vec![Unit], &[]),
            ],
            positive: vec![
                seg(vec![]),
                V(I64),
                V(Bool),
                seg(vec![Unit]),
                eff(&[]),
                eff(&[]),
            ],
            stack_out: vec![Unit],
            allowed: &[],
            reject_stack_in: vec![
                words::sum_of(I64, Bool),
                words::prog(vec![I64], vec![Unit], &[]),
                words::prog(vec![Bool], vec![I64], &[]),
            ],
            reject: vec![
                seg(vec![]),
                V(I64),
                V(Bool),
                seg(vec![Unit]),
                eff(&[]),
                eff(&[]),
            ],
            constraint: StackJoin,
        },
        WordControl {
            def: 18,
            stack_in: vec![
                Bool,
                words::prog(vec![], vec![Unit], &[]),
                words::prog(vec![], vec![Unit], &[]),
            ],
            positive: vec![seg(vec![]), seg(vec![Unit]), eff(&[]), eff(&[])],
            stack_out: vec![Unit],
            allowed: &[],
            reject_stack_in: vec![
                Bool,
                words::prog(vec![], vec![Unit], &[]),
                words::prog(vec![], vec![Unit], &[]),
            ],
            reject: vec![seg(vec![]), seg(vec![Unit]), eff(&[0]), eff(&[0])],
            constraint: StackJoin,
        },
        WordControl {
            def: 19,
            stack_in: vec![],
            positive: vec![seg(vec![]), V(I64)],
            stack_out: vec![words::list_of(I64)],
            allowed: &[],
            reject_stack_in: vec![],
            reject: vec![seg(vec![]), V(I64), V(I64)],
            constraint: noble_kernel::untrusted::Constraint::InstantiationArity,
        },
        WordControl {
            def: 20,
            stack_in: vec![I64, words::list_of(I64)],
            positive: vec![seg(vec![]), V(I64)],
            stack_out: vec![words::list_of(I64)],
            allowed: &[],
            reject_stack_in: vec![I64, words::list_of(Bool)],
            reject: vec![seg(vec![]), V(I64)],
            constraint: StackJoin,
        },
        WordControl {
            def: 21,
            stack_in: vec![
                words::list_of(I64),
                words::prog(vec![], vec![Unit], &[]),
                words::prog(vec![I64, words::list_of(I64)], vec![Unit], &[]),
            ],
            positive: vec![seg(vec![]), V(I64), seg(vec![Unit]), eff(&[]), eff(&[])],
            stack_out: vec![Unit],
            allowed: &[],
            reject_stack_in: vec![
                words::list_of(I64),
                words::prog(vec![], vec![Unit], &[]),
                words::prog(vec![I64, words::list_of(I64)], vec![I64], &[]),
            ],
            reject: vec![seg(vec![]), V(I64), seg(vec![Unit]), eff(&[]), eff(&[])],
            constraint: StackJoin,
        },
        WordControl {
            def: 22,
            stack_in: vec![Text],
            positive: vec![seg(vec![])],
            stack_out: vec![Unit],
            allowed: &[0],
            reject_stack_in: vec![Text],
            reject: vec![seg(vec![])],
            constraint: EffectInclusion(noble_kernel::types::EffId(0)),
        },
    ]
}

/// Every bootstrap word has one accepting and one rejecting control, and
/// the table is exactly the 23-entry definition order.
// r[verify VT-M3-02]
#[test]
fn every_word_has_a_positive_and_a_rejection_control() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    assert_eq!(env.defs.len(), 23);
    let controls = word_controls();
    assert_eq!(controls.len(), 23);
    let mut index = 0;
    while index < controls.len() {
        let control = &controls[index];
        let def = noble_kernel::contracts::Definition(control.def);
        let program = support::candidate(
            vec![support::invocation(def, control.positive.clone())],
            vec![0],
        );
        let support::Seen::Accepted(checked) = support::seen(
            &env,
            &support::request(
                control.stack_in.clone(),
                control.stack_out.clone(),
                control.allowed,
            ),
            &program,
        )?
        else {
            return Err(format!(
                "word {} must accept its positive control",
                control.def
            ));
        };
        assert_eq!(checked.interface.stack_out, control.stack_out);
        let broken = support::candidate(
            vec![support::invocation(def, control.reject.clone())],
            vec![0],
        );
        let support::Seen::Bad(diagnostic) = support::seen(
            &env,
            &support::request(
                control.reject_stack_in.clone(),
                control.stack_out.clone(),
                &[],
            ),
            &broken,
        )?
        else {
            return Err(format!(
                "word {} must reject its negative control",
                control.def
            ));
        };
        assert_eq!(
            diagnostic.constraint, control.constraint,
            "word {} names its violated constraint",
            control.def
        );
        index += 1;
    }
    Ok(())
}

/// B-CHECK-03: a duplicated first-class program value shares one
/// instantiated interface — the second use cannot claim a fresh
/// instantiation against the same empty stack.
// r[verify VT-M3-01]
#[test]
fn duplicated_program_value_shares_one_interface() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let program_value = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
        noble_kernel::types::EffSet::empty(),
    );
    // `[ B ] : R -- R Program<A,C,e>` with an empty body deriving
    // `[I64] -- [I64]`.
    let quotation = support::quote_node(
        vec![],
        vec![],
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::I64],
        &[],
    );
    let program = support::candidate(
        vec![
            quotation,
            support::invocation(
                DUP,
                vec![
                    support::segment(vec![]),
                    support::value(program_value.clone()),
                ],
            ),
            support::invocation(
                RUN,
                vec![
                    support::segment(vec![]),
                    support::segment(vec![noble_kernel::types::Ty::I64]),
                    support::effect_binding(&[]),
                ],
            ),
            support::invocation(
                RUN,
                vec![
                    support::segment(vec![]),
                    support::segment(vec![noble_kernel::types::Ty::I64]),
                    support::effect_binding(&[]),
                ],
            ),
        ],
        vec![0, 1, 2, 3],
    );
    let support::Seen::Bad(diagnostic) = support::seen(
        &env,
        &support::request(
            vec![],
            vec![noble_kernel::types::Ty::I64, program_value.clone()],
            &[],
        ),
        &program,
    )?
    else {
        return Err("the duplicated program value must reject at its second run".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::StackJoin,
        "the second copy cannot re-instantiate against the same stack"
    );
    Ok(())
}

/// B-CHECK-06: eliminating `Sum<Resource<R>, I64>` exposes the resource
/// payload at its own type inside the branch program (the positive
/// control), while the sum itself stays non-`Data` and cannot be
/// duplicated (the negative control).
// r[verify VT-M3-01]
#[test]
fn sum_resource_payload_exposes_at_its_own_type() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
    let sum = noble_kernel::types::Ty::Sum(
        Box::new(resource.clone()),
        Box::new(noble_kernel::types::Ty::I64),
    );
    let case = noble_kernel::contracts::Definition(17);
    // The exposed branch consumes the resource payload directly.
    let payload_branch = noble_kernel::types::Ty::program(
        vec![resource.clone()],
        vec![noble_kernel::types::Ty::Unit],
        noble_kernel::types::EffSet::empty(),
    );
    let quiet_branch = noble_kernel::types::Ty::program(
        vec![noble_kernel::types::Ty::I64],
        vec![noble_kernel::types::Ty::Unit],
        noble_kernel::types::EffSet::empty(),
    );
    let program = support::candidate(
        vec![support::invocation(
            case,
            vec![
                support::segment(vec![]),
                support::value(resource.clone()),
                support::value(noble_kernel::types::Ty::I64),
                support::segment(vec![noble_kernel::types::Ty::Unit]),
                support::effect_binding(&[]),
                support::effect_binding(&[]),
            ],
        )],
        vec![0],
    );
    let support::Seen::Accepted(checked) = support::seen(
        &env,
        &support::request(
            vec![sum.clone(), payload_branch, quiet_branch],
            vec![noble_kernel::types::Ty::Unit],
            &[],
        ),
        &program,
    )?
    else {
        return Err("case must accept the payload-exposure control".to_string());
    };
    // The exposed interface carries the payload branch verbatim: the
    // resource stands at its own type, never widened to `Data`.
    assert_eq!(
        checked.interface.stack_in[1].clone(),
        noble_kernel::types::Ty::program(
            vec![resource.clone()],
            vec![noble_kernel::types::Ty::Unit],
            noble_kernel::types::EffSet::empty(),
        )
    );

    // The sum stays non-Data: duplicating it violates eligibility.
    let duplicate = support::candidate(
        vec![support::invocation(
            DUP,
            vec![support::segment(vec![]), support::value(sum.clone())],
        )],
        vec![0],
    );
    let support::Seen::Bad(diagnostic) = support::seen(
        &env,
        &support::request(vec![sum.clone()], vec![sum.clone(), sum.clone()], &[]),
        &duplicate,
    )?
    else {
        return Err("the resource-bearing sum must not be duplicable".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::Eligibility(sum)
    );
    Ok(())
}

/// B-CHECK-06: the eliminators take the real union of the branch bounds —
// r[verify VT-M3-01]
// r[verify VT-M3-01]
#[test]
fn eliminator_union_is_explicit_not_implicit() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let if_word = noble_kernel::contracts::Definition(18);
    let test_emit = noble_kernel::contracts::Definition(22);
    let unit = noble_kernel::types::Ty::Unit;
    // Branch one derives `[] -- [Unit]` carrying `test.emit` (latent `{0}`);
    // branch two derives the same interface quietly. The union bound the
    // `if` word derives is `{0}`, and the request allows nothing — a
    // checker that implicitly joined to the quiet branch's `{}` would
    // accept; the real union rejects.
    let emitting_value =
        noble_kernel::types::Ty::program(vec![], vec![unit.clone()], support::ids(&[0]));
    let program = support::candidate(
        vec![
            support::lit_node(noble_kernel::untrusted::Lit::Bool(true), vec![]),
            support::quote_node(
                vec![2, 3],
                vec![noble_kernel::types::Ty::Bool],
                vec![],
                vec![unit.clone()],
                &[0],
            ),
            support::lit_node(noble_kernel::untrusted::Lit::Text, vec![]),
            support::invocation(test_emit, vec![support::segment(vec![])]),
            support::quote_node(
                vec![5],
                vec![noble_kernel::types::Ty::Bool, emitting_value.clone()],
                vec![],
                vec![unit.clone()],
                &[],
            ),
            support::lit_node(noble_kernel::untrusted::Lit::Unit, vec![]),
            support::invocation(
                if_word,
                vec![
                    support::segment(vec![]),
                    support::segment(vec![unit.clone()]),
                    support::effect_binding(&[0]),
                    support::effect_binding(&[]),
                ],
            ),
        ],
        vec![0, 1, 4, 6],
    );
    let support::Seen::Bad(diagnostic) = support::seen(
        &env,
        &support::request(vec![], vec![unit.clone()], &[]),
        &program,
    )?
    else {
        return Err("the union bound must reject against the empty allowance".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::EffectInclusion(noble_kernel::types::EffId(0)),
        "the quiet branch cannot implicitly absorb the emitting branch's bound"
    );
    Ok(())
}

/// B-CHECK-06: no trusted refinements — the eliminator's witness must match
/// the eliminated structure exactly; a payload-order refinement
/// (`Sum<I64,Bool>` read as `Sum<Bool,I64>`) is a stack-join rejection,
/// not an identification.
// r[verify VT-M3-01]
#[test]
fn advertised_refinement_is_not_trusted() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    let case = noble_kernel::contracts::Definition(17);
    let unit = noble_kernel::types::Ty::Unit;
    let (i64_ty, bool_ty) = (noble_kernel::types::Ty::I64, noble_kernel::types::Ty::Bool);
    // The stack holds `Sum<I64, Bool>`; the witness advertises the
    // refinement `Sum<Bool, I64>` with branches swapped to match.
    let actual_sum =
        noble_kernel::types::Ty::Sum(Box::new(i64_ty.clone()), Box::new(bool_ty.clone()));
    let refined_left = noble_kernel::types::Ty::program(
        vec![bool_ty.clone()],
        vec![unit.clone()],
        noble_kernel::types::EffSet::empty(),
    );
    let refined_right = noble_kernel::types::Ty::program(
        vec![i64_ty.clone()],
        vec![unit.clone()],
        noble_kernel::types::EffSet::empty(),
    );
    let program = support::candidate(
        vec![support::invocation(
            case,
            vec![
                support::segment(vec![]),
                support::value(bool_ty.clone()),
                support::value(i64_ty.clone()),
                support::segment(vec![unit.clone()]),
                support::effect_binding(&[]),
                support::effect_binding(&[]),
            ],
        )],
        vec![0],
    );
    let support::Seen::Bad(diagnostic) = support::seen(
        &env,
        &support::request(
            vec![actual_sum, refined_left, refined_right],
            vec![unit.clone()],
            &[],
        ),
        &program,
    )?
    else {
        return Err("the payload-swapped refinement must reject".to_string());
    };
    assert_eq!(
        diagnostic.constraint,
        noble_kernel::untrusted::Constraint::StackJoin,
        "the checker identifies no refinement of the advertised sum"
    );
    Ok(())
}
