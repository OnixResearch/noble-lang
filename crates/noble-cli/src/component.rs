//! Bounded Component Model compiler shell; execution belongs to the linked host.
mod artifacts;
mod authorized_fs;
mod bounded_region;
mod callback_owner;
mod identity;
mod quota;
mod report;

pub(crate) const USAGE: &str = "usage:
  noble component bindings WIT WORLD
  noble component check-effect WIT WORLD WORD CLAIMED_EFFECT ...
  noble component compile WIT WORLD NEW_DIR EXPORT=SOURCE ...
  noble component compile-checked-u64 WIT WORLD NEW_DIR EXPORT=SOURCE ...
  noble component read-region COMPONENT HOST_BUFFER_HEX OFFSET LENGTH
  noble component callback-owner COMPONENT MODE WIT WORLD EXPORT=SOURCE ...
  noble component read-fs COMPONENT HOST_PREOPENED_FILE READ_RIGHT WIT WORLD EXPORT=SOURCE ...
  noble component quota-core CORE_WASM EXPORT FUEL ALLOCATION_BYTES WIT WORLD EXPORT=SOURCE ...

Component-Sync-Bootstrap and Component-Async-Bootstrap generate typed WIT
bindings and independently check all export bodies before emitting a component.
Every selected world export must be supplied exactly once. Exports use isolated
parameter/result stacks. Native async calls preserve sequential Noble order;
borrowed exports and async borrowing are rejected. NEW_DIR must not exist.
read-region links the separate noble-test:bounded-region/bounded@1.0.0 world.
The invoker selects at most 4096 immutable bytes as hex; the component gets
one owned region and signed relative offset/length, never a native address.
The report distinguishes bounds refusal, protected region reads and owner
release. Native host/engine correspondence and general proof remain open.
callback-owner independently rebuilds and byte-matches its selected WIT,
world and complete Noble export-source recipe before guest invocation. The
host selects the callback claim (authentic, retired generation, live wrong
kind, or live wrong context); retained resource-table validation must precede
publication of the imported owned token to the compiled guest.
quota-core rebuilds the complete Noble WIT/world/export-source recipe and
requires byte-for-byte matching Core Wasm before accepting a private guest
allocator diagnostic. It runs the matched import-free Core with host-selected
Wasmtime fuel and the separate generated-heap byte limit.
Only a matching engine fuel trap or generated allocator quota diagnostic is
reported as a specified quota failure; other traps remain distinct.
Host resource, task-retirement and authorization contracts remain the
responsibility of the linked host profile.";

pub(crate) struct Source {
    name: std::string::String,
    bytes: std::vec::Vec<u8>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; all component refusals become structured diagnostics and a failing exit code; caller arguments and tool failures must not trigger assertion panics."
)]
pub(crate) fn run(arguments: &[std::ffi::OsString]) -> std::process::ExitCode {
    if arguments.get(1).is_some_and(|argument| argument == "read-region") {
        return bounded_region::run(arguments);
    }
    if arguments.get(1).is_some_and(|argument| argument == "callback-owner") {
        return callback_owner::run(arguments);
    }
    if arguments.get(1).is_some_and(|argument| argument == "read-fs") {
        return authorized_fs::run(arguments);
    }
    if arguments.get(1).is_some_and(|argument| argument == "quota-core") {
        return quota::run(arguments);
    }
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
    let world = attempt!(if mode == "compile-checked-u64" {
        noble_contracts::component::World::parse_checked_u64(
            &wit, selected_world, crate::core::SOURCE_LIMITS)
    } else {
        noble_contracts::component::World::parse(
            &wit, selected_world, crate::core::SOURCE_LIMITS)
    }.map_err(report::diagnostic));
    match mode {
        "bindings" if arguments.len() == 4 => report::bindings(&world),
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
        "compile" | "compile-checked-u64" if arguments.len() >= 5 && arguments.len() <= 69 => {
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
    let component = attempt!(artifacts::assemble(world, artifact.wat()));
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
