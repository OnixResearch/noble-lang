//! Host-owned slot registry transitions in the selected real Node/V8 child.
//! This does not claim a compiled Noble `slot.invoke` or LSLOT acceptance.

#[test]
fn host_registry_pins_authorizes_and_retires_real_versions() {
    let node = "/nix/store/sy0c7j0npsq33d9zhnnzvjnzc52f4y0p-nodejs-24.13.0/bin/node";
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/live_slot_registry.mjs");
    let output = std::process::Command::new(node)
        .arg(script)
        .output()
        .expect("launch selected Node/V8 host registry child");
    assert!(
        output.status.success(),
        "host registry transitions failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    assert_eq!(
        output.stdout,
        b"live-slot host registry: pin/CAS/authority/quota/nominal/evidence transitions passed\n"
    );
}
