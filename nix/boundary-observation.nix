# Pure control evaluator over provider-owned DTOs. Full artifact replay is a
# separate mandatory prerequisite; this comparator does not authenticate inputs.
{
  policy,
  selection,
  case,
  observed,
}:
let
  inherit (builtins)
    all
    any
    elem
    filter
    map
    sort
    ;
  require = condition: diagnostic: if condition then [ ] else [ diagnostic ];
  # Exact literal substring search. Nix has no plain `hasInfix` here, and a
  # regex would treat diagnostic punctuation as metacharacters.
  hasInfix =
    needle: text:
    let
      n = builtins.stringLength needle;
      t = builtins.stringLength text;
    in
    if n == 0 then
      true
    else if t < n then
      false
    else if builtins.substring 0 n text == needle then
      true
    else
      hasInfix needle (builtins.substring 1 (t - 1) text);
  positive = case.observation == "clean";
  compilerOnly = case.observation == "compiler_only";
  roleEdge = case.observation == "role_edge";
  policyBlocked = case.observation == "policy";
  precollection = case.observation == "precollection";
  ds = observed.deny_status;
  lint =
    f:
    f.lint == case.deny_lint
    && f.severity == "error"
    && f.file == (if case.deny_source == "" then policy.kernel_source else case.deny_source)
    && f.crate_name == "noble_kernel";
  denyValid =
    if positive then
      observed.deny_exit == 0
      && ds.status == "clean"
      && ds.error_findings == 0
      && ds.warning_findings == 0
      && ds.cargo_process_exit.code == 0
    else if roleEdge then
      observed.deny_exit == 1
      && ds.cargo_process_exit.code == 0
      && ds.phases.architecture.status == "blocked"
      && ds.phases.lint.status == "clean"
    else if policyBlocked && case.deny_lint == "" then
      observed.deny_exit == 1
      && ds.cargo_process_exit.code == 0
      && ds.phases.architecture.status == "blocked"
      && ds.phases.lint.status == "clean"
    else if precollection then
      observed.deny_exit == 2
      && observed.precollection.exit == 2
      && hasInfix case.expected observed.precollection.diagnostic
      && hasInfix case.expected_secondary observed.precollection.diagnostic
      && hasInfix policy.test_source.escaped observed.precollection.diagnostic
      && !observed.precollection.status_published
    else
      observed.deny_exit == 2
      && ds.status == "integration-failure"
      && ds.cargo_process_exit.code == 101
      && any lint observed.deny_findings
      && all (
        f: f.severity != "error" || f.group != "tigerstyle::unknown" || f.lint == "unsafe_code"
      ) observed.deny_findings;
  ir = observed.ir;
  receipt = observed.receipt;
  units = ir.units;
  roster = map (u: "${u.package_name}:${u.target_kind}:${u.source_path}:${u.role}") units;
  expectedRoster =
    policy.units ++ (if case.base == "acyclic_shell" then policy.acyclic_extra_units else [ ]);
  kernels = filter (u: u.package_name == "noble-kernel" && u.role == "domain-core") units;
  owned =
    u: fact: any (m: m.unit_id == u.unit_id && elem fact.fact_id m.fact_ids) ir.unit_memberships;
  factsFor = u: filter (f: owned u f) ir.facts;
  itemPresent =
    u: any (f: f.kind == "item" && f.qualified_path == "noble_kernel::consume_budget") (factsFor u);
  expectedFact =
    f:
    if case.observation == "effect" then
      f.kind == "effect" && f.capability_family == case.expected
    else if case.observation == "call" then
      f.kind == "call" && f.resolved_definition == case.expected
    else if case.observation == "type" then
      f.kind == "type" && f.origin_crate == "std" && f.definition_path == case.expected
    else
      true;
  effectFinding =
    u:
    any (
      f:
      f.code == "domain-core-effect"
      && f.status == "open"
      && f.package_name == "noble-kernel"
      && f.unit_id == u.unit_id
      && f.source_path == policy.kernel_source
      && any (fact: fact.fact_id == f.fact_id && expectedFact fact) (factsFor u)
    ) receipt.findings;
  edgeFinding = any (
    f:
    f.code == "forbidden-role-edge"
    && f.status == "open"
    && f.package_name == "noble-kernel"
    && (case.edge_message == "" || f.message == case.edge_message)
    && any (u: u.unit_id == f.unit_id) kernels
  ) receipt.findings;
  observationValid =
    if positive then
      receipt.status == "clean" && receipt.findings == [ ]
    else if roleEdge then
      receipt.status == "blocked" && edgeFinding
    else if precollection then
      # No artifact bundle exists. Rejection happens before collection.
      true
    else if policyBlocked then
      receipt.status == "blocked"
      && any (
        f:
        f.code == case.expected
        && f.status == "open"
        && (case.expected_scope == "" || f.source_path == case.expected_scope)
        && (case.expected_message == "" || hasInfix case.expected_message (f.message or ""))
      ) receipt.findings
    else
      all (u: any expectedFact (factsFor u)) kernels
      && (case.observation != "effect" || (receipt.status == "blocked" && all effectFinding kernels));
  diagnostics =
    require denyValid "required-deny-all-result"
    ++ (
      if precollection then
        [ ]
      else
        require (
          ds.metadata.toolchain == "${selection.configuration.quality_rust}-${selection.configuration.target}"
        ) "deny-toolchain"
    )
    ++ (
      if compilerOnly || precollection then
        [ ]
      else
        require (observed.replay.status == "valid") "artifact-replay"
        ++ require (
          ir.schema_version == "octet-compiler-architecture-ir/v2"
          && receipt.schema_version == "octet-project-architecture-receipt/v2"
        ) "provider-schema"
        ++ require (
          sort builtins.lessThan roster == sort builtins.lessThan expectedRoster
        ) "compiler-unit-roster"
        ++ require (all (
          u:
          u.target_triple == selection.configuration.target
          && u.features == selection.configuration.features
          && u.default_features == selection.configuration.default_features
        ) units) "compiler-configuration"
        ++ require (builtins.length kernels == 2 && all itemPresent kernels) "kernel-ownership"
        ++ require (
          receipt.mode == "gate"
          && elem receipt.status [
            "clean"
            "blocked"
          ]
        ) "architecture-mode"
        ++ require (
          observed.observation_status.cargo_process_exit.code == 0
          && observed.observation_exit == observed.observation_status.exit_code
          && observed.observation_exit == (if receipt.status == "clean" then 0 else 1)
        ) "observation-process"
        ++ require observationValid "expected-boundary-observation"
    );
in
{
  inherit diagnostics;
  valid = diagnostics == [ ];
}
