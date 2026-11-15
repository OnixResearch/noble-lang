#!/usr/bin/env python3
"""Evaluate one pre-submitted LLM routing artifact without consulting its author."""

import argparse
import hashlib
import importlib.util
import json
import sys
from dataclasses import asdict
from pathlib import Path

import onnx
import onnxruntime as ort


checker_spec = importlib.util.spec_from_file_location(
    "authored_checker", Path(__file__).with_name("authored-checker.py"))
checker = importlib.util.module_from_spec(checker_spec)
sys.modules[checker_spec.name] = checker
checker_spec.loader.exec_module(checker)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(prompt_path, transcript_path, compiled_path, output_dir):
    require(onnx.__version__ == "1.20.1" and ort.__version__ == "1.23.2",
            "selected runtime version drift")
    prompt = prompt_path.read_text()
    transcript_bytes = transcript_path.read_bytes()
    transcript = json.loads(transcript_bytes)
    require(transcript["schema"] == "noble-recurrent-llm-author-transcript/v1" and
            transcript["prompt_path"] == "verification/recurrent-weights/llm-author-prompt.txt" and
            transcript["prompt_sha256"] == hashlib.sha256(prompt.encode()).hexdigest() and
            isinstance(transcript["response"], dict),
            "authored prompt or complete response does not match committed transcript")
    response = transcript["response"]
    require(isinstance(response, dict) and
            set(response) == {"indices", "derivation", "model_identity", "exposure"},
            "authored response is incomplete or adds undeclared fields")
    indices = response["indices"]
    require(type(indices) is list and len(indices) == 128 and
            all(type(index) is int and 0 <= index < 128 for index in indices),
            "authored routing array is not exactly 128 source indices")
    require(type(response["derivation"]) is str and response["derivation"].strip(),
            "author did not give a derivation")
    output_dir.mkdir(mode=0o700, parents=True, exist_ok=True)
    model_bytes = checker.authored_model(indices)
    model_path = output_dir / "llm-authored-scan.onnx"
    model_path.write_bytes(model_bytes)
    verdict = checker.check_candidate("stack-swap", checker.CODEC, model_bytes)

    changed = indices.copy()
    changed[0] = 0 if indices[0] != 0 else 1
    wrong_model = checker.authored_model(changed)
    controls = {
        "wrong_routing_weight": checker.check_candidate("stack-swap", checker.CODEC, wrong_model),
        "wrong_codec": checker.check_candidate("stack-swap", "big-endian", model_bytes),
        "wrong_source": checker.check_candidate("accumulator", checker.CODEC, model_bytes),
        "corrupted_wire_bytes": checker.check_candidate(
            "stack-swap", checker.CODEC, bytes([model_bytes[0] ^ 1]) + model_bytes[1:]),
    }
    require(all(control.kind == "REFUSE" for control in controls.values()),
            "authored control accepted a corrupted model/configuration")
    compiled = json.loads(compiled_path.read_text())
    require(compiled["schema"] == "noble.recurrent-model-state-receipt/v1" and
            compiled["model_state"]["coverage"]["stock"],
            "missing separate compiled stock-runtime evidence")
    compiled_swap = next(result for result in compiled["model_state"]["results"]
                         if result["program"] == "stack-swap")
    require(compiled_swap["coverage"]["tiny"]["transitions"] == 16 and
            compiled_swap["coverage"]["tiny"]["mismatches"] == 0,
            "compiled checker did not pass its finite domain")
    model_identity = response["model_identity"]
    exposure = response["exposure"]
    valid_identity = isinstance(model_identity, str) and model_identity not in ("", "unknown")
    blind_author = exposure == (
        "No compiler output or checker feedback was inspected; only the author-facing prompt was read.")
    go = (valid_identity and blind_author and verdict.kind == "ACCEPT" and
          verdict.passed == 16 and len(controls) == 4)
    return {
        "schema": "noble-recurrent-llm-author-arm/v1",
        "criterion": "verification/recurrent-weights/batch9-criteria.txt",
        "prompt_sha256": hashlib.sha256(prompt.encode()).hexdigest(),
        "transcript_sha256": hashlib.sha256(transcript_bytes).hexdigest(),
        "reported_model_identity": model_identity,
        "reported_prior_exposure": exposure,
        "submission_attempts": 1,
        "no_training_or_fitting": True,
        "candidate_model_sha256": hashlib.sha256(model_bytes).hexdigest(),
        "candidate_model_bytes": len(model_bytes),
        "candidate_path": str(model_path),
        "llm_authored": asdict(verdict),
        "controls": {name: asdict(control) for name, control in controls.items()},
        "compiled_comparator": {
            "source_commit": compiled["source_commit"],
            "model_sha256": compiled_swap["model_sha256"],
            "property": checker.PROPERTY,
            "tiny_passed": 16,
            "tiny_total": 16,
            "first_counterexample": None,
            "parameter_count": compiled_swap["parameter_count"],
            "weight_bytes": compiled_swap["weight_bytes"],
            "corruption_refusals": len(compiled_swap["controls"]),
        },
        "decision": "GO" if go else "NO-GO",
        "decision_reason": "independent one-shot author passed named property and custody" if go else
                           "authorship identity/exposure or semantic bar not met",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--prompt", type=Path, required=True)
    parser.add_argument("--transcript", type=Path, required=True)
    parser.add_argument("--compiled-receipt", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(run(args.prompt, args.transcript, args.compiled_receipt, args.output_dir),
                     sort_keys=True))


if __name__ == "__main__":
    main()
