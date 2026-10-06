#![feature(register_tool)]
#![register_tool(tigerstyle)]

use noble_kernel::contracts::{
    environment, Behavior, BoundClockRegistration, Definition, NominalDecl, LIVE_DISPATCH,
    TEST_CLOCK, TEST_EMIT,
};
use noble_kernel::execution::{Definition as ExecutableDefinition, Submission};
use noble_kernel::shapes::Pattern;
use noble_kernel::types::{EffId, EffSet, NominalShape, NominalTypeId, ResourceKind, Ty};
use noble_kernel::untrusted::{Node, NodeId};
use noble_kernel::words::{Inst, Scheme};
use noble_wasm::source::Compiler;

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn diagnostic(problem: noble_wasm::Diagnostic) -> &'static str {
    match problem {
        noble_wasm::Diagnostic::Invalid => "invalid",
        noble_wasm::Diagnostic::Exhausted => "exhausted",
        noble_wasm::Diagnostic::Unsupported => "unsupported",
        noble_wasm::Diagnostic::Defective => "defective",
    }
}

fn account(module: u64, kind: u32) -> NominalDecl {
    NominalDecl {
        id: NominalTypeId { module, ordinal: 0 },
        shape: NominalShape::Opaque(Box::new(Ty::Resource(ResourceKind(kind)))),
        exported: true,
        public: [false, false],
    }
}

fn source() -> Result<(noble_contracts::source::Session, Ty, Ty), String> {
    let one = account(110, 41);
    let two = account(111, 42);
    let env = environment()
        .map_err(|problem| format!("bootstrap: {problem:?}"))?
        .enable_live_slots()
        .register_live_resource(one.clone())
        .map_err(|problem| format!("register Account@1: {problem:?}"))?
        .register_live_resource(two.clone())
        .map_err(|problem| format!("register Account@2: {problem:?}"))?;
    let mut session = noble_contracts::source::Session::new_live_slots(env)
        .map_err(|error| error.diagnostic().message.clone())?;
    session
        .register_live_resource("Account@1", one.id)
        .map_err(|error| error.diagnostic().message.clone())?;
    session
        .register_live_resource("Account@2", two.id)
        .map_err(|error| error.diagnostic().message.clone())?;
    let one_type = session
        .parse_live_type("Account@1")
        .map_err(|error| error.diagnostic().message.clone())?;
    let two_type = session
        .parse_live_type("Account@2")
        .map_err(|error| error.diagnostic().message.clone())?;
    Ok((session, one_type, two_type))
}

