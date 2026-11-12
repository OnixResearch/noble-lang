//! Caller-selected build toolchain binding (`noble-mc2-build/v2`).
//!
//! The v1 report binds the Lean producer and consumer recheck through
//! [`super::Document::toolchain`](super::Document), but not the frozen Lean
//! binary or the contract library it compiled against. `--report-schema v2`
//! additionally binds both from the caller's exact paths: the binary's bytes
//! and every file of the library tree, sorted by relative path. [`select`]
//! then refuses that binding unless the bytes are the ones this build actually
//! uses — the configured Lean release (`NOBLE_LEAN` or the selected
//! installation) and the contract library root (`NOBLE_CONTRACT_LIBRARY` or
//! the pinned default) that proof checking and library compilation invoke.
//! Nothing is inferred from the Lean version string, and v1 reports never
//! carry these fields.

use std::io::Read;

/// The most library files one binding may cover; the contract library is far
/// smaller, and traversal refuses the next file instead of storing it.
const MAX_FILES: usize = 8192;

/// The most library directories one binding may cover.
const MAX_DIRECTORIES: usize = 2048;

/// The most bytes one binding may read from the library tree.
const MAX_BYTES: u64 = 1 << 31;

/// A selected Lean binary and contract library tree, bound by digest.
pub(crate) struct Selected {
    pub(super) lean_sha256: std::string::String,
    pub(super) lean_version: &'static str,
    pub(super) lean_commit: &'static str,
    pub(super) library_sha256: std::string::String,
    pub(super) library_files: u64,
    pub(super) library_bytes: u64,
}

impl Selected {
    pub(super) fn json(&self) -> crate::workflow::encoding::Json {
        crate::workflow::encoding::object([
            (
                "lean",
                crate::workflow::encoding::object([
                    (
                        "binary_sha256",
                        crate::workflow::encoding::string(&self.lean_sha256),
                    ),
                    (
                        "version",
                        crate::workflow::encoding::string(self.lean_version),
                    ),
                    (
                        "commit",
                        crate::workflow::encoding::string(self.lean_commit),
                    ),
                    (
                        "basis",
                        crate::workflow::encoding::string(
                            "SHA-256 of the exact --lean-binary bytes, refused unless they equal the configured Lean release this build invokes; version and commit are the selected sandbox pins, not read from the binary",
                        ),
                    ),
                ]),
            ),
            (
                "library_tree",
                crate::workflow::encoding::object([
                    (
                        "sha256",
                        crate::workflow::encoding::string(&self.library_sha256),
                    ),
                    (
                        "files",
                        crate::workflow::encoding::Json::Number(self.library_files),
                    ),
                    (
                        "bytes",
                        crate::workflow::encoding::Json::Number(self.library_bytes),
                    ),
                    (
                        "basis",
                        crate::workflow::encoding::string(
                            "BLAKE3 over each file's relative path length, path, content length and content, in sorted path order; refused unless it equals the configured contract library root this build compiles",
                        ),
                    ),
                ]),
            ),
        ])
    }
}

fn refused(message: std::string::String) -> crate::workflow::output::Failure {
    crate::workflow::output::Failure::unsupported("toolchain-binding", message)
}

fn read_bounded(
    path: &std::path::Path,
    limit: u64,
) -> Result<std::vec::Vec<u8>, crate::workflow::output::Failure> {
    debug_assert!(!path.as_os_str().is_empty());
    let file = std::fs::File::open(path)
        .map_err(|error| refused(std::format!("cannot open {}: {error}", path.display())))?;
    let length = file
        .metadata()
        .map_err(|error| refused(std::format!("cannot stat {}: {error}", path.display())))?
        .len();
    // Refuse before allocating: an oversized file costs no memory on the refusal path, which is what
    // the bound is for, not only what it checks.
    if length > limit {
        return Err(refused(std::format!(
            "{} is {length} bytes, over the remaining {limit}-byte budget",
            path.display()
        )));
    }
    let mut bytes = std::vec::Vec::new();
    // `take(limit + 1)` covers the file growing between the metadata check and the read.
    file.take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| refused(std::format!("cannot read {}: {error}", path.display())))?;
    if bytes.len() as u64 > limit {
        return Err(refused(std::format!(
            "{} grew past the remaining {limit}-byte budget while being read",
            path.display()
        )));
    }
    debug_assert!(bytes.len() as u64 <= limit);
    Ok(bytes)
}

