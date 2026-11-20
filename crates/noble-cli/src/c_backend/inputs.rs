//! Descriptor-anchored bounded inputs and outputs for the nonselected C
//! source build. Every host-selected path is walked from the filesystem root
//! one component at a time with `O_NOFOLLOW`, so no `.`/`..`, ancestor
//! symlink, magic link or non-directory component can redirect it. Each input
//! is snapshotted into a sealed memfd and used from that snapshot; each output
//! is written through the directory descriptor this build created, never by
//! re-resolving the output path. This is not native admission: trusted-anchor
//! selection, mutable hard-link review and the admitted build/launch remain
//! open under CB-ADMIT-01.

use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fd::OwnedFd;
use rustix::fs::{self, AtFlags, FileType, MemfdFlags, Mode, OFlags, SealFlags, SeekFrom};
use rustix::io::{self, Errno};

const COMPONENTS_MAX: usize = 128;
const CHUNK: usize = 4096;

/// Split an absolute path into its parent components and leaf, refusing
/// anything the descriptor walk cannot represent exactly.
fn components(path: &Path) -> Result<(Vec<String>, String), String> {
    let bytes = path.as_os_str().as_bytes();
    if bytes.first() != Some(&b'/') {
        return Err(format!(
            "{}: C build inputs must be lexically absolute",
            path.display()
        ));
    }
    let mut parts = Vec::new();
    for part in bytes[1..].split(|byte| *byte == b'/') {
        if part.is_empty() {
            return Err(format!(
                "{}: C build path components must be non-empty and normalized",
                path.display()
            ));
        }
        if part == b"." || part == b".." {
            return Err(format!(
                "{}: C build paths refuse `.` and `..` components",
                path.display()
            ));
        }
        let text = std::str::from_utf8(part)
            .map_err(|_| format!("{}: C build path components must be UTF-8", path.display()))?;
        parts.push(text.to_owned());
        if parts.len() > COMPONENTS_MAX {
            return Err(format!(
                "{}: C build path depth exceeds {COMPONENTS_MAX} components",
                path.display()
            ));
        }
    }
    let leaf = parts
        .pop()
        .ok_or_else(|| format!("{}: C build path names no component", path.display()))?;
    Ok((parts, leaf))
}

fn component_error(dir: &OwnedFd, path: &Path, name: &str, error: Errno) -> String {
    let is_symlink = fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW)
        .map(|stat| FileType::from_raw_mode(stat.st_mode).is_symlink())
        .unwrap_or(false);
    if is_symlink || error == Errno::LOOP {
        format!(
            "{}: path component {name:?} is a symlink; C build inputs are opened without following symlinks",
            path.display()
        )
    } else if error == Errno::NOTDIR {
        format!(
            "{}: path component {name:?} is not a directory",
            path.display()
        )
    } else {
        format!("{}: path component {name:?}: {error}", path.display())
    }
}

fn root() -> Result<OwnedFd, String> {
    fs::open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|error| format!("filesystem root: {error}"))
}

fn open_dir(parent: &OwnedFd, name: &str, path: &Path) -> Result<OwnedFd, String> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|error| component_error(parent, path, name, error))
}

