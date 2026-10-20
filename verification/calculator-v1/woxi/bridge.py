#!/usr/bin/env python3
"""Optional CPython stand-in reference for the Noble calculator (issue #24).

Reads one request (schema noble-woxi-request/v1) and writes one response
(noble-woxi-response/v1). The request carries each case as an exact
interchange tree built by `interchange.mjs`: integer literals are decimal digit
strings with a decimal scale, never floats and never Noble source text. This
bridge walks the tree left to right and applies the Noble adapter policy
(POLICY below) on exact integers. Each arithmetic step is one exact
computation by `python-fractions`, a stand-in on CPython `fractions.Fraction`.
It is not the Woxi reference and its results are never reported as an oracle
run. The Woxi reference is the Rust helper in [`helper/`](helper/src/main.rs),
which links the `woxi` crate and applies the same adapter policy.

Only exact integers and normalized rationals are accepted as step results.
Anything else is an `oracle-error`; a resource abort is an `oracle-timeout`,
never Noble's deterministic ResourceLimit.

Usage: bridge.py REQUEST.json RESPONSE.json
"""

import json
import math
import platform
import sys
from fractions import Fraction

POLICY_VERSION = "noble-woxi-policy/1"
REQUEST_SCHEMA = "noble-woxi-request/v1"
RESPONSE_SCHEMA = "noble-woxi-response/v1"
BINARY_OPS = ("add", "sub", "mul", "div", "pow")
class Diagnostic(Exception):
    """A Noble numeric diagnostic predicted by the adapter policy."""

    def __init__(self, category, rule):
        super().__init__(category)
        self.category = category
        self.rule = rule


class OracleTimeout(Exception):
    pass


class OracleError(Exception):
    pass


def exact(numerator, denominator):
    """Validates one untrusted exact step result: d > 0 and gcd(|n|, d) = 1."""
    if type(numerator) is not int or type(denominator) is not int:
        raise OracleError("non-integer numerator or denominator")
    if denominator <= 0 or math.gcd(numerator, denominator) != 1:
        raise OracleError(f"non-normalized rational {numerator}/{denominator}")
    return (numerator, denominator)


def evaluate(node, arith, trace):
    """Noble adapter policy over exact steps; operands evaluate left to right.

    POLICY (noble-woxi-policy/1):
      lit  digits/10^scale, from the exact digit string;
      neg  -x;  add x+y;  sub x-y;  mul x*y;
      div  y = 0 is DivisionByZero (no reference call), else x/y;
      pow  a non-integral exponent value is NonIntegerExponent (2/2 is
           integral); exponent 0 gives 1, including 0^0; base 0 with a
           negative exponent is DivisionByZero; else x^e.
    The first diagnostic in left-to-right order stops the walk.
    """
    op = node["op"]
    if op == "lit":
        digits, scale = node["digits"], node["scale"]
        if not (isinstance(digits, str) and digits.isascii() and digits.isdigit()):
            raise OracleError("literal digits must be an ASCII decimal string")
        if type(scale) is not int or scale < 0:
            raise OracleError("literal scale must be a non-negative integer")
        value = arith.literal(int(digits), scale)
        trace.append(("lit", value))
        return value
    if op == "neg":
        value = arith.neg(evaluate(node["x"], arith, trace))
        trace.append(("neg", value))
        return value
    if op not in BINARY_OPS:
        raise OracleError(f"operation {op!r} is outside the supported subset")
    x = evaluate(node["x"], arith, trace)
    y = evaluate(node["y"], arith, trace)
    if op == "div" and y[0] == 0:
        raise Diagnostic("DivisionByZero", "zero-divisor")
    if op == "pow":
        if y[1] != 1:
            raise Diagnostic("NonIntegerExponent", "nonintegral-exponent")
        if y[0] == 0:
            trace.append(("pow", (1, 1), "zero-exponent-is-one"))
            return (1, 1)
        if x[0] == 0 and y[0] < 0:
            raise Diagnostic("DivisionByZero", "zero-base-negative-exponent")
        value = arith.pow(x, y[0])
    else:
        value = getattr(arith, op)(x, y)
    trace.append((op, value))
    return value


