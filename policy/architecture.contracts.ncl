# Closed contracts for project-owned FCIS and Hexagonal architecture policy.
let NonEmptyString = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value > 0
) in
let NonEmptyArray = std.contract.from_predicate (fun value =>
  std.is_array value && std.array.length value > 0
) in
let Blake3HexLength = 64 in
let Blake3 = std.contract.from_predicate (fun value =>
  std.is_string value && std.string.length value == Blake3HexLength
) in
let OneOf = fun allowed => std.contract.from_predicate (fun value =>
  std.is_string value && std.array.elem value allowed
) in
let SchemaVersion = OneOf ["octet-project-architecture-policy/v1"] in
let Mode = OneOf ["inventory", "advisory", "gate", "admission"] in
let Role = OneOf [
  "domain-core",
  "application-shell",
  "inbound-adapter",
  "outbound-adapter",
  "composition-root",
  "test",
  "generated",
] in
let ProductionRole = OneOf [
  "domain-core",
  "application-shell",
  "inbound-adapter",
  "outbound-adapter",
  "composition-root",
] in
let AdapterRole = OneOf ["inbound-adapter", "outbound-adapter"] in
let ProviderKind = OneOf ["pure-value", "application-contract", "infrastructure"] in
let ScopeKind = OneOf ["package", "source"] in
let CapabilityFamily = OneOf [
  "time",
  "randomness",
  "filesystem",
  "network",
  "process",
  "environment",
  "secrets",
  "unsafe-code",
  "global-state",
  "concurrency",
  "observability",
  "ffi",
  "allocation",
] in
let CoreCapabilityHandling = OneOf ["explicit-fact", "effect-plan", "forbidden"] in
let CapabilityExecutorRole = OneOf ["application-shell", "outbound-adapter", "composition-root"] in
let Target = {
  triple | NonEmptyString,
  features | Array NonEmptyString,
  default_features | Bool,
} in
let Production = {
  packages | NonEmptyArray | Array NonEmptyString,
  source_scopes | NonEmptyArray | Array NonEmptyString,
  targets | NonEmptyArray | Array Target,
} in
let RoleAssignment = {
  scope_kind | ScopeKind,
  scope | NonEmptyString,
  role | Role,
} in
let DependencyRule = {
  caller_role | ProductionRole,
  allowed_callee_roles | Array ProductionRole,
  edge_kinds | NonEmptyArray | Array NonEmptyString,
} in
let ProviderClassification = {
  package | NonEmptyString,
  kind | ProviderKind,
  permitted_caller_roles | NonEmptyArray | Array ProductionRole,
  edge_kinds | NonEmptyArray | Array NonEmptyString,
  rationale | NonEmptyString,
} in
let Port = {
  port_trait | NonEmptyString,
  application_owner_scope | NonEmptyString,
  permitted_adapter_scopes | Array NonEmptyString,
  forbidden_concrete_types | Array NonEmptyString | default = [],
  reserved | Bool,
} in
let CapabilityClassification = {
  family | CapabilityFamily,
  core_handling | CoreCapabilityHandling,
  executor_role | CapabilityExecutorRole,
  rationale | NonEmptyString,
} in
let EffectAuthority = OneOf ["unprotected", "protected"] in
let WitnessUse = OneOf ["consume", "borrow"] in
let ProtectedEffectProtocol = {
  decision_scope | NonEmptyString,
  witness_owner_scope | NonEmptyString,
  receipt_scope | NonEmptyString,
  authority_input_types | NonEmptyArray | Array NonEmptyString,
  decision_functions | NonEmptyArray | Array NonEmptyString,
  authorized_output_types | NonEmptyArray | Array NonEmptyString,
  witness_type | NonEmptyString,
  witness_constructors | NonEmptyArray | Array NonEmptyString,
  witness_binding_types | NonEmptyArray | Array NonEmptyString,
  plan_type | NonEmptyString,
  attempt_type | NonEmptyString,
  observed_outcome_type | NonEmptyString,
  receipt_type | NonEmptyString,
  receipt_constructors | NonEmptyArray | Array NonEmptyString,
  failure_receipt_constructors | default = [] | Array NonEmptyString,
  success_observation_constructors | NonEmptyArray | Array NonEmptyString,
  failure_observation_constructors | NonEmptyArray | Array NonEmptyString,
  witness_use | WitnessUse,
  borrow_scope | default = null,
  borrow_rationale | default = null,
  non_claims | NonEmptyArray | Array NonEmptyString,
} in
let OutboundEffectClassification = {
  family | CapabilityFamily,
  executor_scope | NonEmptyString,
  executor_methods | NonEmptyArray | Array NonEmptyString,
  authority | EffectAuthority,
  rationale | NonEmptyString,
  protected | default = null,
} in
let NoStdObligation = {
  scope | NonEmptyString,
} in
let AdmissionBinding = {
  strict_profile | OneOf ["deterministic-core"],
  effect_closure_policy | NonEmptyString,
} in
let Waiver = {
  finding_id | NonEmptyString,
  policy_blake3 | Blake3,
  source_blake3 | Blake3,
  owner | NonEmptyString,
  reason | NonEmptyString,
  review_condition | NonEmptyString,
  affected_claim | NonEmptyString,
} in
let Positive = std.contract.from_predicate (fun value => std.is_number value && value > 0) in
let CollectionLimits = {
  max_units | Positive,
  max_shards | Positive,
  max_modules | Positive,
  max_items | Positive,
  max_types | Positive,
  max_traits | Positive,
  max_implementations | Positive,
  max_signatures | Positive,
  max_calls | Positive,
  max_constructors | Positive,
  max_effects | Positive,
  max_expansions | Positive,
  max_configurations | Positive,
  max_type_shapes | Positive,
  max_function_safety_facts | Positive,
  max_value_provenance_facts | Positive,
  max_value_flow_bodies | Positive,
  max_value_flow_steps | Positive,
  max_value_flow_rounds | Positive,
  max_unknowns | Positive,
  max_spans | Positive,
  max_span_bytes | Positive,
  max_shard_bytes | Positive,
  max_artifact_bytes | Positive,
} in
let ProjectArchitecturePolicyContract = {
  schema_version | SchemaVersion,
  project_id | NonEmptyString,
  mode | Mode,
  production | Production,
  roles | NonEmptyArray | Array RoleAssignment,
  dependency_rules | NonEmptyArray | Array DependencyRule,
  provider_classifications | Array ProviderClassification | default = [],
  ports | Array Port | default = [],
  capability_classifications | NonEmptyArray | Array CapabilityClassification,
  outbound_effects | Array OutboundEffectClassification | default = [],
  core_no_std | Array NoStdObligation | default = [],
  admission | default = null,
  waivers | Array Waiver | default = [],
  collection_limits | CollectionLimits,
  non_claims | NonEmptyArray | Array NonEmptyString,
} in
let has_duplicates = fun values =>
  std.array.length values != std.array.length (std.array.dedup values)