/// Every regular file under `root`, as sorted `(relative, absolute)` pairs.
///
/// Both counts are enforced during traversal: the file or directory that would
/// exceed a bound is refused before it is stored, so a caller pointing at a
/// huge tree is refused rather than walked. Names must be UTF-8 so the digest
/// covers the exact bytes the binding reports instead of a lossy spelling that
/// distinct trees can share.
fn walk(
    root: &std::path::Path,
    file_limit: usize,
    directory_limit: usize,
) -> Result<
    std::vec::Vec<(std::string::String, std::path::PathBuf)>,
    crate::workflow::output::Failure,
> {
    debug_assert!(!root.as_os_str().is_empty());
    debug_assert!(file_limit != 0);
    let mut found = std::vec::Vec::new();
    let mut pending = std::vec![root.to_path_buf()];
    let mut directory_count = 1usize;
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory).map_err(|error| {
            refused(std::format!("cannot list {}: {error}", directory.display()))
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                refused(std::format!("cannot list {}: {error}", directory.display()))
            })?;
            let path = entry.path();
            let kind = entry.file_type().map_err(|error| {
                refused(std::format!("cannot stat {}: {error}", path.display()))
            })?;
            if kind.is_symlink() {
                return Err(refused(std::format!(
                    "library tree must not contain symlinks: {}",
                    path.display()
                )));
            }
            if kind.is_dir() {
                directory_count = directory_count.saturating_add(1);
                if directory_count > directory_limit {
                    return Err(refused(std::format!(
                        "library tree has more than {directory_limit} directories"
                    )));
                }
                pending.push(path);
                continue;
            }
            if !kind.is_file() {
                return Err(refused(std::format!(
                    "library tree must contain regular files only: {}",
                    path.display()
                )));
            }
            let Ok(stripped) = path.strip_prefix(root) else {
                return Err(refused(std::format!(
                    "library file escapes its root: {}",
                    path.display()
                )));
            };
            let Some(relative) = stripped.to_str() else {
                return Err(refused(std::format!(
                    "library tree file names must be UTF-8: {}",
                    path.display()
                )));
            };
            if relative.is_empty() {
                return Err(refused(std::format!(
                    "library file escapes its root: {}",
                    path.display()
                )));
            }
            if found.len() >= file_limit {
                return Err(refused(std::format!(
                    "library tree has more than {file_limit} files"
                )));
            }
            found.push((std::string::String::from(relative), path));
        }
    }
    found.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(found)
}

/// The BLAKE3 binding of one walked tree.
struct TreeDigest {
    hash: std::string::String,
    files: usize,
    bytes: u64,
}

