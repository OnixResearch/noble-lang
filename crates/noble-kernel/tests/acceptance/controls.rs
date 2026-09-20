#[path = "controls/data.rs"]
mod data;
#[path = "controls/eliminators.rs"]
mod eliminators;
#[path = "controls/primitives.rs"]
mod primitives;
#[path = "controls/programs.rs"]
mod programs;

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
fn bootstrap() -> Vec<WordControl> {
    vec![
        primitives::duplicate(),
        primitives::discard(),
        primitives::swap(),
        programs::dip(),
        primitives::integer_binary(4, noble_kernel::types::Ty::I64),
        primitives::integer_binary(5, noble_kernel::types::Ty::I64),
        primitives::integer_binary(6, noble_kernel::types::Ty::I64),
        primitives::integer_binary(7, noble_kernel::types::Ty::Bool),
        programs::capture(),
        programs::compose(),
        programs::run(),
        programs::inspect(),
        data::unit(),
        data::pair(),
        data::unpair(),
        data::left(),
        data::right(),
        eliminators::case(),
        eliminators::conditional(),
        data::nil(),
        data::cons(),
        eliminators::list_case(),
        primitives::emit(),
    ]
}

/// Every bootstrap word has one accepting and one rejecting control, and
/// the table is exactly the 23-entry definition order.
// r[verify VT-M3-02]
#[test]
fn every_word_has_a_positive_and_a_rejection_control() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|defect| format!("{defect:?}"))?;
    assert_eq!(env.defs.len(), 23);
    let controls = bootstrap();
    assert_eq!(controls.len(), 23);
    let mut index = 0;
    while index < controls.len() {
        let control = &controls[index];
        let def = noble_kernel::contracts::Definition(control.def);
        let program = crate::support::candidate(
            vec![crate::support::invocation(def, control.positive.clone())],
            vec![0],
        );
        let crate::support::Seen::Accepted(checked) = crate::support::seen(
            &env,
            &crate::support::request(
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
        let broken = crate::support::candidate(
            vec![crate::support::invocation(def, control.reject.clone())],
            vec![0],
        );
        let crate::support::Seen::Bad(diagnostic) = crate::support::seen(
            &env,
            &crate::support::request(
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

/// Helpers for the per-word control table (fragment v1: the complete
/// 23-entry bootstrap table).
mod words {
    pub(super) fn prog(
        input: Vec<noble_kernel::types::Ty>,
        output: Vec<noble_kernel::types::Ty>,
        effects: &[u32],
    ) -> noble_kernel::types::Ty {
        noble_kernel::types::Ty::program(input, output, crate::support::ids(effects))
    }

    pub(super) fn pair_of(
        left: noble_kernel::types::Ty,
        right: noble_kernel::types::Ty,
    ) -> noble_kernel::types::Ty {
        noble_kernel::types::Ty::Pair(Box::new(left), Box::new(right))
    }

    pub(super) fn sum_of(
        left: noble_kernel::types::Ty,
        right: noble_kernel::types::Ty,
    ) -> noble_kernel::types::Ty {
        noble_kernel::types::Ty::Sum(Box::new(left), Box::new(right))
    }

    pub(super) fn list_of(item: noble_kernel::types::Ty) -> noble_kernel::types::Ty {
        noble_kernel::types::Ty::List(Box::new(item))
    }
}
