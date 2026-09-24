//! Pinned Canonical ABI assembly. Tool success does not prove lowering correct.

pub(super) const WASM_TOOLS: &str =
    "/nix/store/2n9b5yzj7n8jlnglp60gd5nmfjnf4xhf-wasm-tools-1.245.1/bin/wasm-tools";
const ARTIFACT_LIMIT_BYTES: usize = 4_194_304;

pub(super) struct Bundle {
    pub(super) core: std::vec::Vec<u8>,
    pub(super) binary: std::vec::Vec<u8>,
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; pinned tool identity, supervised deadline, each assembler/validator exit and bounded output reads are checked fallibly before returning a complete bundle."
)]
pub(super) fn assemble(
    wit: &[u8],
    world: &str,
    wat: &[u8],
) -> Result<Bundle, crate::workflow::output::Failure> {
    let temporary = attempt!(crate::workflow::artifacts::Temporary::create());
    let deadline = attempt!(crate::sandbox::now()
        .checked_add(std::time::Duration::from_secs(30))
        .ok_or_else(|| crate::workflow::output::Failure::error(
            "component-tool-budget",
            "deadline overflow".into()
        )));
    let version = attempt!(invoke(&temporary.path, &["--version"], deadline));
    if version.stdout.trim() != "wasm-tools 1.245.1" {
        return Err(crate::workflow::output::Failure::unsupported(
            "component-tool-pin",
            "Component-Sync-Bootstrap requires wasm-tools 1.245.1".into(),
        ));
    }
    attempt!(crate::workflow::artifacts::write_source(
        &temporary.path.join("world.wit"),
        wit
    ));
    attempt!(crate::workflow::artifacts::write_source(
        &temporary.path.join("module.wat"),
        wat
    ));
    attempt!(invoke(
        &temporary.path,
        &["parse", "module.wat", "-o", "core.wasm"],
        deadline,
    ));
    attempt!(invoke(
        &temporary.path,
        &[
            "component",
            "embed",
            "--world",
            world,
            "--encoding",
            "utf8",
            "world.wit",
            "core.wasm",
            "-o",
            "embedded.wasm",
        ],
        deadline,
    ));
    attempt!(invoke(
        &temporary.path,
        &["component", "new", "embedded.wasm", "-o", "component.wasm"],
        deadline,
    ));
    attempt!(invoke(
        &temporary.path,
        &["validate", "component.wasm"],
        deadline,
    ));
    Ok(Bundle {
        core: attempt!(crate::workflow::artifacts::read_bounded(
            &temporary.path.join("embedded.wasm"),
            ARTIFACT_LIMIT_BYTES,
            "component-core-artifact",
        )),
        binary: attempt!(crate::workflow::artifacts::read_bounded(
            &temporary.path.join("component.wasm"),
            ARTIFACT_LIMIT_BYTES,
            "component-artifact",
        )),
    })
}

fn invoke(
    directory: &std::path::Path,
    arguments: &[&str],
    deadline: std::time::Instant,
) -> Result<crate::sandbox::Transcript, crate::workflow::output::Failure> {
    let mut command = std::process::Command::new(WASM_TOOLS);
    command.env_clear().current_dir(directory).args(arguments);
    let transcript = attempt!(crate::sandbox::process::capture(command, deadline));
    if !transcript.status.success() {
        return Err(crate::workflow::output::Failure::error(
            "component-assembly",
            transcript.stderr,
        ));
    }
    Ok(transcript)
}

pub(super) fn stage(
    destination: &std::path::Path,
) -> Result<crate::workflow::artifacts::Temporary, crate::workflow::output::Failure> {
    attempt!(std::fs::create_dir(destination).map_err(|error| {
        crate::workflow::output::Failure::error("component-destination", error.to_string())
    }));
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    crate::workflow::artifacts::Temporary::create_in(parent)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; every fresh artifact write targets the owned private staging directory and propagates its filesystem error before publication."
)]
pub(super) fn emit(
    staging: &crate::workflow::artifacts::Temporary,
    wit: &[u8],
    wat: &[u8],
    component: &Bundle,
    report: &crate::workflow::encoding::Json,
) -> Result<(), crate::workflow::output::Failure> {
    attempt!(crate::workflow::artifacts::write_source(
        &staging.path.join("world.wit"),
        wit
    ));
    attempt!(crate::workflow::artifacts::write_source(
        &staging.path.join("module.wat"),
        wat
    ));
    attempt!(crate::workflow::artifacts::write_source(
        &staging.path.join("core.wasm"),
        &component.core
    ));
    attempt!(crate::workflow::artifacts::write_source(
        &staging.path.join("component.wasm"),
        &component.binary
    ));
    crate::workflow::artifacts::write_source(
        &staging.path.join("report.json"),
        report.encode().as_bytes(),
    )
}
