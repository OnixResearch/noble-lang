//! Production-linked, source-derived kernel adversarial checks; no guest interpreter.
use anyhow::{bail, ensure, Context, Result};
use noble_contracts::{source::{BoundOperation, ModuleKind, ModuleSession}, Limits};
use noble_kernel::{
    acceptance,
    contracts::{self, Behavior, Definition, Env},
    execution::Submission,
    types::{EffSet, NominalShape, Ty},
    untrusted::{Candidate, Constraint, Node, NodeId, Outcome, Request},
    words::{Binding, Inst},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

fn source_error(problem: noble_contracts::source::Error) -> anyhow::Error {
    anyhow::anyhow!("production source {:?}: {}", problem.stage(), problem.diagnostic().message)
}
fn input(document: &Value, field: &str) -> Result<Vec<u8>> {
    let path = document.get(field).and_then(Value::as_str)
        .with_context(|| format!("missing frozen input path {field}"))?;
    fs::read(path).with_context(|| format!("reading frozen {field}: {path}"))
}
fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn register(session: ModuleSession, source: &[u8]) -> Result<ModuleSession> {
    let prepared = session.prepare(source, &[], Limits::default()).map_err(source_error)?;
    ensure!(matches!(prepared.kind(), ModuleKind::Module),
        "source was not registered as an actual module");
    let (session, committed) = session.commit(prepared);
    committed.map_err(source_error)?;
    Ok(session)
}
fn submission(session: &ModuleSession, source: &[u8], inputs: &[Ty]) -> Result<Submission> {
    let prepared = session.prepare(source, inputs, Limits::default()).map_err(source_error)?;
    ensure!(matches!(prepared.kind(), ModuleKind::Expression),
        "candidate did not come from a real source expression");
    prepared.submission().cloned().context("no production source submission")
}
fn accepted(label: &str, env: &Env, request: &Request, candidate: &Candidate) -> Result<()> {
    match acceptance::check(env, request, candidate) {
        Outcome::Accepted(_) => Ok(()),
        other => bail!("{label}: production kernel did not accept baseline: {other:?}"),
    }
}
fn invalid(label: &str, env: &Env, request: &Request, candidate: &Candidate) -> Result<Constraint> {
    match acceptance::check(env, request, candidate) {
        Outcome::Invalid(diagnostic) => Ok(diagnostic.constraint),
        other => bail!("{label}: production kernel did not issue typed invalid result: {other:?}"),
    }
}
fn record(name: &str, observed: Constraint, expected: Constraint, checks: usize) -> Result<Value> {
    ensure!(observed == expected,
        "{name}: wrong typed kernel constraint: expected {expected:?}, observed {observed:?}");
    ensure!(checks >= 2, "{name}: missing production baseline or hostile kernel check");
    let (kind, definition, effect) = match &observed {
        Constraint::PrivateDefinition(def) => ("PrivateDefinition", Some(def.0), None),
        Constraint::StackJoin => ("StackJoin", None, None),
        Constraint::InvalidType => ("InvalidType", None, None),
        Constraint::EffectInclusion(effect) => ("EffectInclusion", None, Some(effect.0)),
        Constraint::InvalidContract => ("InvalidContract", None, None),
        other => bail!("{name}: unexpected typed constraint after equality: {other:?}"),
    };
    Ok(json!({ "name": name, "production_kernel_check_called": true,
        "kernel_calls": checks, "baseline_outcome": "accepted", "kernel_outcome": "invalid",
        "accepted": false, "constraint_kind": kind, "constraint_definition": definition,
        "constraint_effect": effect,
        "constraint": format!("{observed:?}") }))
}
fn nominal(session: &ModuleSession, name: &str) -> Result<Ty> {
    session.resolve_type(name).map_err(source_error)
}
fn owner_candidate(session: &ModuleSession) -> Result<Value> {
    let source = submission(session, b"ledger@1.make_user", &[])?;
    let user = nominal(session, "ledger@1.UserId")?;
    let Ty::Nominal(user_id, user_shape) = user else { bail!("UserId lost nominal identity"); };
    ensure!(matches!(user_shape.as_ref(), NominalShape::Opaque(inner) if inner.as_ref() == &Ty::I64),
        "private constructor fixture lost exact opaque I64 representation");
    let definition = source.definitions.iter().find(|item| item.body.candidate.nodes.iter().any(|node| {
        matches!(node, Node::Invocation { def, .. } if matches!(source.environment.kind(*def), Some(Behavior::NominalNew(id)) if id == user_id))
    })).context("no real source-defined private UserId constructor invocation")?;
    let constructor = definition.body.candidate.nodes.iter().find_map(|node| match node {
        Node::Invocation { def, .. }
            if source.environment.kind(*def) == Some(Behavior::NominalNew(user_id)) => Some(*def),
        _ => None,
    }).context("missing exact source-derived private constructor definition")?;
    let declaration = source.environment.nominal(user_id).context("missing checked UserId schema")?;
    ensure!(&declaration.shape == user_shape.as_ref() && !declaration.public[0],
        "private UserId constructor does not match its checked nominal schema");
    let mut request = source.request.clone();
    request.expected = definition.expected.clone();
    let index = usize::try_from(definition.definition.0).context("definition index overflow")?;
    let owner = source.environment.definition_owners.get(index).copied().flatten()
        .context("missing trusted definition owner")?;
    let mut environment = source.environment.clone();
    environment.caller_module = Some(owner);
    accepted("owner private constructor", &environment, &request, &definition.body.candidate)?;
    environment.caller_module = None;
    let reason = invalid("foreign private constructor", &environment, &request,
        &definition.body.candidate)?;
    record("forged-private-constructor-owner", reason,
        Constraint::PrivateDefinition(constructor), 2)
}
fn private_arm_match(session: &ModuleSession) -> Result<Value> {
    let status = nominal(session, "ledger@1.Status")?;
    let Ty::Nominal(status_id, status_shape) = &status else { bail!("Status lost nominal identity"); };
    ensure!(matches!(status_shape.as_ref(), NominalShape::Variant(left, right)
        if left.as_ref() == &Ty::I64 && right.as_ref() == &Ty::Text),
        "Status matcher fixture lost exact I64/Text variant shape");
    let source = submission(session, b"ledger@1.inspect_status", std::slice::from_ref(&status))?;
    let definition = source.definitions.iter().find(|item| item.body.candidate.nodes.iter().any(|node| {
        matches!(node, Node::Invocation { def, .. } if matches!(source.environment.kind(*def), Some(Behavior::NominalMatch(id)) if id == *status_id))
    })).context("no actual source-defined owner-only Status.match")?;
    let matcher = definition.body.candidate.nodes.iter().find_map(|node| match node {
        Node::Invocation { def, .. }
            if source.environment.kind(*def) == Some(Behavior::NominalMatch(*status_id)) => Some(*def),
        _ => None,
    }).context("missing exact source-derived private-arm matcher definition")?;
    let declaration = source.environment.nominal(*status_id).context("missing checked Status schema")?;
    ensure!(&declaration.shape == status_shape.as_ref() && declaration.public == [true, false],
        "private-arm matcher is not a checked public/private two-arm variant");
    let mut request = source.request.clone();
    request.expected = definition.expected.clone();
    let index = usize::try_from(definition.definition.0).context("definition index overflow")?;
    let owner = source.environment.definition_owners.get(index).copied().flatten()
        .context("owner of private-arm matcher is missing")?;
    let mut environment = source.environment.clone();
    environment.caller_module = Some(owner);
    accepted("owner exhaustive private-arm match", &environment, &request, &definition.body.candidate)?;
    environment.caller_module = None;
    let reason = invalid("foreign private-arm match", &environment, &request,
        &definition.body.candidate)?;
    record("forged-private-arm-match", reason,
        Constraint::PrivateDefinition(matcher), 2)
}
fn checked_i64_dup(session: &ModuleSession) -> Result<Submission> {
    submission(session, b"dup", &[Ty::I64])
}
fn same_layout(session: &ModuleSession) -> Result<Value> {
    let user = nominal(session, "ledger@1.UserId")?;
    let order = nominal(session, "ledger@1.OrderId")?;
    let (Ty::Nominal(user_id, user_shape), Ty::Nominal(order_id, order_shape)) = (&user, &order) else {
        bail!("same-layout declared types lost nominal identity");
    };
    ensure!(user_id != order_id && user_shape == order_shape
        && matches!(user_shape.as_ref(), NominalShape::Opaque(inner) if inner.as_ref() == &Ty::I64),
        "same-layout control does not isolate distinct declaration ordinals");
    let original = submission(session, b"ledger@1.make_user", &[])?;
    accepted("real owner-produced UserId", &original.environment, &original.request,
        &original.body.candidate)?;
    ensure!(original.request.expected.stack_out == [user],
        "actual source result was not the UserId nominal");
    let mut request = original.request.clone();
    request.expected.stack_out = vec![order];
    let reason = invalid("same-layout UserId to OrderId", &original.environment,
        &request, &original.body.candidate)?;
    record("forged-same-layout-nominal", reason, Constraint::StackJoin, 2)
}
fn substitute_i64(ty: &mut Ty, replacement: &Ty) -> usize {
    match ty {
        Ty::I64 => { *ty = replacement.clone(); 1 }
        Ty::Pair(left, right) | Ty::Sum(left, right) =>
            substitute_i64(left, replacement) + substitute_i64(right, replacement),
        Ty::List(item) => substitute_i64(item, replacement),
        Ty::Program(inputs, outputs, _) => inputs.iter_mut().chain(outputs.iter_mut())
            .map(|item| substitute_i64(item, replacement)).sum(),
        Ty::Nominal(_, _) | Ty::Resource(_) | Ty::Unit | Ty::Bool | Ty::Text
        | Ty::Syntax | Ty::Contract | Ty::Evidence | Ty::Certified => 0,
    }
}
fn resource_eligibility(session: &ModuleSession, operation: &str, source: &[u8]) -> Result<Value> {
    let resource = nominal(session, "ledger@1.CounterOwner")?;
    let Ty::Nominal(id, shape) = &resource else { bail!("resource fixture lost nominal identity"); };
    let NominalShape::Opaque(inner) = shape.as_ref() else { bail!("resource fixture is not opaque"); };
    let Ty::Resource(kind) = inner.as_ref() else { bail!("nominal representation is not Resource"); };
    ensure!(*kind == contracts::FIXTURE_RESOURCE,
        "{operation}: nominal payload did not retain the registered fixture ResourceKind");
    ensure!(session.prepare(source, std::slice::from_ref(&resource), Limits::default()).is_err(),
        "{operation}: frontend accepted invalid resource use before kernel peer check");
    let original = submission(session, source, &[Ty::I64])?;
    accepted(&format!("baseline {operation} I64"), &original.environment,
        &original.request, &original.body.candidate)?;
    ensure!(original.environment.resource_kinds.contains(kind),
        "{operation}: host did not validate fixture Resource kind");
    ensure!(original.environment.nominal(*id).is_some_and(|decl| &decl.shape == shape.as_ref()),
        "{operation}: source-derived exact nominal schema missing from checked environment");
    let mut request = original.request.clone();
    let mut changed = 0;
    for ty in request.expected.stack_in.iter_mut().chain(request.expected.stack_out.iter_mut()) {
        changed += substitute_i64(ty, &resource);
    }
    ensure!(changed >= 1, "{operation}: no source-derived I64 type to replace");
    let mut candidate = original.body.candidate.clone();
    let mut witnesses = 0;
    for node in &mut candidate.nodes {
        if let Node::Invocation { inst, .. } = node {
            for binding in &mut inst.bindings {
                if let Binding::Value(Ty::I64) = binding {
                    *binding = Binding::Value(resource.clone());
                    witnesses += 1;
                }
            }
        }
    }
    ensure!(witnesses == 1, "{operation}: expected one actual source-derived Data witness");
    let actual = acceptance::check(&original.environment, &request, &candidate);
    let diagnostic = match actual {
        Outcome::Invalid(diagnostic) => diagnostic,
        other => bail!("{operation}: kernel did not reject resource use: {other:?}"),
    };
    ensure!(matches!(diagnostic.constraint, Constraint::Eligibility(ref ty) if ty == &resource),
        "{operation}: kernel did not derive Eligibility on exact checked nominal representation: {:?}",
        diagnostic.constraint);
    Ok(json!({ "operation": operation, "production_kernel_check_called": true,
        "baseline_outcome": "accepted", "kernel_outcome": "invalid",
        "constraint_kind": "Eligibility", "nominal_origin": "ledger@1.CounterOwner",
        "checked_nominal_id": { "module": id.module.to_string(), "ordinal": id.ordinal },
        "checked_nominal_shape": { "kind": "opaque-resource", "resource_kind": kind.0 },
        "known_fixture_resource_kind": true, "kernel_calls": 2 }))
}
fn forged_shape(session: &ModuleSession) -> Result<Value> {
    let original = checked_i64_dup(session)?;
    accepted("I64 duplication", &original.environment, &original.request,
        &original.body.candidate)?;
    let real = nominal(session, "ledger@1.CounterOwner")?;
    let Ty::Nominal(id, shape) = real else { bail!("declared CounterOwner lost nominal identity"); };
    ensure!(matches!(shape.as_ref(), NominalShape::Opaque(inner) if matches!(inner.as_ref(), Ty::Resource(_))),
        "fixture resource wrapper did not retain the real Resource kind");
    ensure!(original.environment.nominal(id).is_some_and(|decl| &decl.shape == shape.as_ref()),
        "forged schema control did not retain the actual checked Resource nominal shape");
    let forged = Ty::Nominal(id, Box::new(NominalShape::Opaque(Box::new(Ty::I64))));
    let mut request = original.request.clone();
    request.expected.stack_in = vec![forged.clone()];
    request.expected.stack_out = vec![forged.clone(), forged.clone()];
    let mut candidate = original.body.candidate.clone();
    let mut changed = 0;
    for node in &mut candidate.nodes {
        if let Node::Invocation { inst, .. } = node {
            for binding in &mut inst.bindings {
                if matches!(binding, Binding::Value(Ty::I64)) {
                    *binding = Binding::Value(forged.clone());
                    changed += 1;
                }
            }
        }
    }
    ensure!(changed == 1, "did not mutate exactly one actual source-derived dup type witness");
    let reason = invalid("forged resource-free nominal schema", &original.environment,
        &request, &candidate)?;
    record("forged-resource-free-schema", reason, Constraint::InvalidType, 2)
}
fn structural_sum(session: &ModuleSession) -> Result<Value> {
    let original = submission(session, b"3", &[])?;
    ensure!(original.body.candidate.nodes.len() == 1, "unexpected literal source candidate");
    let id = original.environment.kinds.iter().position(|kind| *kind == Behavior::Inl)
        .context("no structural sum constructor in production environment")?;
    let mut candidate = original.body.candidate.clone();
    let new_node = NodeId(u32::try_from(candidate.nodes.len()).context("node index overflow")?);
    candidate.nodes.push(Node::Invocation { def: Definition(u32::try_from(id)?),
        inst: Inst { bindings: vec![Binding::Stack(vec![]), Binding::Value(Ty::I64),
            Binding::Value(Ty::Text)] } });
    candidate.body.push(new_node);
    let mut request = original.request.clone();
    request.expected.stack_out = vec![Ty::Sum(Box::new(Ty::I64), Box::new(Ty::Text))];
    accepted("structural inl from source literal", &original.environment, &request, &candidate)?;
    let status = nominal(session, "ledger@1.Status")?;
    let Ty::Nominal(status_id, status_shape) = &status else { bail!("Status lost nominal identity"); };
    ensure!(matches!(status_shape.as_ref(), NominalShape::Variant(left, right)
        if left.as_ref() == &Ty::I64 && right.as_ref() == &Ty::Text)
        && original.environment.nominal(*status_id)
            .is_some_and(|decl| &decl.shape == status_shape.as_ref()),
        "structural Sum control did not target the checked I64/Text nominal Status");
    request.expected.stack_out = vec![status];
    let reason = invalid("raw Sum to nominal Status", &original.environment, &request,
        &candidate)?;
    record("raw-sum-to-nominal", reason, Constraint::StackJoin, 2)
}
fn bound_session(emitter_source: &[u8]) -> Result<ModuleSession> {
    let operation = BoundOperation { module_name: "ledger".into(), module_version: 1,
        operation: "test.emit".into(), adapter_identity: "version-A".into(), adapter_slot: 0,
        input: vec![Ty::Text], output: vec![], effects: vec![contracts::TEST_EMIT] };
    let session = ModuleSession::new(&[operation]).map_err(source_error)?;
    register(session, emitter_source)
}
fn bound_submission(emitter_source: &[u8]) -> Result<Submission> {
    submission(&bound_session(emitter_source)?, b"ledger@1.handle", &[])
}
fn wrong_effect(original: &Submission) -> Result<Value> {
    accepted("real bound-effect source", &original.environment, &original.request,
        &original.body.candidate)?;
    let mut request = original.request.clone();
    request.expected.allowed_effects = EffSet::empty();
    let reason = invalid("erased declared test.emit effect", &original.environment,
        &request, &original.body.candidate)?;
    record("wrong-effect-witness", reason,
        Constraint::EffectInclusion(contracts::TEST_EMIT), 2)
}
fn forged_binding(original: &Submission) -> Result<Value> {
    accepted("real bound-adapter contract", &original.environment, &original.request,
        &original.body.candidate)?;
    let mut environment = original.environment.clone();
    let row = environment.bound_adapters.iter_mut().find(|row| row.adapter_identity == "version-A")
        .context("no actual bound adapter row in checked source environment")?;
    ensure!(row.input == [Ty::Text] && row.output.is_empty()
        && row.effects == EffSet::from_ids(&[contracts::TEST_EMIT]),
        "baseline manifest row was not the checked Text--/test.emit contract");
    row.input = vec![Ty::I64];
    let reason = invalid("forged bound-adapter input contract", &environment, &original.request,
        &original.body.candidate)?;
    record("forged-bound-adapter-contract", reason, Constraint::InvalidContract, 2)
}
fn ambient_bypass(emitter_source: &[u8], literal_source: &[u8]) -> Result<Value> {
    let session = bound_session(emitter_source)?;
    let original = submission(&session, literal_source, &[])?;
    ensure!(original.environment.declared_modules,
        "declared session did not retain trusted opt-in kernel context");
    accepted("declared source literal", &original.environment, &original.request,
        &original.body.candidate)?;
    ensure!(original.body.candidate.nodes.len() == 1, "ambient literal fixture is not one node");
    let legacy = original.environment.kinds.iter().position(|kind| *kind == Behavior::TestEmit)
        .context("missing legacy Core-Bootstrap test.emit definition")?;
    let mut candidate = original.body.candidate.clone();
    let node = NodeId(u32::try_from(candidate.nodes.len()).context("legacy node overflow")?);
    let legacy_def = Definition(u32::try_from(legacy)?);
    candidate.nodes.push(Node::Invocation { def: legacy_def,
        inst: Inst { bindings: vec![Binding::Stack(vec![])] } });
    candidate.body.push(node);
    let mut request = original.request.clone();
    request.expected.stack_out = vec![Ty::Unit];
    request.expected.allowed_effects = EffSet::from_ids(&[contracts::TEST_EMIT]);
    let reason = invalid("declared legacy test.emit bypass", &original.environment,
        &request, &candidate)?;
    record("ambient-test-emit-bypass", reason,
        Constraint::PrivateDefinition(legacy_def), 2)
}
fn stale_module_identity(session: ModuleSession, second_source: &[u8]) -> Result<(ModuleSession, Value)> {
    let session = register(session, second_source)?;
    let version_a = nominal(&session, "ledger@1.UserId")?;
    let version_b = nominal(&session, "ledger@2.UserId")?;
    let (Ty::Nominal(id_a, shape_a), Ty::Nominal(id_b, shape_b)) = (&version_a, &version_b) else {
        bail!("versioned UserId types lost checked nominal identity");
    };
    ensure!(id_a != id_b && id_a.module != id_b.module && id_a.ordinal == id_b.ordinal
        && shape_a == shape_b
        && matches!(shape_a.as_ref(), NominalShape::Opaque(inner) if inner.as_ref() == &Ty::I64),
        "different declared source versions did not retain separate equal-shape nominal ids");
    let original = submission(&session, b"ledger@1.make_user", &[])?;
    accepted("source module version-A type", &original.environment, &original.request,
        &original.body.candidate)?;
    ensure!(original.request.expected.stack_out == [version_a],
        "real source expected interface did not retain A's UserId");
    let mut request = original.request.clone();
    request.expected.stack_out = vec![version_b];
    let reason = invalid("stale nominal module identity", &original.environment,
        &request, &original.body.candidate)?;
    Ok((session, record("stale-module-identity", reason, Constraint::StackJoin, 2)?))
}
fn stale_preparation(module: &[u8], second: &[u8], import: &[u8]) -> Result<Value> {
    let session = ModuleSession::new(&[]).map_err(source_error)?;
    let session = register(session, module)?;
    let prepared = session.prepare(import, &[], Limits::default()).map_err(source_error)?;
    ensure!(matches!(prepared.kind(), ModuleKind::Import),
        "stale check did not prepare a genuine source import");
    let prepared_accepted = true;
    let session = register(session, second)?;
    let generation = session.generation();
    let prior_a = nominal(&session, "ledger@1.UserId")?;
    let prior_b = nominal(&session, "ledger@2.UserId")?;
    let (session, committed) = session.commit(prepared);
    let rejection = committed.expect_err("stale preparation was incorrectly committed");
    ensure!(rejection.stage() == noble_contracts::source::Stage::Acceptance,
        "stale preparation rejected at wrong production stage");
    ensure!(session.generation() == generation, "rejected commit mutated session generation");
    ensure!(nominal(&session, "ledger@1.UserId")? == prior_a
        && nominal(&session, "ledger@2.UserId")? == prior_b,
        "rejected stale commit changed resolved immutable module identity");
    ensure!(session.resolve_type("account.UserId").is_err(),
        "stale alias was published despite refused commit");
    Ok(json!({ "prepare_accepted": prepared_accepted,
        "intervening_commit_accepted": true, "old_commit_rejected": true,
        "namespace_unchanged_after_rejection": true, "generation": generation,
        "kernel_or_source_stage": format!("{:?}", rejection.stage()), "guest_requests": 0 }))
}
fn dependency_control(module: &[u8]) -> Result<Value> {
    let session = ModuleSession::new(&[]).map_err(source_error)?;
    let session = register(session, module)?;
    let source = submission(&session, b"ledger@1.handle", &[])?;
    let first = Definition(23);
    let referenced = Definition(24);
    ensure!(source.environment.kind(first) == Some(Behavior::Named)
        && source.environment.kind(referenced) == Some(Behavior::Named)
        && source.environment.definition_owners[23].is_some(),
        "no actual source-derived first Named definition at slot 23");
    ensure!(source.definitions.iter().any(|row| row.definition == first
        && row.body.candidate.nodes.iter().any(|node|
            matches!(node, Node::Invocation { def, .. } if *def == referenced)))
        && source.definitions.iter().any(|row| row.definition == referenced),
        "first Named body and its helper are not actually specialized from source");
    accepted("real source first-Named dependency", &source.environment, &source.request,
        &source.body.candidate)?;
    noble_wasm::source::Compiler::new().prepare(&source)
        .map_err(|error| anyhow::anyhow!("first Named backend baseline refused: {}",
            match error {
                noble_wasm::Diagnostic::Invalid => "Invalid",
                noble_wasm::Diagnostic::Unsupported => "Unsupported",
                noble_wasm::Diagnostic::Exhausted => "Exhausted",
                noble_wasm::Diagnostic::Defective => "Defective",
            }))?;
    let mut forged = source.clone();
    let edges = forged.environment.deps.get_mut(23).context("missing first Named edge row")?;
    let previous = edges.len();
    edges.retain(|edge| *edge != referenced);
    ensure!(previous == edges.len() + 1,
        "source backend did not retain exactly one required first-Named dependency");
    ensure!(matches!(noble_wasm::source::Compiler::new().prepare(&forged),
        Err(noble_wasm::Diagnostic::Invalid)),
        "backend accepted a missing source-derived dependency edge from first Named definition");
    Ok(json!({ "first_named_definition": first.0,
        "referenced_definition": referenced.0,
        "baseline_outcome": "accepted", "forged_outcome": "invalid",
        "production_backend_prepare_called": true }))
}
fn run(workload_path: &Path) -> Result<Value> {
    let workload_bytes = fs::read(workload_path).context("reading exact gate workload")?;
    let workload: Value = serde_json::from_slice(&workload_bytes)?;
    ensure!(workload["schema"] == "noble-declared-modules-kernel-workload/v1",
        "wrong peer workload schema");
    let module = input(&workload, "module_source")?;
    let second = input(&workload, "mutate_registry_source")?;
    let import = input(&workload, "stale_preparation_source")?;
    let emitter = input(&workload, "emitter_source")?;
    let host = input(&workload, "host_bindings")?;
    let resource_dup = input(&workload, "resource_dup_source")?;
    let resource_drop = input(&workload, "resource_drop_source")?;
    let resource_capture = input(&workload, "resource_capture_source")?;
    let ambient_literal = input(&workload, "ambient_literal_source")?;
    let dependency = input(&workload, "dependency_source")?;
    ensure!(host.as_slice() == b"bind ledger@1 test.emit version-A Text -- ! test.emit allow\n",
        "kernel peer input differs from actual selected host binding bytes");
    let stale = stale_preparation(&module, &second, &import)?;
    let dependencies = dependency_control(&dependency)?;
    let session = ModuleSession::new(&[]).map_err(source_error)?;
    let session = register(session, &module)?;
    let bound = bound_submission(&emitter)?;
    let [owner, arm, layout, shape, sum, effect, binding] = [
        owner_candidate(&session)?,
        private_arm_match(&session)?,
        same_layout(&session)?,
        forged_shape(&session)?,
        structural_sum(&session)?,
        wrong_effect(&bound)?,
        forged_binding(&bound)?,
    ];
    let (session, stale_identity) = stale_module_identity(session, &second)?;
    let candidates = [owner, arm, layout, shape, sum, effect, binding,
        stale_identity, ambient_bypass(&emitter, &ambient_literal)?];
    let resource_operations = [
        resource_eligibility(&session, "dup", &resource_dup)?,
        resource_eligibility(&session, "drop", &resource_drop)?,
        resource_eligibility(&session, "capture", &resource_capture)?,
    ];
    let requested = workload["candidates"].as_array().context("missing hostile candidate list")?;
    ensure!(candidates.len() == requested.len(), "gate omitted hostile kernel controls");
    for (actual, expected) in candidates.iter().zip(requested) {
        ensure!(actual["name"] == *expected, "hostile kernel control order/name differs");
    }
    Ok(json!({ "schema": "noble-declared-modules-kernel-peer/v1", "outcome": "passed",
        "workload_sha256": hash(&workload_bytes),
        "source_sha256": {"module": hash(&module), "second": hash(&second),
            "import": hash(&import), "emitter": hash(&emitter), "binding": hash(&host),
            "resource_dup": hash(&resource_dup), "resource_drop": hash(&resource_drop),
            "resource_capture": hash(&resource_capture), "ambient_literal": hash(&ambient_literal),
            "dependency": hash(&dependency)},
        "stale": stale, "dependencies": dependencies, "candidates": candidates,
        "resource_operations": resource_operations }))
}
fn main() {
    let result = (|| -> Result<Value> {
        let args: Vec<_> = env::args_os().collect();
        ensure!(args.len() == 2, "usage: PEER KERNEL_PEER_WORKLOAD_JSON");
        run(Path::new(&args[1]))
    })();
    match result {
        Ok(report) => println!("{}", report),
        Err(problem) => {
            eprintln!("noble-declared-modules-peer: {problem:#}");
            std::process::exit(1);
        }
    }
}
