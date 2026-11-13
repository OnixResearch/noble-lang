#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this generator writes fixed boundary rows as Lean text and asserts each row's intended outcome, so expect/assert on the fixed rows and string building are the readable form."
)]
//! `sem::exact_f32_decimal` and `sem::rne_f32_decimal` against the Lean model
//! `NobleFoundation.Binary32.DecimalImport` (issue #40), at binary32 boundary decimals.
//! `DecimalImportCorpus.lean` is generated from these rows; its header states what the kernel
//! checks. Regenerate after a deliberate change with `cargo test -p noble-ngraph --test soft_float
//! -- --ignored write_the_lean_file`, then rerun the foundation lane.

use noble_ngraph::sem;

/// The generated file the foundation lane kernel-checks.
const COMMITTED: &str =
    include_str!("../../../../proofs/foundation/NobleFoundation/DecimalImportCorpus.lean");

/// `(2^23 - 1) × 2^-149`, written exactly: 112 digits, the most an exact binary32 decimal needs.
const LARGEST_SUBNORMAL: &str = "1175494210692441075487029444849287348827052428745893333857174530571588870475618904265502351336181163787841796875";
/// `2^-126`.
const SMALLEST_NORMAL: &str =
    "11754943508222875079687365372222456778186655567720875215087517062784172594547271728515625";
/// `2^-149`.
const LEAST_SUBNORMAL: &str = "140129846432481707092372958328991613128026194187651577175706828388979108268586060148663818836212158203125";
/// `2^-150`, the tie between zero and the least subnormal.
const HALF_LEAST_SUBNORMAL: &str = "700649232162408535461864791644958065640130970938257885878534141944895541342930300743319094181060791015625";
/// `3 × 2^-151`, above that tie.
const THREE_QUARTERS_LEAST_SUBNORMAL: &str = "10509738482436128031927971874674370984601964564073868288178012129173433120143954511149786412715911865234375";
/// `1 + 2^-24 + 2^-80`: rounding through binary64 first lands on the tie `1 + 2^-24` and then on
/// `1.0`; one rounding gives `1 + 2^-23`.
const DOUBLE_ROUNDING: &str =
    "100000005960464477539062582718061255302767487140869206996285356581211090087890625";

/// One boundary row: Lean name, sign, digits and power of ten, exact-import result, and finite
/// RNE result (`None` means the mathematical model overflows to infinity and the Rust API refuses).
type Row = (
    &'static str,
    bool,
    &'static str,
    i64,
    Option<u32>,
    Option<u32>,
);

const ROWS: [Row; 19] = [
    (
        "twoPow24",
        false,
        "16777216",
        0,
        Some(0x4B80_0000),
        Some(0x4B80_0000),
    ),
    (
        "twoPow24PlusOne",
        false,
        "16777217",
        0,
        None,
        Some(0x4B80_0000),
    ),
    (
        "twoPow24PlusTwo",
        false,
        "16777218",
        0,
        Some(0x4B80_0001),
        Some(0x4B80_0001),
    ),
    (
        "negativeTwoPow24",
        true,
        "16777216",
        0,
        Some(0xCB80_0000),
        Some(0xCB80_0000),
    ),
    (
        "maxFinite",
        false,
        "340282346638528859811704183484516925440",
        0,
        Some(0x7F7F_FFFF),
        Some(0x7F7F_FFFF),
    ),
    (
        "aboveMaxFinite",
        false,
        "340282346638528859811704183484516925441",
        0,
        None,
        Some(0x7F7F_FFFF),
    ),
    (
        "halfwayToOverflow",
        false,
        "340282356779733661637539395458142568448",
        0,
        None,
        None,
    ),
    ("oneE39", false, "1", 39, None, None),
    (
        "smallestNormal",
        false,
        SMALLEST_NORMAL,
        -126,
        Some(0x0080_0000),
        Some(0x0080_0000),
    ),
    (
        "largestSubnormal",
        false,
        LARGEST_SUBNORMAL,
        -149,
        Some(0x007F_FFFF),
        Some(0x007F_FFFF),
    ),
    (
        "leastSubnormal",
        false,
        LEAST_SUBNORMAL,
        -149,
        Some(0x0000_0001),
        Some(0x0000_0001),
    ),
    (
        "halfLeastSubnormal",
        false,
        HALF_LEAST_SUBNORMAL,
        -150,
        None,
        Some(0x0000_0000),
    ),
    (
        "threeQuartersLeastSubnormal",
        false,
        THREE_QUARTERS_LEAST_SUBNORMAL,
        -151,
        None,
        Some(0x0000_0001),
    ),
    (
        "doubleRounding",
        false,
        DOUBLE_ROUNDING,
        -80,
        None,
        Some(0x3F80_0001),
    ),
    (
        "carryToOne",
        false,
        "99999999999",
        -11,
        None,
        Some(0x3F80_0000),
    ),
    (
        "halfBelowTwoPow23",
        false,
        "83886075",
        -1,
        Some(0x4AFF_FFFF),
        Some(0x4AFF_FFFF),
    ),
    ("pointOne", false, "1", -1, None, Some(0x3DCC_CCCD)),
    (
        "negativeZero",
        true,
        "0",
        0,
        Some(0x8000_0000),
        Some(0x8000_0000),
    ),
    ("negativeUnderflow", true, "1", -46, None, Some(0x8000_0000)),
];

