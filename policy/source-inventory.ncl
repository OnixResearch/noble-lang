# Reviewed classification policy for the compiler-derived source inventory.
# Configuration and expected classification, not an execution receipt.
# Reviewed on 2026-09-15 against the compiler-derived subject set of the M1
# workspace at tree 3abd178bda9543155c545fdacf6b07f3fd5bbc49.
# Renewal is an explicit reviewed action; the build never refreshes this file.
let Subject = {
  qualified_path | String,
  package | String,
  category | [| 'body, 'generated, 'structural |],
  disposition | String,
  refinement | String,
  note | String,
} in
let
  body = fun qualified package_name text => {
    qualified_path = qualified,
    package = package_name,
    category = 'body,
    disposition = "open",
    refinement = "open",
    note = text,
  } | Subject in
let
  generated = fun qualified package_name text => {
    qualified_path = qualified,
    package = package_name,
    category = 'generated,
    disposition = "generated",
    refinement = "not-applicable",
    note = text,
  } | Subject in
let
  structural = fun qualified package_name text => {
    qualified_path = qualified,
    package = package_name,
    category = 'structural,
    disposition = "structural",
    refinement = "not-applicable",
    note = text,
  } | Subject in
{
  schema_version | String = "noble-source-inventory-policy/v1",
  owner | String = "noble-maintainers",
  scope_note | String = "Named M1 workspace scope only: the noble-kernel and noble-cli packages on the selected target. This is not a whole-project verification claim.",
  # Every compiler-derived production subject must appear here exactly once,
  # and every entry here must be observed by the compiler.
  subjects | Array Subject = [
    body "noble_kernel::consume_budget" "noble-kernel" "The M1 transition. Task 1.3 and 1.4 recorded a compatibility probe, but no source-bound integrated extraction gate exists yet, so this stays open work until task 3.1 binds it.",
    body "noble_cli::main" "noble-cli" "Shell entry point. The shell owns orchestration and effect execution; it is not an Aeneas extraction subject in M1.",
    body "noble_cli::read_arguments" "noble-cli" "Shell argument capture. The only approved ambient-environment allowance.",
    body "noble_cli::parse_budget" "noble-cli" "Shell input parsing with a typed error outcome.",
    body "noble_cli::write_outcome" "noble-cli" "Unprotected console effect executed by the composition root.",
    body "noble_cli::input_error_message" "noble-cli" "Shell diagnostic text for invalid input.",
    generated "noble_kernel::main" "noble-kernel" "Test-harness entry point generated for the library test unit. No Noble-authored source body.",
    generated "noble_kernel::main::test" "noble-kernel" "Generated test-harness item. No Noble-authored source body.",
    generated "noble_cli::main::test" "noble-cli" "Generated test-harness item. No Noble-authored source body.",
    generated "noble-cli::closure@crates/noble-cli/src/main.rs:1565:1571" "noble-cli" "Anonymous closure inside an accounted shell body. It has no independent obligation.",
    structural "noble_kernel::BudgetOutcome" "noble-kernel" "Kernel outcome type. A type declaration, not a body.",
    structural "noble_kernel::core" "noble-kernel" "Core prelude import marker.",
    structural "noble_kernel::{use#0}" "noble-kernel" "Import marker within the kernel root.",
    structural "noble_cli::InputError" "noble-cli" "Shell error type. A type declaration, not a body.",
    structural "noble_cli::std" "noble-cli" "Standard-library import marker.",
    structural "noble_cli::{use#0}" "noble-cli" "Import marker within the shell root.",
  ],
  # Compiler-derived test-labelled subjects. They keep the test role and cannot
  # discharge a production obligation.
  test_subjects | Array String = [
    "budget::main",
    "budget::main::test",
    "budget::positive_budget_decrements_once",
    "budget::std",
    "budget::test",
    "budget::zero_budget_is_exhausted",
    "budget::{use#0}",
    "noble-kernel::closure@crates/noble-kernel/tests/budget.rs:30:333",
    "noble-kernel::closure@crates/noble-kernel/tests/budget.rs:365:516",
    "noble-cli::closure@crates/noble-cli/tests/smoke.rs:30:634",
    "noble-cli::closure@crates/noble-cli/tests/smoke.rs:666:1756",
    "noble-cli::closure@crates/noble-cli/tests/smoke.rs:1883:2468",
    "smoke::main",
    "smoke::main::test",
    "smoke::shell_calls_the_real_transition",
    "smoke::shell_rejects_invalid_input_without_transition_output",
    "smoke::shell_rejects_non_utf8_budget",
    "smoke::shell_rejects_non_utf8_budget::INVALID_UTF8_BYTE",
    "smoke::std",
    "smoke::test",
    "smoke::{use#0}",
  ],
  # Every observed macro or generated expansion origin, including the builtin
  # expansion that has no definition path.
  macro_origins | Array String = [
    "<builtin>",
    "std::assert",
    "std::assert_eq",
    "std::eprintln",
    "std::format_args_nl",
    "std::prelude::v1::test",
    "std::println",
    "std::vec",
  ],
  # Reviewed production dependency closure. Noble has no third-party production
  # Cargo dependency; the only production edge is the shell-to-kernel edge.
  dependencies | Array { caller | String, callee | String, edge_kind | String } = [
    { caller = "noble-cli", callee = "noble-kernel", edge_kind = "normal" },
  ],
  # Non-Rust and toolchain components selected by the reviewed tool policy.
  tools | Array { name | String, category | String } = [
    { name = "aeneas", category = "proof-tool" },
    { name = "cairn", category = "lifecycle-tool" },
    { name = "charon", category = "proof-tool" },
    { name = "extraction_rust", category = "rust-toolchain" },
    { name = "lean", category = "proof-tool" },
    { name = "nickel", category = "policy-tool" },
    { name = "octet", category = "architecture-tool" },
    { name = "octet_standards", category = "architecture-tool" },
    { name = "quality_rust", category = "rust-toolchain" },
    { name = "upstream_pin_check", category = "compatibility-tool" },
  ],
  # Declared future components. They are open work and carry no verification.
  future_components | Array { name | String, status | String } = [
    { name = "Wasmtime", status = "open" },
    { name = "WIT", status = "open" },
    { name = "Verus", status = "open" },
    { name = "byte-view dependency", status = "open" },
  ],
  # VT-SCOPE-04 records. Kernel bodies can never use one.
  exceptions | Array {
    symbol | String,
    package | String,
    source_identity | String,
    owner | String,
    constraint | String,
    contract | String,
    assumptions | String,
    evidence | String,
    reassessment | String,
  } = [],
}
