#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; read_bounded rejects non-files, I/O failures and content beyond the caller's byte budget before and after reading. Files may change after metadata observation, so these are typed input failures rather than assertions."
)]
pub(crate) fn read_bounded(
    path: &std::path::Path,
    limit_bytes: usize,
    kind: &'static str,
) -> Result<std::vec::Vec<u8>, super::output::Failure> {
    let metadata = attempt!(std::fs::metadata(path).map_err(
        |error| super::output::Failure::error(kind, std::format!("{}: {error}", path.display()))
    ));
    if !metadata.is_file() {
        return Err(super::output::Failure::error(
            kind,
            std::format!("{} is not a regular file", path.display()),
        ));
    }
    let read_limit_bytes = attempt!(u64::try_from(limit_bytes)
        .map_err(|error| super::output::Failure::error(kind, error.to_string())));
    if metadata.len() > read_limit_bytes {
        return Err(super::output::Failure::error(
            "input-byte-limit",
            std::format!("{kind} exceeds {limit_bytes} bytes"),
        ));
    }
    let file = attempt!(std::fs::File::open(path)
        .map_err(|error| super::output::Failure::error(kind, error.to_string())));
    let mut reader = std::io::Read::take(file, read_limit_bytes.saturating_add(1));
    let mut bytes = std::vec::Vec::new();
    attempt!(std::io::Read::read_to_end(&mut reader, &mut bytes)
        .map_err(|error| super::output::Failure::error(kind, error.to_string())));
    if bytes.len() > limit_bytes {
        return Err(super::output::Failure::error(
            "input-byte-limit",
            std::format!("{kind} exceeds {limit_bytes} bytes"),
        ));
    }
    Ok(bytes)
}

pub(crate) fn create_directory(path: &std::path::Path) -> Result<(), super::output::Failure> {
    std::fs::create_dir_all(path)
        .map_err(|error| super::output::Failure::error("artifact-io", error.to_string()))
}

pub(crate) fn write_source(
    path: &std::path::Path,
    bytes: &[u8],
) -> Result<(), super::output::Failure> {
    if let Some(parent) = path.parent() {
        attempt!(create_directory(parent));
    }
    let mut file = attempt!(std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| super::output::Failure::error(
            "artifact-io",
            std::format!("{}: {error}", path.display())
        )));
    std::io::Write::write_all(&mut file, bytes)
        .map_err(|error| super::output::Failure::error("artifact-io", error.to_string()))
}

pub(super) fn read_mount(path: &std::path::Path, guest: &str) -> crate::sandbox::Mount {
    crate::sandbox::Mount {
        host: path.to_path_buf(),
        guest: guest.into(),
        writable: false,
    }
}

pub(super) fn guest_path(
    root: &str,
    relative: &std::path::Path,
) -> Result<std::string::String, super::output::Failure> {
    let text = attempt!(relative
        .to_str()
        .ok_or_else(|| super::output::Failure::error(
            "artifact-path",
            "non-UTF8 generated path".into()
        )));
    Ok(std::format!("{root}/{text}"))
}

pub(super) fn output_slots(
    root: &std::path::Path,
    output: &std::path::Path,
) -> Result<std::vec::Vec<std::path::PathBuf>, super::output::Failure> {
    let mut slots = std::vec::Vec::with_capacity(4);
    slots.push(output.to_path_buf());
    slots.push(output.with_extension("ir"));
    [".server", ".private"].into_iter().for_each(|suffix| {
        let mut name = output.as_os_str().to_os_string();
        name.push(suffix);
        slots.push(std::path::PathBuf::from(name));
    });
    attempt!(slots
        .iter()
        .try_for_each(|slot| write_source(&root.join(slot), &[])));
    Ok(slots)
}

