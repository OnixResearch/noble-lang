use std::string::{String, ToString};
use std::vec::Vec;

pub(super) const MANIFEST: &str = "verification/procedure-library/producer-targets-v3.txt";

pub(super) fn manifest_targets() -> Result<[(String, String); 2], String> {
    manifest_targets_from(MANIFEST)
}

pub(super) fn manifest_targets_from(relative: &str) -> Result<[(String, String); 2], String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or_else(|| String::from("worktree root unavailable"))?;
    let text = std::fs::read_to_string(root.join(relative))
        .map_err(|error| format!("read frozen target manifest: {error}"))?;
    let mut targets = Vec::with_capacity(2);
    for line in text.lines().map(str::trim) {
        if let Some(declaration) = line.strip_prefix("task ") {
            let words = declaration.split_whitespace().collect::<Vec<_>>();
            if words.len() != 5 || words[1..] != ["arithmetic", "i64", "inputs=3", "bound=8"] {
                return Err(String::from("unexpected producer target declaration"));
            }
            targets.push((String::from(words[0]), None));
        } else if let Some(reference) = line.strip_prefix("ref ") {
            let target = targets
                .last_mut()
                .ok_or_else(|| String::from("reference without a preceding target"))?;
            if target.1.is_some() || reference.is_empty() {
                return Err(String::from("missing or duplicate target reference"));
            }
            target.1 = Some(reference.to_string());
        }
    }
    let [(first_name, first_reference), (second_name, second_reference)] = targets
        .try_into()
        .map_err(|_| String::from("frozen target count is not two"))?;
    Ok([
        (
            first_name,
            first_reference.ok_or_else(|| String::from("target declaration lacks a reference"))?,
        ),
        (
            second_name,
            second_reference.ok_or_else(|| String::from("target declaration lacks a reference"))?,
        ),
    ])
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
            tokens.push(&reference[cursor..cursor.saturating_add(1)]);
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

struct Frame<'a> {
    operator: &'a str,
    left: Option<i64>,
}

pub(super) fn evaluate(reference: &str, inputs: [i64; 3]) -> Result<i64, String> {
    let tokens = tokenize(reference)?;
    let mut frames = Vec::with_capacity(tokens.len() / 3);
    let mut cursor = 0;
    while let Some(token) = tokens.get(cursor).copied() {
        cursor = cursor.saturating_add(1);
        if token == "(" {
            let operator = tokens
                .get(cursor)
                .copied()
                .ok_or_else(|| String::from("reference lacks an operator"))?;
            cursor = cursor.saturating_add(1);
            if !matches!(operator, "+" | "-" | "*") {
                return Err(String::from("reference has an unsupported operator"));
            }
            frames.push(Frame {
                operator,
                left: None,
            });
            continue;
        }
        if token == ")" {
            return Err(String::from(
                "reference has an unexpected closing parenthesis",
            ));
        }
        let mut value = match token {
            "x" => inputs[0],
            "y" => inputs[1],
            "z" => inputs[2],
            _ => token
                .parse::<i64>()
                .map_err(|error| format!("invalid reference constant {token}: {error}"))?,
        };
        loop {
            let Some(frame) = frames.last_mut() else {
                if cursor != tokens.len() {
                    return Err(String::from("reference has trailing tokens"));
                }
                return Ok(value);
            };
            let Some(left) = frame.left.take() else {
                frame.left = Some(value);
                break;
            };
            let operator = frame.operator;
            frames.pop();
            if tokens.get(cursor) != Some(&")") {
                return Err(String::from("reference lacks a closing parenthesis"));
            }
            cursor = cursor.saturating_add(1);
            value = match operator {
                "+" => left.wrapping_add(value),
                "-" => left.wrapping_sub(value),
                "*" => left.wrapping_mul(value),
                _ => return Err(String::from("reference has an unsupported operator")),
            };
        }
    }
    Err(String::from("reference ended before the expression"))
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
