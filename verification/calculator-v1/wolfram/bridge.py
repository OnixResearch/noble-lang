#!/usr/bin/env python3
"""Optional Wolfram reference for the Noble calculator (issue #24).

Reads one request (schema noble-wolfram-request/v1) and writes one response
(noble-wolfram-response/v1). The request carries each case as an exact
interchange tree built by `interchange.mjs`: integer literals are decimal digit
strings with a decimal scale, never floats and never Noble source text. This
bridge walks the tree left to right and applies the Noble adapter policy
(POLICY below) on exact integers. Each arithmetic step is one exact
computation by the selected arithmetic:

  wolfram          a Wolfram kernel via WolframClientForPython (MIT). The
                   expression is built from wolframclient's `wl` factory and
                   sent as WXF, so the Wolfram Language parser never sees input.
  python-fractions a stand-in on CPython `fractions.Fraction`. It is not
                   Wolfram and its results are never reported as an oracle run.

The kernel is untrusted: only an Integer or a normalized Rational is accepted
as a step result. Anything else is an `oracle-error`; a Wolfram timeout or
memory abort is an `oracle-timeout`, never Noble's deterministic
ResourceLimit. A missing client, kernel or license is `oracle-unavailable`.

Usage: bridge.py REQUEST.json RESPONSE.json
"""

import json
import math
import platform
import sys
from fractions import Fraction

POLICY_VERSION = "noble-wolfram-policy/1"
REQUEST_SCHEMA = "noble-wolfram-request/v1"
RESPONSE_SCHEMA = "noble-wolfram-response/v1"
TIMEOUT_MARK = "noble-oracle-timeout"
MEMORY_MARK = "noble-oracle-memory"
BINARY_OPS = ("add", "sub", "mul", "div", "pow")

# Raw Wolfram evaluations that document the intentional differences. The
# adapter never uses these values; a different raw result means the policy's
# premises changed for this kernel and the run is an oracle-error.
# (id, head, base, exponent or (numerator, denominator), expected InputForm)
PROBES = (
    ("zero-power-zero", "Power", 0, 0, "Indeterminate"),
    ("one-over-zero", "Divide", 1, 0, "ComplexInfinity"),
    ("zero-over-zero", "Divide", 0, 0, "Indeterminate"),
    ("zero-power-negative", "Power", 0, -1, "ComplexInfinity"),
    ("perfect-square-half-power", "Power", 4, (1, 2), "2"),
    ("irrational-half-power", "Power", 2, (1, 2), "Sqrt[2]"),
)


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

    POLICY (noble-wolfram-policy/1):
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


class Unavailable(Exception):
    pass


class WolframArithmetic:
    """Exact steps on a Wolfram kernel through WolframClientForPython."""

    name = "wolfram"

    def __init__(self, kernel, timeout_seconds, memory_bytes):
        try:
            import wolframclient
            from wolframclient.deserializers import binary_deserialize
            from wolframclient.evaluation import WolframLanguageSession
            from wolframclient.language import wl
            from wolframclient.language.expression import WLFunction, WLSymbol
        except Exception as error:  # ImportError, or a native pyzmq load failure
            raise Unavailable(f"wolframclient is not importable: {error}") from error
        self.wl = wl
        self.WLFunction = WLFunction
        self.WLSymbol = WLSymbol
        self.deserialize = binary_deserialize
        self.timeout = timeout_seconds
        self.memory = memory_bytes
        try:
            # inputform_string_evaluation=False: a Python string is never
            # parsed as Wolfram Language input.
            self.session = WolframLanguageSession(kernel=kernel, inputform_string_evaluation=False)
            self.session.start(block=True, timeout=max(60, timeout_seconds))
        except Exception as error:
            raise Unavailable(f"no usable Wolfram kernel: {type(error).__name__}: {error}") from error
        version = self._call_raw(wl.List(WLSymbol("$Version"), WLSymbol("$VersionNumber"),
                                         WLSymbol("$ReleaseNumber"), WLSymbol("$SystemID")))
        self.environment = {
            "client": {"package": "wolframclient", "version": wolframclient.__version__},
            "python": platform.python_version(),
            "kernel": {"path": kernel, "version": str(version[0]), "version_number": str(version[1]),
                       "release": str(version[2]), "system_id": str(version[3])},
            "resource_settings": {"time_constrained_seconds": timeout_seconds, "memory_constrained_bytes": memory_bytes},
        }

    def _call_raw(self, expr):
        wxf = self.session.evaluate_wxf(expr)
        return self.deserialize(wxf)

    def _operand(self, value):
        n, d = value
        return n if d == 1 else self.wl.Rational(n, d)

    def _step(self, expr):
        wl = self.wl
        guarded = wl.TimeConstrained(wl.MemoryConstrained(expr, self.memory, MEMORY_MARK), self.timeout, TIMEOUT_MARK)
        result = self._call_raw(guarded)
        if result in (TIMEOUT_MARK, MEMORY_MARK):
            raise OracleTimeout(f"Wolfram {result}")
        if type(result) is int:
            return exact(result, 1)
        if isinstance(result, self.WLFunction) and getattr(result.head, "name", None) == "Rational" \
                and len(result.args) == 2:
            return exact(*result.args)
        raise OracleError(f"non-exact Wolfram result {result!r}")

    def literal(self, digits, scale):
        return self._step(self.wl.Times(digits, self.wl.Power(10, -scale)))

    def neg(self, x):
        return self._step(self.wl.Times(-1, self._operand(x)))

    def add(self, x, y):
        return self._step(self.wl.Plus(self._operand(x), self._operand(y)))

    def sub(self, x, y):
        return self._step(self.wl.Subtract(self._operand(x), self._operand(y)))

    def mul(self, x, y):
        return self._step(self.wl.Times(self._operand(x), self._operand(y)))

    def div(self, x, y):
        return self._step(self.wl.Divide(self._operand(x), self._operand(y)))

    def pow(self, x, e):
        return self._step(self.wl.Power(self._operand(x), e))

    def probes(self):
        wl = self.wl
        out = []
        for name, head, left, right, expected in PROBES:
            argument = right if isinstance(right, int) else wl.Rational(*right)
            shown = right if isinstance(right, int) else f"Rational[{right[0]}, {right[1]}]"
            # Quiet suppresses the kernel's Power::infy style messages only.
            text = self._call_raw(wl.Quiet(wl.ToString(wl.InputForm(getattr(wl, head)(left, argument)))))
            out.append({"id": name, "expression": f"{head}[{left}, {shown}]", "raw_result": str(text),
                        "expected_raw": expected, "ok": str(text) == expected})
        return out

    def close(self):
        try:
            self.session.terminate()
        except Exception:
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
    try:
        if make_arithmetic is not None:
            arith = make_arithmetic(request)
        elif kind == "python-fractions":
            arith = FractionArithmetic()
        elif kind == "wolfram":
            arith = WolframArithmetic(request.get("kernel"), request["timeout_seconds"], request["memory_bytes"])
        else:
            raise SystemExit(f"unknown arithmetic {kind}")
    except Unavailable as error:
        return {**response, "status": "oracle-unavailable", "reason": str(error), "results": []}
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
