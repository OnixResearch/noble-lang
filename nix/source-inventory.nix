# Pure inventory comparator. Reviewed expectations versus compiler-derived data.
# This evaluator makes no whole-project verification or refinement claim.
{
  inventory,
  policy,
  architecturePolicy,
  selection,
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
    substring
    ;
  require = condition: diagnostic: if condition then [ ] else [ diagnostic ];
  sorted = values: sort builtins.lessThan values;
  unique = values: sorted (attrNames (listToAttrs (map (v: { name = v; value = null; }) values)));
  equalSets = left: right: unique left == unique right;
  empty = values: values == [ ];
  pathStartsWith = prefix: path: substring 0 (builtins.stringLength prefix) path == prefix;

  subjects = policy.subjects;
  bodyDispositions = [
    "extracted"
    "modeled"
    "excepted"
    "open"
  ];
  refinements = [
    "proved"
    "open"
  ];
  isBody = s: s.category == "body";
  bodies = filter isBody subjects;
  generated = filter (s: s.category == "generated") subjects;
  structural = filter (s: s.category == "structural") subjects;
  reviewedPaths = unique (map (s: s.qualified_path) subjects);
  observedPaths = unique (map (s: s.qualified_path) inventory.production_subjects);
  observedProduction = listToAttrs (
    map (s: {
      name = s.qualified_path;
      value = s;
    }) inventory.production_subjects
  );
  packageOf =
    path:
    let
      subject = observedProduction.${path};
      owner = filter (u: elem u.unit_id subject.units) inventory.units;
    in
    if owner == [ ] then "<unowned>" else (head owner).package_name;
  entriesBound = filter (
    entry: observedProduction ? ${entry.qualified_path} && packageOf entry.qualified_path != entry.package
  ) subjects;
  edgeKey = e: "${e.caller_package}->${e.callee_package}:${e.edge_kind}";
  observedEdges = sorted (map edgeKey inventory.dependencies.production_edges);
  reviewedEdges = sorted (map (e: "${e.caller}->${e.callee}:${e.edge_kind}") policy.dependencies);
  observedTools = unique inventory.tools;
  reviewedTools = unique (map (t: t.name) policy.tools);
  exceptionSymbols = map (e: e.symbol) policy.exceptions;
  kernelException = e: pathStartsWith "noble_kernel::" e.symbol || e.package == "noble-kernel";
  exceptionComplete =
    e:
    all (field: e ? ${field} && builtins.isString e.${field} && e.${field} != "") [
      "symbol"
      "source_identity"
      "owner"
      "constraint"
      "contract"
      "assumptions"
      "evidence"
      "reassessment"
    ];
  exceptedWithoutRecord = filter (
    s: s.disposition == "excepted" && !(elem s.qualified_path exceptionSymbols)
  ) bodies;
  configurationKey = c: builtins.toJSON {
    target = c.target;
    features = c.features;
    default_features = c.default_features;
  };
  observedConfigurations = sorted (map configurationKey inventory.configurations);
  declaredConfigurations = sorted (
    map (
      row:
      builtins.toJSON {
        target = row.triple;
        features = row.features;
        default_features = row.default_features;
      }
    ) architecturePolicy.production.targets
  );
  declaredPackages = architecturePolicy.production.packages;
  observedPackages = unique (map (u: u.package_name) inventory.units);
  declaredScopes = architecturePolicy.production.source_scopes;
  observedPathsInUnits = map (u: u.source_path) inventory.units;
  countDisposition = disposition: length (filter (s: s.disposition == disposition) bodies);
  counts = {
    compiler_production_items = length inventory.production_subjects;
    reviewed_subjects = length subjects;
    bodies = length bodies;
    generated = length generated;
    structural = length structural;
    extracted = countDisposition "extracted";
    modeled = countDisposition "modeled";
    excepted = countDisposition "excepted";
    open = countDisposition "open";
    proved = length (filter (s: s.refinement == "proved") bodies);
    open_refinement = length (filter (s: s.refinement == "open") bodies);
    test_subjects = length (unique policy.test_subjects);
    macro_origins = length (unique policy.macro_origins);
    units = length inventory.units;
    configurations = length inventory.configurations;
    dependencies = length policy.dependencies;
    tools = length reviewedTools;
    future_components = length policy.future_components;
  };
  diagnostics =
    require (inventory.coverage.status == "complete") "coverage-status"
    ++ require (empty inventory.coverage.missing_units) "coverage-missing-units"
    ++ require (empty inventory.coverage.unexpected_units) "coverage-unexpected-units"
    ++ require (empty inventory.coverage.duplicate_units) "coverage-duplicate-units"
    ++ require (inventory.coverage.expected_units == inventory.coverage.observed_units)
      "coverage-unit-count"
    ++ require (observedConfigurations == declaredConfigurations) "required-configuration"
    ++ require (all (pkg: elem pkg observedPackages) declaredPackages) "package-coverage"
    ++ require (
      all (scope: any (path: pathStartsWith scope path) observedPathsInUnits) declaredScopes
    ) "source-scope-coverage"
    ++ require (
      all (
        s:
        if isBody s then
          elem s.disposition bodyDispositions
        else if s.category == "generated" then
          s.disposition == "generated"
        else
          s.category == "structural" && s.disposition == "structural"
      ) subjects
    ) "subject-disposition"
    ++ require (all (s: !isBody s || elem s.refinement refinements) subjects) "subject-refinement"
    ++ require (all (path: elem path reviewedPaths) observedPaths) "subject-omission"
    ++ require (all (path: elem path observedPaths) reviewedPaths) "subject-stale"
    ++ require (empty entriesBound) "subject-binding"
    ++ require (empty exceptedWithoutRecord) "excepted-subject-without-record"
    ++ require (equalSets policy.test_subjects (map (s: s.qualified_path) inventory.test_subjects))
      "test-subject-classification"
    ++ require (equalSets policy.macro_origins inventory.macro_origins) "macro-classification"
    ++ require (reviewedEdges == observedEdges) "dependency-closure"
    ++ require (equalSets reviewedTools observedTools) "tool-classification"
    ++ require (
      all (c: c.status == "open" && !(elem c.name reviewedPaths)) policy.future_components
    ) "future-component-claim"
    ++ require (all (e: !(kernelException e)) policy.exceptions) "kernel-exception-rejected"
    ++ require (all exceptionComplete policy.exceptions) "exception-incomplete"
    ++ require (counts.bodies + counts.generated + counts.structural == counts.reviewed_subjects)
      "subject-partition"
    ++ require (counts.reviewed_subjects == length observedPaths) "reviewed-scope-partition"
    ++ require (
      counts.bodies == counts.extracted + counts.modeled + counts.excepted + counts.open
    ) "count-partition"
    ++ require (counts.bodies == counts.proved + counts.open_refinement) "refinement-partition";
in
{
  inherit diagnostics counts;
  valid = diagnostics == [ ];
  non_claims = [
    "inventory-classification-is-not-refinement"
    "open-work-is-not-verified"
    "extraction-status-is-not-a-proof"
    "structural-and-generated-subjects-are-accounting-not-noble-bodies"
    "not-whole-project-verification"
  ];
}
