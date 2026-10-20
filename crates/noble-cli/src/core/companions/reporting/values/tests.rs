// The protocol has integer semantics: -0 and 0 denote the same integer. serde
// retains -0 as a floating signed zero; do not require its internal Number tag.
fn same_integer_json(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    match (left, right) {
        (serde_json::Value::Number(left), serde_json::Value::Number(right)) => {
            left == right || (left.as_f64() == Some(0.0) && right.as_f64() == Some(0.0))
        }
        (serde_json::Value::Array(left), serde_json::Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same_integer_json(left, right))
        }
        (serde_json::Value::Object(left), serde_json::Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, value)| {
                    right
                        .get(key)
                        .is_some_and(|right| same_integer_json(value, right))
                })
        }
        _ => left == right,
    }
}

#[test]
fn report_corpus_preserves_independently_decoded_semantics() -> Result<(), String> {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("../../../report/tests/corpus.json"))
            .map_err(|error| error.to_string())?;
    let cases = cases
        .as_array()
        .ok_or("conformance corpus must be an array")?;
    for case in cases {
        let id = case["id"].as_str().ok_or("case must have an identity")?;
        let document = case["document"]
            .as_str()
            .ok_or("case must have a document")?;
        let decoded = crate::core::report::parse(document);
        if case["accept"] == true {
            let value = decoded.map_err(|error| format!("{id}: {error}"))?;
            let reference: serde_json::Value =
                serde_json::from_str(document).map_err(|error| error.to_string())?;
            assert!(same_integer_json(&reference, &case["value"]), "{id}");
            let encoded = super::text_of(&value);
            let roundtrip: serde_json::Value =
                serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
            assert!(same_integer_json(&roundtrip, &reference), "{id}");
            assert_eq!(crate::core::report::parse(&encoded)?, value, "{id}");
        } else {
            assert!(decoded.is_err(), "{id}: {document:?}");
        }
    }
    Ok(())
}

#[test]
fn reencoded_report_integers_remain_exact_at_both_numeric_boundaries() -> Result<(), String> {
    for (document, signed, unsigned) in [
        ("-9223372036854775808", Some(i64::MIN), None),
        ("9223372036854775807", Some(i64::MAX), Some(i64::MAX as u64)),
        ("9223372036854775808", None, Some(1_u64 << 63)),
        ("18446744073709551615", None, Some(u64::MAX)),
    ] {
        let decoded = crate::core::report::parse(document)?;
        let encoded = super::text_of(&decoded);
        let reference: serde_json::Value =
            serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
        assert_eq!(reference.as_i64(), signed, "{document}");
        assert_eq!(reference.as_u64(), unsigned, "{document}");
        assert_eq!(crate::core::report::parse(&encoded)?, decoded);
    }
    Ok(())
}
