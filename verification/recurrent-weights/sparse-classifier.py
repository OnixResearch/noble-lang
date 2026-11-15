#!/usr/bin/env python3
"""Independently authored, non-gating sparse classifier for a bounded citation shape.

No Noble compiler output, fitted parameters, or trained weights enter this model.
The classifier is deliberately not a full citation checker.
"""

import argparse
import hashlib
import itertools
import json
import re
from dataclasses import asdict, dataclass
from pathlib import Path

import numpy as np
import onnx
import onnxruntime as ort
from onnx import AttributeProto, TensorProto, helper, numpy_helper

SOURCE_PATTERN = r"^(1|2) (R|RR) (1|2)$"
PROPERTY = "bounded ASCII language " + SOURCE_PATTERN
ALPHABET = ("1", "2", "R", " ", "X")
MAX_LENGTH = 6
CODEC = "ascii-positional-one-hot/6x5-zero-padded-f32/v1"
TOTAL_CASES = sum(len(ALPHABET) ** length for length in range(MAX_LENGTH + 1))
SOURCE_RE = re.compile(SOURCE_PATTERN, flags=re.ASCII)
BIT_ONE = 0x3F800000

# Labels are frozen independently of the ONNX graph and the exhaustive recognizer.
BEHAVIORAL_CASES = (
    ("1 R 1", True), ("2 R 2", True), ("1 R 2", True),
    ("2 RR 1", True), ("1 RR 2", True), ("2 RR 2", True),
    ("", False), ("1", False), ("1R1", False),
    ("1  R1", False), ("1 R1", False), ("1 RR1", False),
    ("1 X 1", False), ("X R 1", False), ("1 R X", False),
    ("1 R 1 ", False),
)


@dataclass(frozen=True)
class Verdict:
    kind: str
    property: str
    passed: int
    tested: int
    total: int
    first_counterexample: dict | None
    reason: str | None
    model_sha256: str
    initializer_elements: int | None
    nonzero_initializer_elements: int | None
    classifier_weight_elements: int | None
    nonzero_classifier_weights: int | None


def source_recognizes(text):
    """Written source recognizer, separate from graph construction and inference."""
    return (isinstance(text, str) and text.isascii() and
            len(text) <= MAX_LENGTH and all(char in ALPHABET for char in text) and
            SOURCE_RE.fullmatch(text) is not None)


def encode_strings(texts):
    """Six positions of five ordered ASCII glyphs; unused trailing positions are all +0."""
    if isinstance(texts, str) or not texts:
        raise ValueError("codec requires a nonempty sequence of strings")
    encoded = np.zeros((len(texts), MAX_LENGTH * len(ALPHABET)), dtype=np.float32)
    for row, text in enumerate(texts):
        if not isinstance(text, str) or not text.isascii() or len(text) > MAX_LENGTH or any(
                char not in ALPHABET for char in text):
            raise ValueError("codec rejects non-ASCII, out-of-alphabet or overlong text")
        for position, char in enumerate(text):
            encoded[row, position * len(ALPHABET) + ALPHABET.index(char)] = 1.0
    return encoded


def validate_encoded(encoded):
    if not isinstance(encoded, np.ndarray) or encoded.dtype != np.dtype("float32") or \
            encoded.ndim != 2 or encoded.shape[0] == 0 or \
            encoded.shape[1] != MAX_LENGTH * len(ALPHABET):
        raise ValueError("codec requires nonempty batch x 30 binary32 cells")
    bits = encoded.view(np.uint32)
    if np.any((bits != 0) & (bits != BIT_ONE)):
        raise ValueError("codec rejects non-exact one-hot cells (including -0 and NaN)")
    occupancy = encoded.reshape((-1, MAX_LENGTH, len(ALPHABET))).sum(axis=2)
    if np.any((occupancy != 0) & (occupancy != 1)):
        raise ValueError("codec rejects multi-hot positions")
    if np.any(occupancy[:, 1:] > occupancy[:, :-1]):
        raise ValueError("codec rejects non-trailing padding")


