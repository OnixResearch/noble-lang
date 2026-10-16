//! Vector and training-data files and value tokens (design D5,
//! NGRAPH-FORMAT-01, NGRAPH-CASE-09) and the report rendering of values
//! (design D7).

use noble_ngraph::format::{data, value, vectors};
use noble_ngraph::ir::ScalarType;
use noble_ngraph::Value;

fn code<T: std::fmt::Debug>(r: Result<T, noble_ngraph::Diagnostic>) -> String {
    match r {
        Ok(v) => format!("accepted {v:?}"),
        Err(d) => d.code_string(),
    }
}

#[test]
fn f32_tokens_are_exact_encodings_or_exact_decimals() {
    let f = |t: &str| value::parse(ScalarType::F32, t, 1);
    assert_eq!(
        f("0x7fc00001").unwrap(),
        Value::F32(0x7fc0_0001),
        "NaN payload kept"
    );
    assert_eq!(f("0xff800000").unwrap(), Value::F32(0xff80_0000));
    assert_eq!(f("0x1").unwrap(), Value::F32(1), "subnormal");
    assert_eq!(f("-0").unwrap(), Value::F32(0x8000_0000));
    assert_eq!(f("-0.5").unwrap(), Value::F32((-0.5f32).to_bits()));
    assert_eq!(f("16777216").unwrap(), Value::F32(0x4b80_0000));
    assert_eq!(code(f("0.1")), "E-LITERAL-INEXACT");
    assert_eq!(code(f("16777217")), "E-LITERAL-INEXACT");
    assert_eq!(code(f("nan")), "E-LITERAL-NONFINITE");
    assert_eq!(code(f("-Infinity")), "E-LITERAL-NONFINITE");
    for bad in ["0x", "0x123456789", "0xzz", "abc", "1..0"] {
        assert_eq!(code(f(bad)), "E-FORMAT", "{bad}");
    }
    let i = |t: &str| value::parse(ScalarType::I64, t, 1);
    assert_eq!(i("-9223372036854775808").unwrap(), Value::I64(i64::MIN));
    assert_eq!(code(i("9223372036854775808")), "E-FORMAT");
    assert_eq!(code(i("1.0")), "E-FORMAT");
    let b = |t: &str| value::parse(ScalarType::Bool, t, 1);
    assert_eq!(b("true").unwrap(), Value::Bool(true));
    assert_eq!(code(b("1")), "E-FORMAT");
}

/// Decimal digits of `m · 5^k`.
fn times_pow5(m: u64, k: u32) -> String {
    let mut digits: Vec<u64> = m
        .to_string()
        .bytes()
        .rev()
        .map(|b| u64::from(b - b'0'))
        .collect();
    for _ in 0..k {
        let mut carry = 0;
        for d in &mut digits {
            let v = *d * 5 + carry;
            *d = v % 10;
            carry = v / 10;
        }
        while carry > 0 {
            digits.push(carry % 10);
            carry /= 10;
        }
    }
    digits
        .iter()
        .rev()
        .map(|d| char::from(b'0' + u8::try_from(*d).unwrap_or(0)))
        .collect()
}

/// The decimal expansion `0.000…digits` of `digits × 10^-149`.
fn below_one(digits: &str) -> String {
    format!("0.{}{digits}", "0".repeat(149 - digits.len()))
}

