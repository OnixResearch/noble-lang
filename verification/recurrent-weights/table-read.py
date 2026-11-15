#!/usr/bin/env python3
"""Checked Noble F32Index table transition on selected, single-thread ORT CPU.

The numeric operand and one-hot query are graph inputs linked by a checked
zero-difference guard. ONNX lacks Noble's refusal operator: exact input facts
and that guard are checked at this host boundary before inference. No source
lookup supplies a model result; the exported F32Index becomes Gather/Where.
"""

import argparse
import copy
import hashlib
import json
import struct
import time
from pathlib import Path

import numpy as np
import onnx
import onnxruntime as ort
from onnx import TensorProto, helper, numpy_helper

SCHEMA = "noble.recurrent-table-export/v1"
CODEC = "binary32-exact/one-hot-ascii-ab-0..3/v1"
KEYS = ("", "A", "B", "AA", "AB", "BA", "BB", "AAA", "AAB", "ABA", "ABB",
        "BAA", "BAB", "BBA", "BBB")
ENTRIES = {"A": 3, "BB": 5, "ABA": 9}  # Independent source relation, never used by export.
NEAR = {"A": ("", "AA", "AB", "BA"), "BB": ("B", "ABB", "BBA", "BBB"),
        "ABA": ("AB", "BA")}
CELLS = 19
STEPS = 64
F32 = np.dtype("float32")
BIT_ZERO = 0x00000000
BIT_ONE = 0x3F800000


def require(ok, reason):
    if not ok:
        raise ValueError(reason)


class Counterexample(ValueError):
    def __init__(self, reason, row):
        super().__init__(reason)
        self.row = row


def sha(blob):
    return hashlib.sha256(blob).hexdigest()


def as_float(bits):
    return struct.unpack("<f", struct.pack("<I", bits))[0]


def tensor(name, values, dtype):
    return numpy_helper.from_array(np.asarray(values, dtype=dtype), name=name)


def key_index(key):
    require(type(key) is str and len(key) <= 3 and all(letter in "AB" for letter in key),
            "key must be ASCII A/B, length <= 3")
    suffix = 0
    for letter in key:
        suffix = suffix * 2 + int(letter == "B")
    return (1 << len(key)) - 1 + suffix


def source(state, key):
    require(type(state) is int and 0 <= state <= 1024, "source state range")
    key_index(key)
    hit = ENTRIES.get(key, 0)
    return [state + hit, hit]


def validate_spec(spec):
    require(type(spec) is dict and spec.get("schema") == SCHEMA
            and spec.get("program") == "table-read" and spec.get("codec") == CODEC
            and spec.get("query_shape") == "one-hot-17"
            and spec.get("index_range") == [-1, 15] and spec.get("max_steps") == STEPS
            and spec.get("slots") == CELLS and spec.get("snapshot_bytes") == "",
            "unsupported checked table graph ABI/configuration")
    for field in ("graph_id", "snapshot_id"):
        require(type(spec.get(field)) is str and len(spec[field]) == 64
                and all(digit in "0123456789abcdef" for digit in spec[field]),
                field + " is not a checked identity")
    canonical = spec.get("canonical_graph")
    require(type(canonical) is str and len(canonical) > 0 and len(canonical) % 2 == 0
            and all(digit in "0123456789abcdef" for digit in canonical),
            "missing canonical checked graph bytes")
    nodes = spec.get("nodes")
    require(type(nodes) is list and len(nodes) == 28
            and nodes[:CELLS] == [["input", n] for n in range(CELLS)]
            and spec.get("outputs") == [27, 26], "table graph shape/output ABI mismatch")
    # Narrow, audited subset of the checked IR. Each emitted value is traced to
    # an actual node; unknown ops, wrong one-hot linkage, and other index rules
    # cannot silently become a Python dictionary lookup.
    affine = nodes[19]
    require(affine == ["affine", BIT_ZERO,
                       [[int(np.float32(position - 1).view(np.uint32)), position + 2]
                        for position in range(17)]]
            and nodes[20] == ["sub", 1, 19]
            and nodes[21] == ["guard_zero", 20]
            and nodes[27] == ["add", 0, 26],
            "index/one-hot binding or transition was not checked")
    for index in range(22, 26):
        node = nodes[index]
        require(type(node) is list and len(node) == 2 and node[0] == "const"
                and type(node[1]) is int and node[1] in (BIT_ZERO, 0x40400000,
                                                          0x40A00000, 0x41100000),
                "table constant is not an exact small integer")
    index_node = nodes[26]
    require(type(index_node) is list and len(index_node) == 3
            and index_node[:2] == ["index_zero", 1]
            and type(index_node[2]) is list and len(index_node[2]) == 15
            and all(type(value) is int and value in range(22, 26)
                    for value in index_node[2]),
            "F32Index missing 15 candidates or Zero fallback")
    return nodes


