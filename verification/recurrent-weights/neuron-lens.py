#!/usr/bin/env python3
"""Source-bound, non-proof Jacobian audit of the two selected checked F32 graphs.

Consumes the Rust checked-graph export used by the selected stock ORT CPU lane.
No scores, thresholds, or gradient outcomes participate in the intended unit map.
"""

import argparse
import hashlib
import importlib.util
import json
import sys
from pathlib import Path

import numpy as np
import onnx
import onnxruntime as ort


stock_spec = importlib.util.spec_from_file_location(
    "stock_runtime", Path(__file__).with_name("stock-runtime.py"))
stock = importlib.util.module_from_spec(stock_spec)
stock_spec.loader.exec_module(stock)

# Identities of the checked export and its two graph/snapshot pairs, selected by
# the stock-runtime/model-state receipts. This prevents a producer-impossible
# node list carrying a copied graph_id from becoming a different "unit map".
SELECTED_EXPORT_SHA256 = "bf84fa4454e7313a9500a4ece04d2fbc8b6f1ddcb3dfb4698a3ae19014f4b4ae"
SELECTED = {
    "accumulator": {
        "graph_id": "f9bb01aff2957947f252aa2f5e6579be349cf8582965412e254d05a74823763c",
        "graph_sha256": "8c31b0f220973d163efaeea0477800173648080f69aa668b5db5d29be91083c7",
        "snapshot_id": "ebb216b86beec8269c357e1d03360ee6d952ff7a8c95a70a7e54361c3acf26e5",
        "snapshot_sha256": "22b6f43bd8d27738d3213f29e96b62d01d9d6c0ab4f9732aaae803186f51eab7",
    },
    "stack-swap": {
        "graph_id": "2f012a518cf5a931c968906cc04ae6a573d54afe7edd94d91cd4349b6dc1f038",
        "graph_sha256": "ce5ae5a155bc4c4bea77f58630a15b8af1959856594990491705049e12781c8d",
        "snapshot_id": "978a8138e2c1c98c4f66e727e3223bde87efe1aa63ff46a0c2a4983284b34907",
        "snapshot_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    },
}

PAPER = "https://github.com/stdlogits/neuron-lens/blob/44867947fa69316eae035e4c7dbb3bb3431affb1/docs/design.md"
FROZEN_BAR = {"minimum_internal_eligible_units": 64,
              "minimum_nonzero_true_gradient_units_per_required_direction": 1,
              "minimum_top_read_fraction": 0.5, "minimum_top_driver_fraction": 0.5}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read_label(bit):
    return f"output[{bit // 64}].bit[{bit % 64}]"


def driver_label(bit, program):
    if program == "stack-swap":
        return f"state[{bit // 64}].bit[{bit % 64}]"
    return f"{('state[0]' if bit % 2 == 0 else 'input[0]')}.bit[{bit // 2}]"


def intended_map(spec):
    """Freeze roles from the selected checked source/codec, before any execution.

    A source input has a known primary same-position output bit (carry can also
    affect higher bits). A computed adder output has a known output bit but NO
    unique input driver: both operands and a carry can drive it. The swap has
    no computational internal nodes; its output ports alias source input nodes.
    Guards and intermediate selects are deliberately not attributed to a bit.
    """
    program = spec["program"]
    nodes = spec["nodes"]
    output_slots = {node: bit for bit, node in enumerate(spec["outputs"])}
    stock.require(len(output_slots) == len(spec["outputs"]), "duplicate output source node")
    units = []
    seen_inputs = 0
    for index, node in enumerate(nodes):
        kind = node[0]
        item = {"node": index, "operator": kind, "role": "composite/unsupported",
                "expected_read": None, "expected_driver": None, "internal": False}
        if kind == "input":
            stock.require(node[1] == seen_inputs, "noncanonical source input order")
            item["role"] = "direct state bit" if program == "stack-swap" or seen_inputs % 2 == 0 else "direct input bit"
            item["expected_driver"] = driver_label(seen_inputs, program)
            output_bit = ((seen_inputs + 64) % 128 if program == "stack-swap"
                          else seen_inputs // 2)
            item["expected_read"] = read_label(output_bit)
            item["read_note"] = ("primary same-position result bit; carry may drive higher bits"
                                 if program == "accumulator" else "exact source permutation")
            seen_inputs += 1
        elif kind == "select" and index in output_slots:
            stock.require(program == "accumulator", "unexpected computed swap output")
            item["role"] = "semantic output bit"
            item["expected_read"] = read_label(output_slots[index])
            item["internal"] = True
            item["driver_unsupported_reason"] = "adder output has multiple possible source drivers"
        elif kind == "select":
            item["unsupported_reason"] = "composite intermediate select, no single source bit"
        elif kind == "guard_bit":
            item["unsupported_reason"] = "checked admission guard, no differentiable value"
        elif kind in ("param", "const"):
            item["unsupported_reason"] = "constant or immutable weight, not a source variable"
        elif kind not in ("param", "const", "select", "guard_bit"):
            raise ValueError(f"unsupported checked graph node: {kind}")
        if index in output_slots:
            item["output_port"] = read_label(output_slots[index])
        units.append(item)
    stock.require(seen_inputs == stock.EXPECTED[program][0], "source input count differs")
    stock.require(all(unit["role"] != "composite/unsupported" for unit in
                      (units[node] for node in spec["outputs"])), "unmapped semantic output")
    return units


def encoded_input(program, state, inputs):
    if program == "accumulator":
        cells = np.empty(128, dtype=np.float32)
        cells[0::2] = stock.encode_words(state, 1)
        cells[1::2] = stock.encode_words(inputs, 1)
        return cells
    return stock.encode_words(state, 2)


def local_values(spec, cells):
    """Evaluate exactly the exporter-supported bit-closed ops, not a source oracle."""
    values = [None] * len(spec["nodes"])
    input_index = param_index = 0
    for index, node in enumerate(spec["nodes"]):
        kind = node[0]
        if kind == "input":
            values[index] = float(cells[input_index])
            input_index += 1
        elif kind == "param":
            values[index] = stock.bit_float(spec["weights"][param_index])
            param_index += 1
        elif kind == "const":
            values[index] = stock.bit_float(node[1])
        elif kind == "select":
            predicate, off, on = node[1:]
            values[index] = values[on] if values[predicate] == 1.0 else values[off]
        elif kind == "guard_bit":
            stock.require(values[node[1]] in (0.0, 1.0), "bit guard refused")
        else:
            raise ValueError(f"unselected operator {kind}")
    stock.require(input_index == 128 and param_index == len(spec["weights"]),
                  "incomplete graph input/param read")
    return values


def derivatives(spec, values, relp_style):
    """Full node x input K/S and node x output J/R, in opposite traversal directions.

    Equal(x, 1) has zero derivative almost everywhere, but its real derivative
    is UNDEFINED at x = 1. The standard branchwise AD convention takes zero
    predicate cotangent there and differentiates ONLY the selected Where
    branch. Report that singularity; never call it a mathematical derivative
    through equality at x = 1. This is not a finite difference.

    The declared RelP-style DIVERGENCE from the paper: on a hard exact-bit
    Equal/Where select use an identity straight-through rule d predicate = d x,
    and differentiate the interpolation (1-p)*off + p*on. Thus the additional
    contribution is (on-off)*d predicate. The paper's LN hold-scale, SiLU
    identity and multiplicative half rules have no matching operators here;
    it does NOT prescribe this straight-through rule for Equal/Where.
    """
    nodes = spec["nodes"]
    input_count = stock.EXPECTED[spec["program"]][0]
    output_count = len(spec["outputs"])
    backward_to_input = np.zeros((len(nodes), input_count), dtype=np.float64)
    forward_to_output = np.zeros((len(nodes), output_count), dtype=np.float64)
    input_index = 0
    for index, node in enumerate(nodes):
        if node[0] == "input":
            backward_to_input[index, input_index] = 1.0
            input_index += 1
        elif node[0] == "select":
            predicate, off, on = node[1:]
            chosen = on if values[predicate] == 1.0 else off
            backward_to_input[index] = backward_to_input[chosen]
            if relp_style:
                backward_to_input[index] += (values[on] - values[off]) * backward_to_input[predicate]
    for bit, node in enumerate(spec["outputs"]):
        forward_to_output[node, bit] += 1.0
    for index in range(len(nodes) - 1, -1, -1):
        node = nodes[index]
        if node[0] == "select":
            predicate, off, on = node[1:]
            chosen = on if values[predicate] == 1.0 else off
            forward_to_output[chosen] += forward_to_output[index]
            if relp_style:
                forward_to_output[predicate] += (values[on] - values[off]) * forward_to_output[index]
    return forward_to_output, backward_to_input


def top(scores, labels):
    """Signed Jacobian directions are ranked by absolute response, no tie-breaking."""
    magnitudes = np.abs(scores)
    maximum = float(np.max(magnitudes))
    if maximum == 0.0:
        return {"kind": "zero-gradient", "top": None, "tied": 0}
    winners = np.flatnonzero(magnitudes == maximum)
    if len(winners) != 1:
        return {"kind": "tie", "top": None, "tied": len(winners),
                "labels": [labels[int(index)] for index in winners]}
    position = int(winners[0])
    return {"kind": "unique", "top": labels[position],
            "sensitivity": float(scores[position]), "tied": 1}


def score_direction(units, matrix, field, labels):
    stats = {"eligible": 0, "unsupported": 0, "zero_gradient": 0,
             "tie": 0, "match": 0, "first_mismatch": None, "nonzero_internal": 0,
             "zero_gradient_all_nodes": 0, "zero_gradient_internal_nodes": 0,
             "tie_all_nodes": 0}
    for unit in units:
        result = top(matrix[unit["node"]], labels)
        if result["kind"] == "zero-gradient":
            stats["zero_gradient_all_nodes"] += 1
            if unit["internal"]:
                stats["zero_gradient_internal_nodes"] += 1
        elif result["kind"] == "tie":
            stats["tie_all_nodes"] += 1
        expected = unit[field]
        if expected is None:
            stats["unsupported"] += 1
            continue
        stats["eligible"] += 1
        if result["kind"] == "zero-gradient":
            stats["zero_gradient"] += 1
        elif unit["internal"]:
            stats["nonzero_internal"] += 1
        if result["kind"] == "tie":
            stats["tie"] += 1
        if result["kind"] == "unique" and result["top"] == expected:
            stats["match"] += 1
        elif stats["first_mismatch"] is None:
            stats["first_mismatch"] = {"node": unit["node"], "role": unit["role"],
                                       "expected": expected, "actual": result}
    stats["matching_fraction"] = stats["match"] / stats["eligible"] if stats["eligible"] else None
    return stats


def score_methods(spec, units, matrices):
    output_labels = [read_label(bit) for bit in range(len(spec["outputs"]))]
    input_labels = [driver_label(bit, spec["program"]) for bit in range(128)]
    # Each derivative policy has a single full Jacobian; J/K or R/S differ in
    # which transport direction is primary, not in the underlying derivative.
    methods = {}
    for method, policy, primary in (("J", "true", "forward-to-decoded-output"),
                                    ("R", "relp-style", "forward-to-decoded-output"),
                                    ("K", "true", "backward-to-encoded-input"),
                                    ("S", "relp-style", "backward-to-encoded-input")):
        forward, backward = matrices[policy]
        methods[method] = {"derivative": policy, "primary_direction": primary,
                           "top_read": score_direction(units, forward, "expected_read", output_labels),
                           "top_driver": score_direction(units, backward, "expected_driver", input_labels)}
    return methods


def counts(units):
    roles = ("direct state bit", "direct input bit", "semantic output bit", "composite/unsupported")
    by_role = {role: sum(unit["role"] == role for unit in units) for role in roles}
    return {"total_graph_nodes": len(units), "by_role": by_role,
            "eligible_any_direction": len(units) - by_role["composite/unsupported"],
            "unsupported_both_directions": by_role["composite/unsupported"],
            "internal_eligible_read": sum(unit["internal"] and unit["expected_read"] is not None for unit in units),
            "internal_eligible_driver": sum(unit["internal"] and unit["expected_driver"] is not None for unit in units),
            "internal_eligible_both": sum(unit["internal"] and unit["expected_read"] is not None
                                          and unit["expected_driver"] is not None for unit in units)}


def judgement(unit_counts, methods):
    reasons = []
    if unit_counts["internal_eligible_both"] < FROZEN_BAR["minimum_internal_eligible_units"]:
        reasons.append("fewer than 64 nontrivial internal units with both intended read and driver")
    for method in ("J", "R", "K", "S"):
        read = methods[method]["top_read"]
        driver = methods[method]["top_driver"]
        primary = read if method in ("J", "R") else driver
        if (method in ("J", "K") and primary["nonzero_internal"] <
                FROZEN_BAR["minimum_nonzero_true_gradient_units_per_required_direction"]):
            reasons.append(f"{method}: no nonzero true-gradient internal unit in its required direction")
        for label, part in (("read", read), ("driver", driver)):
            threshold = FROZEN_BAR["minimum_top_read_fraction" if label == "read"
                                   else "minimum_top_driver_fraction"]
            if part["matching_fraction"] is None or part["matching_fraction"] < threshold:
                reasons.append(f"{method}: top-{label} agreement below 50% (zeros/ties remain in denominator)")
    return {"verdict": "NO-GO" if reasons else "GO", "reasons": reasons,
            "frozen_bar": FROZEN_BAR}


def audit(spec):
    program = spec["program"]
    identity = SELECTED[program]
    for key in ("graph_id", "snapshot_id"):
        stock.require(spec[key] == identity[key], f"{program}: unselected {key}")
    stock.require(sha(bytes.fromhex(spec["canonical_graph"])) == identity["graph_sha256"]
                  and sha(bytes.fromhex(spec["snapshot_bytes"])) == identity["snapshot_sha256"],
                  f"{program}: unselected canonical graph or snapshot")
    model_bytes, guards = stock.export(spec)
    unit_map = intended_map(spec)
    # The source-bound map is fully materialized and hashed BEFORE any case is
    # executed or scored. No derivative result can change a map entry.
    map_bytes = json.dumps(unit_map, sort_keys=True, separators=(",", ":")).encode("utf-8")
    map_sha256 = sha(map_bytes)
    rows = spec["vectors"]["tiny"]
    stock.require(len(rows) == 16, "tiny corpus must have exactly 16 cases")
    options = ort.SessionOptions()
    options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
    options.intra_op_num_threads = 1
    options.inter_op_num_threads = 1
    session = ort.InferenceSession(model_bytes, sess_options=options,
                                   providers=["CPUExecutionProvider"])
    stock.require(session.get_providers() == ["CPUExecutionProvider"]
                  and [item.name for item in session.get_inputs()] == ["input_bits"]
                  and [item.name for item in session.get_outputs()] == ["output_bits"],
                  "unselected ONNX provider or ABI")
    matrices = {policy: [np.zeros((len(unit_map), len(spec["outputs"])), dtype=np.float64),
                         np.zeros((len(unit_map), 128), dtype=np.float64)]
                for policy in ("true", "relp-style")}
    input_nodes = [unit["node"] for unit in unit_map if unit["operator"] == "input"]
    case_rows = []
    bit_significance = np.exp2(np.arange(64, dtype=np.float64))
    for index, row in enumerate(rows):
        stock.require(type(row) is list and len(row) == 3, "malformed tiny row")
        state, inputs, target = (stock.vector_words(part) for part in row)
        first, second = divmod(index, 4)
        stock.require((state == [first] and inputs == [second] if program == "accumulator"
                       else state == [first, second] and inputs == []),
                      "tiny corpus is not the selected 4x4 grid")
        stock.require(stock.source(program, state, inputs) == target,
                      "source disagrees with checked tiny row")
        cells = encoded_input(program, state, inputs)
        values = local_values(spec, cells)
        local = np.asarray([values[node] for node in spec["outputs"]], dtype=np.float32)
        runtime = session.run(["output_bits"], {"input_bits": cells})[0]
        stock.require(np.array_equal(local.view(np.uint32), runtime.view(np.uint32))
                      and stock.decode_words(runtime, len(spec["outputs"])) == target,
                      f"case {index}: local/selected ORT/source discrepancy")
        case = {"index": index, "state": state, "input": inputs,
                "source_output": target, "local_output": stock.decode_words(local, len(spec["outputs"])),
                "selected_ort_output": stock.decode_words(runtime, len(spec["outputs"])),
                "exact_equal_at_one_predicates": sum(
                    node[0] == "select" and values[node[1]] == 1.0 for node in spec["nodes"]),
                "policies": {}}
        for policy in ("true", "relp-style"):
            forward, backward = derivatives(spec, values, policy == "relp-style")
            stock.require(np.array_equal(forward[input_nodes],
                                         backward[spec["outputs"]].T),
                          f"case {index}: {policy} Jacobian dual traversal disagrees")
            decoded_word_jacobian = forward.reshape(len(unit_map), -1, 64) @ bit_significance
            case["policies"][policy] = {
                "nonzero_output_to_input_entries":
                    int(np.count_nonzero(backward[spec["outputs"]])),
                "nonzero_node_to_decoded_word_entries":
                    int(np.count_nonzero(decoded_word_jacobian))}
            matrices[policy][0] += forward / 16
            matrices[policy][1] += backward / 16
        case_rows.append(case)
    unit_counts = counts(unit_map)
    methods = score_methods(spec, unit_map, matrices)
    return {"program": program, "graph_id": spec["graph_id"],
            "graph_sha256": identity["graph_sha256"], "snapshot_id": spec["snapshot_id"],
            "snapshot_sha256": identity["snapshot_sha256"], "onnx_model_sha256": sha(model_bytes),
            "guards_elided_under_exact_bit_contract": guards,
            "corpus": {"name": "checked Rust tiny 4x4 state/input grid", "cases": len(rows),
                       "local_and_selected_ort_source_matches": len(rows),
                       "supplemental_cases": 0, "rows": case_rows},
            "intended_unit_map_sha256": map_sha256, "intended_unit_map": unit_map,
            "unit_counts": unit_counts, "methods": methods,
            "decision": judgement(unit_counts, methods)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--export", required=True,
                        help="selected checked-graphs.json from the Rust stock_runtime_export test")
    args = parser.parse_args()
    stock.require(sys.version_info[:3] == (3, 13, 12)
                  and ort.__version__ == "1.23.2" and onnx.__version__ == "1.20.1",
                  "unselected ONNX Python/ORT version")
    export = Path(args.export).read_bytes()
    stock.require(sha(export) == SELECTED_EXPORT_SHA256,
                  "unselected or modified checked graph export bytes")
    specs = json.loads(export)
    stock.require(type(specs) is list and [spec["program"] for spec in specs] ==
                  ["accumulator", "stack-swap"], "selected checked graph pair missing")
    results = [audit(spec) for spec in specs]
    print(json.dumps({"schema": "noble.recurrent-neuron-lens/v1",
                      "source_export_sha256": sha(export), "criterion":
                      "verification/recurrent-weights/batch9-criteria.txt",
                      "paper_design": PAPER, "selected_runtime": {
                          "onnxruntime": ort.__version__, "onnx": onnx.__version__,
                          "provider": "CPUExecutionProvider", "optimizations": "disabled"},
                      "derivatives": {
                          "true": "branchwise AD convention: Equal predicate zero cotangent, Where selected-branch derivative only; mathematical derivative through equality is undefined at x=1 (reported per case)",
                          "relp-style": "DECLARED DIVERGENCE: Equal(x,1) straight-through dx; Where (1-p)off+p on, with (on-off)dp; not the paper's exact LN/SiLU/half rules",
                          "decoder": "fixed linear extension into per-bit u64 contributions; bit j contributes 2^j times its exact-bit cell. Rank by absolute per-bit sensitivity normalized by that fixed 2^j, not the nondifferentiable validated integer decoder. There is no paper-style vocabulary, unembedding, normalization or softmax in this graph",
                          "averaging": "arithmetic mean of 16 signed Jacobians, before absolute-value ranking; exact ties and zero gradients do not match"},
                      "results": results,
                      "decision": "GO" if all(row["decision"]["verdict"] == "GO" for row in results)
                      else "NO-GO",
                      "limitations": "One-step checked graphs, not a Jacobian through Scan across 1,000 steps. Non-gating interpretability audit, not a simulation theorem, emitter correctness, decoder correctness or inference-runtime refinement."},
                     sort_keys=True))


if __name__ == "__main__":
    main()
