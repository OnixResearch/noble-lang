#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; numeric scanning consumes present bytes and delegates syntax/range rejection to fallible integer/float parsing; invalid numbers return errors, not panic."
)]
pub(super) fn parse(scan: &mut super::Scan<'_>) -> Result<crate::value::Json, String> {
    let start = scan.at;
    if scan.text.get(scan.at) == Some(&b'-') {
        scan.at += 1;
    }
    let mut is_float = false;
    while let Some(byte) = scan.text.get(scan.at) {
        match byte {
            b'0'..=b'9' => scan.at += 1,
            b'.' | b'e' | b'E' | b'+' | b'-' => {
                is_float = true;
                scan.at += 1;
            }
            _ => break,
        }
    }
    let text = std::str::from_utf8(&scan.text[start..scan.at]).map_err(|_| "bad number")?;
    if is_float {
        text.parse::<f64>()
            .map(crate::value::Json::Float)
            .map_err(|_| format!("invalid float `{text}`"))
    } else {
        text.parse::<i64>()
            .map(crate::value::Json::Int)
            .map_err(|_| format!("invalid integer `{text}`"))
    }
}
