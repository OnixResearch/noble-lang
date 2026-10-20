#[test]
fn decodes_every_short_escape() {
    assert_eq!(
        super::super::parse(r#""\"\\\/\b\f\n\r\t""#),
        Ok(super::super::Value::Text(std::string::String::from(
            "\"\\/\u{8}\u{c}\n\r\t",
        )))
    );
}

#[test]
fn rejects_every_raw_control_byte() {
    for byte in 0_u8..=31 {
        let mut document = std::string::String::from("\"");
        document.push(char::from(byte));
        document.push('"');
        assert!(super::super::parse(&document).is_err());
    }
}

#[test]
fn decodes_all_escaped_controls() {
    for byte in 0_u8..=31 {
        let document = std::format!("\"\\u{byte:04x}\"");
        let expected = std::format!("{}", char::from(byte));
        assert_eq!(
            super::super::parse(&document),
            Ok(super::super::Value::Text(expected))
        );
    }
}

#[test]
fn matches_raw_and_escaped_bmp_and_non_bmp() {
    assert_eq!(
        super::super::parse("\"é雪\""),
        super::super::parse(r#""\u00e9\u96ea""#)
    );
    assert_eq!(
        super::super::parse("\"😀\""),
        super::super::parse(r#""\uD83D\uDE00""#)
    );
}

#[test]
fn accepts_unicode_escape_scalar_boundaries() {
    assert_eq!(
        super::super::parse(r#""\u0000\uFFFF""#),
        Ok(super::super::Value::Text(std::string::String::from(
            "\u{0}\u{ffff}",
        )))
    );
}

#[test]
fn rejects_non_hex_and_signed_unicode_escape_digits() {
    for document in [r#""\u+041""#, r#""\u-041""#, r#""\u0G00""#, r#""\u001""#] {
        assert!(super::super::parse(document).is_err());
    }
}

#[test]
fn decodes_surrogate_pair_extremes() {
    assert_eq!(
        super::super::parse(r#""\uD800\uDC00""#),
        Ok(super::super::Value::Text(std::string::String::from(
            "\u{10000}"
        )))
    );
    assert_eq!(
        super::super::parse(r#""\uDBFF\uDFFF""#),
        Ok(super::super::Value::Text(std::string::String::from(
            "\u{10ffff}"
        )))
    );
}

#[test]
fn rejects_malformed_isolated_and_reversed_surrogates() {
    for document in [
        r#""\uD800""#,
        r#""\uD800x""#,
        r#""\uD800\uD800""#,
        r#""\uD800\uDC0G""#,
        r#""\uDC00""#,
        r#""\uDC00\uD800""#,
        r#""\uD800\uD""#,
    ] {
        assert!(super::super::parse(document).is_err());
    }
}

#[test]
fn decodes_long_mixed_utf8_string_in_order() {
    let mut document = std::string::String::from("\"");
    let mut expected = std::string::String::new();
    for _ in 0..512 {
        document.push_str("aé");
        document.push_str(r#"\u96ea"#);
        document.push('😀');
        document.push_str(r#"\uD83D\uDE00"#);
        expected.push_str("aé雪😀😀");
    }
    document.push('"');
    assert_eq!(
        super::super::parse(&document),
        Ok(super::super::Value::Text(expected))
    );
}