class FractionArithmetic:
    """Stand-in reference on CPython Fraction. Not Wolfram."""

    name = "python-fractions"
    MAX_EXPONENT = 1 << 20

    def __init__(self):
        self.environment = {"implementation": "CPython fractions.Fraction",
                            "python": platform.python_version()}

    @staticmethod
    def _out(value):
        return exact(value.numerator, value.denominator)

    def literal(self, digits, scale):
        return self._out(Fraction(digits, 10 ** scale))

    def neg(self, x):
        return self._out(-Fraction(*x))

    def add(self, x, y):
        return self._out(Fraction(*x) + Fraction(*y))

    def sub(self, x, y):
        return self._out(Fraction(*x) - Fraction(*y))

    def mul(self, x, y):
        return self._out(Fraction(*x) * Fraction(*y))

    def div(self, x, y):
        return self._out(Fraction(*x) / Fraction(*y))

    def pow(self, x, e):
        if abs(e) > self.MAX_EXPONENT:
            raise OracleTimeout(f"stand-in exponent bound {self.MAX_EXPONENT} exceeded")
        return self._out(Fraction(*x) ** e)

    def probes(self):
        return []

    def close(self):
        pass


def reference(case, arith):
    trace = []
    try:
        n, d = evaluate(case["interchange"], arith, trace)
        return {"kind": "value", "n": str(n), "d": str(d), "steps": len(trace)}
    except Diagnostic as diagnostic:
        return {"kind": "diagnostic", "category": diagnostic.category, "rule": diagnostic.rule, "steps": len(trace)}
    except OracleTimeout as error:
        return {"kind": "oracle-timeout", "detail": str(error)}
    except OracleError as error:
        return {"kind": "oracle-error", "detail": str(error)}


def run(request, make_arithmetic=None):
    if request.get("schema") != REQUEST_SCHEMA:
        raise SystemExit(f"request schema must be {REQUEST_SCHEMA}")
    if request.get("policy_version") != POLICY_VERSION:
        return {"schema": RESPONSE_SCHEMA, "policy_version": POLICY_VERSION, "status": "oracle-error",
                "reason": f"request policy {request.get('policy_version')} != bridge policy {POLICY_VERSION}",
                "results": []}
    kind = request["arithmetic"]
    response = {"schema": RESPONSE_SCHEMA, "policy_version": POLICY_VERSION, "arithmetic": kind,
                "python": platform.python_version()}
    if make_arithmetic is not None:
        arith = make_arithmetic(request)
    elif kind == "python-fractions":
        arith = FractionArithmetic()
    else:
        raise SystemExit(f"arithmetic {kind} is not served by bridge.py; use the Woxi helper")
    try:
        response["environment"] = arith.environment
        probes = arith.probes()
        response["probes"] = probes
        response["results"] = [{"id": case["id"], "reference": reference(case, arith)} for case in request["cases"]]
        changed = [probe["id"] for probe in probes if not probe["ok"]]
        response["status"] = "oracle-error" if changed else "ok"
        if changed:
            response["reason"] = f"policy probes changed: {', '.join(changed)}"
    except Exception as error:  # infrastructure failure after start
        response.update(status="oracle-error", reason=f"{type(error).__name__}: {error}")
        response.setdefault("results", [])
    finally:
        arith.close()
    return response


def main(argv):
    if len(argv) != 3:
        raise SystemExit("usage: bridge.py REQUEST.json RESPONSE.json")
    sys.set_int_max_str_digits(0)
    with open(argv[1], encoding="utf-8") as handle:
        request = json.load(handle)
    response = run(request)
    with open(argv[2], "x", encoding="utf-8") as handle:
        json.dump(response, handle, indent=2)
        handle.write("\n")
    print(response["status"] + (f": {response['reason']}" if response.get("reason") else ""))
    return {"ok": 0, "oracle-unavailable": 3}.get(response["status"], 4)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
