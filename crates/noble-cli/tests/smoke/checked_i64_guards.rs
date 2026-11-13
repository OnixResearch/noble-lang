use super::checked_i64::run_session;

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

fn assert_refusal(
    name: &str,
    arguments: &[&str],
    source: &str,
    profile: &str,
    outcome: &str,
    stage: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = run_session(arguments, &[source.to_owned()])?;
    assert!(!output.status.success(), "{name} unexpectedly succeeded");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["profile"], profile, "{name}");
    assert_eq!(report["stage"], stage, "{name}");
    assert_eq!(report["outcome"], outcome, "{name}");
    assert_no_host_requests(&report, name);
    Ok(())
}

#[test]
fn checked_i64_rejections_and_resource_exhaustion_start_no_guest(
) -> Result<(), Box<dyn std::error::Error>> {
    struct Case {
        name: &'static str,
        arguments: &'static [&'static str],
        source: &'static str,
        profile: &'static str,
        outcome: &'static str,
        stage: &'static str,
    }
    let cases = [
        Case {
            name: "wrong operand type",
            arguments: &["session", "--framed", "--checked-i64"],
            source: "1 true i64.checked-add",
            profile: "Checked-I64-v1",
            outcome: "type-reject",
            stage: "check",
        },
        Case {
            name: "builtin without profile",
            arguments: &["session", "--framed"],
            source: "1 2 i64.checked-add",
            profile: "Core-Bootstrap",
            outcome: "unbound-word",
            stage: "resolve",
        },
        Case {
            name: "zero source-work limit",
            arguments: &["session", "--framed", "--checked-i64", "--source-work", "0"],
            source: "1 2 i64.checked-add",
            profile: "Checked-I64-v1",
            outcome: "exhausted",
            stage: "parse",
        },
    ];
    for case in cases {
        assert_refusal(
            case.name,
            case.arguments,
            case.source,
            case.profile,
            case.outcome,
            case.stage,
        )?;
    }
    Ok(())
}