/// Other spellings of some rows' values, which the parser must normalize to the same import.
const SPELLINGS: [(&str, &str); 5] = [
    ("0.5", "5e-1"),
    ("16777216.000", "16777216e0"),
    ("-0.0", "-0e0"),
    ("000123.4500", "12345e-2"),
    ("1E+39", "1e39"),
];

fn text(row: &Row) -> String {
    let (_, negative, digits, exponent, _, _) = *row;
    format!("{}{digits}e{exponent}", if negative { "-" } else { "" })
}

fn hex(bits: u32) -> String {
    format!("0x{bits:08X}")
}

/// One row as Lean text, after checking that Rust computes the row's intended outcome.
fn row_text(row: &Row) -> String {
    let (name, negative, digits, exponent, exact, nearest) = *row;
    let literal = text(row);
    assert_eq!(
        sem::exact_f32_decimal(&literal),
        exact,
        "exact import of {literal}"
    );
    let parsed = sem::rne_f32_decimal(&literal);
    match nearest {
        Some(bits) => assert_eq!(parsed, Ok(bits), "nearest binary32 of {literal}"),
        None => assert_eq!(
            parsed,
            Err(sem::DecimalRefusal::Overflow),
            "overflow refusal of {literal}"
        ),
    }
    let terms: Vec<String> = digits.chars().map(String::from).collect();
    let exact_term = exact.map_or_else(
        || String::from("none"),
        |bits| format!("some {}", hex(bits)),
    );
    let rounded = match nearest {
        Some(bits) => format!(
            "theorem {name}_rne : rne {name} = {} := by\n  decide\n\n",
            hex(bits)
        ),
        None => {
            format!("theorem {name}_overflow : outcomeClass {name} = .overflow := by\n  decide\n\n")
        }
    };
    format!(
        "/-- `{literal}` -/\n\
         def {name} : Decimal :=\n  ⟨{negative}, [{digits}], {exponent}⟩\n\n\
         theorem {name}_exact : exactImport {name} = {exact_term} := by\n  decide\n\n\
         {rounded}",
        digits = terms.join(", "),
    )
}

const HEADER: &str = "import NobleFoundation.DecimalImport

/-!
# Exact decimal import against `sem.rs`, at binary32 boundaries (issue #40)

Generated by `crates/noble-ngraph/tests/soft_float_dir/decimal.rs`; do not edit by hand.
`the_lean_file_is_the_real_import` fails when the Rust results change, until this file is
regenerated and the foundation lane rerun.

Each row is a decimal `[-]digits e exponent`. The generator checked that
`sem::exact_f32_decimal` returns the row's `*_exact` value and that
`sem::rne_f32_decimal` returns either the expected finite bits or the explicit overflow refusal.
The kernel decides that the model's `exactImport`, `rne`, or `outcomeClass` gives the same answer.
The rows cover `2^24` and its neighbours, the largest finite value and
just above it, the tie to overflow, the smallest normal, the largest and least subnormals, the
ties around the least subnormal, a decimal that double rounding through binary64 would get wrong,
a carry into the next binade, signed zero and negative underflow.

This is agreement at these rows, not a proof about `sem.rs`.
-/

namespace NobleFoundation.Binary32.DecimalImport.Corpus

open NobleFoundation.Binary32 NobleFoundation.Binary32.DecimalImport

set_option exponentiation.threshold 4096
set_option maxRecDepth 100000

";

/// The whole generated Lean module.
fn lean_file() -> String {
    let mut lean = String::from(HEADER);
    for row in &ROWS {
        lean.push_str(&row_text(row));
    }
    lean.push_str("end NobleFoundation.Binary32.DecimalImport.Corpus\n");
    lean
}

#[test]
fn other_spellings_import_the_same() {
    for (spelling, canonical) in SPELLINGS {
        assert_eq!(
            sem::exact_f32_decimal(spelling),
            sem::exact_f32_decimal(canonical),
            "{spelling} and {canonical}"
        );
    }
}

#[test]
fn the_lean_file_is_the_real_import() {
    assert!(
        lean_file() == COMMITTED,
        "the Rust results changed: regenerate DecimalImportCorpus.lean with `cargo test -p noble-ngraph --test soft_float -- --ignored write_the_lean_file` and rerun the foundation lane"
    );
}

#[test]
#[ignore = "writes proofs/foundation/NobleFoundation/DecimalImportCorpus.lean; run after a deliberate change"]
fn write_the_lean_file() {
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proofs/foundation/NobleFoundation/DecimalImportCorpus.lean"
        ),
        lean_file(),
    )
    .expect("generated file written");
}