def export(spec):
    nodes = validate_spec(spec)
    graph_nodes = []
    initializers = []
    names = []
    for number, op in enumerate(nodes):
        name = f"v{number}"
        kind = op[0]
        if kind == "input":
            initializers.append(tensor(f"input_at_{number}", [op[1]], np.int64))
            graph_nodes.append(helper.make_node("Gather", ["input_cells", f"input_at_{number}"],
                                                [name], axis=0))
        elif kind == "const":
            initializers.append(tensor(name, [as_float(op[1])], F32))
        elif kind == "affine":
            initializers.append(tensor("affine_bias", [as_float(op[1])], F32))
            previous = "affine_bias"
            for step, (coefficient, operand) in enumerate(op[2]):
                weight = f"coefficient_{step}"
                product = f"product_{step}"
                result = name if step == len(op[2]) - 1 else f"sum_{step}"
                initializers.append(tensor(weight, [as_float(coefficient)], F32))
                graph_nodes.append(helper.make_node("Mul", [weight, names[operand]], [product]))
                graph_nodes.append(helper.make_node("Add", [previous, product], [result]))
                previous = result
        elif kind in ("sub", "add"):
            graph_nodes.append(helper.make_node("Sub" if kind == "sub" else "Add",
                                                [names[op[1]], names[op[2]]], [name]))
        elif kind == "guard_zero":
            # Exact one-hot and matching numeric index are validated by step_cells.
            names.append(None)
            continue
        elif kind == "index_zero":
            # Noble semantics: integral -1 or >= 15 returns +0, not a wrapped
            # negative Gather or a clamped table value. Inputs already satisfy
            # Integral/Range and one-hot agreement before ORT sees them.
            graph_nodes.append(helper.make_node("Concat", [names[n] for n in op[2]],
                                                ["candidate_table"], axis=0))
            graph_nodes.append(helper.make_node("Cast", [names[op[1]]], ["index_i64"],
                                                to=TensorProto.INT64))
            initializers.extend((tensor("min_index", [0], np.int64),
                                 tensor("last_index", [14], np.int64),
                                 tensor("end_index", [15], np.int64),
                                 tensor("fallback_zero", [0.0], F32)))
            graph_nodes.append(helper.make_node("Max", ["index_i64", "min_index"], ["floor_i64"]))
            graph_nodes.append(helper.make_node("Min", ["floor_i64", "last_index"], ["safe_i64"]))
            graph_nodes.append(helper.make_node("Gather", ["candidate_table", "safe_i64"],
                                                ["candidate"], axis=0))
            graph_nodes.append(helper.make_node("GreaterOrEqual", ["index_i64", "min_index"],
                                                ["not_below"]))
            graph_nodes.append(helper.make_node("Less", ["index_i64", "end_index"],
                                                ["not_above"]))
            graph_nodes.append(helper.make_node("And", ["not_below", "not_above"], ["inside"]))
            graph_nodes.append(helper.make_node("Where", ["inside", "candidate", "fallback_zero"],
                                                [name]))
        else:
            raise ValueError("unsupported checked graph operator: " + str(kind))
        names.append(name)
    graph_nodes.append(helper.make_node("Concat", [names[index] for index in spec["outputs"]],
                                        ["next_and_hit"], axis=0))
    graph = helper.make_graph(graph_nodes, "noble-checked-f32index-table-read", [
        helper.make_tensor_value_info("input_cells", TensorProto.FLOAT, [CELLS])
    ], [helper.make_tensor_value_info("next_and_hit", TensorProto.FLOAT, [2])],
        initializer=initializers)
    model = helper.make_model(graph, ir_version=10, producer_name="noble-table-f32index/v1",
                              opset_imports=[helper.make_operatorsetid("", 18)])
    onnx.checker.check_model(model)
    return model.SerializeToString(deterministic=True)


def cells_for(state, index):
    require(type(state) is int and 0 <= state <= 1024, "state must be 0..1024 integer")
    require(type(index) is int and -1 <= index <= 15, "numeric index outside checked range")
    cells = np.zeros(CELLS, dtype=F32)
    cells[0] = state
    cells[1] = index
    cells[index + 3] = 1.0
    return cells


