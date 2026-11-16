// Test-only Rust-emitted plan corpus; not a replacement for the independent Lean model.

use super::*;
use alloc::borrow::ToOwned;
use alloc::string::ToString;
use crate::identity::IdentityScheme;

const REGENERATE: &str = "node verification/net-plans/regenerate-lean-corpus.mjs";
const RETAINED: &str =
    include_str!("../../../../../proofs/foundation/NobleFoundation/NetI64PlanCorpus.lean");
const SOURCE_BEGIN: &str = "NOBLE_NET_PLAN_CORPUS_BEGIN";
const SOURCE_END: &str = "NOBLE_NET_PLAN_CORPUS_END";

#[derive(Clone, Copy)]
struct RuleCase {
    name: &'static str,
    symbol: Symbol,
    value: u64,
    lean_rule: &'static str,
    opcode: u8,
    boundaries: u8,
    results: u8,
    model_bytes: &'static str,
    physical_start: &'static str,
    physical_redex: &'static str,
    physical_after: &'static str,
    physical_theorem: &'static str,
}

// The only full physical comparisons are to independently declared model
// fixtures. Other orientations/modes still parse and check rule symbols.
const RULES: [RuleCase; 4] = [
    RuleCase {
        name: "op2",
        symbol: Symbol::Op2(Op::Add),
        value: 27,
        lean_rule: ".op2 .add",
        opcode: 1,
        boundaries: 2,
        results: 1,
        model_bytes: "op2OutputFixture.emit",
        physical_start: "op2PhysicalStart",
        physical_redex: "op2PhysicalRedex",
        physical_after: "op2PhysicalAfter",
        physical_theorem: "PhysicalState.op2_output_execution_exact",
    },
    RuleCase {
        name: "op1",
        symbol: Symbol::Op1(Op::Sub, 7),
        value: 13,
        lean_rule: ".op1 .sub (BitVec.ofNat 64 7)",
        opcode: 2,
        boundaries: 1,
        results: 1,
        model_bytes: "op1OutputFixture.emit",
        physical_start: "op1PhysicalStart",
        physical_redex: "op1PhysicalRedex",
        physical_after: "op1PhysicalAfter",
        physical_theorem: "PhysicalState.op1_wrapping_execution_exact",
    },
    RuleCase {
        name: "dup",
        symbol: Symbol::Dup,
        value: 42,
        lean_rule: ".dup",
        opcode: 3,
        boundaries: 2,
        results: 2,
        model_bytes: "dupOutputFixture.emit",
        physical_start: "dupPhysicalStart",
        physical_redex: "dupPhysicalRedex",
        physical_after: "dupPhysicalAfter",
        physical_theorem: "PhysicalState.dup_execution_exact",
    },
    RuleCase {
        name: "era",
        symbol: Symbol::Era,
        value: 42,
        lean_rule: ".era",
        opcode: 4,
        boundaries: 0,
        results: 0,
        model_bytes: "eraPlanBytes",
        physical_start: "eraPhysicalStart",
        physical_redex: "eraPhysicalRedex",
        physical_after: "eraPhysicalAfter",
        physical_theorem: "PhysicalState.era_execution_exact",
    },
];

struct EmittedCase {
    rule: RuleCase,
    name: alloc::string::String,
    reverse: bool,
    policy: StoragePolicy,
    bytes: rewrite::PlanBytes,
    model_fixture: bool,
}

// Fresh Rust compiler inputs beyond the four independently modeled physical
// examples: extreme payloads and an erased zero. Only their emitted bytes and
// selected-rule fields are compared, not a fabricated physical fixture proof.
const EXTRAS: [RuleCase; 4] = [
    RuleCase {
        name: "op2Max",
        value: u64::MAX,
        ..RULES[0]
    },
    RuleCase {
        name: "op1Zero",
        value: 0,
        ..RULES[1]
    },
    RuleCase {
        name: "dupZero",
        value: 0,
        ..RULES[2]
    },
    RuleCase {
        name: "eraZero",
        value: 0,
        ..RULES[3]
    },
];

#[test]
fn generated_lean_plan_corpus_is_fresh() {
    assert_eq!(RETAINED, render_corpus(), "run `{REGENERATE}`");
}

#[test]
#[ignore = "emits the real compiler byte corpus to the shell regeneration command"]
#[should_panic(expected = "NOBLE_NET_PLAN_CORPUS_BEGIN")]
fn regenerate_lean_plan_corpus() {
    // The test-only domain core has no filesystem or output port. The shell
    // script captures this bounded panic payload and owns the file write.
    panic!("{SOURCE_BEGIN}\n{}\n{SOURCE_END}", render_corpus());
}

