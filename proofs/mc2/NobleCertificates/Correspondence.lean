import NobleCertificates.Rules
import NobleContractImpl.Funs

/-!
Correspondence with the actual whole-crate Charon/Aeneas translation.

The semantic certificate relation does not itself prove the Rust replay
interpreter sound. These lemmas establish the explicitly stated finite decoder,
registry eligibility and wrapper-layout contracts. The compilation audit also
requires every authored companion body and the host verifier parameter.
-/

open Aeneas Aeneas.Std Aeneas.Std.Result

namespace NobleCertificates.Correspondence

set_option maxHeartbeats 1000000
set_option maxRecDepth 2048
set_option Aeneas.customDoElab false

def rustRule : RuleId → noble_contracts.companion.rules.RuleId
  | .admitLeanV1 => .AdmitLeanV1
  | .composeV1 => .ComposeV1
  | .instantiateV1 => .InstantiateV1
  | .guardV1 => .GuardV1
  | .projectV1 => .ProjectV1
  | .invokeV1 => .InvokeV1

def ruleCode : RuleId → U32
  | .admitLeanV1 => 1#u32
  | .composeV1 => 2#u32
  | .instantiateV1 => 3#u32
  | .guardV1 => 4#u32
  | .projectV1 => 5#u32
  | .invokeV1 => 6#u32

