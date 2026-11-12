#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![allow(
    tigerstyle::no_unwrap,
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; this integration harness constructs boundary scenarios and asserts their outcomes, so terse fixture names, unwraps and long setup legs are the readable form."
)]
//! Host-owned slot transitions and checked CLI lanes in selected Node/V8.
//! These tests do not claim an independently checked LSLOT05 proof. Every lane
//! that a cargo test runs keeps its own `.mjs` witness under `tests/`; the
//! harnesses live here so the reviewed compilation-unit capacity is unchanged.

const NODE: &str = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";

/// Run one selected-CLI lane with the real binary and require exit 0.
fn run_lane(lane: &str) {
    let output = std::process::Command::new(NODE)
        .arg(std::format!("{}/tests/{lane}", env!("CARGO_MANIFEST_DIR")))
        .arg(env!("CARGO_BIN_EXE_noble"))
        .output()
        .expect("launch selected Node/V8 lane child");
    assert!(
        output.status.success(),
        "checked CLI lane {lane} failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[test]
fn host_registry_pins_authorizes_and_retires_real_versions() {
    run_lane("live_slot_registry.mjs");
}

#[test]
fn cli_replay_refuses_changed_root_before_guest_and_keeps_exact_replay() {
    run_lane("live_slot_replay_failure.mjs");
}

/// The nested-root chain (canonical LSLOT-02) previously ran only from the ad
/// hoc `verification/live-slot-current-source` gate, which accepted a failing
/// exit; a cargo test now requires it to pass.
#[test]
fn nested_slot_roots_keep_one_pinned_map_epoch() {
    run_lane("live_slot_nested_roots.mjs");
}

/// The canonical LSLOT-05 proof-required refusal previously ran only from the
/// ad hoc gate, which accepted a failing exit.
#[test]
fn a_proof_required_slot_refuses_a_publication_without_target_evidence() {
    run_lane("live_slot_proof_refusal.mjs");
}

/// A nominal-scoped `effect` grant can never authorize — or re-enable — a
/// protected effect that a slot-scoped grant denied, through either the
/// authority file or a runtime policy request.
#[test]
fn an_effect_grant_is_never_widened_from_the_nominal_scope() {
    run_lane("live_slot_grant_scope.mjs");
}

/// The host refuses a root whose checked Program outputs exceed the reviewed
/// saved-Program ledger before the guest runs, naming the capacity.
#[test]
fn an_over_budget_saved_program_root_is_refused_before_the_guest_runs() {
    run_lane("live_slot_program_capacity.mjs");
}

/// A control that commits during a frozen replay keeps its operator receipt and
/// does not poison the session.
#[test]
fn a_control_that_commits_during_replay_keeps_its_receipt_and_the_session() {
    run_lane("live_slot_replay_control.mjs");
}
