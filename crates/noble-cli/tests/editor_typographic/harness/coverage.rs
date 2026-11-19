//! Coverage leg: kinds, frozen expectations, hole binding and rendering.

use serde_json::{json, Value};

use super::super::cases::Fixture;
use super::super::document;
use super::super::measure::Runner;
use super::super::surface::View;
use super::{analyze_facts, widget_kinds};

#[test]
fn covers_every_kind_and_hole_state_through_the_cli() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = super::super::frozen::Snapshot::load()?;
    let mut runner = Runner::new(&super::super::scratch("coverage"))?;
    let mut measured = Vec::with_capacity(corpus.fixtures.len());
    for fixture in &corpus.fixtures {
        measured.push(check_fixture(&mut runner, fixture)?);
    }
    let mut kinds: Vec<String> = Vec::new();
    for measurement in &measured {
        let Some(entry) = measurement.get("kinds").and_then(Value::as_array) else {
            return Err("measurement carries no kinds".into());
        };
        for kind in entry {
            kinds.push(kind.as_str().unwrap_or_default().to_owned());
        }
    }
    kinds.sort();
    kinds.dedup();
    assert_eq!(
        kinds,
        super::super::FIVE_KINDS,
        "the corpus must cover all five kinds"
    );

    let mut states = Vec::with_capacity(corpus.hole_states.len());
    for state in &corpus.hole_states {
        let fixture = corpus
            .fixture(&state.fixture)
            .ok_or_else(|| format!("hole state {} names an unknown fixture", state.state))?;
        let measurement = measured
            .iter()
            .find(|entry| entry["fixture"] == fixture.name.as_str())
            .ok_or_else(|| format!("hole state {} was not measured", state.state))?;
        let holes = measurement["holes"].as_u64().unwrap_or(0);
        assert!(
            holes >= 1,
            "hole state {} must render at least one hole",
            state.state
        );
        states.push(json!({
            "state": state.state,
            "fixture": fixture.name,
            "holes": holes,
            "program_effect": measurement["program_effect"].clone(),
        }));
    }
    // The unresolved-effect and known-effect states must differ in the report,
    // not only in the corpus prose.
    let unresolved = states
        .iter()
        .find(|state| state["state"] == "unresolved-effect")
        .ok_or("missing unresolved-effect state")?;
    let known = states
        .iter()
        .find(|state| state["state"] == "known-effect")
        .ok_or("missing known-effect state")?;
    assert_eq!(unresolved["program_effect"]["known"], json!([]));
    assert_eq!(unresolved["program_effect"]["unresolved"], json!(true));
    assert_eq!(known["program_effect"]["known"], json!([0]));
    assert_eq!(known["program_effect"]["unresolved"], json!(true));
    Ok(())
}

fn check_fixture(runner: &mut Runner, fixture: &Fixture) -> Result<Value, String> {
    let view = View::from_nodes(&fixture.nodes).map_err(|refusal| refusal.detail)?;
    assert_eq!(
        widget_kinds(&view.root),
        fixture.kinds,
        "{}: widget kinds",
        fixture.name
    );
    assert!(
        document::nodes_equal(&view.to_nodes(), &fixture.nodes),
        "{}: identity re-encoding changed the node tree",
        fixture.name
    );
    let document = fixture.document()?;
    let analyze = runner.analyze(&fixture.name, &document)?;
    let (holes, effect) = analyze_facts(&analyze)?;
    assert_eq!(
        analyze.exit, fixture.expect.analyze_exit,
        "{}",
        fixture.name
    );
    assert_eq!(holes.len(), fixture.expect.holes, "{}", fixture.name);
    assert_eq!(
        effect.known, fixture.expect.program_effect_known,
        "{}",
        fixture.name
    );
    assert_eq!(
        effect.unresolved, fixture.expect.program_effect_unresolved,
        "{}",
        fixture.name
    );
    let mut bound = view;
    bound.set_program_effect(effect.clone());
    bound
        .bind_report(&holes)
        .map_err(|refusal| refusal.detail)?;
    assert_eq!(bound.hole_facts(), holes, "{}: bound holes", fixture.name);
    let rendered = bound.render();
    assert_eq!(
        rendered.matches("hole{").count(),
        holes.len(),
        "{}: rendered holes",
        fixture.name
    );
    for hole in &holes {
        assert!(
            rendered.contains(&hole.input_stack) && rendered.contains(&hole.output_stack),
            "{}: a rendered hole hides its reported stacks",
            fixture.name
        );
    }
    let admit = runner.admit(&fixture.name, &document)?;
    assert_eq!(admit.exit, fixture.expect.admit_exit, "{}", fixture.name);
    assert_eq!(admit.stage, fixture.expect.admit_stage, "{}", fixture.name);
    assert_eq!(
        admit.outcome, fixture.expect.admit_outcome,
        "{}",
        fixture.name
    );
    assert_eq!(
        admit.category, fixture.expect.admit_category,
        "{}",
        fixture.name
    );
    if document::contains_hole(&fixture.nodes) {
        super::assert_hole_refusal(&fixture.name, &admit)?;
    }
    Ok(json!({
        "fixture": fixture.name,
        "kinds": fixture.kinds,
        "holes": holes.len(),
        "program_effect": {
            "known": effect.known,
            "unresolved": effect.unresolved,
        },
        "analyze": analyze.json(),
        "admit": admit.json(),
        "rendered": rendered,
    }))
}
