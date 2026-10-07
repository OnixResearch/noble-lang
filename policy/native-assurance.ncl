# Reviewed native and dependency assurance policy.
# Configuration and expectations, not an execution receipt.
{
  schema_version | String = "noble-native-assurance-policy/v1",
  owner | String = "noble-maintainers",
  scope_note | String =
    "Five-crate workspace scope: noble-kernel, noble-contracts, noble-wasm, noble-cli and noble-syndicate on x86_64-unknown-linux-gnu. This records native Rust and dependency scope, not whole-program soundness or Wasm runtime correspondence. Owned WAT assets and Node/wasm-tools/Binaryen subprocesses are separately byte-bound by tool selection; their semantics are unproved and are not Rust inventory subjects or native linked dependencies. The independent Wasmtime verification peers are separate Cargo workspaces, with the M8 peer's external Cargo lock derived exactly from the reviewed M7 peer lock and selected source/vendor NAR identities. Engine behavior, native resource lifetime and host credential provenance remain explicit assumptions, not production dependency or unsafe-code discharge.",
  required_packages | Array String = [ "noble-kernel", "noble-contracts", "noble-wasm", "noble-cli", "noble-syndicate" ],
  required_targets | Array { triple | String, features | Array String, default_features | Bool } = [
    { triple = "x86_64-unknown-linux-gnu", features = [], default_features = true },
  ],
  # The comparator requires all three basis entries, but they are not equal
  # evidence. The scoped owned-unsafe basis is the workspace unsafe_code =
  # "forbid" lint, the five member [lints] workspace = true opt-ins and a
  # successful compiler/Cargo run. Foreign-item absence is real compiler
  # evidence. The pinned collector emits function-safety facts only for
  # value-flow-tracked return types and the architecture policy tracks none,
  # so their absence and the empty derived site list are vacuous.
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
    # Source-observed direct normal edges from the CLI only. The comparator
    # matches role->name:edge_kind; version and assumption are recorded for
    # review but not compared. The versions were compared once against the
    # sealed Cargo.lock. Wasmtime and 29 other path packages have no
    # Cargo.lock checksum; only the selected Wasmtime source NAR binds them.
    # The architecture policy now permits composition-root normal edges to
    # these providers, including a possible noble-syndicate caller; this
    # comparator separately binds the seven observed noble-cli callers.
    # Neither classification audits transitive dependencies,
    # external unsafe code, native linking or implementation safety.
    production | Array { name | String, version | String, role | String, edge_kind | String, assumption | String } = [
      {
        name = "anyhow",
        version = "1.0.100",
        role = "noble-cli",
        edge_kind = "normal",
        assumption = "CLI error/context formatting and propagation rely on this external crate; a reported error is not proof that every host effect was avoided.",
      },
      {
        name = "blake3",
        version = "1.8.7",
        role = "noble-cli",
        edge_kind = "normal",
        assumption = "CLI experimental identity digests use the selected std/pure features; collision resistance, origin authenticity and external implementation safety are not established here.",
      },
      {
        name = "libc",
        version = "0.2.189",
        role = "noble-cli",
        edge_kind = "normal",
        assumption = "CLI authority path walking uses Linux O_NOFOLLOW/O_DIRECTORY flags through std OpenOptions; kernel and /proc/self/fd behavior and external libc safety remain host assumptions.",
      },
      {
        name = "serde_json",
        version = "1.0.151",
        role = "noble-cli",
        edge_kind = "normal",
        assumption = "CLI JSON parsing and serialization require separate bounded-input and authority/schema checks; syntactic parsing and external implementation safety do not establish trusted commands or receipts.",
      },
      {
        name = "wasmparser",
        version = "0.243.0",
        role = "noble-cli",
        edge_kind = "normal",
        assumption = "CLI Wasm validation and decoding depend on the external parser; its behavior does not prove binary correspondence, ABI semantics, or guest execution safety.",
      },
      {
        name = "wasmtime",
        version = "40.0.2",
        role = "noble-cli",
        edge_kind = "normal",
        assumption = "CLI host adapters execute through the selected absolute Wasmtime source; engine/trap behavior, native dependencies and host resource lifetime remain unproved assumptions, not a trusted proof boundary.",
      },
      {
        name = "wat",
        version = "1.243.0",
        role = "noble-cli",
        edge_kind = "normal",
        assumption = "CLI converts selected UTF-8 WAT to Wasm before separate byte/ABI checks; parser correctness and the resulting guest semantics are not proved by this inventory.",
      },
    ],
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
  # The derivation emits no foreign-library records, so this empty list and
  # its comparator check are not proof of an empty link closure; Wasmtime's
  # build script links a C helper into noble-cli.
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
      argument = "Safe sequential Rust with explicit owned inputs. The workspace unsafe_code forbid lint is the owned-unsafe basis, and no compiler foreign-item fact exists for this crate; absent function-safety facts are not unsafe evidence.",
    },
    {
      scope = "noble-contracts",
      argument = "Safe no_std frontend with explicit owned source bytes and an inward dependency on noble-kernel. The workspace unsafe_code forbid lint is the owned-unsafe basis; native accounting must confirm the absence of foreign-item facts.",
    },
    {
      scope = "noble-wasm",
      argument = "Safe no_std emitter with explicit owned checked-kernel and checked WIT export inputs and allocation limits, depending inward on noble-kernel and noble-contracts. The workspace unsafe_code forbid lint is the owned-unsafe basis; native accounting must confirm the absence of foreign-item facts. Emitting or byte-binding owned WAT does not prove those fragments, backend lowering, loader binding or Wasm execution semantics. The selected Miri subject remains the kernel budget test and does not establish emitter execution coverage.",
    },
    {
      scope = "noble-cli",
      argument = "Composition-root orchestration invokes external JSON/Wasm parsers, Wasmtime execution, digest generation and Linux filesystem flags. The workspace forbids owned unsafe_code in this crate, but does not audit external unsafe implementations, engine behavior, host resource lifetime or command authority; those require the caller checks and explicit assumptions above.",
    },
    {
      scope = "noble-syndicate",
      argument = "Safe std composition-root policy serializes bounded M7 table access with a Mutex and host-owned AtomicU64 session identity. The workspace unsafe_code forbid lint is the owned-unsafe basis; compiler accounting must confirm no foreign-item facts and no third-party production dependencies. Host-assigned facets and engine trap authenticity remain explicit assumptions; source inventory leaves new choreography/parser/refinement bodies open.",
    },
  ],
  target_assumptions | Array { target | String, word_bits | Number, assumptions | Array String } = [
    {
      target = "x86_64-unknown-linux-gnu",
      word_bits = 64,
      assumptions = [
        "64-bit little-endian target",
        "strict provenance is the required Miri model",
        "The compiler/Cargo inventory does not audit linked system libraries or transitive native/unsafe dependency implementations; no owned foreign-item fact is not a linker-closure or host-ABI proof",
        "Core-Bootstrap managed-linear-memory execution, Component-Sync-Bootstrap and the two M3 Wasm experiments are separately scoped guest execution, not additional claimed native Rust targets",
        "Verification-only Wasmtime peers and their dependencies are not included in the five-crate production unsafe-code or native dependency inventory",
      ],
    },
  ],
}
