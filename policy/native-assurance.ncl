# Reviewed native and dependency assurance policy.
# Configuration and expectations, not an execution receipt.
{
  schema_version | String = "noble-native-assurance-policy/v1",
  owner | String = "noble-maintainers",
  scope_note | String =
    "Four-crate workspace scope: noble-kernel, noble-contracts, noble-wasm and noble-cli on x86_64-unknown-linux-gnu. This records native Rust and dependency scope, not whole-program soundness or Wasm runtime correspondence. Owned WAT assets and verification-only Node/wasm-tools/Binaryen processes are separately byte-bound by tool selection; their semantics are unproved and are not Rust inventory subjects or native linked dependencies.",
  required_packages | Array String = [ "noble-kernel", "noble-contracts", "noble-wasm", "noble-cli" ],
  required_targets | Array { triple | String, features | Array String, default_features | Bool } = [
    { triple = "x86_64-unknown-linux-gnu", features = [], default_features = true },
  ],
  # An empty owned-unsafe scope is only valid when compiler accounting supports it.
  owned_unsafe = {
    expectation | String = "empty",
    sites | Array { symbol | String, argument | String, reviewer | String } = [],
    accounting_basis | Array String = [
      "workspace-lint-unsafe-code-forbid",
      "compiler-function-safety-facts-absent",
      "compiler-foreign-item-facts-absent",
    ],
  },
  dependencies = {
    # Reviewed production dependency records. Noble has no third-party production dependency.
    production | Array { name | String, version | String, role | String, assumption | String } = [],
    fixture_only | Array { name | String, version | String, lock | String, purpose | String } = [
      {
        name = "getrandom",
        version = "0.4.3",
        lock = "verification/fixtures/randomness.Cargo.lock",
        purpose = "Boundary randomness fixture only. It is not a production dependency and establishes no dependency safety.",
      },
    ],
  },
  compiler_tools | Array { name | String, role | String } = [
    { name = "cargo-octet", role = "architecture-collector" },
    { name = "charon", role = "extraction" },
    { name = "aeneas", role = "extraction" },
    { name = "lean", role = "proof-assistant" },
    { name = "miri", role = "interpreter" },
  ],
  foreign_libraries | Array { name | String, linkage | String, assumption | String } = [],
  miri = {
    required | Bool = true,
    toolchain | String = "nightly-2026-08-18",
    flags | Array String = [ "-Zmiri-strict-provenance" ],
    target | String = "x86_64-unknown-linux-gnu",
    subject | String = "crates/noble-kernel/Cargo.toml",
    required_tests | Array String = [ "budget" ],
    unsupported_configurations | Array String = [],
  },
  safety_arguments | Array { scope | String, argument | String } = [
    {
      scope = "noble-kernel",
      argument = "Safe sequential Rust with explicit owned inputs. The workspace forbids unsafe_code, and no compiler function-safety or foreign-item fact exists for this crate.",
    },
    {
      scope = "noble-contracts",
      argument = "Safe no_std frontend with explicit owned source bytes and an inward dependency on noble-kernel. The workspace forbids unsafe_code; native accounting must confirm the absence of unsafe and foreign-item facts.",
    },
    {
      scope = "noble-wasm",
      argument = "Safe no_std emitter with explicit owned checked-kernel inputs and allocation limits, depending inward on noble-kernel. The workspace forbids unsafe_code; native accounting must confirm the absence of unsafe and foreign-item facts. Emitting or byte-binding owned WAT does not prove those fragments, backend lowering, loader binding or Wasm execution semantics. The selected Miri subject remains the kernel budget test and does not establish emitter execution coverage.",
    },
    {
      scope = "noble-cli",
      argument = "Shell-only orchestration and unprotected console output. The workspace forbids unsafe_code in this crate too.",
    },
  ],
  target_assumptions | Array { target | String, word_bits | Number, assumptions | Array String } = [
    {
      target = "x86_64-unknown-linux-gnu",
      word_bits = 64,
      assumptions = [
        "64-bit little-endian target",
        "strict provenance is the required Miri model",
        "no target-specific native library is linked",
        "WasmGC and managed-linear-memory execution are separate verification experiments, not additional claimed native targets",
      ],
    },
  ],
}
