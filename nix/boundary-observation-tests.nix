# Synthetic comparator fixtures only. These do not claim compiler provenance.
{ policy, selection }:
let
  inherit (builtins)
    head
    filter
    map
    listToAttrs
    ;
  cases = listToAttrs (
    map (c: {
      name = c.id;
      value = c;
    }) policy.cases
  );
  unit =
    index: row:
    let
      cells = filter builtins.isString (builtins.split ":" row);
    in
    {
      unit_id = toString index;
      package_name = builtins.elemAt cells 0;
      target_kind = builtins.elemAt cells 1;
      source_path = builtins.elemAt cells 2;
      role = builtins.elemAt cells 3;
      target_triple = selection.configuration.target;
      features = selection.configuration.features;
      default_features = selection.configuration.default_features;
    };
  units = builtins.genList (i: unit i (builtins.elemAt policy.units i)) (
    builtins.length policy.units
  );
  kernels = filter (u: u.package_name == "noble-kernel" && u.role == "domain-core") units;
  item = {
    kind = "item";
    fact_id = "body";
    qualified_path = "noble_kernel::consume_budget";
  };
  clean = {
    deny_exit = 0;
    deny_findings = [ ];
    deny_status = {
      status = "clean";
      error_findings = 0;
      warning_findings = 0;
      cargo_process_exit.code = 0;
      metadata.toolchain = "${selection.configuration.quality_rust}-${selection.configuration.target}";
    };
    observation_exit = 0;
    observation_status = {
      cargo_process_exit.code = 0;
      exit_code = 0;
    };
    ir = {
      schema_version = "octet-compiler-architecture-ir/v2";
      inherit units;
      facts = [ item ];
      unit_memberships = map (u: {
        inherit (u) unit_id;
        fact_ids = [
          "body"
          "effect"
        ];
      }) kernels;
    };
    receipt = {
      schema_version = "octet-project-architecture-receipt/v2";
      mode = "gate";
      status = "clean";
      findings = [ ];
    };
    replay.status = "valid";
  };
  denied = clean // {
    deny_exit = 2;
    deny_status = clean.deny_status // {
      status = "integration-failure";
      error_findings = 1;
      cargo_process_exit.code = 101;
    };
    deny_findings = [
      {
        lint = "impure_call_in_core";
        severity = "error";
        file = policy.kernel_source;
        crate_name = "noble_kernel";
        group = "tigerstyle::purity";
      }
    ];
    observation_exit = 1;
    observation_status = {
      cargo_process_exit.code = 0;
      exit_code = 1;
    };
    ir = clean.ir // {
      facts = [
        item
        {
          kind = "effect";
          fact_id = "effect";
          capability_family = "filesystem";
        }
      ];
    };
    receipt = clean.receipt // {
      status = "blocked";
      findings = map (u: {
        code = "domain-core-effect";
        status = "open";
        package_name = "noble-kernel";
        inherit (u) unit_id;
        source_path = policy.kernel_source;
        fact_id = "effect";
      }) kernels;
    };
  };
  randomInput = denied // {
    deny_findings = map (f: f // { lint = cases.randomness.deny_lint; }) denied.deny_findings;
    ir = denied.ir // {
      facts = [
        item
        {
          kind = "effect";
          fact_id = "effect";
          capability_family = "randomness";
        }
      ];
    };
  };
  withFact =
    case: fact:
    denied
    // {
      deny_findings = map (f: f // { lint = case.deny_lint; }) denied.deny_findings;
      observation_exit = 0;
      observation_status = clean.observation_status;
      ir = denied.ir // {
        facts = [
          item
          (fact // { fact_id = "effect"; })
        ];
      };
      receipt = clean.receipt;
    };
  callInput = withFact cases.global-state {
    kind = "call";
    resolved_definition = "core::sync::atomic::Atomic::<u32>::fetch_add";
  };
  typeInput = withFact cases.host-return {
    kind = "type";
    origin_crate = "std";
    definition_path = "std::fs::File";
  };
  unsafeInput = {
    inherit (denied) deny_exit deny_status;
    deny_findings = map (
      f:
      f
      // {
        lint = "unsafe_code";
        group = "tigerstyle::unknown";
      }
    ) denied.deny_findings;
  };
  policyInput = {
    inherit (denied) deny_exit deny_status;
    deny_findings = map (
      f:
      f
      // {
        lint = cases.no-std-test-witness.deny_lint;
        file = cases.no-std-test-witness.deny_source;
      }
    ) denied.deny_findings;
    observation_exit = 1;
    observation_status = {
      cargo_process_exit.code = 0;
      exit_code = 1;
    };
    ir = clean.ir;
    receipt = {
      schema_version = "octet-project-architecture-receipt/v2";
      mode = "gate";
      status = "blocked";
      findings = [
        {
          code = "no-std-obligation";
          status = "open";
          severity = "policy";
          package_name = null;
          unit_id = null;
          source_path = "noble-kernel";
          message = "domain-core scope `noble-kernel` lacks compiler-confirmed crate:no-std";
        }
      ];
    };
    replay.status = "valid";
  };
  precollectionInput = {
    deny_exit = 2;
    deny_status = { };
    deny_findings = [ ];
    precollection = {
      exit = 2;
      diagnostic = "error: invalid Cargo diagnostic path: capability locator `${policy.test_source.escaped}` contains `..`\n";
      status_published = false;
    };
  };
  extraUnits = builtins.genList (
    i: unit (i + builtins.length units) (builtins.elemAt policy.acyclic_extra_units i)
  ) (builtins.length policy.acyclic_extra_units);
  reverseInput = clean // {
    deny_exit = 1;
    deny_status = clean.deny_status // {
      phases = {
        architecture.status = "blocked";
        lint.status = "clean";
      };
    };
    observation_exit = 1;
    observation_status = {
      cargo_process_exit.code = 0;
      exit_code = 1;
    };
    ir = clean.ir // {
      units = units ++ extraUnits;
    };
    receipt = denied.receipt // {
      findings = [
        {
          code = "forbidden-role-edge";
          status = "open";
          package_name = "noble-kernel";
          unit_id = (head kernels).unit_id;
        }
      ];
    };
  };
  dependencyInput = reverseInput // {
    ir = reverseInput.ir // {
      inherit units;
    };
    receipt = reverseInput.receipt // {
      findings = map (
        f: f // { message = cases.randomness-dependency.edge_message; }
      ) reverseInput.receipt.findings;
    };
  };
  evaluate =
    case: observed:
    import ./boundary-observation.nix {
      inherit
        policy
        selection
        case
        observed
        ;
    };
  reject =
    name: case: observed: expected:
    let
      result = evaluate case observed;
    in
    assert !result.valid && builtins.elem expected result.diagnostics;
    name;
  fs = cases.filesystem;
  tests = [
    (
      assert (evaluate cases.randomness-dependency dependencyInput).valid;
      "dependency-only rejection retains the workspace roster"
    )
    (reject "wrong dependency rejection" cases.randomness-dependency (
      dependencyInput
      // {
        receipt = dependencyInput.receipt // {
          findings = map (f: f // { message = "unrelated dependency"; }) dependencyInput.receipt.findings;
        };
      }
    ) "expected-boundary-observation")
    (
      assert (evaluate cases.randomness randomInput).valid;
      "randomness in both production observers"
    )
    (reject "filesystem is not randomness" cases.randomness (
      randomInput // { ir = denied.ir; }
    ) "expected-boundary-observation")
    (reject "wrong randomness lint" cases.randomness (
      randomInput // { deny_findings = denied.deny_findings; }
    ) "required-deny-all-result")
    (reject "one randomness observer is insufficient" cases.randomness (
      randomInput
      // {
        ir = randomInput.ir // {
          unit_memberships = [ (head randomInput.ir.unit_memberships) ];
        };
      }
    ) "expected-boundary-observation")
    (reject "test-only randomness cannot witness production" cases.randomness (
      randomInput
      // {
        ir = randomInput.ir // {
          unit_memberships = map (m: m // { fact_ids = [ "body" ]; }) randomInput.ir.unit_memberships ++ [
            {
              unit_id = "2";
              fact_ids = [ "effect" ];
            }
          ];
        };
      }
    ) "expected-boundary-observation")
    (
      assert (evaluate cases.global-state callInput).valid;
      "resolved compiler call"
    )
    (
      assert (evaluate cases.host-return typeInput).valid;
      "resolved host return type"
    )
    (
      assert (evaluate cases.unsafe-block unsafeInput).valid;
      "compiler prohibition without invented IR"
    )
    (
      assert (evaluate cases.reverse-dependency reverseInput).valid;
      "acyclic forbidden role edge"
    )
    (reject "unresolved compiler call" cases.global-state (
      callInput
      // {
        ir = callInput.ir // {
          facts = [
            item
            {
              kind = "call";
              fact_id = "effect";
              resolved_definition = null;
            }
          ];
        };
      }
    ) "expected-boundary-observation")
    (reject "test-only host type" cases.host-return (
      typeInput
      // {
        ir = typeInput.ir // {
          unit_memberships = map (m: m // { fact_ids = [ "body" ]; }) typeInput.ir.unit_memberships;
        };
      }
    ) "expected-boundary-observation")
    (reject "wrong host type origin" cases.host-return (
      typeInput
      // {
        ir = typeInput.ir // {
          facts = [
            item
            {
              kind = "type";
              fact_id = "effect";
              origin_crate = "fake";
              definition_path = "std::fs::File";
            }
          ];
        };
      }
    ) "expected-boundary-observation")
    (reject "wrong unsafe failure" cases.unsafe-block (
      unsafeInput // { deny_findings = [ ]; }
    ) "required-deny-all-result")
    (
      assert (evaluate cases.unsafe-block unsafeInput).valid;
      "compiler prohibition without invented IR"
    )
    (
      assert (evaluate cases.unsafe-foreign-declaration unsafeInput).valid;
      "foreign unsafe declaration is a compiler prohibition"
    )
    (reject "foreign declaration needs the compiler prohibition" cases.unsafe-foreign-declaration (
      unsafeInput // { deny_findings = [ ]; }
    ) "required-deny-all-result")
    (reject "foreign declaration rejects an unrelated error" cases.unsafe-foreign-declaration (
      unsafeInput
      // {
        deny_findings = unsafeInput.deny_findings ++ [
          {
            severity = "error";
            group = "tigerstyle::unknown";
            lint = "E0425";
          }
        ];
      }
    ) "required-deny-all-result")
    (
      assert (evaluate cases.no-std-test-witness policyInput).valid;
      "test-configured no-std witness cannot satisfy the production obligation"
    )
    (reject "clean no-std receipt" cases.no-std-test-witness (
      policyInput
      // {
        receipt = policyInput.receipt // { status = "clean"; findings = [ ]; };
      }
    ) "expected-boundary-observation")
    (reject "wrong no-std finding code" cases.no-std-test-witness (
      policyInput
      // {
        receipt = policyInput.receipt // {
          findings = map (f: f // { code = "unrelated-finding"; }) policyInput.receipt.findings;
        };
      }
    ) "expected-boundary-observation")
    (reject "wrong no-std finding message" cases.no-std-test-witness (
      policyInput
      // {
        receipt = policyInput.receipt // {
          findings = map (f: f // { message = "unrelated message"; }) policyInput.receipt.findings;
        };
      }
    ) "expected-boundary-observation")
    (reject "test-only no-std finding owner" cases.no-std-test-witness (
      policyInput
      // {
        receipt = policyInput.receipt // {
          findings = map (f: f // { source_path = "noble-cli"; }) policyInput.receipt.findings;
        };
      }
    ) "expected-boundary-observation")
    (reject "waived no-std finding is not a rejection" cases.no-std-test-witness (
      policyInput
      // {
        receipt = policyInput.receipt // {
          findings = map (f: f // { status = "waived"; }) policyInput.receipt.findings;
        };
      }
    ) "expected-boundary-observation")
    (reject "approved no-std gate" cases.no-std-test-witness (
      policyInput // { deny_exit = 0; }
    ) "required-deny-all-result")
    (
      assert (evaluate cases.test-source-import precollectionInput).valid;
      "production import of test source rejects before collection"
    )
    (reject "published test-source bundle" cases.test-source-import (
      precollectionInput // { precollection = precollectionInput.precollection // { status_published = true; }; }
    ) "required-deny-all-result")
    (reject "unrelated precollection diagnostic" cases.test-source-import (
      precollectionInput
      // {
        precollection = precollectionInput.precollection // {
          diagnostic = "error: unrelated compiler failure\n";
        };
      }
    ) "required-deny-all-result")
    (reject "missing escaped path" cases.test-source-import (
      precollectionInput
      // {
        precollection = precollectionInput.precollection // {
          diagnostic = "error: invalid Cargo diagnostic path: contains `..`\n";
        };
      }
    ) "required-deny-all-result")
    (reject "Cargo cycle is not role enforcement" cases.reverse-dependency (
      reverseInput
      // {
        deny_status = reverseInput.deny_status // {
          cargo_process_exit.code = 101;
        };
      }
    ) "required-deny-all-result")
    (reject "wrong edge owner" cases.reverse-dependency (
      reverseInput
      // {
        receipt = reverseInput.receipt // {
          findings = map (f: f // { package_name = "noble-cli"; }) reverseInput.receipt.findings;
        };
      }
    ) "expected-boundary-observation")
    (
      assert (evaluate cases.workspace clean).valid;
      "clean synthetic comparator input"
    )
    (
      assert (evaluate fs denied).valid;
      "matched synthetic compiler effect"
    )
    (reject "successful negative" fs (denied // { deny_exit = 0; }) "required-deny-all-result")
    (reject "wrong lint" fs (denied // { deny_findings = [ ]; }) "required-deny-all-result")
    (reject "warning not error" fs (
      denied // { deny_findings = map (f: f // { severity = "warning"; }) denied.deny_findings; }
    ) "required-deny-all-result")
    (reject "unrelated compiler failure" fs (
      denied
      // {
        deny_findings = denied.deny_findings ++ [
          {
            severity = "error";
            group = "tigerstyle::unknown";
            lint = "E0425";
          }
        ];
      }
    ) "required-deny-all-result")
    (reject "wrong source" fs (
      denied
      // {
        deny_findings = map (f: f // { file = "crates/noble-cli/src/main.rs"; }) denied.deny_findings;
      }
    ) "required-deny-all-result")
    (reject "wrong compiler" fs (
      denied
      // {
        deny_status = denied.deny_status // {
          metadata.toolchain = "unselected";
        };
      }
    ) "deny-toolchain")
    (reject "invalid replay" fs (denied // { replay.status = "invalid"; }) "artifact-replay")
    (reject "old schema" fs (
      denied
      // {
        ir = denied.ir // {
          schema_version = "v1";
        };
      }
    ) "provider-schema")
    (reject "missing unit" fs (
      denied
      // {
        ir = denied.ir // {
          units = builtins.tail units;
        };
      }
    ) "compiler-unit-roster")
    (reject "test promotion" fs (
      denied
      // {
        ir = denied.ir // {
          units = map (u: if u.role == "domain-core" then u // { role = "test"; } else u) units;
        };
      }
    ) "kernel-ownership")
    (reject "wrong target" fs (
      denied
      // {
        ir = denied.ir // {
          units = map (u: u // { target_triple = "other"; }) units;
        };
      }
    ) "compiler-configuration")
    (reject "missing membership" fs (
      denied
      // {
        ir = denied.ir // {
          unit_memberships = [ ];
        };
      }
    ) "kernel-ownership")
    (reject "wrong body" fs (
      denied
      // {
        ir = denied.ir // {
          facts = map (f: if f.kind == "item" then f // { qualified_path = "other"; } else f) denied.ir.facts;
        };
      }
    ) "kernel-ownership")
    (reject "advisory mode" fs (
      denied
      // {
        receipt = denied.receipt // {
          mode = "advisory";
        };
      }
    ) "architecture-mode")
    (reject "failed fact collection" fs (
      denied
      // {
        observation_status = denied.observation_status // {
          cargo_process_exit.code = 101;
        };
      }
    ) "observation-process")
    (reject "missing expected effect" fs (
      denied
      // {
        ir = denied.ir // {
          facts = [ item ];
        };
      }
    ) "expected-boundary-observation")
    (reject "missing per-unit finding" fs (
      denied
      // {
        receipt = denied.receipt // {
          findings = [ (head denied.receipt.findings) ];
        };
      }
    ) "expected-boundary-observation")
    (reject "unrelated finding" fs (
      denied
      // {
        receipt = denied.receipt // {
          findings = map (f: f // { code = "required-compiler-unknown"; }) denied.receipt.findings;
        };
      }
    ) "expected-boundary-observation")
  ];
in
builtins.deepSeq tests tests
