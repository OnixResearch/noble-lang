#!/usr/bin/env python3
"""Execute checked Noble transitions with ONNX Scan-carried state, not a host loop."""

import argparse
import copy
import importlib.util
import json
import time
from pathlib import Path

import numpy as np
import onnx
import onnxruntime as ort
from onnx import TensorProto, helper, numpy_helper

module_spec = importlib.util.spec_from_file_location(
    "stock_runtime", Path(__file__).with_name("stock-runtime.py"))
stock = importlib.util.module_from_spec(module_spec)
module_spec.loader.exec_module(stock)


def build_scan(spec):
    started = time.perf_counter_ns()
    one_step_bytes, guards = stock.export(spec)
    one_step = onnx.load_model_from_string(one_step_bytes)
    program = spec["program"]
    state_cells = stock.EXPECTED[program][1]
    token_cells = 64 if program == "accumulator" else 1
    initializers = {item.name: copy.deepcopy(item) for item in one_step.graph.initializer}
    body_nodes = []
    for original in one_step.graph.node:
        node = copy.deepcopy(original)
        if node.op_type == "Gather" and node.input[0] == "input_bits":
            index_name = node.input[1]
            index = int(numpy_helper.to_array(initializers[index_name])[0])
            if program == "accumulator":
                state_slot = index % 2 == 0
                local_index = index // 2
            else:
                state_slot = True
                local_index = index
            node.input[0] = "body_state_bits" if state_slot else "body_token_bits"
            initializers[index_name] = stock.tensor(index_name, [local_index], np.int64)
        body_nodes.append(node)
    body_nodes.append(helper.make_node("Identity", ["output_bits"], ["body_next_state_bits"]))
    body_nodes.append(helper.make_node("Identity", ["output_bits"], ["body_observation_bits"]))
    body = helper.make_graph(body_nodes, program + "-one-step-body", [
        helper.make_tensor_value_info("body_state_bits", TensorProto.FLOAT, [state_cells]),
        helper.make_tensor_value_info("body_token_bits", TensorProto.FLOAT, [token_cells]),
    ], [
        helper.make_tensor_value_info("body_next_state_bits", TensorProto.FLOAT, [state_cells]),
        helper.make_tensor_value_info("body_observation_bits", TensorProto.FLOAT, [state_cells]),
    ], initializer=list(initializers.values()))
    outer = helper.make_graph([
        helper.make_node("Scan", ["initial_state_bits", "scan_inputs"],
                         ["final_state_bits", "observations"], body=body, num_scan_inputs=1)
    ], program + "-recurrent-scan", [
        helper.make_tensor_value_info("initial_state_bits", TensorProto.FLOAT, [state_cells]),
        helper.make_tensor_value_info("scan_inputs", TensorProto.FLOAT, ["steps", token_cells]),
    ], [
        helper.make_tensor_value_info("final_state_bits", TensorProto.FLOAT, [state_cells]),
        helper.make_tensor_value_info("observations", TensorProto.FLOAT, ["steps", state_cells]),
    ])
    model = helper.make_model(outer, producer_name="noble-recurrent-scan/v1", ir_version=10,
                              opset_imports=[helper.make_operatorsetid("", 18)])
    onnx.checker.check_model(model)
    return model.SerializeToString(deterministic=True), guards, time.perf_counter_ns() - started


def rss_bytes(field):
    for line in Path("/proc/self/status").read_text().splitlines():
        if line.startswith(field + ":"):
            return int(line.split()[1]) * 1024
    raise ValueError("missing process RSS field " + field)


