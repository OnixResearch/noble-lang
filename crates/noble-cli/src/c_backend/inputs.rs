//! Descriptor-anchored bounded inputs and outputs for the nonselected C
//! source build. Every host-selected path is walked from the filesystem root
//! one component at a time with `openat2` (`RESOLVE_NO_SYMLINKS`,
//! `RESOLVE_NO_MAGICLINKS`, `RESOLVE_BENEATH`) and `O_PATH | O_NOFOLLOW` on
//! trusted ancestors (`st_uid` equal to `euid` or `0`, and non-group/world-
//! writable unless protected by the sticky bit `0o1000`). Each input is
//! snapshotted into a sealed `memfd` retained across the build (`SealedInput`),
//! and the leaf descriptor is rewound and re-hashed with BLAKE3 after sealing
//! to detect in-place rewrites even when stat metadata is unchanged. Each
//! output directory is verified for `euid` ownership, non-writability, and
//! `RawDir` emptiness after `mkdirat` and written through that bound
//! descriptor. This is not native admission: same-UID `mkdirat` concurrency
//! and the admitted build/launch remain open under CB-ADMIT-01.

use std::mem::MaybeUninit;
use std::ops::Deref;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fd::OwnedFd;
use rustix::fs::{
    self, AtFlags, FileType, MemfdFlags, Mode, OFlags, RawDir, ResolveFlags, SealFlags, SeekFrom,
    Stat,
};
use rustix::io::{self, Errno};

const COMPONENTS_MAX: usize = 128;
const CHUNK: usize = 4096;

/// An immutable input snapshot backed by a sealed `memfd` retained for the
/// lifetime of the build.
pub(super) struct SealedInput {
    _memfd: OwnedFd,
    bytes: Vec<u8>,
}

