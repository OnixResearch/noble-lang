#!/usr/bin/env python3
"""Independent external proposal of binary32 expected bits (issue #38 phase 4).

This is deliberately *not* the Lean model: it uses Python integers and its own
round-to-nearest/ties-to-even search over representable values instead of
Noble's floor-logarithm scaling, and Python's `struct` for the binary64 to
binary32 encoding of each proposed result. Lean then checks every proposal
against the selected model with a kernel-checkable proof; a proposal is not
evidence on its own.

Usage: propose.py OUT.json
"""
import json
import struct
import sys
from fractions import Fraction

SIGN = 1 << 31


def encode(sign_negative, exponent, fraction):
    return (SIGN if sign_negative else 0) | (exponent << 23) | fraction


def decode(bits):
    """Exact value of a finite encoding, or None for inf/NaN."""
    exponent = (bits >> 23) & 0xFF
    fraction = bits & 0x7FFFFF
    negative = bool(bits & SIGN)
    if exponent == 0xFF:
        return None
    if exponent == 0:
        value = Fraction(fraction, 1) * Fraction(2) ** -149
    else:
        value = Fraction(2**23 + fraction, 1) * Fraction(2) ** (exponent - 127 - 23)
    return -value if negative else value


def classify(bits):
    exponent = (bits >> 23) & 0xFF
    fraction = bits & 0x7FFFFF
    negative = bool(bits & SIGN)
    if exponent == 0xFF:
        return "infinity" if fraction == 0 else "notANumber"
    if exponent == 0:
        if fraction == 0:
            return "negZero" if negative else "posZero"
        return "subnormal"
    return "normal"


INF = 0x7F800000
CANONICAL_NAN = 0x7FC00000


def is_inf(bits):
    return (bits & 0x7FFFFFFF) == INF


def finite_bits(value, negative):
    """Nearest binary32 encoding of the exact rational `value` (nonzero), by
    bracketing the value's magnitude and rounding the step count on the grid at
    its exponent rather than by replicating the Lean scaling."""
    value = abs(Fraction(value))
    exponent = 0
    scale = Fraction(1)
    if value >= 1:
        while value >= scale * 2:
            scale *= 2
            exponent += 1
    else:
        while value < scale:
            scale /= 2
            exponent -= 1
    if exponent < -126:
        # Gradual underflow: the grid is one smallest subnormal.
        steps = value / Fraction(2) ** -149
        lower = steps.numerator // steps.denominator
        remainder = steps - lower
        if remainder > Fraction(1, 2):
            lower += 1
        elif remainder == Fraction(1, 2) and lower % 2 == 1:
            lower += 1
        if lower >= 2**23:
            # Rounded up into the smallest normal.
            return encode(negative, 1, 0)
        if lower == 0:
            return encode(negative, 0, 0)
        return encode(negative, 0, lower)
    spacing = Fraction(2) ** (exponent - 23)
    steps = value / spacing
    lower = steps.numerator // steps.denominator
    remainder = steps - lower
    if remainder > Fraction(1, 2):
        lower += 1
    elif remainder == Fraction(1, 2) and lower % 2 == 1:
        lower += 1
    if lower >= 2**24:
        # A carry out of the 24-bit significand increments the exponent.
        lower = 2**23
        exponent += 1
    if exponent > 127:
        return INF | (SIGN if negative else 0)
    return encode(negative, exponent + 127, lower - 2**23)


def add(x, y):
    cx, cy = classify(x), classify(y)
    if cx == "notANumber" or cy == "notANumber":
        return CANONICAL_NAN, "NaN propagates as one canonical NaN"
    nx, ny = bool(x & SIGN), bool(y & SIGN)
    if cx == "infinity" and cy == "infinity":
        return (INF | (SIGN if nx else 0), "same-sign infinities") if nx == ny else (CANONICAL_NAN, "inf + -inf is NaN")
    if cx == "infinity":
        return INF | (SIGN if nx else 0), "infinity plus finite"
    if cy == "infinity":
        return INF | (SIGN if ny else 0), "finite plus infinity"
    exact = decode(x) + decode(y)
    if exact == 0:
        return encode(nx and ny, 0, 0), "zero sign keeps negative only for two negative zeros"
    return finite_bits(exact, exact < 0), "round the exact sum once"


