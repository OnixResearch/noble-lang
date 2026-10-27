//! Rust-only regression evidence for K-NUM-01, not execution of CORE-14.

// r[verify K-NUM-01]
#[test]
fn signed_representation_round_trips() {
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        assert_eq!(noble_kernel::integer::I64::from_i64(value).to_i64(), value);
    }
}

#[test]
fn addition_matches_signed_wrap_boundaries() {
    let cases = [
        (41, 1, 42),
        (i64::MAX, 1, i64::MIN),
        (i64::MIN, -1, i64::MAX),
    ];
    for (deeper, top, expected) in cases {
        let actual = noble_kernel::integer::I64::from_i64(deeper)
            .wrapping_add(noble_kernel::integer::I64::from_i64(top));
        assert_eq!(actual.to_i64(), expected);
    }
}

#[test]
fn subtraction_preserves_operand_order() {
    let cases = [
        (7, 3, 4),
        (3, 7, -4),
        (i64::MIN, 1, i64::MAX),
        (i64::MAX, -1, i64::MIN),
    ];
    for (deeper, top, expected) in cases {
        let actual = noble_kernel::integer::I64::from_i64(deeper)
            .wrapping_sub(noble_kernel::integer::I64::from_i64(top));
        assert_eq!(actual.to_i64(), expected);
    }
}

#[test]
fn multiplication_wraps_instead_of_trapping_or_saturating() {
    let cases = [
        (6, 7, 42),
        (i64::MAX, 2, -2),
        (i64::MIN, -1, i64::MIN),
        (i64::MIN, 2, 0),
        (i64::MAX, 0, 0),
    ];
    for (deeper, top, expected) in cases {
        let actual = noble_kernel::integer::I64::from_i64(deeper)
            .wrapping_mul(noble_kernel::integer::I64::from_i64(top));
        assert_eq!(actual.to_i64(), expected);
    }
}

#[test]
fn equality_accepts_matching_values() {
    for value in [i64::MIN, -1, 0, 1, i64::MAX] {
        assert!(
            noble_kernel::integer::I64::from_i64(value)
                .is_equal(noble_kernel::integer::I64::from_i64(value))
        );
    }
}

#[test]
fn equality_rejects_distinct_values() {
    for (left, right) in [(0, 1), (-1, 1), (i64::MIN, i64::MAX)] {
        assert!(
            !noble_kernel::integer::I64::from_i64(left)
                .is_equal(noble_kernel::integer::I64::from_i64(right))
        );
    }
}
