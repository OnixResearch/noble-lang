use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::{One, Pow, Signed, Zero};
use serde_json::{json, Value};
use std::fs;

const POLICY_VERSION: &str = "noble-woxi-policy/1";
const REQUEST_SCHEMA: &str = "noble-woxi-request/v1";
const RESPONSE_SCHEMA: &str = "noble-woxi-response/v1";
const MAX_EXPONENT: u32 = 1 << 20;
const MAX_SCALE: u32 = 1 << 20;
const WOXI_VERSION: &str = "0.3.0";

#[derive(Debug)]
enum StepError {
    Diagnostic(&'static str, &'static str),
    Timeout(String),
    Oracle(String),
}

fn exact(n: BigInt, d: BigInt) -> Result<(BigInt, BigInt), StepError> {
    if d <= BigInt::zero() {
        return Err(StepError::Oracle(format!("non-positive denominator {d}")));
    }
    if n.gcd(&d) != BigInt::one() {
        return Err(StepError::Oracle(format!("non-normalized rational {n}/{d}")));
    }
    Ok((n, d))
}

fn operand(value: &(BigInt, BigInt)) -> String {
    if value.1.is_one() {
        value.0.to_string()
    } else {
        format!("Rational[{}, {}]", value.0, value.1)
    }
}

fn parse_exact(text: &str) -> Result<(BigInt, BigInt), StepError> {
    let text = text.trim();
    let (n, d) = match text.split_once('/') {
        Some((n, d)) => (n.trim(), d.trim()),
        None => (text, "1"),
    };
    let numerator = BigInt::parse_bytes(n.as_bytes(), 10)
        .ok_or_else(|| StepError::Oracle(format!("non-integer result {text:?}")))?;
    let denominator = BigInt::parse_bytes(d.as_bytes(), 10)
        .ok_or_else(|| StepError::Oracle(format!("non-integer denominator {text:?}")))?;
    exact(numerator, denominator)
}

fn woxi(expression: &str) -> Result<String, StepError> {
    woxi::interpret_with_stdout(expression)
        .map(|result| result.result)
        .map_err(|error| StepError::Oracle(format!("woxi: {error}")))
}

fn step(expression: &str) -> Result<(BigInt, BigInt), StepError> {
    parse_exact(&woxi(expression)?)
}

fn literal(digits: &str, scale: u32) -> Result<(BigInt, BigInt), StepError> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(StepError::Oracle("literal digits must be ASCII decimal".into()));
    }
    let numerator = BigInt::parse_bytes(digits.as_bytes(), 10)
        .ok_or_else(|| StepError::Oracle("literal digits do not parse".into()))?;
    let denominator = BigInt::from(10u32).pow(scale);
    step(&format!("Rational[{}, {}]", numerator, denominator))
}

fn evaluate(node: &Value) -> Result<(BigInt, BigInt), StepError> {
    let op = node
        .get("op")
        .and_then(Value::as_str)
        .ok_or_else(|| StepError::Oracle("operation missing".into()))?;
    match op {
        "lit" => {
            let digits = node
                .get("digits")
                .and_then(Value::as_str)
                .ok_or_else(|| StepError::Oracle("literal digits missing".into()))?;
            let scale = node
                .get("scale")
                .and_then(Value::as_u64)
                .ok_or_else(|| StepError::Oracle("literal scale missing".into()))?;
            let scale = u32::try_from(scale)
                .map_err(|_| StepError::Oracle("literal scale is too large".into()))?;
            if scale > MAX_SCALE {
                return Err(StepError::Timeout(format!("literal scale {scale} exceeds bound")));
            }
            literal(digits, scale)
        }
        "neg" => {
            let x = evaluate(node.get("x").ok_or_else(|| StepError::Oracle("neg operand missing".into()))?)?;
            step(&format!("Times[-1, {}]", operand(&x)))
        }
        "add" | "sub" | "mul" | "div" => {
            let x = evaluate(node.get("x").ok_or_else(|| StepError::Oracle("left operand missing".into()))?)?;
            let y = evaluate(node.get("y").ok_or_else(|| StepError::Oracle("right operand missing".into()))?)?;
            if op == "div" && y.0.is_zero() {
                return Err(StepError::Diagnostic("DivisionByZero", "zero-divisor"));
            }
            let head = match op {
                "add" => "Plus",
                "sub" => "Subtract",
                "mul" => "Times",
                _ => "Divide",
            };
            step(&format!("{head}[{}, {}]", operand(&x), operand(&y)))
        }
        "pow" => {
            let x = evaluate(node.get("x").ok_or_else(|| StepError::Oracle("base missing".into()))?)?;
            let y = evaluate(node.get("y").ok_or_else(|| StepError::Oracle("exponent missing".into()))?)?;
            if !y.1.is_one() {
                return Err(StepError::Diagnostic("NonIntegerExponent", "nonintegral-exponent"));
            }
            if y.0.is_zero() {
                return Ok((BigInt::one(), BigInt::one()));
            }
            if x.0.is_zero() && y.0.is_negative() {
                return Err(StepError::Diagnostic("DivisionByZero", "zero-base-negative-exponent"));
            }
            let exponent = if y.0.is_negative() {
                -&y.0
            } else {
                y.0.clone()
            };
            let exponent_u32 = u32::try_from(&exponent).ok();
            match exponent_u32 {
                Some(value) if value <= MAX_EXPONENT => {}
                _ => {
                    return Err(StepError::Timeout(format!(
                        "exponent {exponent} exceeds bound {MAX_EXPONENT}"
                    )))
                }
            }
            step(&format!("Power[{}, {}]", operand(&x), y.0))
        }
        other => Err(StepError::Oracle(format!("operation {other:?} is outside the subset"))),
    }
}