fn emitted_cases() -> alloc::vec::Vec<EmittedCase> {
    let mut cases = alloc::vec::Vec::with_capacity((RULES.len() + EXTRAS.len()) * 4);
    for (rule, model_fixture) in RULES
        .into_iter()
        .map(|rule| (rule, true))
        .chain(EXTRAS.into_iter().map(|rule| (rule, false)))
    {
        for reverse in [false, true] {
            for policy in [StoragePolicy::Reuse, StoragePolicy::Allocate] {
                let (slots, outputs, pair) = plan_fixture(rule.symbol, rule.value, reverse);
                let bytes = rewrite::compile(&slots, &outputs, pair, policy)
                    .unwrap_or_else(|error| panic!("{} plan compilation: {error:?}", rule.name));
                rewrite::check(&bytes, &slots, &outputs, pair, policy)
                    .unwrap_or_else(|error| panic!("{} plan check: {error:?}", rule.name));
                let orientation = if reverse { "Reverse" } else { "Forward" };
                let storage = if policy == StoragePolicy::Reuse {
                    "Reuse"
                } else {
                    "Allocate"
                };
                cases.push(EmittedCase {
                    rule,
                    name: alloc::format!("{}{orientation}{storage}", rule.name),
                    reverse,
                    policy,
                    bytes,
                    model_fixture,
                });
            }
        }
    }
    cases
}

fn stream_digest(cases: &[EmittedCase]) -> crate::ir::Digest {
    let mut hasher = IDS.start();
    hasher.update(b"noble-net-i64-plan-corpus/1");
    for revision in [
        CONTRACT_REVISION,
        PLAN_REVISION,
        PLAN_RULES_REVISION,
        PLAN_REPRESENTATION_REVISION,
        PLAN_CHECKER_REVISION,
        PLAN_EXECUTOR_REVISION,
    ] {
        hasher.update(
            &u64::try_from(revision.len())
                .expect("revision length fits u64")
                .to_le_bytes(),
        );
        hasher.update(revision.as_bytes());
    }
    for case in cases {
        hasher.update(
            &u64::try_from(case.name.len())
                .expect("case name length fits u64")
                .to_le_bytes(),
        );
        hasher.update(case.name.as_bytes());
        hasher.update(
            &u64::try_from(case.bytes.as_bytes().len())
                .expect("plan byte length fits u64")
                .to_le_bytes(),
        );
        hasher.update(case.bytes.as_bytes());
    }
    hasher.finish()
}

fn render_corpus() -> alloc::string::String {
    let cases = emitted_cases();
    let compiler_source = crate::ir::hex(&digest_bytes(
        &IDS,
        &[
            b"net-plan-compiler-and-checker-source/1",
            include_str!("../rewrite.rs").as_bytes(),
        ],
    ));
    let net_source = crate::ir::hex(&digest_bytes(
        &IDS,
        &[
            b"net-plan-test-and-executor-source/1",
            include_str!("../../net.rs").as_bytes(),
        ],
    ));
    let generator_source = crate::ir::hex(&digest_bytes(
        &IDS,
        &[
            b"net-plan-corpus-generator-source/1",
            include_str!("corpus.rs").as_bytes(),
        ],
    ));
    let model_source = crate::ir::hex(&digest_bytes(
        &IDS,
        &[
            b"net-plan-model-and-lowering-source/2",
            include_str!("../../../../../proofs/foundation/NobleFoundation/NetI64Plans.lean")
                .as_bytes(),
            include_str!("../../../../../proofs/foundation/NobleFoundation/NetI64PlanLowering.lean")
                .as_bytes(),
        ],
    ));
    let digest = crate::ir::hex(&stream_digest(&cases));
    let mut source = alloc::format!(
        r#"import NobleFoundation.NetI64Plans

/-! Finite Rust compiler-to-Lean byte correspondence. These literal bytes come only
from rewrite::compile, then pass rewrite::check. The corresponding Lean fixture,
parser, rule and physical executor are independent obligations, not a universal
Rust compiler correctness proof. Regenerate with:
{REGENERATE}
-/
-- contract={CONTRACT_REVISION}; plan={PLAN_REVISION}
-- rules={PLAN_RULES_REVISION}; representation={PLAN_REPRESENTATION_REVISION}
-- checker={PLAN_CHECKER_REVISION}; executor={PLAN_EXECUTOR_REVISION}
-- source crates/noble-ngraph/src/net/rewrite.rs test-net-digest={compiler_source}
-- source crates/noble-ngraph/src/net.rs test-net-digest={net_source}
-- source crates/noble-ngraph/src/net/plan/corpus.rs test-net-digest={generator_source}
-- source proofs/foundation/NobleFoundation/NetI64Plans.lean + NetI64PlanLowering.lean test-net-digest={model_source}
-- length-framed emitted byte stream + revisions test-net-digest={digest}
-- test-net is a test-only identity scheme, not a cryptographic signature.

namespace NobleFoundation.NetI64PlanCorpus

open NobleFoundation.NetI64
open NobleFoundation.NetI64Plans

"#
    );
    for case in &cases {
        write_case(&mut source, case);
    }
    source.push_str("end NobleFoundation.NetI64PlanCorpus\n");
    source
}

