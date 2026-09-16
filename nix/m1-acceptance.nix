# Pure M1 acceptance comparator.
# Independent expectation versus observed phase evidence. Makes no claim itself.
{
  expectation,
  observed,
}:
let
  inherit (builtins)
    all
    any
    attrNames
    elem
    filter
    length
    listToAttrs
    map
    sort
    ;
  require = condition: diagnostic: if condition then [ ] else [ diagnostic ];
  sorted = values: sort builtins.lessThan values;
  unique = values: sorted (attrNames (listToAttrs (map (v: { name = v; value = null; }) values)));

  phases = observed.phases or [ ];
  phaseById = listToAttrs (
    map (p: {
      name = p.id;
      value = p;
    }) phases
  );
  requiredIds = map (p: p.id) expectation.required_phases;
  observedIds = map (p: p.id) phases;

  # A phase is complete only with a passed status and a matching binding.
  phaseDiagnostics = builtins.concatLists (
    map (
      phase:
      let
        same = phaseById.${phase.id} or null;
      in
      if same == null then
        [ "phase-missing:${phase.id}" ]
      else if same.status != "passed" then
        [ "phase-${same.status}:${phase.id}" ]
      else if phase.binding != "none" && (same.binding or "") != phase.binding then
        [ "phase-binding-mismatch:${phase.id}" ]
      else
        [ ]
    ) expectation.required_phases
  );

  unexpectedPhases = filter (id: !(elem id requiredIds)) observedIds;
  subjectOk = (observed.subject.repository or "") == expectation.subject.repository
    && (observed.subject.milestone or "") == expectation.subject.milestone
    && (observed.subject.scope or "") == expectation.subject.scope;
  packagesOk = unique (observed.packages or [ ]) == sorted expectation.required_packages;
  targetOk =
    (observed.target.triple or "") == expectation.required_target.triple
    && (observed.target.features or [ ]) == expectation.required_target.features
    && (observed.target.default_features or false) == expectation.required_target.default_features;
  pinsOk = all (name: (observed.pins.${name} or "") == expectation.pins.${name}) (
    attrNames expectation.pins
  );
  promotions = observed.promoted_roles or [ ];
  promotionOk = all (role: !(elem role expectation.forbidden_promotions)) promotions;
  refinementOk =
    expectation.refinement_required || (observed.refinement or "open") != "proved";
  # The expectation must be the independently recorded one, not producer-supplied.
  authorityOk = (observed.expectation_digest or "") != "producer-supplied";

  counts = {
    required_phases = length expectation.required_phases;
    observed_phases = length phases;
    passed_phases = length (filter (p: p.status == "passed") phases);
    failed_phases = length (filter (p: p.status == "failed") phases);
    unsupported_phases = length (filter (p: p.status == "unsupported") phases);
    timed_out_phases = length (filter (p: p.status == "timed-out") phases);
    forbidden_promotions = length promotions;
  };

  diagnostics =
    require subjectOk "stale-subject"
    ++ require packagesOk "scope-package-mismatch"
    ++ require targetOk "stale-configuration"
    ++ require pinsOk "stale-tool-pin"
    ++ require authorityOk "producer-supplied-expectation"
    ++ phaseDiagnostics
    ++ require (unexpectedPhases == [ ]) "unexpected-phase"
    ++ require promotionOk "evidence-role-promotion"
    ++ require refinementOk "refinement-without-required-proof"
    ++ require (counts.passed_phases == counts.required_phases) "acceptance-incomplete";
in
{
  inherit diagnostics counts;
  valid = diagnostics == [ ];
  accepted_scope = if diagnostics == [ ] then expectation.claimed_scope else null;
  non_claims = [
    "acceptance-is-not-authentication"
    "smoke-scope-is-not-refinement"
    "expectation-is-independent-of-the-receipt"
  ];
}
