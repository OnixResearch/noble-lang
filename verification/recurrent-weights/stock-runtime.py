#!/usr/bin/env python3
"""Export admitted Noble F32Select graphs to ONNX and compare on stock ORT CPU.

This is a test-only host boundary, not an ONNX backend or a claim about hybrid state.
The source of nodes and weights is the checked Rust export, not this file.
"""

import argparse
import hashlib
import json
import struct
import time
from pathlib import Path

import numpy as np
import onnx
import onnxruntime as ort
from onnx import TensorProto, helper, numpy_helper

SCHEMA = "noble.recurrent-stock-export/v1"
CODEC = "binary32-exact/u64-bit-le/v1"
U64_MAX = (1 << 64) - 1
BIT_ZERO = 0x00000000
BIT_ONE = 0x3F800000
EXPECTED = {"accumulator": (128, 64, 2), "stack-swap": (128, 128, 0)}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def bit_float(bits):
    return struct.unpack("<f", struct.pack("<I", bits))[0]


def validate_cells(values, length):
    cells = np.asarray(values, dtype=np.float32)
    require(cells.shape == (length,), "wrong cell shape")
    require(np.all((cells.view(np.uint32) == BIT_ZERO) | (cells.view(np.uint32) == BIT_ONE)),
            "non-exact bit cell")
    return cells


def encode_words(words, expected_words):
    require(type(words) in (list, tuple) and len(words) == expected_words,
            "wrong word shape")
    for word in words:
        require(type(word) is int and 0 <= word <= U64_MAX, "out-of-domain word")
    return np.fromiter(((word >> bit) & 1 for word in words for bit in range(64)),
                       dtype=np.float32, count=expected_words * 64)


def decode_words(values, output_cells):
    cells = validate_cells(values, output_cells)
    return [sum(int(cells[start + bit]) << bit for bit in range(64))
            for start in range(0, output_cells, 64)]


def source(program, state, inputs):
    if program == "accumulator":
        require(len(state) == 1 and len(inputs) == 1, "accumulator source shape")
        return [(state[0] + inputs[0]) & U64_MAX]
    require(program == "stack-swap" and len(state) == 2 and not inputs,
            "stack-swap source shape")
    return [state[1], state[0]]


def vector_words(values):
    require(type(values) is list, "malformed vector word list")
    words = []
    for value in values:
        require(type(value) is str and 1 <= len(value) <= 20
                and value.isascii() and value.isdecimal(),
                "noncanonical 64-bit word text")
        word = int(value)
        require(str(word) == value and 0 <= word <= U64_MAX,
                "vector word outside 64-bit domain")
        words.append(word)
    return words


def tensor(name, values, kind):
    return numpy_helper.from_array(np.asarray(values, dtype=kind), name=name)


