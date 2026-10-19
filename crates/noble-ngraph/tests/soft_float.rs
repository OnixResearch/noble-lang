#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::non_trait_imports,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! `sem::soft` (design D2, NGRAPH-CASE-14): the `no_std` library's float
//! routines agree bitwise with `std` on every non-NaN result, and give a NaN
//! wherever `std` does. The default tier checks every special value, every
//! power of two and 100,000 seeded bit patterns per routine; the ignored tier
//! checks 10^7.

use noble_ngraph::prng::SplitMix64;
use noble_ngraph::sem::soft;

fn same64(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn same32(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn check64(x: f64) {
    let pairs: [(&str, f64, f64); 5] = [
        ("sqrt", soft::sqrt_f64(x), x.sqrt()),
        ("trunc", soft::trunc_f64(x), x.trunc()),
        ("floor", soft::floor_f64(x), x.floor()),
        ("ceil", soft::ceil_f64(x), x.ceil()),
        ("round", soft::round_f64(x), x.round()),
    ];
    for (name, ours, std) in pairs {
        assert!(
            same64(ours, std),
            "{name}({x:e} = {:#018x}): soft {:#018x}, std {:#018x}",
            x.to_bits(),
            ours.to_bits(),
            std.to_bits()
        );
    }
}

fn check32(x: f32) {
    let (ours, std) = (soft::trunc_f32(x), x.trunc());
    assert!(same32(ours, std), "trunc_f32({:#010x})", x.to_bits());
}

fn specials() -> Vec<f64> {
    let mut v = vec![
        0.0,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::from_bits(0x000f_ffff_ffff_ffff),
        f64::MAX,
        f64::INFINITY,
        f64::NAN,
        f64::from_bits(0x7ff0_0000_0000_0001),
        0.5,
        1.0,
        1.5,
        2.5,
        3.5,
        0.499_999_999_999_999_94,
        4_503_599_627_370_495.5,
        4_503_599_627_370_496.0,
        4_503_599_627_370_497.0,
        f64::from_bits(0x4340_0000_0000_0001),
        1e-308,
        2.0,
        4.0,
        1e300,
        f64::EPSILON,
        1.0 - f64::EPSILON / 2.0,
        1.0 + f64::EPSILON,
    ];
    let neg: Vec<f64> = v.iter().map(|x| -x).collect();
    v.extend(neg);
    v
}

#[test]
fn special_values_match_std() {
    for x in specials() {
        check64(x);
        #[allow(clippy::cast_possible_truncation)]
        check32(x as f32);
    }
    for x in [
        f32::from_bits(1),
        f32::MIN_POSITIVE,
        8_388_607.5,
        8_388_608.0,
        f32::from_bits(0x4b80_0001),
        -0.0,
    ] {
        check32(x);
        check32(-x);
    }
}

#[test]
fn powers_of_two_are_exact() {
    let mut up = 1.0f64;
    for k in 0..=1100 {
        assert_eq!(soft::pow2_f64(k).to_bits(), up.to_bits(), "2^{k}");
        up *= 2.0;
    }
    let mut down = 1.0f64;
    for k in 0..=1100 {
        assert_eq!(soft::pow2_f64(-k).to_bits(), down.to_bits(), "2^-{k}");
        down *= 0.5;
    }
    for k in -300..=300 {
        assert_eq!(
            soft::pow2_f64(k).to_bits(),
            2f64.powi(k).to_bits(),
            "powi 2^{k}"
        );
    }
}

/// `n` seeded patterns per routine: half raw bit patterns, half with a
/// moderate exponent (|x| near 2^-5 .. 2^60), where rounding decisions live.
fn random_tier(n: u64, seed: u64) {
    let mut r = SplitMix64::new(seed);
    for i in 0..n {
        let raw = r.next_u64();
        let bits = if i % 2 == 0 {
            raw
        } else {
            let exp = 1018 + (r.next_u64() % 66);
            (raw & 0x800f_ffff_ffff_ffff) | (exp << 52)
        };
        check64(f64::from_bits(bits));
        #[allow(clippy::cast_possible_truncation)]
        let b32 = if i % 2 == 0 {
            raw as u32
        } else {
            let exp = 122 + (r.next_u64() % 34) as u32;
            (raw as u32 & 0x807f_ffff) | (exp << 23)
        };
        check32(f32::from_bits(b32));
    }
}

#[test]
fn seeded_bit_patterns_match_std() {
    random_tier(100_000, 0x50f7_f10a_7000_0001);
}

#[test]
#[ignore = "slow (10^7 patterns per routine); run: cargo test --release -p noble-ngraph --test soft_float -- --ignored"]
fn ten_million_bit_patterns_match_std() {
    random_tier(10_000_000, 0x50f7_f10a_7000_0002);
}
