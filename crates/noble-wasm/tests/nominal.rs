#![allow(
    tigerstyle::excessive_file_length,
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
#![feature(register_tool)]
#![register_tool(tigerstyle)]

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn wasm_diagnostic(error: noble_wasm::Diagnostic) -> &'static str {
    match error {
        noble_wasm::Diagnostic::Invalid => "invalid",
        noble_wasm::Diagnostic::Exhausted => "exhausted",
        noble_wasm::Diagnostic::Unsupported => "unsupported",
        noble_wasm::Diagnostic::Defective => "defective",
    }
}

fn module_submission(
    module: &[u8],
    expression: &[u8],
    bindings: &[noble_contracts::source::BoundOperation],
) -> Result<noble_kernel::execution::Submission, String> {
    let mut session = noble_contracts::source::ModuleSession::new(
        bindings,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    for source in [module, b"import ledger@1 as account".as_slice()] {
        let prepared = session
            .prepare(source, &[], LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        let (next, outcome) = session.commit(prepared);
        outcome.map_err(|error| error.diagnostic().message.clone())?;
        session = next;
    }
    session
        .prepare(expression, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?
        .submission()
        .cloned()
        .ok_or_else(|| "missing executable source submission".into())
}

fn source_submission() -> Result<noble_kernel::execution::Submission, String> {
    module_submission(
        b"module ledger@1 [ opaque UserId I64 public variant Status Ready I64 public Failed Text public require emit Text -- ! test.emit export UserId export Status export handle def handle [ 7 UserId.new UserId.into drop \"hello\" emit ] ]",
        b"account.handle",
        &[noble_contracts::source::BoundOperation {
            module_name: "ledger".into(),
            module_version: 1,
            operation: "test.emit".into(),
            adapter_identity: "version-A".into(),
            adapter_slot: 7,
            input: vec![noble_kernel::types::Ty::Text],
            output: vec![],
            effects: vec![noble_kernel::types::EffId(0)],
        }],
    )
}

fn result_session() -> Result<noble_contracts::source::ModuleSession, String> {
    let session = noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let module = session
        .prepare(noble_contracts::source::RESULT_LIBRARY_SOURCE, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let (session, outcome) = session.commit(module);
    outcome.map_err(|error| error.diagnostic().message.clone())?;
    Ok(session)
}

fn result_submission() -> Result<noble_kernel::execution::Submission, String> {
    let session = result_session()?;
    let ty = session
        .resolve_type("result@1.Result<I64,Text>")
        .map_err(|error| error.diagnostic().message.clone())?;
    let imported = session
        .prepare(b"import result@1 as choice", &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let (session, outcome) = session.commit(imported);
    outcome.map_err(|error| error.diagnostic().message.clone())?;
    let constructor = session
        .prepare(
            b"module result_cases@1 [ signature make_ok forall<S:stack> [ S -- S result@1.Result<I64,Text> ! pure ] export make_ok def make_ok [ 2 result@1.Result.Ok ] ]",
            &[],
            LIMITS,
        )
        .map_err(|error| error.diagnostic().message.clone())?;
    let (session, outcome) = session.commit(constructor);
    outcome.map_err(|error| error.diagnostic().message.clone())?;
    let prepared = session
        .prepare(b"result_cases@1.make_ok [ 1 + ] choice.map_ok", &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    if prepared.output() != core::slice::from_ref(&ty) {
        return Err("generic Result compiled to a different ordered instance".into());
    }
    prepared
        .submission()
        .cloned()
        .ok_or_else(|| "missing generic Result submission".into())
}

#[test]
fn forged_generic_result_instances_are_rejected() -> Result<(), String> {
    let submission = result_submission()?;
    noble_wasm::source::Compiler::new()
        .prepare(&submission)
        .map_err(|error| {
            format!(
                "generic Result refused by Wasm compiler: {}",
                wasm_diagnostic(error)
            )
        })?;

    let mut wrong_order = submission.clone();
    wrong_order.environment.generic_variants[0]
        .payload_params
        .swap(0, 1);
    assert!(
        noble_wasm::source::Compiler::new()
            .prepare(&wrong_order)
            .err()
            == Some(noble_wasm::Diagnostic::Invalid),
        "a declaration with reversed generic payload arguments was accepted"
    );
    let mut wrong_shape = submission;
    let noble_kernel::types::Ty::GenericNominal(_, _, shape) =
        &mut wrong_shape.request.expected.stack_out[0]
    else {
        return Err("generic output lost its concrete family instance".into());
    };
    **shape = noble_kernel::types::NominalShape::Variant(
        Box::new(noble_kernel::types::Ty::Text),
        Box::new(noble_kernel::types::Ty::I64),
    );
    assert!(
        noble_wasm::source::Compiler::new()
            .prepare(&wrong_shape)
            .err()
            == Some(noble_wasm::Diagnostic::Invalid),
        "a mismatched concrete generic representation was accepted"
    );
    Ok(())
}

#[test]
fn admitted_nominal_compiles_but_forged_bindings_and_shapes_do_not() -> Result<(), String> {
    let submission = source_submission()?;
    noble_wasm::source::Compiler::new()
        .prepare(&submission)
        .map_err(|error| {
            format!(
                "valid source refused by compiler: {}",
                wasm_diagnostic(error)
            )
        })?;

    let mut changed_slot = submission.clone();
    let adapter = changed_slot
        .environment
        .bound_adapters
        .first_mut()
        .ok_or_else(|| "bound adapter not retained".to_string())?;
    adapter.adapter_slot = adapter.adapter_slot.wrapping_add(1);
    assert!(
        noble_wasm::source::Compiler::new()
            .prepare(&changed_slot)
            .err()
            == Some(noble_wasm::Diagnostic::Invalid),
        "forged immutable dispatch slot was accepted"
    );

    let mut changed_contract = submission.clone();
    changed_contract.environment.bound_adapters[0]
        .output
        .push(noble_kernel::types::Ty::Unit);
    assert!(
        noble_wasm::source::Compiler::new()
            .prepare(&changed_contract)
            .err()
            == Some(noble_wasm::Diagnostic::Invalid),
        "forged adapter result contract was accepted"
    );

    let mut changed_shape = submission;
    let nominal = changed_shape
        .environment
        .nominals
        .first_mut()
        .ok_or_else(|| "nominal schema not retained".to_string())?;
    nominal.shape =
        noble_kernel::types::NominalShape::Opaque(Box::new(noble_kernel::types::Ty::Bool));
    assert!(
        noble_wasm::source::Compiler::new()
            .prepare(&changed_shape)
            .err()
            == Some(noble_wasm::Diagnostic::Invalid),
        "mismatched nominal schema was accepted"
    );
    Ok(())
}

fn session_with_validated_resource_schema() -> Result<noble_contracts::source::ModuleSession, String>
{
    let session = noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let module = session.prepare(
        b"module fixture@1 [ opaque Ticket Pair<I64,Resource<test.counter>> private export Ticket ]",
        &[],
        LIMITS,
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let (next, outcome) = session.commit(module);
    outcome.map_err(|error| error.diagnostic().message.clone())?;
    Ok(next)
}

#[test]
fn unused_validated_resource_schema_does_not_reject_unrelated_execution() -> Result<(), String> {
    let session = session_with_validated_resource_schema()?;
    let expression = session
        .prepare(b"1", &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let submission = expression
        .submission()
        .ok_or_else(|| "missing unrelated source submission".to_string())?;
    let compiled = noble_wasm::source::Compiler::new()
        .prepare(submission)
        .map_err(|_| "unused validated resource schema blocked unrelated Wasm".to_string())?;
    let wat = core::str::from_utf8(compiled.wat()).map_err(|_| "invalid unrelated WAT UTF-8")?;
    assert!(
        wat.contains("(call $push_i64 (i64.const 1))"),
        "unrelated execution did not retain its I64 literal"
    );
    Ok(())
}

#[test]
fn first_declared_operation_does_not_alias_core_abort() -> Result<(), String> {
    verify_core_abort()?;

    let binding = noble_contracts::source::BoundOperation {
        module_name: "ledger".into(),
        module_version: 1,
        operation: "test.emit".into(),
        adapter_identity: "version-A".into(),
        adapter_slot: 0,
        input: vec![noble_kernel::types::Ty::Text],
        output: vec![],
        effects: vec![noble_kernel::types::EffId(0)],
    };
    let bound = module_submission(
        b"module ledger@1 [ require emit Text -- ! test.emit export handle def handle [ \"bound\" emit ] ]",
        b"account.handle",
        &[binding],
    )?;
    assert_eq!(
        bound
            .environment
            .kind(noble_kernel::contracts::Definition(23)),
        Some(noble_kernel::contracts::Behavior::BoundEmit(0)),
        "first declared definition is not the bound operation"
    );
    let bound_wat = noble_wasm::source::Compiler::new()
        .prepare(&bound)
        .map_err(|_| "first bound operation refused".to_string())?;
    let bound_wat = core::str::from_utf8(bound_wat.wat()).map_err(|_| "invalid bound WAT UTF-8")?;
    assert!(
        bound_wat.contains("(call $op_emit_bound (i32.const 0))"),
        "first bound operation lost its immutable native slot"
    );
    assert!(
        !bound_wat.contains("(call $op_abort)"),
        "first bound operation was mislowered as Core abort"
    );
    assert!(
        !bound_wat.contains("(import \"noble\" \"test_emit\""),
        "declared operation gained an ambient host import"
    );

    let nominal = module_submission(
        b"module ledger@1 [ opaque UserId I64 private export handle def handle [ 7 UserId.new UserId.into ] ]",
        b"account.handle",
        &[],
    )?;
    assert!(
        matches!(
            nominal
                .environment
                .kind(noble_kernel::contracts::Definition(23)),
            Some(noble_kernel::contracts::Behavior::NominalNew(_))
        ),
        "first declared definition is not the nominal constructor"
    );
    let nominal_wat = noble_wasm::source::Compiler::new()
        .prepare(&nominal)
        .map_err(|_| "first nominal constructor refused".to_string())?;
    let nominal_wat =
        core::str::from_utf8(nominal_wat.wat()).map_err(|_| "invalid nominal WAT UTF-8")?;
    assert!(
        nominal_wat.contains("(call $op_nominal_construct"),
        "first nominal constructor lost its nominal lowering"
    );
    assert!(
        !nominal_wat.contains("(call $op_abort)"),
        "first nominal constructor was mislowered as Core abort"
    );
    Ok(())
}

#[test]
fn bound_code_export_compiles_real_deny_only_slot_import() -> Result<(), String> {
    use noble_kernel::contracts::TEST_CODE_EXPORT;
    use noble_kernel::types::{EffSet, Ty};
    let binding = noble_contracts::source::BoundOperation {
        module_name: "ledger".into(),
        module_version: 1,
        operation: "test.code-export".into(),
        adapter_identity: "worker-test-code-export@1.0.0".into(),
        adapter_slot: 9,
        input: vec![Ty::Text, Ty::Text],
        output: vec![],
        effects: vec![TEST_CODE_EXPORT],
    };
    let submission = module_submission(
        b"module ledger@1 [ require code_export Text Text -- ! test.code-export export capture def capture [ \"private-capture\" \"local-destination\" code_export ] ]",
        b"account.capture", &[binding],
    )?;
    assert_eq!(
        submission
            .environment
            .kind(noble_kernel::contracts::Definition(23)),
        Some(noble_kernel::contracts::Behavior::BoundCodeExport(9))
    );
    assert_eq!(
        submission.request.expected.allowed_effects.as_slice(),
        [TEST_CODE_EXPORT]
    );
    let compiled = noble_wasm::source::Compiler::new_worker()
        .prepare(&submission)
        .map_err(|error| format!("code-export source rejected: {}", wasm_diagnostic(error)))?;
    let wat = core::str::from_utf8(compiled.wat()).map_err(|_| "code-export WAT is not UTF-8")?;
    assert!(wat.contains("(import \"noble\" \"test_code_export_bound\" (func $host_code_export_bound (param i32) (result i32)))"));
    assert!(wat.contains("(call $op_code_export_bound (i32.const 9))"));
    assert!(!wat.contains("(import \"noble\" \"test_emit_bound\""));
    let mut forged = submission;
    forged.environment.bound_adapters[0].effects = EffSet::empty();
    assert!(matches!(
        noble_wasm::source::Compiler::new_worker().prepare(&forged),
        Err(noble_wasm::Diagnostic::Invalid)
    ));
    Ok(())
}

/// What a hostile or mistaken host does with the deny-only export import.
#[derive(Clone, Copy)]
enum ExportHost {
    ReportsSuccess,
    Traps,
}

fn wasm_result<T>(result: wasmtime::Result<T>) -> Result<T, String> {
    result.map_err(|error| error.to_string())
}

/// Compiles the WORKER-12 `ledger@1.capture` fixture to a Wasm binary.
fn code_export_module() -> Result<Vec<u8>, String> {
    let binding = noble_contracts::source::BoundOperation {
        module_name: "ledger".into(),
        module_version: 1,
        operation: "test.code-export".into(),
        adapter_identity: "worker-test-code-export@1.0.0".into(),
        adapter_slot: 9,
        input: vec![noble_kernel::types::Ty::Text, noble_kernel::types::Ty::Text],
        output: vec![],
        effects: vec![noble_kernel::contracts::TEST_CODE_EXPORT],
    };
    let submission = module_submission(
        b"module ledger@1 [ require code_export Text Text -- ! test.code-export export capture def capture [ \"private-capture\" \"local-destination\" code_export ] ]",
        b"account.capture",
        &[binding],
    )?;
    let compiled = noble_wasm::source::Compiler::new_worker()
        .prepare(&submission)
        .map_err(|error| format!("code-export source rejected: {}", wasm_diagnostic(error)))?;
    wat::parse_bytes(compiled.wat())
        .map(std::borrow::Cow::into_owned)
        .map_err(|error| error.to_string())
}

/// Links the fixture's runtime imports. Only the export import misbehaves,
/// and it records every slot it receives.
fn link_code_export_imports(
    module: &wasmtime::Module,
    store: &mut wasmtime::Store<Vec<i32>>,
    host: ExportHost,
) -> Result<wasmtime::Linker<Vec<i32>>, String> {
    use wasmtime::{Caller, ExternType, Global, Linker, Memory, Ref, Table, Val, ValType};
    let mut linker = Linker::new(store.engine());
    let imports: Vec<(String, ExternType)> = module
        .imports()
        .map(|import| (import.name().to_string(), import.ty()))
        .collect();
    for (name, ty) in imports {
        match ty {
            ExternType::Memory(ty) => {
                let memory = wasm_result(Memory::new(&mut *store, ty))?;
                wasm_result(linker.define(&*store, "noble", &name, memory))?;
            }
            ExternType::Table(ty) => {
                let table = wasm_result(Table::new(&mut *store, ty, Ref::Func(None)))?;
                wasm_result(linker.define(&*store, "noble", &name, table))?;
            }
            ExternType::Global(ty) => {
                let value = match ty.content() {
                    ValType::I32 => Val::I32(0),
                    ValType::I64 => Val::I64(0),
                    ValType::F32 | ValType::F64 | ValType::V128 | ValType::Ref(_) => {
                        return Err(format!("unexpected global import {name}"))
                    }
                };
                let global = wasm_result(Global::new(&mut *store, ty, value))?;
                wasm_result(linker.define(&*store, "noble", &name, global))?;
            }
            ExternType::Func(ty) if name == "test_code_export_bound" => {
                // Only the adapter slot crosses: no capture or destination
                // pointer, length or bytes can reach the host.
                let params: Vec<ValType> = ty.params().collect();
                let results: Vec<ValType> = ty.results().collect();
                if !matches!(params.as_slice(), [ValType::I32])
                    || !matches!(results.as_slice(), [ValType::I32])
                {
                    return Err("code-export import is not (i32) -> i32".into());
                }
                wasm_result(linker.func_wrap(
                    "noble",
                    "test_code_export_bound",
                    move |mut caller: Caller<'_, Vec<i32>>, slot: i32| -> wasmtime::Result<i32> {
                        caller.data_mut().push(slot);
                        if matches!(host, ExportHost::Traps) {
                            Err(wasmtime::Error::msg("host export threw"))
                        } else {
                            Ok(0)
                        }
                    },
                ))?;
            }
            ExternType::Func(_) | ExternType::Tag(_) => {
                return Err(format!("unexpected import {name}"));
            }
        }
    }
    Ok(linker)
}

/// Runs the compiled WORKER-12 fixture in a real Wasmtime instance whose
/// export import misbehaves, returning whether the invocation completed, the
/// `$failure` metric afterwards and every slot the host saw.
fn run_code_export_guest(host: ExportHost) -> Result<(bool, i64, Vec<i32>), String> {
    let engine = wasm_result(wasmtime::Engine::new(&wasmtime::Config::new()))?;
    let module = wasm_result(wasmtime::Module::new(&engine, code_export_module()?))?;
    let mut store = wasmtime::Store::new(&engine, Vec::<i32>::new());
    let linker = link_code_export_imports(&module, &mut store, host)?;
    let instance = wasm_result(linker.instantiate(&mut store, &module))?;
    let begin = wasm_result(
        instance.get_typed_func::<(i32, i32, i32, i32, i32, i32), i32>(&mut store, "begin"),
    )?;
    let budget = (32_768, 256, 32, 2048, 4096, 4096);
    assert_eq!(wasm_result(begin.call(&mut store, budget))?, 0);
    let install = wasm_result(instance.get_typed_func::<(), i32>(&mut store, "worker_install"))?;
    assert_eq!(wasm_result(install.call(&mut store, ()))?, 0);
    let named =
        wasm_result(instance.get_typed_func::<i32, i32>(&mut store, "worker_named_handle"))?;
    let handle = wasm_result(named.call(&mut store, 0))?;
    assert!(handle > 0, "installed capture Program has no handle");
    assert_eq!(wasm_result(begin.call(&mut store, budget))?, 0);
    let invoke =
        wasm_result(instance.get_typed_func::<(i32, i64), i64>(&mut store, "worker_invoke"))?;
    let is_completed = invoke.call(&mut store, (handle, 41)).is_ok();
    let metric = wasm_result(instance.get_typed_func::<i32, i64>(&mut store, "metric"))?;
    let failure = wasm_result(metric.call(&mut store, 0))?;
    Ok((is_completed, failure, store.data().clone()))
}

#[test]
fn bound_code_export_stays_failed_when_host_reports_success() -> Result<(), String> {
    let (is_completed, failure, slots) = run_code_export_guest(ExportHost::ReportsSuccess)?;
    assert_eq!(slots, [9], "expected exactly one slot-only export request");
    // The guest returned normally, yet the poisoned invocation stays failed:
    // a zero status from the host cannot clear the terminal failure.
    assert!(
        is_completed,
        "a status-0 host import should not trap the guest"
    );
    assert_eq!(failure, 5, "host success cleared the code-export failure");

    let (is_completed, failure, slots) = run_code_export_guest(ExportHost::Traps)?;
    assert_eq!(slots, [9], "expected exactly one slot-only export request");
    assert!(
        !is_completed,
        "a throwing host import must not complete the invocation"
    );
    assert_eq!(failure, 5, "a throwing host left the invocation unpoisoned");
    Ok(())
}

fn verify_core_abort() -> Result<(), String> {
    let core = noble_contracts::source::Session::new()
        .prepare(b"test.abort", &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let core = core
        .submission()
        .ok_or_else(|| "missing Core abort submission".to_string())?;
    let core_wat = noble_wasm::source::Compiler::new()
        .prepare(core)
        .map_err(|_| "historical optional abort refused".to_string())?;
    let core_wat = core::str::from_utf8(core_wat.wat()).map_err(|_| "invalid Core WAT UTF-8")?;
    if !core_wat.contains("(call $op_abort)") || core_wat.contains("\"test_emit_bound\"") {
        return Err("Core def23 lost its historical abort lowering".into());
    }
    Ok(())
}

#[test]
fn omitted_first_declared_dependency_is_rejected() -> Result<(), String> {
    let mut submission = module_submission(
        b"module ledger@1 [ require emit Text -- ! test.emit export handle def handle [ \"bound\" emit ] ]",
        b"account.handle",
        &[noble_contracts::source::BoundOperation {
            module_name: "ledger".into(), module_version: 1,
            operation: "test.emit".into(), adapter_identity: "version-A".into(),
            adapter_slot: 0, input: vec![noble_kernel::types::Ty::Text], output: vec![],
            effects: vec![noble_kernel::types::EffId(0)],
        }],
    )?;
    let named = usize::try_from(submission.definitions[0].definition.0)
        .map_err(|_| "unrepresentable named definition")?;
    assert!(
        submission.environment.deps[named].contains(&noble_kernel::contracts::Definition(23)),
        "frontend omitted the resolved bound dependency"
    );
    submission.environment.deps[named]
        .retain(|def| *def != noble_kernel::contracts::Definition(23));
    assert!(
        noble_wasm::source::Compiler::new()
            .prepare(&submission)
            .err()
            == Some(noble_wasm::Diagnostic::Invalid),
        "backend accepted a missing exact def23 dependency"
    );
    Ok(())
}