fn write_case(source: &mut impl core::fmt::Write, case: &EmittedCase) {
    let name = &case.name;
    let rule = case.rule;
    let chunks = case
        .bytes
        .as_bytes()
        .chunks(16)
        .map(|chunk| {
            alloc::format!(
                "  [{}]",
                chunk
                    .iter()
                    .map(u8::to_string)
                    .collect::<alloc::vec::Vec<_>>()
                    .join(", ")
            )
        })
        .collect::<alloc::vec::Vec<_>>()
        .join(" ++\n");
    let byte_identity = crate::ir::hex(&digest_bytes(
        &IDS,
        &[
            b"noble-net-i64-plan-image/1",
            PLAN_RULES_REVISION.as_bytes(),
            PLAN_CHECKER_REVISION.as_bytes(),
            case.bytes.as_bytes(),
        ],
    ));
    core::writeln!(
        source,
        "-- {name}: emitted_bytes={}; rules+checker+bytes test-net-digest={byte_identity}\ndef {name}Bytes : List UInt8 :=\n{chunks}\n",
        case.bytes.as_bytes().len()
    )
    .expect("write corpus bytes");
    let (rule_left, rule_right) = if case.reverse {
        (
            alloc::format!(".num (BitVec.ofNat 64 {})", rule.value),
            rule.lean_rule.to_owned(),
        )
    } else {
        (
            rule.lean_rule.to_owned(),
            alloc::format!(".num (BitVec.ofNat 64 {})", rule.value),
        )
    };
    let is_reused = case.policy == StoragePolicy::Reuse;
    let first_action =
        u8::from(is_reused && rule.results != 0 && (!case.reverse || rule.results == 2));
    let second_action =
        u8::from(is_reused && rule.results != 0 && (case.reverse || rule.results == 2));
    let reservation = if is_reused { 0 } else { rule.results };
    let rule_opcode = rule.opcode;
    let boundaries = rule.boundaries;
    let results = rule.results;
    core::writeln!(
        source,
        r#"theorem {name}_decoded_fields_and_symbols :
  (BytePlan.decode {name}Bytes).isSome = true ∧
  (compileRule ({rule_left}) ({rule_right})).isSome = true ∧
  ((BytePlan.decode {name}Bytes).map (fun image =>
    (image.reuse, image.opcode.toNat, image.pair,
     image.action.1.toNat, image.action.2.toNat,
     image.boundaries.length, image.results.length, image.writes.length,
     image.externalWrites.length, image.arenaReservation,
     image.scratchReservation, image.worklistReservation,
     image.publications.length,
     image.results.map (fun result => result.2.2.1)))) =
  (compileRule ({rule_left}) ({rule_right})).map (fun selected =>
    ({is_reused}, {rule_opcode}, (0, 0, 1, 0),
     {first_action}, {second_action},
     {boundaries}, {results}, {boundaries}, {boundaries},
     {reservation}, 256, 0, 0, selected.results.map Prod.snd)) := by
  constructor
  · set_option maxRecDepth 3000 in decide
  constructor
  · set_option maxRecDepth 3000 in decide
  · set_option maxRecDepth 3000 in rfl
"#
    )
    .expect("write decoded field and model-rule comparison");
    if case.model_fixture && !case.reverse && is_reused {
        let model_bytes = rule.model_bytes;
        let physical_start = rule.physical_start;
        let physical_redex = rule.physical_redex;
        let physical_after = rule.physical_after;
        let physical_theorem = rule.physical_theorem;
        core::writeln!(
            source,
            r#"-- The model fixture was declared independently of the Rust writer.
theorem {name}_matches_model : {name}Bytes = {model_bytes} := by
  decide

theorem {name}_physical :
  {physical_start}.executeCheckedImage {name}Bytes {physical_redex} =
    some {physical_after} := by
  rw [{name}_matches_model]
  exact {physical_theorem}
"#
        )
        .expect("write independent physical comparison");
    }
}
