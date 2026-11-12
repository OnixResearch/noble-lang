#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Real selected-CLI coverage of the saved-Program ledger boundary: the host
//! admits exactly the reviewed capacity of owned Program outputs and refuses the
//! next root before the guest runs, naming the capacity, instead of executing a
//! root whose outputs it cannot retain. This is behavioral evidence, not a
//! live-slot theorem.

#[test]
fn an_over_budget_saved_program_root_is_refused_before_the_guest_runs() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_program_capacity.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 saved-Program capacity child");
    assert!(
        output.status.success(),
        "checked CLI saved-Program capacity witness failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
