use noble_contracts::{
    intrinsic::{self, ContractGoal, PendingGoal},
    source::{ModuleSession, Stage},
    Limits,
};
use noble_kernel::{
    contracts::Definition,
    types::Ty,
    untrusted::{Node, NodeId},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{env, fs};

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn prepare(units: &[Vec<u8>]) -> Result<(ContractGoal, intrinsic::ProofBatch), String> {
    let mut session = ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    for source in &units[..2] {
        let pending = session
            .prepare(source, &[], Limits::default())
            .map_err(|e| format!("{e:?}"))?;
        let (next, committed) = session.commit(pending);
        committed.map_err(|e| format!("{e:?}"))?;
        session = next;
    }
    let pending = session
        .prepare(&units[2], &[], Limits::default())
        .map_err(|e| format!("{e:?}"))?;
    let batch = pending
        .proof_obligations()
        .ok_or("no actual proof obligation")?
        .clone();
    let Some(PendingGoal::Contract { contract }) =
        batch.obligations.first().map(|obligation| &obligation.goal)
    else {
        return Err("no accepted named contract goal".into());
    };
    Ok((contract.clone(), batch))
}
fn mutation(goal: &ContractGoal, name: &str) -> Result<String, String> {
    let mut changed = goal.clone();
    let uses = &mut changed.subject.named_uses;
    match name {
        "reuse-one-specialization-slot-or-occurrence-for-two-calls" => {
            uses[2].definition = uses[1].definition
        }
        "wrong-row-owner-or-call-byte-span-or-lexical-ordinal" => uses[1].definition_owner += 1,
        "wrong-call-byte-span" => uses[1].source_span.as_mut().ok_or("missing span")?.start += 1,
        "call-byte-span-covers-only-d-instead-of-full-d.step-original-token" => {
            uses[1].source_span.as_mut().ok_or("missing span")?.end -= 5;
        }
        "call-byte-span-covers-wrong-original-word-token-with-ordered-nonempty-span" => {
            // Preserve BOTH ordered six-byte occurrence spans and the already
            // accepted two d.step graph nodes. Replace the second complete
            // lexical word in the retained original module with q.step. This
            // is a real different word, not a shifted partial `.step ` slice.
            let span = uses[2].source_span.ok_or("missing second lexical span")?;
            let bytes = &mut changed.subject.module_source;
            let word = bytes
                .get_mut(span.start as usize..span.end as usize)
                .ok_or("second word outside retained source")?;
            if word != b"d.step" {
                return Err("wrong original word is not a full d.step".into());
            }
            word.copy_from_slice(b"q.step");
            let start = changed.subject.source_span.start as usize;
            let end = changed.subject.source_span.end as usize;
            changed.subject.definition_source = bytes[start..end].to_vec();
            let root_index = uses[0].module_index;
            changed.subject.source_dependencies[root_index].full_source = bytes.clone();
        }
        "source-body-graph-mismatch" => {
            let selected = uses[0].definition;
            let row = changed
                .subject
                .accepted_submission
                .definitions
                .iter_mut()
                .find(|row| row.definition == selected)
                .ok_or("missing subject row")?;
            row.body.candidate.body.swap(0, 1);
        }
        "original-imported-donor-not-exported" => {
            let donor = &mut changed.subject.source_dependencies[uses[1].module_index].full_source;
            let old = b"export step";
            let at = donor
                .windows(old.len())
                .position(|word| word == old)
                .ok_or("original donor lacks export step")?;
            donor[at..at + old.len()].fill(b' ');
        }
        "wrong-lexical-ordinal" => uses[1].definition_ordinal += 1,
        "missing-or-partial-per-use-Inst-and-typed-derivation" => {
            let selected = uses[0].definition;
            let row = changed
                .subject
                .accepted_submission
                .definitions
                .iter_mut()
                .find(|row| row.definition == selected)
                .ok_or("missing subject row")?;
            let Node::Invocation { inst, .. } = &mut row.body.candidate.nodes[0] else {
                return Err("missing call".into());
            };
            inst.bindings
                .push(noble_kernel::words::Binding::Stack(vec![Ty::I64]));
        }
        "partial-step-literal-Inst" => {
            let slot = uses[1].definition;
            let row = changed
                .subject
                .accepted_submission
                .definitions
                .iter_mut()
                .find(|row| row.definition == slot)
                .ok_or("missing step row")?;
            let Node::Literal { inst, .. } = &mut row.body.candidate.nodes[0] else {
                return Err("missing literal".into());
            };
            inst.bindings.clear();
        }
        "wrong-typed-specialization-derivation" => {
            let slot = uses[2].definition;
            let row = changed
                .subject
                .accepted_submission
                .definitions
                .iter_mut()
                .find(|row| row.definition == slot)
                .ok_or("missing step row")?;
            row.expected.stack_out = vec![Ty::Bool];
        }
        "reuse-lexical-occurrence" => uses[2].candidate_node = uses[1].candidate_node,
        "orphan-extra-or-unreachable-graph-row-even-if-ExactClosure-shaped" => {
            let mut extra = changed.subject.accepted_submission.definitions[0].clone();
            extra.definition = Definition(1234);
            changed.subject.accepted_submission.definitions.push(extra);
        }
        "unreachable-candidate-node" => {
            let row = &mut changed.subject.accepted_submission.definitions[0];
            row.body.candidate.nodes.push(Node::Literal {
                lit: noble_kernel::untrusted::Lit::I64(0),
                inst: noble_kernel::words::Inst { bindings: vec![] },
            });
        }
        "misbound-submission-graph-node" => {
            changed.subject.named_uses[2].candidate_node = NodeId(0);
        }
        "requires-false-public-API" => {
            changed.requires.kind = intrinsic::FormKind::Atom("false".into());
        }
        "weakened-ensures-public-API" => {
            changed.ensures.kind = intrinsic::FormKind::Atom("true".into())
        }
        _ => return Err(format!("unhandled mutation {name}")),
    }
    match intrinsic::prepare_named_contract(&changed, Limits::default()) {
        Ok(_) => Err(format!("{name}: forged graph generated an exact claim")),
        Err(problem) => {
            let message = problem.message;
            if message.is_empty() {
                return Err(format!("{name}: empty refusal"));
            }
            if name == "call-byte-span-covers-only-d-instead-of-full-d.step-original-token"
                && message != "named call differs from exact original word token"
            {
                return Err(format!(
                    "{name}: refused before whole-word check: {message}"
                ));
            }
            if name == "call-byte-span-covers-wrong-original-word-token-with-ordered-nonempty-span"
                && message != "original named word has no retained import alias"
            {
                return Err(format!(
                    "{name}: did not inspect different complete q.step token: {message}"
                ));
            }
            if name == "original-imported-donor-not-exported"
                && message != "named source definition is absent or not exported"
            {
                return Err(format!(
                    "{name}: did not enforce original donor export: {message}"
                ));
            }
            Ok(message)
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths: Vec<_> = env::args_os().skip(1).collect();
    if paths.len() != 3 {
        return Err("usage: peer DEFINITIONS IMPORT SUBJECT".into());
    }
    let units: Vec<Vec<u8>> = paths.iter().map(fs::read).collect::<Result<_, _>>()?;
    let (goal, batch) = prepare(&units)?;
    let statement = intrinsic::prepare_named_contract(&goal, Limits::default())
        .map_err(|e| format!("{e:?}"))?;
    let checked =
        intrinsic::check_batch(&batch, Limits::default()).map_err(|e| format!("{e:?}"))?;
    if checked.len() != 1 || checked[0].claim != "NamedV2Obligation.claim" {
        return Err("wrong checked exact proof".into());
    }
    let uses = &goal.subject.named_uses;
    if uses.len() != 3
        || uses[1].definition == uses[2].definition
        || uses[1].source_span == uses[2].source_span
        || uses[1].definition_ordinal != uses[2].definition_ordinal
        || uses[1].definition_owner != uses[2].definition_owner
        || goal.subject.accepted_submission.definitions.len() != 3
    {
        return Err("real source lacked two distinct authenticated lexical uses".into());
    }
    let names = [
        "reuse-one-specialization-slot-or-occurrence-for-two-calls",
        "wrong-row-owner-or-call-byte-span-or-lexical-ordinal",
        "wrong-call-byte-span",
        "call-byte-span-covers-only-d-instead-of-full-d.step-original-token",
        "call-byte-span-covers-wrong-original-word-token-with-ordered-nonempty-span",
        "source-body-graph-mismatch",
        "wrong-lexical-ordinal",
        "missing-or-partial-per-use-Inst-and-typed-derivation",
        "partial-step-literal-Inst",
        "wrong-typed-specialization-derivation",
        "reuse-lexical-occurrence",
        "original-imported-donor-not-exported",
        "orphan-extra-or-unreachable-graph-row-even-if-ExactClosure-shaped",
        "unreachable-candidate-node",
        "misbound-submission-graph-node",
        "requires-false-public-API",
        "weakened-ensures-public-API",
    ];
    let mut results = Vec::<Value>::new();
    for name in names {
        let diagnostic = mutation(&goal, name)?;
        results.push(json!({"change":name,"outcome":"rejected","diagnostic":diagnostic}));
    }
    // A previously prepared @5 proof cannot be published after the retained
    // alias is rebound to an identical-looking @6 lexical token. The callback
    // must not run: source namespace identity is checked before proof checking.
    let mut session = ModuleSession::new(&[]).map_err(|e| format!("{e:?}"))?;
    for bytes in &units[..2] {
        let pending = session
            .prepare(bytes, &[], Limits::default())
            .map_err(|e| format!("{e:?}"))?;
        let (next, result) = session.commit(pending);
        result.map_err(|e| format!("{e:?}"))?;
        session = next;
    }
    let stale = session
        .prepare(&units[2], &[], Limits::default())
        .map_err(|e| format!("{e:?}"))?;
    let rebound_definition =
        String::from_utf8(units[0].clone())?.replace("Definitions@5", "Definitions@6");
    let rebound_import =
        String::from_utf8(units[1].clone())?.replace("Definitions@5", "Definitions@6");
    for bytes in [rebound_definition.as_bytes(), rebound_import.as_bytes()] {
        let pending = session
            .prepare(bytes, &[], Limits::default())
            .map_err(|e| format!("{e:?}"))?;
        let (next, result) = session.commit(pending);
        result.map_err(|e| format!("{e:?}"))?;
        session = next;
    }
    let before = session.generation();
    let (after, result) = session.commit_verified(stale, |_| -> Result<_, String> {
        panic!("stale @5 source graph reached verifier callback")
    });
    let error = result
        .err()
        .ok_or("rebound alias published stale source graph")?;
    if error.stage() != Stage::Acceptance || after.generation() != before {
        return Err(
            format!("alias rebind did not refuse atomically at acceptance: {error:?}").into(),
        );
    }
    let fresh = after
        .prepare(&units[2], &[], Limits::default())
        .map_err(|e| format!("fresh @6 source cannot prepare: {e:?}"))?;
    let fresh_batch = fresh
        .proof_obligations()
        .ok_or("fresh @6 proof obligation missing")?;
    let Some(PendingGoal::Contract {
        contract: fresh_goal,
    }) = fresh_batch.obligations.first().map(|proof| &proof.goal)
    else {
        return Err("fresh @6 source has no named contract goal".into());
    };
    if !fresh_goal.subject.source_dependencies.iter().any(|source| {
        source.module == "Definitions"
            && source.version == 6
            && source.full_source == rebound_definition.as_bytes()
    }) {
        return Err("fresh @6 goal omitted rebound immutable donor".into());
    }
    intrinsic::prepare_named_contract(fresh_goal, Limits::default())
        .map_err(|e| format!("{e:?}"))?;
    intrinsic::check_batch(fresh_batch, Limits::default()).map_err(|e| format!("{e:?}"))?;
    results.push(json!({"change":"replay-previously-prepared-Definitions@5-source-bound-goal-graph-and-claim-after-d-alias-rebind-to-Definitions@6-with-identical-d.step-token-text",
        "outcome":"rejected","stage":"acceptance","checker_callback_called":false,
        "generation_before":before,"generation_after":after.generation(),
        "fresh_v6_prepared_and_source_checked":true}));
    println!(
        "{}",
        json!({"schema":"noble-intrinsic-named-v2-peer/v1",
        "result":"passed","source_sha256":units.iter().map(|unit| sha(unit)).collect::<Vec<_>>(),
        "generated_statement_sha256":sha(statement.as_bytes()),
        "generated_statement":statement,
        "source_proof_term_sha256":sha(checked[0].lean_term.as_bytes()),
        "root_slot":uses[0].definition.0,"call_slots":[uses[1].definition.0,uses[2].definition.0],
        "root_module_index":uses[0].module_index,"donor_module_index":uses[1].module_index,
        "source_nodes":[uses[1].source_node,uses[2].source_node],
        "mutations":results,"guest_requests":0,"protected_operations":0})
    );
    Ok(())
}
