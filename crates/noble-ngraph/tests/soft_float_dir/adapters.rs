#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_panic,
    tigerstyle::no_unwrap,
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; this generator writes a fixed boundary matrix as Lean text and asserts each row against the real adapter, so the fixed fixtures and string building are the readable form."
)]
//! Real scalar-conversion adapters against a generated kernel-checked model corpus (issue #40).

use noble_ngraph::sem::{self, ConversionError};

const COMMITTED: &str =
    include_str!("../../../../proofs/foundation/NobleFoundation/Binary32ConvertCorpus.lean");

const I64_RNE: [(&str, i64, u32); 11] = [
    ("zero", 0, 0x0000_0000),
    ("twoPow24", 16_777_216, 0x4b80_0000),
    ("twoPow24PlusOne", 16_777_217, 0x4b80_0000),
    ("twoPow24PlusThree", 16_777_219, 0x4b80_0002),
    ("negativeTwoPow24PlusOne", -16_777_217, 0xcb80_0000),
    ("negativeTwoPow24PlusThree", -16_777_219, 0xcb80_0002),
    ("minimum", i64::MIN, 0xdf00_0000),
    ("minimumSuccessor", i64::MIN + 1, 0xdf00_0000),
    ("maximum", i64::MAX, 0x5f00_0000),
    ("maximumPredecessor", i64::MAX - 1, 0x5f00_0000),
    ("one", 1, 0x3f80_0000),
];

