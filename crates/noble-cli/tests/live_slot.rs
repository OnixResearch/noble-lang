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
    run_lane_args(lane, &[]);
}

fn run_lane_args(lane: &str, arguments: &[&str]) {
    let output = std::process::Command::new(NODE)
        .arg(std::format!("{}/tests/{lane}", env!("CARGO_MANIFEST_DIR")))
        .arg(env!("CARGO_BIN_EXE_noble"))
        .args(arguments)
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

/// One root selects the same leaf slot twice while a compare-and-set republication of that leaf
/// commits between the two selections. Both selections must come from the root's pinned map, so
/// the root performs exactly the pinned version's events twice or not at all; a runtime that read
/// the registry's current map per dispatch would mix the versions and perform exactly one.
#[test]
fn a_held_root_selects_the_same_leaf_twice_from_its_pinned_map() {
    run_lane("live_slot_two_selections.mjs");
}

/// Two interface-identical versions of the same caller slot move one distinguished event out of
/// the caller and into the callee: the old version emits and then dispatches, the new one only
/// dispatches and the callee's version emits. Both keep the same checked caller site and the same
/// stack result. A root frame cannot emit, which is why the calling *slot* performs the effect.
#[test]
fn an_event_relocates_from_the_caller_version_into_the_callee() {
    run_lane("live_slot_caller_emits.mjs");
}

/// A root pinned between two compatible slot publications observes the mixed pair rather than an
/// all-old or all-new pair; the lane also checks old/new caller-to-callee relocation.
#[test]
fn sequential_publications_expose_a_mixed_compatible_pair() {
    run_lane("live_slot_relocation_controls.mjs");
}

#[cfg(not(feature = "live-test-barrier"))]
#[test]
fn shipped_cli_does_not_contain_the_test_only_current_map_seam() {
    const MARKER: &[u8] = b"noble-test-current-map-seam/v1";
    let binary = std::fs::read(env!("CARGO_BIN_EXE_noble")).expect("read selected noble binary");
    assert!(
        !binary.windows(MARKER.len()).any(|window| window == MARKER),
        "default noble binary must not contain the test-only current-map seam"
    );
}

#[cfg(feature = "live-test-barrier")]
#[test]
fn current_map_mutant_duplicates_and_omits_relocated_events() {
    run_lane_args("live_slot_relocation_controls.mjs", &["current-map"]);
}