#[test]
fn decimal_exactness_is_decided_inside_the_binary32_window() {
    let f = |t: &str| value::parse(ScalarType::F32, t, 1);
    // The longest exact expansions are accepted: 2^-149, the largest finite
    // value, and (2^24 - 1) · 2^-149 with 112 significant digits.
    let least = times_pow5(1, 149);
    assert_eq!(least.len(), 105);
    assert_eq!(f(&below_one(&least)).unwrap(), Value::F32(1));
    let longest = times_pow5((1 << 24) - 1, 149);
    assert_eq!(longest.len(), 112);
    assert_eq!(f(&below_one(&longest)).unwrap(), Value::F32(0x00ff_ffff));
    let max = "340282346638528859811704183484516925440";
    assert_eq!(f(max).unwrap(), Value::F32(0x7f7f_ffff));
    assert_eq!(
        f(&format!("000{max}.000")).unwrap(),
        Value::F32(0x7f7f_ffff),
        "leading and trailing zeros are stripped"
    );
    // One more significant digit is refused, as is everything outside the
    // window; the existing codes stay.
    assert_eq!(
        code(f(&format!("{}1", below_one(&longest)))),
        "E-LITERAL-INEXACT"
    );
    assert_eq!(
        code(f(&format!("{}1", below_one(&least)))),
        "E-LITERAL-INEXACT"
    );
    assert_eq!(code(f(&format!("{max}1"))), "E-LITERAL-INEXACT");
    assert_eq!(code(f(&format!("{max}.5"))), "E-LITERAL-INEXACT");
    assert_eq!(code(f("1e39")), "E-LITERAL-INEXACT");
    assert_eq!(code(f("1e-46")), "E-LITERAL-INEXACT");
    assert_eq!(code(f("1e-99999999999999999999999")), "E-LITERAL-INEXACT");
    assert_eq!(code(f("Infinity")), "E-LITERAL-NONFINITE");
    // Long literals: only their significant digits matter.
    let long_one = format!("1.{}", "0".repeat(100_000));
    assert_eq!(f(&long_one).unwrap(), Value::F32(0x3f80_0000));
    assert_eq!(code(f(&format!("{long_one}1"))), "E-LITERAL-INEXACT");
    assert_eq!(f("0e99999999999999999999999").unwrap(), Value::F32(0));
    assert_eq!(f(".5").unwrap(), Value::F32(0x3f00_0000));
    assert_eq!(f("5.").unwrap(), Value::F32(0x40a0_0000));
    assert_eq!(f("1e+1").unwrap(), Value::F32(0x4120_0000));
}

#[test]
fn a_leading_plus_sign_is_not_a_number() {
    let f = |t: &str| value::parse(ScalarType::F32, t, 1);
    let i = |t: &str| value::parse(ScalarType::I64, t, 1);
    for bad in ["+1.5", "+0", "0x+1", "+", "1e"] {
        assert_eq!(code(f(bad)), "E-FORMAT", "{bad}");
    }
    for bad in ["+5", "+0", "--5", "-"] {
        assert_eq!(code(i(bad)), "E-FORMAT", "{bad}");
    }
    assert_eq!(i("-5").unwrap(), Value::I64(-5));
}

#[test]
fn rendering_keeps_bits_and_merges_every_nan() {
    assert_eq!(value::render(Value::F32(0x8000_0000)), "f32:0x80000000");
    assert_eq!(value::render(Value::F32(0x0040_0000)), "f32:0x00400000");
    assert_eq!(value::render(Value::F32(0x7f80_0000)), "f32:0x7f800000");
    for nan in [0x7fc0_0000u32, 0x7fc0_0001, 0xffc0_0000, 0x7f80_0001] {
        assert_eq!(value::render(Value::F32(nan)), "f32:nan");
    }
    assert_eq!(value::render(Value::I64(-7)), "i64:-7");
    assert_eq!(value::render(Value::Bool(false)), "bool:false");
}