fn reference(case: &Value) -> Value {
    match case
        .get("interchange")
        .ok_or_else(|| StepError::Oracle("interchange missing".into()))
        .and_then(evaluate)
    {
        Ok((n, d)) => json!({"kind": "value", "n": n.to_string(), "d": d.to_string()}),
        Err(StepError::Diagnostic(category, rule)) => {
            json!({"kind": "diagnostic", "category": category, "rule": rule})
        }
        Err(StepError::Timeout(detail)) => json!({"kind": "oracle-timeout", "detail": detail}),
        Err(StepError::Oracle(detail)) => json!({"kind": "oracle-error", "detail": detail}),
    }
}

fn probes() -> Vec<Value> {
    let expected = [
        ("zero-power-zero", "0^0", "Indeterminate"),
        ("one-over-zero", "1/0", "ComplexInfinity"),
        ("zero-over-zero", "0/0", "Indeterminate"),
        ("zero-power-negative", "0^-1", "ComplexInfinity"),
        ("perfect-square-half-power", "4^(1/2)", "2"),
        ("irrational-half-power", "2^(1/2)", "Sqrt[2]"),
    ];
    expected
        .iter()
        .map(|(id, expression, expected_raw)| {
            let raw = woxi(expression).unwrap_or_else(|error| format!("oracle-error: {error:?}"));
            json!({
                "id": id,
                "expression": expression,
                "raw_result": raw,
                "expected_raw": expected_raw,
                "ok": raw.trim() == *expected_raw,
            })
        })
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: noble-woxi-bridge REQUEST.json RESPONSE.json");
        std::process::exit(4);
    }
    let request: Value = match fs::read_to_string(&args[1])
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
    {
        Some(value) => value,
        None => {
            eprintln!("noble-woxi-bridge: unreadable request");
            std::process::exit(4);
        }
    };
    let mut response = json!({
        "schema": RESPONSE_SCHEMA,
        "policy_version": POLICY_VERSION,
        "arithmetic": request.get("arithmetic").cloned().unwrap_or(Value::Null),
        "environment": {
            "implementation": "woxi",
            "woxi_version": WOXI_VERSION,
            "helper_version": env!("CARGO_PKG_VERSION"),
            "rust": option_env!("RUSTC_VERSION").unwrap_or("unknown"),
        },
    });
    if request.get("schema").and_then(Value::as_str) != Some(REQUEST_SCHEMA) {
        response["status"] = json!("oracle-error");
        response["reason"] = json!(format!("request schema must be {REQUEST_SCHEMA}"));
        response["results"] = json!([]);
    } else if request.get("policy_version").and_then(Value::as_str) != Some(POLICY_VERSION) {
        response["status"] = json!("oracle-error");
        response["reason"] = json!(format!("request policy version mismatch"));
        response["results"] = json!([]);
    } else {
        let probe_results = probes();
        let changed: Vec<&str> = probe_results
            .iter()
            .filter(|probe| probe["ok"] != json!(true))
            .filter_map(|probe| probe["id"].as_str())
            .collect();
        let results: Vec<Value> = request
            .get("cases")
            .and_then(Value::as_array)
            .map(|cases| {
                cases
                    .iter()
                    .map(|case| {
                        json!({
                            "id": case.get("id").cloned().unwrap_or(Value::Null),
                            "reference": reference(case),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        response["probes"] = json!(probe_results);
        response["results"] = json!(results);
        if changed.is_empty() {
            response["status"] = json!("ok");
        } else {
            response["status"] = json!("oracle-error");
            response["reason"] = json!(format!("policy probes changed: {}", changed.join(", ")));
        }
    }
    let encoded = serde_json::to_string_pretty(&response).expect("response encodes");
    if fs::write(&args[2], format!("{encoded}\n")).is_err() {
        eprintln!("noble-woxi-bridge: cannot write response");
        std::process::exit(4);
    }
    let status = response["status"].as_str().unwrap_or("oracle-error");
    if let Some(reason) = response["reason"].as_str() {
        eprintln!("{status}: {reason}");
    } else {
        eprintln!("{status}");
    }
    std::process::exit(match status {
        "ok" => 0,
        "oracle-unavailable" => 3,
        _ => 4,
    });
}