/// Read at most `max` bytes from `fd`, refusing a larger stream rather than
/// truncating it.
fn read_bounded(fd: &OwnedFd, max: usize, path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; CHUNK];
    loop {
        let count = io::read(fd, &mut chunk)
            .map_err(|error| format!("{}: bounded read: {error}", path.display()))?;
        if count == 0 {
            return Ok(bytes);
        }
        if bytes.len() + count > max {
            return Err(format!("{} exceeds {max} bytes", path.display()));
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
}

fn write_all(fd: &OwnedFd, mut bytes: &[u8]) -> Result<(), String> {
    while !bytes.is_empty() {
        let count = io::write(fd, bytes).map_err(|error| error.to_string())?;
        if count == 0 {
            return Err("short write".to_owned());
        }
        bytes = &bytes[count..];
    }
    Ok(())
}

/// Snapshot `bytes` into a sealed memfd and read the snapshot back, so later
/// stages consume the sealed bytes rather than the mutable path.
fn seal(bytes: &[u8], path: &Path) -> Result<Vec<u8>, String> {
    let snapshot = fs::memfd_create(
        "noble-c-build-input",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map_err(|error| format!("{}: sealed snapshot: {error}", path.display()))?;
    write_all(&snapshot, bytes)
        .map_err(|error| format!("{}: sealed snapshot write: {error}", path.display()))?;
    let seals = SealFlags::WRITE | SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL;
    fs::fcntl_add_seals(&snapshot, seals)
        .map_err(|error| format!("{}: sealed snapshot seals: {error}", path.display()))?;
    let observed = fs::fcntl_get_seals(&snapshot)
        .map_err(|error| format!("{}: sealed snapshot seals: {error}", path.display()))?;
    if !observed.contains(seals) {
        return Err(format!(
            "{}: sealed snapshot does not retain every requested seal",
            path.display()
        ));
    }
    fs::seek(&snapshot, SeekFrom::Start(0))
        .map_err(|error| format!("{}: sealed snapshot rewind: {error}", path.display()))?;
    let sealed = read_bounded(&snapshot, bytes.len(), path)?;
    if sealed != bytes {
        return Err(format!(
            "{}: sealed snapshot differs from the bytes read",
            path.display()
        ));
    }
    Ok(sealed)
}

/// Open one host-selected input by walking from the filesystem root without
/// following any symlink, refuse a non-regular, oversized or hard-linked
/// file, and return the bytes of its sealed snapshot. A leaf replaced between
/// the open and the post-snapshot identity check is refused.
pub(super) fn snapshot(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let (parents, leaf) = components(path)?;
    let mut dir = root()?;
    for name in &parents {
        dir = open_dir(&dir, name, path)?;
    }
    let file = fs::openat(
        &dir,
        leaf.as_str(),
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|error| component_error(&dir, path, &leaf, error))?;
    let opened = fs::fstat(&file).map_err(|error| format!("{}: {error}", path.display()))?;
    if !FileType::from_raw_mode(opened.st_mode).is_file() {
        return Err(format!("{} must be a bounded regular file", path.display()));
    }
    if opened.st_nlink != 1 {
        return Err(format!(
            "{}: hard-linked mutable input refused",
            path.display()
        ));
    }
    if opened.st_size < 0 || u64::try_from(opened.st_size).unwrap_or(u64::MAX) > max as u64 {
        return Err(format!("{} exceeds {max} bytes", path.display()));
    }
    let bytes = read_bounded(&file, max, path)?;
    let sealed = seal(&bytes, path)?;
    let after = fs::statat(&dir, leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        format!(
            "{}: input vanished during snapshot: {error}",
            path.display()
        )
    })?;
    if after.st_dev != opened.st_dev || after.st_ino != opened.st_ino {
        return Err(format!(
            "{}: input was replaced during the snapshot",
            path.display()
        ));
    }
    Ok(sealed)
}

/// A new output directory bound to the descriptor this build created.
pub(super) struct OutputDir {
    dir: OwnedFd,
}

impl OutputDir {
    /// Create each `name` under the bound directory, refusing an existing
    /// entry and any symlink, and write exactly `bytes`.
    pub(super) fn write_new(&self, name: &str, bytes: &[u8]) -> Result<(), String> {
        if name.is_empty() || name.contains('/') || name == "." || name == ".." {
            return Err(format!(
                "{name:?}: output names must be one normal component"
            ));
        }
        let file = fs::openat(
            &self.dir,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o644),
        )
        .map_err(|error| format!("{name}: new output file: {error}"))?;
        write_all(&file, bytes).map_err(|error| format!("{name}: {error}"))
    }
}

/// Create one new output directory under the walked parent and bind its
/// descriptor. The directory is re-opened for reading and its identity is
/// compared with the leaf name, refusing a directory replaced during
/// creation.
pub(super) fn create_output(path: &Path) -> Result<OutputDir, String> {
    let (parents, leaf) = components(path)?;
    let mut dir = root()?;
    for name in &parents {
        dir = open_dir(&dir, name, path)?;
    }
    fs::mkdirat(&dir, leaf.as_str(), Mode::from_raw_mode(0o755)).map_err(|error| {
        if error == Errno::EXIST {
            format!("{}: new output directory already exists", path.display())
        } else {
            format!("{}: new output directory: {error}", path.display())
        }
    })?;
    let created = fs::openat(
        &dir,
        leaf.as_str(),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|error| component_error(&dir, path, &leaf, error))?;
    let opened = fs::fstat(&created).map_err(|error| format!("{}: {error}", path.display()))?;
    let named = fs::statat(&dir, leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| format!("{}: new output directory: {error}", path.display()))?;
    if !FileType::from_raw_mode(opened.st_mode).is_dir()
        || named.st_dev != opened.st_dev
        || named.st_ino != opened.st_ino
    {
        return Err(format!(
            "{}: output directory was replaced during creation",
            path.display()
        ));
    }
    Ok(OutputDir { dir: created })
}