def export(spec):
    name = spec["program"]
    require(spec["schema"] == SCHEMA and name in EXPECTED, "export schema/program")
    input_cells, output_cells, param_count = EXPECTED[name]
    slots = spec["slots"]
    require(type(slots) is list and len(slots) == input_cells + param_count
            and slots.count(0) == input_cells and slots.count(1) == param_count
            and all(type(slot) is int and slot in (0, 1) for slot in slots),
            "unsupported slot ABI")
    weights = spec["weights"]
    require(len(weights) == param_count
            and all(type(weight) is int and weight in (BIT_ZERO, BIT_ONE) for weight in weights),
            "non-bit snapshot")
    require(struct.pack("<" + "I" * param_count, *weights).hex() == spec["snapshot_bytes"],
            "snapshot bytes differ from the exported weights")
    require(len(bytes.fromhex(spec["graph_id"])) == 32
            and len(bytes.fromhex(spec["snapshot_id"])) == 32
            and len(bytes.fromhex(spec["canonical_graph"])) > 0,
            "missing admitted graph/snapshot identity")
    require(len(spec["outputs"]) == output_cells, "wrong output ABI")

    nodes = []
    initializers = [tensor("one", [1.0], np.float32)]
    value_names = []
    seen_inputs = 0
    seen_params = 0
    guard_count = 0
    for index, op in enumerate(spec["nodes"]):
        require(type(op) is list and op, "malformed node")
        kind = op[0]
        output = f"v{index}"
        if kind in ("input", "param"):
            require(len(op) == 2 and type(op[1]) is int
                    and 0 <= op[1] < len(slots), "malformed slot node")
            if kind == "input":
                require(slots[op[1]] == 0, "input slot mismatch")
                initializers.append(tensor(f"input_index_{index}", [seen_inputs], np.int64))
                nodes.append(helper.make_node("Gather", ["input_bits", f"input_index_{index}"],
                                              [output], axis=0))
                seen_inputs += 1
            else:
                require(slots[op[1]] == 1, "param slot mismatch")
                initializers.append(tensor(output, [bit_float(weights[seen_params])], np.float32))
                seen_params += 1
        elif kind == "const":
            require(len(op) == 2 and type(op[1]) is int
                    and op[1] in (BIT_ZERO, BIT_ONE), "unsupported constant")
            initializers.append(tensor(output, [bit_float(op[1])], np.float32))
        elif kind == "select":
            require(len(op) == 4 and all(type(operand) is int and 0 <= operand < index
                                          and value_names[operand] is not None for operand in op[1:]),
                    "non-canonical select")
            bit, off, on = (value_names[operand] for operand in op[1:])
            nodes.append(helper.make_node("Equal", [bit, "one"], [f"condition_{index}"]))
            nodes.append(helper.make_node("Where", [f"condition_{index}", on, off], [output]))
        elif kind == "guard_bit":
            require(len(op) == 2 and type(op[1]) is int and 0 <= op[1] < index
                    and value_names[op[1]] is not None, "malformed bit guard")
            guard_count += 1
            value_names.append(None)
            continue
        else:
            raise ValueError(f"unsupported graph operator: {kind}")
        value_names.append(output)
    require(seen_inputs == input_cells and seen_params == param_count,
            "graph does not consume its complete slot ABI")
    require(all(type(index) is int and 0 <= index < len(value_names)
                and value_names[index] is not None for index in spec["outputs"]),
            "output references a non-value")
    nodes.append(helper.make_node("Concat", [value_names[index] for index in spec["outputs"]],
                                  ["output_bits"], axis=0))
    graph = helper.make_graph(nodes, name + "-checked-ngraph-f32", [
        helper.make_tensor_value_info("input_bits", TensorProto.FLOAT, [input_cells])
    ], [helper.make_tensor_value_info("output_bits", TensorProto.FLOAT, [output_cells])],
        initializer=initializers)
    model = helper.make_model(graph, ir_version=10, producer_name="noble-recurrent-export/v1",
                              opset_imports=[helper.make_operatorsetid("", 18)])
    onnx.checker.check_model(model)
    return model.SerializeToString(deterministic=True), guard_count


class StockEngine:
    def __init__(self, spec, path, expected_sha):
        self.program = spec["program"]
        self.input_cells, self.output_cells, _ = EXPECTED[self.program]
        self.model_sha = expected_sha
        self.path = path
        self.assert_model_identity()
        options = ort.SessionOptions()
        options.intra_op_num_threads = 1
        options.inter_op_num_threads = 1
        options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
        started = time.perf_counter_ns()
        self.session = ort.InferenceSession(str(path), sess_options=options,
                                            providers=["CPUExecutionProvider"])
        self.load_ns = time.perf_counter_ns() - started
        require(self.session.get_providers() == ["CPUExecutionProvider"],
                "wrong inference provider")
        require(len(self.session.get_inputs()) == 1 and
                self.session.get_inputs()[0].name == "input_bits" and
                self.session.get_inputs()[0].shape == [self.input_cells] and
                len(self.session.get_outputs()) == 1 and
                self.session.get_outputs()[0].name == "output_bits" and
                self.session.get_outputs()[0].shape == [self.output_cells],
                "runtime graph ABI differs")

    def assert_model_identity(self):
        require(sha(self.path.read_bytes()) == self.model_sha, "model identity mismatch")

    def step_cells(self, cells):
        self.assert_model_identity()
        encoded = validate_cells(cells, self.input_cells)
        result = self.session.run(["output_bits"], {"input_bits": encoded})
        require(len(result) == 1, "runtime observation shape")
        return decode_words(result[0], self.output_cells)

    def step(self, state, inputs, *, declared_program=None, codec=CODEC,
             budget=1, cancelled=False):
        require(declared_program is None or declared_program == self.program,
                "program configuration mismatch")
        require(codec == CODEC, "codec configuration mismatch")
        require(type(budget) is int and budget > 0 and not cancelled,
                "budget exhausted or cancelled")
        expected_state_words = self.output_cells // 64
        expected_input_words = (self.input_cells - self.output_cells) // 64
        require(type(state) in (list, tuple) and len(state) == expected_state_words
                and type(inputs) in (list, tuple) and len(inputs) == expected_input_words,
                "state/input word shape mismatch")
        if self.program == "accumulator":
            cells = np.empty(128, dtype=np.float32)
            cells[0::2] = encode_words(state, 1)
            cells[1::2] = encode_words(inputs, 1)
        else:
            cells = encode_words(state, 2)
        return self.step_cells(cells)


