//! #103: a rejected stack join reports where its two sides first differ, counted from the top,
//! with the words that produced each side. Golden cases for three common mistakes.

fn difference(source: &[u8]) -> Result<noble_contracts::StackDifference, String> {
    let error = noble_contracts::source::Session::new()
        .prepare(source, &[], super::limits())
        .expect_err("a mismatched stack join must not be accepted");
    let join = error
        .diagnostic()
        .join()
        .ok_or("the stack rejection has no structured join")?;
    // The positional difference is added beside the existing text, which stays as it was.
    assert!(error
        .diagnostic()
        .message
        .starts_with("stack/program join: expected "));
    join.difference
        .clone()
        .ok_or_else(|| String::from("the stack rejection has no positional difference"))
}

const fn span(start: u32, end: u32) -> Option<noble_contracts::Span> {
    Some(noble_contracts::Span { start, end })
}

#[test]
fn arity_off_by_one_differs_below_the_shared_top() -> Result<(), String> {
    let difference = difference(b"1 +")?;
    assert_eq!(
        difference,
        noble_contracts::StackDifference {
            shared_prefix: 1,
            expected: String::from("I64"),
            actual: String::from("[]"),
            expected_word: span(2, 3),
            actual_word: span(0, 1),
        }
    );
    assert_eq!(difference.position(), 2);
    Ok(())
}

#[test]
fn swapped_operands_differ_at_the_top() -> Result<(), String> {
    let difference = difference(b"[ 1 ] [ 2 ] true if")?;
    assert_eq!(difference.shared_prefix, 0);
    assert_eq!(difference.position(), 1);
    assert!(
        difference.expected.starts_with("Program<"),
        "if expects a quotation on top: {difference:?}"
    );
    assert_eq!(difference.actual, "Bool");
    assert_eq!(difference.expected_word, span(17, 19));
    assert_eq!(difference.actual_word, span(12, 16));
    Ok(())
}

#[test]
fn quotation_in_place_of_a_value_differs_below_the_shared_top() -> Result<(), String> {
    let difference = difference(b"[ 1 ] 2 +")?;
    assert_eq!(difference.shared_prefix, 1);
    assert_eq!(difference.position(), 2);
    assert_eq!(difference.expected, "I64");
    assert!(
        difference.actual.starts_with("Program<"),
        "the quotation sits where + expects a value: {difference:?}"
    );
    assert_eq!(difference.expected_word, span(8, 9));
    assert_eq!(difference.actual_word, span(6, 7));
    Ok(())
}

#[test]
fn a_mismatch_at_the_start_of_a_body_has_no_producing_word() -> Result<(), String> {
    let difference = difference(b"+")?;
    assert_eq!(difference.shared_prefix, 0);
    assert_eq!(difference.expected, "I64");
    assert_eq!(difference.actual, "[]");
    assert_eq!(difference.expected_word, span(0, 1));
    assert_eq!(difference.actual_word, None);
    Ok(())
}
