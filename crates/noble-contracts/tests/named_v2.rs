const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};
const SOURCE: &[u8] = b"module Subject@3 [ def twice [ d.step d.step ] contract 2 twice-law [ subject twice input [ x I64 ] output [ y I64 ] requires [ true ] ensures [ (eq (out y) (add (in x) 2)) ] ] proof 2 twice-correct for twice-law [ (export-named-unary-I64 (pc-named-call 0 (exec-literal-1) (exec-add)) (pc-named-call 1 (exec-literal-1) (exec-add)) (by-exact-tail) (named-true-eq-wrap)) ] ]";

fn fixture_for(
    source: &[u8],
) -> Result<
    (
        noble_contracts::intrinsic::ContractGoal,
        noble_contracts::intrinsic::ProofBatch,
    ),
    String,
> {
    let mut session =
        noble_contracts::source::ModuleSession::new(&[]).map_err(|error| format!("{error:?}"))?;
    for source in [
        b"module Definitions@5 [ export step def step [ 1 + ] ]".as_slice(),
        b"import Definitions@5 as d".as_slice(),
    ] {
        let prepared = session
            .prepare(source, &[], LIMITS)
            .map_err(|error| format!("{error:?}"))?;
        let (next, committed) = session.commit(prepared);
        committed.map_err(|error| format!("{error:?}"))?;
        session = next;
    }
    let prepared = session
        .prepare(source, &[], LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let batch = prepared
        .proof_obligations()
        .ok_or("missing named proof obligations")?
        .clone();
    let Some(noble_contracts::intrinsic::PendingGoal::Contract { contract }) =
        batch.obligations.first().map(|proof| &proof.goal)
    else {
        return Err("missing named contract proof".into());
    };
    Ok((contract.as_ref().clone(), batch))
}
fn fixture() -> Result<
    (
        noble_contracts::intrinsic::ContractGoal,
        noble_contracts::intrinsic::ProofBatch,
    ),
    String,
> {
    fixture_for(SOURCE)
}

#[test]
fn checked_named_v2_two_distinct_source_calls() -> Result<(), String> {
    let (goal, batch) = fixture()?;
    noble_contracts::intrinsic::prepare_named_contract(&goal, LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    let checked = noble_contracts::intrinsic::check_batch(&batch, LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(checked.len(), 1);
    assert_eq!(
        checked[0].kind,
        noble_contracts::intrinsic::ProofKind::NamedContract
    );
    assert_eq!(checked[0].claim, "NamedV2Obligation.claim");
    assert_eq!(goal.subject.input_types, [noble_kernel::types::Ty::I64]);
    assert_eq!(goal.subject.named_uses.len(), 3);
    assert_ne!(
        goal.subject.named_uses[1].definition,
        goal.subject.named_uses[2].definition
    );
    assert_eq!(
        goal.subject.named_uses[1].definition_ordinal,
        goal.subject.named_uses[2].definition_ordinal
    );
    Ok(())
}

#[test]
fn named_v2_requires_original_imported_step() -> Result<(), String> {
    let source = String::from_utf8(SOURCE.to_vec()).map_err(|error| error.to_string())?;
    let local = source.replace(
        "def twice [ d.step d.step ]",
        "def step [ 1 + ] def twice [ step step ]",
    );
    assert!(
        fixture_for(local.as_bytes()).is_err(),
        "a locally defined step cannot stand in for the original imported donor"
    );
    Ok(())
}

#[test]
fn named_v2_rejects_forged_owner_slot_and_ordinal() -> Result<(), String> {
    let (goal, _) = fixture()?;
    let mut slot = goal.clone();
    slot.subject.named_uses[1].definition = slot.subject.named_uses[2].definition;
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&slot, LIMITS).is_err(),
        "slot alias"
    );
    let mut owner = goal.clone();
    owner.subject.named_uses[1].definition_owner += 1;
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&owner, LIMITS).is_err(),
        "forged imported owner"
    );
    let mut ordinal = goal.clone();
    ordinal.subject.named_uses[1].definition_ordinal += 1;
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&ordinal, LIMITS).is_err(),
        "forged lexical ordinal"
    );
    Ok(())
}

