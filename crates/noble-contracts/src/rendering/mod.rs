//! Deterministic export of the retained acceptance subject and resolved claim.
//! No producer text is interpolated into Lean syntax.

#![expect(
    tigerstyle::mutating_input_in_pure,
    reason = "Owner: noble-maintainers; export_lean alone allocates the mutable output String; helper appends are bounded by the prepared candidate and expression arenas, 256-constructor types, and at most 20 decimal digits per number; the borrowed Prepared and its retained acceptance state remain unchanged."
)]

mod expression;
mod subject;
mod types;

/// Export an immutable, successfully prepared contract. The shell must retain
/// and independently recheck this exact module, not a producer's replacement.
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; Prepared is immutable and privately constructed only after ordinary acceptance and predicate resolution; rendering introduces no unchecked producer text."
)]
pub fn export_lean(prepared: &crate::Prepared) -> alloc::string::String {
    let mut out = alloc::string::String::with_capacity(4096);
    out.push_str("import NobleContracts\nimport NobleContracts.Expression\n\n");
    out.push_str("open NobleContracts\nnamespace MC1Obligation\n\n");
    out.push_str("def irRevision : Nat := 1\ndef semanticRevision : Nat := ");
    number(&mut out, u64::from(prepared.candidate().revision));
    out.push('\n');
    subject::emit(&mut out, prepared.candidate());
    types::named_stack(&mut out, "inputTypes", prepared.inputs());
    types::named_stack(&mut out, "outputTypes", prepared.outputs());
    types::named_stack(&mut out, "paramTypes", prepared.params());
    expression::emit(&mut out, prepared);
    out.push_str("def precondition : Term := expression_");
    number(&mut out, u64::from(prepared.requires()));
    out.push_str("\ndef postcondition : Term := expression_");
    number(&mut out, u64::from(prepared.ensures()));
    out.push_str("\ndef claim : Prop := exportedClaim program inputTypes outputTypes paramTypes\n");
    out.push_str("  (Holds precondition) (Holds postcondition)\n\nend MC1Obligation\n");
    out
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers. Appending decimal digits requires the caller's growable String, not a fixed str slice."
)]
#[expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-maintainers. Division by ten strictly decreases multi-digit u64 values, bounding decimal serialization to 20 frames without a temporary buffer."
)]
/// Append the decimal digits of `value`: exactly its `{value}` rendering.
pub(crate) fn number(out: &mut alloc::string::String, value: u64) {
    // Division strictly decreases non-single-digit inputs: at most 20 frames.
    if value >= 10 {
        number(out, value / 10);
    }
    // A decimal remainder selects its scalar directly, without narrowing.
    out.push(match value % 10 {
        0 => '0',
        1 => '1',
        2 => '2',
        3 => '3',
        4 => '4',
        5 => '5',
        6 => '6',
        7 => '7',
        8 => '8',
        _ => '9',
    });
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers. Appending signed integer syntax requires the caller's growable String, not a fixed str slice."
)]
fn signed(out: &mut alloc::string::String, value: i64) {
    out.push('(');
    integer(out, value);
    out.push(')');
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers. Appending integer syntax requires the caller's growable String, not a fixed str slice."
)]
/// Append `value` in decimal with a leading `-` when it is negative: exactly
/// its `{value}` rendering, without the formatting machinery.
pub(crate) fn integer(out: &mut alloc::string::String, value: i64) {
    if value < 0 {
        out.push('-');
    }
    number(out, value.unsigned_abs());
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers. Appending Lean failure syntax requires the caller's growable String, not a fixed str slice."
)]
fn rejected(out: &mut alloc::string::String, message: &str) {
    // Keep quotes as characters: the pinned Aeneas string printer does not
    // escape embedded quotes. This still emits an explicit Lean failure.
    out.push_str("(by fail ");
    out.push('"');
    out.push_str(message);
    out.push('"');
    out.push(')');
}

#[cfg(test)]
mod tests {
    /// The digit renderers produce exactly the `format!` decimal text.
    #[test]
    fn decimals_match_the_formatted_text() {
        for value in [
            0,
            1,
            9,
            10,
            99,
            100,
            4_096,
            u64::from(u32::MAX),
            18_446_744_073_709_551_614,
            u64::MAX,
        ] {
            let mut out = alloc::string::String::from("x");
            super::number(&mut out, value);
            assert_eq!(out, alloc::format!("x{value}"));
        }
        for value in [
            i64::MIN,
            -9_223_372_036_854_775_807,
            -100,
            -10,
            -9,
            -1,
            0,
            1,
            9,
            10,
            i64::MAX,
        ] {
            let mut out = alloc::string::String::from("x");
            super::integer(&mut out, value);
            assert_eq!(out, alloc::format!("x{value}"));
            let mut signed = alloc::string::String::new();
            super::signed(&mut signed, value);
            assert_eq!(signed, alloc::format!("({value})"));
        }
    }
}
