//! Reviewed rule identities of the `Named-Contract-Calls-v3` profile.
//!
//! The table is deliberately outside every revision-hashed input. `lib.rs` is
//! not part of `checker_revision()` or `named_checker_revision()`, and this
//! module is not part of the intrinsic `MODEL` closure, so adding or changing a
//! v3 rule identity never re-stales an accepted v1 or v2 dependency. Editing an
//! identity is still a reviewed rule-set change under `V-INTRINSIC-01` and must
//! be reviewed together with the canonical spec delta.
//!
//! Nothing consumes the table yet: the revision-3 symbolic checker (task 2 of
//! `reusable-contract-calls`) is its first consumer, and until then the entries
//! are inert records.

/// Each entry is the proof-term rule id and the exact Lean declaration the
/// strict consumer instantiates. Every declaration is a Lean core/Std theorem
/// pinned by `proofs/mc1/ContractGate.lean` under the strict axiom list
/// (`propext`, `Classical.choice`, `Quot.sound`); none is an axiom. The
/// `List` identities are the `wire@1` left/right identity and associativity
/// rules, and the `BitVec` identities are the bounded wrapping-arithmetic
/// lemma rules.
pub const REVIEWED_RULES_V3: &[(&str, &str)] = &[
    ("bv-add-assoc", "BitVec.add_assoc"),
    ("bv-add-comm", "BitVec.add_comm"),
    ("bv-mul-add", "BitVec.mul_add"),
    ("list-append-assoc", "List.append_assoc"),
    ("list-append-nil", "List.append_nil"),
    ("list-nil-append", "List.nil_append"),
];
