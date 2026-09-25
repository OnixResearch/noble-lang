let contracts = import "./architecture.contracts.ncl" in
{
  schema_version = "octet-project-architecture-policy/v1",
  project_id = "noble",
  mode = "gate",
  production = {
    packages = ["noble-kernel", "noble-contracts", "noble-wasm", "noble-cli", "noble-syndicate"],
    source_scopes = ["crates/noble-kernel/src", "crates/noble-contracts/src", "crates/noble-wasm/src", "crates/noble-cli/src", "crates/noble-syndicate/src"],
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
    { scope_kind = "source", scope = "crates/noble-kernel/src", role = "domain-core" },
    { scope_kind = "source", scope = "crates/noble-contracts/src", role = "domain-core" },
    { scope_kind = "source", scope = "crates/noble-wasm/src", role = "domain-core" },
    { scope_kind = "source", scope = "crates/noble-cli/src", role = "composition-root" },
    { scope_kind = "source", scope = "crates/noble-syndicate/src", role = "composition-root" },
    { scope_kind = "source", scope = "crates/noble-kernel/tests", role = "test" },
    { scope_kind = "source", scope = "crates/noble-contracts/tests", role = "test" },
    { scope_kind = "source", scope = "crates/noble-wasm/tests", role = "test" },
    { scope_kind = "source", scope = "crates/noble-cli/tests", role = "test" },
    { scope_kind = "source", scope = "crates/noble-syndicate/tests", role = "test" },
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
  # The workspace has no third-party Cargo providers or application-owned ports.
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
  provider_classifications = [],
  ports = [],
  capability_classifications = [
    {
      family = "filesystem",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "Only the CLI reads bounded source, contract, proof, companion-session, WIT, rule-library and toolchain inputs, creates fresh artifacts, and removes its private temporary workspace. Proof-required builds release only after exact source/claim-bound admission. Component builds emit only independently accepted typed exports for one selected WIT world. Owned Wasm/JS assets are separately byte-bound by tool selection.",
    },
    {
      family = "process",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "Only the CLI resolves the local Lean runtime closure, launches isolated proof workers, runs the pinned resource-free Core-Bootstrap/MC2 Node helper, and assembles the pinned synchronous component ABI with bounded wasm-tools subprocesses. Proof workers require bubblewrap and a user-systemd scope; missing proof isolation fails closed. The core consumes immutable exact independent-check observations and never invokes a host callback. Checker soundness and observation authenticity remain trusted premises; a process exit or producer report is not evidence admission. The trusted Node/Wasm tool subprocess boundary is separate and is not claimed to have the proof sandbox.",
    },
    {
      family = "environment",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "Only the CLI captures process arguments, consumer-selected absolute tool and rule-library paths, the user runtime directory, and the temporary-directory root. Child environments are cleared and explicitly populated.",
    },
    {
      family = "time",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "Only the CLI observes clocks for bounded proof/runtime-worker supervision and private temporary-directory naming; clock values never enter Noble or contract meaning.",
    },
    {
      family = "concurrency",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "Only the CLI starts and joins bounded subprocess readers, receives bounded runtime replies and sleeps while supervising workers. Domain-core evaluation remains synchronous and effect-free.",
    },
    {
      family = "observability",
      core_handling = "forbidden",
      executor_role = "composition-root",
      rationale = "Only the CLI emits execution/verification/build/companion reports, help, internal smoke outcomes and diagnostics. Explanation and statement generation do not launch proof tools. Resource-free guest test requests, inspection results, captured traces and seven-outcome text are observations, not proof acceptance or protected capabilities.",
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
        "noble-cli::closure@crates/noble-cli/src/sandbox/runtime.rs:3086:3094",
        "noble-cli::closure@crates/noble-cli/src/sandbox/runtime.rs:467:478",
        "noble-cli::closure@crates/noble-cli/src/sandbox/runtime.rs:4761:4767",
        "noble-cli::closure@crates/noble-cli/src/workflow/artifacts.rs:4597:4603",
        "noble-cli::closure@crates/noble-cli/src/workflow/rules.rs:6716:6723",
        "noble::<workflow::artifacts::Temporary as std::ops::Drop>::drop",
        "noble::build::artifacts::emit",
        "noble::component::artifacts::emit",
        "noble::core::assemble_wat",
        "noble::core::companions::controls::checker::LeanConsumer::run",
        "noble::core::companions::controls::exercise",
        "noble::core::companions::entry::read_bounded",
        "noble::core::execute",
        "noble::core::framing::read_file",
        "noble::core::framing::write_new",
        "noble::core::worker::configuration",
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
      rationale = "Local consumer I/O reads selected regular files and rule-library sources, creates only fresh output files/directories, and deletes only owned temporary artifacts. Path metadata observations are included even where the bounded collector does not classify std::path calls. No protected domain operation or artifact authenticity follows from this classification.",
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
        "noble::core::compile",
        "noble::core::output::print",
        "noble::core::run",
        "noble::main",
        "noble::workflow::print_report",
        "noble::write_outcome",
      ],
      authority = "unprotected",
      rationale = "Consumer-facing execution, verification, build and companion-session reports, help and smoke diagnostics write only to the CLI output streams. Output is not an authorization witness, an external-success receipt, accepted evidence, or permission to execute a protected domain effect.",
    },
  ],
  core_no_std = [{ scope = "noble-kernel" }, { scope = "noble-contracts" }, { scope = "noble-wasm" }],
  admission = null,
  waivers = [],
  collection_limits = {
    # M7 has 29 real units (five packages); the acyclic boundary control adds
    # two shell-library units. Retain every target and test shard.
    max_units = 31,
    max_shards = 31,
    # M7's kernel dataspace module pushes both kernel library shards over the
    # earlier 1024 module-fact cap. Preserve complete compiler coverage.
    max_modules = 2048,
    max_items = 4096,
    max_types = 4096,
    max_traits = 512,
    max_implementations = 1024,
    max_signatures = 4096,
    max_calls = 8192,
    max_constructors = 4096,
    max_effects = 4096,
    max_expansions = 4096,
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
    max_spans = 8192,
    max_span_bytes = 1048576,
    max_shard_bytes = 16777216,
    max_artifact_bytes = 67108864,
  },
  non_claims = [
    "complete-fcis-proof",
    "complete-hexagonal-proof",
    "runtime-determinism",
    "whole-program-correctness",
    "release-eligibility",
  ],
} | contracts.ProjectArchitecturePolicy | contracts.ProjectArchitecturePolicyRules