in
let assignment_key = fun assignment => assignment.scope_kind ++ ":" ++ assignment.scope in
let production_keys = fun policy =>
  (policy.production.packages |> std.array.map (fun package => "package:" ++ package))
  @ (policy.production.source_scopes |> std.array.map (fun scope => "source:" ++ scope))
in
let assignment_for = fun key assignments =>
  assignments |> std.array.filter (fun assignment => assignment_key assignment == key)
in
let role_for_source = fun scope assignments =>
  let matching = assignment_for ("source:" ++ scope) assignments in
  if std.array.length matching == 1 then (std.array.first matching).role else ""
in
let role_for_scope = fun scope assignments =>
  let matching = assignments |> std.array.filter (fun assignment => assignment.scope == scope) in
  if std.array.length matching == 1 then (std.array.first matching).role else ""
in
let production_roles = fun policy =>
  policy.roles
  |> std.array.filter (fun assignment => std.array.elem (assignment_key assignment) (production_keys policy))
  |> std.array.map (fun assignment => assignment.role)
  |> std.array.dedup
in
let path_components = fun path => std.string.split "/" path in
let workspace_path_valid = fun path =>
  let components = path_components path in
  std.string.length path > 0
  && path != "."
  && std.array.length components > 0
  && std.array.first components != ""
  && !std.array.elem ".." components
in
let array_is_prefix = fun left right =>
  let left_length = std.array.length left in
  left_length <= std.array.length right
  && (
    std.array.generate (fun index => index) left_length
    |> std.array.all (fun index => std.array.at index left == std.array.at index right)
  )
