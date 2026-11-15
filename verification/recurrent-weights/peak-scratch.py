#!/usr/bin/env python3
"""Measure the selected ORT CPU BFC arena, not process RSS.

The slot numbers below are the OrtApi v23 declaration order in ONNX Runtime
v1.23.2's include/onnxruntime/core/session/onnxruntime_c_api.h. The library is
both policy-selected and byte-hashed before any function pointer is called.
No sampling, allocation hook, Python heap counter, or process RSS is used.
"""

import argparse
import ctypes as c
import hashlib
import json
import os
import sys
import time
from pathlib import Path

import onnx


ORT_VERSION = "1.23.2"
ORT_BINARY_SHA256 = "02863aba1dfc2ff57212f3be9a2316f2ca24afffa4d2ccb4bcf71d0323182c38"
SELECTED_EXPORT_SHA256 = "bf84fa4454e7313a9500a4ece04d2fbc8b6f1ddcb3dfb4698a3ae19014f4b4ae"
SELECTED_MODELS = {
    "accumulator": "a797dcd849805991b43771cf49e9e30a232b324963839d8b76af18dfceb795c8",
    "stack-swap": "db793d0d78ee729457f5af5f3476dbff12714c698dc5ba25a9d08d2f5dfbd9ea",
}
API_VERSION = 23
# ORT_API_VERSION=23, 64-bit Linux, OrtApi fields in header declaration order.
SLOTS = {
    "GetErrorMessage": 2, "CreateEnv": 3, "CreateSessionFromArray": 8,
    "Run": 9, "CreateSessionOptions": 10, "EnableCpuMemArena": 18,
    "SetSessionGraphOptimizationLevel": 23,
    "SetIntraOpNumThreads": 24, "SetInterOpNumThreads": 25,
    "CreateTensorWithDataAsOrtValue": 49, "GetTensorMutableData": 51,
    "CreateCpuMemoryInfo": 69, "MemoryInfoGetType": 74,
    "ReleaseEnv": 92, "ReleaseStatus": 93, "ReleaseMemoryInfo": 94,
    "ReleaseSession": 95, "ReleaseValue": 96, "ReleaseSessionOptions": 100,
    "CreateAllocator": 131, "ReleaseAllocator": 132,
    "GetTensorMemoryInfo": 194, "GetKeyValuePairs": 298,
    "ReleaseKeyValuePairs": 300, "GetTensorSizeInBytes": 318,
    "AllocatorGetStats": 319,
}
PTR = c.c_void_p
P_PTR = c.POINTER(PTR)
U64_MAX = (1 << 64) - 1
BIT_ONE = 0x3F800000
REQUIRED_STATS = {"InUse", "TotalAllocated", "MaxInUse", "NumAllocs",
                  "NumReserves", "NumArenaExtensions", "NumArenaShrinkages"}


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def sha256(path):
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").hexdigest()


