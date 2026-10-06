//! A classified dynamic saved owner can pin an independently admitted static child.
use noble_kernel::types::{EffSet, Ty};
use noble_wasm::source::OriginProgramValue;

#[test]
fn selected_dynamic_owner_retains_actual_published_static_child() -> Result<(), String> {
    let env = noble_kernel::contracts::environment().map_err(|error| format!("{error:?}"))?
        .enable_live_slots();
    let mut session = noble_contracts::source::Session::new_live_slots(env)
        .map_err(|error| error.diagnostic().message.clone())?;
    let limits = noble_contracts::Limits { bytes: 65_536, nodes: 16_384, depth: 64, work: 2_000_000 };
    let definition = session.prepare(b"def builder [ quote [ 1 + ] compose ]", &[], limits)
        .map_err(|error| error.diagnostic().message.clone())?;
    session.commit(definition).map_err(|error| error.diagnostic().message.clone())?;
    let call = session.prepare(b"builder", &[Ty::I64], limits)
        .map_err(|error| error.diagnostic().message.clone())?;
    let selected = session.checked_selected_target(&call, "builder")
        .map_err(|error| format!("{error:?}"))?;
    let prepared = noble_wasm::source::Compiler::new_live_slots()
        .prepare_checked_selected(&selected)
        .map_err(|_| "checked selected builder failed compiler admission")?;
    let origin = prepared.selected_origin().ok_or("selected builder has no checked origin")?;
    let [output] = origin.outputs.as_slice() else {
        return Err("selected builder must return one checked Program".into());
    };
    let OriginProgramValue::Compose { right_program_index, .. } = &output.result else {
        return Err("selected saved Program lacks checked static child".into());
    };
    let child = prepared.program_metadata().get(*right_program_index)
        .ok_or("checked static child metadata absent")?;
    if child.program_index != *right_program_index || child.stack_in != [Ty::I64]
        || child.stack_out != [Ty::I64] || child.effects != EffSet::empty()
        || *right_program_index == origin.named_program_index
        || *right_program_index == origin.root_program_index {
        return Err("static child does not match independently checked Q slot contract".into());
    }
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/live_slot_selected_retention.mjs");
    let result = std::process::Command::new(node)
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_noble"))
        .arg(child.program_index.to_string())
        .arg(child.entry.to_string())
        .output()
        .map_err(|error| format!("launch selected CLI retention child: {error}"))?;
    if !result.status.success() {
        return Err(format!("selected dynamic-owner retention failed: stdout={} stderr={}",
            String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr)));
    }
    Ok(())
}
