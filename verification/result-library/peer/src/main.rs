//! Independent source-derived Result family and hostile Env checks. No guest interpreter.
use anyhow::{bail, ensure, Context, Result};
use noble_contracts::{source::{BoundOperation, ModuleKind, ModuleSession}, Limits};
use noble_kernel::{
    acceptance,
    contracts::{self, Behavior, Env},
    execution::Submission,
    types::{EffSet, NominalShape, Ty},
    untrusted::{Constraint, Outcome},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

fn source_error(problem: noble_contracts::source::Error) -> anyhow::Error {
    anyhow::anyhow!("production source {:?}: {}", problem.stage(), problem.diagnostic().message)
}

fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

fn read_input(document: &Value, field: &str) -> Result<(Vec<u8>, Value)> {
    let file = document.get(field).and_then(Value::as_str)
        .with_context(|| format!("missing exact source input {field}"))?;
    let bytes = fs::read(file).with_context(|| format!("reading {field}: {file}"))?;
    let identity = json!({ "file": file, "sha256": hash(&bytes), "bytes": bytes.len() });
    Ok((bytes, identity))
}

fn host_operation(workload: &Value) -> Result<(BoundOperation, Value)> {
    let host = &workload["host_binding"];
    let name = host["module"].as_str().context("missing bound source module name")?;
    let version = u32::try_from(host["version"].as_u64().context("missing bound module version")?)
        .context("bound module version exceeds u32")?;
    let identity = host["adapter_identity"].as_str().context("missing bound adapter identity")?;
    let (bytes, source) = read_input(workload, "host_binding_file")?;
    ensure!(bytes.as_slice() == format!(
        "bind {name}@{version} test.emit {identity} Text -- ! test.emit allow\n"
    ).as_bytes(), "independent peer binding disagrees with exact authorized host source");
    Ok((BoundOperation {
        module_name: name.into(), module_version: version,
        operation: "test.emit".into(), adapter_identity: identity.into(), adapter_slot: 0,
        input: vec![Ty::Text], output: vec![], effects: vec![contracts::TEST_EMIT],
    }, source))
}

fn transition(session: ModuleSession, source: &[u8], kind: ModuleKind) -> Result<ModuleSession> {
    let prepared = session.prepare(source, &[], Limits::default()).map_err(source_error)?;
    ensure!(prepared.kind() == kind, "source did not produce required module/import transition");
    let (session, committed) = session.commit(prepared);
    committed.map_err(source_error)?;
    Ok(session)
}

fn original_submission(session: &ModuleSession, source: &[u8]) -> Result<Submission> {
    let prepared = session.prepare(source, &[], Limits::default()).map_err(source_error)?;
    ensure!(prepared.kind() == ModuleKind::Expression,
        "baseline candidate was not a real source expression");
    prepared.submission().cloned().context("baseline expression did not produce a submission")
}

fn rejected(label: &str, forged: &mut Submission, environment: Env) -> Result<Value> {
    forged.environment = environment;
    let reason = match acceptance::check(
        &forged.environment, &forged.request, &forged.body.candidate,
    ) {
        Outcome::Invalid(diagnostic) => diagnostic.constraint,
        other => bail!("{label}: hostile environment not typed-invalid: {other:?}"),
    };
    ensure!(reason == Constraint::InvalidContract,
        "{label}: expected malformed Env contract, got {reason:?}");
    ensure!(matches!(noble_wasm::source::Compiler::new().prepare(&forged),
        Err(noble_wasm::Diagnostic::Invalid)),
        "{label}: independent Wasm admission accepted or misclassified forged Env");
    Ok(json!({ "name": label, "mutated_environment": true,
        "baseline_kernel": "accepted", "hostile_kernel": "invalid",
        "constraint_kind": "InvalidContract", "backend_prepare": "invalid" }))
}

fn run(workload_path: &Path) -> Result<Value> {
    let workload_bytes = fs::read(workload_path).context("reading exact gate workload")?;
    let workload: Value = serde_json::from_slice(&workload_bytes)?;
    ensure!(workload["schema"] == "noble-result-library-kernel-workload/v1",
        "wrong independent peer workload schema");
    let modules = workload["modules"].as_array().context("missing real source module list")?;
    ensure!(modules.len() >= 3, "library, bound trace and concrete helper modules required");
    let imports = workload["imports"].as_array().context("missing real import source list")?;
    let (operation, binding_source) = host_operation(&workload)?;
    let mut session = ModuleSession::new(&[operation]).map_err(source_error)?;
    let mut source_inputs = Vec::new();
    for file in modules {
        let file = file.as_str().context("module path must be a string")?;
        let bytes = fs::read(file).with_context(|| format!("reading source module {file}"))?;
        session = transition(session, &bytes, ModuleKind::Module)?;
        source_inputs.push(json!({ "file": file, "sha256": hash(&bytes), "bytes": bytes.len(),
            "kind": "module" }));
    }
    for file in imports {
        let file = file.as_str().context("import path must be a string")?;
        let bytes = fs::read(file).with_context(|| format!("reading source import {file}"))?;
        session = transition(session, &bytes, ModuleKind::Import)?;
        source_inputs.push(json!({ "file": file, "sha256": hash(&bytes), "bytes": bytes.len(),
            "kind": "import" }));
    }
    let (expression, expression_identity) = read_input(&workload, "positive_source")?;
    let first = session.resolve_type("result@1.Result<I64,Text>").map_err(source_error)?;
    let second = session.resolve_type("result@1.Result<Text,I64>").map_err(source_error)?;
    let Ty::GenericNominal(first_id, first_args, first_shape) = &first else {
        bail!("Result<I64,Text> was not a checked generic nominal instance");
    };
    let Ty::GenericNominal(second_id, second_args, second_shape) = &second else {
        bail!("Result<Text,I64> was not a checked generic nominal instance");
    };
    ensure!(first_id == second_id && first != second && first_args.as_ref() == &[Ty::I64, Ty::Text]
        && second_args.as_ref() == &[Ty::Text, Ty::I64],
        "generic Result instantiations lost type argument order or family identity");
    ensure!(matches!(first_shape.as_ref(), NominalShape::Variant(left, right)
            if left.as_ref() == &Ty::I64 && right.as_ref() == &Ty::Text)
        && matches!(second_shape.as_ref(), NominalShape::Variant(left, right)
            if left.as_ref() == &Ty::Text && right.as_ref() == &Ty::I64),
        "generic Result instance representation does not follow ordered arguments");
    let original = original_submission(&session, &expression)?;
    ensure!(original.request.expected.stack_out == [first.clone()],
        "source-derived baseline did not have the concrete Result<I64,Text> output");
    let env = &original.environment;
    let family = env.generic_variants.iter().position(|decl| decl.id == *first_id)
        .context("checked source submission omitted its Result family")?;
    let decl = &env.generic_variants[family];
    ensure!(decl.exported && decl.public == [true, true] && decl.payload_params == [0, 1]
        && env.generic_instance_matches(&first, *first_id)
        && env.generic_instance_matches(&second, *first_id),
        "source-derived public Result family descriptor is not canonical");
    let maker = env.kinds.iter().position(|kind| *kind == Behavior::GenericLeft(*first_id))
        .context("source-derived Ok constructor is missing")?;
    let matcher = env.kinds.iter().position(|kind| *kind == Behavior::GenericMatch(*first_id))
        .context("source-derived Result matcher is missing")?;
    ensure!(!env.defs[matcher].effects.is_empty(),
        "source-defined matcher erased callback effect variables");
    ensure!(env.definition_owners[matcher] == Some(first_id.module),
        "source-defined Result matcher owner is missing");
    ensure!(matches!(acceptance::check(env, &original.request, &original.body.candidate),
        Outcome::Accepted(_)), "production kernel did not accept the real baseline expression");
    ensure!(noble_wasm::source::Compiler::new().prepare(&original).is_ok(),
        "independent Wasm admission refused source-derived positive Result expression");
    let (bypass_expression, bypass_identity) = read_input(&workload, "effectful_bypass_source")?;
    let bypass = original_submission(&session, &bypass_expression)?;
    ensure!(bypass.request.expected.stack_out == [first.clone()]
        && bypass.request.expected.allowed_effects == EffSet::from_ids(&[contracts::TEST_EMIT]),
        "bypassed source callback did not retain its complete Result output and conservative effect");
    ensure!(matches!(acceptance::check(
        &bypass.environment, &bypass.request, &bypass.body.candidate,
    ), Outcome::Accepted(_)), "production kernel refused source-derived effectful bypass");
    ensure!(noble_wasm::source::Compiler::new().prepare(&bypass).is_ok(),
        "independent Wasm admission refused checked effectful bypass");
    let mut erased_request = bypass.request.clone();
    erased_request.expected.allowed_effects = EffSet::empty();
    let erased_effect = match acceptance::check(
        &bypass.environment, &erased_request, &bypass.body.candidate,
    ) {
        Outcome::Invalid(diagnostic) => diagnostic.constraint,
        other => bail!("erasing bypassed callback effect was not typed-invalid: {other:?}"),
    };
    ensure!(erased_effect == Constraint::EffectInclusion(contracts::TEST_EMIT),
        "erasing latent bypassed effect rejected for the wrong typed reason: {erased_effect:?}");
    let first_class_program = Ty::program(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    ensure!(first_class_program.is_data(), "kernel unexpectedly excludes valid Program from Data");
    let program_result = env.generic_instance(
        *first_id, [first_class_program, Ty::Text],
    ).context("source-declared generic Result excludes a valid first-class Program payload")?;
    ensure!(env.generic_instance_matches(&program_result, *first_id),
        "Program payload did not retain canonical generic Result identity");
    let resource = Ty::Resource(contracts::FIXTURE_RESOURCE);
    let resource_interface = Ty::program(
        vec![resource.clone()], vec![resource.clone()],
        EffSet::from_ids(&[contracts::TEST_EMIT]),
    );
    ensure!(resource_interface.is_data(),
        "resource-typed Program interface was mistaken for captured Resource");
    let resource_interface_result = env.generic_instance(
        *first_id, [resource_interface, Ty::Text],
    ).context("source-declared Result excludes a resource-interface Program payload")?;
    ensure!(env.generic_instance_matches(&resource_interface_result, *first_id),
        "resource-interface Program payload lost the original Result family");
    ensure!(env.generic_instance(*first_id, [resource.clone(), Ty::Text]).is_none()
        && env.generic_instance(*first_id, [Ty::I64, resource]).is_none(),
        "resource-bearing first-slice generic Result instance admitted");

    let mut reordered = env.clone();
    reordered.generic_variants[family].payload_params = [1, 0];
    let mut erased_constructor = env.clone();
    erased_constructor.defs[maker].stack_out.clear();
    let mut erased_effect = env.clone();
    erased_effect.defs[matcher].effects.clear();
    let mut foreign_owner = env.clone();
    foreign_owner.definition_owners[matcher] = None;
    let mut forged = original.clone();
    let controls = [
        rejected("reordered-generic-payloads", &mut forged, reordered)?,
        rejected("erased-Ok-constructor-contract", &mut forged, erased_constructor)?,
        rejected("erased-matcher-callback-effects", &mut forged, erased_effect)?,
        rejected("unowned-generic-matcher", &mut forged, foreign_owner)?,
    ];
    Ok(json!({ "schema": "noble-result-library-kernel-peer/v1",
        "source_inputs": source_inputs, "positive_source": expression_identity,
        "host_binding_source": binding_source, "effectful_bypass_source": bypass_identity,
        "family": { "module": first_id.module.to_string(), "ordinal": first_id.ordinal,
            "public": true, "payload_parameters": [0, 1] },
        "instances": ["Result<I64,Text>", "Result<Text,I64>"],
        "baseline": { "source_prepared": true, "production_kernel": "accepted",
            "independent_wasm_admission": "accepted", "checked_result_output": "Result<I64,Text>" },
        "conservative_effect": { "bypassed_source_static": "accepted",
            "checked_effect": "test.emit", "erased_effect": "EffectInclusion(test.emit)",
            "independent_wasm_admission": "accepted" },
        "program_payload_control": { "data": true, "generic_instance": "accepted",
            "resource_interface": "accepted" },
        "resource_argument_controls": { "success_arm": "refused", "error_arm": "refused" },
        "controls": controls }))
}

fn main() -> Result<()> {
    let args = env::args().collect::<Vec<_>>();
    ensure!(args.len() == 2,
        "usage: noble-result-library-peer FROZEN_SOURCE_WORKLOAD_JSON");
    let report = run(Path::new(&args[1]))?;
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
