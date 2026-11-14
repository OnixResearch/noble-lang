const LEAN_EXPECTATIONS: &str =
    include_str!("../../../../../proofs/foundation/NobleFoundation/CheckedI64Expectations.lean");

type LeanCase = (String, String, String, String);
type ParseResult<T> = Result<T, Box<dyn std::error::Error>>;

fn invalid_data(message: &str) -> Box<dyn std::error::Error> {
    Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        message.to_owned(),
    ))
}

fn lean_string(field: &str) -> Result<String, Box<dyn std::error::Error>> {
    field
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_owned)
        .ok_or_else(|| invalid_data("Lean operation or expected value is not quoted"))
}

pub(super) fn lean_cases() -> ParseResult<Vec<LeanCase>> {
    let table = LEAN_EXPECTATIONS
        .split_once("def retained")
        .and_then(|(_, tail)| tail.split_once('['))
        .and_then(|(_, tail)| tail.split_once(']'))
        .map(|(table, _)| table)
        .ok_or_else(|| invalid_data("Lean retained table is missing"))?;
    let mut cases = Vec::new();
    for line in table.lines() {
        let row = line.trim().trim_end_matches(',').trim();
        if row.is_empty() {
            continue;
        }
        let tuple = row
            .strip_prefix('(')
            .and_then(|row| row.strip_suffix(')'))
            .ok_or_else(|| invalid_data("Lean case row is not a tuple"))?;
        let fields: Vec<_> = tuple.split(',').map(str::trim).collect();
        if fields.len() != 4 {
            return Err(invalid_data("Lean case row does not have four fields"));
        }
        cases.push((
            lean_string(fields[0])?,
            fields[1].to_owned(),
            fields[2].to_owned(),
            lean_string(fields[3])?,
        ));
    }
    Ok(cases)
}
