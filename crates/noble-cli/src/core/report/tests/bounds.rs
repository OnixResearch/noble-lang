#[test]
fn document_limit_counts_utf8_bytes_at_the_exact_boundary() -> Result<(), String> {
    let payload = "é".repeat((4_194_304 - 2) / 2);
    let exact = format!("\"{payload}\"");
    let decoded = super::super::parse(&exact)?;
    assert_eq!(decoded.text(), Some(payload.as_str()));
    assert!(super::super::parse(&format!("\"{payload}x\"")).is_err());
    Ok(())
}

#[test]
fn nesting_limit_counts_values_including_empty_containers() -> Result<(), String> {
    let limit = 32;
    for (open, close) in [("[", "]"), ("{\"a\":", "}")] {
        let exact = format!("{}null{}", open.repeat(limit - 1), close.repeat(limit - 1));
        let too_deep = format!("{}null{}", open.repeat(limit), close.repeat(limit));
        let expected = (0..limit - 1).fold(super::super::Value::Null, |value, _| {
            if open == "[" {
                super::super::Value::Array(vec![value])
            } else {
                super::super::Value::Object(std::collections::BTreeMap::from([(
                    String::from("a"),
                    value,
                )]))
            }
        });
        assert_eq!(super::super::parse(&exact)?, expected);
        assert!(super::super::parse(&too_deep).is_err());
    }
    for (terminal, leaf) in [
        ("[]", super::super::Value::Array(vec![])),
        (
            "{}",
            super::super::Value::Object(std::collections::BTreeMap::new()),
        ),
    ] {
        let exact = format!(
            "{}{terminal}{}",
            "[".repeat(limit - 1),
            "]".repeat(limit - 1)
        );
        let too_deep = format!("{}{terminal}{}", "[".repeat(limit), "]".repeat(limit));
        let expected =
            (0..limit - 1).fold(leaf, |value, _| super::super::Value::Array(vec![value]));
        assert_eq!(super::super::parse(&exact)?, expected);
        assert!(super::super::parse(&too_deep).is_err());
    }
    Ok(())
}