def restore(checkpoint, engine):
    require(type(checkpoint) is dict and set(checkpoint) == {
        "index", "phase", "program", "codec", "model_sha256", "state", "remaining_budget"
    }, "checkpoint is incomplete or has hybrid/extraneous state")
    index = checkpoint["index"]
    require(type(index) is int and 0 <= index <= 1000
            and checkpoint["phase"] == "completed_transition", "illegal snapshot index")
    require(checkpoint["program"] == engine.program and checkpoint["codec"] == CODEC
            and checkpoint["model_sha256"] == engine.model_sha,
            "checkpoint configuration/weights mismatch")
    require(type(checkpoint["remaining_budget"]) is int
            and checkpoint["remaining_budget"] == 1000 - index, "checkpoint budget mismatch")
    encode_words(checkpoint["state"], engine.output_cells // 64)
    return checkpoint["state"]


def refused(label, operation, results):
    try:
        operation()
    except (ValueError, TypeError, RuntimeError, OSError) as error:
        results.append({"control": label, "refused": True, "reason": str(error)})
        return
    raise AssertionError(f"control silently accepted: {label}")


def controls(engine, spec, model_bytes, output_dir):
    results = []
    valid = engine.step([0] if engine.program == "accumulator" else [1, 2],
                        [1] if engine.program == "accumulator" else [])
    invalid = encode_words([0] if engine.program == "accumulator" else [1, 2],
                           engine.output_cells // 64)
    if engine.program == "accumulator":
        invalid = np.repeat(invalid, 2)
    for label, bits in [("nan", 0x7FC00000), ("negative-zero", 0x80000000),
                        ("fractional", 0x3F000000)]:
        bad = invalid.copy()
        bad[0] = bit_float(bits)
        refused("corrupted-state-" + label, lambda bad=bad: engine.step_cells(bad), results)
    refused("short-input-cells", lambda: engine.step_cells(invalid[:-1]), results)
    refused("wrong-state-shape", lambda: engine.step([], [1]), results)
    refused("wrong-input-shape", lambda: engine.step(
        [0] if engine.program == "accumulator" else [1, 2],
        [] if engine.program == "accumulator" else [1]), results)
    refused("wrong-program", lambda: engine.step(
        [0] if engine.program == "accumulator" else [1, 2],
        [1] if engine.program == "accumulator" else [], declared_program="wrong"), results)
    refused("wrong-decoder", lambda: engine.step(
        [0] if engine.program == "accumulator" else [1, 2],
        [1] if engine.program == "accumulator" else [], codec="big-endian"), results)
    refused("wrong-output-length", lambda: decode_words(np.zeros(engine.output_cells - 1),
                                                        engine.output_cells), results)
    malformed_output = np.zeros(engine.output_cells, dtype=np.float32)
    malformed_output[0] = np.float32(0.5)
    refused("malformed-output", lambda: decode_words(malformed_output,
                                                     engine.output_cells), results)
    refused("zero-step-budget", lambda: engine.step(
        [0] if engine.program == "accumulator" else [1, 2],
        [1] if engine.program == "accumulator" else [], budget=0), results)
    refused("boundary-cancellation", lambda: engine.step(
        [0] if engine.program == "accumulator" else [1, 2],
        [1] if engine.program == "accumulator" else [], cancelled=True), results)

    corrupt_path = output_dir / (engine.program + "-corrupt.onnx")
    corrupt_path.write_bytes(model_bytes[:-1] + bytes([model_bytes[-1] ^ 1]))
    original_path = engine.path
    engine.path = corrupt_path
    try:
        refused("corrupted-model-bytes", engine.assert_model_identity, results)
    finally:
        engine.path = original_path

    state = [0] if engine.program == "accumulator" else [1, 2]
    base = {"index": 0, "phase": "completed_transition", "program": engine.program,
            "codec": CODEC, "model_sha256": engine.model_sha,
            "state": state, "remaining_budget": 1000}
    require(restore(base, engine) == state, "initial boundary restore")
    require(restore({**base, "index": 1000, "remaining_budget": 0}, engine) == state,
            "request-end boundary restore")
    third = spec["vectors"]["teacher_forced"][2]
    resumed = restore({**base, "index": 2, "remaining_budget": 998,
                       "state": vector_words(third[0])}, engine)
    require(engine.step(resumed, vector_words(third[1]), budget=998) == vector_words(third[2]),
            "resumed stock execution differs from Noble/source")
    refused("request-end-budget-exhausted", lambda: engine.step(
        state, [1] if engine.program == "accumulator" else [], budget=0), results)
    for label, variant in [
        ("missing-recurrent-state", {k: v for k, v in base.items() if k != "state"}),
        ("kv-without-recurrent", {**{k: v for k, v in base.items() if k != "state"},
                                  "kv": [1, 2]}),
        ("scratch-without-recurrent", {**{k: v for k, v in base.items() if k != "state"},
                                       "scratch": [1, 2]}),
        ("hybrid-material-unsupported", {**base, "kv": [1, 2]}),
        ("negative-snapshot-index", {**base, "index": -1}),
        ("past-last-snapshot-index", {**base, "index": 1001}),
        ("mid-graph-snapshot", {**base, "phase": "mid_graph"}),
        ("wrong-checkpoint-program", {**base, "program": "wrong"}),
        ("wrong-checkpoint-weights", {**base, "model_sha256": "0" * 64}),
        ("wrong-checkpoint-budget", {**base, "remaining_budget": 999}),
        ("wrong-checkpoint-state-shape", {**base, "state": []}),
    ]:
        refused(label, lambda variant=variant: restore(variant, engine), results)
    require(valid == source(engine.program, state,
                            [1] if engine.program == "accumulator" else []),
            "complete boundary continuation differs")
    return results


def compare_vectors(engine, spec):
    coverage = {}
    for group, rows in spec["vectors"].items():
        require(group in ("tiny", "boundary", "teacher_forced", "free_running"),
                "unregistered vector group")
        expected_count = (6 if engine.program == "accumulator" else 0) if group == "boundary" else (
            16 if group == "tiny" else 1000)
        require(len(rows) == expected_count,
                "missing predeclared vectors")
        state = None
        started = time.perf_counter_ns()
        for position, row in enumerate(rows):
            require(type(row) is list and len(row) == 3, "malformed Noble vector")
            original_state, inputs, noble_output = (vector_words(part) for part in row)
            require(source(engine.program, original_state, inputs) == noble_output,
                    f"Noble vector disagrees with independent source: {group} {position}")
            if group == "free_running":
                require(state is None or original_state == state,
                        "Noble trajectory did not preserve target feedback")
                current = original_state if state is None else state
            else:
                current = original_state
            runtime_output = engine.step(current, inputs)
            require(runtime_output == noble_output,
                    f"stock/Noble/source mismatch: {engine.program} {group} {position}")
            if group == "free_running":
                state = runtime_output
        elapsed = time.perf_counter_ns() - started
        coverage[group] = {"transitions": len(rows), "mismatches": 0,
                           "elapsed_ns": elapsed, "ns_per_transition":
                           elapsed / len(rows) if rows else None}
    return coverage


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--export", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    require(ort.__version__ == "1.23.2" and onnx.__version__ == "1.20.1",
            "unselected ONNX package versions")
    specs = json.loads(Path(args.export).read_text())
    require(type(specs) is list and [spec["program"] for spec in specs] ==
            ["accumulator", "stack-swap"], "missing or reordered checked graph")
    output_dir = Path(args.output_dir)
    output_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    results = []
    for spec in specs:
        model_bytes, guard_count = export(spec)
        require(export(spec)[0] == model_bytes, "nondeterministic ONNX export")
        path = output_dir / (spec["program"] + ".onnx")
        path.write_bytes(model_bytes)
        engine = StockEngine(spec, path, sha(model_bytes))
        coverage = compare_vectors(engine, spec)
        refusals = controls(engine, spec, model_bytes, output_dir)
        results.append({"program": spec["program"], "model_sha256": sha(model_bytes),
                        "model_bytes": len(model_bytes), "model_path": str(path),
                        "graph_sha256": sha(bytes.fromhex(spec["canonical_graph"])),
                        "graph_id": spec["graph_id"], "snapshot_id": spec["snapshot_id"],
                        "snapshot_sha256": sha(bytes.fromhex(spec["snapshot_bytes"])),
                        "guard_count": guard_count, "runtime_load_ns": engine.load_ns,
                        "coverage": coverage, "refusals": refusals,
                        "onnx_ir": 10, "onnx_opset": 18,
                        "provider": engine.session.get_providers()})
    print(json.dumps({"schema": "noble.recurrent-stock-runtime/v1",
                      "runtime": {"onnxruntime": ort.__version__, "onnx": onnx.__version__,
                                  "numpy": np.__version__, "provider": "CPUExecutionProvider",
                                  "optimizations": "disabled", "thread_count": 1},
                      "coverage": {"dense": "none", "sparse": "none",
                                   "stock": "two exported ONNX graphs: tiny domains, boundaries, "
                                            "1,000 teacher-forced and 1,000 free-running transitions per program"},
                      "results": results}, sort_keys=True))


if __name__ == "__main__":
    main()
