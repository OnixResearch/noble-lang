# Cranelift execution route evaluation

## ADDED Requirements

### Requirement: NATIVE-ENGINE-01
r[NATIVE-ENGINE-01]

A Wasmtime/Cranelift engine for Core-Bootstrap computation MUST be a distinct, explicitly requested, nonselected profile until a separate selection decision. It MUST execute the byte-identical emitted and validated Core Wasm that the selected Node/V8 route would receive from the same independently accepted source and options. It MUST pin the Wasmtime version, cargo features and complete engine configuration. It MUST provide every imported host operation with the same contract and authorization checks as the selected host. Noble-emitted dispatch and allocation accounting MUST remain the profile's declared work quota. Engine fuel, if enabled, MUST be a separately named unit with its own accounting revision. Deadlines and cancellation MUST be reported separately from deterministic exhaustion. The profile MUST declare and enforce a bound on instances and code retained per Store or session; reaching it MUST refuse or end the session explicitly. Unsupported profiles, including live sessions, the calculator, live slots and the WasmGC representation unless separately admitted, MUST refuse rather than fall back to Node/V8. Node/V8 MUST NOT fall back to Wasmtime either.

### Requirement: NATIVE-AOT-01
r[NATIVE-AOT-01]

A precompiled Wasmtime artifact MUST NOT be loaded unless its bytes equal the host's own trusted precompilation of independently re-derived Wasm for the accepted subject, or come from a separately reviewed authenticated build route. The artifact key MUST bind the Wasm bytes, engine version, features, full configuration and target. A candidate-supplied digest, manifest, path or signature MUST NOT authorize loading. Any `unsafe` deserialization MUST be confined to a minimal separately inventoried boundary with a recorded safety argument. It MUST NOT be enabled by relaxing the workspace-wide `unsafe_code` policy. Mismatched, foreign-version, foreign-configuration, wrong-target or mutated artifacts MUST refuse before deserialization, with zero guest execution. Reports MUST show offline compilation cost separately from online load and instantiation cost.

### Requirement: NATIVE-CLIF-01
r[NATIVE-CLIF-01]

A direct Noble → Cranelift IR → native experiment MUST be a separately scoped, nonselected profile. It applies only to independently accepted requested computation in an explicitly bounded subset, and unsupported inputs MUST refuse rather than narrow or fall back. Type, effect, capture-eligibility and ownership acceptance and host authorization MUST precede lowering; Cranelift IR verification or successful compilation MUST NOT substitute for them. Guest code MUST address runtime data only through checked handles or offsets, never raw host addresses. Program dispatch MUST be confined to a bounds-checked, append-only, single-signature entry table. Arithmetic, operand order, traps, effect prefixes, captures, recipes and quota classifications MUST match the selected semantics. Generated native code for untrusted Programs MUST execute only inside a separately pinned out-of-process sandbox meeting obligations at least equivalent to CB-SANDBOX-01. Executable-code publication, retirement and retained-code bytes MUST be accounted, and code reachable from a Program value, active frame or saved Program MUST NOT be retired. Any `unsafe` executable-memory or code-pointer boundary MUST be separately inventoried with a recorded safety argument and MUST NOT weaken the workspace `unsafe_code` policy.

### Requirement: NATIVE-GATE-01
r[NATIVE-GATE-01]

No Wasmtime or direct-native route MAY replace or join the selected Core execution target without pinned differential evidence against the selected Node/V8 route on the same accepted sources and inputs. That evidence MUST be checked against independently derived expectations and compare ordered values, signed I64 text, normalized recipes, effect prefixes, traps and quota classes. Each run MUST record tool versions, features, configuration, representation, optimization modes, hardware and cache state. Frontend, lowering, assembly, engine compilation, artifact loading, instantiation, first-result and steady-state costs MUST be reported separately with repeated samples. Unsupported workloads MUST be recorded as unsupported, never as successful or zero-cost. Decision thresholds MUST be fixed before measurement. Existing Wasm, Mathlib comparison, C-backend or proof evidence MUST NOT be inherited by a new route.