def authored_model():
    """Two hand-set affine detectors, not a translation from the source regexp.

    The five-glyph unit scores five for a complete short citation and penalizes
    EVERY occupied sixth position. The six-glyph unit scores six only for the
    long citation. Because the codec is exact one-hot, all other scores are at
    most four and five respectively. Thresholding at 4.5/5.5 is exact in f32.
    """
    weights = np.zeros((MAX_LENGTH * len(ALPHABET), 2), dtype=np.float32)
    # Position 0: either volume digit, both units.
    for glyph in ("1", "2"):
        weights[ALPHABET.index(glyph), :] = 1.0
    weights[1 * 5 + ALPHABET.index(" "), :] = 1.0
    weights[2 * 5 + ALPHABET.index("R"), :] = 1.0
    # Short branch: one reporter, final space, final page, no sixth glyph.
    weights[3 * 5 + ALPHABET.index(" "), 0] = 1.0
    for glyph in ("1", "2"):
        weights[4 * 5 + ALPHABET.index(glyph), 0] = 1.0
    weights[5 * 5:6 * 5, 0] = -1.0
    # Long branch: two reporters, final space, final page.
    weights[3 * 5 + ALPHABET.index("R"), 1] = 1.0
    weights[4 * 5 + ALPHABET.index(" "), 1] = 1.0
    for glyph in ("1", "2"):
        weights[5 * 5 + ALPHABET.index(glyph), 1] = 1.0
    graph = helper.make_graph([
        helper.make_node("MatMul", ["glyphs", "feature_weights"], ["scores"]),
        helper.make_node("Add", ["scores", "bias"], ["margins"]),
        helper.make_node("Relu", ["margins"], ["positive_margins"]),
        helper.make_node("ReduceSum", ["positive_margins", "axes"], ["positive_sum"],
                         keepdims=1),
        helper.make_node("Greater", ["positive_sum", "zero"], ["accepted"]),
    ], "hand-authored-sparse-regular-language", [
        helper.make_tensor_value_info("glyphs", TensorProto.FLOAT, ["batch", 30]),
    ], [
        helper.make_tensor_value_info("accepted", TensorProto.BOOL, ["batch", 1]),
    ], initializer=[
        numpy_helper.from_array(weights, name="feature_weights"),
        numpy_helper.from_array(np.asarray([-4.5, -5.5], dtype=np.float32), name="bias"),
        numpy_helper.from_array(np.asarray([1], dtype=np.int64), name="axes"),
        numpy_helper.from_array(np.asarray([0.0], dtype=np.float32), name="zero"),
    ])
    model = helper.make_model(graph, producer_name="independent-hand-authored-sparse/v1",
                              ir_version=10, opset_imports=[helper.make_operatorsetid("", 18)])
    onnx.checker.check_model(model)
    return model.SerializeToString(deterministic=True)


def parameter_counts(model):
    """Count all dense initializers, including those in any nested graph."""
    arrays = []

    def visit(graph):
        if graph.sparse_initializer:
            raise ValueError("sparse external initializer representation is unsupported")
        for initializer in graph.initializer:
            if initializer.data_location != TensorProto.DEFAULT:
                raise ValueError("external initializer data is forbidden")
            arrays.append(numpy_helper.to_array(initializer))
        for node in graph.node:
            for attribute in node.attribute:
                if attribute.type in (AttributeProto.TENSOR, AttributeProto.TENSORS):
                    raise ValueError("hidden tensor-valued node constants are forbidden")
                if attribute.type == AttributeProto.GRAPH:
                    visit(attribute.g)
                elif attribute.type == AttributeProto.GRAPHS:
                    for nested in attribute.graphs:
                        visit(nested)

    visit(model.graph)
    weight_arrays = [numpy_helper.to_array(item) for item in model.graph.initializer
                     if item.name in ("feature_weights", "bias")]
    return (sum(int(array.size) for array in arrays),
            sum(int(np.count_nonzero(array)) for array in arrays),
            sum(int(array.size) for array in weight_arrays),
            sum(int(np.count_nonzero(array)) for array in weight_arrays))


def checked_output(session, encoded):
    validate_encoded(encoded)
    result = session.run(["accepted"], {"glyphs": encoded})[0]
    if result.dtype != np.dtype("bool") or result.shape != (encoded.shape[0], 1):
        raise ValueError("candidate emitted wrong output dtype or batch shape")
    return result[:, 0]


def corpus():
    return ["".join(glyphs) for length in range(MAX_LENGTH + 1)
            for glyphs in itertools.product(ALPHABET, repeat=length)]


