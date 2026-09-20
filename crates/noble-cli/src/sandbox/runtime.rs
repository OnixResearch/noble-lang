#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; executable resolves consumer-configured absolute paths and rejects missing or non-file tools with Unsupported. Host installation state is fallible input, not an assertion precondition."
)]
pub(super) fn executable(
    key: &str,
    candidates: &[&str],
) -> Result<std::path::PathBuf, crate::workflow::output::Failure> {
    let default = candidates
        .iter()
        .find(|candidate| std::path::Path::new(candidate).is_file());
    let fallback = default
        .copied()
        .unwrap_or("/nonexistent/noble-required-tool");
    let selected = super::configured_path(key, std::path::Path::new(fallback));
    if !selected.is_absolute() {
        return Err(crate::workflow::output::Failure::unsupported(
            "consumer-configuration",
            std::format!("{key} must select an absolute executable path"),
        ));
    }
    let canonical = attempt!(std::fs::canonicalize(&selected).map_err(|error| {
        crate::workflow::output::Failure::unsupported(
            "missing-tool",
            std::format!("cannot resolve {key} at {}: {error}", selected.display()),
        )
    }));
    if !canonical.is_file() {
        return Err(crate::workflow::output::Failure::unsupported(
            "missing-tool",
            std::format!("{key} does not identify an executable file"),
        ));
    }
    Ok(canonical)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; installation checks the selected executable's release-bin layout and lib/lean/Init.olean through Unsupported failures. Missing installation files must not panic."
)]
pub(super) fn installation(
) -> Result<(std::path::PathBuf, std::path::PathBuf), crate::workflow::output::Failure> {
    let lean = attempt!(executable(
        "NOBLE_LEAN",
        &[
            "/run/current-system/sw/bin/lean",
            "/usr/local/bin/lean",
            "/usr/bin/lean",
        ]
    ));
    let lean_root = attempt!(lean
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or_else(|| {
            crate::workflow::output::Failure::unsupported(
                "toolchain-layout",
                "Lean must be selected from its release bin directory".into(),
            )
        }))
    .to_path_buf();
    if !lean_root.join("lib/lean/Init.olean").is_file() {
        return Err(crate::workflow::output::Failure::unsupported(
            "toolchain-layout",
            "selected Lean installation has no lib/lean/Init.olean".into(),
        ));
    }
    Ok((lean, lean_root))
}

pub(super) fn roots(
    lean_root: &std::path::Path,
    deadline: std::time::Instant,
) -> Result<std::vec::Vec<std::path::PathBuf>, crate::workflow::output::Failure> {
    if lean_root.parent() == Some(std::path::Path::new("/nix/store")) {
        return nix_roots(lean_root, deadline);
    }
    let mut roots = std::vec::Vec::with_capacity(4);
    roots.push(lean_root.to_path_buf());
    ["/usr/lib", "/lib", "/lib64"]
        .into_iter()
        .for_each(|public| {
            let path = std::path::PathBuf::from(public);
            if path.is_dir() {
                roots.push(path);
            }
        });
    Ok(roots)
}

#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; nix_roots rejects failed queries, non-store directories, more than 128 roots and an absent Lean root through Unsupported. Subprocess output remains fallible even for consumer-selected tools."
)]
fn nix_roots(
    lean_root: &std::path::Path,
    deadline: std::time::Instant,
) -> Result<std::vec::Vec<std::path::PathBuf>, crate::workflow::output::Failure> {
    let nix_store = attempt!(executable(
        "NOBLE_NIX_STORE",
        &[
            "/run/current-system/sw/bin/nix-store",
            "/nix/var/nix/profiles/default/bin/nix-store",
            "/usr/bin/nix-store",
        ]
    ));
    let mut query = std::process::Command::new(nix_store);
    // The canonical executable is multicall nix; requisites require argv[0].
    #[cfg(unix)]
    std::os::unix::process::CommandExt::arg0(&mut query, "nix-store");
    query
        .env_clear()
        .args(["--query", "--requisites"])
        .arg(lean_root);
    let transcript = attempt!(super::process::capture(query, deadline));
    if !transcript.status.success() {
        return Err(crate::workflow::output::Failure::unsupported(
            "runtime-closure-unavailable",
            std::format!(
                "cannot resolve the selected Lean Nix closure: {}",
                transcript.stderr,
            ),
        ));
    }
    let mut roots = std::vec::Vec::with_capacity(128);
    attempt!(transcript.stdout.lines().try_for_each(|line| {
        let path = std::path::PathBuf::from(line);
        if path.parent() != Some(std::path::Path::new("/nix/store"))
            || !path.is_dir()
            || roots.len() >= 128
        {
            return Err(crate::workflow::output::Failure::unsupported(
                "runtime-closure-invalid",
                "invalid or oversized Lean runtime closure".into(),
            ));
        }
        roots.push(path);
        Ok::<(), crate::workflow::output::Failure>(())
    }));
    if !roots.iter().any(|root| root == lean_root) {
        return Err(crate::workflow::output::Failure::unsupported(
            "runtime-closure-invalid",
            "Lean installation is absent from its runtime closure".into(),
        ));
    }
    Ok(roots)
}
