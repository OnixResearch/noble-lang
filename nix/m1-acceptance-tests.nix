# Synthetic M1 acceptance controls. Not acceptance evidence.
{ }:
let
  expectation = {
    schema_version = "noble-m1-acceptance-expectation/v1";
    subject = {
      repository = "Noble";
      milestone = "M1";
      scope = "named M1 workspace scope: noble-kernel and noble-cli";
    };
    required_packages = [
      "noble-cli"
      "noble-kernel"
    ];
    required_target = {
      triple = "x86_64-unknown-linux-gnu";
      features = [ ];
      default_features = true;
    };
    pins = {
      octet = "235255bc4972ced9128fd5b4d1ec66ff7508ded4";
      aeneas = "505b6ca35217e7be5c96c3e2f8045edfbdf47291";
      charon = "b104e24fea7d721b71e6c39fd70f26ff20bc0980";
      lean_rev = "68218e876d2a38b1985b8590fff244a83c321783";
      quality_rust = "nightly-2026-03-21";
      extraction_rust = "nightly-2026-08-18";
    };
    required_phases = [
      {
        id = "octet";
        kind = "scoped-check";
        binding = "selection";
      }
      {
        id = "source-coverage";
        kind = "scoped-check";
        binding = "inventory";
      }
      {
        id = "extraction";
        kind = "tool-run";
        binding = "inventory";
      }
    ];
    claimed_scope = "smoke";
    refinement_required = false;
    forbidden_promotions = [
      "lint-as-refinement"
      "miri-as-refinement"
      "generated-lean-as-refinement"
      "receipt-replay-as-refinement"
    ];
  };
  observed = {
    subject = {
      repository = "Noble";
      milestone = "M1";
      scope = "named M1 workspace scope: noble-kernel and noble-cli";
    };
    packages = [
      "noble-cli"
      "noble-kernel"
    ];
    target = {
      triple = "x86_64-unknown-linux-gnu";
      features = [ ];
      default_features = true;
    };
    pins = expectation.pins;
    phases = [
      {
        id = "octet";
        status = "passed";
        binding = "selection";
      }
      {
        id = "source-coverage";
        status = "passed";
        binding = "inventory";
      }
      {
        id = "extraction";
        status = "passed";
        binding = "inventory";
      }
    ];
    promoted_roles = [ ];
    refinement = "open";
    expectation_digest = "b3:recorded-independent-expectation";
  };
  evaluate =
    exp: obs:
    import ./m1-acceptance.nix {
      expectation = exp;
      observed = obs;
    };
  withObserved = change: observed // change;
  withPhase =
    id: change:
    withObserved {
      phases = map (p: if p.id == id then p // change else p) observed.phases;
    };
  reject =
    name: exp: obs: expected:
    let
      result = evaluate exp obs;
    in
    if result.valid then
      throw "control unexpectedly passed: ${name}"
    else if builtins.elem expected result.diagnostics then
      name
    else
      throw "wrong diagnostic for ${name}: ${builtins.toJSON result.diagnostics}";
  tests = [
    (
      assert (evaluate expectation observed).valid;
      "complete independently bound acceptance"
    )
    (
      assert (evaluate expectation observed).accepted_scope == "smoke";
      "accepted scope is the declared smoke scope"
    )
    (reject "stale subject" expectation (withObserved {
      subject = observed.subject // { milestone = "M2"; };
    }) "stale-subject")
    (reject "stale package scope" expectation (withObserved {
      packages = [ "noble-kernel" ];
    }) "scope-package-mismatch")
    (reject "stale configuration" expectation (withObserved {
      target = observed.target // { features = [ "extra" ]; };
    }) "stale-configuration")
    (reject "stale tool pin" expectation (withObserved {
      pins = observed.pins // { octet = "0000000000000000000000000000000000000000"; };
    }) "stale-tool-pin")
    (reject "producer-supplied expectation" expectation (withObserved {
      expectation_digest = "producer-supplied";
    }) "producer-supplied-expectation")
    (reject "missing required phase" expectation (withObserved {
      phases = builtins.filter (p: p.id != "extraction") observed.phases;
    }) "phase-missing:extraction")
    (reject "failed phase" expectation (withPhase "extraction" { status = "failed"; }) "phase-failed:extraction")
    (reject "unsupported phase" expectation (withPhase "extraction" { status = "unsupported"; })
      "phase-unsupported:extraction")
    (reject "timed-out phase" expectation (withPhase "extraction" { status = "timed-out"; })
      "phase-timed-out:extraction")
    (reject "stale phase binding" expectation (withPhase "extraction" { binding = "selection"; })
      "phase-binding-mismatch:extraction")
    (reject "unexpected extra phase" expectation (withObserved {
      phases = observed.phases ++ [
        {
          id = "advisory-smoke";
          status = "passed";
          binding = "none";
        }
      ];
    }) "unexpected-phase")
    (reject "evidence role promotion" expectation (withObserved {
      promoted_roles = [ "generated-lean-as-refinement" ];
    }) "evidence-role-promotion")
    (reject "refinement claimed without proof" expectation (withObserved {
      refinement = "proved";
    }) "refinement-without-required-proof")
    (reject "incomplete matrix" expectation (withPhase "source-coverage" { status = "not-run"; })
      "phase-not-run:source-coverage")
  ];
in
builtins.deepSeq tests tests