def check_candidate(candidate_bytes, expected_sha256, *, property_name=PROPERTY,
                    codec=CODEC, alphabet=ALPHABET, max_length=MAX_LENGTH,
                    retain_rows=False):
    """Exhaustive source-vs-ORT checker; returns a typed verdict, not an exception."""
    digest = hashlib.sha256(candidate_bytes).hexdigest()
    counts = (None, None, None, None)
    passed = tested = 0
    counterexample = reason = None
    session = rows = None
    try:
        if property_name != PROPERTY or codec != CODEC or tuple(alphabet) != ALPHABET or \
                max_length != MAX_LENGTH:
            raise ValueError("unsupported source property, codec, alphabet or bound")
        if digest != expected_sha256:
            raise ValueError("candidate model identity differs from committed candidate")
        if len(candidate_bytes) > 1024 * 1024:
            raise ValueError("candidate model exceeds one-megabyte admission budget")
        model = onnx.load_model_from_string(candidate_bytes)
        onnx.checker.check_model(model)
        if model.ir_version != 10 or [(item.domain, item.version) for item in
                                       model.opset_import] != [("", 18)]:
            raise ValueError("unsupported ONNX IR or opset")
        counts = parameter_counts(model)
        options = ort.SessionOptions()
        options.intra_op_num_threads = 1
        options.inter_op_num_threads = 1
        options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_DISABLE_ALL
        session = ort.InferenceSession(candidate_bytes, sess_options=options,
                                       providers=["CPUExecutionProvider"])
        if session.get_providers() != ["CPUExecutionProvider"] or \
                [(item.name, item.shape, item.type) for item in session.get_inputs()] != [
                    ("glyphs", ["batch", 30], "tensor(float)")] or \
                [(item.name, item.shape, item.type) for item in session.get_outputs()] != [
                    ("accepted", ["batch", 1], "tensor(bool)")]:
            raise ValueError("candidate provider or input/output ABI differs")
        texts = corpus()
        if len(texts) != TOTAL_CASES:
            raise ValueError("exhaustive corpus cardinality differs")
        predicted = checked_output(session, encode_strings(texts))
        actual = predicted.tolist()
        rows = [] if retain_rows else None
        for index, (text, answer) in enumerate(zip(texts, actual, strict=True)):
            expected = source_recognizes(text)
            tested += 1
            passed += int(answer == expected)
            if rows is not None:
                rows.append({"input": text, "source": expected, "onnx": answer})
            if counterexample is None and answer != expected:
                counterexample = {"index": index, "input": text,
                                  "expected": expected, "actual": answer}
        if counterexample is not None:
            reason = "exhaustive source-vs-ONNX language counterexample"
    except Exception as error:
        reason = str(error) or type(error).__name__
    verdict = Verdict("ACCEPT" if reason is None and passed == TOTAL_CASES else "REFUSE",
                      PROPERTY, passed, tested, TOTAL_CASES, counterexample, reason,
                      digest, *counts)
    return verdict, session, rows


def mutated_model(original):
    """Valid ONNX bytes with a required positive feature deleted (semantic control)."""
    model = onnx.load_model_from_string(original)
    initializer = next(item for item in model.graph.initializer
                       if item.name == "feature_weights")
    weights = numpy_helper.to_array(initializer).copy()
    weights[2 * 5 + ALPHABET.index("R"), 0] = 0.0
    initializer.CopyFrom(numpy_helper.from_array(weights, name="feature_weights"))
    return model.SerializeToString(deterministic=True)


def input_control(session, *, strings=None, encoded=None):
    if session is None:
        return {"kind": "UNMEASURED", "reason": "baseline candidate did not load"}
    try:
        checked_output(session, encode_strings(strings) if strings is not None else encoded)
    except Exception as error:
        return {"kind": "REFUSE", "reason": str(error) or type(error).__name__}
    return {"kind": "ACCEPT", "reason": None}


