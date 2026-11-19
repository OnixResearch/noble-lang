//! Refusal leg: renderer refusals and the unchanged ingress refusals.

use serde_json::{json, Value};

use super::super::frozen::Snapshot;
use super::super::measure::Runner;
use super::super::surface::View;
use super::assert_no_new_action;

#[test]
fn negative_controls_refuse_before_admission() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = Snapshot::load()?;
    let mut runner = Runner::new(&super::super::scratch("controls"))?;
    negative_measurement(&corpus, &mut runner)?;
    assert_no_new_action(&corpus)?;
    Ok(())
}

/// Every frozen control is refused by the renderer with no CLI invocation, and
/// the hostile transport documents still refuse with no accepted program. The
/// refusal rows and the hostile verdicts are returned so the receipt can carry
/// both.
pub(super) fn negative_measurement(
    corpus: &Snapshot,
    runner: &mut Runner,
) -> Result<(Vec<Value>, Vec<Value>), String> {
    let mut refused = Vec::with_capacity(corpus.negative_controls.len());
    for control in &corpus.negative_controls {
        let fixture = corpus
            .fixture(&control.fixture)
            .ok_or_else(|| format!("{}: unknown fixture", control.name))?;
        let view = View::from_nodes(&fixture.nodes).map_err(|refusal| refusal.detail)?;
        let before = runner.invocations();
        let error = control
            .gesture
            .apply(&view)
            .expect_err(&format!("{}: the gesture was not refused", control.name));
        assert_eq!(error.code, control.code, "{}", control.name);
        assert!(!error.detail.is_empty(), "{}", control.name);
        assert_eq!(
            runner.invocations(),
            before,
            "{}: a refused gesture must not reach the CLI",
            control.name
        );
        refused.push(json!({
            "control": control.name,
            "code": error.code,
            "fixture": fixture.name,
            "admit_invocations": runner.invocations().saturating_sub(before),
        }));
    }
    assert_eq!(refused.len(), corpus.negative_controls.len());

    let mut hostile = Vec::with_capacity(corpus.hostile_transport.len());
    for (index, document) in corpus.hostile_transport.iter().enumerate() {
        let name = format!("hostile-{index}");
        let admit = runner.admit(&name, document)?;
        let analyze = runner.analyze(&name, document)?;
        for verdict in [&analyze, &admit] {
            assert_eq!(verdict.exit, 2, "{name}: {}", verdict.summary());
            assert_eq!(verdict.outcome, "reject", "{name}");
            assert_eq!(
                verdict.accepted_program_created,
                Some(false),
                "{name}: the ingress claimed an accepted program"
            );
            assert_eq!(verdict.guest_requests, 0, "{name}");
            assert_eq!(verdict.protected_operations, 0, "{name}");
        }
        hostile.push(json!({
            "document": document,
            "analyze": analyze.json(),
            "admit": admit.json(),
        }));
    }
    Ok((hostile, refused))
}