pub(super) fn output_mounts(
    mut mounts: std::vec::Vec<crate::sandbox::Mount>,
    root: &std::path::Path,
    slots: &[std::path::PathBuf],
    guest: &str,
) -> Result<std::vec::Vec<crate::sandbox::Mount>, super::output::Failure> {
    mounts.reserve(slots.len());
    attempt!(slots.iter().try_for_each(|slot| {
        mounts.push(crate::sandbox::Mount {
            host: root.join(slot),
            guest: attempt!(guest_path(guest, slot)),
            writable: true,
        });
        Ok::<(), super::output::Failure>(())
    }));
    Ok(mounts)
}

pub(super) fn finish_outputs(
    root: &std::path::Path,
    main: &std::path::Path,
    slots: &[std::path::PathBuf],
) -> Result<(), super::output::Failure> {
    slots.iter().try_for_each(|slot| {
        let path = root.join(slot);
        let metadata = attempt!(std::fs::symlink_metadata(&path)
            .map_err(|error| super::output::Failure::error("proof-artifact", error.to_string())));
        if !metadata.is_file() || metadata.len() > crate::sandbox::ARTIFACT_LIMIT {
            return Err(super::output::Failure::error(
                "proof-artifact",
                "invalid or oversized Lean artifact".into(),
            ));
        }
        if metadata.len() != 0 {
            return Ok(());
        }
        if slot == main {
            return Err(super::output::Failure::error(
                "proof-artifact",
                "Lean exited without producing its object file".into(),
            ));
        }
        std::fs::remove_file(path)
            .map_err(|error| super::output::Failure::error("artifact-io", error.to_string()))
    })
}

pub(crate) struct Temporary {
    pub(crate) path: std::path::PathBuf,
}

impl Temporary {
    #[allow(
        tigerstyle::ambient_env,
        tigerstyle::ambient_clock,
        reason = "Owner: noble-maintainers. The shell allocates a private unique disposable proof workspace."
    )]
    pub(crate) fn create() -> Result<Self, super::output::Failure> {
        let timestamp = attempt!(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| super::output::Failure::error("temporary-root", error.to_string())))
        .as_nanos();
        let parent = attempt!(std::fs::canonicalize(std::env::temp_dir())
            .map_err(|error| super::output::Failure::error("temporary-root", error.to_string())));
        let mut attempt = 0;
        while attempt < 128 {
            let path = parent.join(std::format!(
                "noble-mc1-{}-{timestamp}-{attempt}",
                std::process::id()
            ));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
            match builder.create(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => attempt += 1,
                Err(error) => {
                    return Err(super::output::Failure::error(
                        "temporary-root",
                        error.to_string(),
                    ))
                }
            }
        }
        Err(super::output::Failure::error(
            "temporary-root",
            "could not allocate a fresh temporary directory".into(),
        ))
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.path) {
            // Best-effort private-workspace cleanup cannot replace the already
            // determined proof result, including while unwinding another error.
            drop(error);
        }
    }
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; emit creates a fresh destination and writes only report-present artifacts with create_new. Existing destinations and partial filesystem failures must be reported as Failure, not asserted away."
)]
pub(super) fn emit(
    path: &std::path::Path,
    report: &mut super::output::Report,
    proof: Option<&str>,
) -> Result<(), super::output::Failure> {
    attempt!(std::fs::create_dir(path).map_err(|error| {
        super::output::Failure::error(
            "emit-destination",
            std::format!("new output directory {}: {error}", path.display()),
        )
    }));
    if let Some(source) = &report.source {
        attempt!(write_source(
            &path.join("contract.noble"),
            source.as_bytes()
        ));
    }
    if let Some(generated) = &report.generated {
        attempt!(write_source(
            &path.join("MC1Obligation.lean"),
            generated.as_bytes()
        ));
    }
    if let Some(proof) = proof {
        attempt!(write_source(&path.join("MC1Proof.lean"), proof.as_bytes()));
    }
    if let Some(wire) = &report.proof_wire {
        attempt!(write_source(&path.join("MC1Proof.json"), wire.as_bytes()));
    }
    report.emitted = Some(path.to_string_lossy().into_owned());
    let mut encoded = report.json().encode();
    encoded.push('\n');
    write_source(&path.join("report.json"), encoded.as_bytes())
}
