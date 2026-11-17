# Reviewed selection policy. This is configuration, not an execution receipt.
let Pin = { rev | String, narHash | String } in
{
  schema_version | String = "noble-tool-selection-policy/v1",
  sources | {
    aeneas | Pin, aeneas_lean | Pin, aeneas_nixpkgs | Pin, cairn | Pin, charon | Pin,
    extraction_nixpkgs | Pin, nixpkgs | Pin, octet | Pin, rust_overlay | Pin,
  } = {
    # The extractor and Lean library share Aeneas revision 1238879f.
    aeneas = {
      rev = "1238879f9ea550512096e9bf8bd6fe7407b3eaef",
      narHash = "sha256-HXAeoR35sHdqQj9FEz4ulPvVAdxqZlBcMG2ly7OYNDk=",
    },
    # The Aeneas Lean library every proof project requires; it is the extractor's same source tree.
    aeneas_lean = {
      rev = "1238879f9ea550512096e9bf8bd6fe7407b3eaef",
      narHash = "sha256-HXAeoR35sHdqQj9FEz4ulPvVAdxqZlBcMG2ly7OYNDk=",
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
      rev = "9e95cf318e3a2964fcf5e03104cde5b718f98ea5",
      narHash = "sha256-5k2jpmT+hl0ocmJPeyVKlf2ehHJDVJiZy8sovKWHiIg=",
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
      rev = "9d134f00e902586c08bc71e8dc910d305c8dd875",
      narHash = "sha256-b9eFelLNGuhS4FpGD3uVh3yS0J2jtGz2/NDN+9agVP4=",
    },
    rust_overlay = {
      rev = "a1d497a9415999853809ba22a7959b1c36dc6554",
      narHash = "sha256-hulcXtNnQVDTPXiC8dQpTX3FPHxklIfX7Cx2kd2GDrM=",
    },
  },
  configuration = {
    host | String = "x86_64-linux",
    target | String = "x86_64-unknown-linux-gnu",
    word_bits | Number = 64,
    quality_rust | String = "nightly-2026-03-21",
    extraction_rust | String = "nightly-2026-09-17",
    miri_rust | String = "nightly-2026-09-17",
    miri_flags | Array String = ["-Zmiri-strict-provenance"],
    features | Array String = [],
    # Reviewed workspace names, separate from explicitly listed features;
    # cargo_args --all-features still enables them in non-kernel extraction.
    workspace_features | Array String = ["live-test-barrier", "safety-observe"],
    all_features | Bool = true,
    default_features | Bool = true,
    panic | String = "unwind",
    overflow_checks | Bool = true,
    extraction_profile | String = "dev",
    extraction_subject | String = "crates/noble-kernel/Cargo.toml",
    # The declared subject: Charon translates this item and what it reaches, not the whole library.
    extraction_start_from | String = "noble_kernel::consume_budget",
    # The promoted &'static refusal control for #97 passed with this option enabled.
    charon_args | Array String = ["--preset=aeneas", "--error-on-warnings", "--inline-anon-consts"],
    cargo_args | Array String = ["--lib", "--all-features", "--target", "x86_64-unknown-linux-gnu", "--locked", "--offline"],
    aeneas_args | Array String = ["-backend", "lean", "-abort-on-error", "-warnings-as-errors", "-no-progress-bar", "-emit-json"],
    # Charon's Cargo subcommand compiles the selected Rust subject before extraction.
    charon_timeout_seconds | Number = 900,
    translator_timeout_seconds | Number = 120,
    lean_timeout_seconds | Number = 900,
    lean_heartbeats | Number = 1000000,
    lean_recursion_depth | Number = 2048,
  },
  lean = {
    toolchain | String = "leanprover/lean4:v4.35.0",
    version | String = "4.35.0",
    rev | String = "eb6cafb7290e8bd5f2937522d8d804f58f02bafc",
    archive_url | String = "https://github.com/leanprover/lean4/releases/download/v4.35.0/lean-4.35.0-linux.tar.zst",
    archive_sha256 | String = "7289597a44543246c9227b8cae501767e095ce17f6d448b95fc4a8dc97268f6d",
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
    onnx_python = {
      source = "nixpkgs",
      attribute = "python3.withPackages(onnx,onnxruntime)",
      version = "3.13.12",
      role = "recurrent-weights-stock-model-export-and-execution",
    },
    onnx_runtime = {
      source = "nixpkgs",
      attribute = "onnxruntime",
      version = "1.23.2",
      role = "recurrent-weights-stock-cpu-inference-runtime",
    },
  },
  # Experimental issue #127 verifier; this is not a selected Nix tool or a
  # production/Lean gate. The MM0 replay adapter verifies the archived source
  # digest and the built binary before any experimental certificate is accepted.
  experimental_mm0 = {
    source_url = "https://github.com/digama0/mm0.git",
    source_rev = "0d414c0bfdaaeb7fea571895127abc1fa5a3d956",
    git_archive_sha256 = "741ee2b6e13eb189dcf75dead7dbbfc6f818e33800ea781bda99926b1dcf01db",
    checker_source = "mm0-c/main.c",
    checker_build = "gcc -O2 mm0-c/main.c -o mm0-c",
    checker_flags = ["-O2"],
    parser_enabled = true,
    producer_source = "mm0-rs/Cargo.toml",
    producer_build = "cargo build --locked --manifest-path mm0-rs/Cargo.toml --bin mm0-rs",
    role = "opt-in-experimental-graph-certificate-only",
  },
  # Issue #127 qualified backend: compiler-lab #220 `validate mm0-certificate`.
  # Compiler-lab is private, so Noble records its commit and the identities of
  # the Nix-built checker closure instead of a fetch location. Noble consumes the
  # backend's admission report; it does not build or run this checker itself.
  # verification/ngraph/mm0/compiler-lab-request.mjs refuses any other commit,
  # closure, checker, pins or base theory.
  qualified_mm0 = {
    backend = "compiler-lab validate mm0-certificate",
    compiler_lab_commit = "46a988bcbc3bf9b3a6296cecbef8eb4d499c454e",
    checker_profile = "mm0-c-parser-v1",
    obligation_profile = "ngraph-i64-wrapping-v1",
    system = "x86_64-linux",
    pins_sha256 = "563a90f813ec7693d19e58e36cfe63e5a7cbe816be82bf85be478db1f69ee3b0",
    expression_sha256 = "a5c3385cb11246534cc26e64343209b5bb39a2122f1bd5ed8c83cd4772cca0ef",
    tools_closure = "/nix/store/xkfk12m94zcdffzc98dnl57mmkyqs6y9-compiler-lab-mm0-certificate-tools",
    closure_identity_sha256 = "9800e8c5a12b8fd5f6bd50c6e866d48a64bc52b398be83e75f28ea0c00e993d1",
    checker_sha256 = "7b00b9c1373098567fcefda96ef4cffbea8fb7fe65ff729400ba9ab2cc06babb",
    negative_checker_sha256 = "4f690e9da63e89d8bbd90fda0da94034da3a281d7f04dccb0d8ebaf4e52afb30",
    mm0_source_rev = "0d414c0bfdaaeb7fea571895127abc1fa5a3d956",
    base_theory_sha256 = {
      "peano.mm0" = "463a7d6e4f4791313dedcfd737053dcf2cc07b466371e263c38a994c6fb72052",
      "peano_hex.mm0" = "e2f5e461c568e0e3ef784b507e9111272b38956093df98b943ccc4cd806e351b",
    },
    role = "opt-in-experimental-graph-certificate-only",
  },
  owned_wasm_runtime = {
    assets | Array String = [
      "crates/noble-wasm/runtime/accounting.wat",
      "crates/noble-wasm/runtime/common.wat",
      "crates/noble-wasm/runtime/data.wat",
      "crates/noble-wasm/runtime/experiment.wat",
      "crates/noble-wasm/runtime/gc-storage.wat",
      "crates/noble-wasm/runtime/linear-address.wat",
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
  #
  # Every `component_sync` field is classified below by what binds it. The
  # comparator refuses a component_sync field that is not classified, so a new
  # pin cannot enter as an unchecked constant.
  #   - flake-fetcher: the selected Wasmtime tree is fetched at the recorded NAR
  #     hash, so the hash decides the tree rather than describing it.
  #   - flake-assert: `flake.nix` asserts the realised store path equals the
  #     recorded path (Wasmtime source and reviewed vendor).
  #   - comparator: the observed side is read from the selected input (the
  #     Wasmtime workspace version, the selected wasm-tools version).
  #   - declaration: a reviewed configuration string with no runtime binding.
  #   - unbound: a recorded host path that no flake input provides. `linker_bin`
  #     and `publication_probe` are used by the manual M5 gate; binding them
  #     requires selecting the nixpkgs revision they were captured from.
  component_sync = import "../verification/m5/pins.json",
  component_sync_bindings | { _ : String } = {
    schema = "declaration: pin schema identifier",
    profile = "declaration: reviewed component profile name",
    world = "declaration: reviewed canonical world name",
    wit = "declaration: reviewed WIT path, also source-hashed by the M5 gate",
    wasm_tools_version = "comparator: observed selected wasm-tools version must match",
    wasmtime_version = "comparator: observed selected Wasmtime workspace version must match",
    wasmtime_source = "flake-assert: fetched tree path must equal this reviewed store path",
    wasmtime_source_nar_hash = "flake-fetcher: the selected Wasmtime tree is fetched at this NAR hash",
    vendor = "flake-assert: built reviewed vendor path must equal this reviewed store path",
    vendor_nar_hash = "declaration: reviewed vendor NAR hash, not recomputed by a check",
    linker_bin = "unbound: recorded host linker directory for the manual M5 gate",
    canonical_abi = "declaration: reviewed canonical ABI profile name",
    gc_control = "declaration: reviewed GC control description",
    publication_probe = "unbound: recorded host bash and prlimit paths for the manual M5 gate",
    non_claims = "declaration: reviewed non-claim list",
  },
  future_unselected | Array String = ["Verus", "byte-view dependency"],
  claims | Array String = ["selection-policy-only", "execution-required-for-compatibility", "refinement-open", "m1-acceptance-open"],
}
