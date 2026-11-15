#!/usr/bin/env python3
"""Check independently authored routing weights against a named source property."""

import argparse
import hashlib
import json
from dataclasses import asdict, dataclass

import numpy as np
import onnx
import onnxruntime as ort
from onnx import TensorProto, helper, numpy_helper

CODEC = "binary32-exact/u64-bit-le/v1"
PROPERTY = "one-step stack-swap (a,b)->(b,a) under " + CODEC + " for a,b in 0..3"


@dataclass(frozen=True)
class Verdict:
    kind: str
    property: str
    passed: int
    total: int
    first_counterexample: dict | None
    reason: str | None
    model_sha256: str
    initializer_elements: int
    nonzero_initializers: int


def authored_model(indices):
    """This graph and its routing table are authored here, never by Noble's emitter."""
    body = helper.make_graph([
        helper.make_node("Gather", ["body_state_bits", "routing_indices"],
                         ["body_next_state_bits"], axis=0),
        helper.make_node("Identity", ["body_next_state_bits"], ["body_observation_bits"]),
    ], "independent-stack-swap-body", [
        helper.make_tensor_value_info("body_state_bits", TensorProto.FLOAT, [128]),
        helper.make_tensor_value_info("body_token_bits", TensorProto.FLOAT, [1]),
    ], [
        helper.make_tensor_value_info("body_next_state_bits", TensorProto.FLOAT, [128]),
        helper.make_tensor_value_info("body_observation_bits", TensorProto.FLOAT, [128]),
    ], initializer=[numpy_helper.from_array(np.asarray(indices, dtype=np.int64),
                                             name="routing_indices")])
    graph = helper.make_graph([
        helper.make_node("Scan", ["initial_state_bits", "scan_inputs"],
                         ["final_state_bits", "observations"], body=body, num_scan_inputs=1),
    ], "independently-authored-stack-swap", [
        helper.make_tensor_value_info("initial_state_bits", TensorProto.FLOAT, [128]),
        helper.make_tensor_value_info("scan_inputs", TensorProto.FLOAT, ["steps", 1]),
    ], [
        helper.make_tensor_value_info("final_state_bits", TensorProto.FLOAT, [128]),
        helper.make_tensor_value_info("observations", TensorProto.FLOAT, ["steps", 128]),
    ])
    model = helper.make_model(graph, producer_name="independent-hand-authored-routing/v1",
                              ir_version=10, opset_imports=[helper.make_operatorsetid("", 18)])
    onnx.checker.check_model(model)
    return model.SerializeToString(deterministic=True)


def _bits(values):
    cells = np.asarray(values, dtype=np.float32)
    if cells.shape != (128,) or not np.all(np.isin(cells.view(np.uint32),
                                                     [0x00000000, 0x3F800000])):
        raise ValueError("candidate emitted a non-exact state bit or wrong output shape")
    return [sum(int(cells[index + bit]) << bit for bit in range(64))
            for index in (0, 64)]


def _initializers(model):
    scan = model.graph.node[0]
    if scan.op_type != "Scan":
        raise ValueError("candidate is not recurrent Scan")
    body = next(attribute.g for attribute in scan.attribute if attribute.name == "body")
    values = [numpy_helper.to_array(item) for item in (*model.graph.initializer,
                                                         *body.initializer)]
    return sum(array.size for array in values), sum(np.count_nonzero(array) for array in values)


def check_candidate(source_transition, state_codec, candidate_bytes):
    """Only source name, codec and untrusted candidate bytes; no emitter dependency."""
    digest = hashlib.sha256(candidate_bytes).hexdigest()
    count = nonzero = passed = 0
    first = reason = None
    try:
        if source_transition != "stack-swap" or state_codec != CODEC:
            raise ValueError("unsupported source transition or state codec")
        if len(candidate_bytes) > 1024 * 1024:
            raise ValueError("candidate model exceeds checker budget")
        model = onnx.load_model_from_string(candidate_bytes)
        onnx.checker.check_model(model)
        count, nonzero = _initializers(model)
        options = ort.SessionOptions()
        options.intra_op_num_threads = 1
        options.inter_op_num_threads = 1
        options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
        session = ort.InferenceSession(candidate_bytes, sess_options=options,
                                       providers=["CPUExecutionProvider"])
        if session.get_providers() != ["CPUExecutionProvider"] or [
                item.name for item in session.get_inputs()] != ["initial_state_bits", "scan_inputs"] or [
                item.shape for item in session.get_inputs()] != [[128], ["steps", 1]] or [
                item.name for item in session.get_outputs()] != ["final_state_bits", "observations"] or [
                item.shape for item in session.get_outputs()] != [[128], ["steps", 128]]:
            raise ValueError("candidate state/observation ABI differs")
        for left in range(4):
            for right in range(4):
                state = np.fromiter(((word >> bit) & 1 for word in (left, right)
                                     for bit in range(64)), dtype=np.float32, count=128)
                final, observation = session.run(["final_state_bits", "observations"], {
                    "initial_state_bits": state,
                    "scan_inputs": np.zeros((1, 1), dtype=np.float32),
                })
                actual = _bits(final)
                if observation.shape != (1, 128) or _bits(observation[0]) != actual:
                    raise ValueError("final state and observation differ")
                if actual != [right, left]:
                    first = {"state": [str(left), str(right)], "input": [],
                             "expected": [str(right), str(left)],
                             "actual": [str(word) for word in actual]}
                    reason = "one-step simulation counterexample"
                    break
                passed += 1
            if first is not None:
                break
    except Exception as error:
        reason = str(error) or type(error).__name__
    return Verdict("ACCEPT" if reason is None and passed == 16 else "REFUSE",
                   PROPERTY, passed, 16, first, reason, digest, int(count), int(nonzero))


def main():
    argparse.ArgumentParser().parse_args()
    if ort.__version__ != "1.23.2" or onnx.__version__ != "1.20.1":
        raise ValueError("unselected authored-checker runtime")
    good = authored_model(list(range(64, 128)) + list(range(64)))
    wrong_indices = list(range(64, 128)) + list(range(64))
    wrong_indices[0] = 0
    wrong = authored_model(wrong_indices)
    accepted = check_candidate("stack-swap", CODEC, good)
    refused = check_candidate("stack-swap", CODEC, wrong)
    controls = {
        "wrong_codec": check_candidate("stack-swap", "big-endian", good),
        "wrong_source": check_candidate("accumulator", CODEC, good),
        "malformed_model": check_candidate("stack-swap", CODEC, b"not an ONNX model"),
        "corrupt_bytes": check_candidate("stack-swap", CODEC, good[:-1] + bytes([good[-1] ^ 1])),
    }
    if accepted.kind != "ACCEPT" or accepted.passed != 16 or accepted.nonzero_initializers != 127:
        raise ValueError("independently authored correct weights were not accepted")
    if refused.kind != "REFUSE" or refused.first_counterexample is None or refused.passed >= 16:
        raise ValueError("independently authored bad weights were silently accepted")
    if any(verdict.kind != "REFUSE" for verdict in controls.values()):
        raise ValueError("candidate corruption or configuration was silently accepted")
    print(json.dumps({"schema": "noble.recurrent-authored-checker/v1",
                      "authorship": "hand-written routing-index initializer, independent of Noble emitter",
                      "accuracy": None, "llm_authored": "not run; no prompt/transcript/model identity",
                      "compiled_weight_checker_property": PROPERTY,
                      "accepted": asdict(accepted), "refused": asdict(refused),
                      "controls": {name: asdict(verdict) for name, verdict in controls.items()}},
                     sort_keys=True))


if __name__ == "__main__":
    main()
