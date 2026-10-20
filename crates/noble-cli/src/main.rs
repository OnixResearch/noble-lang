#![feature(register_tool)]
#![register_tool(tigerstyle)]
#![expect(
    tigerstyle::non_trait_imports,
    reason = "Owner: noble-maintainers; the composition root imports its own modules and domain types directly for readability"
)]
#![expect(
    tigerstyle::bool_naming,
    reason = "Owner: noble-maintainers; existing CLI state flags retain their source names until the calculation and live-slot APIs are renamed"
)]
#![expect(
    tigerstyle::excessive_file_length,
    reason = "Owner: noble-maintainers; the CLI entry point keeps its published command routes in one review unit"
)]
#![expect(
    tigerstyle::function_length,
    reason = "Owner: noble-maintainers; command routes remain linear so their argument and refusal order is reviewable"
)]
#![expect(
    tigerstyle::platform_dependent_cast,
    reason = "Owner: noble-maintainers; supported targets have usize >= 32 bits and the flagged casts are host-side u32 indices"
)]
#![expect(
    tigerstyle::ignored_result,
    reason = "Owner: noble-maintainers; diagnostic and report writes target caller-owned String/String buffers whose fmt::Write is infallible"
)]
#![expect(
    tigerstyle::unbounded_collection_growth,
    reason = "Owner: noble-maintainers; CLI report and trace vectors are bounded by the selected command's input rows or nodes"
)]
#![expect(
    tigerstyle::ambient_clock,
    reason = "Owner: noble-maintainers; the CLI measures and reports elapsed wall time at the composition-root boundary"
)]
#![expect(
    tigerstyle::ambient_env,
    reason = "Owner: noble-maintainers; the CLI reads host policy environment only at its explicit composition-root boundary"
)]
#![expect(
    tigerstyle::explicit_defaults,
    reason = "Owner: noble-maintainers; defaulted report and policy fields are intentionally explicit at their construction sites"
)]
#![expect(
    tigerstyle::path_segment_repetition,
    reason = "Owner: noble-maintainers; module paths name commands and item names retain their public operation spelling"
)]
#![expect(
    tigerstyle::underscore_in_module_filename,
    reason = "Owner: noble-maintainers; existing command module filenames are part of the selected source layout"
)]
#![expect(
    tigerstyle::too_many_parameters,
    reason = "Owner: noble-maintainers; command and harness signatures mirror their public argument sets"
)]
#![expect(
    tigerstyle::no_unwrap,
    reason = "Owner: noble-maintainers; CLI lookups are guarded by prior validation and retain explicit panic diagnostics"
)]
#![expect(
    tigerstyle::catch_all_on_enum,
    reason = "Owner: noble-maintainers; closed CLI dispatch matches retain their documented default diagnostic"
)]
#![expect(
    tigerstyle::fallible_int_fallback,
    reason = "Owner: noble-maintainers; command counters are clamped to the selected report width"
)]
#![expect(
    tigerstyle::sentinel_fallback,
    reason = "Owner: noble-maintainers; CLI option markers use an explicit absent sentinel outside the guest ABI"
)]
#![expect(
    tigerstyle::nested_conditionals,
    reason = "Owner: noble-maintainers; command preflight keeps related argument and policy checks together"
)]
#![expect(
    tigerstyle::renamed_imports,
    reason = "Owner: noble-maintainers; selected imports retain their short domain spelling"
)]
#![expect(
    tigerstyle::no_recursion,
    reason = "Owner: noble-maintainers; selected command walks are bounded by validated input depth"
)]
#![expect(
    tigerstyle::module_file_count,
    reason = "Owner: noble-maintainers; the CLI module layout follows its published command groups"
)]
#![expect(
    tigerstyle::contradictory_time,
    reason = "Owner: noble-maintainers; the CLI records both monotonic elapsed time and the host wall-clock receipt"
)]
#![expect(
    tigerstyle::no_panic,
    reason = "Owner: noble-maintainers; the selected invariants are checked before the diagnostic panic at the CLI boundary"
)]
#![expect(
    tigerstyle::drop_must_use,
    reason = "Owner: noble-maintainers; the ignored results are report/Drop finalizers whose failure is surfaced separately"
)]
#![expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; CLI arithmetic works on decoded bounded counts and report offsets already checked against the selected artifact, and overflow is refused before publication"
)]
#![expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; command routes and report builders assert the published argument and artifact invariants, whose ordered checks are the review subject"
)]
#![expect(
    tigerstyle::foreign_enum_catch_all,
    reason = "Owner: noble-maintainers; the flagged fallbacks are on Option/Result refusal paths that must preserve the existing diagnostic"
)]
#![expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-maintainers; the flagged parameter names are the published argument and artifact vocabulary, and every call site fixes their order"
)]
#![expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; helper constness is not part of the CLI contract, and the flagged helpers read runtime tables or build diagnostics"
)]
#![expect(
    tigerstyle::numeric_units,
    reason = "Owner: noble-maintainers; the flagged limits are abstract budget and work counts named by their selected budget kind, not time or byte quantities"
)]
#![expect(
    tigerstyle::compound_condition,
    reason = "Owner: noble-maintainers; command and report guards keep their full refusal predicate in one condition so the published refusal order stays reviewable"
)]
#![expect(
    tigerstyle::fragile_exhaustive_enum_match,
    reason = "Owner: noble-maintainers; the CLI matches the closed kernel and artifact vocabularies, so a new variant must force every route to change"
)]
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
mod calculator;
mod component;
mod core;
mod ngraph;
mod safety_observe;
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
    let _observation = safety_observe::begin();
    let arguments: std::vec::Vec<std::ffi::OsString> = read_arguments().skip(1).collect();
    if arguments
        .first()
        .is_some_and(|argument| argument == "--internal-calculator-wat-assemble")
    {
        return calculator::assemble_wat(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "--internal-calculator-wat-version")
    {
        return calculator::assembler_version(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "calculator")
    {
        return calculator::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "component")
    {
        return component::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "ngraph")
    {
        return ngraph::run(&arguments);
    }
    if arguments.first().is_some_and(|argument| argument == "live")
        && arguments.get(1).is_some_and(|argument| argument == "slot")
    {
        return core::live_slot::run(&arguments);
    }
    if arguments.first().is_some_and(|argument| argument == "live") {
        return core::live::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "run" || argument == "session" || argument == "compile")
    {
        return core::entry::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "admit-artifact")
    {
        return core::artifact::run(&arguments);
    }
    if arguments
        .first()
        .is_some_and(|argument| argument == "editor")
    {
        return core::editor::run(&arguments);
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
        .is_some_and(|argument| argument == "verify-module")
    {
        return workflow::intrinsic::run(&arguments);
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
            "{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}\n\n{}",
            core::USAGE,
            workflow::USAGE,
            build::USAGE,
            core::companions::USAGE,
            backend::USAGE,
            component::USAGE,
            calculator::USAGE,
            ngraph::USAGE
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