fn submission(
    session: &noble_contracts::source::Session,
    source: &[u8],
    inputs: &[Ty],
) -> Result<(Submission, noble_kernel::untrusted::Checked), String> {
    let prepared = session
        .prepare(source, inputs, LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    Ok((
        prepared
            .submission()
            .ok_or("source did not produce a submission")?
            .clone(),
        prepared
            .checked()
            .ok_or("source did not independently check a submission")?
            .clone(),
    ))
}

fn live_inputs(account: &Ty) -> Vec<Ty> {
    vec![
        account.clone(),
        Ty::I64,
        Ty::live_ref(
            vec![account.clone(), Ty::I64],
            vec![account.clone(), Ty::I64],
            EffSet::from_ids(&[TEST_EMIT]),
        ),
    ]
}

#[test]
fn checked_versioned_slot_admits_exact_ordered_target_before_any_body_runs() -> Result<(), String> {
    let (session, one, two) = source()?;
    let (caller, caller_checked) = submission(&session, b"slot.invoke", &live_inputs(&one))?;
    let (target, target_checked) = submission(&session, b"1 +", &[one.clone(), Ty::I64])?;
    let (wrong_version, wrong_checked) = submission(&session, b"1 +", &[two, Ty::I64])?;
    let (wrong_order, order_checked) = submission(&session, b"", &[Ty::I64, one])?;
    let [site] = caller_checked.live_sites.as_slice() else {
        return Err("slot submission did not retain exactly one checked site".into());
    };
    let mut over_ceiling = target_checked.interface.clone();
    over_ceiling.effects = EffSet::from_ids(&[LIVE_DISPATCH]);
    if caller_checked.interface.effects != EffSet::from_ids(&[TEST_EMIT, LIVE_DISPATCH])
        || !site.admits_target(&target_checked.interface)
        || site.admits_target(&wrong_checked.interface)
        || site.admits_target(&order_checked.interface)
        || site.admits_target(&over_ceiling)
    {
        return Err("exact ordered nominal, resource, or effect ceiling was lost".into());
    }
    for accepted in [&caller, &target, &wrong_version, &wrong_order] {
        Compiler::new_live_slots()
            .prepare(accepted)
            .map_err(|problem| format!("genuine checked slot-profile source was refused: {}", diagnostic(problem)))?;
    }
    Ok(())
}

#[test]
fn legacy_profiles_and_forged_slot_metadata_are_rejected() -> Result<(), String> {
    let (session, one, _) = source()?;
    let (caller, _) = submission(&session, b"slot.invoke", &live_inputs(&one))?;
    for legacy in [Compiler::new(), Compiler::new_live(), Compiler::new_text_cursor()] {
        if legacy.prepare(&caller).err() != Some(noble_wasm::Diagnostic::Invalid) {
            return Err("legacy profile admitted a slot environment".into());
        }
    }
    let mut forged = caller.clone();
    forged.environment.resource_kinds[1] = ResourceKind(91);
    if Compiler::new_live_slots().prepare(&forged).err() != Some(noble_wasm::Diagnostic::Invalid) {
        return Err("forged host resource-kind catalog was admitted".into());
    }
    forged = caller.clone();
    forged.environment.nominals[0].shape =
        NominalShape::Opaque(Box::new(Ty::Resource(ResourceKind(91))));
    if Compiler::new_live_slots().prepare(&forged).err() != Some(noble_wasm::Diagnostic::Invalid) {
        return Err("forged host nominal schema was admitted".into());
    }
    forged = caller.clone();
    forged.environment.live_resource_nominals[0] = NominalTypeId { module: 912, ordinal: 0 };
    if Compiler::new_live_slots().prepare(&forged).err() != Some(noble_wasm::Diagnostic::Invalid) {
        return Err("unregistered nominal identity was admitted".into());
    }
    forged = caller.clone();
    forged.environment.effects.retain(|effect| *effect != LIVE_DISPATCH);
    if Compiler::new_live_slots().prepare(&forged).err() != Some(noble_wasm::Diagnostic::Invalid) {
        return Err("unavailable live dispatch effect was admitted".into());
    }
    forged = caller.clone();
    forged.environment.effects.push(EffId(73));
    if Compiler::new_live_slots().prepare(&forged).err() != Some(noble_wasm::Diagnostic::Invalid) {
        return Err("unknown effect identity was admitted".into());
    }
    forged = caller.clone();
    forged.environment.defs[22].effects.clear();
    if Compiler::new_live_slots().prepare(&forged).err() != Some(noble_wasm::Diagnostic::Invalid) {
        return Err("forged builtin host-effect contract was admitted".into());
    }
    forged = caller.clone();
    forged.environment.kinds[22] = Behavior::Named;
    if Compiler::new_live_slots().prepare(&forged).err() != Some(noble_wasm::Diagnostic::Invalid) {
        return Err("forged builtin host behavior was admitted".into());
    }
    forged = caller.clone();
    let Node::SlotInvoke { site_id, .. } = &mut forged.body.candidate.nodes[0] else {
        return Err("source did not emit a slot invocation".into());
    };
    *site_id += 1;
    if Compiler::new_live_slots().prepare(&forged).err() != Some(noble_wasm::Diagnostic::Invalid) {
        return Err("slot site with a forged node identity was admitted".into());
    }
    Ok(())
}

#[test]
fn registered_host_clock_is_replayed_without_granting_unknown_host_contracts() -> Result<(), String> {
    for clock_first in [false, true] {
        let mut host = environment()
            .map_err(|problem| format!("bootstrap: {problem:?}"))?;
        if !clock_first {
            host = host.enable_live_slots();
        }
        let (mut host, _) = host
            .declare_bound_clock(BoundClockRegistration {
                adapter_identity: "clock-version-A".into(),
                adapter_slot: 17,
                owner: 110,
                input: vec![],
                output: vec![Ty::I64],
                effects: EffSet::from_ids(&[TEST_CLOCK]),
            })
            .map_err(|problem| format!("register clock: {problem:?}"))?;
        if clock_first {
            host = host.enable_live_slots();
        }
        let session = noble_contracts::source::Session::new_live_slots(host)
            .map_err(|error| error.diagnostic().message.clone())?;
        let (submission, _) = submission(&session, b"1", &[])?;
        Compiler::new_live_slots()
            .prepare(&submission)
            .map_err(|problem| format!("registered host clock was refused: {}", diagnostic(problem)))?;

        let mut forged = submission;
        forged.environment.bound_adapters[0].adapter_identity.clear();
        if Compiler::new_live_slots().prepare(&forged).err()
            != Some(noble_wasm::Diagnostic::Invalid)
        {
            return Err("an unregistered host clock contract was admitted".into());
        }
    }
    Ok(())
}

#[test]
fn named_body_is_independently_checked_against_exact_slot_profile_contract() -> Result<(), String> {
    let (session, one, _) = source()?;
    let (mut named, _) = submission(&session, b"1 +", &[one.clone(), Ty::I64])?;
    let nominal = match &one {
        Ty::Nominal(id, shape) => Pattern::Nominal(*id, shape.clone()),
        _ => return Err("host resource did not resolve to its exact nominal schema".into()),
    };
    let stack = vec![nominal, Pattern::I64];
    named.environment.defs.push(Scheme {
        var_kinds: vec![],
        stack_in: stack.clone(),
        stack_out: stack,
        effects: vec![],
    });
    named.environment.kinds.push(Behavior::Named);
    named.environment.deps.push(vec![]);
    named.environment.definition_owners.push(Some(110));
    named.definitions.push(ExecutableDefinition {
        definition: Definition(23),
        identity: 901,
        body: named.body.clone(),
        expected: named.request.expected.clone(),
    });
    named.body.candidate.nodes = vec![Node::Invocation {
        def: Definition(23),
        inst: Inst { bindings: vec![] },
    }];
    named.body.candidate.body = vec![NodeId(0)];
    named.body.texts.clear();
    Compiler::new_live_slots()
        .prepare(&named)
        .map_err(|problem| format!("independently checked nominal definition was refused: {}", diagnostic(problem)))?;

    let (incorrect, _) = submission(&session, b"true", &[one, Ty::I64])?;
    let mut forged = named;
    forged.definitions[0].body = incorrect.body;
    if Compiler::new_live_slots().prepare(&forged).err() != Some(noble_wasm::Diagnostic::Invalid) {
        return Err("named body with a forged result interface was admitted".into());
    }
    Ok(())
}
