use std::string::String;
use std::vec::Vec;

pub(crate) fn evaluate_program(program: &str, inputs: [i64; 3]) -> Result<i64, String> {
    let words = program.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() || words.len() > 8 {
        return Err(String::from(
            "producer target exceeds the declared word bound",
        ));
    }
    let mut stack = Vec::with_capacity(4);
    stack.extend_from_slice(&inputs);
    for word in words {
        match word {
            "dup" => {
                let top = *stack.last().ok_or_else(|| String::from("dup underflow"))?;
                stack.push(top);
            }
            "swap" => {
                let stack_items = stack.len();
                if stack_items < 2 {
                    return Err(String::from("swap underflow"));
                }
                stack.swap(stack_items.saturating_sub(2), stack_items.saturating_sub(1));
            }
            "+" | "-" | "*" => {
                let right = stack
                    .pop()
                    .ok_or_else(|| String::from("operator underflow"))?;
                let left = stack
                    .pop()
                    .ok_or_else(|| String::from("operator underflow"))?;
                let result = match word {
                    "+" => left.wrapping_add(right),
                    "-" => left.wrapping_sub(right),
                    "*" => left.wrapping_mul(right),
                    _ => return Err(String::from("unrecognized producer arithmetic word")),
                };
                stack.push(result);
            }
            "=" => return Err(String::from("Boolean target is outside the I64 criterion")),
            literal => {
                let value = literal
                    .parse::<i64>()
                    .map_err(|error| format!("unknown producer word {literal}: {error}"))?;
                if ![0, 1, 2, 3, 5, 7].contains(&value) {
                    return Err(format!(
                        "producer constant {literal} is outside the grammar"
                    ));
                }
                stack.push(value);
            }
        }
        if stack.len() > 4 {
            return Err(String::from("producer target exceeds max-stack 4"));
        }
    }
    match stack.as_slice() {
        [result] => Ok(*result),
        _ => Err(String::from(
            "producer target does not leave exactly one I64",
        )),
    }
}
