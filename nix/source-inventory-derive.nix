# Pure derivation of the compiler-derived source inventory.
# Inputs are collector artifacts, the reviewed architecture policy, and the
# reviewed tool-selection policy. This file performs no I/O and makes no claim.
{
  ir,
  coverage,
  cargoGraph,
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
    tail
    ;
  uniqueSorted =
    values:
    sort builtins.lessThan (
      attrNames (listToAttrs (map (value: { name = value; value = null; }) values))
    );
  units = ir.units;
  memberships = ir.unit_memberships;
  unitById = listToAttrs (map (u: { name = u.unit_id; value = u; }) units);
  has = attrs: name: attrs ? ${name};
  ownerUnits =
    factId:
    map (m: m.unit_id) (filter (m: elem factId m.fact_ids) memberships);
  roleOf = unitId: unitById.${unitId}.role;
  # A subject is test-only when every observing unit carries the test role.
  isTestOnly = subject: all (unitId: roleOf unitId == "test") subject.units;
  itemSubjects = map (f: {
    qualified_path = f.qualified_path;
    item_kind = f.item_kind;
    origin_crate = f.origin_crate;
    visibility = f.visibility;
    units = ownerUnits f.fact_id;
  }) (filter (f: f.kind == "item") ir.facts);
  production = filter (s: !isTestOnly s) itemSubjects;
  tests = filter isTestOnly itemSubjects;
  macroOrigins = uniqueSorted (
    map (
      f:
      let
        definition = f.macro_definition or null;
      in
      if definition == null then "<builtin>" else definition
    ) (filter (f: f.kind == "expansion") ir.facts)
  );
  selectedPackages = cargoGraph.selected_packages;
  edges = cargoGraph.edges;
  external = filter (e: !(elem e.callee_package selectedPackages)) edges;
  configurations = uniqueSorted (
    map (
      u:
      builtins.toJSON {
        target = u.target_triple;
        features = u.features;
        default_features = u.default_features;
      }
    ) units
  );
  dependencies = map (e: {
    caller_package = e.caller_package;
    callee_package = e.callee_package;
    edge_kind = e.edge_kind;
  }) edges;
in
{
  schema = "noble-source-inventory/v1";
  generated_by = {
    tool = "cargo-octet";
    toolchain = if length units == 0 then "<none>" else (head units).toolchain_blake3;
    ir_id = ir.architecture_ir_id;
    policy_blake3 = ir.policy_blake3;
    coverage_id = coverage.coverage_id;
    coverage_status = coverage.status;
    cargo_graph_id = cargoGraph.cargo_graph_id;
  };
  coverage = {
    status = coverage.status;
    expected_units = length coverage.expected_unit_ids;
    observed_units = length coverage.observed_unit_ids;
    missing_units = coverage.missing_unit_ids;
    unexpected_units = coverage.unexpected_unit_ids;
    duplicate_units = coverage.duplicate_unit_ids;
  };
  units = map (u: {
    unit_id = u.unit_id;
    package_name = u.package_name;
    target_kind = u.target_kind;
    source_path = u.source_path;
    role = u.role;
    target_triple = u.target_triple;
    features = u.features;
    default_features = u.default_features;
  }) units;
  configurations = map builtins.fromJSON configurations;
  production_subjects = production;
  test_subjects = tests;
  macro_origins = macroOrigins;
  dependencies = {
    selected_packages = selectedPackages;
    production_edges = dependencies;
    external_edges = map (e: { inherit (e) caller_package callee_package edge_kind; }) external;
  };
  tools = attrNames selection.tool_paths;
  future_components = selection.future_unselected;
  counts = {
    units = length units;
    configurations = length configurations;
    production_subjects = length production;
    test_subjects = length tests;
    macro_origins = length macroOrigins;
    production_edges = length dependencies;
    external_edges = length external;
  };
}
