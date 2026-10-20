"""Offline tests of bridge.py: adapter policy and unavailable states.
Standard library only; the Woxi reference is the separate Rust helper.

    python3 -m unittest discover -s verification/calculator-v1/woxi -p 'test_*.py'
"""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import bridge  # noqa: E402


def lit(digits, scale=0):
    return {"op": "lit", "digits": digits, "scale": scale}


def op(name, x, y=None):
    return {"op": name, "x": x} if y is None else {"op": name, "x": x, "y": y}


class Recording(bridge.FractionArithmetic):
    """The stand-in arithmetic, recording which steps were requested."""

    def __init__(self):
        super().__init__()
        self.calls = []

    def __getattribute__(self, name):
        attribute = super().__getattribute__(name)
        if name in ("literal", "neg", "add", "sub", "mul", "div", "pow"):
            def recorded(*args):
                self.calls.append(name)
                return attribute(*args)
            return recorded
        return attribute


def outcome(tree, arith=None):
    return bridge.reference({"id": "t", "interchange": tree}, arith or bridge.FractionArithmetic())


class Policy(unittest.TestCase):
    def test_zero_power_zero_is_one_without_a_power_step(self):
        arith = Recording()
        self.assertEqual(outcome(op("pow", lit("0"), lit("0")), arith)["n"], "1")
        self.assertNotIn("pow", arith.calls)
        self.assertEqual(outcome(op("pow", lit("7", 1), op("sub", lit("2"), lit("2"))))["n"], "1")

    def test_zero_divisor_is_a_diagnostic_and_never_reaches_the_reference(self):
        arith = Recording()
        result = outcome(op("div", lit("0"), op("sub", lit("3"), lit("3"))), arith)
        self.assertEqual((result["kind"], result["category"], result["rule"]),
                         ("diagnostic", "DivisionByZero", "zero-divisor"))
        self.assertNotIn("div", arith.calls)
        self.assertEqual(outcome(op("pow", lit("0"), op("neg", lit("1"))))["rule"], "zero-base-negative-exponent")

    def test_exponents_are_judged_by_value(self):
        self.assertEqual(outcome(op("pow", lit("4"), op("div", lit("1"), lit("2"))))["category"], "NonIntegerExponent")
        self.assertEqual(outcome(op("pow", lit("0"), op("div", lit("1"), lit("2"))))["category"], "NonIntegerExponent")
        self.assertEqual(outcome(op("pow", lit("4"), op("div", lit("2"), lit("2"))))["n"], "4")
        self.assertEqual((outcome(op("pow", lit("2"), op("neg", lit("3"))))["n"],
                          outcome(op("pow", lit("2"), op("neg", lit("3"))))["d"]), ("1", "8"))

    def test_first_diagnostic_in_left_to_right_order_wins(self):
        left = op("pow", op("div", lit("1"), lit("0")), op("div", lit("1"), lit("2")))
        self.assertEqual(outcome(left)["category"], "DivisionByZero")
        right = op("add", op("pow", lit("2"), op("div", lit("1"), lit("2"))), op("div", lit("1"), lit("0")))
        self.assertEqual(outcome(right)["category"], "NonIntegerExponent")

    def test_literals_are_exact_digit_strings(self):
        wide = "1" * 400
        self.assertEqual(outcome(lit(wide)), {"kind": "value", "n": wide, "d": "1", "steps": 1})
        self.assertEqual(outcome(op("add", lit("1", 1), lit("2", 1)))["d"], "10")
        self.assertEqual(outcome(lit("000000000000000000001", 20))["d"], "1" + "0" * 20)
        for bad in (lit("1.5"), lit("-1"), lit("١"), {"op": "lit", "digits": 5, "scale": 0}, lit("1", -1)):
            self.assertEqual(outcome(bad)["kind"], "oracle-error", bad)

    def test_operations_outside_the_subset_are_errors(self):
        self.assertEqual(outcome({"op": "call", "x": lit("1"), "y": lit("2")})["kind"], "oracle-error")


class States(unittest.TestCase):
    def request(self, **overrides):
        return {"schema": bridge.REQUEST_SCHEMA, "policy_version": bridge.POLICY_VERSION, "arithmetic": "python-fractions",
                "timeout_seconds": 1, "memory_bytes": 1,
                "cases": [{"id": "a", "interchange": lit("1")}], **overrides}

    def test_policy_mismatch_is_an_error(self):
        response = bridge.run(self.request(policy_version="noble-woxi-policy/0"))
        self.assertEqual(response["status"], "oracle-error")

    def test_changed_policy_probe_is_an_error(self):
        class Probed(bridge.FractionArithmetic):
            def probes(self):
                return [{"id": "zero-power-zero", "raw_result": "1", "expected_raw": "Indeterminate", "ok": False}]
        response = bridge.run(self.request(), make_arithmetic=lambda _r: Probed())
        self.assertEqual(response["status"], "oracle-error")
        self.assertIn("zero-power-zero", response["reason"])

    def test_stand_in_runs_without_woxi(self):
        response = bridge.run(self.request(arithmetic="python-fractions"))
        self.assertEqual((response["status"], response["results"][0]["reference"]["n"]), ("ok", "1"))


if __name__ == "__main__":
    unittest.main()
