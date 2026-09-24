#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![register_tool(octet)]
//! Consumer-owned contract verification and the internal extraction smoke route.

// Match Result explicitly: the pinned collector cannot resolve `?` desugaring.
// From preserves the conversion performed by the ordinary Result operator.
macro_rules! attempt {
    ($step:expr) => {
        match $step {
            Ok(value) => value,
            Err(error) => return Err(::core::convert::From::from(error)),
        }
    };
}

mod backend;
mod build;
mod component;
mod core;
mod sandbox;
mod workflow;

// This private parser error is closed. New cases require an explicit diagnostic.
#[octet::sealed_enum]
enum InputError {
    Usage,
    Budget,
}

// r[impl VT-M1-02]
#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; CLI routing accepts arbitrary OS argument bytes and reports usage or budget failures as exit statuses. Assertions would turn malformed user input into panics."
)]
fn main() -> std::process::ExitCode {
    let arguments: std::vec::Vec<std::ffi::OsString> = read_arguments().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "component")
    {
        return component::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "run" || argument == "session" || argument == "compile")
    {
        return core::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "wasm-experiment")
    {
        return backend::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "verify" || argument == "explain-proof")
    {
        return workflow::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "build")
    {
        return build::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "companions")
    {
        return core::companions::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "--help")
    {
        println!(
            "{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}",
            core::USAGE,
            workflow::USAGE,
            build::USAGE,
            core::companions::USAGE,
            backend::USAGE,
            component::USAGE
        );
        return std::process::ExitCode::SUCCESS;
    }
    match parse_budget(arguments.into_iter()) {
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
        InputError::Usage => "usage: noble --internal-budget-smoke <u32>",
        InputError::Budget => "budget must be an unsigned 32-bit integer",
    }
}