#[test]
fn named_v2_rejects_partial_word_wrong_token_and_alias_rebind() -> Result<(), String> {
    let (goal, _) = fixture()?;
    let mut partial = goal.clone();
    let span = partial.subject.named_uses[1]
        .source_span
        .as_mut()
        .ok_or("first original word span")?;
    span.end = span
        .start
        .checked_add(1)
        .ok_or("first original word span start overflows")?;
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&partial, LIMITS).is_err(),
        "partial d is not the original d.step source word"
    );
    let mut other_token = goal.clone();
    let original_body = other_token.subject.named_uses[0].body_span;
    let bracket_end = original_body
        .start
        .checked_add(1)
        .ok_or("original body span start overflows")?;
    other_token.subject.named_uses[1].source_span = Some(noble_contracts::Span {
        start: original_body.start,
        end: bracket_end,
    });
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&other_token, LIMITS).is_err(),
        "nonempty bracket position is not the original caller word"
    );
    let mut rebound = goal.clone();
    let alias = rebound
        .subject
        .named_imports
        .first_mut()
        .ok_or("retained imported alias")?;
    alias.owner = goal.subject.definition_owner;
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&rebound, LIMITS).is_err(),
        "alias cannot silently change original source module owner"
    );
    let mut renamed_alias = goal.clone();
    let alias = renamed_alias
        .subject
        .named_imports
        .first_mut()
        .ok_or("retained imported alias")?;
    alias.alias = "other".into();
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&renamed_alias, LIMITS).is_err(),
        "source still says d.step, not other.step"
    );
    let mut hidden = goal.clone();
    let module_index = hidden.subject.named_uses[1].module_index;
    let source = &mut hidden.subject.source_dependencies[module_index].full_source;
    let exported = b"export step";
    let start = source
        .windows(exported.len())
        .position(|word| word == exported)
        .ok_or("original donor has explicit export")?;
    let end = start
        .checked_add(exported.len())
        .ok_or("explicit export range overflows")?;
    source[start..end].fill(b' ');
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&hidden, LIMITS).is_err(),
        "a same-position hidden original step cannot become an imported callee"
    );
    Ok(())
}

#[test]
fn named_v2_rejects_wrong_inst_and_unreachable_graph() -> Result<(), String> {
    let (goal, _) = fixture()?;
    let mut inst = goal.clone();
    let selected = inst.subject.named_uses[0].definition;
    let body = &mut inst
        .subject
        .accepted_submission
        .definitions
        .iter_mut()
        .find(|definition| definition.definition == selected)
        .ok_or("selected definition")?
        .body
        .candidate;
    let at = usize::try_from(body.body[0].0)
        .map_err(|_| "first call node exceeds host address space")?;
    let noble_kernel::untrusted::Node::Invocation { inst: bindings, .. } = &mut body.nodes[at]
    else {
        return Err("missing first call".into());
    };
    bindings
        .bindings
        .push(noble_kernel::words::Binding::Stack(vec![
            noble_kernel::types::Ty::I64,
        ]));
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&inst, LIMITS).is_err(),
        "forged per-call Inst"
    );
    let mut orphan_node = goal.clone();
    let selected = orphan_node.subject.named_uses[0].definition;
    let body = &mut orphan_node
        .subject
        .accepted_submission
        .definitions
        .iter_mut()
        .find(|definition| definition.definition == selected)
        .ok_or("selected definition")?
        .body
        .candidate;
    body.nodes.push(noble_kernel::untrusted::Node::Literal {
        lit: noble_kernel::untrusted::Lit::I64(0),
        inst: noble_kernel::words::Inst { bindings: vec![] },
    });
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&orphan_node, LIMITS).is_err(),
        "unreachable node"
    );
    let mut orphan_row = goal.clone();
    let mut extra = orphan_row.subject.accepted_submission.definitions[0].clone();
    extra.definition = noble_kernel::contracts::Definition(1234);
    orphan_row
        .subject
        .accepted_submission
        .definitions
        .push(extra);
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&orphan_row, LIMITS).is_err(),
        "orphan row"
    );
    let mut orphan_environment = goal.clone();
    let env = &mut orphan_environment.subject.accepted_submission.environment;
    env.defs
        .push(env.defs.last().ok_or("missing named scheme")?.clone());
    env.kinds.push(noble_kernel::contracts::Behavior::Named);
    env.deps.push(vec![]);
    env.definition_owners
        .push(Some(goal.subject.definition_owner));
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&orphan_environment, LIMITS).is_err(),
        "orphan environment row"
    );
    let mut duplicate_occurrence = goal.clone();
    duplicate_occurrence.subject.named_uses[2].candidate_node = noble_kernel::untrusted::NodeId(0);
    assert!(
        noble_contracts::intrinsic::prepare_named_contract(&duplicate_occurrence, LIMITS).is_err(),
        "misbound occurrence"
    );
    Ok(())
}

#[path = "named_v2/proofs.rs"]
mod proofs;
