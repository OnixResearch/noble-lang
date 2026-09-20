# Synthetic native-assurance comparator controls. Not compiler or interpreter evidence.
{ }:
let
  inventory = {
    schema = "noble-native-assurance-inventory/v1";
    scope = {
      packages = [
        "noble-cli"
        "noble-contracts"
        "noble-kernel"
        "noble-wasm"
      ];
      target_triple = "x86_64-unknown-linux-gnu";
      features = [ ];
      default_features = true;
      units = 6;
      ir_id = "aaaa";
      policy_blake3 = "bbbb";
      coverage_id = null;
    };
    owned_unsafe = {
      workspace_forbids_unsafe = true;
      sites = [ ];
      function_safety_facts = 0;
      foreign_items = [ ];
      production_items = 17;
      item_kinds = [
        "constant"
        "enum"
        "function"
        "other"
      ];
    };
    dependencies = {
      selected_packages = [
        "noble-cli"
        "noble-contracts"
        "noble-kernel"
        "noble-wasm"
      ];
      production_edges = [ ];
      external_edges = [ ];
      fixture_packages = [
        {
          name = "getrandom";
          version = "0.4.3";
        }
      ];
    };
    compiler_tools = [
      "aeneas"
      "cargo-octet"
      "charon"
      "lean"
      "miri"
    ];
    foreign_libraries = [ ];
    counts = {
      units = 6;
      production_items = 17;
      function_safety_facts = 0;
      unsafe_sites = 0;
      foreign_items = 0;
      production_dependencies = 0;
      fixture_packages = 1;
    };
  };
  policy = {
    schema_version = "noble-native-assurance-policy/v1";
    required_packages = [
      "noble-kernel"
      "noble-contracts"
      "noble-wasm"
      "noble-cli"
    ];
    required_targets = [
      {
        triple = "x86_64-unknown-linux-gnu";
        features = [ ];
        default_features = true;
      }
    ];
    owned_unsafe = {
      expectation = "empty";
      sites = [ ];
      accounting_basis = [
        "workspace-lint-unsafe-code-forbid"
        "compiler-function-safety-facts-absent"
        "compiler-foreign-item-facts-absent"
      ];
    };
    dependencies = {
      production = [ ];
      fixture_only = [
        {
          name = "getrandom";
          version = "0.4.3";
          lock = "verification/fixtures/randomness.Cargo.lock";
          purpose = "fixture only";
        }
      ];
    };
    compiler_tools = [
      {
        name = "cargo-octet";
        role = "architecture-collector";
      }
      {
        name = "charon";
        role = "extraction";
      }
      {
        name = "aeneas";
        role = "extraction";
      }
      {
        name = "lean";
        role = "proof-assistant";
      }
      {
        name = "miri";
        role = "interpreter";
      }
    ];
    foreign_libraries = [ ];
    miri = {
      required = true;
      toolchain = "nightly-2026-08-18";
      flags = [ "-Zmiri-strict-provenance" ];
      target = "x86_64-unknown-linux-gnu";
      subject = "crates/noble-kernel/Cargo.toml";
      required_tests = [ "budget" ];
      unsupported_configurations = [ ];
    };
    safety_arguments = [
      {
        scope = "noble-kernel";
        argument = "safe sequential Rust";
      }
      {
        scope = "noble-contracts";
        argument = "safe no_std frontend";
      }
      {
        scope = "noble-wasm";
        argument = "safe no_std emitter; owned WAT semantics remain unproved";
      }
      {
        scope = "noble-cli";
        argument = "shell only";
      }
    ];
    target_assumptions = [
      {
        target = "x86_64-unknown-linux-gnu";
        word_bits = 64;
        assumptions = [ "64-bit target" ];
      }
    ];
  };
  evaluate =
    inv: pol:
    import ./native-assurance.nix {
      inventory = inv;
      policy = pol;
    };
  reject =
    name: inv: pol: expected:
    let
      result = evaluate inv pol;
    in
    if result.valid then
      throw "control unexpectedly passed: ${name}"
    else if builtins.elem expected result.diagnostics then
      name
    else
      throw "wrong diagnostic for ${name}: ${builtins.toJSON result.diagnostics}";
  withInventory = change: inventory // change;
  withPolicy = change: policy // change;
  withOwnedUnsafe = change: inventory // { owned_unsafe = inventory.owned_unsafe // change; };
  tests = [
    (
      assert (evaluate inventory policy).valid;
      "reviewed native inventory matches compiler and Cargo facts"
    )
    (
      assert (evaluate inventory policy).counts.unsafe_sites == 0;
      "empty owned-unsafe scope is reported, not assumed"
    )
    (reject "missing scope package" (withInventory {
      scope = inventory.scope // { packages = [ "noble-kernel" ]; };
    }) policy "scope-package-missing")
    (reject "Wasm emitter missing from native scope" (withInventory {
      scope = inventory.scope // {
        packages = builtins.filter (package: package != "noble-wasm") inventory.scope.packages;
      };
    }) policy "scope-package-missing")
    (reject "missing required target" (withInventory {
      scope = inventory.scope // { target_triple = "aarch64-unknown-linux-gnu"; };
    }) policy "scope-target-missing")
    (reject "feature drift" (withInventory {
      scope = inventory.scope // { features = [ "extra" ]; };
    }) policy "scope-target-missing")
    (reject "unsafe forbid evidence missing" (withOwnedUnsafe { workspace_forbids_unsafe = false; }) policy
      "unsafe-forbid-evidence-missing")
    (reject "accounting basis missing" inventory (withPolicy {
      owned_unsafe = policy.owned_unsafe // { accounting_basis = [ "workspace-lint-unsafe-code-forbid" ]; };
    }) "unsafe-accounting-basis-missing")
    (reject "unsafe function facts present" (withOwnedUnsafe { function_safety_facts = 1; }) policy
      "unsafe-function-facts-present")
    (reject "foreign items present" (withOwnedUnsafe { foreign_items = [ "noble_kernel::ffi" ]; }) policy
      "unsafe-foreign-items-present")
    (reject "owned unsafe site without record" (withOwnedUnsafe {
      sites = [
        {
          qualified_path = "noble_kernel::hidden";
          is_unsafe = true;
        }
      ];
    }) policy "owned-unsafe-without-record")
    (reject "reviewed unsafe site absent" inventory (withPolicy {
      owned_unsafe = policy.owned_unsafe // {
        sites = [
          {
            symbol = "noble_kernel::claimed";
            argument = "claimed";
            reviewer = "noble-maintainers";
          }
        ];
      };
    }) "reviewed-unsafe-site-absent")
    (reject "unclassified production dependency" (withInventory {
      dependencies = inventory.dependencies // {
        production_edges = inventory.dependencies.production_edges ++ [
          {
            caller_package = "noble-kernel";
            callee_package = "serde";
            edge_kind = "normal";
          }
        ];
      };
    }) policy "production-dependency-mismatch")
    (reject "missing fixture dependency record" (withInventory {
      dependencies = inventory.dependencies // { fixture_packages = [ ]; };
    }) policy "fixture-dependency-missing")
    (reject "fixture lock unrecorded" inventory (withPolicy {
      dependencies = policy.dependencies // {
        fixture_only = [
          {
            name = "getrandom";
            version = "0.4.3";
            lock = "";
            purpose = "fixture only";
          }
        ];
      };
    }) "fixture-lock-unrecorded")
    (reject "unclassified compiler tool" (withInventory { compiler_tools = [ "miri" ]; }) policy
      "compiler-tool-mismatch")
    (reject "unrecorded foreign library" (withInventory {
      foreign_libraries = [ "libc" ];
    }) policy "foreign-library-mismatch")
    (reject "missing safety argument" inventory (withPolicy {
      safety_arguments = [
        {
          scope = "noble-kernel";
          argument = "safe sequential Rust";
        }
      ];
    }) "safety-argument-missing")
    (reject "missing target assumptions" inventory (withPolicy { target_assumptions = [ ]; })
      "target-assumption-missing")
    (reject "Wasm emitter missing safety argument" inventory (withPolicy {
      safety_arguments = builtins.filter (entry: entry.scope != "noble-wasm") policy.safety_arguments;
    }) "safety-argument-missing")
    (reject "miri not required" inventory (withPolicy { miri = policy.miri // { required = false; }; })
      "miri-not-required")
    (reject "unsupported required miri configuration" inventory (withPolicy {
      miri = policy.miri // { unsupported_configurations = [ "wasm32" ]; };
    }) "miri-unsupported-configuration")
    (reject "miri flags missing" inventory (withPolicy { miri = policy.miri // { flags = [ ]; }; })
      "miri-flags-missing")
    (reject "miri tests missing" inventory (withPolicy { miri = policy.miri // { required_tests = [ ]; }; })
      "miri-tests-missing")
    (reject "miri configuration mismatch" inventory (withPolicy {
      miri = policy.miri // { toolchain = "stable"; };
    }) "miri-configuration-mismatch")
    (reject "empty safety argument" inventory (withPolicy {
      safety_arguments = map (
        entry: entry // { argument = if entry.scope == "noble-kernel" then "" else entry.argument; }
      ) policy.safety_arguments;
    }) "safety-argument-empty")
  ];
in
builtins.deepSeq tests tests
