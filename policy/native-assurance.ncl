# Reviewed native and dependency assurance policy.
# Configuration and expectations, not an execution receipt.
{
  schema_version | String = "noble-native-assurance-policy/v1",
  owner | String = "noble-maintainers",
  scope_note | String =
    "Named M1 workspace scope: noble-kernel and noble-cli on x86_64-unknown-linux-gnu. This records native and dependency scope, not whole-program soundness.",
  required_packages | Array String = [ "noble-kernel", "noble-cli" ],
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
      ],
    },
  ],
}
