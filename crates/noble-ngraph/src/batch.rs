//! Bounded CPU batch evaluation of independent rows (issue #41).
//!
//! The reference model is `proofs/foundation/NobleFoundation/BatchRows.lean`: a row is one wrapping
//! `I64` operation over two explicit operands, and a batch is the *ordered map* of independent
//! single-row runs, so partial progress is the ordered map over the prefix and a refused row can
//! never change another row's result.
//!
//! This module is the tested implementation of that shape, not a proof of it. Rows delegate to the
//! same `sem::i64_*` reference operations the single-row paths use, so a row's result is the
//! documented K-NUM-01 wrapping result rather than a second evaluator. The model's literal-range
//! refusals cannot occur in `i64`, which is why `evaluate` is total; the refusal rows of the model
//! are therefore **not** exercised here and no correspondence claim covers them.

/// One batch row: one wrapping `I64` operation over two explicit operands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    /// The operation, applied to both operands in the documented order.
    pub op: RowOp,
    /// The deeper operand (the left operand of the machine's stack order).
    pub deeper: i64,
    /// The top operand.
    pub top: i64,
}

/// The wrapping operations a batch row admits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowOp {
    /// Wrapping `a + b`.
    Add,
    /// Wrapping `a - b`.
    Sub,
    /// Wrapping `a * b`.
    Mul,
}

impl Row {
    /// One independent row run.
    #[must_use]
    pub fn run(self) -> i64 {
        match self.op {
            RowOp::Add => crate::sem::i64_add(self.deeper, self.top),
            RowOp::Sub => crate::sem::i64_sub(self.deeper, self.top),
            RowOp::Mul => crate::sem::i64_mul(self.deeper, self.top),
        }
    }
}

/// The batch semantics: the ordered map of independent row runs.
#[must_use]
pub fn evaluate(rows: &[Row]) -> alloc::vec::Vec<i64> {
    rows.iter().copied().map(Row::run).collect()
}

/// The batch semantics with a caller-supplied output buffer, so a repeated batch does not allocate
/// per call. The buffer keeps its capacity and its previous contents are overwritten in order.
pub fn evaluate_into(rows: &[Row], out: &mut alloc::vec::Vec<i64>) {
    out.clear();
    out.extend(rows.iter().copied().map(Row::run));
}

/// The declared node work of one batch: one operation per row, counted separately from the
/// staging/output writes, which are also one per row. Both counters are exact functions of the row
/// count, so a cost claim can name them instead of elapsed time.
#[must_use]
pub fn declared_row_work(row_count: usize) -> (usize, usize) {
    (row_count, row_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_batch_is_the_ordered_map_of_its_rows() {
        let rows = [
            Row {
                op: RowOp::Add,
                deeper: 2,
                top: 3,
            },
            Row {
                op: RowOp::Sub,
                deeper: 2,
                top: 3,
            },
            Row {
                op: RowOp::Mul,
                deeper: -4,
                top: 5,
            },
        ];
        assert_eq!(evaluate(&rows), alloc::vec![5, -1, -20]);
        // A warm output buffer is overwritten in order and does not grow on the same row count,
        // which is the reuse the batch entry point exists for.
        let mut out = alloc::vec::Vec::with_capacity(8);
        evaluate_into(&rows, &mut out);
        assert_eq!(out, alloc::vec![5, -1, -20]);
        let capacity = out.capacity();
        evaluate_into(&rows, &mut out);
        assert_eq!(out, alloc::vec![5, -1, -20]);
        assert_eq!(out.capacity(), capacity);
    }

    #[test]
    fn wrapping_boundaries_are_the_documented_results() {
        let rows = [
            Row {
                op: RowOp::Add,
                deeper: i64::MAX,
                top: 1,
            },
            Row {
                op: RowOp::Sub,
                deeper: i64::MIN,
                top: 1,
            },
            Row {
                op: RowOp::Mul,
                deeper: i64::MAX,
                top: 2,
            },
        ];
        assert_eq!(evaluate(&rows), alloc::vec![i64::MIN, i64::MAX, -2]);
    }

    #[test]
    fn an_empty_batch_is_empty_and_a_prefix_is_a_prefix() {
        assert!(evaluate(&[]).is_empty());
        let rows = [
            Row {
                op: RowOp::Mul,
                deeper: 6,
                top: 7,
            },
            Row {
                op: RowOp::Add,
                deeper: 1,
                top: 2,
            },
        ];
        assert_eq!(&evaluate(&rows)[..1], &evaluate(&rows[..1])[..]);
        assert_eq!(declared_row_work(rows.len()), (2, 2));
    }
}