fn tree_digest(
    files: &[(std::string::String, std::path::PathBuf)],
) -> Result<TreeDigest, crate::workflow::output::Failure> {
    debug_assert!(!files.is_empty());
    let mut hasher = blake3::Hasher::new();
    let mut total_bytes: u64 = 0;
    for (relative, path) in files {
        debug_assert!(!relative.is_empty());
        let bytes = attempt!(read_bounded(path, MAX_BYTES.saturating_sub(total_bytes)));
        total_bytes = total_bytes.saturating_add(bytes.len() as u64);
        hasher.update(&(relative.len() as u64).to_le_bytes());
        hasher.update(relative.as_bytes());
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(TreeDigest {
        hash: hasher.finalize().to_hex().to_string(),
        files: files.len(),
        bytes: total_bytes,
    })
}

/// Digest the caller's paths without yet deciding whether they are the paths
/// this build uses.
fn bind(
    lean_binary: &std::path::Path,
    library_tree: &std::path::Path,
) -> Result<Selected, crate::workflow::output::Failure> {
    debug_assert!(!lean_binary.as_os_str().is_empty());
    debug_assert!(!library_tree.as_os_str().is_empty());
    let metadata = std::fs::symlink_metadata(lean_binary).map_err(|error| {
        refused(std::format!(
            "cannot stat --lean-binary {}: {error}",
            lean_binary.display()
        ))
    })?;
    if !metadata.file_type().is_file() {
        return Err(refused(std::format!(
            "--lean-binary must be a regular file: {}",
            lean_binary.display()
        )));
    }
    let library_root = std::fs::symlink_metadata(library_tree).map_err(|error| {
        refused(std::format!(
            "cannot stat --library-tree {}: {error}",
            library_tree.display()
        ))
    })?;
    if !library_root.file_type().is_dir() {
        return Err(refused(std::format!(
            "--library-tree must be a directory: {}",
            library_tree.display()
        )));
    }
    let files = attempt!(walk(library_tree, MAX_FILES, MAX_DIRECTORIES));
    if files.is_empty() {
        return Err(refused(std::string::String::from(
            "--library-tree contains no files; v2 binds the contract library the build compiles",
        )));
    }
    let tree = attempt!(tree_digest(&files));
    let binary = attempt!(read_bounded(lean_binary, MAX_BYTES));
    debug_assert!(!tree.hash.is_empty());
    Ok(Selected {
        lean_sha256: crate::workflow::intrinsic::sha256(&binary),
        lean_version: crate::sandbox::PIN,
        lean_commit: crate::sandbox::COMMIT,
        library_sha256: tree.hash,
        library_files: tree.files as u64,
        library_bytes: tree.bytes,
    })
}

/// Refuse a binding whose bytes are not the bytes this build uses.
fn verify_used(selected: &Selected) -> Result<(), crate::workflow::output::Failure> {
    debug_assert!(!selected.lean_sha256.is_empty());
    debug_assert!(!selected.library_sha256.is_empty());
    let lean = attempt!(crate::sandbox::release_binary());
    let used_binary = attempt!(read_bounded(&lean, MAX_BYTES));
    if crate::workflow::intrinsic::sha256(&used_binary) != selected.lean_sha256 {
        return Err(refused(std::format!(
            "--lean-binary bytes differ from the Lean release this build invokes ({})",
            lean.display()
        )));
    }
    let root = attempt!(crate::workflow::contract_library_root());
    let files = attempt!(walk(&root, MAX_FILES, MAX_DIRECTORIES));
    let used = attempt!(tree_digest(&files));
    if used.hash != selected.library_sha256 {
        return Err(refused(std::format!(
            "--library-tree content differs from the contract library this build compiles ({})",
            root.display()
        )));
    }
    Ok(())
}

/// Bind one Lean binary and one contract library tree, and refuse the binding
/// unless those bytes are the ones this build uses.
///
/// # Errors
/// `toolchain-binding` when a path is missing, unreadable, a symlink, not a
/// regular file, escapes its root, is not UTF-8 named, is an empty tree,
/// exceeds the file, directory or byte bound, or differs from the configured
/// Lean release or contract library root.
pub(crate) fn select(
    lean_binary: &std::path::Path,
    library_tree: &std::path::Path,
) -> Result<Selected, crate::workflow::output::Failure> {
    debug_assert!(!lean_binary.as_os_str().is_empty());
    let selected = attempt!(bind(lean_binary, library_tree));
    attempt!(verify_used(&selected));
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::{bind, select, walk};

    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("noble-toolchain-{name}-{}", std::process::id()))
    }

    #[test]
    fn a_tree_binding_sorts_paths_and_binds_exact_content() {
        let root = scratch("order");
        let library = root.join("lib");
        std::fs::create_dir_all(library.join("sub")).expect("create tree");
        let lean = root.join("lean");
        std::fs::write(&lean, b"binary").expect("write binary");
        std::fs::write(library.join("a.olean"), b"alpha").expect("write a");
        std::fs::write(library.join("z.olean"), b"zeta").expect("write z");
        std::fs::write(library.join("sub/b.olean"), b"beta").expect("write b");
        let walked = walk(&library, 8, 8).expect("walk tree");
        let relative: std::vec::Vec<_> = walked.iter().map(|entry| entry.0.clone()).collect();
        // Traversal appends a nested file after its parent's siblings, so this
        // ordering is only reachable through the explicit sort.
        assert_eq!(relative, ["a.olean", "sub/b.olean", "z.olean"]);
        let mut expected = blake3::Hasher::new();
        for (relative, contents) in [
            ("a.olean", b"alpha".as_slice()),
            ("sub/b.olean", b"beta".as_slice()),
            ("z.olean", b"zeta".as_slice()),
        ] {
            expected.update(&(relative.len() as u64).to_le_bytes());
            expected.update(relative.as_bytes());
            expected.update(&(contents.len() as u64).to_le_bytes());
            expected.update(contents);
        }
        let result = bind(&lean, &library);
        assert!(result.is_ok(), "bind first");
        let Ok(first) = result else { return };
        assert_eq!(first.library_files, 3);
        assert_eq!(first.library_bytes, 13);
        assert_eq!(
            first.library_sha256,
            expected.finalize().to_hex().to_string(),
            "the digest is over sorted relative paths"
        );
        let result = bind(&lean, &library);
        assert!(result.is_ok(), "bind again");
        let Ok(again) = result else { return };
        assert_eq!(first.library_sha256, again.library_sha256);
        assert_eq!(first.lean_sha256, again.lean_sha256);
        std::fs::write(library.join("sub/b.olean"), b"gamma").expect("rewrite b");
        let result = bind(&lean, &library);
        assert!(result.is_ok(), "bind changed");
        let Ok(changed) = result else { return };
        assert_ne!(first.library_sha256, changed.library_sha256);
        assert_eq!(first.lean_sha256, changed.lean_sha256);
        std::fs::write(&lean, b"other").expect("rewrite binary");
        let result = bind(&lean, &library);
        assert!(result.is_ok(), "bind replaced");
        let Ok(replaced) = result else { return };
        assert_ne!(first.lean_sha256, replaced.lean_sha256);
        std::fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn an_empty_library_tree_is_refused() {
        let root = scratch("empty");
        let library = root.join("lib");
        std::fs::create_dir_all(&library).expect("create empty tree");
        let lean = root.join("lean");
        std::fs::write(&lean, b"not the selected Lean").expect("write binary");
        let result = select(&lean, &library);
        assert!(result.is_err(), "an empty tree binds nothing");
        let Err(refused) = result else { return };
        assert!(
            refused.message.contains("contains no files"),
            "{}",
            refused.message
        );
        std::fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn traversal_bounds_are_enforced_before_entries_are_stored() {
        let root = scratch("bounds");
        let library = root.join("lib");
        std::fs::create_dir_all(library.join("one")).expect("create tree");
        std::fs::create_dir_all(library.join("two")).expect("create second directory");
        std::fs::write(library.join("a.olean"), b"a").expect("write a");
        std::fs::write(library.join("b.olean"), b"b").expect("write b");
        std::fs::write(library.join("one/c.olean"), b"c").expect("write c");
        let exact = walk(&library, 3, 3);
        assert!(exact.is_ok(), "a tree exactly at both bounds is allowed");
        let Ok(exact) = exact else { return };
        assert_eq!(exact.len(), 3);
        let over_files = walk(&library, 2, 3);
        assert!(over_files.is_err(), "a fourth file is refused");
        let Err(refused) = over_files else { return };
        assert!(
            refused.message.contains("more than 2 files"),
            "{}",
            refused.message
        );
        let over_directories = walk(&library, 3, 2);
        assert!(over_directories.is_err(), "a third directory is refused");
        let Err(refused) = over_directories else {
            return;
        };
        assert!(
            refused.message.contains("more than 2 directories"),
            "{}",
            refused.message
        );
        std::fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_non_utf8_file_name_is_refused() {
        use std::os::unix::ffi::OsStrExt;
        let root = scratch("nonutf8");
        let library = root.join("lib");
        std::fs::create_dir_all(&library).expect("create tree");
        let lean = root.join("lean");
        std::fs::write(&lean, b"binary").expect("write binary");
        let name = std::ffi::OsStr::from_bytes(b"bad\xff.olean");
        std::fs::write(library.join(name), b"alpha").expect("write unnamed");
        let result = bind(&lean, &library);
        assert!(result.is_err(), "a lossy name cannot bind exactly");
        let Err(refused) = result else { return };
        assert!(
            refused.message.contains("must be UTF-8"),
            "{}",
            refused.message
        );
        std::fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_symlink_in_the_tree_is_refused() {
        let root = scratch("link");
        let library = root.join("lib");
        std::fs::create_dir_all(&library).expect("create tree");
        let lean = root.join("lean");
        std::fs::write(&lean, b"binary").expect("write binary");
        std::fs::write(root.join("target"), b"x").expect("write target");
        std::os::unix::fs::symlink(root.join("target"), library.join("link.olean"))
            .expect("symlink");
        let result = bind(&lean, &library);
        assert!(result.is_err(), "symlink is refused");
        let Err(refused) = result else { return };
        assert!(refused.message.contains("symlinks"), "{}", refused.message);
        std::fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_file_over_the_remaining_budget_is_refused_before_allocation() {
        let root = scratch("bound");
        std::fs::create_dir_all(&root).expect("create root");
        let path = root.join("big.olean");
        std::fs::write(&path, b"12345").expect("write big");
        let refused = super::read_bounded(&path, 4);
        assert!(refused.is_err(), "a file over the budget is refused");
        let Err(failure) = refused else { return };
        assert!(
            failure.message.contains("over the remaining"),
            "{}",
            failure.message
        );
        let exact = super::read_bounded(&path, 5);
        assert!(exact.is_ok(), "a file exactly at the budget is allowed");
        let Ok(bytes) = exact else { return };
        assert_eq!(bytes.len(), 5);
        std::fs::remove_dir_all(&root).expect("clean up");
    }
}
