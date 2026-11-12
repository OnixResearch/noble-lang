# Every reviewed policy family, its export, and what reads it.
#
# `evaluator` and `tests` name the path that consumes the family, or "" when
# the repository consumes the export only by hand. The policy check requires
# the registered export set to equal the exported set it verifies, so a new
# family cannot be added without registering it here.
{
  schema_version | String = "noble-policy-registry/v1",
  families = [{
    id | String = "noble.boundary-controls",
    owner | String = "noble-maintainers",
    source | String = "policy/boundary-controls.ncl",
    dependencies | Array String = ["verification/fixtures/randomness.Cargo.lock"],
    generated_output | String = "policy/boundary-controls.json",
    additional_outputs | Array String = [],
    evaluator | String = "nix/boundary-observation.nix",
    tests | String = "nix/boundary-observation-tests.nix",
    kind | String = "test-configuration-not-runtime-receipt",
  }, {
    id | String = "noble.tool-selection-policy",
    owner | String = "noble-maintainers",
    source | String = "policy/tool-selection.ncl",
    dependencies | Array String = [
      "policy/tool-selection-files.ncl",
      "policy/tool-selection-tools.ncl",
      "verification/m5/pins.json",
    ],
    generated_output | String = "policy/tool-selection.json",
    additional_outputs | Array String = [],
    evaluator | String = "nix/tool-selection.nix",
    tests | String = "nix/tool-selection-tests.nix",
    kind | String = "configuration-not-runtime-receipt",
  }, {
    id | String = "octet.project-architecture-policy",
    owner | String = "noble-maintainers",
    source | String = "policy/architecture.ncl",
    dependencies | Array String = [
      "policy/architecture.contracts.ncl",
      "policy/architecture-export-spec.ncl",
    ],
    generated_output | String = "policy/architecture.json",
    additional_outputs | Array String = ["policy/architecture.manifest.json"],
    evaluator | String = "nix/source-inventory.nix",
    tests | String = "nix/source-inventory-tests.nix",
    kind | String = "configuration-not-runtime-receipt",
  }, {
    id | String = "noble.architecture-export-manifest",
    owner | String = "noble-maintainers",
    source | String = "policy/architecture-export-spec.ncl",
    dependencies | Array String = ["policy/architecture.ncl"],
    generated_output | String = "policy/architecture.manifest.json",
    additional_outputs | Array String = [],
    evaluator | String = "",
    tests | String = "",
    kind | String = "generated-export-manifest-not-runtime-receipt",
  }, {
    id | String = "noble.source-inventory-policy",
    owner | String = "noble-maintainers",
    source | String = "policy/source-inventory.ncl",
    dependencies | Array String = ["policy/architecture.json"],
    generated_output | String = "policy/source-inventory.json",
    additional_outputs | Array String = [],
    evaluator | String = "nix/source-inventory.nix",
    tests | String = "nix/source-inventory-tests.nix",
    kind | String = "reviewed-classification-not-compiler-evidence",
  }, {
    id | String = "noble.native-assurance-policy",
    owner | String = "noble-maintainers",
    source | String = "policy/native-assurance.ncl",
    dependencies | Array String = [],
    generated_output | String = "policy/native-assurance.json",
    additional_outputs | Array String = [],
    evaluator | String = "nix/native-assurance.nix",
    tests | String = "nix/native-assurance-tests.nix",
    kind | String = "reviewed-expectation-not-execution-receipt",
  }, {
    id | String = "noble.lean-backend-inputs",
    owner | String = "noble-maintainers",
    source | String = "policy/lean-inputs.ncl",
    dependencies | Array String = ["proofs/m1/lake-manifest.json"],
    generated_output | String = "policy/lean-inputs.json",
    additional_outputs | Array String = [],
    # No flake check consumes this record; it is the reviewed identity list for
    # the manual offline Lean backend lane (`nix/offline-inputs.nix`).
    evaluator | String = "",
    tests | String = "",
    kind | String = "reviewed-pin-record-not-consumed-by-a-check",
  }, {
    id | String = "noble.m1-acceptance-expectation",
    owner | String = "noble-maintainers",
    source | String = "policy/m1-acceptance.ncl",
    dependencies | Array String = [],
    generated_output | String = "policy/m1-acceptance.json",
    additional_outputs | Array String = [],
    # The comparator exists but no flake check evaluates it yet.
    evaluator | String = "nix/m1-acceptance.nix",
    tests | String = "nix/m1-acceptance-tests.nix",
    kind | String = "reviewed-expectation-not-m1-acceptance",
  }],
}
