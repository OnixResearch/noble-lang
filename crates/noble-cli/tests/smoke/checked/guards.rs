fn assert_no_host_requests(report: &serde_json::Value, name: &str) {
    for field in [
        "guest_requests",
        "protected_operations",
        "candidate_prepare_requests",
    ] {
        assert_eq!(report[field], 0, "{name}: {field}");
    }
    assert_eq!(report["prior_stack"], "unchanged", "{name}");
}

struct RefusalCase {
    name: &'static str,
    arguments: &'static [&'static str],
    source: &'static str,
    profile: &'static str,
    outcome: &'static str,
    stage: &'static str,
}

fn assert_refusal(case: &RefusalCase) -> Result<(), Box<dyn std::error::Error>> {
    let output = super::checked_i64::run_session(case.arguments, &[case.source.to_owned()])?;
    assert!(
        !output.status.success(),
        "{} unexpectedly succeeded",
        case.name
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["profile"], case.profile, "{}", case.name);
    assert_eq!(report["stage"], case.stage, "{}", case.name);
    assert_eq!(report["outcome"], case.outcome, "{}", case.name);
    assert_no_host_requests(&report, case.name);
    Ok(())
}

#[test]
fn rejections_and_resource_exhaustion_start_no_guest() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        RefusalCase {
            name: "wrong operand type",
            arguments: &["session", "--framed", "--checked-i64"],
            source: "1 true i64.checked-add",
            profile: "Checked-I64-v1",
            outcome: "type-reject",
            stage: "check",
        },
        RefusalCase {
            name: "builtin without profile",
            arguments: &["session", "--framed"],
            source: "1 2 i64.checked-add",
            profile: "Core-Bootstrap",
            outcome: "unbound-word",
            stage: "resolve",
        },
        RefusalCase {
            name: "zero source-work limit",
            arguments: &["session", "--framed", "--checked-i64", "--source-work", "0"],
            source: "1 2 i64.checked-add",
            profile: "Checked-I64-v1",
            outcome: "exhausted",
            stage: "parse",
        },
    ];
    for case in &cases {
        assert_refusal(case)?;
    }
    Ok(())
}
