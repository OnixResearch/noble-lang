//! D3 binary32 contract and K-NUM-01 `I64` operations, and `≡obs`.
//!
//! Host binary32 arithmetic is IEEE 754 RNE on the pinned x86_64 target
//! (SSE, no FTZ/DAZ, no contraction: rustc never contracts `a * b + c`). The
//! independent `rustc_apfloat` leg of the `noble-ngraph-experiment` harness
//! checks this per operation; it is not assumed.
//!
//! The library is `no_std`: float functions that `core` does not provide are
//! the correctly rounded software routines in [`soft`].

use crate::ir::{Literal, OutOfRange, ScalarType};
#[allow(unused_imports)]
use crate::prelude::*;

/// A graph value at the ABI (D3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    /// `I64`.
    I64(i64),
    /// `Bool`.
    Bool(bool),
    /// `F32` as binary32 bits.
    F32(u32),
}

impl Value {
    /// Type.
    #[must_use]
    pub const fn ty(self) -> ScalarType {
        match self {
            Self::I64(_) => ScalarType::I64,
            Self::Bool(_) => ScalarType::Bool,
            Self::F32(_) => ScalarType::F32,
        }
    }

    /// `F32` value.
    #[must_use]
    pub const fn f32(v: f32) -> Self {
        Self::F32(v.to_bits())
    }

    /// The literal's value.
    #[must_use]
    pub const fn of_literal(l: Literal) -> Self {
        match l {
            Literal::I64(v) => Self::I64(v),
            Literal::Bool(v) => Self::Bool(v),
            Literal::F32(v) => Self::F32(v),
        }
    }

    /// Little-endian ABI bytes (D3).
    pub fn write_abi(self, out: &mut Vec<u8>) {
        match self {
            Self::I64(v) => out.extend_from_slice(&v.to_le_bytes()),
            Self::Bool(v) => out.push(u8::from(v)),
            Self::F32(v) => out.extend_from_slice(&v.to_le_bytes()),
        }
    }
}

/// `≡obs` on one value: bitwise, except all NaN encodings form one class.
#[must_use]
pub fn obs_eq(a: Value, b: Value) -> bool {
    match (a, b) {
        (Value::F32(x), Value::F32(y)) => {
            let (fx, fy) = (f32::from_bits(x), f32::from_bits(y));
            (fx.is_nan() && fy.is_nan()) || x == y
        }
        _ => a == b,
    }
}

/// `≡obs` on value sequences.
#[must_use]
pub fn obs_eq_all(a: &[Value], b: &[Value]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| obs_eq(*x, *y))
}

/// Binary32 `+0` bits.
pub const POS_ZERO: u32 = 0x0000_0000;
/// Binary32 `-0` bits (bias of a bias-free sum).
pub const NEG_ZERO: u32 = 0x8000_0000;
/// Binary32 `1` bits.
pub const ONE: u32 = 0x3f80_0000;

/// `rn(a + b)`.
#[must_use]
pub fn add(a: f32, b: f32) -> f32 {
    a + b
}

/// `rn(a - b)`.
#[must_use]
pub fn sub(a: f32, b: f32) -> f32 {
    a - b
}

/// `rn(a × b)`.
#[must_use]
pub fn mul(a: f32, b: f32) -> f32 {
    a * b
}

/// `relu(x)`: NaN ↦ NaN; `x > 0 ? x : +0` (so `relu(-0) = +0`).
#[must_use]
pub fn relu(x: f32) -> f32 {
    if x.is_nan() || x > 0.0 {
        x
    } else {
        0.0
    }
}

/// `reglu(l, g) = rn(l × relu(g))`.
#[must_use]
pub fn reglu(l: f32, g: f32) -> f32 {
    l * relu(g)
}

/// `step(s) = s ≥ 0.5 ? 1 : +0` (defined on the checked range `[0,1]`).
#[must_use]
pub fn step(s: f32) -> f32 {
    if s >= 0.5 {
        1.0
    } else {
        0.0
    }
}

/// Whether bits are an exact bit (`+0` or `1`).
#[must_use]
pub const fn is_bit(bits: u32) -> bool {
    bits == POS_ZERO || bits == ONE
}

/// `select`: bitwise copy of `on` if `bit` is `1`, else of `off`. Callers
/// guarantee `bit` is an exact bit (checked fact).
#[must_use]
pub const fn select(bit: u32, off: u32, on: u32) -> u32 {
    if bit == ONE {
        on
    } else {
        off
    }
}