class OrtApi:
    def __init__(self, library_path):
        self.library = c.CDLL(str(library_path))
        get_base = self.library.OrtGetApiBase
        get_base.restype = PTR
        base = get_base()
        require(base, "selected ORT library has no API base")
        base_slots = (PTR * 2).from_address(base)
        version = c.CFUNCTYPE(c.c_char_p)(base_slots[1])().decode("ascii")
        require(version == ORT_VERSION, "ORT binary version differs from selected v1.23.2")
        get_api = c.CFUNCTYPE(PTR, c.c_uint32)(base_slots[0])
        api = get_api(API_VERSION)
        require(api, "selected ORT binary has no v23 C API")
        self.slots = (PTR * (max(SLOTS.values()) + 1)).from_address(api)
        require(all(self.slots[index] for index in SLOTS.values()),
                "selected ORT C API lacks a required arena/statistics entry")
        self.version = version
        self.message = self.fn("GetErrorMessage", c.c_char_p, PTR)
        self.release_status = self.fn("ReleaseStatus", None, PTR)

    def fn(self, name, result, *args):
        return c.CFUNCTYPE(result, *args)(self.slots[SLOTS[name]])

    def checked(self, name, *signature):
        raw = self.fn(name, PTR, *signature)

        def call(*args):
            status = raw(*args)
            if status:
                try:
                    message = self.message(status).decode("utf-8", errors="replace")
                finally:
                    self.release_status(status)
                raise RuntimeError(f"ORT {name}: {message}")
        return call

    def snapshot(self, allocator):
        pairs = PTR()
        started = time.perf_counter_ns()
        self.checked("AllocatorGetStats", PTR, P_PTR)(allocator, c.byref(pairs))
        require(pairs.value, "ORT allocator returned null statistics")
        try:
            keys = c.POINTER(c.c_char_p)()
            values = c.POINTER(c.c_char_p)()
            count = c.c_size_t()
            self.fn("GetKeyValuePairs", None, PTR, c.POINTER(c.POINTER(c.c_char_p)),
                    c.POINTER(c.POINTER(c.c_char_p)), c.POINTER(c.c_size_t))(
                        pairs, c.byref(keys), c.byref(values), c.byref(count))
            stats = {keys[i].decode("ascii"): int(values[i]) for i in range(count.value)}
        finally:
            self.fn("ReleaseKeyValuePairs", None, PTR)(pairs)
        elapsed = time.perf_counter_ns() - started
        require(REQUIRED_STATS <= stats.keys(),
                "selected ORT allocator does not expose BFC arena statistics")
        require(stats["MaxInUse"] >= stats["InUse"] >= 0 and
                stats["TotalAllocated"] >= stats["InUse"],
                "inconsistent ORT arena statistics")
        return stats, elapsed


def checked_words(raw, size):
    require(type(raw) is list and len(raw) == size, "incorrect checked vector word count")
    words = []
    for text in raw:
        require(type(text) is str and text.isascii() and text.isdecimal() and
                str(int(text)) == text and 0 <= int(text) <= U64_MAX,
                "noncanonical checked vector word")
        words.append(int(text))
    return words


def prepare_vectors(export, program):
    specs = json.loads(export.read_text())
    require(type(specs) is list and [spec["program"] for spec in specs] ==
            ["accumulator", "stack-swap"], "checked export has wrong program roster")
    spec = next(spec for spec in specs if spec["program"] == program)
    require(spec["schema"] == "noble.recurrent-stock-export/v1", "wrong checked export schema")
    rows = spec["vectors"]["free_running"]
    require(type(rows) is list and len(rows) == 1000, "missing 1000 checked free-running rows")
    state_size = 1 if program == "accumulator" else 2
    first_state = checked_words(rows[0][0], state_size)
    scan_tokens = []
    expected_states = []
    last = first_state
    for index, row in enumerate(rows):
        require(type(row) is list and len(row) == 3, "malformed checked free-running row")
        state = checked_words(row[0], state_size)
        words = checked_words(row[1], 1 if program == "accumulator" else 0)
        target = checked_words(row[2], state_size)
        require(state == last, f"checked trajectory discontinuity at step {index}")
        source = ([(state[0] + words[0]) & U64_MAX] if program == "accumulator"
                  else [state[1], state[0]])
        require(target == source, f"checked source relation mismatch at step {index}")
        last = target
        expected_states.append(target)
        scan_tokens.extend(words if program == "accumulator" else [0])
    state_cells = state_size * 64
    initial = (c.c_float * state_cells)(
        *(float((word >> bit) & 1) for word in first_state for bit in range(64)))
    tokens = (c.c_float * (len(scan_tokens) * (64 if state_size == 1 else 1)))(
        *(float((word >> bit) & 1) for word in scan_tokens
          for bit in range(64 if state_size == 1 else 1)))
    return spec, initial, tokens, expected_states


