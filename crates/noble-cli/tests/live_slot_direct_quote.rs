#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Saved Program owners against the real selected CLI. A direct typed quote must retain its actual
//! boxed capture as a usable saved Program. A saved Program whose compiled body reads a named
//! definition through its module's global must survive that module's supersession, and the module
//! retires only after the owner is released (OWN-CASE-06). A Program saved from a slot dispatch
//! keeps, and is charged for, its version's package across replacement (LREF-CASE-03).
//! Shared and composed owners keep their Programs alive until their own release (OWN-CASE-02,
//! OWN-CASE-03). A root whose invocation fails releases its map pin and every frame pin exactly
//! once from the host ledger, and the designed `test.abort` escape trap is measured as refused by
//! the live-slot source profile (OWN-CASE-05). After an abnormal root exit the session continues
//! only when that release is provably exact, and is poisoned otherwise (LREF-CASE-01). Behavioral
//! evidence, not a proof. An empty quotation, whose entry is the shared core empty entry rather
//! than code in its module's span, installs, runs and is saved like any other Program.
//! A Program escaped from a slot dispatch keeps its own recipe after publication (LREF-CASE-04),
//! one installed generic caller selects the version each root pinned (LREF-CASE-05), and a root
//! held across a publication selects its pinned version for every dispatch while an older saved
//! Program it receives runs its own code (LREF-CASE-06).

#[test]
fn direct_quote_owner_can_run_and_release_without_retaining_code() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_direct_quote.mjs"
    );
    let output = std::process::Command::new(node)
        .arg(script)
        .arg(env!("CARGO_BIN_EXE_noble"))
        .output()
        .expect("launch selected live-slot quote-owner child");
    assert!(
        output.status.success(),
        "selected direct quote failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn a_saved_named_caller_survives_module_root_retirement() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_module_root_retention.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 module-root retention child");
    assert!(
        output.status.success(),
        "checked CLI module-root retention witness failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn a_dispatched_result_keeps_and_is_charged_for_its_package() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_package_retention.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 package retention child");
    assert!(
        output.status.success(),
        "checked CLI package retention witness failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn shared_and_composed_owners_outlive_each_other() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_ownership_cases.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 ownership case child");
    assert!(
        output.status.success(),
        "checked CLI ownership cases failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn a_failed_root_issues_no_saved_owner_and_poisons_the_session() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_own_trap.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 ownership trap child");
    assert!(
        output.status.success(),
        "checked CLI ownership trap witness failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn an_abnormal_root_exit_releases_its_pins_exactly_once() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_abnormal_exit.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 abnormal root exit child");
    assert!(
        output.status.success(),
        "checked CLI abnormal root exit lane failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn an_empty_quotation_installs_runs_and_is_saved() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_empty_quotation.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 empty quotation child");
    assert!(
        output.status.success(),
        "checked CLI empty quotation lane failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn an_escaped_result_never_follows_publication() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_escaped_result.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 escaped result child");
    assert!(
        output.status.success(),
        "checked CLI escaped result lane failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn a_saved_generic_caller_selects_the_version_each_root_pinned() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_generic_caller_roots.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 generic caller child");
    assert!(
        output.status.success(),
        "checked CLI generic caller lane failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn a_held_root_keeps_its_map_and_runs_an_older_program() {
    let output = std::process::Command::new(
        "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node",
    )
    .arg(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/live_slot_held_root_mixing.mjs"
    ))
    .arg(env!("CARGO_BIN_EXE_noble"))
    .output()
    .expect("launch selected Node/V8 held root child");
    assert!(
        output.status.success(),
        "checked CLI held root lane failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
