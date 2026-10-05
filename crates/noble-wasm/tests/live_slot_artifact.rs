#![feature(register_tool)]
#![register_tool(tigerstyle)]

use noble_kernel::contracts::environment;
use noble_kernel::types::Ty;

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