in
let source_scopes_disjoint = fun roles =>
  let scopes = roles
    |> std.array.filter (fun assignment => assignment.scope_kind == "source")
    |> std.array.map (fun assignment => assignment.scope)
  in
  let indexes = std.array.generate (fun index => index) (std.array.length scopes) in
  std.array.all (fun left_index =>
    std.array.all (fun right_index =>
      if left_index == right_index then true else
        let left = path_components (std.array.at left_index scopes) in
        let right = path_components (std.array.at right_index scopes) in
        !(array_is_prefix left right || array_is_prefix right left)
    ) indexes
  ) indexes
in
let rules_valid = fun policy =>
  let expected_keys = production_keys policy in
  let assignment_keys = policy.roles |> std.array.map assignment_key in
  let caller_roles = policy.dependency_rules |> std.array.map (fun rule => rule.caller_role) in
  !has_duplicates policy.production.packages
  && !has_duplicates policy.production.source_scopes
  && std.array.all workspace_path_valid policy.production.source_scopes
  && std.array.all (fun assignment =>
    assignment.scope_kind != "source" || workspace_path_valid assignment.scope
  ) policy.roles
  && source_scopes_disjoint policy.roles
  && !has_duplicates assignment_keys
  && std.array.all (fun key => std.array.length (assignment_for key policy.roles) == 1) expected_keys
  && std.array.all (fun assignment =>
    let production = std.array.elem (assignment_key assignment) expected_keys in
    if production then
      std.array.elem assignment.role [
        "domain-core",
        "application-shell",
        "inbound-adapter",
        "outbound-adapter",
        "composition-root",
      ]
    else
      std.array.elem assignment.role ["test", "generated"]
  ) policy.roles
  && !has_duplicates caller_roles
  && std.array.all (fun role => std.array.elem role caller_roles) (production_roles policy)
  && !has_duplicates (policy.provider_classifications |> std.array.map (fun provider => provider.package))
  && std.array.all (fun provider =>
    !std.array.elem provider.package policy.production.packages
    && std.array.length provider.permitted_caller_roles > 0
    && std.array.length provider.edge_kinds > 0
  ) policy.provider_classifications
  && !has_duplicates (policy.ports |> std.array.map (fun port => port.port_trait))
  && std.array.all (fun port =>
    role_for_source port.application_owner_scope policy.roles == "application-shell"
    && (port.reserved || std.array.length port.permitted_adapter_scopes > 0)
    && std.array.all (fun scope =>
      std.array.elem (role_for_source scope policy.roles) ["inbound-adapter", "outbound-adapter"]
    ) port.permitted_adapter_scopes
  ) policy.ports
  && !has_duplicates (policy.capability_classifications |> std.array.map (fun item => item.family))
  && !has_duplicates (policy.outbound_effects |> std.array.map (fun item => item.family))
  && std.array.all (fun item =>
    std.array.elem (role_for_source item.executor_scope policy.roles) [
      "application-shell", "outbound-adapter", "composition-root",
    ]
    && std.array.length item.executor_methods > 0
    && (
      if item.authority == "unprotected" then item.protected == null else
        item.protected != null
        && (item.protected | ProtectedEffectProtocol) == item.protected
        && role_for_source item.protected.decision_scope policy.roles == "domain-core"
        && role_for_source item.protected.witness_owner_scope policy.roles == "domain-core"
        && role_for_source item.protected.receipt_scope policy.roles == "application-shell"
        && (
          if item.protected.witness_use == "consume" then
            item.protected.borrow_scope == null && item.protected.borrow_rationale == null
          else
            item.protected.borrow_scope != null && item.protected.borrow_rationale != null
        )
    )
  ) policy.outbound_effects
  && std.array.all (fun obligation =>
    role_for_scope obligation.scope policy.roles == "domain-core"
  ) policy.core_no_std
  && (
    if policy.mode == "admission" then
      policy.admission != null
      && (policy.admission | AdmissionBinding) == policy.admission
      && workspace_path_valid policy.admission.effect_closure_policy
    else
      policy.admission == null
  )
  && !has_duplicates (policy.waivers |> std.array.map (fun waiver => waiver.finding_id))
  && policy.collection_limits.max_shards >= policy.collection_limits.max_units
in
let ProjectArchitecturePolicyRulesContract = std.contract.from_predicate rules_valid in
{
  ProjectArchitecturePolicy = ProjectArchitecturePolicyContract,
  ProjectArchitecturePolicyRules = ProjectArchitecturePolicyRulesContract,
  Mode = Mode,
  Role = Role,
  AdapterRole = AdapterRole,
  CapabilityFamily = CapabilityFamily,
}
