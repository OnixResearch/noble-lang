#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![register_tool(octet)]
//! Internal extraction smoke route, not a Noble language evaluator.

// This private parser error is closed. New cases require an explicit diagnostic.
#[octet::sealed_enum]
enum InputError {
    Usage,
    Budget,
}

// r[impl VT-M1-02]
fn main() -> std::process::ExitCode {
    match parse_budget(read_arguments().skip(1)) {
        Ok(budget) => {
            write_outcome(noble_kernel::consume_budget(budget));
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", input_error_message(error));
            std::process::ExitCode::from(2)
        }
    }
}

// This is the only process-argument observation in Noble-owned production code.
#[allow(
    tigerstyle::ambient_env,
    reason = "Owner: noble-maintainers. The CLI composition root captures process arguments before decoding."
)]
fn read_arguments() -> std::env::ArgsOs {
    std::env::args_os()
}

#[allow(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers. Pinned rustc rejects const Iterator and OsString decoding with E0277."
)]
fn parse_budget(mut args: impl Iterator<Item = std::ffi::OsString>) -> Result<u32, InputError> {
    let route = args.next();
    let input = args.next();
    if route.as_deref() != Some(std::ffi::OsStr::new("--internal-budget-smoke"))
        || args.next().is_some()
    {
        return Err(InputError::Usage);
    }
    input
        .as_deref()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|text| text.parse::<u32>().ok())
        .ok_or(InputError::Budget)
}

fn write_outcome(outcome: noble_kernel::BudgetOutcome) {
    match outcome {
        noble_kernel::BudgetOutcome::Remaining(next) => println!("remaining:{next}"),
        noble_kernel::BudgetOutcome::Exhausted => println!("exhausted"),
    }
}

const fn input_error_message(error: InputError) -> &'static str {
    match error {
        InputError::Usage => "usage: noble-cli --internal-budget-smoke <u32>",
        InputError::Budget => "budget must be an unsigned 32-bit integer",
    }
}
