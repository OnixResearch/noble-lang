# Pure Octet pin, scope, and lint-budget controls for VT-M1-05.
# Reviewed configuration only: this makes no enforcement or compatibility claim.
{
  flakeNix,
  flakeLock,
  preCommit,
  selectionJson,
  cargoToml,
  architecturePolicyJson,
}:
let
  inherit (builtins)
    all
    any
    elem
    filter
    fromJSON
    fromTOML
    length
    match
    split
    ;
  require = condition: diagnostic: if condition then [ ] else [ diagnostic ];
  sorted = values: builtins.sort builtins.lessThan values;
  lines = text: filter builtins.isString (split "\n" text);
  rev = selectionJson.sources.octet.rev;
  narHash = selectionJson.sources.octet.narHash;

  lock = fromJSON flakeLock;
  octetNode = lock.nodes.${lock.nodes.${lock.root}.inputs.octet};
  lockRev = octetNode.locked.rev;
  lockNarHash = octetNode.locked.narHash;
  lockUrl = octetNode.locked.url or "";

  preCommitOctetLine = filter (
    line: (match ".*OnixResearch/octet.*" line != null) || (match ".*octet.*" line != null)
  ) (lines preCommit);
  preCommitRevLines = filter (line: match "[[:space:]]*rev:[[:space:]]*[0-9a-f]{40}[[:space:]]*" line != null) (
    lines preCommit
  );
  preCommitArgs = filter (line: match ".*args:.*" line != null) (lines preCommit);

  cargo = fromTOML cargoToml;
  lintTable = cargo.workspace.lints.rust or { };
  lintAttributes = builtins.attrNames lintTable;
  lintLevels = map (name: lintTable.${name}) lintAttributes;
  metadata = cargo.workspace.metadata.octet or { };

  policy = fromJSON architecturePolicyJson;
  policyMode = policy.mode;
  policyWaivers = policy.waivers;

  # VT-M1-05 requires the policy to declare roles, packages, sources,
  # providers, dependencies, ports and adapters, targets, and features.
  requiredPackages = [
    "noble-cli"
    "noble-contracts"
    "noble-kernel"
    "noble-wasm"
  ];
  requiredScopes = [
    "crates/noble-cli/src"
    "crates/noble-contracts/src"
    "crates/noble-kernel/src"
    "crates/noble-wasm/src"
  ];
  requiredCorePackages = [
    "noble-contracts"
    "noble-kernel"
    "noble-wasm"
  ];
  production = policy.production or { };
  declaredPackages = production.packages or [ ];
  declaredScopes = production.source_scopes or [ ];
  declaredTargets = production.targets or [ ];
  roles = policy.roles or [ ];
  dependencyRules = policy.dependency_rules or [ ];
  capabilityFamilies = map (c: c.family) (policy.capability_classifications or [ ]);
  forbiddenFamilies = map (c: c.family) (filter (c: c.core_handling == "forbidden") (policy.capability_classifications or [ ]));
  outbound = policy.outbound_effects or [ ];
  coreNoStd = policy.core_no_std or [ ];
  targetComplete = all (t: t ? triple && t ? features && t ? default_features) declaredTargets;

  diagnostics =
    # One immutable Octet revision across Nix, the lock, and the deny-all hook.
    require (match ".*OnixResearch/octet\\?rev=${rev}.*" flakeNix != null) "flake-octet-revision-mismatch"
    ++ require (lockRev == rev) "flake-lock-octet-revision-mismatch"
    ++ require (lockNarHash == narHash) "flake-lock-octet-nar-mismatch"
    ++ require (match ".*OnixResearch/octet.*" lockUrl != null) "flake-lock-octet-url-mismatch"
    ++ require (preCommitOctetLine != [ ]) "precommit-octet-repo-missing"
    ++ require (preCommitRevLines == [ "    rev: ${rev}" ]) "precommit-octet-revision-mismatch"
    # The full catalog must run across the workspace, all targets, and all features.
    ++ require (any (line: match ".*--workspace.*" line != null) preCommitArgs) "precommit-scope-missing"
    ++ require (any (line: match ".*--all-targets.*" line != null) preCommitArgs) "precommit-targets-missing"
    ++ require (any (line: match ".*--all-features.*" line != null) preCommitArgs) "precommit-features-missing"
    ++ require (metadata.default_scope or [ ] == [ "--workspace" ]) "octet-metadata-scope-mismatch"
    ++ require (
      metadata.cargo_check_args or [ ] == [
        "--all-targets"
        "--all-features"
      ]
    ) "octet-metadata-arguments-mismatch"
    # No disabled lint and no finding budget.
    ++ require (lintTable ? unsafe_code && lintTable.unsafe_code == "forbid") "unsafe-lint-not-forbidden"
    ++ require (all (level: level != "allow" && level != "warn") lintLevels) "lint-downgrade-present"
    ++ require (lintAttributes != [ ]) "lint-table-empty"
    ++ require (policyMode == "gate") "architecture-mode-not-gate"
    ++ require (policyWaivers == [ ]) "architecture-waivers-present"
    ++ require (!(policy ? finding_budget)) "architecture-finding-budget-present"
    # Declared policy structure for VT-M1-05.
    ++ require (all (pkg: elem pkg declaredPackages) requiredPackages) "policy-package-missing"
    ++ require (all (scope: elem scope declaredScopes) requiredScopes) "policy-source-scope-missing"
    ++ require (declaredTargets != [ ]) "policy-target-missing"
    ++ require targetComplete "policy-target-incomplete"
    ++ require (all (pkg: elem "package:${pkg}" (map (r: "package:${r.scope}") roles)) requiredPackages)
      "policy-package-role-missing"
    ++ require (all (scope: elem "source:${scope}" (map (r: "source:${r.scope}") roles)) requiredScopes)
      "policy-source-role-missing"
    ++ require (dependencyRules != [ ]) "policy-dependency-rules-missing"
    ++ require (
      all (family: elem family capabilityFamilies) [
        "environment"
        "observability"
        "unsafe-code"
      ]
    ) "policy-capability-family-missing"
    ++ require (
      all (family: elem family forbiddenFamilies) [
        "environment"
        "observability"
        "unsafe-code"
      ]
    ) "policy-capability-not-forbidden"
    ++ require (outbound != [ ]) "policy-outbound-effects-missing"
    ++ require (all (effect: effect ? authority && effect.authority != "") outbound) "policy-outbound-authority-missing"
    ++ require (all (effect: effect ? executor_scope && effect.executor_scope != "") outbound)
      "policy-outbound-executor-missing"
    ++ require (all (entry: elem entry.scope requiredCorePackages) coreNoStd) "policy-core-no-std-mismatch"
    ++ require (all (scope: any (entry: entry.scope == scope) coreNoStd) requiredCorePackages)
      "policy-core-no-std-missing"
    ++ require (policy ? ports && policy.ports == [ ]) "policy-ports-not-declared-empty"
    ++ require (
      policy ? provider_classifications && policy.provider_classifications == [ ]
    ) "policy-providers-not-declared-empty";
in
{
  inherit diagnostics rev narHash;
  valid = diagnostics == [ ];
  lint_lints = lintAttributes;
  policy_summary = {
    mode = policyMode;
    packages = declaredPackages;
    source_scopes = declaredScopes;
    targets = map (t: t.triple) declaredTargets;
    roles = length roles;
    dependency_rules = length dependencyRules;
    capability_families = sorted capabilityFamilies;
    forbidden_families = sorted forbiddenFamilies;
    outbound_effects = length outbound;
    core_no_std = length coreNoStd;
    ports = length (policy.ports or [ ]);
    providers = length (policy.provider_classifications or [ ]);
  };
  non_claims = [
    "pin-agreement-is-not-compiler-authenticity"
    "policy-mode-check-is-not-enforcement-evidence"
    "policy-structure-is-not-compiler-fact-coverage"
    "not-m1-acceptance"
  ];
}
