# Synthetic controls for the Octet pin, scope, and lint-budget checks.
{ }:
let
  rev = "235255bc4972ced9128fd5b4d1ec66ff7508ded4";
  policyBase = {
    mode = "gate";
    waivers = [ ];
    production = {
      packages = [
        "noble-kernel"
        "noble-contracts"
        "noble-cli"
      ];
      source_scopes = [
        "crates/noble-kernel/src"
        "crates/noble-contracts/src"
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
    roles = [
      {
        scope_kind = "package";
        scope = "noble-kernel";
        role = "domain-core";
      }
      {
        scope_kind = "package";
        scope = "noble-contracts";
        role = "domain-core";
      }
      {
        scope_kind = "package";
        scope = "noble-cli";
        role = "composition-root";
      }
      {
        scope_kind = "source";
        scope = "crates/noble-kernel/src";
        role = "domain-core";
      }
      {
        scope_kind = "source";
        scope = "crates/noble-contracts/src";
        role = "domain-core";
      }
      {
        scope_kind = "source";
        scope = "crates/noble-cli/src";
        role = "composition-root";
      }
    ];
    dependency_rules = [
      {
        caller_role = "composition-root";
        allowed_callee_roles = [ "domain-core" ];
        edge_kinds = [ "normal" ];
      }
    ];
    capability_classifications = [
      {
        family = "environment";
        core_handling = "forbidden";
      }
      {
        family = "observability";
        core_handling = "forbidden";
      }
      {
        family = "unsafe-code";
        core_handling = "forbidden";
      }
    ];
    outbound_effects = [
      {
        executor_scope = "crates/noble-cli/src";
        authority = "unprotected";
      }
    ];
    core_no_std = [
      { scope = "noble-kernel"; }
      { scope = "noble-contracts"; }
    ];
    ports = [ ];
    provider_classifications = [ ];
  };
  withArchitecture = change: builtins.toJSON (policyBase // change);
  src = {
    flakeNix = ''octet.url = "git+ssh://git@github.com/OnixResearch/octet?rev=${rev}";'';
    flakeLock = builtins.toJSON {
      root = "root";
      nodes = {
        root.inputs.octet = "octet";
        octet.locked = {
          rev = rev;
          narHash = "sha256-4vFO0V3jtN4GrFbIcqHPqjaZdQK5tQof682BFOKiFtw=";
          url = "ssh://git@github.com/OnixResearch/octet";
        };
      };
    };
    preCommit = ''
      repos:
        - repo: ssh://git@github.com/OnixResearch/octet
          rev: ${rev}
          hooks:
            - id: octet-deny-all
              args: ["--workspace", "--", "--all-targets", "--all-features"]
    '';
    selectionJson = {
      sources.octet.rev = rev;
      sources.octet.narHash = "sha256-4vFO0V3jtN4GrFbIcqHPqjaZdQK5tQof682BFOKiFtw=";
    };
    cargoToml = ''
      [workspace.lints.rust]
      unsafe_code = "forbid"

      [workspace.metadata.octet]
      default_scope = ["--workspace"]
      cargo_check_args = ["--all-targets", "--all-features"]
    '';
    architecturePolicyJson = builtins.toJSON policyBase;
  };
  evaluate =
    overrides:
    import ./octet-controls.nix {
      flakeNix = overrides.flakeNix or src.flakeNix;
      flakeLock = overrides.flakeLock or src.flakeLock;
      preCommit = overrides.preCommit or src.preCommit;
      selectionJson = overrides.selectionJson or src.selectionJson;
      cargoToml = overrides.cargoToml or src.cargoToml;
      architecturePolicyJson = overrides.architecturePolicyJson or src.architecturePolicyJson;
    };
  reject =
    name: overrides: expected:
    let
      result = evaluate overrides;
    in
    if result.valid then
      throw "control unexpectedly passed: ${name}"
    else if builtins.elem expected result.diagnostics then
      name
    else
      throw "wrong diagnostic for ${name}: ${builtins.toJSON result.diagnostics}";
  tests = [
    (assert (evaluate { }).valid; "matching Octet pin, scope, and lint configuration")
    (assert (evaluate { }).rev == rev; "the reviewed revision is reported")
    (reject "flake url revision" {
      flakeNix = ''octet.url = "git+ssh://git@github.com/OnixResearch/octet?rev=0000000000000000000000000000000000000000";'';
    } "flake-octet-revision-mismatch")
    (reject "flake lock revision" {
      flakeLock = builtins.toJSON {
        root = "root";
        nodes = {
          root.inputs.octet = "octet";
          octet.locked = {
            rev = "0000000000000000000000000000000000000000";
            narHash = "sha256-4vFO0V3jtN4GrFbIcqHPqjaZdQK5tQof682BFOKiFtw=";
            url = "ssh://git@github.com/OnixResearch/octet";
          };
        };
      };
    } "flake-lock-octet-revision-mismatch")
    (reject "flake lock nar hash" {
      flakeLock = builtins.toJSON {
        root = "root";
        nodes = {
          root.inputs.octet = "octet";
          octet.locked = {
            rev = rev;
            narHash = "sha256-0000000000000000000000000000000000000000000=";
            url = "ssh://git@github.com/OnixResearch/octet";
          };
        };
      };
    } "flake-lock-octet-nar-mismatch")
    (reject "pre-commit revision" {
      preCommit = ''
        repos:
          - repo: ssh://git@github.com/OnixResearch/octet
            rev: 0000000000000000000000000000000000000000
            hooks:
              - id: octet-deny-all
                args: ["--workspace", "--", "--all-targets", "--all-features"]
      '';
    } "precommit-octet-revision-mismatch")
    (reject "pre-commit missing octet repo" {
      preCommit = ''
        repos:
          - repo: ssh://git@github.com/Other/other
            rev: ${rev}
      '';
    } "precommit-octet-repo-missing")
    (reject "pre-commit scope" {
      preCommit = ''
        repos:
          - repo: ssh://git@github.com/OnixResearch/octet
            rev: ${rev}
            hooks:
              - id: octet-deny-all
                args: ["--", "--all-targets", "--all-features"]
      '';
    } "precommit-scope-missing")
    (reject "pre-commit feature scope" {
      preCommit = ''
        repos:
          - repo: ssh://git@github.com/OnixResearch/octet
            rev: ${rev}
            hooks:
              - id: octet-deny-all
                args: ["--workspace", "--", "--all-targets"]
      '';
    } "precommit-features-missing")
    (reject "octet metadata scope" {
      cargoToml = ''
        [workspace.lints.rust]
        unsafe_code = "forbid"

        [workspace.metadata.octet]
        default_scope = ["--package", "noble-kernel"]
        cargo_check_args = ["--all-targets", "--all-features"]
      '';
    } "octet-metadata-scope-mismatch")
    (reject "lint downgrade" {
      cargoToml = ''
        [workspace.lints.rust]
        unsafe_code = "forbid"
        dead_code = "allow"

        [workspace.metadata.octet]
        default_scope = ["--workspace"]
        cargo_check_args = ["--all-targets", "--all-features"]
      '';
    } "lint-downgrade-present")
    (reject "unsafe lint not forbidden" {
      cargoToml = ''
        [workspace.lints.rust]
        unsafe_code = "warn"

        [workspace.metadata.octet]
        default_scope = ["--workspace"]
        cargo_check_args = ["--all-targets", "--all-features"]
      '';
    } "unsafe-lint-not-forbidden")
    (reject "architecture mode" {
      architecturePolicyJson = builtins.toJSON {
        mode = "advisory";
        waivers = [ ];
      };
    } "architecture-mode-not-gate")
    (reject "architecture waiver" {
      architecturePolicyJson = builtins.toJSON {
        mode = "gate";
        waivers = [
          {
            finding_id = "x";
          }
        ];
      };
    } "architecture-waivers-present")
    (reject "architecture finding budget" {
      architecturePolicyJson = builtins.toJSON {
        mode = "gate";
        waivers = [ ];
        finding_budget = 1;
      };
    } "architecture-finding-budget-present")
    (reject "policy package missing" {
      architecturePolicyJson = withArchitecture {
        production = policyBase.production // { packages = [ "noble-kernel" ]; };
      };
    } "policy-package-missing")
    (reject "policy source scope missing" {
      architecturePolicyJson = withArchitecture {
        production = policyBase.production // {
          source_scopes = [ "crates/noble-kernel/src" ];
        };
      };
    } "policy-source-scope-missing")
    (reject "policy target missing" {
      architecturePolicyJson = withArchitecture {
        production = policyBase.production // { targets = [ ]; };
      };
    } "policy-target-missing")
    (reject "policy target incomplete" {
      architecturePolicyJson = withArchitecture {
        production = policyBase.production // {
          targets = [ { triple = "x86_64-unknown-linux-gnu"; } ];
        };
      };
    } "policy-target-incomplete")
    (reject "policy package role missing" {
      architecturePolicyJson = withArchitecture {
        roles = [
          {
            scope_kind = "source";
            scope = "crates/noble-kernel/src";
            role = "domain-core";
          }
          {
            scope_kind = "source";
            scope = "crates/noble-cli/src";
            role = "composition-root";
          }
        ];
      };
    } "policy-package-role-missing")
    (reject "policy dependency rules missing" {
      architecturePolicyJson = withArchitecture { dependency_rules = [ ]; };
    } "policy-dependency-rules-missing")
    (reject "policy capability family missing" {
      architecturePolicyJson = withArchitecture {
        capability_classifications = [
          {
            family = "environment";
            core_handling = "forbidden";
          }
        ];
      };
    } "policy-capability-family-missing")
    (reject "policy capability not forbidden" {
      architecturePolicyJson = withArchitecture {
        capability_classifications = [
          {
            family = "environment";
            core_handling = "forbidden";
          }
          {
            family = "observability";
            core_handling = "forbidden";
          }
          {
            family = "unsafe-code";
            core_handling = "allowed";
          }
        ];
      };
    } "policy-capability-not-forbidden")
    (reject "policy outbound effects missing" {
      architecturePolicyJson = withArchitecture { outbound_effects = [ ]; };
    } "policy-outbound-effects-missing")
    (reject "policy outbound authority missing" {
      architecturePolicyJson = withArchitecture {
        outbound_effects = [
          {
            executor_scope = "crates/noble-cli/src";
            authority = "";
          }
        ];
      };
    } "policy-outbound-authority-missing")
    (reject "policy core no-std missing" {
      architecturePolicyJson = withArchitecture { core_no_std = [ ]; };
    } "policy-core-no-std-missing")
    (reject "contract frontend no-std missing" {
      architecturePolicyJson = withArchitecture {
        core_no_std = [ { scope = "noble-kernel"; } ];
      };
    } "policy-core-no-std-missing")
    (reject "policy core no-std mismatch" {
      architecturePolicyJson = withArchitecture {
        core_no_std = [ { scope = "noble-cli"; } ];
      };
    } "policy-core-no-std-mismatch")
    (reject "policy ports not declared" {
      architecturePolicyJson = builtins.toJSON (builtins.removeAttrs policyBase [ "ports" ]);
    } "policy-ports-not-declared-empty")
    (reject "policy providers not declared" {
      architecturePolicyJson = builtins.toJSON (
        builtins.removeAttrs policyBase [ "provider_classifications" ]
      );
    } "policy-providers-not-declared-empty")
    (assert (evaluate { }).policy_summary.mode == "gate"; "policy summary reports the mode")
    (assert (evaluate { }).policy_summary.forbidden_families == [
      "environment"
      "observability"
      "unsafe-code"
    ]; "policy summary reports the forbidden capability families")
  ];
in
builtins.deepSeq tests tests
