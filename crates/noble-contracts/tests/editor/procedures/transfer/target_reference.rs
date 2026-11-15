use std::string::{String, ToString};
use std::vec::Vec;

pub(super) const MANIFEST: &str = "verification/procedure-library/producer-targets-v2.txt";

pub(super) fn manifest_targets() -> Result<[(String, String); 2], String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or_else(|| String::from("worktree root unavailable"))?;
    let text = std::fs::read_to_string(root.join(MANIFEST))
        .map_err(|error| format!("read frozen target manifest: {error}"))?;
    let mut targets = Vec::with_capacity(2);
    for line in text.lines().map(str::trim) {
        if let Some(declaration) = line.strip_prefix("task ") {
            let words = declaration.split_whitespace().collect::<Vec<_>>();
            if words.len() != 5 || words[1..] != ["arithmetic", "i64", "inputs=3", "bound=8"] {
                return Err(String::from("unexpected producer target declaration"));
            }
            targets.push((String::from(words[0]), String::new()));
        } else if let Some(reference) = line.strip_prefix("ref ") {
            let target = targets
                .last_mut()
                .ok_or_else(|| String::from("reference without a preceding target"))?;
            if !target.1.is_empty() || reference.is_empty() {
                return Err(String::from("missing or duplicate target reference"));
            }
            target.1 = reference.to_string();
        }
    }
    if targets.iter().any(|(_, reference)| reference.is_empty()) {
        return Err(String::from("target declaration lacks a reference"));
    }
    targets
        .try_into()
        .map_err(|_| String::from("frozen target count is not two"))
}

fn tokenize(reference: &str) -> Result<Vec<&str>, String> {
    if !reference.is_ascii() {
        return Err(String::from("reference grammar is ASCII only"));
    }
    let bytes = reference.as_bytes();
    let mut tokens = Vec::with_capacity(24);
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if bytes[cursor] == b'(' || bytes[cursor] == b')' {
            tokens.push(&reference[cursor..cursor + 1]);
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < bytes.len()
            && !bytes[cursor].is_ascii_whitespace()
            && bytes[cursor] != b'('
            && bytes[cursor] != b')'
        {
            cursor += 1;
        }
        tokens.push(&reference[start..cursor]);
    }
    Ok(tokens)
}

fn expression(tokens: &[&str], cursor: &mut usize, inputs: [i64; 3]) -> Result<i64, String> {
    let token = tokens
        .get(*cursor)
        .copied()
        .ok_or_else(|| String::from("reference ended before the expression"))?;
    *cursor += 1;
    if token == "(" {
        let operator = tokens
            .get(*cursor)
            .copied()
            .ok_or_else(|| String::from("reference lacks an operator"))?;
        *cursor += 1;
        let left = expression(tokens, cursor, inputs)?;
        let right = expression(tokens, cursor, inputs)?;
        if tokens.get(*cursor) != Some(&")") {
            return Err(String::from("reference lacks a closing parenthesis"));
        }
        *cursor += 1;
        return match operator {
            "+" => Ok(left.wrapping_add(right)),
            "-" => Ok(left.wrapping_sub(right)),
            "*" => Ok(left.wrapping_mul(right)),
            _ => Err(String::from("reference has an unsupported operator")),
        };
    }
    match token {
        "x" => Ok(inputs[0]),
        "y" => Ok(inputs[1]),
        "z" => Ok(inputs[2]),
        _ => token
            .parse::<i64>()
            .map_err(|error| format!("invalid reference constant {token}: {error}")),
    }
}

pub(super) fn evaluate(reference: &str, inputs: [i64; 3]) -> Result<i64, String> {
    let tokens = tokenize(reference)?;
    let mut cursor = 0;
    let value = expression(&tokens, &mut cursor, inputs)?;
    if cursor != tokens.len() {
        return Err(String::from("reference has trailing tokens"));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonheldout_reference_parser_is_bounded_and_well_formed() -> Result<(), String> {
        assert_eq!(evaluate("(+ x (* y z))", [8, 3, 4])?, 20);
        assert_eq!(evaluate("(- x z)", [i64::MIN, 0, 1])?, i64::MAX);
        for invalid in ["(+ x)", "(+ x z", "(= x y)", "(+ x z) 1", "((+ x y))"] {
            assert!(evaluate(invalid, [1, 2, 3]).is_err(), "accepted {invalid}");
        }
        Ok(())
    }
}
