# Test configuration, not a runtime receipt or an acceptance authority.
let Case = {
  id | String,
  base | [| 'workspace, 'acyclic_shell |],
  mutation | [| 'none, 'body, 'randomness, 'randomness_dependency, 'host_return, 'unsafe_function, 'unsafe_foreign_declaration, 'no_std_test_witness, 'test_source_import, 'reverse_dependency |],
  statement | String,
  needs_std | Bool,
  deny_lint | String,
  deny_source | String | default = "",
  observation | [| 'clean, 'effect, 'call, 'type, 'compiler_only, 'role_edge, 'policy, 'precollection |],
  expected | String,
  edge_message | String | default = "",
  expected_message | String | default = "",
  expected_scope | String | default = "",
  expected_secondary | String | default = "",
} in
let case = fun case_id mutation_value statement_value std_value lint_value observation_value expected_value => {
  id = case_id, base = 'workspace, mutation = mutation_value, statement = statement_value,
  needs_std = std_value, deny_lint = lint_value, observation = observation_value, expected = expected_value,
} | Case in
{
  schema_version | String = "noble-boundary-controls/v1",
  owner | String = "noble-maintainers",
  timeout_seconds | Number = 180,
  target | String = "x86_64-unknown-linux-gnu",
  kernel_source | String = "crates/noble-kernel/src/lib.rs",
  randomness_dependency = {
    name | String = "getrandom",
    version | String = "0.4.3",
    lockfile | String = "verification/fixtures/randomness.Cargo.lock",
  },
  # Production source that imports test-labelled source by a parent-relative path.
  test_source = {
    path | String = "crates/noble-kernel/tests/support/production_import.rs",
    relative | String = "../tests/support/production_import.rs",
    escaped | String = "crates/noble-kernel/src/../tests/support/production_import.rs",
    module | String = "production_import",
  },
  units | Array String = [
    "noble-kernel:lib:crates/noble-kernel/src/lib.rs:domain-core",
    "noble-kernel:lib+test:crates/noble-kernel/src/lib.rs:domain-core",
    "noble-kernel:test+test:crates/noble-kernel/tests/budget.rs:test",
    "noble-cli:bin:crates/noble-cli/src/main.rs:composition-root",
    "noble-cli:bin+test:crates/noble-cli/src/main.rs:composition-root",
    "noble-cli:test+test:crates/noble-cli/tests/smoke.rs:test",
    "noble-cli:test+test:crates/noble-cli/tests/core.rs:test",
    "noble-cli:test+test:crates/noble-cli/tests/build.rs:test",
    "noble-contracts:lib:crates/noble-contracts/src/lib.rs:domain-core",
    "noble-contracts:lib+test:crates/noble-contracts/src/lib.rs:domain-core",
    "noble-contracts:test+test:crates/noble-contracts/tests/contracts.rs:test",
    "noble-contracts:test+test:crates/noble-contracts/tests/source.rs:test",
    "noble-contracts:test+test:crates/noble-contracts/tests/companion.rs:test",
    "noble-contracts:test+test:crates/noble-contracts/tests/component.rs:test",
    "noble-kernel:test+test:crates/noble-kernel/tests/acceptance.rs:test",
    "noble-kernel:test+test:crates/noble-kernel/tests/docexamples.rs:test",
    "noble-kernel:test+test:crates/noble-kernel/tests/dx01.rs:test",
    "noble-kernel:test+test:crates/noble-kernel/tests/fragment.rs:test",
    "noble-kernel:test+test:crates/noble-kernel/tests/m5-authority.rs:test",
    "noble-kernel:test+test:crates/noble-kernel/tests/m5-resources.rs:test",
    "noble-kernel:test+test:crates/noble-kernel/tests/m6-async.rs:test",
    "noble-kernel:test+test:crates/noble-kernel/tests/property.rs:test",
    "noble-kernel:test+test:crates/noble-kernel/tests/recursion-schema-cycle.rs:test",
    "noble-wasm:lib:crates/noble-wasm/src/lib.rs:domain-core",
    "noble-wasm:lib+test:crates/noble-wasm/src/lib.rs:domain-core",
  ],
  acyclic_extra_units | Array String = [
    "noble-cli:lib:crates/noble-cli/src/lib.rs:composition-root",
    "noble-cli:lib+test:crates/noble-cli/src/lib.rs:composition-root",
  ],
  body_anchor | String = "    match remaining.checked_sub(1) {",
  cases | Array Case = [
    case "workspace" 'none "" false "" 'clean "",
    (case "randomness-dependency" 'randomness_dependency "" false "" 'role_edge "forbidden-role-edge") & {
      edge_message | force = "package `noble-kernel` with role `DomainCore` has forbidden normal dependency on unclassified external provider `getrandom`",
    },
    (case "acyclic-shell" 'none "" false "" 'clean "") & { base | force = 'acyclic_shell },
    case "filesystem" 'body "let _ = std::fs::read(\"boundary-never-executed\");" true "impure_call_in_core" 'effect "filesystem",
    case "network" 'body "let _ = std::net::TcpStream::connect(\"127.0.0.1:9\");" true "impure_call_in_core" 'effect "network",
    case "process" 'body "let _ = std::process::Command::new(\"boundary-never-executed\");" true "impure_call_in_core" 'effect "process",
    case "environment" 'body "let _ = std::env::var_os(\"NOBLE_BOUNDARY_FIXTURE\");" true "ambient_env" 'effect "environment",
    case "clock" 'body "let _ = std::time::SystemTime::now();" true "ambient_clock" 'effect "time",
    case "randomness" 'randomness "let mut bytes = [0_u8; 4]; let _ = getrandom::fill(&mut bytes);" false "ambient_random" 'effect "randomness",
    case "global-state" 'body m%"
      static HIDDEN: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
          let _ = HIDDEN.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    "% false "global_state_in_core" 'call "core::sync::atomic::Atomic::<u32>::fetch_add",
    case "interior-state" 'body m%"
      std::thread_local! { static HIDDEN: core::cell::Cell<u32> = const { core::cell::Cell::new(0) }; }
          HIDDEN.with(|value| value.set(remaining));
    "% true "interior_mutability_in_core" 'call "core::cell::Cell::<T>::set",
    case "unsafe-block" 'body "let _ = unsafe { core::ptr::read(&remaining) };" false "unsafe_code" 'compiler_only "",
    case "unsafe-function" 'unsafe_function "" false "unsafe_code" 'compiler_only "",
    case "unsafe-foreign-declaration" 'unsafe_foreign_declaration "" false "unsafe_code" 'compiler_only "",
    case "host-return" 'host_return "" true "resource_return_from_core" 'type "std::fs::File",
    case "test-source-import" 'test_source_import "" false "" 'precollection "invalid Cargo diagnostic path",
    (case "no-std-test-witness" 'no_std_test_witness "" false "fallible_int_fallback" 'policy "no-std-obligation") & {
      deny_source | force = "crates/noble-kernel/src/acceptance/mod.rs",
      expected_message | force = "lacks compiler-confirmed crate:no-std",
      expected_scope | force = "noble-kernel",
    },
    (case "reverse-dependency" 'reverse_dependency "" false "" 'role_edge "forbidden-role-edge") & { base | force = 'acyclic_shell },
  ],
  non_claims | Array String = [
    "not-complete-boundary-matrix",
    "not-source-inventory",
    "auxiliary-default-lints-not-deny-all-acceptance",
    "fixture-effects-never-executed",
    "not-refinement-or-m1-acceptance",
  ],
}
