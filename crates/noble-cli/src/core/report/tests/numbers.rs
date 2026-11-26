#[test]
fn number_and_string_roundtrip_boundaries() -> Result<(), String> {
    (|| -> Result<(), String> {
        for (document, signed, unsigned) in [
            ("-9223372036854775808", Some(i64::MIN), None),
            ("-1", Some(-1), None),
            ("-0", Some(0), Some(0)),
            ("0", Some(0), Some(0)),
            ("9223372036854775807", Some(i64::MAX), Some(i64::MAX as u64)),
            ("9223372036854775808", None, Some(1_u64 << 63)),
            ("18446744073709551615", None, Some(u64::MAX)),
        ] {
            let value = super::super::parse(document)?;
            assert_eq!(value.number(), signed, "signed field: {document}");
            assert_eq!(value.index(), unsigned, "unsigned field: {document}");
        }
        for document in ["-9223372036854775809", "18446744073709551616"] {
            assert!(super::super::parse(document).is_err(), "{document}");
        }
        Ok(())
    })()?;
    (|| -> Result<(), String> {
        for value in [0, i64::MAX as u64, 1_u64 << 63, u64::MAX] {
            let encoded = crate::workflow::encoding::Json::Number(value).encode();
            let decoded = super::super::parse(&encoded)?;
            assert_eq!(decoded.index(), Some(value));
            assert_eq!(decoded.number(), i64::try_from(value).ok());
            let independent: serde_json::Value =
                serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
            assert_eq!(independent.as_u64(), Some(value));
        }
        Ok(())
    })()?;
    (|| -> Result<(), String> {
        let controls: String = (0_u8..32).map(char::from).collect();
        for value in [
            controls.as_str(),
            "quote\" slash/ backslash\\",
            "é中😀\u{10ffff}",
        ] {
            let encoded = crate::workflow::encoding::string(value).encode();
            let decoded = super::super::parse(&encoded)?;
            assert_eq!(decoded.text(), Some(value));
            let independent: String =
                serde_json::from_str(&encoded).map_err(|error| error.to_string())?;
            assert_eq!(independent, value);
        }
        Ok(())
    })()?;
    Ok(())
}