impl Deref for SealedInput {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

/// Split an absolute path into its parent components and leaf, refusing
/// anything the descriptor walk cannot represent exactly.
fn components(path: &Path) -> Result<(Vec<String>, String), String> {
    let bytes = path.as_os_str().as_bytes();
    if bytes.first() != Some(&b'/') {
        return Err(format!(
            "{}: C build paths must be lexically absolute",
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
    let observed = fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW);
    let file_type = observed
        .as_ref()
        .map(|stat| FileType::from_raw_mode(stat.st_mode));
    if file_type.is_ok_and(|kind| kind.is_symlink()) || error == Errno::LOOP {
        format!(
            "{}: path component {name:?} is a symlink or magic link; C build inputs are opened without following symlinks",
            path.display()
        )
    } else if error == Errno::XDEV {
        format!(
            "{}: path component {name:?} escapes its trusted directory anchor",
            path.display()
        )
    } else if error == Errno::NOTDIR
        || file_type.is_ok_and(|kind| kind.is_file() || kind.is_symlink())
    {
        format!(
            "{}: path component {name:?} is not a directory",
            path.display()
        )
    } else if error == Errno::NOENT {
        format!("{}: path component {name:?} does not exist", path.display())
    } else {
        format!("{}: path component {name:?}: {error}", path.display())
    }
}

fn effective_uid() -> Result<u32, String> {
    let probe = fs::memfd_create("noble-euid", MemfdFlags::CLOEXEC)
        .map_err(|error| format!("effective uid probe: {error}"))?;
    let stat = fs::fstat(&probe).map_err(|error| format!("effective uid stat: {error}"))?;
    Ok(stat.st_uid)
}

fn verify_trusted_dir(
    dir: &OwnedFd,
    path: &Path,
    component: &str,
    euid: u32,
) -> Result<(), String> {
    let stat = fs::fstat(dir).map_err(|error| format!("{}: {error}", path.display()))?;
    if !FileType::from_raw_mode(stat.st_mode).is_dir() {
        return Err(format!(
            "{}: path component {component:?} is not a directory",
            path.display()
        ));
    }
    if stat.st_uid != euid && stat.st_uid != 0 {
        return Err(format!(
            "{}: ancestor {component:?} is owned by uid {}, not the current user or root",
            path.display(),
            stat.st_uid
        ));
    }
    if (stat.st_mode & 0o022) != 0 && (stat.st_mode & 0o1000) == 0 {
        return Err(format!(
            "{}: ancestor {component:?} is group- or world-writable without the sticky bit",
            path.display()
        ));
    }
    Ok(())
}

fn root(euid: u32) -> Result<OwnedFd, String> {
    let dir = fs::openat2(
        fs::CWD,
        "/",
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|error| format!("filesystem root: {error}"))?;
    verify_trusted_dir(&dir, Path::new("/"), "/", euid)?;
    Ok(dir)
}

fn open_dir(parent: &OwnedFd, name: &str, path: &Path, euid: u32) -> Result<OwnedFd, String> {
    let dir = fs::openat2(
        parent,
        name,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS | ResolveFlags::BENEATH,
    )
    .map_err(|error| component_error(parent, path, name, error))?;
    verify_trusted_dir(&dir, path, name, euid)?;
    Ok(dir)
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
        if bytes
            .len()
            .checked_add(count)
            .is_none_or(|total| total > max)
        {
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

/// Snapshot `bytes` into a sealed `memfd`, read the sealed copy back, and
/// retain the descriptor in `SealedInput` across the build.
fn seal(bytes: &[u8], path: &Path) -> Result<SealedInput, String> {
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
    Ok(SealedInput {
        _memfd: snapshot,
        bytes: sealed,
    })
}

fn same_identity(a: &Stat, b: &Stat) -> bool {
    a.st_dev == b.st_dev
        && a.st_ino == b.st_ino
        && a.st_nlink == b.st_nlink
        && a.st_size == b.st_size
}

fn same_timestamps(a: &Stat, b: &Stat) -> bool {
    a.st_mtime == b.st_mtime
        && a.st_mtime_nsec == b.st_mtime_nsec
        && a.st_ctime == b.st_ctime
        && a.st_ctime_nsec == b.st_ctime_nsec
}

fn verify_leaf_post_seal(
    dir: &OwnedFd,
    leaf: &str,
    file: &OwnedFd,
    opened: &Stat,
    sealed: &SealedInput,
    max: usize,
    path: &Path,
) -> Result<(), String> {
    let after_entry = fs::statat(dir, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        format!(
            "{}: input vanished during snapshot: {error}",
            path.display()
        )
    })?;
    let after_fd = fs::fstat(file).map_err(|error| format!("{}: {error}", path.display()))?;
    if !same_identity(opened, &after_entry) || !same_identity(opened, &after_fd) {
        return Err(format!(
            "{}: input was replaced during the snapshot",
            path.display()
        ));
    }
    fs::seek(file, SeekFrom::Start(0))
        .map_err(|error| format!("{}: input rewind during snapshot: {error}", path.display()))?;
    let recheck = read_bounded(file, max, path)?;
    if blake3::hash(&recheck) != blake3::hash(&sealed.bytes) || recheck != sealed.bytes {
        return Err(format!(
            "{}: input changed in place during the snapshot",
            path.display()
        ));
    }
    if !same_timestamps(opened, &after_entry) || !same_timestamps(opened, &after_fd) {
        return Err(format!(
            "{}: input was replaced during the snapshot",
            path.display()
        ));
    }
    Ok(())
}

/// Open one host-selected input by walking trusted ancestors from `/` without
/// following symlinks or magic links, refuse non-regular, oversized, unlinked,
/// or hard-linked files, snapshot into a retained sealed `memfd`, and re-verify
/// both leaf content digest and metadata after sealing.
pub(super) fn snapshot(path: &Path, max: usize) -> Result<SealedInput, String> {
    snapshot_with_hook(path, max, || Ok(()))
}

fn snapshot_with_hook<F>(path: &Path, max: usize, after_seal_hook: F) -> Result<SealedInput, String>
where
    F: FnOnce() -> Result<(), String>,
{
    let euid = effective_uid()?;
    let (parents, leaf) = components(path)?;
    let mut dir = root(euid)?;
    for name in &parents {
        dir = open_dir(&dir, name, path, euid)?;
    }
    let file = fs::openat2(
        &dir,
        leaf.as_str(),
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS | ResolveFlags::BENEATH,
    )
    .map_err(|error| component_error(&dir, path, &leaf, error))?;
    let opened = fs::fstat(&file).map_err(|error| format!("{}: {error}", path.display()))?;
    if !FileType::from_raw_mode(opened.st_mode).is_file() {
        return Err(format!("{} must be a bounded regular file", path.display()));
    }
    if opened.st_nlink == 0 {
        return Err(format!(
            "{}: unlinked input refused by the source-only rule",
            path.display()
        ));
    }
    if opened.st_nlink != 1 {
        return Err(format!(
            "{}: multiply-linked input refused by the source-only rule",
            path.display()
        ));
    }
    if opened.st_size < 0 || u64::try_from(opened.st_size).unwrap_or(u64::MAX) > max as u64 {
        return Err(format!("{} exceeds {max} bytes", path.display()));
    }
    let bytes = read_bounded(&file, max, path)?;
    if u64::try_from(opened.st_size).unwrap_or(u64::MAX) != bytes.len() as u64 {
        return Err(format!(
            "{}: input changed size while it was read",
            path.display()
        ));
    }
    let expected_digest = blake3::hash(&bytes);
    let sealed = seal(&bytes, path)?;
    if blake3::hash(&sealed) != expected_digest {
        return Err(format!(
            "{}: sealed snapshot digest mismatch",
            path.display()
        ));
    }
    after_seal_hook()?;
    verify_leaf_post_seal(&dir, &leaf, &file, &opened, &sealed, max, path)?;
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
        let file = fs::openat2(
            &self.dir,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o644),
            ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS | ResolveFlags::BENEATH,
        )
        .map_err(|error| format!("{name}: new output file: {error}"))?;
        write_all(&file, bytes).map_err(|error| format!("{name}: {error}"))
    }
}

fn verify_empty_dir(dir: &OwnedFd, path: &Path) -> Result<(), String> {
    let mut buf = [MaybeUninit::<u8>::uninit(); 4096];
    let mut iter = RawDir::new(dir, &mut buf);
    while let Some(entry) = iter.next() {
        let entry =
            entry.map_err(|error| format!("{}: directory check: {error}", path.display()))?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            return Err(format!(
                "{}: output directory was not empty during creation",
                path.display()
            ));
        }
    }
    Ok(())
}

/// Create one new output directory under trusted walked ancestors and bind its
/// `O_RDONLY` descriptor for `RawDir` emptiness inspection and `write_new`.
/// Cross-principal substitution is closed by ancestor trust plus reopened
/// ownership/mode/`RawDir`-emptiness checks; Linux has no atomic
/// `mkdirat`-returning-fd syscall against a concurrent same-UID racer.
pub(super) fn create_output(path: &Path) -> Result<OutputDir, String> {
    create_output_with_hook(path, || Ok(()))
}

fn create_output_with_hook<F>(path: &Path, after_mkdir_hook: F) -> Result<OutputDir, String>
where
    F: FnOnce() -> Result<(), String>,
{
    let euid = effective_uid()?;
    let (parents, leaf) = components(path)?;
    let mut dir = root(euid)?;
    for name in &parents {
        dir = open_dir(&dir, name, path, euid)?;
    }
    fs::mkdirat(&dir, leaf.as_str(), Mode::from_raw_mode(0o755)).map_err(|error| {
        if error == Errno::EXIST {
            format!("{}: new output directory already exists", path.display())
        } else {
            format!("{}: new output directory: {error}", path.display())
        }
    })?;
    after_mkdir_hook()?;
    let created = fs::openat2(
        &dir,
        leaf.as_str(),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS | ResolveFlags::BENEATH,
    )
    .map_err(|error| component_error(&dir, path, &leaf, error))?;
    let opened = fs::fstat(&created).map_err(|error| format!("{}: {error}", path.display()))?;
    let named = fs::statat(&dir, leaf.as_str(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| format!("{}: new output directory: {error}", path.display()))?;
    if !FileType::from_raw_mode(opened.st_mode).is_dir()
        || named.st_dev != opened.st_dev
        || named.st_ino != opened.st_ino
        || opened.st_uid != euid
        || (opened.st_mode & 0o022) != 0
    {
        return Err(format!(
            "{}: output directory was replaced during creation",
            path.display()
        ));
    }
    verify_empty_dir(&created, path)?;
    Ok(OutputDir { dir: created })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Seek as _, SeekFrom as StdSeekFrom, Write as _};
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::path::PathBuf;

    struct TestScratch(PathBuf);

    impl TestScratch {
        fn new() -> Result<Self, String> {
            let dir = std::env::temp_dir().join(format!(
                "noble-c-inputs-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos()
            ));
            std::fs::create_dir(&dir).map_err(|e| e.to_string())?;
            Ok(Self(dir))
        }
    }

    impl Drop for TestScratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn in_place_rewrite_refusal_and_read_bounded_limits() -> Result<(), String> {
        (|| -> Result<(), String> {
            let scratch = TestScratch::new()?;
            let source = scratch.0.join("input.noble");
            std::fs::write(&source, b"1").map_err(|e| e.to_string())?;

            let result = snapshot_with_hook(&source, 4096, || {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .open(&source)
                    .map_err(|e| e.to_string())?;
                file.seek(StdSeekFrom::Start(0))
                    .map_err(|e| e.to_string())?;
                file.write_all(b"2").map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
                Ok(())
            });

            let err = match result {
                Ok(_) => return Err("expected in-place rewrite to be refused".to_owned()),
                Err(err) => err,
            };
            if !err.contains("changed in place during the snapshot") {
                return Err(format!("unexpected error diagnostic: {err}"));
            }

            let sealed = snapshot(&source, 4096)?;
            let expected_seals =
                SealFlags::WRITE | SealFlags::SHRINK | SealFlags::GROW | SealFlags::SEAL;
            let observed = fs::fcntl_get_seals(&sealed._memfd).map_err(|e| e.to_string())?;
            if !observed.contains(expected_seals) {
                return Err("retained memfd lost expected seals".to_owned());
            }
            std::fs::write(&source, b"3").map_err(|e| e.to_string())?;
            if &*sealed != b"2" {
                return Err("retained SealedInput did not remain immutable".to_owned());
            }
            Ok(())
        })()?;
        (|| -> Result<(), String> {
            let fd = fs::memfd_create("noble-read-bounded-test", MemfdFlags::CLOEXEC)
                .map_err(|e| e.to_string())?;
            write_all(&fd, b"x")?;
            let path = Path::new("/memfd-test");

            fs::seek(&fd, SeekFrom::Start(0)).map_err(|e| e.to_string())?;
            let ok = read_bounded(&fd, usize::MAX, path)?;
            if ok != b"x" {
                return Err(format!("unexpected bytes read with max=usize::MAX: {ok:?}"));
            }

            fs::seek(&fd, SeekFrom::Start(0)).map_err(|e| e.to_string())?;
            let err = match read_bounded(&fd, 0, path) {
                Ok(_) => return Err("expected max=0 read_bounded to refuse".to_owned()),
                Err(err) => err,
            };
            if !err.contains("exceeds 0 bytes") {
                return Err(format!("unexpected max=0 diagnostic: {err}"));
            }

            Ok(())
        })()?;
        Ok(())
    }

    #[test]
    fn output_directory_substitution_between_mkdirat_and_reopen_is_refused() -> Result<(), String> {
        let scratch = TestScratch::new()?;

        let non_empty = scratch.0.join("non-empty-out");
        let non_empty_result = create_output_with_hook(&non_empty, || {
            std::fs::remove_dir(&non_empty).map_err(|e| e.to_string())?;
            std::fs::create_dir(&non_empty).map_err(|e| e.to_string())?;
            std::fs::write(non_empty.join("trojan"), b"evil").map_err(|e| e.to_string())?;
            Ok(())
        });
        if non_empty_result.is_ok() {
            return Err("expected non-empty substituted output dir to refuse".to_owned());
        }

        let writable = scratch.0.join("writable-out");
        let writable_result = create_output_with_hook(&writable, || {
            std::fs::remove_dir(&writable).map_err(|e| e.to_string())?;
            std::fs::create_dir(&writable).map_err(|e| e.to_string())?;
            std::fs::set_permissions(&writable, std::fs::Permissions::from_mode(0o777))
                .map_err(|e| e.to_string())?;
            Ok(())
        });
        if writable_result.is_ok() {
            return Err("expected 0o777 substituted output dir to refuse".to_owned());
        }

        let target = scratch.0.join("symlink-target");
        std::fs::create_dir(&target).map_err(|e| e.to_string())?;
        let sym = scratch.0.join("symlink-out");
        let sym_result = create_output_with_hook(&sym, || {
            std::fs::remove_dir(&sym).map_err(|e| e.to_string())?;
            symlink(&target, &sym).map_err(|e| e.to_string())?;
            Ok(())
        });
        if sym_result.is_ok() {
            return Err("expected symlink substituted output dir to refuse".to_owned());
        }

        Ok(())
    }
}