theorem rule_decode_roundtrip (rule : RuleId) :
    noble_contracts.companion.rules.RuleId.decode { ruleset := 1#u32, code := ruleCode rule } =
      ok (core.result.Result.Ok (rustRule rule)) := by
  cases rule <;> simp [noble_contracts.companion.rules.RuleId.decode,
    noble_contracts.companion.rules.RULESET_V1, ruleCode, rustRule,
    U32.ofNat, UScalar.ofNat, UScalar.ofNatCore]

/-- An unknown version is refused before considering even a known rule tag. -/
theorem rule_decode_bad_version (version code : U32) (unsupported : version ≠ 1#u32) :
    noble_contracts.companion.rules.RuleId.decode { ruleset := version, code } =
      ok (core.result.Result.Err .UnsupportedRuleset) := by
  simp [noble_contracts.companion.rules.RuleId.decode,
    noble_contracts.companion.rules.RULESET_V1, unsupported]

/-- The finite decoder has no catch-all rule or successful unknown-tag path. -/
theorem rule_decode_unknown (code : U32)
    (h₁ : code ≠ 1#u32) (h₂ : code ≠ 2#u32) (h₃ : code ≠ 3#u32)
    (h₄ : code ≠ 4#u32) (h₅ : code ≠ 5#u32) (h₆ : code ≠ 6#u32) :
    noble_contracts.companion.rules.RuleId.decode { ruleset := 1#u32, code } =
      ok (core.result.Result.Err .UnknownRule) := by
  simp [noble_contracts.companion.rules.RuleId.decode,
    noble_contracts.companion.rules.RULESET_V1]
  split <;> simp_all only [U32.ofNat, UScalar.ofNat, UScalar.ofNatCore]
  all_goals first
    | exact (h₁ rfl).elim
    | exact (h₂ rfl).elim
    | exact (h₃ rfl).elim
    | exact (h₄ rfl).elim
    | exact (h₅ rfl).elim
    | exact (h₆ rfl).elim

theorem rule_code_corresponds (rule : RuleId) :
    noble_contracts.companion.rules.RuleId.code (rustRule rule) =
      ok (ruleCode rule) := by
  cases rule <;> rfl

def rustGuard : GuardTemplate → noble_contracts.companion.GuardTemplate
  | .ltI64Max => .LtI64Max
  | .neI64Min => .NeI64Min
  | .eqI64Literal value => .EqI64Literal ⟨value⟩

def semanticGuard : noble_contracts.companion.GuardTemplate → GuardTemplate
  | .LtI64Max => .ltI64Max
  | .NeI64Min => .neI64Min
  | .EqI64Literal value => .eqI64Literal value.bv

/-- This representation transport preserves every capture bit, including
the signed minimum, rather than assuming an injective finite digest. -/
theorem guard_template_roundtrip (template : GuardTemplate) :
    semanticGuard (rustGuard template) = template := by
  cases template <;> rfl

/-- Once the actual registry lookups select these entries, successful release
requires their retained outcome to be `Proved`. The lookup equations bind the
claim to the real registry operations, not to a handwritten registry model. -/
theorem release_requires_proved
    (engine : noble_contracts.companion.Core)
    (id : noble_contracts.companion.registry.EvidenceId)
    (entry : noble_contracts.companion.registry.EvidenceEntry)
    (contract : noble_contracts.companion.registry.ContractEntry)
    (released : noble_contracts.companion.Release)
    (evidenceLookup : noble_contracts.companion.registry.entries.Registry.evidence
      engine.registry id = ok (.Ok entry))
    (contractLookup : noble_contracts.companion.registry.entries.Registry.contract
      engine.registry entry.contract = ok (.Ok contract))
    (success : noble_contracts.companion.core.Core.release engine id = ok (.Ok released)) :
    entry.outcome = .Proved := by
  cases outcome : entry.outcome <;>
    simp_all [noble_contracts.companion.core.Core.release,
      noble_contracts.companion.release.select,
      noble_contracts.companion.registry.application.proved,
      noble_contracts.companion.registry.entries.Registry.stale,
      core.cmp.PartialEq.ne.trait_default]
  all_goals
    split_ifs at success <;>
      simp_all [core.cmp.PartialEq.ne.default,
        noble_contracts.companion.admit.Outcome.Insts.CoreCmpPartialEqOutcome.eq,
        noble_contracts.companion.admit.Outcome.read_discriminant]
  all_goals split_ifs at success <;> simp_all

/-- A real registry assumption cannot be promoted to a released obligation,
even if an attacker pairs that evidence class with a `Proved` outcome. -/
theorem release_rejects_assumptions
    (engine : noble_contracts.companion.Core)
    (id : noble_contracts.companion.registry.EvidenceId)
    (entry : noble_contracts.companion.registry.EvidenceEntry)
    (contract : noble_contracts.companion.registry.ContractEntry)
    (released : noble_contracts.companion.Release)
    (evidenceLookup : noble_contracts.companion.registry.entries.Registry.evidence
      engine.registry id = ok (.Ok entry))
    (contractLookup : noble_contracts.companion.registry.entries.Registry.contract
      engine.registry entry.contract = ok (.Ok contract))
    (assumptionClass : entry.class = .Assumption) :
    noble_contracts.companion.core.Core.release engine id ≠ ok (.Ok released) := by
  intro success
  have outcome := release_requires_proved engine id entry contract released
    evidenceLookup contractLookup success
  simp_all [noble_contracts.companion.core.Core.release,
    noble_contracts.companion.release.select,
    noble_contracts.companion.registry.application.proved,
    noble_contracts.companion.registry.entries.Registry.stale,
    core.cmp.PartialEq.ne.trait_default]
  split_ifs at success <;>
    simp_all [core.cmp.PartialEq.ne.default,
      noble_contracts.companion.admit.Outcome.Insts.CoreCmpPartialEqOutcome.eq,
      noble_contracts.companion.admit.Outcome.read_discriminant]
  all_goals split_ifs at success <;> simp_all

/-- The actual Rust scalar representation of each semantic template's literal. -/
def guardLiteral : GuardTemplate → I64
  | .ltI64Max => core.num.I64.MAX
  | .neI64Min => core.num.I64.MIN
  | .eqI64Literal value => ⟨value⟩

/-- Exact wrapper grammar with the extracted decimal renderer left explicit.
The closed byte-size proofs are reused from generated code: this definition
introduces no new native axiom and does not assert parser correctness. -/
def sourceLayout (template : GuardTemplate) (subject : Str) : Result String := do
  let out ← noble_contracts.alloc.string.String.new
  let out ← noble_contracts.alloc.string.String.push_str out
    (toStr "[ dup " (of_decide_eq_true
      noble_contracts.companion.guard.wrapper_source._native.decide.ax_1))
  let out ← noble_contracts.companion.guard.push_i64 out (guardLiteral template)
  let beforeSubject := match template with
    | .eqI64Literal _ => toStr " = [ [ " (of_decide_eq_true
        noble_contracts.companion.guard.wrapper_source._native.decide.ax_8)
    | _ => toStr " = [ drop 1 inl ] [ [ " (of_decide_eq_true
        noble_contracts.companion.guard.wrapper_source._native.decide.ax_2)
  let afterSubject := match template with
    | .eqI64Literal _ => toStr " ] run inr ] [ drop 1 inl ] if ]" (of_decide_eq_true
        noble_contracts.companion.guard.wrapper_source._native.decide.ax_9)
    | _ => toStr " ] run inr ] if ]" (of_decide_eq_true
        noble_contracts.companion.guard.wrapper_source._native.decide.ax_3)
  let out ← noble_contracts.alloc.string.String.push_str out beforeSubject
  let out ← noble_contracts.alloc.string.String.push_str out subject
  noble_contracts.alloc.string.String.push_str out afterSubject

theorem wrapper_source_layout (engine : noble_contracts.companion.Core)
    (template : GuardTemplate) (subject : Str) :
    noble_contracts.companion.guard.wrapper_source engine (rustGuard template) subject =
      sourceLayout template subject := by
  cases template <;> rfl

/-- Live invocation retains the same supplied program on acceptance; rejection
consumes it without running it. The primitive execution theorem is separate. -/
def invocationLayout (template : GuardTemplate) : Result String := do
  let out ← noble_contracts.alloc.string.String.Insts.CoreConvertFromShared0Str.from
    (toStr "[ swap dup " (of_decide_eq_true
      noble_contracts.companion.guard.invocation_wrapper._native.decide.ax_1))
  let out ← noble_contracts.companion.guard.push_i64 out (guardLiteral template)
  let branches := match template with
    | .eqI64Literal _ => toStr " = [ swap run inr ] [ drop drop 1 inl ] if ]"
        (of_decide_eq_true
          noble_contracts.companion.guard.invocation_wrapper._native.decide.ax_4)
    | _ => toStr " = [ drop drop 1 inl ] [ swap run inr ] if ]"
        (of_decide_eq_true
          noble_contracts.companion.guard.invocation_wrapper._native.decide.ax_2)
  noble_contracts.alloc.string.String.push_str out branches

theorem invocation_wrapper_layout (template : GuardTemplate) :
    noble_contracts.companion.guard.invocation_wrapper (rustGuard template) =
      invocationLayout template := by
  cases template <;>
    simp only [noble_contracts.companion.guard.invocation_wrapper, invocationLayout,
      rustGuard, guardLiteral, bind_tc_ok]

end NobleCertificates.Correspondence