const I64_EXACT: [(&str, i64, Option<u32>); 7] = [
    ("zero", 0, Some(0x0000_0000)),
    ("twoPow24", 16_777_216, Some(0x4b80_0000)),
    ("twoPow24PlusOne", 16_777_217, None),
    ("negativeTwoPow24", -16_777_216, Some(0xcb80_0000)),
    ("minimum", i64::MIN, Some(0xdf00_0000)),
    ("minimumSuccessor", i64::MIN + 1, None),
    ("maximum", i64::MAX, None),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum I64Result {
    Value(i64),
    Inexact,
    NotFinite,
    Overflow,
}

fn result(actual: Result<i64, ConversionError>) -> I64Result {
    match actual {
        Ok(value) => I64Result::Value(value),
        Err(ConversionError::Inexact) => I64Result::Inexact,
        Err(ConversionError::NotFinite) => I64Result::NotFinite,
        Err(ConversionError::Overflow) => I64Result::Overflow,
    }
}

const F32_TRUNC: [(&str, u32, I64Result); 16] = [
    ("one", 0x3f80_0000, I64Result::Value(1)),
    ("positiveFraction", 0x3fe0_0000, I64Result::Value(1)),
    ("negativeFraction", 0xbfe0_0000, I64Result::Value(-1)),
    ("positiveZero", 0x0000_0000, I64Result::Value(0)),
    ("negativeZero", 0x8000_0000, I64Result::Value(0)),
    ("positiveSubnormal", 0x0000_0001, I64Result::Value(0)),
    ("negativeSubnormal", 0x8000_0001, I64Result::Value(0)),
    ("minimum", 0xdf00_0000, I64Result::Value(i64::MIN)),
    (
        "maximumFiniteInRange",
        0x5eff_ffff,
        I64Result::Value(i64::MAX - (1i64 << 39) + 1),
    ),
    ("positiveLimit", 0x5f00_0000, I64Result::Overflow),
    ("negativeBelowLimit", 0xdf00_0001, I64Result::Overflow),
    ("positiveInfinity", 0x7f80_0000, I64Result::NotFinite),
    ("negativeInfinity", 0xff80_0000, I64Result::NotFinite),
    ("positiveNaN", 0x7fc0_0001, I64Result::NotFinite),
    ("negativeNaN", 0xffc0_0001, I64Result::NotFinite),
    ("largeFraction", 0x3f80_0001, I64Result::Value(1)),
];

const F32_EXACT: [(&str, u32, I64Result); 15] = [
    ("one", 0x3f80_0000, I64Result::Value(1)),
    ("positiveFraction", 0x3fe0_0000, I64Result::Inexact),
    ("negativeFraction", 0xbfe0_0000, I64Result::Inexact),
    ("positiveZero", 0x0000_0000, I64Result::Value(0)),
    ("negativeZero", 0x8000_0000, I64Result::Value(0)),
    ("positiveSubnormal", 0x0000_0001, I64Result::Inexact),
    ("negativeSubnormal", 0x8000_0001, I64Result::Inexact),
    ("minimum", 0xdf00_0000, I64Result::Value(i64::MIN)),
    (
        "maximumFiniteInRange",
        0x5eff_ffff,
        I64Result::Value(i64::MAX - (1i64 << 39) + 1),
    ),
    ("positiveLimit", 0x5f00_0000, I64Result::Overflow),
    ("negativeBelowLimit", 0xdf00_0001, I64Result::Overflow),
    ("positiveInfinity", 0x7f80_0000, I64Result::NotFinite),
    ("negativeInfinity", 0xff80_0000, I64Result::NotFinite),
    ("positiveNaN", 0x7fc0_0001, I64Result::NotFinite),
    ("negativeNaN", 0xffc0_0001, I64Result::NotFinite),
];

fn lean_integer(value: i64) -> String {
    if value < 0 {
        format!("({value})")
    } else {
        value.to_string()
    }
}

fn truncation_value(bits: u32) -> Option<i64> {
    F32_TRUNC.iter().find_map(|(_, candidate, expected)| {
        if *candidate != bits {
            return None;
        }
        match expected {
            I64Result::Value(value) => Some(*value),
            I64Result::Inexact | I64Result::NotFinite | I64Result::Overflow => None,
        }
    })
}

fn f32_exact_proof(bits: u32, expected: I64Result, truncation: Option<i64>) -> String {
    match expected {
        I64Result::Value(value) => {
            let truncation = truncation.expect("exact adapter row has a truncating result");
            assert_eq!(value, truncation, "exact adapter returns its truncation");
            format!(
                "by\n  have truncated_value : truncateBinary32 ({bits}#32) = {} := by decide\n  have exact_value : exactValue ({bits}#32) = ({} : ℚ) := by\n    norm_num [exactValue, scaledMagnitude, exponent, fraction, isNegative, pow2]\n  simp [f32ToI64Exact, finiteBits, classify, exponent, fraction, isNegative, exact_value, truncated_value, I64Range]",
                lean_integer(truncation),
                lean_integer(truncation),
            )
        }
        I64Result::Inexact => {
            let truncation = truncation.expect("inexact adapter row has an in-range truncation");
            format!(
                "by\n  have truncated_value : truncateBinary32 ({bits}#32) = {} := by decide\n  have exact_value_ne : exactValue ({bits}#32) ≠ ({} : ℚ) := by\n    norm_num [exactValue, scaledMagnitude, exponent, fraction, isNegative, pow2]\n  simp [f32ToI64Exact, finiteBits, classify, exponent, fraction, isNegative, exact_value_ne, truncated_value, I64Range]",
                lean_integer(truncation),
                lean_integer(truncation),
            )
        }
        I64Result::NotFinite | I64Result::Overflow => String::from("by decide"),
    }
}

fn expected_result(value: I64Result) -> String {
    match value {
        I64Result::Value(integer) => {
            format!(".ok (BitVec.ofInt 64 {})", lean_integer(integer))
        }
        I64Result::Inexact => String::from(".error .inexact"),
        I64Result::NotFinite => String::from(".error .notFinite"),
        I64Result::Overflow => String::from(".error .overflow"),
    }
}

fn lean_file() -> String {
    let mut output = String::from(
        "import NobleFoundation.Binary32Convert\n\n\
/-!\n# Real adapter outputs against the kernel-checked conversion model (issue #40)\n\n\
Generated by `crates/noble-ngraph/tests/soft_float_dir/adapters.rs`; do not edit by hand.\n\
Each row below was checked against the real Rust adapter by the freshness test. The corresponding\n\
Lean theorem kernel-checks the model expectation using `decide` or exact rational arithmetic. This\n\
is finite correspondence evidence, not a universal Rust refinement proof.\n-/\n\n\
namespace NobleFoundation.Binary32.Convert.Corpus\n\n\
open NobleFoundation.Binary32.Convert\n\n\
open NobleFoundation.Binary32\n\n\
open NobleFoundation.Binary32.Error\n\n\
open NobleFoundation.Binary32.DecimalImport\n\n\
open NobleFoundation.CheckedI64\n\n\
set_option exponentiation.threshold 4096\n\n",
    );
    for (name, input, bits) in I64_RNE {
        assert_eq!(sem::i64_to_f32_rne(input), bits, "i64 RNE {name}");
        output.push_str(&format!(
            "theorem i64_{name}_rne : i64ToF32Rne (BitVec.ofInt 64 {}) = 0x{bits:08x} := by decide\n\n",
            lean_integer(input)
        ));
    }
    for (name, input, expected) in I64_EXACT {
        let actual = sem::i64_to_f32_exact(input);
        match expected {
            Some(bits) => assert_eq!(actual, Ok(bits), "i64 exact {name}"),
            None => assert_eq!(actual, Err(ConversionError::Inexact), "i64 exact {name}"),
        }
        let term = expected.map_or_else(
            || String::from(".error .inexact"),
            |bits| format!(".ok 0x{bits:08x}"),
        );
        let rounded_bits = sem::i64_to_f32_rne(input);
        output.push_str(&format!(
            "theorem i64_{name}_exact : i64ToF32Exact (BitVec.ofInt 64 {}) = {term} := by\n  have rounded : i64ToF32Rne (BitVec.ofInt 64 {}) = 0x{rounded_bits:08x} := by decide\n  rw [i64ToF32Exact]\n  simp only [rounded]\n  norm_num [finiteBits, classify, exponent, fraction, isNegative, exactValue, scaledMagnitude, pow2]\n\n",
            lean_integer(input),
            lean_integer(input),
        ));
    }
    for (name, bits, expected) in F32_TRUNC {
        assert_eq!(
            result(sem::f32_to_i64_trunc(bits)),
            expected,
            "f32 trunc {name}"
        );
        output.push_str(&format!(
            "theorem f32_{name}_trunc : f32ToI64Trunc 0x{bits:08x} = {} := by decide\n\n",
            expected_result(expected)
        ));
    }
    for (name, bits, expected) in F32_EXACT {
        assert_eq!(
            result(sem::f32_to_i64_exact(bits)),
            expected,
            "f32 exact {name}"
        );
        let truncation = truncation_value(bits);
        output.push_str(&format!(
            "theorem f32_{name}_exact : f32ToI64Exact 0x{bits:08x} = {} := {}\n\n",
            expected_result(expected),
            f32_exact_proof(bits, expected, truncation),
        ));
    }
    output.push_str("end NobleFoundation.Binary32.Convert.Corpus\n");
    output
}

#[test]
fn the_lean_file_is_the_real_adapter_import() {
    assert!(
        lean_file() == COMMITTED,
        "the adapter results changed: regenerate Binary32ConvertCorpus.lean with `cargo test -p noble-ngraph --test soft_float -- --ignored write_the_adapter_file` and rerun the foundation lane"
    );
}

#[test]
#[ignore = "writes proofs/foundation/NobleFoundation/Binary32ConvertCorpus.lean; run after a deliberate change"]
fn write_the_adapter_file() {
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proofs/foundation/NobleFoundation/Binary32ConvertCorpus.lean"
        ),
        lean_file(),
    )
    .expect("generated adapter corpus written");
}