class Engine:
    def __init__(self, path, model_sha):
        self.path = path
        self.model_sha = model_sha
        self.check_identity()
        options = ort.SessionOptions()
        options.intra_op_num_threads = 1
        options.inter_op_num_threads = 1
        options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
        started = time.perf_counter_ns()
        self.session = ort.InferenceSession(str(path), sess_options=options,
                                            providers=["CPUExecutionProvider"])
        self.load_ns = time.perf_counter_ns() - started
        require(self.session.get_providers() == ["CPUExecutionProvider"], "wrong CPU kernel")
        ins, outs = self.session.get_inputs(), self.session.get_outputs()
        require(len(ins) == len(outs) == 1 and ins[0].name == "input_cells"
                and ins[0].shape == [CELLS] and outs[0].name == "next_and_hit"
                and outs[0].shape == [2] and ins[0].type == outs[0].type == "tensor(float)",
                "ONNX F32Index table ABI or dtype mismatch")

    def check_identity(self):
        require(sha(self.path.read_bytes()) == self.model_sha, "table model digest mismatch")

    def step_cells(self, raw):
        self.check_identity()
        cells = np.asarray(raw)
        require(cells.dtype == F32 and cells.shape == (CELLS,), "binary32/19-cell input required")
        state, index = float(cells[0]), float(cells[1])
        require(np.isfinite(state) and state.is_integer() and 0 <= state <= 1024,
                "invalid numeric state")
        require(np.isfinite(index) and index.is_integer() and abs(index) < (1 << 24)
                and -1 <= index <= 15, "fractional, NaN, or out-of-range index")
        bits = cells[2:].view(np.uint32)
        require(np.all((bits == BIT_ZERO) | (bits == BIT_ONE)) and
                np.count_nonzero(bits == BIT_ONE) == 1, "query must be exact one-hot")
        require(int(np.argmax(bits == BIT_ONE)) - 1 == index,
                "numeric index disagrees with checked one-hot key")
        started = time.perf_counter_ns()
        result = self.session.run(["next_and_hit"], {"input_cells": cells})[0]
        elapsed = time.perf_counter_ns() - started
        require(result.dtype == F32 and result.shape == (2,), "output dtype/shape mismatch")
        require(np.all(np.isfinite(result)) and np.all(result == np.floor(result))
                and np.all(result >= 0) and result[0] <= 1033 and result[1] <= 9,
                "nonintegral or out-of-range table observation")
        return [int(result[0]), int(result[1])], elapsed

    def step(self, state, key, *, program="table-read", codec=CODEC,
             budget=1, cancelled=False):
        require(program == "table-read" and codec == CODEC, "wrong table program/codec")
        require(type(budget) is int and budget >= 1 and not cancelled,
                "table step budget or cancellation")
        return self.step_cells(cells_for(state, key_index(key)))

    def run(self, state, keys, *, budget=STEPS, cancelled=False):
        require(type(keys) is list and 1 <= len(keys) <= STEPS and type(budget) is int
                and len(keys) <= budget and budget <= STEPS and not cancelled,
                "table semantic-step budget/cancellation")
        states = []
        elapsed = 0
        for key in keys:
            observed, ns = self.step(state, key)
            states.append(observed)
            state = observed[0]  # ORT-produced state, never a source/teacher-forced state.
            elapsed += ns
        return states, {"host_calls": len(keys), "semantic_steps": len(keys),
                        "ort_run_ns": elapsed, "input_bytes": len(keys) * CELLS * 4,
                        "output_bytes": len(keys) * 2 * 4}