def accuracy(session):
    if session is None:
        return {"kind": "UNMEASURED", "reason": "baseline candidate did not load"}
    texts = [text for text, _ in BEHAVIORAL_CASES]
    if any(label != source_recognizes(text) for text, label in BEHAVIORAL_CASES):
        raise ValueError("fixed behavioral labels disagree with independent source")
    actual = checked_output(session, encode_strings(texts)).tolist()
    rows = [{"input": text, "label": label, "onnx": prediction}
            for (text, label), prediction in zip(BEHAVIORAL_CASES, actual, strict=True)]
    return {"kind": "MEASURED", "correct": sum(row["label"] == row["onnx"] for row in rows),
            "total": len(rows), "rows": rows}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args()
    if ort.__version__ != "1.23.2" or onnx.__version__ != "1.20.1":
        raise ValueError("unselected ONNX/ORT Python runtime")
    output_dir = Path(args.output_dir)
    output_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    original = authored_model()
    model_file = output_dir / "sparse-classifier.onnx"
    with model_file.open("xb") as output:
        output.write(original)
    recorded = model_file.read_bytes()
    digest = hashlib.sha256(original).hexdigest()
    accepted, session, exhaustive_rows = check_candidate(recorded, digest, retain_rows=True)
    wrong_weights = mutated_model(original)
    invalid_model = b"not an ONNX model"
    controls = {
        "wrong_source_property": check_candidate(original, digest,
                                                   property_name="full-citation-checker")[0],
        "wrong_codec": check_candidate(original, digest, codec="UTF-8/v1")[0],
        "wrong_alphabet": check_candidate(original, digest,
                                          alphabet=("1", "2", "R", " ", "Y"))[0],
        "wrong_length_bound": check_candidate(original, digest, max_length=5)[0],
        "malformed_model": check_candidate(invalid_model,
                                           hashlib.sha256(invalid_model).hexdigest())[0],
        "corrupt_model_bytes": check_candidate(original[:-1], digest)[0],
        "changed_weight_with_new_digest": check_candidate(
            wrong_weights, hashlib.sha256(wrong_weights).hexdigest())[0],
    }
    good_input = encode_strings(["1 R 1"])
    fractional = good_input.copy()
    fractional[0, 0] = 0.5
    negative_zero = good_input.copy()
    negative_zero[0, 0] = -0.0
    nonfinite = good_input.copy()
    nonfinite[0, 0] = np.nan
    multi_hot = good_input.copy()
    multi_hot[0, 1] = 1.0
    hole = good_input.copy()
    hole[0, 1 * 5 + ALPHABET.index(" ")] = 0.0
    input_controls = {
        "non_ascii": input_control(session, strings=["é"]),
        "overlong": input_control(session, strings=["1 RRR 1"]),
        "wrong_shape": input_control(session, encoded=np.zeros((1, 29), dtype=np.float32)),
        "fractional_cell": input_control(session, encoded=fractional),
        "negative_zero_cell": input_control(session, encoded=negative_zero),
        "nan_cell": input_control(session, encoded=nonfinite),
        "multi_hot": input_control(session, encoded=multi_hot),
        "non_trailing_padding": input_control(session, encoded=hole),
    }
    control_verdicts = [*controls.values(), *input_controls.values()]
    go = (accepted.kind == "ACCEPT" and accepted.passed == TOTAL_CASES and
          accepted.nonzero_initializer_elements is not None and
          accepted.nonzero_initializer_elements < 400 and
          all(control.kind == "REFUSE" if isinstance(control, Verdict) else
              control["kind"] == "REFUSE" for control in control_verdicts) and
          controls["changed_weight_with_new_digest"].first_counterexample is not None)
    print(json.dumps({
        "schema": "noble.recurrent-sparse-classifier/v1",
        "scope": "non-gating independently hand-authored weights; not a full citation checker",
        "authorship": "fixed hand-set affine weights; no training, gradients or Noble compiler output",
        "source": {"property": PROPERTY, "regex": SOURCE_PATTERN, "alphabet": list(ALPHABET),
                   "lengths": [0, MAX_LENGTH], "codec": CODEC,
                   "encoding": "six positions x five ASCII glyphs; binary32 exact 0/1; trailing zero pad"},
        "runtime": {"onnxruntime": ort.__version__, "onnx": onnx.__version__,
                    "numpy": np.__version__, "provider": "CPUExecutionProvider",
                    "onnx_ir": 10, "onnx_opset": 18, "optimizations": "disabled",
                    "thread_count": 1},
        "model": {"file": model_file.name, "sha256": digest, "bytes": len(original),
                  "initializer_elements": accepted.initializer_elements,
                  "nonzero_initializer_elements": accepted.nonzero_initializer_elements,
                  "classifier_weight_elements": accepted.classifier_weight_elements,
                  "nonzero_classifier_weights": accepted.nonzero_classifier_weights},
        "checker_verdict": asdict(accepted), "exhaustive_rows": exhaustive_rows,
        "behavioral_accuracy": accuracy(session),
        "controls": {name: asdict(value) for name, value in controls.items()},
        "input_controls": input_controls, "nonzero_threshold_exclusive": 400,
        "decision": "GO" if go else "NO-GO",
    }, sort_keys=True, separators=(",", ":")))


if __name__ == "__main__":
    main()