#[test]
fn vector_files_are_typed_and_refused_as_a_whole() {
    let ok =
        "# probes\nnoble-ngraph-vectors/0\ntypes f32 i64\nv 0x3fc00000 -3   # row 1\n\nv -0 9\n";
    let v = vectors::parse(ok).expect("parses");
    assert_eq!(v.types, vec![ScalarType::F32, ScalarType::I64]);
    assert_eq!(
        v.rows,
        vec![
            vec![Value::F32(0x3fc0_0000), Value::I64(-3)],
            vec![Value::F32(0x8000_0000), Value::I64(9)]
        ]
    );
    assert_eq!(vectors::parse(&vectors::print(&v)).expect("reparses"), v);
    let none = vectors::parse("noble-ngraph-vectors/0\ntypes\nv\nv\n").expect("zero inputs");
    assert_eq!(none.rows, vec![Vec::<Value>::new(), Vec::new()]);
    let cases = [
        ("noble-ngraph-vectors/1\ntypes f32\nv 1\n", "E-FORMAT"),
        ("types f32\nv 1\n", "E-FORMAT"),
        ("", "E-FORMAT"),
        ("noble-ngraph-vectors/0\nv 1\n", "E-FORMAT"),
        ("noble-ngraph-vectors/0\n", "E-FORMAT"),
        ("noble-ngraph-vectors/0\ntypes f32\ntypes f32\n", "E-FORMAT"),
        ("noble-ngraph-vectors/0\ntypes f64\n", "E-FORMAT"),
        ("noble-ngraph-vectors/0\ntypes f32\nv 1\nv\n", "E-FORMAT"),
        (
            "noble-ngraph-vectors/0\ntypes f32\nv 1\nv 1 2\n",
            "E-FORMAT",
        ),
        ("noble-ngraph-vectors/0\ntypes f32\nw 1\n", "E-FORMAT"),
        (
            "noble-ngraph-vectors/0\ntypes f32\nv 1\nv 0.1\n",
            "E-LITERAL-INEXACT",
        ),
        ("noble-ngraph-vectors/0 extra\ntypes f32\n", "E-FORMAT"),
    ];
    for (text, want) in cases {
        assert_eq!(code(vectors::parse(text)), want, "{text:?}");
    }
}

#[test]
fn data_files_split_inputs_and_targets_at_one_separator() {
    let d = data::parse(
        "noble-ngraph-data/0\ntypes f32 ; f32 f32\npoint 0.5 ; 1 -0\npoint 0x3f800000;2 4\n",
    )
    .expect("parses");
    assert_eq!(d.inputs, vec![ScalarType::F32]);
    assert_eq!(d.targets, vec![ScalarType::F32, ScalarType::F32]);
    assert_eq!(
        d.points[1],
        (
            vec![Value::F32(0x3f80_0000)],
            vec![Value::F32(2f32.to_bits()), Value::F32(4f32.to_bits())]
        )
    );
    assert_eq!(data::parse(&data::print(&d)).expect("reparses"), d);
    let pts = d.to_points().expect("f32");
    assert_eq!(pts[0].targets[1].to_bits(), 0x8000_0000);
    assert_eq!(data::Data::of_points(&pts), d);
    let cases = [
        ("noble-ngraph-data/1\ntypes f32 ; f32\n", "E-FORMAT"),
        ("noble-ngraph-data/0\ntypes f32 f32\n", "E-FORMAT"),
        ("noble-ngraph-data/0\ntypes f32 ; f32 ; f32\n", "E-FORMAT"),
        (
            "noble-ngraph-data/0\ntypes f32 ; f32\npoint 1 2\n",
            "E-FORMAT",
        ),
        (
            "noble-ngraph-data/0\ntypes f32 ; f32\npoint 1 ; 2 3\n",
            "E-FORMAT",
        ),
        ("noble-ngraph-data/0\npoint 1 ; 2\n", "E-FORMAT"),
        (
            "noble-ngraph-data/0\ntypes f32 ; f32\npoint 0.1 ; 2\n",
            "E-LITERAL-INEXACT",
        ),
    ];
    for (text, want) in cases {
        assert_eq!(code(data::parse(text)), want, "{text:?}");
    }
    let ints = data::parse("noble-ngraph-data/0\ntypes i64 ; f32\npoint 1 ; 2\n").expect("parses");
    assert_eq!(code(ints.to_points()), "E-INTERFACE");
}
