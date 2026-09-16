# Pure native and dependency assurance comparator.
# Reviewed expectations against compiler and Cargo facts. No soundness claim.
{
  inventory,
  policy,
}:
let
  inherit (builtins)
    all
    any
    attrNames
    elem
    filter
    head
    length
    listToAttrs
    map
    sort
    ;
  require = condition: diagnostic: if condition then [ ] else [ diagnostic ];
  sorted = values: sort builtins.lessThan values;
  unique = values: sorted (attrNames (listToAttrs (map (v: { name = v; value = null; }) values)));
  empty = values: values == [ ];

  scopePackages = unique inventory.scope.packages;
  requiredPackages = policy.required_packages;
  requiredTargets = policy.required_targets;
  targetObserved = any (
    row: row.triple == inventory.scope.target_triple
      && row.features == inventory.scope.features
      && row.default_features == inventory.scope.default_features
  ) requiredTargets;

  # An empty owned-unsafe scope requires positive accounting evidence.
  basis = policy.owned_unsafe.accounting_basis;
  accountingPresent = all (name: elem name (unique basis)) [
    "workspace-lint-unsafe-code-forbid"
    "compiler-function-safety-facts-absent"
    "compiler-foreign-item-facts-absent"
  ];
  emptyOwnedUnsafeClaim =
    policy.owned_unsafe.expectation == "empty"
    && empty inventory.owned_unsafe.sites
    && policy.owned_unsafe.sites == [ ];
  ownedUnsafeRecorded = map (s: s.symbol) policy.owned_unsafe.sites;
  observedUnsafe = map (s: s.qualified_path) inventory.owned_unsafe.sites;

  edgeKey = e: "${e.caller_package}->${e.callee_package}:${e.edge_kind}";
  observedProduction = sorted (map edgeKey inventory.dependencies.production_edges);
  reviewedProduction = sorted (
    map (d: "${d.role}->${d.name}:${d.edge_kind or "normal"}") policy.dependencies.production
  );
  fixtureObserved = unique (map (p: "${p.name} ${p.version}") inventory.dependencies.fixture_packages);
  fixtureReviewed = unique (map (p: "${p.name} ${p.version}") policy.dependencies.fixture_only);  toolObserved = unique inventory.compiler_tools;
  toolReviewed = unique (map (t: t.name) policy.compiler_tools);
  foreignObserved = unique inventory.foreign_libraries;
  foreignReviewed = unique (map (f: f.name) policy.foreign_libraries);
  safetyScopes = unique (map (s: s.scope) policy.safety_arguments);
  assumptionTargets = unique (map (t: t.target) policy.target_assumptions);
  miriUnsupported = policy.miri.unsupported_configurations;

  counts = {
    units = inventory.counts.units;
    production_items = inventory.counts.production_items;
    function_safety_facts = inventory.counts.function_safety_facts;
    unsafe_sites = inventory.counts.unsafe_sites;
    foreign_items = inventory.counts.foreign_items;
    production_dependencies = inventory.counts.production_dependencies;
    fixture_packages = inventory.counts.fixture_packages;
    reviewed_unsafe_sites = length policy.owned_unsafe.sites;
    compiler_tools = length toolReviewed;
    foreign_libraries = length foreignReviewed;
    safety_arguments = length policy.safety_arguments;
    target_assumptions = length policy.target_assumptions;
    required_miri_tests = length policy.miri.required_tests;
  };

  diagnostics =
    require (all (pkg: elem pkg scopePackages) requiredPackages) "scope-package-missing"
    ++ require targetObserved "scope-target-missing"
    ++ require (
      policy.owned_unsafe.expectation == "empty" -> inventory.owned_unsafe.workspace_forbids_unsafe
    ) "unsafe-forbid-evidence-missing"
    ++ require (
      policy.owned_unsafe.expectation == "empty" -> accountingPresent
    ) "unsafe-accounting-basis-missing"
    ++ require (
      policy.owned_unsafe.expectation == "empty" -> inventory.owned_unsafe.function_safety_facts == 0
    ) "unsafe-function-facts-present"
    ++ require (
      policy.owned_unsafe.expectation == "empty" -> empty inventory.owned_unsafe.foreign_items
    ) "unsafe-foreign-items-present"
    ++ require (
      policy.owned_unsafe.expectation == "empty" -> empty inventory.owned_unsafe.sites
    ) "owned-unsafe-without-record"
    ++ require (
      all (path: elem path observedUnsafe) ownedUnsafeRecorded
    ) "reviewed-unsafe-site-absent"
    ++ require (observedProduction == reviewedProduction) "production-dependency-mismatch"
    ++ require (
      all (entry: elem entry fixtureObserved) fixtureReviewed
    ) "fixture-dependency-missing"
    ++ require (all (p: p.lock != "") policy.dependencies.fixture_only) "fixture-lock-unrecorded"
    ++ require (toolObserved == toolReviewed) "compiler-tool-mismatch"
    ++ require (foreignObserved == foreignReviewed) "foreign-library-mismatch"
    ++ require (
      all (pkg: elem pkg safetyScopes) requiredPackages
    ) "safety-argument-missing"
    ++ require (
      all (row: elem row.triple assumptionTargets) requiredTargets
    ) "target-assumption-missing"
    ++ require policy.miri.required "miri-not-required"
    ++ require (empty miriUnsupported) "miri-unsupported-configuration"
    ++ require (policy.miri.flags != [ ]) "miri-flags-missing"
    ++ require (policy.miri.subject != "") "miri-subject-unrecorded"
    ++ require (policy.miri.required_tests != [ ]) "miri-tests-missing"
    ++ require (
      policy.miri.toolchain == "nightly-2026-08-18" && policy.miri.target == "x86_64-unknown-linux-gnu"
    ) "miri-configuration-mismatch"
    ++ require (
      all (s: s.argument != "") policy.safety_arguments
    ) "safety-argument-empty"
    ++ require (
      all (t: t.assumptions != [ ]) policy.target_assumptions
    ) "target-assumption-empty";
in
{
  inherit diagnostics counts;
  valid = diagnostics == [ ];
  non_claims = [
    "native-assurance-inventory-is-not-soundness"
    "dependency-records-are-not-dependency-safety"
    "miri-results-are-not-refinement"
    "empty-owned-unsafe-scope-requires-accounting"
    "not-m1-acceptance"
  ];
}
