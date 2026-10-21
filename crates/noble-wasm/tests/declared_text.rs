#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::assertion_density,
    tigerstyle::compound_condition,
    tigerstyle::excessive_file_length,
    tigerstyle::function_length,
    tigerstyle::no_unwrap,
    tigerstyle::non_trait_imports,
    tigerstyle::raw_arithmetic_overflow,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]

use noble_contracts::source::{ModuleSession, Stage};
use noble_kernel::contracts::Behavior;
use noble_wasm::source::Compiler;

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn linked(session: ModuleSession, source: &[u8]) -> Result<ModuleSession, String> {
    let staged = session
        .prepare(source, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let (next, outcome) = session.commit(staged);
    outcome.map_err(|error| error.diagnostic().message.clone())?;
    Ok(next)
}

#[test]
fn source_epoch_reuses_cleared_code_without_rewinding_generation() -> Result<(), String> {
    let session = ModuleSession::new_declared_text(
        &[],
        true,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .ok_or("invalid retained source cap")?,
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let session = linked(
        session,
        include_bytes!("../../noble-contracts/src/fixtures/result.noble"),
    )?;
    let expression = session
        .prepare(b"1 drop", &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let owner = expression
        .declared_text_source_owner()
        .ok_or("missing signed source owner")?;
    let mut compiler = Compiler::new_declared_text();
    let prepared = compiler
        .prepare_declared_text(&owner)
        .map_err(|_| "initial signed source compilation refused")?;
    let span = prepared.code_span();
    let end = span.start + span.slots;
    compiler
        .commit(prepared)
        .map_err(|_| "initial commit refused")?;
    if compiler
        .retire_declared_text_epoch(span.generation + 1, end)
        .is_ok()
        || compiler
            .retire_declared_text_epoch(span.generation, end + 1)
            .is_ok()
        || compiler.declared_text_epoch_state() != Some((span.generation, end))
    {
        return Err("stale or partial worker receipt changed checked compiler state".into());
    }
    compiler
        .retire_declared_text_epoch(span.generation, end)
        .map_err(|_| "checked epoch reset refused")?;
    if compiler.declared_text_epoch_state() != Some((span.generation, 4))
        || compiler
            .retire_declared_text_epoch(span.generation, end)
            .is_ok()
    {
        return Err("source epoch repeated retirement or rewound the compiler".into());
    }
    let next = compiler
        .prepare_declared_text(&owner)
        .map_err(|_| "rechecked source after epoch refused")?;
    if next.code_span().start != 4
        || next.code_span().generation != span.generation + 1
        || next
            .wat()
            .windows(b"table.init $core".len())
            .any(|window| window == b"table.init $core")
    {
        return Err("cleared code did not reuse table4 with monotone source generation".into());
    }
    Ok(())
}

#[test]
fn source_bound_signed_fold_and_nominal_result_require_selected_compiler() -> Result<(), String> {
    let session = ModuleSession::new_declared_text(
        &[],
        true,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let session = linked(
        session,
        include_bytes!("../../noble-contracts/src/fixtures/result.noble"),
    )?;
    let session = linked(
        session,
        include_bytes!("../../../verification/calculator-precursor/decimal-digit.noble"),
    )?;
    let session = linked(
        session,
        include_bytes!("../../../verification/calculator-precursor/decimal-fold.noble"),
    )?;
    let prepared = session
        .prepare(b"\"12345\" 0 0 decimal_fold@1.fold", &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let owner = prepared
        .declared_text_source_owner()
        .ok_or("missing calculator source token")?;
    let submission = owner
        .rederive()
        .map_err(|error| error.diagnostic().message.clone())?;
    if !submission.environment.declared_modules
        || !submission.environment.text_cursor
        || submission
            .environment
            .kind(noble_kernel::contracts::Definition(26))
            != Some(Behavior::TextByte)
        || submission
            .environment
            .kind(noble_kernel::contracts::Definition(27))
            != Some(Behavior::I64Less)
        || !submission
            .request
            .expected
            .allowed_effects
            .as_slice()
            .is_empty()
    {
        return Err("combined declared-text slots or purity were not source checked".into());
    }
    let compiler = Compiler::new_declared_text();
    compiler.prepare_declared_text(&owner).map_err(|error| {
        let reason = match error {
            noble_wasm::Diagnostic::Invalid => "invalid",
            noble_wasm::Diagnostic::Exhausted => "exhausted",
            noble_wasm::Diagnostic::Unsupported => "unsupported",
            noble_wasm::Diagnostic::Defective => "defective",
        };
        format!("declared-text compiler refused signed fold: {reason}")
    })?;
    if compiler.prepare(&submission).is_ok()
        || Compiler::new().prepare(&submission).is_ok()
        || Compiler::new_text_cursor().prepare(&submission).is_ok()
    {
        return Err("public raw Wasm path accepted merged profile".into());
    }
    Ok(())
}

#[test]
fn signed_i64_order_is_pure_and_kept_out_of_ordinary_declared_modules() -> Result<(), String> {
    let source = b"-1 0 <";
    for session in [
        ModuleSession::new(
            &[],
            noble_contracts::source::RetainedSourceLimit::new(65_536)
                .expect("valid legacy retained source cap"),
        ),
        ModuleSession::new_signed_self(
            &[],
            noble_contracts::source::RetainedSourceLimit::new(65_536)
                .expect("valid legacy retained source cap"),
        ),
    ] {
        let session = session.map_err(|error| error.diagnostic().message.clone())?;
        let error = session
            .prepare(source, &[], LIMITS)
            .err()
            .ok_or("ordinary declared module acquired signed comparison")?;
        if error.stage() != noble_contracts::source::Stage::Resolve {
            return Err(format!(
                "ordinary comparison failed at {:?} instead of resolve",
                error.stage()
            ));
        }
    }
    let selected = ModuleSession::new_declared_text(
        &[],
        false,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let prepared = selected
        .prepare(source, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let owner = prepared
        .declared_text_source_owner()
        .ok_or("missing checked source owner")?;
    let submission = owner
        .rederive()
        .map_err(|error| error.diagnostic().message.clone())?;
    if submission
        .environment
        .kind(noble_kernel::contracts::Definition(27))
        != Some(Behavior::I64Less)
        || !submission
            .request
            .expected
            .allowed_effects
            .as_slice()
            .is_empty()
    {
        return Err("selected comparison lost its fixed pure identity".into());
    }
    let module = Compiler::new_declared_text()
        .prepare_declared_text(&owner)
        .map_err(|_| "checked signed comparison failed Wasm compilation")?;
    if !module
        .wat()
        .windows(b"i64.lt_s".len())
        .any(|window| window == b"i64.lt_s")
        || Compiler::new().prepare(&submission).is_ok()
        || Compiler::new_text_cursor().prepare(&submission).is_ok()
    {
        return Err(
            "selected-only signed comparison was not isolated to its checked Wasm ABI".into(),
        );
    }
    Ok(())
}

#[test]
fn lexical_parameters_and_quotation_have_source_bound_checked_wasm() -> Result<(), String> {
    let module = include_bytes!("../../../verification/calculator-precursor/local-bindings.noble");
    let selected = ModuleSession::new_declared_text(
        &[],
        false,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let selected = linked(selected, module)?;
    for source in [
        b"5 6 local_math@1.add".as_slice(),
        b"5 6 local_math@1.stack_add".as_slice(),
        b"5 local_math@1.shadow_global".as_slice(),
        b"5 local_math@1.capture".as_slice(),
        b"5 6 local_math@1.capture_pair".as_slice(),
        b"1 2 3 local_math@1.ordered_three".as_slice(),
        b"5 local_math@1.shadow".as_slice(),
    ] {
        let prepared = selected
            .prepare(source, &[], LIMITS)
            .map_err(|error| error.diagnostic().message.clone())?;
        if prepared.output() != [noble_kernel::types::Ty::I64] {
            return Err("lexically bound module returned the wrong checked interface".into());
        }
        let owner = prepared
            .declared_text_source_owner()
            .ok_or("local module lost source ownership")?;
        Compiler::new_declared_text()
            .prepare_declared_text(&owner)
            .map_err(|_| "source-checked local module failed compiled Wasm admission")?;
    }
    let ordinary = ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let refused = ordinary
        .prepare(module, &[], LIMITS)
        .err()
        .ok_or("ordinary declared modules acquired selected local syntax")?;
    if refused.stage() != Stage::Resolve {
        return Err("ordinary lexical syntax failed after lexical resolution".into());
    }
    Ok(())
}

#[test]
fn local_reuse_discard_capture_and_duplicate_names_fail_closed() -> Result<(), String> {
    let selected = ModuleSession::new_declared_text(
        &[],
        false,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    for source in [
        b"let r r r".as_slice(),
        b"let r".as_slice(),
        b"let r [ r ]".as_slice(),
    ] {
        let resource = noble_kernel::types::Ty::Resource(noble_kernel::contracts::FIXTURE_RESOURCE);
        let denied = selected
            .prepare(source, &[resource], LIMITS)
            .err()
            .ok_or("linear resource escaped lexical Data/Capture eligibility")?;
        if denied.stage() != Stage::Check && denied.stage() != Stage::Acceptance {
            return Err(format!(
                "linear local escaped checked static admission until {:?}: {}",
                denied.stage(),
                denied.diagnostic().message
            ));
        }
    }
    for source in [b"1 let x 2 let x x".as_slice(), b"1 let".as_slice()] {
        let denied = selected
            .prepare(source, &[], LIMITS)
            .err()
            .ok_or("duplicate or malformed local binding was accepted")?;
        if denied.stage() != Stage::Resolve {
            return Err(format!(
                "malformed lexical binder failed at {:?}",
                denied.stage()
            ));
        }
    }
    Ok(())
}

#[test]
fn ordinary_declared_module_does_not_acquire_byte_cursor() -> Result<(), String> {
    let source = b"module cursor@1 [ signature read forall<S:stack> [ S Text I64 -- S Text I64 ! pure ] export read def read [ text.byte ] ]";
    for session in [
        ModuleSession::new(
            &[],
            noble_contracts::source::RetainedSourceLimit::new(65_536)
                .expect("valid legacy retained source cap"),
        ),
        ModuleSession::new_signed_self(
            &[],
            noble_contracts::source::RetainedSourceLimit::new(65_536)
                .expect("valid legacy retained source cap"),
        ),
    ] {
        let session = session.map_err(|error| error.diagnostic().message.clone())?;
        if session.prepare(source, &[], LIMITS).is_ok() {
            return Err("ordinary declared profile admitted text.byte".into());
        }
    }
    let session = ModuleSession::new_declared_text(
        &[],
        true,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    linked(session, source)?;
    Ok(())
}

#[test]
fn nominal_signature_names_bind_only_checked_local_or_exact_exported_owner() -> Result<(), String> {
    let session = ModuleSession::new_declared_text(
        &[],
        true,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let session = linked(
        session,
        include_bytes!("../../noble-contracts/src/fixtures/result.noble"),
    )?;
    let forward = b"module forward@1 [ signature read forall<S:stack> [ S result@1.Result<I64,MissingError> -- S result@1.Result<I64,MissingError> ! pure ] def read [ ] ]";
    let error = session
        .prepare(forward, &[], LIMITS)
        .err()
        .ok_or("undeclared forward nominal was accepted")?;
    if error.diagnostic().message != "unknown signature value type" {
        return Err("forward nominal refused for the wrong reason".into());
    }
    let session = linked(
        session,
        include_bytes!("../../../verification/calculator-precursor/decimal-digit.noble"),
    )?;
    let session = linked(
        session,
        include_bytes!("../../../verification/calculator-precursor/decimal-fold.noble"),
    )?;
    let same_owner = b"module take@1 [ signature identity forall<S:stack> [ S result@1.Result<I64,decimal_fold@1.FoldError> -- S result@1.Result<I64,decimal_fold@1.FoldError> ! pure ] export identity def identity [ ] ]";
    let session = linked(session, same_owner)?;
    for (name, source) in [
        ("wrong version", &b"module bad@1 [ signature read forall<S:stack> [ S result@1.Result<I64,decimal_fold@2.FoldError> -- S result@1.Result<I64,decimal_fold@2.FoldError> ! pure ] def read [ ] ]"[..]),
        ("wrong owner", &b"module bad@1 [ signature read forall<S:stack> [ S result@1.Result<I64,other@1.FoldError> -- S result@1.Result<I64,other@1.FoldError> ! pure ] def read [ ] ]"[..]),
        ("missing local", &b"module bad@1 [ signature read forall<S:stack> [ S result@1.Result<I64,FoldError> -- S result@1.Result<I64,FoldError> ! pure ] def read [ ] ]"[..]),
    ] {
        let error = session.prepare(source, &[], LIMITS).err()
            .ok_or_else(|| format!("{name} nominal signature was accepted"))?;
        if error.diagnostic().message != "unknown signature value type" {
            return Err(format!("{name} nominal refused for the wrong reason"));
        }
    }
    Ok(())
}

#[test]
fn signed_fold_keeps_exact_named_digit_owner_across_later_version() -> Result<(), String> {
    let digit = include_bytes!("../../../verification/calculator-precursor/decimal-digit.noble");
    let fold = include_bytes!("../../../verification/calculator-precursor/decimal-fold.noble");
    let session = ModuleSession::new_declared_text(
        &[],
        true,
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| error.diagnostic().message.clone())?;
    let session = linked(
        session,
        include_bytes!("../../noble-contracts/src/fixtures/result.noble"),
    )?;
    let session = linked(session, digit)?;
    let wrong = String::from_utf8(fold.to_vec())
        .map_err(|error| error.to_string())?
        .replace("decimal_digit@1.digit", "decimal_digit@2.digit");
    let refused = session
        .prepare(wrong.as_bytes(), &[], LIMITS)
        .err()
        .ok_or("fold admitted a missing versioned digit owner")?;
    if refused.stage() != noble_contracts::source::Stage::Resolve {
        return Err(format!(
            "missing digit owner rejected at wrong stage: {:?}",
            refused.stage()
        ));
    }
    let session = linked(session, fold)?;
    let duplicate = session
        .prepare(digit, &[], LIMITS)
        .err()
        .ok_or("duplicate digit@1 publication replaced the first owner")?;
    if duplicate.stage() != noble_contracts::source::Stage::Resolve {
        return Err(format!(
            "duplicate digit owner rejected at wrong stage: {:?}",
            duplicate.stage()
        ));
    }
    let expression = b"\"123\" 0 0 decimal_fold@1.fold";
    let old = session
        .prepare(expression, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let old_owner = old
        .declared_text_source_owner()
        .ok_or("old source lost source token")?;
    let old_wat = Compiler::new_declared_text()
        .prepare_declared_text(&old_owner)
        .map_err(|_| "old signed fold failed to compile")?
        .wat()
        .to_vec();
    let second = b"module decimal_digit@2 [ signature digit forall<S:stack> [ S I64 -- S I64 Bool ! pure ] export digit def digit [ drop 0 false ] ]";
    let session = linked(session, second)?;
    let after = session
        .prepare(expression, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let after_owner = after
        .declared_text_source_owner()
        .ok_or("new source lost source token")?;
    let after_wat = Compiler::new_declared_text()
        .prepare_declared_text(&after_owner)
        .map_err(|_| "later digit@2 altered the old signed fold")?
        .wat()
        .to_vec();
    if old_wat != after_wat {
        return Err("later digit@2 rebound the saved fold or changed its compiled program".into());
    }
    Ok(())
}
