use super::evaluator::evaluate_program;
use super::*;

#[test]
fn nonheldout_three_input_stack_evaluator_has_refusals() -> Result<(), String> {
    assert_eq!(evaluate_program("+ -", [5, -5, 7])?, 3);
    assert_eq!(evaluate_program("- 7 + 7 + +", [5, -5, 7])?, 7);
    assert!(evaluate_program("42 + +", [0, 0, 0]).is_err());
    assert!(evaluate_program("dup dup", [1, 2, 3]).is_err());
    assert!(evaluate_program("= +", [1, 2, 3]).is_err());
    Ok(())
}