/// Index position selected by `index` under the out-of-range rules: `Ok(Some(k))`
/// = candidate `k`; `Ok(None)` = `+0` (`zero` rule); `Err(())` = refused (guard
/// failure). A non-integral or NaN index is refused (it cannot be selected; the
/// checked `integral` fact normally excludes it statically).
#[allow(clippy::result_unit_err)]
pub fn index_position(
    i: f32,
    len: usize,
    below: OutOfRange,
    above: OutOfRange,
) -> Result<Option<usize>, ()> {
    if i.is_nan() || i.is_infinite() || soft::trunc_f32(i) != i {
        return Err(());
    }
    if i < 0.0 {
        return match below {
            OutOfRange::Refuse => Err(()),
            OutOfRange::Clamp => Ok(Some(0)),
            OutOfRange::Zero => Ok(None),
        };
    }
    // |i| < 2^24 under the checked range fact; larger finite values still
    // compare correctly against `len` (≤ 65,536) as f64.
    let fi = f64::from(i);
    #[allow(clippy::cast_precision_loss)]
    if fi >= len as f64 {
        return match above {
            OutOfRange::Refuse => Err(()),
            OutOfRange::Clamp => Ok(Some(len - 1)),
            OutOfRange::Zero => Ok(None),
        };
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(Some(fi as usize))
}

/// Ordered `affine` fold: `v₀ = bias; vᵢ = rn(vᵢ₋₁ + rn(cᵢ × xᵢ))`.
#[must_use]
pub fn affine(bias: f32, terms: impl IntoIterator<Item = (f32, f32)>) -> f32 {
    let mut v = bias;
    for (c, x) in terms {
        v += c * x;
    }
    v
}

/// K-NUM-01 wrapping `a + b` (`a` deeper).
#[must_use]
pub const fn i64_add(a: i64, b: i64) -> i64 {
    a.wrapping_add(b)
}

/// K-NUM-01 wrapping `a - b` (`a` deeper).
#[must_use]
pub const fn i64_sub(a: i64, b: i64) -> i64 {
    a.wrapping_sub(b)
}

/// K-NUM-01 wrapping `a * b` (`a` deeper).
#[must_use]
pub const fn i64_mul(a: i64, b: i64) -> i64 {
    a.wrapping_mul(b)
}

/// Whether `bits` is an exactly representable decimal of `text` (`E-LITERAL-INEXACT` check).
/// Returns `Some(bits)` iff the decimal string denotes exactly a binary32 value.
#[must_use]
pub fn exact_f32_decimal(text: &str) -> Option<u32> {
    // Parse with binary64 (correctly rounded) and binary32; the decimal is
    // exact in binary32 iff both agree and the binary64 value re-prints to an
    // exact decimal equal to the input. We avoid printing by checking exactness
    // with rational arithmetic on the decimal digits.
    let v32: f32 = text.parse().ok()?;
    if !v32.is_finite() {
        return None;
    }
    if decimal_equals_f32(text, v32) {
        Some(v32.to_bits())
    } else {
        None
    }
}

/// Exact comparison of a decimal string `[-]digits[.digits][e[-]exp]` with a
/// finite binary32 value, using big-integer arithmetic on `u32` limbs.
fn decimal_equals_f32(text: &str, v: f32) -> bool {
    let (neg, body) = text
        .strip_prefix('-')
        .map_or((false, text.strip_prefix('+').unwrap_or(text)), |b| {
            (true, b)
        });
    if v.is_sign_negative() != neg && v != 0.0 {
        return false;
    }
    let (mant, exp10) = match body.find(['e', 'E']) {
        Some(p) => match body[p + 1..].parse::<i32>() {
            Ok(e) => (&body[..p], e),
            Err(_) => return false,
        },
        None => (body, 0),
    };
    let (int_part, frac_part) = mant.split_once('.').unwrap_or((mant, ""));
    if !int_part
        .bytes()
        .chain(frac_part.bytes())
        .all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let digits: String = format!("{int_part}{frac_part}");
    let Ok(frac_len) = i32::try_from(frac_part.len()) else {
        return false;
    };
    let e10 = exp10.saturating_sub(frac_len);
    // decimal = D · 10^e10 ; binary = M · 2^e2 (M odd or 0).
    let bits = v.abs().to_bits();
    let (m, e2) = if bits == 0 {
        (0u64, 0i32)
    } else {
        let exp = i32::try_from((bits >> 23) & 0xff).unwrap_or(0);
        let frac = u64::from(bits & 0x7f_ffff);
        if exp == 0 {
            (frac, -149)
        } else {
            (frac | 0x80_0000, exp - 150)
        }
    };
    let d = Big::from_decimal(&digits);
    if m == 0 {
        return d.is_zero();
    }
    if d.is_zero() {
        return false;
    }
    // Compare D·10^e10 with m·2^e2 by cross-multiplying to integers.
    let mut lhs = d;
    let mut rhs = Big::from_u64(m);
    if e10 >= 0 {
        lhs.mul_pow(10, e10.unsigned_abs());
    } else {
        rhs.mul_pow(10, e10.unsigned_abs());
    }
    if e2 >= 0 {
        rhs.mul_pow(2, e2.unsigned_abs());
    } else {
        lhs.mul_pow(2, e2.unsigned_abs());
    }
    lhs.trim();
    rhs.trim();
    lhs.0 == rhs.0
}

/// Minimal unsigned big integer for exact literal checks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Big(Vec<u32>);

impl Big {
    fn from_u64(v: u64) -> Self {
        let mut b = Self(vec![(v & 0xffff_ffff) as u32, (v >> 32) as u32]);
        b.trim();
        b
    }
    fn from_decimal(s: &str) -> Self {
        let mut b = Self::default();
        for c in s.bytes() {
            b.mul_small(10);
            b.add_small(u32::from(c - b'0'));
        }
        b
    }
    fn trim(&mut self) {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }
    fn is_zero(&self) -> bool {
        self.0.iter().all(|&l| l == 0)
    }
    fn mul_small(&mut self, k: u32) {
        let mut carry = 0u64;
        for l in &mut self.0 {
            let p = u64::from(*l) * u64::from(k) + carry;
            *l = (p & 0xffff_ffff) as u32;
            carry = p >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
    }
    fn add_small(&mut self, k: u32) {
        let mut carry = u64::from(k);
        for l in &mut self.0 {
            if carry == 0 {
                break;
            }
            let s = u64::from(*l) + carry;
            *l = (s & 0xffff_ffff) as u32;
            carry = s >> 32;
        }
        if carry != 0 {
            self.0.push(carry as u32);
        }
    }
    fn mul_pow(&mut self, base: u32, n: u32) {
        for _ in 0..n {
            self.mul_small(base);
        }
        self.trim();
    }
}

/// Software float routines for the `no_std` library (design D2): exact
/// `trunc`/`floor`/`ceil`/`round` by exponent and mantissa masking, an
/// exact power of two, and a correctly rounded (roundTiesToEven) binary64
/// square root. Each agrees bitwise with the corresponding `std` method on
/// every non-NaN input; NaN inputs give a NaN.
pub mod soft {
    const F64_SIGN: u64 = 1 << 63;
    const F64_FRAC: u64 = (1 << 52) - 1;

    /// `f64::trunc`: rounds toward zero, keeping the sign of zero.
    #[must_use]
    pub const fn trunc_f64(x: f64) -> f64 {
        let bits = x.to_bits();
        let biased = ((bits >> 52) & 0x7ff) as i32;
        if biased == 0x7ff {
            return x;
        }
        let e = biased - 1023;
        if e < 0 {
            return f64::from_bits(bits & F64_SIGN);
        }
        if e >= 52 {
            return x;
        }
        let mask = (1u64 << (52 - e)) - 1;
        f64::from_bits(bits & !mask)
    }

    /// `f32::trunc`: rounds toward zero, keeping the sign of zero.
    #[must_use]
    pub const fn trunc_f32(x: f32) -> f32 {
        let bits = x.to_bits();
        let biased = ((bits >> 23) & 0xff) as i32;
        if biased == 0xff {
            return x;
        }
        let e = biased - 127;
        if e < 0 {
            return f32::from_bits(bits & 0x8000_0000);
        }
        if e >= 23 {
            return x;
        }
        let mask = (1u32 << (23 - e)) - 1;
        f32::from_bits(bits & !mask)
    }

    /// `f64::floor`.
    #[must_use]
    pub fn floor_f64(x: f64) -> f64 {
        let t = trunc_f64(x);
        // `t == x` covers integers, zeros and infinities; NaN falls through
        // to the comparison below and is returned unchanged.
        if t == x || x.is_nan() {
            return x;
        }
        // |t| < 2^52 here, so `t - 1` is exact.
        if x < 0.0 {
            t - 1.0
        } else {
            t
        }
    }

    /// `f64::ceil`.
    #[must_use]
    pub fn ceil_f64(x: f64) -> f64 {
        let t = trunc_f64(x);
        if t == x || x.is_nan() {
            return x;
        }
        if x > 0.0 {
            t + 1.0
        } else {
            t
        }
    }

    /// `f64::round`: half-way cases round away from zero.
    #[must_use]
    pub fn round_f64(x: f64) -> f64 {
        let t = trunc_f64(x);
        if t == x || x.is_nan() {
            return x;
        }
        // `x - t` is the exact fractional part (representable), |·| < 1.
        let frac = x - t;
        if x > 0.0 {
            if frac >= 0.5 {
                t + 1.0
            } else {
                t
            }
        } else if frac <= -0.5 {
            t - 1.0
        } else {
            t
        }
    }

    /// The exact binary64 value `2^k`: subnormal below `-1022`, `+0`
    /// below `-1074` and `+∞` above `1023`.
    #[must_use]
    pub const fn pow2_f64(k: i32) -> f64 {
        if k > 1023 {
            f64::INFINITY
        } else if k >= -1022 {
            f64::from_bits(((k + 1023) as u64) << 52)
        } else if k >= -1074 {
            f64::from_bits(1u64 << (k + 1074))
        } else {
            0.0
        }
    }

    /// `f64::sqrt`, correctly rounded to nearest, ties to even.
    ///
    /// For finite `x > 0`, `x = M·2^E` with a normalized 53- or 54-bit
    /// integer `M` and even `E`. Then `r = isqrt(M·2^58)` has 56 bits; its
    /// top 53 bits are the truncated significand, bit 2 is the round bit and
    /// the low two bits with the remainder `M·2^58 − r²` form the sticky
    /// bit. An exact tie cannot occur (an odd 54-bit square root would need
    /// a 107-bit square), but the rule handles one.
    #[must_use]
    pub fn sqrt_f64(x: f64) -> f64 {
        let bits = x.to_bits();
        if x.is_nan() || (bits & F64_SIGN != 0 && x != 0.0) {
            return f64::NAN;
        }
        if x == 0.0 || x == f64::INFINITY {
            return x;
        }
        let biased = ((bits >> 52) & 0x7ff) as i32;
        let (mut m, mut e) = if biased == 0 {
            (bits & F64_FRAC, -1074)
        } else {
            ((bits & F64_FRAC) | (1 << 52), biased - 1075)
        };
        // Normalize subnormals so that bit 52 is the leading bit.
        let shift = m.leading_zeros() as i32 - 11;
        m <<= shift;
        e -= shift;
        if e & 1 != 0 {
            m <<= 1;
            e -= 1;
        }
        let n = u128::from(m) << 58;
        let r = n.isqrt();
        let rem = n - r * r;
        let round = (r >> 2) & 1 == 1;
        let sticky = r & 0b11 != 0 || rem != 0;
        #[allow(clippy::cast_possible_truncation)]
        let mut sig = (r >> 3) as u64;
        if round && (sticky || sig & 1 == 1) {
            sig += 1;
        }
        let mut e_res = 3 + (e - 58) / 2;
        if sig == 1 << 53 {
            sig >>= 1;
            e_res += 1;
        }
        #[allow(clippy::cast_sign_loss)]
        let biased_res = (e_res + 1075) as u64;
        f64::from_bits((biased_res << 52) | (sig & F64_FRAC))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_decimal_literals() {
        assert_eq!(exact_f32_decimal("1.5"), Some(1.5f32.to_bits()));
        assert_eq!(exact_f32_decimal("-0"), Some(NEG_ZERO));
        assert_eq!(exact_f32_decimal("0.1"), None);
        assert_eq!(exact_f32_decimal("16777216"), Some(16_777_216f32.to_bits()));
        assert_eq!(exact_f32_decimal("16777217"), None);
        assert_eq!(exact_f32_decimal("1e40"), None);
        // 2^-149, the least subnormal, written exactly.
        let tiny = f32::from_bits(1);
        let mut s = String::from("1");
        let mut b = Big::from_u64(1);
        b.mul_pow(5, 149);
        s.clear();
        // decimal digits of 5^149 then e-149
        let mut digits = Vec::new();
        let mut cur = b;
        while !cur.is_zero() {
            let mut rem = 0u64;
            for l in cur.0.iter_mut().rev() {
                let v = (rem << 32) | u64::from(*l);
                *l = (v / 10) as u32;
                rem = v % 10;
            }
            cur.trim();
            digits.push(char::from(b'0' + rem as u8));
        }
        s.extend(digits.iter().rev());
        s.push_str("e-149");
        assert_eq!(exact_f32_decimal(&s), Some(tiny.to_bits()));
    }

    #[test]
    fn signed_zero_rules() {
        assert_eq!(relu(-0.0).to_bits(), POS_ZERO);
        assert_eq!(reglu(-1.0, 0.0).to_bits(), NEG_ZERO);
        assert_eq!(affine(-0.0, [(1.0, -0.0)]).to_bits(), NEG_ZERO);
        assert_eq!(affine(0.0, [(1.0, -0.0)]).to_bits(), POS_ZERO);
        assert!(obs_eq(Value::F32(0x7fc0_0000), Value::F32(0xffc0_0001)));
        assert!(!obs_eq(Value::F32(POS_ZERO), Value::F32(NEG_ZERO)));
    }
}
