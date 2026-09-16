# Independently bound M1 acceptance expectation.
# Reviewed expectation record, not a receipt. The comparator never rewrites it.
{
  schema_version | String = "noble-m1-acceptance-expectation/v1",
  owner | String = "noble-maintainers",
  subject = {
    repository | String = "Noble",
    milestone | String = "M1",
    task_range | String = "1.1-5.5",
    scope | String = "named M1 workspace scope: noble-kernel and noble-cli",
  },
  required_packages | Array String = [ "noble-kernel", "noble-cli" ],
  required_target = {
    triple | String = "x86_64-unknown-linux-gnu",
    features | Array String = [],
    default_features | Bool = true,
  },
  pins = {
    octet | String = "235255bc4972ced9128fd5b4d1ec66ff7508ded4",
    aeneas | String = "505b6ca35217e7be5c96c3e2f8045edfbdf47291",
    charon | String = "b104e24fea7d721b71e6c39fd70f26ff20bc0980",
    lean_rev | String = "68218e876d2a38b1985b8590fff244a83c321783",
    quality_rust | String = "nightly-2026-03-21",
    extraction_rust | String = "nightly-2026-08-18",
  },
  # Every required phase needs retained evidence with a matching binding.
  required_phases | Array { id | String, kind | String, binding | String } = [
    { id = "format", kind = "scoped-check", binding = "none" },
    { id = "rust-tests", kind = "scoped-check", binding = "none" },
    { id = "clippy", kind = "scoped-check", binding = "none" },
    { id = "documents", kind = "scoped-check", binding = "none" },
    { id = "cairn", kind = "scoped-check", binding = "none" },
    { id = "policy", kind = "scoped-check", binding = "policy" },
    { id = "octet", kind = "scoped-check", binding = "selection" },
    { id = "tool-selection", kind = "scoped-check", binding = "selection" },
    { id = "tool-versions", kind = "scoped-check", binding = "selection" },
    { id = "tool-overrides", kind = "scoped-check", binding = "selection" },
    { id = "boundary-controls", kind = "scoped-check", binding = "selection" },
    { id = "source-coverage", kind = "scoped-check", binding = "inventory" },
    { id = "native-assurance", kind = "scoped-check", binding = "inventory" },
    { id = "extraction", kind = "tool-run", binding = "inventory" },
    { id = "miri", kind = "tool-run", binding = "selection" },
  ],
  # The M1 smoke claim does not assert refinement. A refinement claim must reject.
  claimed_scope | String = "smoke",
  refinement_required | Bool = false,
  forbidden_promotions | Array String = [
    "lint-as-refinement",
    "architecture-as-refinement",
    "miri-as-refinement",
    "generated-lean-as-refinement",
    "receipt-replay-as-refinement",
    "test-evidence-as-production",
    "document-check-as-implementation",
  ],
  non_claims | Array String = [
    "acceptance-is-not-authentication",
    "smoke-scope-is-not-whole-program-correctness",
    "extraction-is-not-refinement",
  ],
}