def compare(engine, spec):
    tiny = spec["vectors"]["tiny"]
    require(type(tiny) is list and len(tiny) == 17, "finite tiny domain incomplete")
    tiny_ns = 0
    for expected_index, row in zip(range(-1, 16), tiny):
        require(type(row) is list and len(row) == 5, "invalid Noble tiny row")
        key, index, state, next_state, hit = row
        require(type(index) is int and index == expected_index and type(state) is int
                and state == 0 and type(next_state) is int and type(hit) is int,
                "tiny row key/index/state")
        if index in (-1, 15):
            require(key is None and [next_state, hit] == [0, 0], "Zero fallback vector")
        else:
            require(key == KEYS[index] and key_index(key) == index
                    and [next_state, hit] == source(state, key), "source/Noble tiny mismatch")
        actual, elapsed = engine.step_cells(cells_for(state, index))
        if actual != [next_state, hit]:
            raise Counterexample("ORT/Noble tiny mismatch",
                                 {"key": key, "numeric_index": index, "state": state,
                                  "expected": [next_state, hit], "actual": actual})
        tiny_ns += elapsed
    near_pairs = 0
    for key, neighbours in NEAR.items():
        expected = source(0, key)[1]
        require(expected != 0, "declared key unexpectedly absent")
        for neighbour in neighbours:
            actual, elapsed = engine.step(0, neighbour)
            if not (actual[1] == source(0, neighbour)[1] == 0 and actual[1] != expected):
                raise Counterexample("near-miss misroute",
                                     {"key": key, "neighbour": neighbour,
                                      "expected_hit": 0, "actual_hit": actual[1]})
            tiny_ns += elapsed
            near_pairs += 1
    free = spec["vectors"]["free_running"]
    require(type(free) is list and len(free) == STEPS, "missing 64 source-bound free rows")
    keys = [KEYS[(step * 7 + step // 4) % len(KEYS)] for step in range(STEPS)]
    current = 0
    for step, (key, row) in enumerate(zip(keys, free)):
        require(type(row) is list and len(row) == 5 and row[0] == key
                and row[1] == key_index(key) and row[2] == current
                and row[3:] == source(current, key),
                f"source/Noble free-running vector mismatch at step {step}")
        current = row[3]
    started = time.perf_counter_ns()
    observed, cost = engine.run(0, keys, budget=STEPS)
    cost["end_to_end_ns"] = time.perf_counter_ns() - started
    for step, (actual, row) in enumerate(zip(observed, free)):
        if actual != row[3:]:
            raise Counterexample("ORT/Noble/source free-run mismatch",
                                 {"step": step, "key": row[0], "state": row[2],
                                  "expected": row[3:], "actual": actual})
    return {"tiny": {"transitions": len(tiny), "mismatches": 0, "ort_run_ns": tiny_ns},
            "near_miss": {"pairs": near_pairs, "misroutes": 0},
            "free_running": {"transitions": STEPS, "mismatches": 0, **cost}}


def controls(engine, spec, model_bytes, output_dir):
    refused = []

    def check(label, action):
        try:
            action()
        except (ValueError, TypeError, RuntimeError, OSError) as error:
            refused.append({"control": label, "refused": True, "reason": str(error)})
            return
        raise AssertionError("control silently accepted: " + label)

    for label, change in (("schema", {"schema": "wrong"}),
                          ("program", {"program": "accumulator"}),
                          ("codec", {"codec": "wrong"}),
                          ("index-range", {"index_range": [-1, 16]}),
                          ("query-shape", {"query_shape": "k-hot"})):
        check("corrupt-spec-" + label, lambda change=change: export({**spec, **change}))
    tampered = copy.deepcopy(spec)
    tampered["nodes"][26][0] = "index_clamp"
    check("corrupt-index-rule", lambda: export(tampered))
    for key in ("ABAB", "X", "Ä", 12):
        check("malformed-key-" + repr(key), lambda key=key: engine.step(0, key))
    check("wrong-state-shape", lambda: engine.step([0], "A"))
    check("invalid-state", lambda: engine.step(1025, "A"))
    check("wrong-program", lambda: engine.step(0, "A", program="accumulator"))
    check("wrong-codec", lambda: engine.step(0, "A", codec="other"))
    check("zero-budget", lambda: engine.step(0, "A", budget=0))
    check("one-step-excess", lambda: engine.run(0, ["A"] * 65, budget=64))
    check("short-budget", lambda: engine.run(0, ["A"] * 64, budget=63))
    check("cancelled", lambda: engine.run(0, ["A"], cancelled=True))
    good = cells_for(0, 1)
    check("wrong-cell-shape", lambda: engine.step_cells(good[:-1]))
    check("coerced-dtype", lambda: engine.step_cells(good.astype(np.float64)))
    for label, value in (("fractional", 0.5), ("NaN", np.nan), ("negative-over", -2.0),
                         ("positive-over", 16.0), ("precision-bound", float(1 << 24))):
        bad = good.copy()
        bad[1] = value
        check("invalid-index-" + label, lambda bad=bad: engine.step_cells(bad))
    for label, value in (("fractional", 0.5), ("NaN", np.nan), ("out-of-range", 1025.0)):
        bad = good.copy()
        bad[0] = value
        check("invalid-state-" + label, lambda bad=bad: engine.step_cells(bad))
    for label, value in (("fractional", 0.5), ("NaN", np.nan), ("negative-zero", -0.0)):
        bad = good.copy()
        bad[4] = value
        check("invalid-onehot-" + label, lambda bad=bad: engine.step_cells(bad))
    for label, edit in (("all-zero", lambda cells: cells.__setitem__(4, 0.0)),
                        ("two-hot", lambda cells: cells.__setitem__(5, 1.0)),
                        ("index-mismatch", lambda cells: cells.__setitem__(1, 2.0))):
        bad = good.copy()
        edit(bad)
        check("invalid-onehot-" + label, lambda bad=bad: engine.step_cells(bad))
    altered = output_dir / "table-read-corrupt.onnx"
    altered.write_bytes(model_bytes[:-1] + bytes([model_bytes[-1] ^ 1]))
    original = engine.path
    engine.path = altered
    try:
        check("corrupt-model-digest", engine.check_identity)
    finally:
        engine.path = original
    return refused


def seeded_fault(spec, data, output_dir):
    model = onnx.load_model_from_string(data)
    candidate = next(item for item in model.graph.initializer if item.name == "v23")
    candidate.CopyFrom(tensor("v23", [0.0], F32))
    onnx.checker.check_model(model)
    wrong = model.SerializeToString(deterministic=True)
    path = output_dir / "table-read-seeded-wrong.onnx"
    path.write_bytes(wrong)
    engine = Engine(path, sha(wrong))
    first = None
    for row in spec["vectors"]["tiny"]:
        key, index, state, next_state, hit = row
        actual, _ = engine.step_cells(cells_for(state, index))
        if actual != [next_state, hit]:
            first = {"key": key, "numeric_index": index,
                     "expected": [next_state, hit], "actual": actual}
            break
    require(first is not None, "seeded wrong F32Index table passed the finite domain")
    return {"model_sha256": sha(wrong), "first_counterexample": first}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--export", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    require(ort.__version__ == "1.23.2" and onnx.__version__ == "1.20.1",
            "table-read pilot requires selected ORT/ONNX Python closure")
    output_dir = Path(args.output_dir).resolve()
    require(not output_dir.is_relative_to(Path(__file__).resolve().parents[2]),
            "table-read model outputs must be outside the source tree")
    output_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    stage = "checked-export-admission"
    try:
        spec = json.loads(Path(args.export).read_text())
        started = time.perf_counter_ns()
        data = export(spec)
        export_ns = time.perf_counter_ns() - started
        require(export(spec) == data, "nondeterministic checked ONNX export")
        path = output_dir / "table-read.onnx"
        path.write_bytes(data)
        stage = "selected-ort-cpu-load"
        engine = Engine(path, sha(data))
        stage = "source-noble-ort-comparison"
        coverage = compare(engine, spec)
        stage = "corruption-and-config-controls"
        rejected = controls(engine, spec, data, output_dir)
        stage = "seeded-fault-detection"
        fault = seeded_fault(spec, data, output_dir)
        report = {"schema": "noble.recurrent-table-read/v1", "decision": "GO",
                  "property": "exact finite F32Index table with Zero fallback and one-hot key",
                  "graph_id": spec["graph_id"], "snapshot_id": spec["snapshot_id"],
                  "canonical_graph_sha256": sha(bytes.fromhex(spec["canonical_graph"])),
                  "model_sha256": sha(data), "model_bytes": len(data), "model_path": str(path),
                  "source_export_sha256": sha(Path(args.export).read_bytes()),
                  "export_ns": export_ns, "load_ns": engine.load_ns,
                  "runtime": {"onnxruntime": ort.__version__, "onnx": onnx.__version__,
                              "provider": "CPUExecutionProvider", "kernel": "Gather/Where; F32",
                              "dtype": "binary32", "opset": 18, "ir": 10,
                              "thread_count": 1, "optimizations": "disabled"},
                  "numeric_budget": {"integral_index_magnitude_lt": 1 << 24,
                                     "admitted_index": [-1, 15], "zero_fallback": [-1, 15],
                                     "max_key_bytes": 3, "max_semantic_steps": STEPS},
                  "query_shape": "one-hot-17", "table_entries": 15,
                  "parameter_elements": 0, "parameter_nonzero": 0,
                  "guard_elision": "one-hot/input facts and exact-zero index binding validated before ORT",
                  "coverage": coverage, "controls": rejected, "seeded_fault": fault,
                  "stock_scope": "separate post-accumulator pilot; no pretrained merge; "
                                 "64 ORT calls carry state through the host"}
    except Exception as error:
        # Do not substitute a Python dictionary or silently weaken the shape:
        # preserve the first failing operation and a machine-readable NO-GO.
        report = {"schema": "noble.recurrent-table-read/v1", "decision": "NO-GO",
                  "failure": {"stage": stage, "kind": type(error).__name__,
                              "reason": str(error),
                              **({"first_counterexample": error.row}
                                 if isinstance(error, Counterexample) else {})}}
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
