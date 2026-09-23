#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; the library path budget is four times the fixed 128-module budget. Const evaluation checks this multiplication for overflow if MODULE_LIMIT changes."
)]
const PATH_LIMIT: usize = super::MODULE_LIMIT * 4;

#[expect(
    tigerstyle::raw_arithmetic_overflow,
    reason = "Owner: noble-maintainers; directory scanning observes one entry beyond the 512-path budget to reject overflow. Const evaluation checks this sentinel addition if PATH_LIMIT changes."
)]
const DIRECTORY_SCAN_LIMIT: usize = PATH_LIMIT + 1;

pub(super) struct Module {
    pub(super) name: std::string::String,
    pub(super) relative: std::path::PathBuf,
    pub(super) source: std::string::String,
    pub(super) imports: std::vec::Vec<std::string::String>,
}

pub(crate) struct Library {
    root: std::path::PathBuf,
    pub(super) modules: std::vec::Vec<Module>,
}

impl Library {
    pub(super) fn json(&self) -> super::encoding::Json {
        super::encoding::object([
            ("root", super::encoding::string(self.root.to_string_lossy())),
            ("toolchain", super::encoding::string(crate::sandbox::PIN)),
            (
                "identity",
                super::encoding::string("exact-source-snapshot; rebuilt without producer caches"),
            ),
            (
                "sources",
                super::encoding::Json::Array(
                    self.modules
                        .iter()
                        .map(|module| {
                            super::encoding::object([
                                ("module", super::encoding::string(&module.name)),
                                ("source", super::encoding::string(&module.source)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; read_library enforces finite path, module and aggregate source-byte budgets and rejects symlinks or inaccessible inputs through Failure. Consumer-selected filesystem contents are not assertion preconditions."
)]
pub(super) fn read_library() -> Result<Library, super::output::Failure> {
    let root = attempt!(selected_root());
    let mut pending = std::vec::Vec::with_capacity(PATH_LIMIT);
    pending.push(std::path::PathBuf::from("NobleContracts.lean"));
    if root.join("NobleContracts").is_dir() {
        pending.push(std::path::PathBuf::from("NobleContracts"));
    }
    let mut modules = std::vec::Vec::with_capacity(super::MODULE_LIMIT);
    let mut bytes = 0_usize;
    let mut visits = 0_usize;
    while let Some(relative) = pending.pop() {
        visits += 1;
        if visits > PATH_LIMIT {
            return Err(path_limit());
        }
        let path = root.join(&relative);
        let metadata = attempt!(std::fs::symlink_metadata(&path).map_err(|error| {
            super::output::Failure::unsupported(
                "library-unavailable",
                std::format!("{}: {error}", path.display()),
            )
        }));
        if metadata.file_type().is_symlink() {
            return Err(super::output::Failure::unsupported(
                "library-symlink",
                "library source symlinks are not permitted".into(),
            ));
        }
        if metadata.is_dir() {
            attempt!(enqueue_directory(&path, &relative, &mut pending, visits));
            continue;
        }
        if relative.extension().and_then(std::ffi::OsStr::to_str) != Some("lean") {
            continue;
        }
        if modules.len() >= super::MODULE_LIMIT {
            return Err(super::output::Failure::unsupported(
                "library-limit",
                "too many rule-library modules".into(),
            ));
        }
        let module = attempt!(read_module(
            &path,
            relative,
            super::LIBRARY_LIMIT.saturating_sub(bytes)
        ));
        bytes = bytes.saturating_add(module.source.len());
        modules.push(module);
    }
    modules.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(Library { root, modules })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; selected_root rejects relative or unresolved library paths and a mismatched byte-bounded toolchain pin with Unsupported. Configuration and filesystem errors must remain typed failures."
)]
#[expect(
    tigerstyle::missing_const_fn,
    reason = "Owner: noble-maintainers; selected_root observes NOBLE_CONTRACT_LIBRARY, canonicalizes a host path and reads lean-toolchain. These runtime environment and filesystem observations cannot be const; reassess only if responsibility moves out of this helper."
)]
fn selected_root() -> Result<std::path::PathBuf, super::output::Failure> {
    let default = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proofs/mc1");
    let selected = crate::sandbox::configured_path("NOBLE_CONTRACT_LIBRARY", &default);
    if !selected.is_absolute() {
        return Err(super::output::Failure::unsupported(
            "consumer-configuration",
            "NOBLE_CONTRACT_LIBRARY must be absolute".into(),
        ));
    }
    let root = attempt!(std::fs::canonicalize(&selected).map_err(|error| {
        super::output::Failure::unsupported(
            "library-unavailable",
            std::format!("{}: {error}", selected.display()),
        )
    }));
    let pin = attempt!(super::artifacts::read_bounded(
        &root.join("lean-toolchain"),
        256,
        "library-toolchain"
    ));
    if std::str::from_utf8(&pin).map(str::trim).ok() != Some(crate::sandbox::PIN) {
        return Err(super::output::Failure::unsupported(
            "library-toolchain",
            "library does not select the pinned Lean release".into(),
        ));
    }
    Ok(root)
}

fn path_limit() -> super::output::Failure {
    super::output::Failure::unsupported(
        "library-limit",
        "library tree exceeds its finite path bound".into(),
    )
}

#[expect(
    tigerstyle::borrowed_argument_types,
    reason = "Owner: noble-maintainers. Enqueuing discovered paths within the shared finite visit budget requires the caller's growable Vec, not a fixed path slice."
)]
fn enqueue_directory(
    path: &std::path::Path,
    relative: &std::path::Path,
    pending: &mut std::vec::Vec<std::path::PathBuf>,
    visits: usize,
) -> Result<(), super::output::Failure> {
    let entries = attempt!(std::fs::read_dir(path)
        .map_err(|error| super::output::Failure::unsupported("library-read", error.to_string())));
    entries.take(DIRECTORY_SCAN_LIMIT).try_for_each(|entry| {
        let entry = attempt!(entry.map_err(|error| super::output::Failure::unsupported(
            "library-read",
            error.to_string()
        )));
        if pending.len().saturating_add(visits) >= PATH_LIMIT {
            return Err(path_limit());
        }
        pending.push(relative.join(entry.file_name()));
        Ok(())
    })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; read_module rejects files beyond the remaining library-byte budget, invalid UTF-8 and invalid module paths through Failure. Import tokens come only from that bounded source snapshot, not an asserted trusted input."
)]
fn read_module(
    path: &std::path::Path,
    relative: std::path::PathBuf,
    remaining_bytes: usize,
) -> Result<Module, super::output::Failure> {
    let source = attempt!(std::string::String::from_utf8(attempt!(
        super::artifacts::read_bounded(path, remaining_bytes, "library-source",)
    ))
    .map_err(|_| {
        super::output::Failure::unsupported(
            "library-encoding",
            "library source is not UTF-8".into(),
        )
    }));
    let name = attempt!(module_name(&relative));
    // Every import token is part of the already byte-bounded source snapshot.
    let imports = source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed
                .strip_prefix("import ")
                .or_else(|| trimmed.strip_prefix("public import "))
        })
        .flat_map(|line| {
            line.split_ascii_whitespace()
                .take_while(|imported| !imported.starts_with("--"))
                .map(str::to_owned)
        })
        .collect();
    Ok(Module {
        name,
        relative,
        source,
        imports,
    })
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; module_name accepts only nonempty ASCII identifier components and returns Unsupported for invalid or non-UTF8 paths. Filesystem-derived names must be validated, not asserted."
)]
fn module_name(path: &std::path::Path) -> Result<std::string::String, super::output::Failure> {
    let without_extension = path.with_extension("");
    let mut name = std::string::String::with_capacity(without_extension.as_os_str().len());
    attempt!(without_extension.components().try_for_each(|component| {
        let text = attempt!(component.as_os_str().to_str().ok_or_else(|| {
            super::output::Failure::unsupported(
                "library-module-name",
                "non-UTF8 module path".into(),
            )
        }));
        if text.is_empty()
            || !text
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(super::output::Failure::unsupported(
                "library-module-name",
                "module paths must contain ASCII identifiers".into(),
            ));
        }
        if !name.is_empty() {
            name.push('.');
        }
        name.push_str(text);
        Ok::<(), super::output::Failure>(())
    }));
    Ok(name)
}
