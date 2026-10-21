#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::assertion_density,
    tigerstyle::bool_naming,
    tigerstyle::excessive_file_length,
    tigerstyle::foreign_enum_catch_all,
    tigerstyle::function_length,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::platform_dependent_cast,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]

use noble_contracts::source::{ModuleKind, ModulePrepared, ModuleSession, Stage};
use noble_kernel::untrusted::{Node, Outcome, Request};
use noble_wasm::source::Compiler;

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn wasm_error(error: noble_wasm::Diagnostic) -> &'static str {
    match error {
        noble_wasm::Diagnostic::Invalid => "invalid",
        noble_wasm::Diagnostic::Exhausted => "exhausted",
        noble_wasm::Diagnostic::Unsupported => "unsupported",
        noble_wasm::Diagnostic::Defective => "defective",
    }
}

fn signed_expression(module: &[u8], expression: &[u8]) -> Result<ModulePrepared, String> {
    let session = ModuleSession::new_signed_self(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let staged = session
        .prepare(module, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    if staged.kind() != ModuleKind::Module || staged.submission().is_some() {
        return Err("module staged an executable initializer".into());
    }
    let (session, outcome) = session.commit(staged);
    outcome.map_err(|error| error.diagnostic().message.clone())?;
    session
        .prepare(expression, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())
}

#[test]
fn source_signed_self_requires_sealed_compiler_and_rejects_coherent_raw_forgery(
) -> Result<(), String> {
    let prepared = signed_expression(
        b"module spin@1 [ signature forever forall<S:stack> [ S -- S ! pure ] export forever def forever [ forever ] ]",
        b"spin@1.forever",
    )?;
    let owner = prepared
        .signed_source_owner()
        .ok_or("missing private signed source owner")?;
    owner.rederive().map_err(|error| {
        format!(
            "sealed source rederive refused: {}",
            error.diagnostic().message
        )
    })?;
    let submission = prepared
        .submission()
        .ok_or("missing signed executable submission")?;
    let definition = submission
        .definitions
        .iter()
        .find(|definition| {
            definition
                .body
                .candidate
                .nodes
                .iter()
                .any(|node| matches!(node, Node::SignedSelf { .. }))
        })
        .ok_or("missing signed owner body")?;
    let mut env = submission.environment.clone();
    env.caller_module = env.definition_owners[definition.definition.0 as usize];
    let request = Request {
        input_bytes: submission.request.input_bytes,
        expected: definition.expected.clone(),
        limits: submission.request.limits,
    };
    if !matches!(
        noble_kernel::acceptance::check(&env, &request, &definition.body.candidate),
        Outcome::Unsupported(_)
    ) {
        return Err("public kernel admitted explicit signed self".into());
    }
    let compiler = Compiler::new();
    if compiler.prepare(submission).is_ok() {
        return Err("raw Wasm compiler emitted explicit signed self".into());
    }
    compiler.prepare_signed(&owner).map_err(|error| {
        format!(
            "signed source refused by independent compiler: {}",
            wasm_error(error)
        )
    })?;

    // The ordinary Named variant with the self edge omitted from Env.deps is
    // a coherent public-DTO forgery. Only the full Wasm submission audit can
    // detect that missing edge; bare kernel preflight has no whole graph.
    let mut omitted = submission.clone();
    let body = omitted
        .definitions
        .iter_mut()
        .find(|entry| entry.definition == definition.definition)
        .ok_or("missing forged owner")?;
    for node in &mut body.body.candidate.nodes {
        if let Node::SignedSelf { def, inst } = node {
            *node = Node::Invocation {
                def: *def,
                inst: inst.clone(),
            };
        }
    }
    if compiler.prepare(&omitted).is_ok() {
        return Err("raw Wasm compiler accepted an omitted ordinary Named self edge".into());
    }
    Ok(())
}

#[test]
fn one_self_site_must_match_the_concrete_owner_body_not_itself() -> Result<(), String> {
    let prepared = signed_expression(
        b"module binder@1 [ signature identity forall<S:stack,A:value> [ S A -- S A ! pure ] export identity def identity [ [ identity ] drop ] ]",
        b"1 binder@1.identity",
    )?;
    let submission = prepared.submission().ok_or("missing signed submission")?;
    let definition = submission
        .definitions
        .iter()
        .find(|definition| definition.owner_inst.is_some())
        .ok_or("missing signed owner concrete binder")?;
    let owner_inst = definition.owner_inst.as_ref().ok_or("missing owner Inst")?;
    let mut env = submission.environment.clone();
    env.caller_module = env
        .definition_owners
        .get(usize::try_from(definition.definition.0).map_err(|_| "owner index overflow")?)
        .copied()
        .flatten();
    let request = Request {
        input_bytes: submission.request.input_bytes,
        expected: definition.expected.clone(),
        limits: submission.request.limits,
    };
    if !matches!(
        noble_kernel::acceptance::check_signed_structure(
            &env,
            &request,
            &definition.body.candidate,
            definition.definition,
            owner_inst,
        ),
        Outcome::Accepted(_)
    ) {
        return Err("source-derived owner body did not obtain structural kernel check".into());
    }
    let mut forged = definition.body.candidate.clone();
    let mut altered = false;
    for node in &mut forged.nodes {
        match node {
            Node::SignedSelf { inst, .. } => {
                let Some(binding) = inst.bindings.get_mut(1) else {
                    return Err("signed value binder is absent".into());
                };
                *binding = noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Unit);
                altered = true;
            }
            Node::Quotation { inst, .. } => {
                for binder in [1, 2] {
                    let Some(binding) = inst.bindings.get_mut(binder) else {
                        return Err("quoted Program binder is absent".into());
                    };
                    *binding =
                        noble_kernel::words::Binding::Stack(vec![noble_kernel::types::Ty::Unit]);
                }
            }
            _ => {}
        }
    }
    if !altered
        || !matches!(
            noble_kernel::acceptance::check_signed_structure(
                &env,
                &request,
                &forged,
                definition.definition,
                owner_inst,
            ),
            Outcome::Unsupported(_)
        )
    {
        return Err("single self site escaped owner-body value binder substitution".into());
    }
    Ok(())
}

#[test]
fn dropped_quotation_cannot_change_the_owner_stack_row() -> Result<(), String> {
    let prepared = signed_expression(
        b"module binder@1 [ signature identity forall<S:stack> [ S -- S ! pure ] export identity def identity [ [ identity ] drop ] ]",
        b"1 binder@1.identity",
    )?;
    let submission = prepared.submission().ok_or("missing signed submission")?;
    let definition = submission
        .definitions
        .iter()
        .find(|definition| definition.owner_inst.is_some())
        .ok_or("missing signed stack owner")?;
    let mut env = submission.environment.clone();
    env.caller_module = env
        .definition_owners
        .get(usize::try_from(definition.definition.0).map_err(|_| "owner index overflow")?)
        .copied()
        .flatten();
    let request = Request {
        input_bytes: submission.request.input_bytes,
        expected: definition.expected.clone(),
        limits: submission.request.limits,
    };
    let mut forged = definition.body.candidate.clone();
    let mut changed = false;
    for node in &mut forged.nodes {
        match node {
            Node::SignedSelf { inst, .. } => {
                *inst
                    .bindings
                    .get_mut(0)
                    .ok_or("missing signed stack binder")? =
                    noble_kernel::words::Binding::Stack(vec![]);
                changed = true;
            }
            Node::Quotation { inst, .. } => {
                for binder in [1, 2] {
                    *inst
                        .bindings
                        .get_mut(binder)
                        .ok_or("missing quoted stack binder")? =
                        noble_kernel::words::Binding::Stack(vec![]);
                }
            }
            _ => {}
        }
    }
    if !changed
        || !matches!(
            noble_kernel::acceptance::check_signed_structure(
                &env,
                &request,
                &forged,
                definition.definition,
                definition.owner_inst.as_ref().ok_or("missing owner Inst")?,
            ),
            Outcome::Unsupported(_)
        )
    {
        return Err("latent quotation changed the rigid owner-body stack row".into());
    }
    Ok(())
}

#[test]
fn signed_structural_owner_witness_caps_untrusted_stack_effect_and_type_before_clone(
) -> Result<(), String> {
    let prepared = signed_expression(
        b"module binder@1 [ signature identity forall<S:stack,A:value> [ S A -- S A ! pure ] export identity def identity [ [ identity ] drop ] ]",
        b"1 binder@1.identity",
    )?;
    let submission = prepared.submission().ok_or("missing signed submission")?;
    let definition = submission
        .definitions
        .iter()
        .find(|definition| definition.owner_inst.is_some())
        .ok_or("missing signed owner")?;
    let mut env = submission.environment.clone();
    env.caller_module = env
        .definition_owners
        .get(usize::try_from(definition.definition.0).map_err(|_| "owner index overflow")?)
        .copied()
        .flatten();
    let request = Request {
        input_bytes: submission.request.input_bytes,
        expected: definition.expected.clone(),
        limits: noble_kernel::untrusted::Limits {
            work: 5_000,
            ..submission.request.limits
        },
    };
    let original = definition.owner_inst.as_ref().ok_or("missing owner Inst")?;
    if !matches!(
        noble_kernel::acceptance::check_signed_structure(
            &env,
            &request,
            &definition.body.candidate,
            definition.definition,
            original,
        ),
        Outcome::Accepted(_)
    ) {
        return Err("valid owner did not fit bounded structural control budget".into());
    }
    let mut huge_stack = original.clone();
    *huge_stack
        .bindings
        .get_mut(0)
        .ok_or("missing stack binder")? =
        noble_kernel::words::Binding::Stack(vec![noble_kernel::types::Ty::I64; 6_000]);
    if !matches!(
        noble_kernel::acceptance::check_signed_structure(
            &env,
            &request,
            &definition.body.candidate,
            definition.definition,
            &huge_stack,
        ),
        Outcome::Exhausted(noble_kernel::untrusted::LimitKind::StackHeight)
    ) {
        return Err("oversized owner stack was traversed rather than bounded".into());
    }
    let effects: Vec<_> = (1..=6_000).map(noble_kernel::types::EffId).collect();
    let mut huge_effects = original.clone();
    *huge_effects
        .bindings
        .get_mut(1)
        .ok_or("missing value binder")? =
        noble_kernel::words::Binding::Effect(noble_kernel::types::EffSet::from_ids(&effects));
    if !matches!(
        noble_kernel::acceptance::check_signed_structure(
            &env,
            &request,
            &definition.body.candidate,
            definition.definition,
            &huge_effects,
        ),
        Outcome::Exhausted(noble_kernel::untrusted::LimitKind::Work)
    ) {
        return Err("oversized owner effect vector escaped request work metering".into());
    }
    let mut deep_ty = noble_kernel::types::Ty::I64;
    for _ in 0..1_000 {
        deep_ty = noble_kernel::types::Ty::List(Box::new(deep_ty));
    }
    let mut huge_value = original.clone();
    *huge_value
        .bindings
        .get_mut(1)
        .ok_or("missing value binder")? = noble_kernel::words::Binding::Value(deep_ty);
    if !matches!(
        noble_kernel::acceptance::check_signed_structure(
            &env,
            &request,
            &definition.body.candidate,
            definition.definition,
            &huge_value,
        ),
        Outcome::Exhausted(noble_kernel::untrusted::LimitKind::TypeSize)
    ) {
        return Err("deep owner type was cloned before a borrowed size guard".into());
    }
    Ok(())
}

#[test]
fn two_matching_forged_quoted_sites_do_not_define_the_owner_binder() -> Result<(), String> {
    let prepared = signed_expression(
        b"module binder@1 [ signature identity forall<S:stack,A:value> [ S A -- S A ! pure ] export identity def identity [ [ identity ] drop [ identity ] drop ] ]",
        b"1 binder@1.identity",
    )?;
    let submission = prepared.submission().ok_or("missing signed submission")?;
    let definition = submission
        .definitions
        .iter()
        .find(|definition| definition.owner_inst.is_some())
        .ok_or("missing signed owner binder")?;
    let mut env = submission.environment.clone();
    env.caller_module = env
        .definition_owners
        .get(usize::try_from(definition.definition.0).map_err(|_| "owner index overflow")?)
        .copied()
        .flatten();
    let request = Request {
        input_bytes: submission.request.input_bytes,
        expected: definition.expected.clone(),
        limits: submission.request.limits,
    };
    let mut forged = definition.body.candidate.clone();
    let mut self_sites = 0usize;
    for node in &mut forged.nodes {
        match node {
            Node::SignedSelf { inst, .. } => {
                let Some(binding) = inst.bindings.get_mut(1) else {
                    return Err("signed value binder is absent".into());
                };
                *binding = noble_kernel::words::Binding::Value(noble_kernel::types::Ty::Unit);
                self_sites += 1;
            }
            Node::Quotation { inst, .. } => {
                for binder in [1, 2] {
                    let Some(binding) = inst.bindings.get_mut(binder) else {
                        return Err("quoted Program binder is absent".into());
                    };
                    *binding =
                        noble_kernel::words::Binding::Stack(vec![noble_kernel::types::Ty::Unit]);
                }
            }
            _ => {}
        }
    }
    if self_sites != 2
        || !matches!(
            noble_kernel::acceptance::check_signed_structure(
                &env,
                &request,
                &forged,
                definition.definition,
                definition.owner_inst.as_ref().ok_or("missing owner Inst")?,
            ),
            Outcome::Unsupported(_)
        )
    {
        return Err("matching forged quotation sites escaped the concrete owner binder".into());
    }
    Ok(())
}

#[test]
fn partial_multiowner_certificate_keeps_public_alias_inert_but_rederives_selected_owner(
) -> Result<(), String> {
    let prepared = signed_expression(
        b"module dual@1 [ signature left forall<S:stack> [ S -- S ! pure ] export left def left [ left ] signature right forall<S:stack> [ S I64 -- S I64 ! pure ] export right def right [ right ] ]",
        b"dual@1.left",
    )?;
    let owner = prepared
        .signed_source_owner()
        .ok_or("missing left signed owner")?;
    owner.rederive().map_err(|error| {
        format!(
            "left sealed source rederive refused: {}",
            error.diagnostic().message
        )
    })?;
    Compiler::new().prepare_signed(&owner).map_err(|error| {
        format!(
            "inert second owner obstructed private left rederivation: {}",
            wasm_error(error)
        )
    })?;
    Ok(())
}

#[test]
fn transitive_same_offset_signed_owners_keep_each_fresh_rank_one_substitution() -> Result<(), String>
{
    let prepared = signed_expression(
        b"module chain@1 [ signature c forall<S:stack> [ S -- S ! pure ] def c [ c ] signature b forall<S:stack> [ S -- S ! pure ] def b [ c b ] signature a forall<S:stack> [ S -- S ! pure ] export a def a [ b a ] ]",
        b"chain@1.a",
    )?;
    let source = prepared
        .signed_source_owner()
        .ok_or("missing acyclic signed owners")?;
    let submission = source
        .rederive()
        .map_err(|error| error.diagnostic().message.clone())?;
    let owners: Vec<_> = submission
        .definitions
        .iter()
        .filter(|definition| definition.owner_inst.is_some())
        .collect();
    if owners.len() != 3
        || owners.iter().any(|definition| {
            submission
                .environment
                .scheme(definition.definition)
                .is_none_or(|scheme| scheme.var_kinds.len() != 1)
        })
    {
        return Err("acyclic signed owners lost separate original rank-1 signatures".into());
    }
    Compiler::new()
        .prepare_signed(&source)
        .map_err(|error| format!("acyclic signed invocation refused: {}", wasm_error(error)))?;
    Ok(())
}

#[test]
fn quoted_self_effect_sites_cannot_rebind_the_independent_owner_effect() -> Result<(), String> {
    for (module, sites) in [
        (b"module effect@1 [ signature loop forall<S:stack,E:effect> [ S -- S ! E ] export loop def loop [ loop [ loop ] drop ] ]".as_slice(), 2usize),
        (b"module effect@1 [ signature loop forall<S:stack,E:effect> [ S -- S ! E ] export loop def loop [ loop [ loop ] drop [ loop ] drop ] ]".as_slice(), 3usize),
    ] {
        let prepared = signed_expression(module, b"effect@1.loop")?;
        let source = prepared.signed_source_owner().ok_or("missing effect source owner")?;
        Compiler::new().prepare_signed(&source)
            .map_err(|error| format!("valid effect owner refused: {}", wasm_error(error)))?;
        let submission = prepared.submission().ok_or("missing effect submission")?;
        let definition = submission.definitions.iter()
            .find(|definition| definition.owner_inst.is_some())
            .ok_or("missing effect owner Inst")?;
        let mut env = submission.environment.clone();
        env.caller_module = env.definition_owners
            .get(usize::try_from(definition.definition.0).map_err(|_| "owner index overflow")?)
            .copied().flatten();
        let request = Request {
            input_bytes: submission.request.input_bytes,
            expected: definition.expected.clone(),
            limits: submission.request.limits,
        };
        let owner_inst = definition.owner_inst.as_ref().ok_or("missing effect owner Inst")?;
        if !matches!(noble_kernel::acceptance::check_signed_structure(
            &env, &request, &definition.body.candidate, definition.definition, owner_inst,
        ), Outcome::Accepted(_)) {
            return Err("valid source-certified owner effect binder was not structurally checked".into());
        }
        let mut forged = definition.body.candidate.clone();
        let owner_effect = match owner_inst.bindings.get(1) {
            Some(noble_kernel::words::Binding::Effect(effect)) => effect,
            _ => return Err("owner's second binding is not an effect".into()),
        };
        let alternative = noble_kernel::words::Binding::Effect(
            if owner_effect.as_slice().is_empty() {
                noble_kernel::types::EffSet::from_ids(&[noble_kernel::contracts::TEST_EMIT])
            } else {
                noble_kernel::types::EffSet::empty()
            }
        );
        let mut encountered = 0usize;
        let mut altered = 0usize;
        for node in &mut forged.nodes {
            if let Node::SignedSelf { inst, .. } = node {
                if encountered > 0 {
                    *inst.bindings.get_mut(1).ok_or("missing quoted self effect binder")? =
                        alternative.clone();
                    altered += 1;
                }
                encountered += 1;
            }
        }
        let outcome = noble_kernel::acceptance::check_signed_structure(
            &env, &request, &forged, definition.definition, owner_inst,
        );
        if encountered != sites || altered != sites - 1
            || !matches!(outcome, Outcome::Unsupported(_)) {
            return Err(format!(
                "matching quoted self effect sites escaped independent body Inst: expected {sites}, found {encountered}, altered {altered}, outcome {outcome:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn nested_quotation_self_is_source_bound_and_foreign_cycles_preserve_namespace(
) -> Result<(), String> {
    let prepared = signed_expression(
        b"module nested@1 [ signature callable forall<S:stack> [ S -- S ! pure ] export callable def callable [ [ callable ] drop ] ]",
        b"nested@1.callable",
    )?;
    let owner = prepared
        .signed_source_owner()
        .ok_or("quoted self did not mint source owner")?;
    owner.rederive().map_err(|error| {
        format!(
            "quoted sealed source rederive refused: {}",
            error.diagnostic().message
        )
    })?;
    Compiler::new()
        .prepare_signed(&owner)
        .map_err(|error| format!("quoted owner refused: {}", wasm_error(error)))?;

    let session = ModuleSession::new_signed_self(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let bad = b"module bad@1 [ signature even forall<S:stack> [ S -- S ! pure ] signature odd forall<S:stack> [ S -- S ! pure ] def even [ odd ] def odd [ even ] ]";
    if !matches!(session.prepare(bad, &[], LIMITS), Err(error) if error.stage() == Stage::Resolve)
        || session.generation() != 0
    {
        return Err("mutual recursion published or evaded the source scheduler".into());
    }
    if !matches!(session.prepare(
        b"module no_signature@1 [ def recur [ recur ] ]", &[], LIMITS,
    ), Err(error) if error.stage() == Stage::Resolve)
    {
        return Err("unsigned lexical self acquired an owner without a signature".into());
    }
    if !matches!(session.prepare(
        b"module polymorphic@1 [ signature recur forall<S:stack> [ S -- S ! pure ] def recur [ 1 recur ] ]",
        &[], LIMITS,
    ), Err(error) if error.stage() == Stage::Check)
    {
        return Err("recursive call changed the bound owner stack row".into());
    }
    let emit = noble_contracts::source::BoundOperation {
        module_name: "ledger".into(),
        module_version: 1,
        operation: "test.emit".into(),
        adapter_identity: "version-A".into(),
        adapter_slot: 3,
        input: vec![noble_kernel::types::Ty::Text],
        output: vec![],
        effects: vec![noble_kernel::contracts::TEST_EMIT],
    };
    let effect_session = ModuleSession::new_signed_self(
        &[emit],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let narrowed = b"module ledger@1 [ require emit Text -- ! test.emit signature leak forall<S:stack,eps:effect> [ S -- S ! eps ] export leak def leak [ \"event\" emit [ leak ] drop ] ]";
    if !matches!(effect_session.prepare(narrowed, &[], LIMITS),
        Err(error) if error.stage() == Stage::Check)
        || effect_session.generation() != 0
    {
        return Err("signed owner narrowed a rigid effect variable to an emitted effect".into());
    }
    let unsigned = ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    if !matches!(unsigned.prepare(
        b"module no@1 [ signature spin forall<S:stack> [ S -- S ! pure ] def spin [ spin ] ]",
        &[], LIMITS,
    ), Err(error) if error.stage() == Stage::Resolve)
    {
        return Err("ordinary declared profile accepted a self cycle".into());
    }
    let next = session
        .prepare(
            b"module valid@1 [ export value def value [ 1 ] ]",
            &[],
            LIMITS,
        )
        .map_err(|error| error.diagnostic().message.clone())?;
    let (_, outcome) = session.commit(next);
    outcome.map_err(|error| error.diagnostic().message.clone())
}
