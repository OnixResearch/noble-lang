#![feature(register_tool)]
#![register_tool(tigerstyle)]

use noble_kernel::contracts::environment;
use noble_kernel::types::{EffSet, Ty};

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

#[test]
fn selected_named_artifact_binds_checked_definition_not_invocation_wrapper() -> Result<(), String> {
    let env = environment().map_err(|error| format!("bootstrap: {error:?}"))?
        .enable_live_slots();
    let mut session = noble_contracts::source::Session::new_live_slots(env)
        .map_err(|error| error.diagnostic().message.clone())?;
    let definition = session.prepare(b"def increment [ 1 + ]", &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    session.commit(definition).map_err(|error| error.diagnostic().message.clone())?;
    let call = session.prepare(b"increment", &[Ty::I64], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let selected = session.checked_selected_target(&call, "increment")
        .map_err(|error| format!("source selection: {error:?}"))?;
    let compiler = noble_wasm::source::Compiler::new_live_slots();
    let checked = compiler.prepare_checked_selected(&selected)
        .map_err(|_| "selected definition failed independent Wasm admission")?;
    let binding = checked.selected_target().ok_or("selected artifact missing binding")?;
    let definition = checked.named_targets().first().ok_or("named body missing")?;
    let wrapper = checked.target_metadata().ok_or("root wrapper missing")?;
    if binding.definition_id != selected.definition_id()
        || binding.source_generation != selected.source_generation()
        || binding.named_program_index != definition.program_index
        || definition.program_index == wrapper.root_program_index
        || definition.definition_identity != selected.definition_id().identity()
        || definition.definition != selected.definition().definition
        || definition.stack_in != selected.body().interface.stack_in
        || definition.stack_out != selected.body().interface.stack_out
        || definition.effects != selected.body().interface.effects
        || definition.checked_recipe.is_empty()
    {
        return Err("selected named Program was conflated with a wrapper or forged recipe".into());
    }
    // Raw producer submissions can compile but cannot acquire a source-bound
    // selected-definition receipt by passing identity fields in JSON/Rust.
    let raw = compiler.prepare(selected.submission())
        .map_err(|_| "raw independently checked candidate failed admission")?;
    if raw.selected_target().is_some() || raw.wat() != checked.wat() {
        return Err("source selection altered code or a raw submission gained authority".into());
    }
    Ok(())
}

fn selected_builder(
    definition_source: &[u8],
    inputs: &[Ty],
) -> Result<noble_wasm::source::Prepared, String> {
    let mut source = noble_contracts::source::Session::new_live_slots(
        environment().map_err(|error| format!("{error:?}"))?.enable_live_slots(),
    ).map_err(|error| error.diagnostic().message.clone())?;
    let definition = source.prepare(definition_source, &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    source.commit(definition).map_err(|error| error.diagnostic().message.clone())?;
    let call = source.prepare(b"builder", inputs, LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let selected = source.checked_selected_target(&call, "builder")
        .map_err(|error| format!("source selection: {error:?}"))?;
    noble_wasm::source::Compiler::new_live_slots().prepare_checked_selected(&selected)
        .map_err(|_| "selected compiler refused checked source".to_owned())
}

#[test]
fn selected_returned_program_distinguishes_input_from_fixed_literal_and_computation() -> Result<(), String> {
    use noble_wasm::source::{OriginAction, OriginFixedArithmetic, OriginFixedExpression,
        OriginI64, OriginOperand, OriginProgramValue, OriginSource};
    let varying = selected_builder(b"def builder [ quote [ + ] compose ]", &[Ty::I64])?;
    let fixed = selected_builder(b"def builder [ 2 quote [ + ] compose ]", &[Ty::I64])?;
    let computed = selected_builder(b"def builder [ 1 1 + quote [ + ] compose ]", &[Ty::I64])?;
    for (prepared, expected, position) in [
        (&varying, "root", 0),
        (&fixed, "literal", 1),
        (&computed, "add", 1),
    ] {
        let origin = prepared.selected_origin().ok_or("source-bound output lineage absent")?;
        let [output] = origin.outputs.as_slice() else {
            return Err("selected builder does not return exactly one dynamic Program".into());
        };
        let OriginProgramValue::Compose { left, right, .. } = output.result.as_ref()
        else { return Err("selected compose output lost its checked children".into()); };
        let OriginProgramValue::Static { node: right_node, program_index: right_program_index } =
            right.as_ref() else {
                return Err("selected compose output has no checked static child".into());
            };
        let OriginProgramValue::Quote { operand: OriginOperand::I64(operand), .. } =
            left.as_ref() else {
            return Err("selected quote operand was replaced by a static child".into());
        };
        let source_operand_matches = match (operand, expected) {
            (OriginI64::RootInput(0), "root") => true,
            (OriginI64::Fixed { value: 2, recipe }, "literal") =>
                matches!(recipe.as_ref(), OriginFixedExpression::Literal(2)),
            (OriginI64::Fixed { value: 2, recipe }, "add") =>
                matches!(recipe.as_ref(), OriginFixedExpression::Arithmetic {
                    kind: OriginFixedArithmetic::Add, left, right, ..
                } if matches!(left.as_ref(), OriginFixedExpression::Literal(1))
                    && matches!(right.as_ref(), OriginFixedExpression::Literal(1))),
            _ => false,
        };
        if !source_operand_matches || output.stack_position != position
            || origin.root_program_index == origin.named_program_index
            || origin.programs.len() != prepared.program_metadata().len()
            || !origin.programs.iter().any(|program| program.program_index == *right_program_index
                && program.source_artifact == OriginSource::Definition
                && program.operations.iter().any(|operation|
                    matches!(operation.action, OriginAction::Add)))
            || !origin.programs[origin.named_program_index].operations.iter().any(|operation|
                operation.node == *right_node
                    && matches!(operation.action,
                        OriginAction::StaticProgram(index) if index == *right_program_index))
            || origin.programs.iter().any(|program|
                program.operations.iter().enumerate().any(|(ordinal, operation)|
                    operation.ordinal as usize != ordinal
                        || operation.table_entry != program.entry + operation.ordinal))
        {
            return Err("fixed, variable or static operation was misattributed".into());
        }
        let root = origin.programs.get(origin.root_program_index)
            .ok_or("selection wrapper missing")?;
        if root.source_artifact != OriginSource::Selection
            || root.operations.len() != 1
            || root.operations[0].authored_span.is_some()
            || root.operations[0].lexical_node.is_some()
            || root.operations[0].generated_witness.is_some()
            || !matches!(root.operations[0].action,
                OriginAction::CallSelected(index) if index == origin.named_program_index)
        {
            return Err("source selected wrapper gained a forged body origin".into());
        }
    }
    let named = &computed.selected_origin().ok_or("computed lineage absent")?.programs[0];
    if !named.operations.iter().any(|operation| matches!(operation.action, OriginAction::Add))
        || named.operations.iter().filter(|operation| matches!(operation.action, OriginAction::I64(1)))
            .count() != 2
    {
        return Err("fixed computed capture omitted its executable provenance".into());
    }
    Ok(())
}

#[test]
fn selected_origin_refuses_unproven_multi_output_root_arithmetic_and_over_cap_graphs() -> Result<(), String> {
    let both = selected_builder(b"def builder [ quote swap quote ]", &[Ty::I64, Ty::I64])?;
    if both.selected_origin().is_some() {
        return Err("two separately saved Program outputs acquired one unverified alias graph".into());
    }
    let unsupported = selected_builder(b"def builder [ 1 + quote [ + ] compose ]", &[Ty::I64])?;
    if unsupported.selected_origin().is_some() {
        return Err("root-varying arithmetic was mislabeled as a fixed captured literal".into());
    }
    let large_source = format!("def builder [ quote [ {}+ ] compose ]",
        "dup drop ".repeat(16));
    let large = selected_builder(large_source.as_bytes(), &[Ty::I64])?;
    if large.selected_target().is_none() || large.selected_origin().is_some() {
        return Err("a valid pure Program exceeding the exact graph observation bound acquired selected identity or lost ordinary compilation".into());
    }
    Ok(())
}

#[test]
fn checked_lexical_program_sharing_and_nested_program_capture_remain_distinct_from_source_sites()
-> Result<(), String> {
    use noble_wasm::source::{OriginI64, OriginOperand, OriginProgramValue};
    let shared = selected_builder(
        b"def builder [ quote [ + ] compose let q q q compose [ 0 + ] compose ]", &[Ty::I64],
    ).map_err(|error| format!("shared Program source: {error}"))?;
    let fresh = selected_builder(
        b"def builder [ dup quote [ + ] compose swap quote [ + ] compose compose [ 0 + ] compose ]", &[Ty::I64],
    ).map_err(|error| format!("fresh quotations source: {error}"))?;
    fn inner(prepared: &noble_wasm::source::Prepared)
        -> Result<(&std::rc::Rc<OriginProgramValue>, &std::rc::Rc<OriginProgramValue>), String> {
        let origin = prepared.selected_origin().ok_or("checked returned DAG missing")?;
        let [output] = origin.outputs.as_slice() else {
            return Err("returned graph is not one independently saved owner".into());
        };
        let OriginProgramValue::Compose { left, .. } = output.result.as_ref() else {
            return Err("returned graph lacks checked pure-I64 static continuation".into());
        };
        let OriginProgramValue::Compose { left, right, .. } = left.as_ref() else {
            return Err("returned graph lacks two composed pure-I64 endomorphisms".into());
        };
        Ok((left, right))
    }
    let (shared_left, shared_right) = inner(&shared)?;
    let (fresh_left, fresh_right) = inner(&fresh)?;
    if !std::rc::Rc::ptr_eq(shared_left, shared_right)
        || std::rc::Rc::ptr_eq(fresh_left, fresh_right) {
        return Err("lexical Program reuse was conflated with two quote executions".into());
    }
    for quote in [shared_left, shared_right, fresh_left, fresh_right] {
        let OriginProgramValue::Compose { left, .. } = quote.as_ref() else {
            return Err("reused Program is not a checked pure-I64 endomorphism".into());
        };
        if !matches!(left.as_ref(), OriginProgramValue::Quote {
            operand: OriginOperand::I64(OriginI64::RootInput(0)), ..
        }) { return Err("the same root I64 binding lost its checked quote operand".into()); }
    }
    let nested = selected_builder(
        b"def builder [ quote let q q quote [ run + ] compose ]", &[Ty::I64],
    ).map_err(|error| format!("nested Program source: {error}"))?;
    let [output] = nested.selected_origin().ok_or("nested Program source origin missing")?
        .outputs.as_slice() else {
            return Err("nested Program quote returned an ambiguous owner".into());
        };
    let OriginProgramValue::Compose { left, .. } = output.result.as_ref() else {
        return Err("nested Program quote lacks checked continuation".into());
    };
    if !matches!(left.as_ref(), OriginProgramValue::Quote {
        operand: OriginOperand::Program(program), ..
    } if matches!(program.as_ref(), OriginProgramValue::Quote {
        operand: OriginOperand::I64(OriginI64::RootInput(0)), ..
    })) {
        return Err("quote(Program) lost its nested checked root capture".into());
    }
    Ok(())
}

#[test]
fn quotation_program_has_its_own_checked_interface_and_code_owner() -> Result<(), String> {
    let session = noble_contracts::source::Session::new_live_slots(
        environment().map_err(|error| format!("{error:?}"))?.enable_live_slots(),
    ).map_err(|error| error.diagnostic().message.clone())?;
    let source = session.prepare(b"[ 1 + ]", &[], LIMITS)
        .map_err(|error| error.diagnostic().message.clone())?;
    let prepared = noble_wasm::source::Compiler::new_live_slots()
        .prepare(source.submission().ok_or("quotation submission missing")?)
        .map_err(|_| "quotation failed independent Wasm admission")?;
    let [wrapper, saved] = prepared.program_metadata() else {
        return Err("quotation did not emit both wrapper and selected Program".into());
    };
    let program = Ty::program(vec![Ty::I64], vec![Ty::I64], EffSet::empty());
    let span = prepared.code_span();
    if wrapper.program_index == saved.program_index
        || wrapper.stack_in != []
        || wrapper.stack_out != [program]
        || saved.stack_in != [Ty::I64]
        || saved.stack_out != [Ty::I64]
        || saved.effects != EffSet::empty()
        || saved.capture_left != 0 || saved.capture_right != 0
        || saved.recipe_leaves != 2
        || saved.entry < span.start
        || saved.entry >= span.start + span.length
        || prepared.target_metadata().is_none_or(|root| {
            root.root_program_index != wrapper.program_index
                || root.input_signature != wrapper.input_signature
                || root.output_signature != wrapper.output_signature
        })
    {
        return Err("saved quote was conflated with its entry wrapper or had forgeable captures".into());
    }
    Ok(())
}

#[test]
fn explicitly_granted_test_host_retains_two_checked_effects_and_selected_body() -> Result<(), String> {
    let env = environment().map_err(|problem| format!("{problem:?}"))?.enable_live_slots();
    let body = b"def replay [ \"first\" test.emit \"second\" test.emit 42 ]";
    let ordinary = noble_contracts::source::Session::new_live_slots(env.clone())
        .map_err(|problem| problem.diagnostic().message.clone())?;
    if ordinary.prepare(body, &[], LIMITS).is_ok() {
        return Err("ordinary live-slot source unexpectedly resolved test.emit".into());
    }
    let flagged_but_unselected = noble_contracts::source::Session::new_live_slots(
        env.clone().enable_live_test_hosts()
    ).map_err(|problem| problem.diagnostic().message.clone())?;
    if flagged_but_unselected.prepare(body, &[], LIMITS).is_ok() {
        return Err("environment flag bypassed the selected source namespace".into());
    }
    let mut granted = noble_contracts::source::Session::new_live_slots_with_test_hosts(env)
        .map_err(|problem| problem.diagnostic().message.clone())?;
    let definition = granted.prepare(body, &[], LIMITS)
        .map_err(|problem| problem.diagnostic().message.clone())?;
    granted.commit(definition).map_err(|problem| problem.diagnostic().message.clone())?;
    let call = granted.prepare(b"replay", &[], LIMITS)
        .map_err(|problem| problem.diagnostic().message.clone())?;
    let selected = granted.checked_selected_target(&call, "replay")
        .map_err(|problem| format!("selected effect body: {problem:?}"))?;
    let prepared = noble_wasm::source::Compiler::new_live_slots()
        .prepare_checked_selected(&selected)
        .map_err(|_| "effectful selected body failed independent Wasm admission")?;
    let named = prepared.named_targets().first().ok_or("named effect body missing")?;
    let global = prepared.program_metadata().get(named.program_index)
        .ok_or("effectful named Program global missing")?;
    let root = prepared.target_metadata().ok_or("effectful wrapper missing")?;
    if prepared.selected_target().is_none_or(|binding| {
        binding.definition_id != selected.definition_id()
            || binding.named_program_index != named.program_index
    })
        || named.stack_in != []
        || named.stack_out != [Ty::Unit, Ty::Unit, Ty::I64]
        || named.effect_mask != 1
        || global.effect_mask != 1
        || global.stack_in != []
        || global.stack_out != [Ty::Unit, Ty::Unit, Ty::I64]
        || root.effect_mask != 1
    {
        return Err("selected test-effect body lost its checked interface".into());
    }
    Ok(())
}
