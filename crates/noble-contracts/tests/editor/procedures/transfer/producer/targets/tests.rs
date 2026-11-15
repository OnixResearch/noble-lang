use super::evaluator::evaluate_program;
use super::*;

#[test]
fn nonheldout_three_input_stack_evaluator_has_refusals() -> Result<(), String> {
    assert_eq!(evaluate_program("+ -", [5, -5, 7])?, 3);
    assert_eq!(evaluate_program("- 7 + 7 + +", [5, -5, 7])?, 7);
    assert!(evaluate_program("42 + +", [0, 0, 0]).is_err());
    assert!(evaluate_program("dup dup", [1, 2, 3]).is_err());
    assert!(evaluate_program("= +", [1, 2, 3]).is_err());
    Ok(())
}

#[test]
fn derived_criterion_matches_source_programs_on_nonheldout_inputs() -> Result<(), String> {
    let criterion = read_json("verification/procedure-library/criteria-producer-targets-v3.json")?;
    let acquisition =
        read_json("verification/synthesis/receipts/2026-11-13/synthesis-v2.json")?;
    let source_tasks = acquisition["tasks"]
        .as_array()
        .ok_or_else(|| String::from("source producer has no task array"))?;
    let program = |name: &str| -> Result<&str, String> {
        source_tasks
            .iter()
            .find(|task| task["name"] == name)
            .and_then(|task| task["search"]["program"].as_str())
            .ok_or_else(|| format!("no retained #116 source program for {name}"))
    };
    let derived = criterion["producer"]["target_tasks"]
        .as_array()
        .filter(|tasks| tasks.len() == 2)
        .ok_or_else(|| String::from("v3 criterion target count drifted"))?;
    let manifest = super::super::target_reference::manifest_targets_from(
        "verification/procedure-library/producer-targets-v3.txt",
    )?;
    for (index, target) in derived.iter().enumerate() {
        let source_order: [&str; 2] = [
            target["source_order"][0]
                .as_str()
                .ok_or_else(|| String::from("v3 first source missing"))?,
            target["source_order"][1]
                .as_str()
                .ok_or_else(|| String::from("v3 second source missing"))?,
        ];
        assert_eq!(source_order, TARGET_SOURCE_ORDERS[index]);
        assert_eq!(target["name"], manifest[index].0);
        let reference = target["reference"]
            .as_str()
            .ok_or_else(|| String::from("v3 derived target has no reference"))?;
        assert_eq!(manifest[index].1, reference);
        let programs = [program(source_order[0])?, program(source_order[1])?];
        for inputs in [[0, 0, 0], [5, -5, 7], [1, 2, 3], [i64::MIN, 11, -9]] {
            let composed = evaluator::evaluate_composed(programs, inputs)?;
            let declared = super::super::target_reference::evaluate(reference, inputs)?;
            assert_eq!(
                composed, declared,
                "v3 derived reference disagrees with #116 source bodies at {inputs:?}"
            );
        }
    }
    Ok(())
}
