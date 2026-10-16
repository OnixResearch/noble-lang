let contracts = import "./architecture.contracts.ncl" in
{
  schema_version = "octet-project-architecture-policy/v1",
  project_id = "noble",
  mode = "gate",
  production = {
    packages = ["noble-kernel", "noble-contracts", "noble-wasm", "noble-cli", "noble-syndicate", "noble-ngraph", "noble-ngraph-experiment"],
    source_scopes = ["crates/noble-kernel/src", "crates/noble-contracts/src", "crates/noble-wasm/src", "crates/noble-cli/src", "crates/noble-syndicate/src", "crates/noble-ngraph/src", "crates/noble-ngraph-experiment/src"],
    targets = [{
      triple = "x86_64-unknown-linux-gnu",
      features = [],
      default_features = true,
    }],
  },
  roles = [
    { scope_kind = "package", scope = "noble-kernel", role = "domain-core" },
    { scope_kind = "package", scope = "noble-contracts", role = "domain-core" },
    { scope_kind = "package", scope = "noble-wasm", role = "domain-core" },
    { scope_kind = "package", scope = "noble-cli", role = "composition-root" },
    { scope_kind = "package", scope = "noble-syndicate", role = "composition-root" },
    { scope_kind = "package", scope = "noble-ngraph", role = "domain-core" },
    { scope_kind = "package", scope = "noble-ngraph-experiment", role = "composition-root" },
    { scope_kind = "source", scope = "crates/noble-kernel/src", role = "domain-core" },
    { scope_kind = "source", scope = "crates/noble-contracts/src", role = "domain-core" },
    { scope_kind = "source", scope = "crates/noble-wasm/src", role = "domain-core" },
    { scope_kind = "source", scope = "crates/noble-cli/src", role = "composition-root" },
    { scope_kind = "source", scope = "crates/noble-syndicate/src", role = "composition-root" },
    { scope_kind = "source", scope = "crates/noble-ngraph/src", role = "domain-core" },
    { scope_kind = "source", scope = "crates/noble-ngraph-experiment/src", role = "composition-root" },
    { scope_kind = "source", scope = "crates/noble-kernel/tests", role = "test" },
    { scope_kind = "source", scope = "crates/noble-contracts/tests", role = "test" },
    { scope_kind = "source", scope = "crates/noble-wasm/tests", role = "test" },
    { scope_kind = "source", scope = "crates/noble-cli/tests", role = "test" },
    { scope_kind = "source", scope = "crates/noble-syndicate/tests", role = "test" },
    { scope_kind = "source", scope = "crates/noble-ngraph/tests", role = "test" },
    { scope_kind = "source", scope = "crates/noble-ngraph-experiment/tests", role = "test" },
  ],
  dependency_rules = [
    {
      caller_role = "domain-core",
      allowed_callee_roles = ["domain-core"],
      edge_kinds = ["normal", "build"],
    },
    {
      caller_role = "composition-root",
      allowed_callee_roles = ["domain-core"],
      edge_kinds = ["normal", "build"],
    },
  ],
  # The observed Cargo graph has ten direct third-party normal edges: seven
  # from noble-cli and three (blake3, rustc_apfloat, serde_json) from the
  # issue #14 noble-ngraph-experiment crate. That crate is an isolated
  # measurement harness outside the ordinary compiler path; the policy has no
  # non-production package role, and Octet collects only production packages,
  # so it is a composition root like noble-cli and noble-syndicate. The
  # Numeric-Graph-v1 library noble-ngraph is domain-core, no_std and has no
  # third-party dependency: the ngraph IR, checker, evaluators, trainer and
  # codecs that moved out of the experiment call no provider, and BLAKE3
  # identities are injected by the CLI and experiment composition roots. No
  # application-owned ports are declared. Provider permission is
  # role-granular: noble-syndicate is also composition-root, so this policy
  # alone would allow its future normal edges to these providers.
  # The separate native and source comparisons bind exact observed callers:
  # the ten external edges are source-reviewed, and a new syndicate external
  # edge would fail source dependency closure. Source comparison
  # remains red on other classifications; neither downstream comparator runs
  # after the blocked Octet architecture gate or attests native safety.
  # The CLI owns consumer I/O; no domain-core crate executes host effects.
  # noble-contracts includes the MC2 companion admission, derivation, registry,
  # guard and release core. Contract/proof text, guest handles, reported tags,
  # runtime observations and producer status text are untrusted inputs, never
  # acceptance witnesses. The core's retained decisions cannot be replaced by
  # a host report, a wrapper, a cached digest or an asserted evidence class.
  # Lean kernel acceptance and the CLI's exact source/claim binding form an
  # explicit trusted boundary; process success alone does not certify a claim.
  # Only the CLI runs the independent checker. The core issues an opaque
  # admission request and consumes a complete immutable host observation,
  # rechecking exact statement/source/class and retained policy/revision before
  # bounded completion. No host callback or proof process executes in the core.
  # Checker soundness and observation authenticity are explicit trusted
  # premises, not Rust-body proofs or unforgeability against arbitrary host
  # Rust. A guest offer, cached status or producer report is not that observation.
  # Owned JS/WAT are byte-bound by tool selection, not Rust compiler subjects
  # or proved runtime semantics. Engine handles identify guest values, not
  # authority to fabricate core evidence or bypass a certified invocation guard.
  # M5 resource ownership and protected admission are deterministic core state
  # machines. Trusted host facts and authenticated observations enter explicitly;
  # a guest request, handle, plan or receipt-shaped data cannot supply authority.
  # The independent Rust/Wasmtime peer is verification-only, outside production
  # workspace accounting. Its engine/native release and credential provenance
  # remain explicit host assumptions, not consequences of pure Rust refinement.
  provider_classifications = [
    {
      package = "anyhow",
      kind = "infrastructure",
      permitted_caller_roles = ["composition-root"],
      edge_kinds = ["normal"],
      rationale = "Observed CLI error/context reporting may consult ambient backtrace settings and capture host backtraces. Role-granular normal permission is not domain-core use or an audit of transitive/native implementation.",
    },
    {
      package = "blake3",
      kind = "pure-value",
      permitted_caller_roles = ["composition-root"],
      edge_kinds = ["normal"],
      rationale = "Observed CLI and ngraph-experiment byte-digest computation. Digest implementation and any optimized or native code retain separate unproved safety assumptions.",
    },
    {
      package = "libc",
      kind = "infrastructure",
      permitted_caller_roles = ["composition-root"],
      edge_kinds = ["normal"],
      rationale = "Observed CLI use of libc open flags for no-follow path walking via Rust OpenOptions; this static edge permission does not establish FFI or native safety.",
    },
    {
      package = "rustc_apfloat",
      kind = "pure-value",
      permitted_caller_roles = ["composition-root"],
      edge_kinds = ["normal"],
      rationale = "Observed ngraph-experiment software IEEE binary32 arithmetic used as an independent comparison model; agreement with it is not a proof of the binary32 model or of the noble-ngraph evaluators.",
    },
    {
      package = "serde_json",
      kind = "pure-value",
      permitted_caller_roles = ["composition-root"],
      edge_kinds = ["normal"],
      rationale = "Observed CLI and ngraph-experiment JSON value parsing and serialization; external input remains untrusted and this is not a semantic acceptance witness.",
    },
    {
      package = "wasmparser",
      kind = "pure-value",
      permitted_caller_roles = ["composition-root"],
      edge_kinds = ["normal"],
      rationale = "Observed CLI Wasm byte parsing; parser output is not proof of engine, guest, or native safety.",
    },
    {
      package = "wasmtime",
      kind = "infrastructure",
      permitted_caller_roles = ["composition-root"],
      edge_kinds = ["normal"],
      rationale = "Observed CLI guest engine boundary; engine, trap, linked C helper, and transitive native implementation remain explicit external host assumptions.",
    },
    {
      package = "wat",
      kind = "pure-value",
      permitted_caller_roles = ["composition-root"],
      edge_kinds = ["normal"],
      rationale = "Observed CLI WAT byte parsing; parser output does not certify guest semantics or native safety.",
    },
  ],
  ports = [],
  capability_classifications = [
    {
      family = "filesystem",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "In the compiler path, only the CLI reads bounded source, contract, proof, companion-session, WIT, rule-library and toolchain inputs, creates fresh artifacts, and removes its private temporary workspace. Proof-required builds release only after exact source/claim-bound admission. Component builds emit only independently accepted typed exports for one selected WIT world. Owned Wasm/JS assets are separately byte-bound by tool selection. The CLI's opt-in noble ngraph entry reads only the named graph, vector, data, snapshot and Core source files under byte bounds, and publishes each output by creating <out>.partial with create-new, hard-linking it to a fresh <out> and removing that partial file; it never truncates, renames over or modifies an existing path. Outside that path, the issue #14 noble-ngraph-experiment harness reads its own inputs and host identity files, writes its measurement receipts, and creates and removes one temporary file.",
    },
    {
      family = "process",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "In the compiler path, only the CLI resolves the local Lean runtime closure, launches isolated proof workers, runs the pinned resource-free Core-Bootstrap/MC2 Node helper, and assembles the pinned synchronous component ABI with bounded wasm-tools subprocesses. Proof workers require bubblewrap and a user-systemd scope; missing proof isolation fails closed. The core consumes immutable exact independent-check observations and never invokes a host callback. Checker soundness and observation authenticity remain trusted premises; a process exit or producer report is not evidence admission. The trusted Node/Wasm tool subprocess boundary is separate and is not claimed to have the proof sandbox. Outside that path, the noble-ngraph-experiment harness runs the selected Node oracle, sha256sum, /bin/sh and a selected Noble CLI binary for its measurement legs; their output is measurement data, not evidence admission.",
    },
    {
      family = "environment",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "In the compiler path, only the CLI captures process arguments, consumer-selected absolute tool and rule-library paths, the user runtime directory, and the temporary-directory root. Child environments are cleared and explicitly populated. Outside that path, the noble-ngraph-experiment harness reads its process arguments and the temporary-directory root.",
    },
    {
      family = "time",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "In the compiler path, only the CLI observes clocks for bounded proof/runtime-worker supervision and private temporary-directory naming; clock values never enter Noble or contract meaning. Outside that path, the noble-ngraph-experiment harness reads monotonic clocks to time measurement legs; timings never enter graph semantics.",
    },
    {
      family = "concurrency",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "In the compiler path, only the CLI starts and joins bounded subprocess readers, receives bounded runtime replies and sleeps while supervising workers. Domain-core evaluation remains synchronous and effect-free. Outside that path, the noble-ngraph-experiment harness starts and joins stdin-writer threads for its subprocesses.",
    },
    {
      family = "observability",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "In the compiler path, only the CLI emits execution/verification/build/companion reports, help, internal smoke outcomes and diagnostics. Explanation and statement generation do not launch proof tools. Resource-free guest test requests, inspection results, captured traces and seven-outcome text are observations, not proof acceptance or protected capabilities. The noble ngraph entry prints noble-ngraph-report/v1 JSON lines and usage diagnostics; a report is not a proof of the graph, evaluator or snapshot. Outside that path, the noble-ngraph-experiment harness prints measurement reports and diagnostics.",
    },
    {
      family = "unsafe-code",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "All Noble-owned Rust forbids unsafe code. No unsafe executor exists.",
    },
  ],
  outbound_effects = [
    {
      family = "filesystem",
      executor_scope = "crates/noble-cli/src",
      executor_methods = [
        "noble-cli::closure@crates/noble-cli/src/ngraph/files.rs:3611:3615",
        "noble-cli::closure@crates/noble-cli/src/ngraph/files.rs:3684:3688",
        "noble-cli::closure@crates/noble-cli/src/sandbox/runtime.rs:3086:3094",
        "noble-cli::closure@crates/noble-cli/src/sandbox/runtime.rs:467:478",
        "noble-cli::closure@crates/noble-cli/src/sandbox/runtime.rs:4761:4767",
        "noble-cli::closure@crates/noble-cli/src/workflow/artifacts.rs:4597:4603",
        "noble-cli::closure@crates/noble-cli/src/workflow/rules.rs:6716:6723",
        "noble::<workflow::artifacts::Temporary as std::ops::Drop>::drop",
        "noble::build::artifacts::emit",
        "noble::component::artifacts::emit",
        "noble::core::assembly::assemble_wat",
        "noble::core::companions::controls::checker::LeanConsumer::run",
        "noble::core::companions::controls::exercise",
        "noble::core::companions::entry::read_bounded",
        "noble::core::entry::execute",
        "noble::core::framing::read_file",
        "noble::core::framing::write_new",
        "noble::core::worker::configuration::build",
        "noble::ngraph::files::publish",
        "noble::ngraph::files::read",
        "noble::sandbox::Environment::discover",
        "noble::sandbox::runtime::executable",
        "noble::sandbox::runtime::installation",
        "noble::workflow::artifacts::Temporary::create",
        "noble::workflow::artifacts::create_directory",
        "noble::workflow::artifacts::emit",
        "noble::workflow::artifacts::read_bounded",
        "noble::workflow::artifacts::write_source",
        "noble::workflow::rules::enqueue_directory",
        "noble::workflow::rules::read_library",
        "noble::workflow::rules::selected_root",
      ],
      authority = "unprotected",
      rationale = "Local consumer I/O reads selected regular files and rule-library sources, creates only fresh output files/directories, and deletes only owned temporary artifacts. noble ngraph publication creates <out>.partial with create-new, hard-links it to a fresh <out> and removes only that partial file. Path metadata observations are included even where the bounded collector does not classify std::path calls. No protected domain operation or artifact authenticity follows from this classification.",
    },
    {
      family = "process",
      executor_scope = "crates/noble-cli/src",
      executor_methods = [
        "noble-cli::closure@crates/noble-cli/src/sandbox/launch.rs:3118:3124",
        "noble-cli::closure@crates/noble-cli/src/sandbox/launch.rs:3228:3235",
        "noble::component::artifacts::invoke",
        "noble::core::companions::controls::checker::LeanConsumer::observation",
        "noble::core::companions::controls::checker::LeanConsumer::run",
        "noble::core::worker::launch",
        "noble::core::worker::process::observe",
        "noble::core::worker::process::stop",
        "noble::sandbox::launch::<impl sandbox::Environment>::run",
        "noble::sandbox::launch::<impl sandbox::Environment>::scoped_command",
        "noble::sandbox::process::capture",
        "noble::sandbox::process::monitor",
        "noble::sandbox::process::stop",
        "noble::sandbox::runtime::nix_roots",
        "noble::workflow::artifacts::Temporary::create",
        "noble::workflow::verification::probe_toolchain",
        "noble::workflow::verification::require_success",
      ],
      authority = "unprotected",
      rationale = "The shell queries the selected local Nix runtime closure, executes proof workers through their required isolation boundary, starts/reaps the immutable Node helper for independently accepted Core-Bootstrap/MC2 Wasm, and runs the selected synchronous component assembler. It supervises bounded replies and deadlines and observes child status. The separately pinned assembler/optimizer and owned JS/WAT are not proven or proof-sandboxed. Guest subject observations are re-bound by the deterministic companion core; reports, handles, wrapper construction and successful execution cannot mint accepted evidence. None of these operations grants a protected guest capability, and missing proof isolation never authorizes a proof-worker fallback.",
    },
    {
      family = "environment",
      executor_scope = "crates/noble-cli/src",
      executor_methods = [
        "noble::core::companions::controls::exercise",
        "noble::read_arguments",
        "noble::sandbox::configured_path",
        "noble::workflow::artifacts::Temporary::create",
      ],
      authority = "unprotected",
      rationale = "Arguments, explicit consumer tool/library selections, the user runtime directory and temporary root are boundary configuration, not domain authority. The no_std core receives explicit source bytes and limits instead of ambient environment access.",
    },
    {
      family = "time",
      executor_scope = "crates/noble-cli/src",
      executor_methods = [
        "noble::sandbox::now",
        "noble::workflow::artifacts::Temporary::create",
      ],
      authority = "unprotected",
      rationale = "Clock observations enforce worker supervision deadlines and name fresh private temporary directories. They neither determine language/contract semantics nor authorize protected operations.",
    },
    {
      family = "concurrency",
      executor_scope = "crates/noble-cli/src",
      executor_methods = [
        "noble::core::worker::Engine::close",
        "noble::core::worker::Engine::finish",
        "noble::core::worker::Engine::reply",
        "noble::core::worker::Engine::start",
        "noble::core::worker::process::observe",
        "noble::sandbox::process::capture",
        "noble::sandbox::process::monitor",
      ],
      authority = "unprotected",
      rationale = "The proof shell starts and joins bounded subprocess-output readers. The runtime shell owns one reader with a bounded reply channel, joins it after child reaping, and receives replies with deadlines. Polling sleeps and local supervision threads grant no protected domain capability.",
    },
    {
      family = "observability",
      executor_scope = "crates/noble-cli/src",
      executor_methods = [
        "noble::<core::worker::Engine as std::ops::Drop>::drop",
        "noble::backend::execute",
        "noble::backend::reject_owner_slot",
        "noble::backend::run",
        "noble::build::print_report",
        "noble::component::run",
        "noble::core::companions::controls::exercise",
        "noble::core::companions::entry::run",
        "noble::core::companions::reporting::emit",
        "noble::core::compilation::compile",
        "noble::core::output::print",
        "noble::core::entry::run",
        "noble::main",
        "noble::ngraph::run",
        "noble::workflow::print_report",
        "noble::write_outcome",
      ],
      authority = "unprotected",
      rationale = "Consumer-facing execution, verification, build, companion-session and noble-ngraph-report/v1 reports, help and smoke diagnostics write only to the CLI output streams. Output is not an authorization witness, an external-success receipt, accepted evidence, or permission to execute a protected domain effect.",
    },
  ],
  core_no_std = [{ scope = "noble-kernel" }, { scope = "noble-contracts" }, { scope = "noble-wasm" }, { scope = "noble-ngraph" }],
  admission = null,
  waivers = [],
  collection_limits = {
    # The pinned offline all-targets/all-features Cargo plan selected exactly 54
    # workspace units before Calculator-v1; its two noble-wasm integration test
    # targets (calculator_collect, calculator_profile) make exactly 56. Earlier
    # acyclic controls on a different source also selected 56, but the fixture's
    # duplicate Cargo dev-dependencies table cannot establish extra units here.
    # The issue #22 integration adds noble-cli tests/calculator_engine.rs (#17)
    # and noble-wasm tests/calculator_numeric_paths.rs (#20); a scratch
    # collection of integration eccbc02 observed exactly 58 complete units.
    # Issue #14 adds the noble-ngraph-experiment production package: lib,
    # lib+test, four bins and their bin+test units, and ten integration tests.
    # A scratch collection of 2d03e99 with this package policy observed
    # exactly 78 complete units; the package's largest shard (lib+test, 7,300 calls,
    # 12,441,948 bytes) and the 163,306,150-byte IR fit the existing caps.
    # Numeric-Graph-v1 adds the domain-core noble-ngraph package (lib,
    # lib+test and seven integration tests) and noble-cli tests/ngraph.rs; the
    # experiment is net zero (ir_check and extract_b1 moved to the library,
    # format_roundtrip and identity_agreement added). A scratch collection of
    # the merged policy (caps raised only in the scratch copy) observed exactly
    # 88 complete units. Its largest shard, the CLI noble bin+test, had 21,870
    # calls, 19,133 constructors, 24,049 spans and 32,741,848 bytes, and the
    # IR was 177,843,955 bytes, so every other cap is unchanged.
    # This exact capacity is not a classification waiver, source/native
    # assurance approval, or a gate PASS.
    max_units = 88,
    max_shards = 88,
    # M7's kernel dataspace module pushes both kernel library shards over the
    # earlier 1024 module-fact cap. Preserve complete compiler coverage.
    max_modules = 2048,
    max_items = 4096,
    max_types = 4096,
    max_traits = 512,
    max_implementations = 1024,
    max_signatures = 4096,
    # Calculator-v1 (issue #22 collection of 44329dc + fmt/clippy): both CLI
    # noble bin and bin+test shards reached the 16,384 call cap, and the bin
    # shard also reached the 16,384 constructor cap, so both were incomplete.
    # Double both bounds for one complete re-collection; truncation is never
    # an inventory.
    max_calls = 32768,
    max_constructors = 32768,
    max_effects = 4096,
    max_expansions = 8192,
    # M5's frontend saturates the previous 1024 compiler-observed attribute/
    # configuration facts per unit. Retain complete collection without widening
    # the single declared target/feature configuration.
    max_configurations = 2048,
    max_type_shapes = 4096,
    max_function_safety_facts = 4096,
    max_value_provenance_facts = 8192,
    max_value_flow_bodies = 4096,
    max_value_flow_steps = 65536,
    max_value_flow_rounds = 64,
    max_unknowns = 4096,
    # This candidate's sealed-source CLI bin+test shard reached the 16,384-span
    # cap and omitted at least 541 distinct referenced spans (minimum 16,925).
    # Permit one bounded re-collection; the incomplete shard is not an inventory.
    max_spans = 32768,
    max_span_bytes = 1048576,
    # The truncated issue #22 CLI shards were already 27,781,842 and
    # 26,658,831 bytes, so complete shards cannot fit 32 MiB. Double the
    # independent shard and artifact byte caps with the call caps above.
    max_shard_bytes = 67108864,
    max_artifact_bytes = 268435456,
  },
  non_claims = [
    "complete-fcis-proof",
    "complete-hexagonal-proof",
    "runtime-determinism",
    "whole-program-correctness",
    "release-eligibility",
  ],
} | contracts.ProjectArchitecturePolicy | contracts.ProjectArchitecturePolicyRules
