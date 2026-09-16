# Synthetic comparator controls. These are not compiler evidence.
{ }:
let
  inherit (builtins) head;
  unit =
    id: package: kind: path: role:
    {
      inherit id;
      unit_id = id;
      package_name = package;
      target_kind = kind;
      source_path = path;
      role = role;
      target_triple = "x86_64-unknown-linux-gnu";
      features = [ ];
      default_features = true;
    };
  units = [
    (unit "k" "noble-kernel" "lib" "crates/noble-kernel/src/lib.rs" "domain-core")
    (unit "c" "noble-cli" "bin" "crates/noble-cli/src/main.rs" "composition-root")
  ];
  inventory = {
    schema = "noble-source-inventory/v1";
    coverage = {
      status = "complete";
      expected_units = 2;
      observed_units = 2;
      missing_units = [ ];
      unexpected_units = [ ];
      duplicate_units = [ ];
    };
    inherit units;
    configurations = [
      {
        target = "x86_64-unknown-linux-gnu";
        features = [ ];
        default_features = true;
      }
    ];
    production_subjects = [
      {
        qualified_path = "noble_kernel::consume_budget";
        item_kind = "function";
        origin_crate = "noble_kernel";
        visibility = "public";
        units = [ "k" ];
      }
      {
        qualified_path = "noble_cli::main";
        item_kind = "function";
        origin_crate = "noble_cli";
        visibility = "private";
        units = [ "c" ];
      }
    ];
    test_subjects = [
      {
        qualified_path = "budget::test";
        item_kind = "function";
        origin_crate = "noble-kernel";
        visibility = "restricted";
        units = [ "k" ];
      }
    ];
    macro_origins = [
      "<builtin>"
      "std::assert"
    ];
    dependencies = {
      selected_packages = [
        "noble-cli"
        "noble-kernel"
      ];
      production_edges = [
        {
          caller_package = "noble-cli";
          callee_package = "noble-kernel";
          edge_kind = "normal";
        }
      ];
      external_edges = [ ];
    };
    tools = [
      "nickel"
      "rust"
    ];
    future_components = [ "wasmtime" ];
    counts = { };
  };
  policy = {
    schema_version = "noble-source-inventory-policy/v1";
    subjects = [
      {
        qualified_path = "noble_kernel::consume_budget";
        package = "noble-kernel";
        category = "body";
        disposition = "open";
        refinement = "open";
        note = "integrated extraction gate is task 3.1";
      }
      {
        qualified_path = "noble_cli::main";
        package = "noble-cli";
        category = "body";
        disposition = "open";
        refinement = "open";
        note = "shell body is not an extraction subject";
      }
    ];
    test_subjects = [ "budget::test" ];
    macro_origins = [
      "<builtin>"
      "std::assert"
    ];
    dependencies = [
      {
        caller = "noble-cli";
        callee = "noble-kernel";
        edge_kind = "normal";
      }
    ];
    tools = [
      {
        name = "nickel";
        category = "policy-tool";
      }
      {
        name = "rust";
        category = "rust-toolchain";
      }
    ];
    future_components = [
      {
        name = "wasmtime";
        status = "open";
      }
    ];
    exceptions = [ ];
  };
  architecturePolicy = {
    production = {
      packages = [
        "noble-kernel"
        "noble-cli"
      ];
      source_scopes = [
        "crates/noble-kernel/src"
        "crates/noble-cli/src"
      ];
      targets = [
        {
          triple = "x86_64-unknown-linux-gnu";
          features = [ ];
          default_features = true;
        }
      ];
    };
  };
  evaluate =
    inv: pol: arch:
    import ./source-inventory.nix {
      inventory = inv;
      policy = pol;
      architecturePolicy = arch;
      selection = { };
    };
  evaluateDefault = inv: pol: evaluate inv pol architecturePolicy;
  reject =
    name: inv: pol: expected:
    let
      result = evaluateDefault inv pol;
    in
    if result.valid then
      throw "control unexpectedly passed: ${name}"
    else if builtins.elem expected result.diagnostics then
      name
    else
      throw "wrong diagnostic for ${name}: ${builtins.toJSON result.diagnostics}";
  rejectArchitecture =
    name: arch: expected:
    let
      result = evaluate inventory policy arch;
    in
    if result.valid then
      throw "control unexpectedly passed: ${name}"
    else if builtins.elem expected result.diagnostics then
      name
    else
      throw "wrong diagnostic for ${name}: ${builtins.toJSON result.diagnostics}";
  withPolicy = change: policy // change;
  withInventory = change: inventory // change;
  replaceFirst = change: list: [ (head list // change) ] ++ builtins.tail list;
  subjects = policy.subjects;
  tests = [
    (
      assert (evaluateDefault inventory policy).valid;
      "complete reviewed inventory matches compiler-derived data"
    )
    (
      assert (evaluateDefault inventory policy).counts.bodies == 2;
      "partition reports the compiler-derived production total"
    )
    (
      assert (evaluateDefault inventory policy).counts.open == 2;
      "open work is reported separately from verification"
    )
    (
      assert (evaluateDefault inventory policy).counts.proved == 0;
      "no refinement proof is implied by the classification"
    )
    (reject "unreviewed compiler subject" (withInventory {
      production_subjects = inventory.production_subjects ++ [
        {
          qualified_path = "noble_cli::unreviewed";
          item_kind = "function";
          origin_crate = "noble_cli";
          visibility = "private";
          units = [ "c" ];
        }
      ];
    }) policy "subject-omission")
    (reject "stale reviewed subject" (withInventory {
      production_subjects = builtins.tail inventory.production_subjects;
    }) policy "subject-stale")
    (reject "package binding mismatch" inventory (withPolicy {
      subjects = replaceFirst { package = "noble-cli"; } subjects;
    }) "subject-binding")
    (reject "unclassified test subject" inventory (withPolicy { test_subjects = [ ]; }) "test-subject-classification")
    (reject "unreviewed macro body" inventory (withPolicy { macro_origins = [ "<builtin>" ]; }) "macro-classification")
    (reject "unclassified dependency" inventory (withPolicy { dependencies = [ ]; }) "dependency-closure")
    (reject "wrong dependency kind" inventory (withPolicy {
      dependencies = [ ((head policy.dependencies) // { edge_kind = "build"; }) ];
    }) "dependency-closure")
    (reject "unclassified tool" inventory (withPolicy { tools = [ (head policy.tools) ]; }) "tool-classification")
    (reject "incomplete compiler coverage" (withInventory {
      coverage = inventory.coverage // { status = "incomplete"; };
    }) policy "coverage-status")
    (reject "missing compiler unit" (withInventory {
      coverage = inventory.coverage // { missing_units = [ "k" ]; };
    }) policy "coverage-missing-units")
    (reject "unexpected compiler unit" (withInventory {
      coverage = inventory.coverage // { unexpected_units = [ "x" ]; };
    }) policy "coverage-unexpected-units")
    (reject "duplicate compiler unit" (withInventory {
      coverage = inventory.coverage // { duplicate_units = [ "k" ]; };
    }) policy "coverage-duplicate-units")
    (reject "unit count mismatch" (withInventory {
      coverage = inventory.coverage // { observed_units = 1; };
    }) policy "coverage-unit-count")
    (rejectArchitecture "changed required target" (architecturePolicy // {
      production = architecturePolicy.production // {
        targets = [
          {
            triple = "aarch64-unknown-linux-gnu";
            features = [ ];
            default_features = true;
          }
        ];
      };
    }) "required-configuration")
    (rejectArchitecture "changed required features" (architecturePolicy // {
      production = architecturePolicy.production // {
        targets = [
          {
            triple = "x86_64-unknown-linux-gnu";
            features = [ "extra" ];
            default_features = true;
          }
        ];
      };
    }) "required-configuration")
    (rejectArchitecture "changed default-feature state" (architecturePolicy // {
      production = architecturePolicy.production // {
        targets = [
          {
            triple = "x86_64-unknown-linux-gnu";
            features = [ ];
            default_features = false;
          }
        ];
      };
    }) "required-configuration")
    (rejectArchitecture "undeclared source scope" (architecturePolicy // {
      production = architecturePolicy.production // {
        source_scopes = [
          "crates/noble-kernel/src"
          "crates/noble-cli/src"
          "crates/noble-other/src"
        ];
      };
    }) "source-scope-coverage")
    (reject "undeclared package coverage" (withInventory {
      units = builtins.tail inventory.units;
    }) policy "package-coverage")
    (reject "invalid disposition" inventory (withPolicy {
      subjects = replaceFirst { disposition = "verified"; } subjects;
    }) "subject-disposition")
    (reject "invalid refinement status" inventory (withPolicy {
      subjects = replaceFirst { refinement = "proven"; } subjects;
    }) "subject-refinement")
    (reject "excepted subject without record" inventory (withPolicy {
      subjects = replaceFirst { disposition = "excepted"; } subjects;
    }) "excepted-subject-without-record")
    (reject "kernel exception rejected" inventory (withPolicy {
      subjects = replaceFirst { disposition = "excepted"; } subjects;
      exceptions = [
        {
          symbol = "noble_kernel::consume_budget";
          package = "noble-kernel";
          source_identity = "b3:aa";
          owner = "noble-maintainers";
          constraint = "tool-diagnostic";
          contract = "budget transition";
          assumptions = "safe sequential Rust";
          evidence = "probe";
          reassessment = "task 3.1";
        }
      ];
    }) "kernel-exception-rejected")
    (reject "incomplete exception record" inventory (withPolicy {
      exceptions = [
        {
          symbol = "shell::adapter";
          package = "noble-cli";
          source_identity = "b3:aa";
          owner = "noble-maintainers";
          constraint = "";
          contract = "adapter";
          assumptions = "";
          evidence = "";
          reassessment = "";
        }
      ];
    }) "exception-incomplete")
    (reject "future component claiming verification" inventory (withPolicy {
      future_components = [
        {
          name = "wasmtime";
          status = "verified";
        }
      ];
    }) "future-component-claim")
    (reject "future component in the production subject list" inventory (withPolicy {
      future_components = [
        {
          name = "noble_cli::main";
          status = "open";
        }
      ];
    }) "future-component-claim")
  ];
in
builtins.deepSeq tests tests