def sub(x, y):
    return add(x, y ^ SIGN)


def mul(x, y):
    cx, cy = classify(x), classify(y)
    if cx == "notANumber" or cy == "notANumber":
        return CANONICAL_NAN, "NaN propagates as one canonical NaN"
    nx, ny = bool(x & SIGN), bool(y & SIGN)
    negative = nx != ny
    if cx == "infinity" or cy == "infinity":
        if cx in ("posZero", "negZero") or cy in ("posZero", "negZero"):
            return CANONICAL_NAN, "zero times infinity is NaN"
        return INF | (SIGN if negative else 0), "infinity times finite is infinity"
    if cx in ("posZero", "negZero") or cy in ("posZero", "negZero"):
        return encode(negative, 0, 0), "zero result keeps the xor sign"
    exact = decode(x) * decode(y)
    return finite_bits(exact, negative), "round the exact product once"


CASES = [
    # Halfway ties in both directions around 1.0.
    ("add", 0x3F800000, 0x33800000, "1.0 + 2^-24 ties to even, down to 1.0"),
    ("add", 0x3F800000, 0x34000000, "1.0 + 2^-23 is exact"),
    ("add", 0x3F800001, 0x33800000, "1.0+2^-23 + 2^-24 ties to even, up away from odd"),
    ("add", 0x3F800000, 0x34300000, "1.0 + 3*2^-25 rounds up"),
    ("sub", 0x3F800001, 0x3F800000, "cancellation 1.0+2^-23 - 1.0 is exact"),
    ("sub", 0x3F800000, 0x3F800000, "x - x is +0"),
    # Signed zeros.
    ("add", 0x00000000, 0x80000000, "(+0) + (-0) is +0"),
    ("add", 0x80000000, 0x80000000, "(-0) + (-0) is -0"),
    ("sub", 0x00000000, 0x00000000, "(+0) - (+0) is +0"),
    ("sub", 0x80000000, 0x00000000, "(-0) - (+0) is -0"),
    ("mul", 0x80000000, 0x3F800000, "(-0) * 1.0 is -0"),
    # Smallest and largest subnormals, and the normal boundary.
    ("add", 0x00000001, 0x00000000, "smallest subnormal is retained"),
    ("mul", 0x00000001, 0x3F800000, "smallest subnormal times 1.0"),
    ("add", 0x007FFFFF, 0x00000001, "largest subnormal plus smallest subnormal is the smallest normal"),
    ("mul", 0x00800000, 0x3F000000, "smallest normal halved is a subnormal with fraction 0x400000"),
    ("sub", 0x00800000, 0x00000001, "smallest normal minus the smallest subnormal is the largest subnormal"),
    ("mul", 0x00000001, 0x00000001, "smallest subnormal squared underflows to +0"),
    # Overflow.
    ("mul", 0x7F7FFFFF, 0x40000000, "largest normal doubled overflows to infinity"),
    ("mul", 0xFF7FFFFF, 0x40000000, "negative overflow keeps the sign"),
    ("add", 0x7F7FFFFF, 0x7F7FFFFF, "largest normal plus itself overflows to infinity"),
    # Infinities and NaNs.
    ("add", 0x7F800000, 0x7F800000, "inf + inf is inf"),
    ("add", 0x7F800000, 0xFF800000, "inf + -inf is NaN"),
    ("sub", 0x7F800000, 0x7F800000, "inf - inf is NaN"),
    ("mul", 0x7F800000, 0x00000000, "inf * 0 is NaN"),
    ("mul", 0xFF800000, 0x40000000, "-inf * 2.0 is -inf"),
    ("add", 0x7FC00000, 0x3F800000, "NaN + 1.0 is the canonical NaN"),
]


def main():
    output = sys.argv[1]
    receiver = {"add": add, "sub": sub, "mul": mul}
    proposals = []
    for operation, left, right, why in CASES:
        bits, reason = receiver[operation](left, right)
        proposals.append({
            "operation": operation,
            "left": left,
            "right": right,
            "expected": bits,
            "expected_class": classify(bits),
            "rationale": why,
            "classification": reason,
        })
    json.dump({"schema": "noble-binary32-external-proposals/v1", "proposals": proposals}, open(output, "w"), indent=2)
    print(f"proposed {len(proposals)} expected encodings")


if __name__ == "__main__":
    main()
