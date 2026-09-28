//! Core-Bootstrap compilation retains its concrete input and independent WAT path.

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; legacy compilation rejects nominal inputs and returns source, independent-acceptance and stdout failures through Failure. User source and host streams are fallible boundaries, not assertion preconditions."
)]
pub(super) fn compile(
    options: &super::super::super::arguments::Options,
    source: &[u8],
) -> Result<u8, super::super::super::output::Failure> {
    let mut inputs = std::vec::Vec::with_capacity(options.inputs.len());
    let mut input_index = 0usize;
    while let Some(input) = options.inputs.get(input_index) {
        match input {
            super::super::super::arguments::InputType::Concrete(ty) => inputs.push(ty.clone()),
            super::super::super::arguments::InputType::Nominal(_) => {
                return Err(super::super::super::output::Failure::new(
                    super::super::super::output::ErrorContext {
                        stage: "arguments",
                        outcome: "invalid-input",
                    },
                    "nominal input requires Declared-Modules-v1",
                ))
            }
        }
        input_index += 1;
    }
    let frontend = noble_contracts::source::Session::new();
    let prepared = attempt!(frontend
        .prepare(source, &inputs, options.limits)
        .map_err(super::super::super::output::Failure::source));
    let submission = attempt!(prepared.submission().ok_or_else(|| {
        super::super::super::output::Failure::new(
            super::super::super::output::ErrorContext {
                stage: "check",
                outcome: "unsupported",
            },
            "compile requires an expression submission",
        )
    }));
    let compiled = attempt!(noble_wasm::source::Compiler::new()
        .prepare(submission)
        .map_err(super::super::super::output::Failure::backend));
    attempt!(
        std::io::Write::write_all(&mut std::io::stdout().lock(), compiled.wat())
            .map_err(super::super::super::framing::io_error)
    );
    Ok(0)
}