def model_metadata(model_bytes, state_cells):
    model = onnx.load_model_from_string(model_bytes)
    require(onnx.__version__ == "1.20.1" and model.ir_version == 10 and
            [(entry.domain, entry.version) for entry in model.opset_import] == [("", 18)] and
            model.producer_name == "noble-recurrent-scan/v1", "unselected Scan model format")
    graph = model.graph
    require(len(graph.node) == 1 and graph.node[0].op_type == "Scan" and
            [part.name for part in graph.input] == ["initial_state_bits", "scan_inputs"] and
            [part.name for part in graph.output] == ["final_state_bits", "observations"],
            "model is not the selected Scan ABI")
    attributes = {item.name: item for item in graph.node[0].attribute}
    require(set(attributes) == {"body", "num_scan_inputs"} and
            attributes["num_scan_inputs"].i == 1 and
            attributes["body"].g.input[0].type.tensor_type.shape.dim[0].dim_value == state_cells,
            "Scan body state cell count differs from program")
    body = attributes["body"].g
    initializers = [*graph.initializer, *body.initializer]
    require(all(not item.external_data and item.raw_data for item in initializers),
            "external or unaccounted initializer payload")
    return {"initializer_count": len(initializers),
            "initializer_payload_bytes": sum(len(item.raw_data) for item in initializers),
            "initializer_location": "ONNX payload bytes; ORT storage/placement is opaque"}


def output_info(api, value):
    mem_info = PTR()
    api.checked("GetTensorMemoryInfo", PTR, P_PTR)(value, c.byref(mem_info))
    allocator_type = c.c_int()
    api.checked("MemoryInfoGetType", PTR, c.POINTER(c.c_int))(
        mem_info, c.byref(allocator_type))
    size = c.c_size_t()
    api.checked("GetTensorSizeInBytes", PTR, c.POINTER(c.c_size_t))(
        value, c.byref(size))
    return {"bytes": size.value, "allocator_type": allocator_type.value,
            "included_in_arena": allocator_type.value == 1}


def decode_bits(api, value, cells, steps):
    data = PTR()
    api.checked("GetTensorMutableData", PTR, P_PTR)(value, c.byref(data))
    require(data.value, "ORT output tensor has null data")
    raw = c.cast(data, c.POINTER(c.c_uint32 * (cells * steps))).contents
    result = []
    for step in range(steps):
        words = []
        for start in range(0, cells, 64):
            word = 0
            for bit in range(64):
                cell = raw[step * cells + start + bit]
                require(cell in (0, BIT_ONE), "ORT emitted a non-exact binary32 bit cell")
                word |= (cell == BIT_ONE) << bit
            words.append(word)
        result.append(words)
    return result