class ScanEngine:
    def __init__(self, spec, path, digest):
        self.program = spec["program"]
        self.state_cells = stock.EXPECTED[self.program][1]
        self.output_cells = self.state_cells
        self.token_cells = 64 if self.program == "accumulator" else 1
        self.model_sha = digest
        self.path = path
        self.assert_model_identity()
        options = ort.SessionOptions()
        options.intra_op_num_threads = 1
        options.inter_op_num_threads = 1
        options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
        start = time.perf_counter_ns()
        self.session = ort.InferenceSession(str(path), sess_options=options,
                                            providers=["CPUExecutionProvider"])
        self.load_ns = time.perf_counter_ns() - start
        stock.require(self.session.get_providers() == ["CPUExecutionProvider"], "wrong provider")
        inputs = self.session.get_inputs()
        outputs = self.session.get_outputs()
        stock.require([item.name for item in inputs] == ["initial_state_bits", "scan_inputs"]
                      and inputs[0].shape == [self.state_cells]
                      and inputs[1].shape == ["steps", self.token_cells]
                      and [item.name for item in outputs] == ["final_state_bits", "observations"]
                      and outputs[0].shape == [self.state_cells]
                      and outputs[1].shape == ["steps", self.state_cells],
                      "recurrent graph/state ABI mismatch")

    def assert_model_identity(self):
        stock.require(stock.sha(self.path.read_bytes()) == self.model_sha,
                      "recurrent model digest mismatch")

    def run_cells(self, initial, tokens, *, budget=1000, cancelled=False):
        self.assert_model_identity()
        initial = np.asarray(initial)
        tokens = np.asarray(tokens)
        stock.require(initial.dtype == np.dtype("float32") and
                      tokens.dtype == np.dtype("float32"),
                      "non-binary32 state or scan input representation")
        initial = stock.validate_cells(initial, self.state_cells)
        stock.require(tokens.ndim == 2 and tokens.shape[1] == self.token_cells
                      and 1 <= tokens.shape[0] <= 1000, "scan input shape/bound")
        for row in tokens:
            stock.validate_cells(row, self.token_cells)
        if self.program == "stack-swap":
            stock.require(np.all(tokens.view(np.uint32) == stock.BIT_ZERO),
                          "nonzero stack-swap scan token")
        stock.require(type(budget) is int and budget >= len(tokens) and not cancelled,
                      "scan budget exhausted or cancelled")
        before = rss_bytes("VmRSS")
        start = time.perf_counter_ns()
        result = self.session.run(["final_state_bits", "observations"],
                                  {"initial_state_bits": initial, "scan_inputs": tokens})
        elapsed = time.perf_counter_ns() - start
        peak = rss_bytes("VmHWM")
        stock.require(len(result) == 2 and result[1].shape == (len(tokens), self.state_cells),
                      "scan output shape mismatch")
        stock.require(result[0].dtype == np.dtype("float32") and
                      result[1].dtype == np.dtype("float32"), "scan output dtype mismatch")
        observations = [stock.decode_words(row, self.state_cells) for row in result[1]]
        final = stock.decode_words(result[0], self.state_cells)
        stock.require(final == observations[-1], "model final state/observation mismatch")
        return observations, {"elapsed_ns": elapsed, "rss_before_bytes": before,
                              "rss_peak_bytes": peak, "host_calls": 1,
                              "scan_steps": len(tokens), "emitted_tokens": len(tokens),
                              "skipped_work": 0, "mutable_state_bytes": self.state_cells * 4,
                              "sequence_input_bytes": tokens.nbytes,
                              "sequence_output_bytes": result[1].nbytes}

    def run(self, state_words, input_words, *, budget=1000, cancelled=False,
            declared_program=None, codec=stock.CODEC):
        start = time.perf_counter_ns()
        stock.require(declared_program is None or declared_program == self.program,
                      "recurrent program mismatch")
        stock.require(codec == stock.CODEC, "recurrent codec mismatch")
        initial = stock.encode_words(state_words, self.state_cells // 64)
        stock.require(type(input_words) is list and 1 <= len(input_words) <= 1000,
                      "scan input count")
        if self.program == "accumulator":
            tokens = np.stack([stock.encode_words(words, 1) for words in input_words])
        else:
            stock.require(all(words == [] for words in input_words), "stack-swap input mismatch")
            tokens = np.zeros((len(input_words), 1), dtype=np.float32)
        observations, cost = self.run_cells(initial, tokens, budget=budget, cancelled=cancelled)
        cost["end_to_end_ns"] = time.perf_counter_ns() - start
        return observations, cost


def vector(row):
    stock.require(type(row) is list and len(row) == 3, "malformed Noble vector")
    return [stock.vector_words(part) for part in row]


def compare(engine, spec):
    coverage = {}
    for group in ("tiny", "boundary", "teacher_forced"):
        rows = spec["vectors"][group]
        expected = 16 if group == "tiny" else 1000 if group == "teacher_forced" else (
            6 if engine.program == "accumulator" else 0)
        stock.require(len(rows) == expected, "missing recurrent reference vectors")
        total_ns = 0
        for index, raw in enumerate(rows):
            state, inputs, target = vector(raw)
            stock.require(stock.source(engine.program, state, inputs) == target,
                          "Noble/source vector mismatch")
            observed, cost = engine.run(state, [inputs], budget=1)
            stock.require(observed == [target], f"Scan/Noble mismatch: {group} {index}")
            total_ns += cost["elapsed_ns"]
        coverage[group] = {"transitions": expected, "mismatches": 0,
                           "host_calls": expected, "elapsed_ns": total_ns}

    rows = spec["vectors"]["free_running"]
    stock.require(len(rows) == 1000, "missing long-run Noble vectors")
    source_rows = [vector(row) for row in rows]
    for index, (state, inputs, target) in enumerate(source_rows):
        stock.require(stock.source(engine.program, state, inputs) == target,
                      "source/Noble long-run mismatch")
        if index:
            stock.require(state == source_rows[index - 1][2], "Noble free-run discontinuity")
    observed, cost = engine.run(source_rows[0][0], [row[1] for row in source_rows], budget=1000)
    for index, state in enumerate(observed):
        stock.require(state == source_rows[index][2],
                      f"Scan-carried state disagrees at free step {index}")
    stock.require(cost["host_calls"] == 1 and cost["scan_steps"] == 1000,
                  "free-running recurrence escaped into host loop")
    coverage["free_running"] = {"transitions": 1000, "mismatches": 0, **cost}
    return coverage, observed[-1]


def controls(engine, spec, model_bytes, output_dir, model_final_state):
    results = []
    state = [0] if engine.program == "accumulator" else [1, 2]
    inputs = [[1]] if engine.program == "accumulator" else [[]]
    valid = engine.run(state, inputs, budget=1)[0]
    for label, words in [("wrong-state-shape", []),
                         ("state-out-of-domain", [-1] + state[1:])]:
        stock.refused(label, lambda words=words: engine.run(words, inputs), results)
    stock.refused("wrong-scan-token-shape", lambda: engine.run_cells(
        stock.encode_words(state, engine.state_cells // 64),
        np.zeros((1, engine.token_cells + 1), dtype=np.float32)), results)
    stock.refused("coerced-state-dtype", lambda: engine.run_cells(
        np.ones(engine.state_cells, dtype=np.float64),
        np.zeros((1, engine.token_cells), dtype=np.float32)), results)
    stock.refused("coerced-token-dtype", lambda: engine.run_cells(
        stock.encode_words(state, engine.state_cells // 64),
        np.zeros((1, engine.token_cells), dtype=np.float64)), results)
    for label, bits in [("nan", 0x7FC00000), ("negative-zero", 0x80000000),
                        ("fractional", 0x3F000000)]:
        bad = stock.encode_words(state, engine.state_cells // 64)
        bad[0] = stock.bit_float(bits)
        stock.refused("corrupted-state-" + label,
                      lambda bad=bad: engine.run_cells(
                          bad, np.zeros((1, engine.token_cells), dtype=np.float32)), results)
    for label, bits in [("nan", 0x7FC00000), ("negative-zero", 0x80000000),
                        ("fractional", 0x3F000000)]:
        bad_token = np.zeros((1, engine.token_cells), dtype=np.float32)
        bad_token[0, 0] = stock.bit_float(bits)
        stock.refused("corrupted-input-token-" + label,
                      lambda bad_token=bad_token: engine.run_cells(
                          stock.encode_words(state, engine.state_cells // 64),
                          bad_token), results)
    if engine.program == "stack-swap":
        nonzero = np.ones((1, 1), dtype=np.float32)
        stock.refused("nonzero-stack-swap-token", lambda: engine.run_cells(
            stock.encode_words(state, 2), nonzero), results)
    stock.refused("zero-length-sequence", lambda: engine.run(state, []), results)
    stock.refused("wrong-program", lambda: engine.run(state, inputs, declared_program="wrong"), results)
    stock.refused("wrong-codec", lambda: engine.run(state, inputs, codec="big-endian"), results)
    stock.refused("budget-short", lambda: engine.run(state, inputs, budget=0), results)
    stock.refused("trajectory-budget-short",
                  lambda: engine.run(state, inputs + inputs, budget=1), results)
    stock.refused("cancelled-before-call", lambda: engine.run(state, inputs, cancelled=True), results)
    stock.refused("wrong-output-length", lambda: stock.decode_words(
        np.zeros(engine.state_cells - 1, dtype=np.float32), engine.state_cells), results)
    for label, bits in [("nan", 0x7FC00000), ("negative-zero", 0x80000000),
                        ("fractional", 0x3F000000)]:
        bad_observation = np.zeros(engine.state_cells, dtype=np.float32)
        bad_observation[0] = stock.bit_float(bits)
        stock.refused("malformed-observation-" + label, lambda bad=bad_observation:
                      stock.decode_words(bad, engine.state_cells), results)
    changed = output_dir / (engine.program + "-scan-corrupt.onnx")
    changed.write_bytes(model_bytes[:-1] + bytes([model_bytes[-1] ^ 1]))
    original = engine.path
    engine.path = changed
    try:
        stock.refused("corrupted-model", engine.assert_model_identity, results)
    finally:
        engine.path = original

    base = {"index": 0, "phase": "completed_transition", "program": engine.program,
            "codec": stock.CODEC, "model_sha256": engine.model_sha,
            "state": state, "remaining_budget": 1000}
    stock.require(stock.restore(base, engine) == state, "initial recurrent restore")
    raw = spec["vectors"]["free_running"]
    stock.require(stock.restore({**base, "index": 1000, "remaining_budget": 0,
                                 "state": model_final_state}, engine) == vector(raw[-1])[2],
                  "request-end recurrent restore")
    prefix, _ = engine.run(state, [vector(row)[1] for row in raw[:2]], budget=1000)
    second_state = prefix[-1]
    stock.require(second_state == vector(raw[2])[0], "model prefix state disagrees at index 2")
    restored = stock.restore({**base, "index": 2, "remaining_budget": 998,
                              "state": second_state}, engine)
    remaining = [vector(row)[1] for row in raw[2:]]
    resumed, resume_cost = engine.run(restored, remaining, budget=998)
    stock.require(len(resumed) == 998 and resume_cost["host_calls"] == 1,
                  "boundary resume shape mismatch")
    for offset, resumed_state in enumerate(resumed):
        stock.require(resumed_state == vector(raw[offset + 2])[2],
                      f"boundary resume mismatch at index {offset + 2}")
    for label, checkpoint in [
        ("missing-recurrent-state", {k: v for k, v in base.items() if k != "state"}),
        ("kv-only", {**{k: v for k, v in base.items() if k != "state"}, "kv": [1]}),
        ("scratch-only", {**{k: v for k, v in base.items() if k != "state"}, "scratch": [1]}),
        ("hybrid-extra", {**base, "kv": [1]}),
        ("negative-index", {**base, "index": -1}),
        ("past-request-end", {**base, "index": 1001}),
        ("mid-body-index", {**base, "phase": "mid_body"}),
        ("wrong-budget", {**base, "remaining_budget": 1}),
        ("wrong-model-digest", {**base, "model_sha256": "0" * 64}),
        ("wrong-checkpoint-program", {**base, "program": "wrong"}),
        ("wrong-checkpoint-state-shape", {**base, "state": []}),
    ]:
        stock.refused(label, lambda checkpoint=checkpoint: stock.restore(checkpoint, engine), results)
    stock.refused("request-end-budget", lambda: engine.run(
        state, inputs, budget=0), results)
    stock.require(valid == [stock.source(engine.program, state, inputs[0])],
                  "initial state observation mismatch")
    return results, resume_cost


def compare_host_loop(spec, output_dir):
    data, _ = stock.export(spec)
    path = output_dir / (spec["program"] + "-one-step.onnx")
    path.write_bytes(data)
    engine = stock.StockEngine(spec, path, stock.sha(data))
    rows = [vector(row) for row in spec["vectors"]["free_running"]]
    current = rows[0][0]
    start = time.perf_counter_ns()
    for index, (_, inputs, expected) in enumerate(rows):
        current = engine.step(current, inputs)
        stock.require(current == expected, f"same-run host-loop mismatch at {index}")
    return {"model_sha256": stock.sha(data), "model_bytes": len(data),
            "runtime_load_ns": engine.load_ns,
            "end_to_end_ns": time.perf_counter_ns() - start,
            "host_calls": len(rows), "transitions": len(rows)}


def seeded_fault(engine, spec, model_bytes, output_dir):
    model = onnx.load_model_from_string(model_bytes)
    body = next(attribute.g for attribute in model.graph.node[0].attribute
                if attribute.name == "body")
    if engine.program == "accumulator":
        changed = False
        for item in body.initializer:
            if item.name == "v257":
                item.CopyFrom(stock.tensor("v257", [0.0], np.float32))
                changed = True
                break
        stock.require(changed, "accumulator one-bit weight not located")
    else:
        changed = False
        for node in body.node:
            if node.op_type == "Concat" and node.output == ["output_bits"]:
                node.input[0] = "v0"
                changed = True
                break
        stock.require(changed, "stack-swap observation not located")
    onnx.checker.check_model(model)
    faulty_bytes = model.SerializeToString(deterministic=True)
    path = output_dir / (engine.program + "-seeded-fault.onnx")
    path.write_bytes(faulty_bytes)
    faulty = ScanEngine(spec, path, stock.sha(faulty_bytes))
    first = None
    for raw in spec["vectors"]["tiny"]:
        state, inputs, expected = vector(raw)
        observed, _ = faulty.run(state, [inputs], budget=1)
        if observed[0] != expected:
            first = {"state": [str(word) for word in state],
                     "input": [str(word) for word in inputs],
                     "expected": [str(word) for word in expected],
                     "actual": [str(word) for word in observed[0]]}
            break
    stock.require(first is not None, "seeded incorrect Scan transition passed domain")
    return {"model_sha256": stock.sha(faulty_bytes), "first_counterexample": first}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--export", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    stock.require(ort.__version__ == "1.23.2" and onnx.__version__ == "1.20.1",
                  "unselected model-state runtime")
    specs = json.loads(Path(args.export).read_text())
    stock.require([spec["program"] for spec in specs] == ["accumulator", "stack-swap"],
                  "missing checked graph")
    output_dir = Path(args.output_dir)
    output_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    results = []
    for spec in specs:
        data, guards, export_ns = build_scan(spec)
        stock.require(build_scan(spec)[0] == data, "nondeterministic recurrent export")
        path = output_dir / (spec["program"] + "-scan.onnx")
        path.write_bytes(data)
        engine = ScanEngine(spec, path, stock.sha(data))
        coverage, model_final_state = compare(engine, spec)
        host_loop = compare_host_loop(spec, output_dir)
        refusals, resume_cost = controls(engine, spec, data, output_dir, model_final_state)
        seeded = seeded_fault(engine, spec, data, output_dir)
        results.append({"program": engine.program, "model_sha256": stock.sha(data),
                        "model_bytes": len(data), "model_path": str(path),
                        "graph_sha256": stock.sha(bytes.fromhex(spec["canonical_graph"])),
                        "graph_id": spec["graph_id"], "snapshot_id": spec["snapshot_id"],
                        "snapshot_sha256": stock.sha(bytes.fromhex(spec["snapshot_bytes"])),
                        "parameter_count": len(spec["weights"]),
                        "weight_bytes": len(bytes.fromhex(spec["snapshot_bytes"])),
                        "export_ns": export_ns, "load_ns": engine.load_ns,
                        "guards_elided_under_exact_bit_closure": guards,
                        "state_bytes": engine.state_cells * 4,
                        "coverage": coverage, "controls": refusals,
                        "stateless_host_loop": host_loop,
                        "resume": {"index": 2, **resume_cost},
                        "seeded_fault": seeded})
    print(json.dumps({"schema": "noble.recurrent-model-state/v1",
                      "runtime": {"onnxruntime": ort.__version__, "onnx": onnx.__version__,
                                  "provider": "CPUExecutionProvider", "opset": 18, "ir": 10,
                                  "optimizations": "disabled", "thread_count": 1},
                      "coverage": {"dense": "none", "sparse": "none",
                                   "stock": "selected ORT CPU: two single-call ONNX Scan "
                                            "1,000-step free runs, separate one-step host loops",
                                   "stateless_host_loop": "separate retained stock-runtime receipt",
                                   "model_carried": "two ONNX Scan models; one call per 1,000-step free run"},
                      "results": results}, sort_keys=True))


if __name__ == "__main__":
    main()
