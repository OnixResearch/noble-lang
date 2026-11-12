# Pure derivation of the native and dependency inventory from compiler and Cargo facts.
# No I/O, no claim: this is accounting input for the reviewed comparator.
{
  ir,
  cargoGraph,
  workspaceToml,
  fixtureLock,
  selection,
}:
let
  inherit (builtins)
    all
    any
    attrNames
    concatStringsSep
    elem
    filter
    length
    map
    match
    sort
    ;
  uniqueSorted =
    values:
    sort builtins.lessThan (
      attrNames (builtins.listToAttrs (map (v: { name = v; value = null; }) values))
    );
  selectedPackages = cargoGraph.selected_packages;
  edges = cargoGraph.edges;
  productionEdges = filter (e: !(elem e.callee_package selectedPackages)) edges;
  unsafeFacts = filter (f: f.kind == "function-safety" && f.is_unsafe) ir.facts;
  safetyFacts = filter (f: f.kind == "function-safety") ir.facts;
  foreignItems = filter (f: f.kind == "item" && f.item_kind == "foreign") ir.facts;
  productionItems = filter (f: f.kind == "item") ir.facts;
  itemKinds = sort builtins.lessThan (
    attrNames (
      builtins.listToAttrs (
        map (f: {
          name = f.item_kind;
          value = null;
        }) productionItems
      )
    )
  );
  tomlLines = filter builtins.isString (builtins.split "\n" workspaceToml);
  forbidsUnsafe = any (
    line: builtins.match "[[:space:]]*unsafe_code = \"forbid\"[[:space:]]*" line != null
  ) tomlLines;
  fixturePackages =
    if fixtureLock == null then
      [ ]
    else
      (builtins.fromTOML fixtureLock).package or [ ];
  # The configurations the inventory was actually collected with: one row per
  # distinct compiler unit configuration. These are compiler facts, never a copy
  # of the reviewed policy (the comparator matches policy against them).
  unitConfigurations = map builtins.fromJSON (
    uniqueSorted (
      map (
        u:
        builtins.toJSON {
          target = u.target_triple;
          inherit (u) features default_features;
        }
      ) ir.units
    )
  );
in
{
  schema = "noble-native-assurance-inventory/v1";
  scope = {
    packages = sort builtins.lessThan (map (u: u.package_name) ir.units);
    targets = unitConfigurations;
    units = length ir.units;
    ir_id = ir.architecture_ir_id;
    policy_blake3 = ir.policy_blake3;
    coverage_id = null;
  };
  owned_unsafe = {
    workspace_forbids_unsafe = forbidsUnsafe;
    sites = map (f: {
      qualified_path = f.qualified_path;
      is_unsafe = f.is_unsafe;
    }) unsafeFacts;
    function_safety_facts = length safetyFacts;
    foreign_items = map (f: f.qualified_path) foreignItems;
    production_items = length productionItems;
    item_kinds = itemKinds;
  };
  dependencies = {
    selected_packages = selectedPackages;
    production_edges = map (e: {
      caller_package = e.caller_package;
      callee_package = e.callee_package;
      edge_kind = e.edge_kind;
    }) productionEdges;
    external_edges = map (e: {
      caller_package = e.caller_package;
      callee_package = e.callee_package;
      edge_kind = e.edge_kind;
    }) productionEdges;
    fixture_packages = map (p: {
      name = p.name;
      version = p.version;
    }) fixturePackages;
  };
  # Declared requirements, not observations: this derivation runs no compiler
  # tool and records no execution. A passing comparator says the declaration
  # matches the reviewed policy, never that a tool ran.
  compiler_tools = {
    provenance = "declared-requirement-not-observed-execution";
    declared = [
      "cargo-octet"
      "charon"
      "aeneas"
      "lean"
      "miri"
    ];
    executions = [ ];
  };
  miri = {
    provenance = "declared-requirement-not-observed-execution";
    executions = [ ];
  };
  foreign_libraries = [ ];
  counts = {
    units = length ir.units;
    production_items = length productionItems;
    function_safety_facts = length safetyFacts;
    unsafe_sites = length unsafeFacts;
    foreign_items = length foreignItems;
    production_dependencies = length productionEdges;
    fixture_packages = length fixturePackages;
  };
}