def measure(api, model_bytes, initial, tokens, expected):
    cells = len(initial)
    env = PTR()
    options = PTR()
    session = PTR()
    memory_info = PTR()
    input_memory_info = PTR()
    allocator = PTR()
    inputs = (PTR * 2)()
    outputs = (PTR * 2)()
    releasers = {"ReleaseValue": api.fn("ReleaseValue", None, PTR),
                 "ReleaseAllocator": api.fn("ReleaseAllocator", None, PTR),
                 "ReleaseMemoryInfo": api.fn("ReleaseMemoryInfo", None, PTR),
                 "ReleaseSession": api.fn("ReleaseSession", None, PTR),
                 "ReleaseSessionOptions": api.fn("ReleaseSessionOptions", None, PTR),
                 "ReleaseEnv": api.fn("ReleaseEnv", None, PTR)}
    try:
        api.checked("CreateEnv", c.c_int, c.c_char_p, P_PTR)(4, b"noble-peak-scratch", c.byref(env))
        api.checked("CreateSessionOptions", P_PTR)(c.byref(options))
        api.checked("SetIntraOpNumThreads", PTR, c.c_int)(options, 1)
        api.checked("SetInterOpNumThreads", PTR, c.c_int)(options, 1)
        api.checked("SetSessionGraphOptimizationLevel", PTR, c.c_int)(options, 0)
        api.checked("EnableCpuMemArena", PTR)(options)
        # Same default memory-pattern setting as the selected Python ScanEngine.
        model_buffer = c.create_string_buffer(model_bytes)
        api.checked("CreateSessionFromArray", PTR, PTR, c.c_size_t, PTR, P_PTR)(
            env, c.cast(model_buffer, PTR), len(model_bytes), options, c.byref(session))
        api.checked("CreateCpuMemoryInfo", c.c_int, c.c_int, P_PTR)(
            1, 0, c.byref(memory_info))  # OrtArenaAllocator, OrtMemTypeDefault
        api.checked("CreateCpuMemoryInfo", c.c_int, c.c_int, P_PTR)(
            0, 0, c.byref(input_memory_info))  # external host buffers, OrtDeviceAllocator
        api.checked("CreateAllocator", PTR, PTR, P_PTR)(
            session, memory_info, c.byref(allocator))
        shape_state = (c.c_int64 * 1)(cells)
        shape_tokens = (c.c_int64 * 2)(1000, len(tokens) // 1000)
        create_input = api.checked("CreateTensorWithDataAsOrtValue", PTR, PTR,
                                   c.c_size_t, c.POINTER(c.c_int64), c.c_size_t,
                                   c.c_int, P_PTR)
        create_input(input_memory_info, c.cast(initial, PTR), c.sizeof(initial),
                     shape_state, 1, 1, c.byref(inputs, 0))
        create_input(input_memory_info, c.cast(tokens, PTR), c.sizeof(tokens),
                     shape_tokens, 2, 1, c.byref(inputs, c.sizeof(PTR)))
        idle, idle_cost = api.snapshot(allocator)
        no_run, control_cost = api.snapshot(allocator)
        input_names = (c.c_char_p * 2)(b"initial_state_bits", b"scan_inputs")
        output_names = (c.c_char_p * 2)(b"final_state_bits", b"observations")
        started = time.perf_counter_ns()
        api.checked("Run", PTR, PTR, c.POINTER(c.c_char_p), c.POINTER(PTR),
                    c.c_size_t, c.POINTER(c.c_char_p), c.c_size_t, c.POINTER(PTR))(
                        session, None, input_names, inputs, 2, output_names, 2, outputs)
        elapsed = time.perf_counter_ns() - started
        live, live_cost = api.snapshot(allocator)
        require(all(outputs), "ORT omitted a Scan output")
        buffers = [output_info(api, item) for item in outputs]
        require([item["bytes"] for item in buffers] ==
                [cells * 4, 1000 * cells * 4], "ORT output buffers have wrong size")
        states = decode_bits(api, outputs[1], cells, 1000)
        final = decode_bits(api, outputs[0], cells, 1)[0]
        require(states == expected and final == expected[-1],
                "measured Scan execution disagrees with all checked free-running vectors")
        for index in range(2):
            releasers["ReleaseValue"](outputs[index])
            outputs[index] = None
        after_release, release_cost = api.snapshot(allocator)
        baseline_stable = idle == no_run
        # MaxInUse is lifetime high-water, not a resettable per-Run counter.
        # Only infer a run peak if it began at the idle level OR the run raised
        # the previous lifetime peak; otherwise the earlier load peak masks it.
        witnessed = baseline_stable and (idle["MaxInUse"] == idle["InUse"] or
                                            live["MaxInUse"] > idle["MaxInUse"])
        reason = (None if witnessed else
                  "arena stats sampler changed idle values" if not baseline_stable else
                  "session-load lifetime MaxInUse masks Run peak; no per-Run arena high-water")
        return {
            "verdict": "MEASURED" if witnessed else "NO-GO", "reason": reason,
            "steps": 1000, "host_calls": 1, "run_wall_ns": elapsed,
            "input_buffers": {"initial_bytes": c.sizeof(initial),
                              "scan_bytes": c.sizeof(tokens), "in_ort_arena": False},
            "output_buffers": {"final_state": buffers[0], "observations": buffers[1]},
            "arena": {"baseline_idle": idle, "no_run_control": no_run,
                      "after_run_outputs_live": live, "after_outputs_release": after_release,
                      "net_run_high_water_bytes": live["MaxInUse"] - idle["InUse"]
                      if witnessed else None,
                      "new_capacity_bytes": live["TotalAllocated"] - idle["TotalAllocated"],
                      "free_retained_capacity_after_release_bytes":
                      after_release["TotalAllocated"] - after_release["InUse"],
                      "lifetime_max_before_run_bytes": idle["MaxInUse"],
                      "max_advanced_during_run": live["MaxInUse"] > idle["MaxInUse"]},
            "profiler": {"kind": "ORT v23 AllocatorGetStats on session CPU BFCArena",
                         "sampling": "idle, no-Run control, after synchronous Run, after output release; no hook inside Run",
                         "counter_read_wall_ns": [idle_cost, control_cost, live_cost, release_cost],
                         "no_run_arena_counter_delta": {
                             key: no_run[key] - idle[key] for key in REQUIRED_STATS},
                         "nonperturbing_control": baseline_stable,
                         "instrumentation_inside_run": False},
            "output_match": {"checked_steps": len(states), "mismatches": 0,
                             "final_state_matches": True},
        }
    finally:
        for item in outputs:
            if item:
                releasers["ReleaseValue"](item)
        for item in inputs:
            if item:
                releasers["ReleaseValue"](item)
        for name, value in [("ReleaseAllocator", allocator), ("ReleaseSession", session),
                            ("ReleaseMemoryInfo", memory_info),
                            ("ReleaseMemoryInfo", input_memory_info),
                            ("ReleaseSessionOptions", options), ("ReleaseEnv", env)]:
            if value.value:
                releasers[name](value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--program", choices=("accumulator", "stack-swap"), required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--model-sha256", required=True)
    parser.add_argument("--export", type=Path, required=True)
    parser.add_argument("--export-sha256", required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    selection = json.loads((root / "policy/tool-selection.json").read_text())
    runtime = selection["tool_paths"]["onnx_runtime"]
    interpreter = selection["tool_paths"]["onnx_python"]["output"]
    library_path = Path(runtime["output"]) / "lib/libonnxruntime.so.1.23.2"
    require(selection["verification_tools"]["onnx_runtime"]["version"] == ORT_VERSION and
            os.path.samefile(sys.executable, Path(interpreter) / "bin/python") and
            sha256(library_path) == ORT_BINARY_SHA256,
            "unselected ORT library, binary hash, or Python interpreter")
    require(args.model_sha256 == SELECTED_MODELS[args.program] and
            args.export_sha256 == SELECTED_EXPORT_SHA256 and
            sha256(args.model) == args.model_sha256 and
            sha256(args.export) == args.export_sha256,
            "Scan model or checked export differs from selected receipt identities")
    model_bytes = args.model.read_bytes()
    require(hashlib.sha256(model_bytes).hexdigest() == args.model_sha256,
            "Scan model changed while being read")
    spec, initial, tokens, expected = prepare_vectors(args.export, args.program)
    metadata = model_metadata(model_bytes, len(initial))
    api = OrtApi(library_path)
    report = {
        "schema": "noble.recurrent-peak-scratch/v1", "program": args.program,
        "model": {"path": str(args.model.resolve()), "sha256": args.model_sha256,
                  "serialized_bytes": len(model_bytes), **metadata},
        "checked_export": {"path": str(args.export.resolve()),
                           "sha256": args.export_sha256, "graph_id": spec["graph_id"],
                           "snapshot_id": spec["snapshot_id"]},
        "weights": {"semantic_snapshot_bytes": len(bytes.fromhex(spec["snapshot_bytes"])),
                    "semantic_snapshot_sha256": hashlib.sha256(
                        bytes.fromhex(spec["snapshot_bytes"])).hexdigest(),
                    "placement": "immutable ONNX initializers may reside inside or outside arena; "
                                 "do not subtract their bytes from the observed baseline"},
        "runtime": {"version": api.version, "provider": "CPUExecutionProvider",
                    "graph_optimizations": "disabled", "threads": 1,
                    "binary_path": str(library_path), "binary_sha256": ORT_BINARY_SHA256,
                    "selected_nix_derivation": runtime["derivation"]},
        "scope": {"counted": "session CPU BFC allocator InUse/MaxInUse/TotalAllocated; "
                             "output tensor data if allocator_type=1; cached BFC workspace stays in "
                             "TotalAllocated after release",
                  "excluded": "Python host input arrays, model bytes, protobuf parse, ORT non-arena "
                              "C++ heap allocations, thread stacks, OS page residency; no RSS proxy",
                  "baseline": "after session load and host input setup; weights not assumed zero",
                  "high_water": "MaxInUse is lifetime since session creation; net only if "
                                "pre-Run MaxInUse equals idle InUse or Run advances lifetime maximum"},
    }
    try:
        report.update(measure(api, model_bytes, initial, tokens, expected))
    except (RuntimeError, ValueError) as exc:
        report.update({"verdict": "NO-GO", "reason": str(exc),
                       "arena": {"net_run_high_water_bytes": None}})
    print(json.dumps(report, sort_keys=True))
    if report["verdict"] != "MEASURED":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
