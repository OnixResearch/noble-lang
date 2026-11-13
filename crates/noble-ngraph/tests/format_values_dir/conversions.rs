use noble_ngraph::ir::ScalarType;
use noble_ngraph::Value;

#[test]
fn explicit_rne_import_is_versioned_bounded_and_single_round() {
    use noble_ngraph::format::LiteralMode;

    let exact = |token: &str| noble_ngraph::format::value::parse(ScalarType::F32, token, 1);
    let rne = |token: &str| {
        noble_ngraph::format::value::parse_with_mode(ScalarType::F32, token, 1, LiteralMode::RneV2)
    };
    assert_eq!(super::code(exact("rne:0.1")), "E-PROFILE");
    assert_eq!(super::code(exact("0.1")), "E-LITERAL-INEXACT");
    assert_eq!(rne("rne:0.1").unwrap(), Value::F32(0x3dcc_cccd));
    assert_eq!(rne("rne:16777217").unwrap(), Value::F32(0x4b80_0000));
    assert_eq!(rne("rne:16777219").unwrap(), Value::F32(0x4b80_0002));
    assert_eq!(rne("rne:-0").unwrap(), Value::F32(0x8000_0000));
    assert_eq!(rne("rne:-1e-46").unwrap(), Value::F32(0x8000_0000));
    let at_limit = format!(
        "rne:1.{}",
        "0".repeat(noble_ngraph::sem::MAX_RNE_DECIMAL_BYTES - 2)
    );
    assert_eq!(rne(&at_limit).unwrap(), Value::F32(0x3f80_0000));
    assert_eq!(super::code(rne("rne:1e39")), "E-LITERAL-OVERFLOW");
    assert_eq!(
        super::code(rne(&format!("rne:1{}", "0".repeat(4096)))),
        "E-BUDGET-LITERAL-BYTES"
    );
    assert_eq!(super::code(rne("rne:+0.1")), "E-FORMAT");
}

#[test]
fn explicit_scalar_adapters_are_checked_and_do_not_saturate() {
    use noble_ngraph::sem::{
        f32_to_i64_exact, f32_to_i64_trunc, i64_to_f32_exact, i64_to_f32_rne, ConversionError,
    };

    assert_eq!(i64_to_f32_rne(1), 0x3f80_0000);
    assert_eq!(i64_to_f32_rne(16_777_217), 0x4b80_0000);
    assert_eq!(i64_to_f32_rne(16_777_219), 0x4b80_0002);
    assert_eq!(i64_to_f32_rne(-16_777_217), 0xcb80_0000);
    assert_eq!(i64_to_f32_rne(-16_777_219), 0xcb80_0002);
    assert_eq!(i64_to_f32_rne(i64::MAX), 0x5f00_0000);
    assert_eq!(i64_to_f32_rne(i64::MAX - 1), 0x5f00_0000);
    assert_eq!(i64_to_f32_rne(i64::MIN), 0xdf00_0000);
    assert_eq!(i64_to_f32_rne(i64::MIN + 1), 0xdf00_0000);
    assert_eq!(i64_to_f32_exact(16_777_216), Ok(0x4b80_0000));
    assert_eq!(i64_to_f32_exact(16_777_217), Err(ConversionError::Inexact));
    assert_eq!(i64_to_f32_exact(i64::MIN), Ok(0xdf00_0000));
    assert_eq!(i64_to_f32_exact(i64::MAX), Err(ConversionError::Inexact));
    assert_eq!(i64_to_f32_exact(0), Ok(0));

    assert_eq!(f32_to_i64_trunc(1.75f32.to_bits()), Ok(1));
    assert_eq!(f32_to_i64_trunc((-1.75f32).to_bits()), Ok(-1));
    assert_eq!(f32_to_i64_trunc(1), Ok(0));
    assert_eq!(f32_to_i64_trunc(0x8000_0001), Ok(0));
    assert_eq!(f32_to_i64_exact(1.0f32.to_bits()), Ok(1));
    assert_eq!(
        f32_to_i64_exact(1.5f32.to_bits()),
        Err(ConversionError::Inexact)
    );
    assert_eq!(f32_to_i64_exact(0), Ok(0));
    assert_eq!(f32_to_i64_exact(0x8000_0000), Ok(0));
    for nonfinite in [0x7f80_0000, 0xff80_0000, 0x7fc0_0001, 0xff80_0001] {
        assert_eq!(f32_to_i64_trunc(nonfinite), Err(ConversionError::NotFinite));
        assert_eq!(f32_to_i64_exact(nonfinite), Err(ConversionError::NotFinite));
    }
    assert_eq!(
        f32_to_i64_trunc(0x5f00_0000),
        Err(ConversionError::Overflow)
    );
    assert_eq!(
        f32_to_i64_exact(0x5f00_0000),
        Err(ConversionError::Overflow)
    );
    assert_eq!(
        f32_to_i64_trunc(0x5eff_ffff),
        Ok(i64::MAX - (1i64 << 39) + 1)
    );
    assert_eq!(
        f32_to_i64_exact(0x5eff_ffff),
        Ok(i64::MAX - (1i64 << 39) + 1)
    );
    assert_eq!(f32_to_i64_exact(0xdf00_0000), Ok(i64::MIN));
    assert_eq!(
        f32_to_i64_trunc(0xdf00_0001),
        Err(ConversionError::Overflow)
    );
    assert_eq!(ConversionError::Inexact.code().base(), "E-CONVERT-INEXACT");
    assert_eq!(
        ConversionError::NotFinite.code().base(),
        "E-CONVERT-NONFINITE"
    );
    assert_eq!(
        ConversionError::Overflow.code().base(),
        "E-CONVERT-OVERFLOW"
    );
}
