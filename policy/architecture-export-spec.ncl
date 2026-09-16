{
  schema_version = "octet-nickel-export-spec/v1",
  family_id = "octet.project-architecture-policy",
  source = "policy/architecture.ncl",
  dependencies = ["policy/architecture.contracts.ncl"],
  selector = "",
  contract = "ProjectArchitecturePolicy and ProjectArchitecturePolicyRules",
  output_format = "json",
  output_path = "policy/architecture.json",
  allow_secret_material = false,
}
