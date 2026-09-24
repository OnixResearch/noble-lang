//! Component-Sync-Bootstrap compiler shell; execution belongs to the linked host.
mod artifacts;
mod report;

pub(crate) const USAGE: &str = "usage:
  noble component bindings WIT WORLD
  noble component check-effect WIT WORLD WORD CLAIMED_EFFECT ...
  noble component compile WIT WORLD NEW_DIR EXPORT=SOURCE ...

Component-Sync-Bootstrap generates typed WIT bindings and independently checks
all export bodies before emitting a component. Every selected world export must
be supplied exactly once. Exports use isolated parameter/result stacks; async
and borrowed exports are rejected. NEW_DIR must not exist. Host resource and
authorization contracts remain the responsibility of the linked host profile.";

pub(crate) struct Source {
    name: std::string::String,
    bytes: std::vec::Vec<u8>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; all component refusals become structured diagnostics and a failing exit code; caller arguments and tool failures must not trigger assertion panics."
)]
pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    let (document, exit) = match execute(arguments) {
        Ok(document) => (document, 0),
        Err(error) => {
            let exit = error.outcome.exit();
            (
                crate::workflow::encoding::object([
                    (
                        "schema",
                        crate::workflow::encoding::string("noble-component/v1"),
                    ),
                    (
                        "outcome",
                        crate::workflow::encoding::string(error.outcome.name()),
                    ),
                    ("diagnostic", error.json()),
                    (
                        "component_emitted",
                        crate::workflow::encoding::Json::Bool(false),
                    ),
                ]),
                exit,
            )
        }
    };
    println!("{}", document.encode());
    std::process::ExitCode::from(exit)
}

#[expect(
    tigerstyle::missing_const_fn,
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; command dispatch performs bounded filesystem reads, fallible WIT parsing and typed effect or compilation checks; it allocates reports and propagates refusals instead of asserting on CLI input."
)]
fn execute(
    arguments: &[std::ffi::OsString],
) -> Result<crate::workflow::encoding::Json, crate::workflow::output::Failure> {
    let mode = attempt!(text(arguments, 1));
    let wit_path = attempt!(arguments.get(2).ok_or_else(usage));
    let selected_world = attempt!(text(arguments, 3));
    let wit = attempt!(crate::workflow::read_bounded(
        std::path::Path::new(wit_path),
        65_536,
        "component-wit-input",
    ));
    let world = attempt!(noble_contracts::component::World::parse(
        &wit,
        selected_world,
        crate::core::SOURCE_LIMITS
    )
    .map_err(report::diagnostic));
    match mode {
        "bindings" if arguments.len() == 4 => Ok(report::bindings(&world)),
        "check-effect" if arguments.len() >= 5 && arguments.len() <= 69 => {
            let word = attempt!(text(arguments, 4));
            let claimed = attempt!(arguments[5..]
                .iter()
                .map(|argument| argument.to_str().ok_or_else(usage))
                .collect::<Result<std::vec::Vec<_>, _>>());
            attempt!(world
                .check_import_effect(word, &claimed)
                .map_err(report::diagnostic));
            Ok(crate::workflow::encoding::object([
                (
                    "schema",
                    crate::workflow::encoding::string("noble-component/v1"),
                ),
                ("outcome", crate::workflow::encoding::string("accepted")),
                (
                    "component_emitted",
                    crate::workflow::encoding::Json::Bool(false),
                ),
                ("guest_requests", crate::workflow::encoding::Json::Number(0)),
            ]))
        }
        "compile" if arguments.len() >= 5 && arguments.len() <= 69 => {
            let output = std::path::Path::new(attempt!(arguments.get(4).ok_or_else(usage)));
            let sources = attempt!(sources(&arguments[5..]));
            compile(output, &world, &sources)
        }
        _ => Err(usage()),
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; all exports undergo frontend and independent backend acceptance before the pinned assembler and atomic bundle publication; every preparation, tool, conversion, write and rename failure is propagated."
)]
fn compile(
    output: &std::path::Path,
    world: &noble_contracts::component::World,
    sources: &[Source],
) -> Result<crate::workflow::encoding::Json, crate::workflow::output::Failure> {
    let exports = attempt!(sources
        .iter()
        .map(|source| {
            world
                .prepare_export(&source.name, &source.bytes, crate::core::SOURCE_LIMITS)
                .map_err(report::diagnostic)
        })
        .collect::<Result<std::vec::Vec<_>, _>>());
    let artifact =
        attempt!(noble_wasm::component::compile(world, &exports).map_err(report::backend));
    let component = attempt!(artifacts::assemble(
        world.wit(),
        world.name(),
        artifact.wat()
    ));
    let document = attempt!(report::compiled(
        world, &artifact, sources, &component, output
    ));
    let staging = attempt!(artifacts::stage(output));
    attempt!(artifacts::emit(
        &staging,
        world.wit(),
        artifact.wat(),
        &component,
        &document
    ));
    attempt!(sources.iter().enumerate().try_for_each(|(index, source)| {
        crate::workflow::artifacts::write_source(
            &staging.path.join(std::format!("export-{index}.noble")),
            &source.bytes,
        )
    }));
    attempt!(std::fs::rename(&staging.path, output).map_err(|error| {
        crate::workflow::output::Failure::error("component-destination", error.to_string())
    }));
    Ok(document)
}

fn sources(
    arguments: &[std::ffi::OsString],
) -> Result<std::vec::Vec<Source>, crate::workflow::output::Failure> {
    arguments
        .iter()
        .map(|argument| {
            let text = attempt!(argument.to_str().ok_or_else(usage));
            let (name, path) = attempt!(text.split_once('=').ok_or_else(usage));
            if name.is_empty() || path.is_empty() {
                return Err(usage());
            }
            let bytes = attempt!(crate::workflow::read_bounded(
                std::path::Path::new(path),
                65_536,
                "component-source-input",
            ));
            Ok(Source {
                name: name.into(),
                bytes,
            })
        })
        .collect()
}

fn text(
    arguments: &[std::ffi::OsString],
    index: usize,
) -> Result<&str, crate::workflow::output::Failure> {
    arguments
        .get(index)
        .and_then(|argument| argument.to_str())
        .ok_or_else(usage)
}

fn usage() -> crate::workflow::output::Failure {
    crate::workflow::output::Failure::error("component-arguments", USAGE.into())
}
