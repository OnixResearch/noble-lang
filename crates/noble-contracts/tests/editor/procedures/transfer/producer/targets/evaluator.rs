use std::string::String;
use std::vec::Vec;

fn execute_words(program: &str, mut stack: Vec<i64>) -> Result<Vec<i64>, String> {
    let word_count = program.split_whitespace().count();
    if word_count == 0 || word_count > 8 {
        return Err(String::from(
            "producer target exceeds the declared word bound",
        ));
    }
    for word in program.split_whitespace() {
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
    Ok(stack)
}

pub(crate) fn evaluate_program(program: &str, inputs: [i64; 3]) -> Result<i64, String> {
    let mut stack = Vec::with_capacity(4);
    stack.extend_from_slice(&inputs);
    let stack = execute_words(program, stack)?;
    match stack.as_slice() {
        [result] => Ok(*result),
        _ => Err(String::from(
            "producer target does not leave exactly one I64",
        )),
    }
}

pub(crate) fn evaluate_composed(programs: [&str; 2], inputs: [i64; 3]) -> Result<i64, String> {
    let mut stack = Vec::with_capacity(4);
    stack.extend_from_slice(&inputs);
    let stack = execute_words(programs[0], stack)?;
    if stack.len() != 2 {
        return Err(String::from("first source does not leave a composable stack"));
    }
    let stack = execute_words(programs[1], stack)?;
    match stack.as_slice() {
        [result] => Ok(*result),
        _ => Err(String::from("second source does not leave a single I64")),
    }
}
