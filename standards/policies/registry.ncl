{
  schema_version | String = "noble-policy-registry/v1",
  families = [{
    id | String = "noble.boundary-controls",
    owner | String = "noble-maintainers",
    source | String = "policy/boundary-controls.ncl",
    dependencies | Array String = ["verification/fixtures/randomness.Cargo.lock"],
    generated_output | String = "policy/boundary-controls.json",
    evaluator | String = "nix/boundary-observation.nix",
    tests | String = "nix/boundary-observation-tests.nix",
    kind | String = "test-configuration-not-runtime-receipt",
  }, {
    id | String = "noble.tool-selection-policy",
    owner | String = "noble-maintainers",
    source | String = "policy/tool-selection.ncl",
    dependencies | Array String = ["policy/tool-selection-files.ncl", "policy/tool-selection-tools.ncl"],
    generated_output | String = "policy/tool-selection.json",
    evaluator | String = "nix/tool-selection.nix",
    tests | String = "nix/tool-selection-tests.nix",
    kind | String = "configuration-not-runtime-receipt",
  }],
}
