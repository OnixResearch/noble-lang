# Reviewed selection policy. This is configuration, not an execution receipt.
let Pin = { rev | String, narHash | String } in
{
  schema_version | String = "noble-tool-selection-policy/v1",
  sources | {
    aeneas | Pin, aeneas_nixpkgs | Pin, cairn | Pin, charon | Pin,
    extraction_nixpkgs | Pin, nixpkgs | Pin, octet | Pin, rust_overlay | Pin,
  } = {
    aeneas = {
      rev = "505b6ca35217e7be5c96c3e2f8045edfbdf47291",
      narHash = "sha256-V6H12+Qg3qlZBGdjoTHCbYWSOwcdZ86XCxOkWURo6Qs=",
    },
    aeneas_nixpkgs = {
      rev = "b3d51a0365f6695e7dd5cdf3e180604530ed33b4",
      narHash = "sha256-4vhDuZ7OZaZmKKrnDpxLZZpGIJvAeMtK6FKLJYUtAdw=",
    },
    cairn = {
      rev = "15f00875562025e7ea7e0d1f4af24d1a2e2ac06f",
      narHash = "sha256-r6maRgum3TeE17B/prR6ixA4R5CuLdEOAN8CWAD+zMg=",
    },
    charon = {
      rev = "b104e24fea7d721b71e6c39fd70f26ff20bc0980",
      narHash = "sha256-T5YOon6o64//bS5NihALMrDGivJNwvmoJRoKIGITEmc=",
    },
    extraction_nixpkgs = {
      rev = "643809054d65fdd466a63e3155b8c498cb483c04",
      narHash = "sha256-1+YGJM7PLfvTsbUxDHKj0NcuEXYFHfZBDuajujCcVmY=",
    },
    nixpkgs = {
      rev = "9cf7092bdd603554bd8b63c216e8943cf9b12512",
      narHash = "sha256-9tpvMGFteZnd3gRQZFlRCohVpqooygFuy9yjuyRL2C0=",
    },
    octet = {
      rev = "235255bc4972ced9128fd5b4d1ec66ff7508ded4",
      narHash = "sha256-4vFO0V3jtN4GrFbIcqHPqjaZdQK5tQof682BFOKiFtw=",
    },
    rust_overlay = {
      rev = "e598b37857b895b81020a65a802ef55f5bbed72f",
      narHash = "sha256-KlepQu/O5m11lAjcJ4ER5bc6bIzyX2UMPDARzMzQfIw=",
    },
  },
  configuration = {
    host | String = "x86_64-linux",
    target | String = "x86_64-unknown-linux-gnu",
    word_bits | Number = 64,
    quality_rust | String = "nightly-2026-03-21",
    extraction_rust | String = "nightly-2026-08-18",
    miri_rust | String = "nightly-2026-08-18",
    miri_flags | Array String = ["-Zmiri-strict-provenance"],
    features | Array String = [],
    all_features | Bool = true,
    default_features | Bool = true,
    panic | String = "unwind",
    overflow_checks | Bool = true,
    extraction_profile | String = "dev",
    extraction_subject | String = "crates/noble-kernel/Cargo.toml",
    charon_args | Array String = ["--preset=aeneas", "--error-on-warnings"],
    cargo_args | Array String = ["--lib", "--all-features", "--target", "x86_64-unknown-linux-gnu", "--locked", "--offline"],
    aeneas_args | Array String = ["-backend", "lean", "-abort-on-error", "-warnings-as-errors", "-no-progress-bar", "-emit-json"],
    translator_timeout_seconds | Number = 120,
    lean_timeout_seconds | Number = 900,
    lean_heartbeats | Number = 1000000,
    lean_recursion_depth | Number = 2048,
  },
  lean = {
    toolchain | String = "leanprover/lean4:v4.31.0",
    version | String = "4.31.0",
    rev | String = "68218e876d2a38b1985b8590fff244a83c321783",
    archive_url | String = "https://github.com/leanprover/lean4/releases/download/v4.31.0/lean-4.31.0-linux.tar.zst",
    archive_sha256 | String = "07a633cc8d9151cbc08825ea4cdda50d4b02a2c9cb852c0131b13046f49cad7f",
    backend_url | String = "https://github.com/AeneasVerif/aeneas",
    backend_subdir | String = "backends/lean",
  },
  # Locked subprocess dependencies of the experimental Core-Bootstrap CLI and
  # verification harnesses, not native linked crates or a component runtime.
  verification_tools | { _ : { source | String, attribute | String, version | String, role | String } } = {
    node = {
      source = "nixpkgs",
      attribute = "nodejs_24",
      version = "24.13.0",
      role = "core-bootstrap-and-verification-wasm-host",
    },
    wasm_tools = {
      source = "nixpkgs",
      attribute = "wasm-tools",
      version = "1.245.1",
      role = "core-bootstrap-and-verification-wasm-assembler",
    },
    binaryen = {
      source = "nixpkgs",
      attribute = "binaryen",
      version = "125",
      role = "core-bootstrap-and-verification-wasm-optimizer",
    },
  },
  owned_wasm_runtime = {
    assets | Array String = [
      "crates/noble-wasm/runtime/accounting.wat",
      "crates/noble-wasm/runtime/common.wat",
      "crates/noble-wasm/runtime/data.wat",
      "crates/noble-wasm/runtime/experiment.wat",
      "crates/noble-wasm/runtime/gc-storage.wat",
      "crates/noble-wasm/runtime/linear-storage.wat",
      "crates/noble-wasm/runtime/operations.wat",
      "crates/noble-wasm/runtime/programs.wat",
      "crates/noble-wasm/runtime/reflection.wat",
      "crates/noble-wasm/runtime/source.wat",
      "crates/noble-wasm/runtime/stack.wat",
    ],
    assurance | String = "byte-bound-runtime-semantics-unproved",
    non_claims | Array String = [
      "rust-compiler-inventory-coverage",
      "rust-refinement",
      "wasm-lowering-correspondence",
      "loader-binding-proof",
      "component-support",
    ],
  },
  # These file bindings use SHA-256; Octet separately binds compiler identities.
  file_sha256 | { _ : String } = import "tool-selection-files.ncl",
  tool_paths | { _ : { derivation | String, output | String } } = import "tool-selection-tools.ncl",
  # A source-built independent Rust peer, not an unreviewed Wasmtime CLI.
  # Compatibility is established by the M5 execution gate, never this selection.
  component_sync = import "../verification/m5/pins.json",
  future_unselected | Array String = ["Verus", "byte-view dependency"],
  claims | Array String = ["selection-policy-only", "execution-required-for-compatibility", "refinement-open", "m1-acceptance-open"],
}
