# Synthetic comparator controls. These are not compiler evidence.
{ comparator ? ./source-inventory.nix }:
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
    schema = "noble-source-inventory/v2";
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
        span_id = "kernel-body";
        from_expansion = false;
      }
      {
        qualified_path = "noble_cli::main";
        item_kind = "function";
        origin_crate = "noble_cli";
        visibility = "private";
        units = [ "c" ];
        span_id = "cli-body";
        from_expansion = false;
      }
    ];
    test_subjects = [
      {
        qualified_path = "budget::test";
        item_kind = "function";
        origin_crate = "noble-kernel";
        visibility = "restricted";
        units = [ "k" ];
        span_id = "test-body";
        from_expansion = false;
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
    import comparator {
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
  reviewed =
    path: category:
    {
      qualified_path = path;
      package = "noble-kernel";
      inherit category;
      disposition = if category == "body" then "open" else category;
      refinement = if category == "body" then "open" else "not-applicable";
      note = "synthetic source provenance control";
    };
  observed =
    path: span: expanded:
    {
      qualified_path = path;
      item_kind = "function";
      origin_crate = "noble_kernel";
      visibility = "private";
      units = [ "k" ];
      span_id = span;
      from_expansion = expanded;
    };
  moduleImpl = "noble_kernel::<Module as core::cmp::PartialEq>";
  moduleEq = "noble_kernel::<Module as core::cmp::PartialEq>::eq";
  moduleInventory = withInventory {
    production_subjects = inventory.production_subjects ++ [
      ((observed moduleImpl "handwritten-impl" false) // { item_kind = "implementation"; })
      (observed moduleEq "handwritten-eq" false)
    ];
  };
  modulePolicy = withPolicy {
    subjects = subjects ++ [ (reviewed moduleImpl "structural") (reviewed moduleEq "body") ];
  };
  derivePath = "noble_kernel::<Derived as core::clone::Clone>::clone";
  deriveInventory = withInventory {
    production_subjects = inventory.production_subjects ++ [
      (observed derivePath "derive-span" true)
    ];
  };
  derivePolicy = withPolicy {
    subjects = subjects ++ [ (reviewed derivePath "generated") ];
  };
  mixedPath = "noble_kernel::test_mixed";
  mixedInventory = withInventory {
    production_subjects = inventory.production_subjects ++ [
      (observed mixedPath "authored-test" false)
      (observed mixedPath "test-registration" true)
    ];
  };
  mixedExpansionFirst = withInventory {
    production_subjects = inventory.production_subjects ++ [
      (observed mixedPath "test-registration" true)
      (observed mixedPath "authored-test" false)
    ];
  };
  mixedBodyPolicy = withPolicy { subjects = subjects ++ [ (reviewed mixedPath "body") ]; };
  mixedGeneratedPolicy = withPolicy { subjects = subjects ++ [ (reviewed mixedPath "generated") ]; };
  derive = import ./source-inventory-derive.nix;
  derivationIR = {
    architecture_ir_id = "synthetic-ir";
    policy_blake3 = "synthetic-policy";
    units = map (u: u // { toolchain_blake3 = "synthetic-toolchain"; }) units;
    unit_memberships = [
      { unit_id = "k"; fact_ids = [ "authored" ]; }
      { unit_id = "c"; fact_ids = [ "expanded" ]; }
    ];
    spans = [
      { span_id = "source-span"; from_expansion = false; }
      { span_id = "derive-span"; from_expansion = true; }
    ];
    facts = [
      {
        kind = "item";
        fact_id = "authored";
        qualified_path = mixedPath;
        item_kind = "function";
        origin_crate = "noble_kernel";
        visibility = "private";
        span_id = "source-span";
      }
      {
        kind = "item";
        fact_id = "expanded";
        qualified_path = mixedPath;
        item_kind = "function";
        origin_crate = "noble_kernel";
        visibility = "private";
        span_id = "derive-span";
      }
    ];
  };
  deriveFixture = ir: derive {
    inherit ir;
    coverage = {
      coverage_id = "synthetic-coverage";
      status = "complete";
      expected_unit_ids = [ "k" "c" ];
      observed_unit_ids = [ "k" "c" ];
      missing_unit_ids = [ ];
      unexpected_unit_ids = [ ];
      duplicate_unit_ids = [ ];
    };
    cargoGraph = {
      cargo_graph_id = "synthetic-graph";
      selected_packages = [ "noble-kernel" ];
      edges = [ ];
    };
    selection = { tool_paths = { }; future_unselected = [ ]; };
  };
  derivationFails = ir: !(builtins.tryEval (builtins.deepSeq (deriveFixture ir).production_subjects true)).success;
  externalEdge = {
    caller_package = "noble-cli";
    callee_package = "anyhow";
    edge_kind = "normal";
  };
  externalInventory = withInventory {
    dependencies = inventory.dependencies // {
      production_edges = inventory.dependencies.production_edges ++ [ externalEdge ];
      external_edges = [ externalEdge ];
    };
  };
  externalPolicy = withPolicy {
    dependencies = policy.dependencies ++ [
      { caller = "noble-cli"; callee = "anyhow"; edge_kind = "normal"; }
    ];
  };
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
    (reject "old inventory schema" (withInventory { schema = "noble-source-inventory/v1"; })
      policy "inventory-schema")
    (reject "missing inventory schema" (builtins.removeAttrs inventory [ "schema" ])
      policy "inventory-schema")
    (reject "missing item span" (withInventory {
      production_subjects = replaceFirst { span_id = ""; } inventory.production_subjects;
    }) policy "subject-span-provenance")
    (reject "missing item provenance" (withInventory {
      production_subjects = [
        (builtins.removeAttrs (head inventory.production_subjects) [ "from_expansion" ])
      ] ++ builtins.tail inventory.production_subjects;
    }) policy "subject-span-provenance")
    (reject "nonboolean item provenance" (withInventory {
      production_subjects = replaceFirst { from_expansion = "false"; } inventory.production_subjects;
    }) policy "subject-span-provenance")
    (reject "missing test provenance" (withInventory {
      test_subjects = [ (builtins.removeAttrs (head inventory.test_subjects) [ "from_expansion" ]) ];
    }) policy "subject-span-provenance")
    (
      assert (evaluateDefault moduleInventory modulePolicy).valid;
      "handwritten Module implementation is structural and its eq is body/open"
    )
    (reject "handwritten Module impl relabeled generated" moduleInventory (withPolicy {
      subjects = subjects ++ [ (reviewed moduleImpl "generated") (reviewed moduleEq "body") ];
    }) "generated-authored-subject")
    (reject "handwritten Module eq relabeled generated" moduleInventory (withPolicy {
      subjects = subjects ++ [ (reviewed moduleImpl "structural") (reviewed moduleEq "generated") ];
    }) "generated-authored-subject")
    (
      assert (evaluateDefault deriveInventory derivePolicy).valid;
      "derive item is eligible for generated accounting, not proof"
    )
    (
      assert (evaluateDefault mixedInventory mixedBodyPolicy).valid;
      "authored test function and expanded registration remain a body"
    )
    (reject "mixed test function relabeled generated" mixedInventory mixedGeneratedPolicy
      "generated-authored-subject")
    (
      assert (evaluateDefault mixedExpansionFirst mixedBodyPolicy).valid;
      "expansion-first mixed test path remains a body"
    )
    (reject "expansion-first mixed path relabeled generated" mixedExpansionFirst mixedGeneratedPolicy
      "generated-authored-subject")
    (
      let result = evaluateDefault inventory (withPolicy {
        subjects = subjects ++ [ (reviewed "noble_kernel::<Old as core::clone::Clone>" "generated") ];
      });
      in
      assert builtins.elem "subject-stale" result.diagnostics
        && !(builtins.elem "generated-authored-subject" result.diagnostics);
      "stale generated path is rejected without inventing an authored observation"
    )
    (
      assert (deriveFixture derivationIR).schema == "noble-source-inventory/v2"
        && map (s: s.from_expansion) (deriveFixture derivationIR).production_subjects == [ false true ]
        && map (s: s.span_id) (deriveFixture derivationIR).production_subjects == [ "source-span" "derive-span" ]
        && map (s: s.units) (deriveFixture derivationIR).production_subjects == [ [ "k" ] [ "c" ] ];
      "derivation preserves both authored and expanded item observations"
    )
    (
      assert derivationFails (derivationIR // { spans = [ (head derivationIR.spans) ]; });
      "derivation rejects missing referenced span"
    )
    (
      assert derivationFails (derivationIR // {
        spans = replaceFirst { from_expansion = "false"; } derivationIR.spans;
      });
      "derivation rejects nonboolean span expansion"
    )
    (
      assert derivationFails (derivationIR // {
        spans = [ (builtins.removeAttrs (head derivationIR.spans) [ "from_expansion" ]) ]
          ++ builtins.tail derivationIR.spans;
      });
      "derivation rejects missing span expansion"
    )
    (
      assert derivationFails (derivationIR // {
        spans = derivationIR.spans ++ [ ((head derivationIR.spans) // { from_expansion = true; }) ];
      });
      "derivation rejects duplicate conflicting span IDs"
    )
    (
      assert derivationFails (derivationIR // {
        spans = derivationIR.spans ++ [ (head derivationIR.spans) ];
      });
      "derivation rejects identical duplicate span IDs"
    )
    (
      assert derivationFails (derivationIR // {
        unit_memberships = [
          { unit_id = "k"; fact_ids = [ ]; }
          { unit_id = "c"; fact_ids = [ "expanded" ]; }
        ];
      });
      "derivation rejects an item without unit membership"
    )
    (
      assert derivationFails (derivationIR // {
        facts = derivationIR.facts ++ [ ((head derivationIR.facts) // { span_id = "derive-span"; }) ];
      });
      "derivation rejects duplicate item fact IDs"
    )
    (
      assert derivationFails (derivationIR // {
        facts = replaceFirst { span_id = ""; } derivationIR.facts;
      });
      "derivation rejects missing item span ID"
    )
    (reject "unreviewed compiler subject" (withInventory {
      production_subjects = inventory.production_subjects ++ [
        {
          qualified_path = "noble_cli::unreviewed";
          item_kind = "function";
          origin_crate = "noble_cli";
          visibility = "private";
          units = [ "c" ];
          span_id = "unreviewed-body";
          from_expansion = false;
        }
      ];
    }) policy "subject-omission")
    (reject "stale reviewed subject" (withInventory {
      production_subjects = builtins.tail inventory.production_subjects;
    }) policy "subject-stale")
    (reject "stale generated subject has no expansion evidence" inventory (withPolicy {
      subjects = subjects ++ [ (reviewed "noble_kernel::<Old as core::clone::Clone>" "generated") ];
    }) "subject-stale")
    (reject "package binding mismatch" inventory (withPolicy {
      subjects = replaceFirst { package = "noble-cli"; } subjects;
    }) "subject-binding")
    (reject "unclassified test subject" inventory (withPolicy { test_subjects = [ ]; }) "test-subject-classification")
    (reject "stale reviewed test subject" inventory (withPolicy {
      test_subjects = policy.test_subjects ++ [ "budget::retired_test" ];
    }) "test-subject-classification")
    (reject "test-only path cannot exempt production body" (withInventory {
      units = inventory.units ++ [
        (unit "t" "noble-kernel" "test+test" "crates/noble-kernel/tests/budget.rs" "test")
      ];
      coverage = inventory.coverage // { expected_units = 3; observed_units = 3; };
      test_subjects = [ ((head inventory.test_subjects) // { units = [ "t" ]; }) ];
      production_subjects = inventory.production_subjects ++ [
        {
          qualified_path = "budget::test";
          item_kind = "function";
          origin_crate = "noble-kernel";
          visibility = "restricted";
          units = [ "k" ];
          span_id = "test-in-production";
          from_expansion = false;
        }
      ];
    }) policy "subject-omission")
    (reject "unreviewed macro body" inventory (withPolicy { macro_origins = [ "<builtin>" ]; }) "macro-classification")
    (reject "unexpected compiler macro origin" (withInventory {
      macro_origins = inventory.macro_origins ++ [ "external::new_macro" ];
    }) policy "macro-classification")
    (reject "stale reviewed macro origin" inventory (withPolicy {
      macro_origins = policy.macro_origins ++ [ "external::old_macro" ];
    }) "macro-classification")
    (reject "unclassified dependency" inventory (withPolicy { dependencies = [ ]; }) "dependency-closure")
    (reject "wrong dependency kind" inventory (withPolicy {
      dependencies = [ ((head policy.dependencies) // { edge_kind = "build"; }) ];
    }) "dependency-closure")
    (
      assert (evaluateDefault externalInventory externalPolicy).valid;
      "exactly reviewed external provider edge"
    )
    (reject "unreviewed external provider edge" externalInventory policy "dependency-closure")
    (reject "stale reviewed external provider edge" inventory externalPolicy "dependency-closure")
    (reject "wrong external provider caller" externalInventory (withPolicy {
      dependencies = policy.dependencies ++ [
        { caller = "noble-kernel"; callee = "anyhow"; edge_kind = "normal"; }
      ];
    }) "dependency-closure")
    (reject "wrong external provider kind" externalInventory (withPolicy {
      dependencies = policy.dependencies ++ [
        { caller = "noble-cli"; callee = "anyhow"; edge_kind = "build"; }
      ];
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
