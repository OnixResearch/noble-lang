//! Linux-only save notification: a selected .tmp file and paired atomic rename
//! nominate a candidate, but inotify cannot attest a writer's fsync or closure.
//! A separately fsynced, host-owned copy binds the bounded selected bytes
//! before the ordinary selected-path and generation publication checks.

use std::io::{Stdin, Write};
#[cfg(feature = "live-test-barrier")]
use std::io::Read;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, FileExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rustix::event::{poll, PollFd, PollFlags};
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use rustix::io::Errno;

const MAX_EVENTS_PER_DRAIN: usize = 4096;
const MAX_RENAME_PAIRS: usize = 64;
static NEXT_SEAL_DIRECTORY: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "live-test-barrier")]
static ACCEPTED_SUBMISSION_PAUSED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn pinned(directory: &std::fs::File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
}

struct SealDirectory {
    parent: std::fs::File,
    directory: std::fs::File,
    name: String,
    named_path: PathBuf,
    identity: (u64, u64),
}

impl SealDirectory {
    fn is_current(&self) -> bool {
        self.directory.metadata().is_ok_and(|metadata| metadata.is_dir()
            && (metadata.dev(), metadata.ino()) == self.identity
            && metadata.permissions().mode() & 0o777 == 0o700)
            && std::fs::symlink_metadata(&self.named_path).is_ok_and(|metadata|
                metadata.is_dir() && (metadata.dev(), metadata.ino()) == self.identity)
    }
}

impl Drop for SealDirectory {
    fn drop(&mut self) {
        if self.is_current() && std::fs::remove_dir(&self.named_path).is_ok() {
            let _ = self.parent.sync_all();
        }
    }
}

pub(super) struct SealedSnapshot {
    _parent: std::fs::File,
    directory: std::fs::File,
    named_directory: PathBuf,
    directory_identity: (u64, u64),
    file: std::fs::File,
    named_file: PathBuf,
    file_identity: super::FileIdentity,
    source_digest: String,
    source_identity: super::FileIdentity,
    source_directory: (u64, u64),
    sequence: u64,
    generation: u64,
}

impl SealedSnapshot {
    pub(super) fn matches(
        &self,
        snapshot: &super::Snapshot,
        sequence: u64,
        generation: u64,
    ) -> bool {
        if self.sequence != sequence || self.generation != generation
            || self.source_digest != snapshot.digest
            || self.source_identity != snapshot.identity
            || self.source_directory != snapshot.directory
            || !self.directory.metadata().is_ok_and(|metadata| metadata.is_dir()
                && (metadata.dev(), metadata.ino()) == self.directory_identity
                && metadata.permissions().mode() & 0o777 == 0o700)
            || !std::fs::symlink_metadata(&self.named_directory)
                .is_ok_and(|metadata| metadata.is_dir()
                    && (metadata.dev(), metadata.ino()) == self.directory_identity)
        {
            return false;
        }
        let valid = |metadata: &std::fs::Metadata| metadata.is_file()
            && super::source_identity(metadata) == self.file_identity
            && metadata.nlink() == 1 && metadata.permissions().mode() & 0o777 == 0o400;
        if !self.file.metadata().is_ok_and(|metadata| valid(&metadata))
            || !std::fs::symlink_metadata(&self.named_file)
                .is_ok_and(|metadata| valid(&metadata))
        {
            return false;
        }
        let mut offset = 0;
        let mut buffer = [0u8; 8192];
        while offset < snapshot.bytes.len() {
            let count = buffer.len().min(snapshot.bytes.len() - offset);
            let read = match self.file.read_at(&mut buffer[..count], offset as u64) {
                Ok(read) if read > 0 => read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                _ => return false,
            };
            if &buffer[..read] != &snapshot.bytes[offset..offset + read] {
                return false;
            }
            offset += read;
        }
        self.file.metadata().is_ok_and(|metadata| valid(&metadata))
            && std::fs::symlink_metadata(&self.named_file)
                .is_ok_and(|metadata| valid(&metadata))
    }

    pub(super) fn committed_matches(
        &self,
        snapshot: &super::Snapshot,
        sequence: u64,
        generation: u64,
    ) -> bool {
        self.generation.checked_add(1) == Some(generation)
            && self.matches(snapshot, sequence, self.generation)
    }
}

impl Drop for SealedSnapshot {
    fn drop(&mut self) {
        if std::fs::symlink_metadata(&self.named_file).is_ok_and(|metadata|
            (metadata.dev(), metadata.ino())
                == (self.file_identity.0, self.file_identity.1))
            && std::fs::remove_file(&self.named_file).is_ok()
        {
            let _ = self.directory.sync_all();
        }
    }
}

pub(super) enum Ready {
    Input,
    Save,
}

pub(super) struct Watcher {
    descriptor: OwnedFd,
    watch_id: i32,
    directory: std::fs::File,
    directory_identity: (u64, u64),
    selected: PathBuf,
    basename: Vec<u8>,
    moved_from_cookies: [u32; MAX_RENAME_PAIRS],
    moved_from_count: usize,
    sequence: u64,
    pending: bool,
    pending_paired: bool,
    broken: Option<String>,
    seals: Option<SealDirectory>,
    #[cfg(feature = "live-test-barrier")]
    fail_next_seal_directory_sync: bool,
}

impl Watcher {
    pub(super) fn new(path: &Path) -> Result<Self, String> {
        let selected = super::selected_path(path)?;
        let parent = selected.parent().ok_or("watched source lacks a parent")?;
        let basename = selected.file_name().ok_or("watched source lacks a basename")?
            .as_bytes().to_vec();
        if basename.len() > 251 {
            return Err("watched source basename cannot have a bounded .tmp sibling".into());
        }
        let directory = std::fs::OpenOptions::new().read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(parent).map_err(|error| error.to_string())?;
        let before = directory.metadata().map_err(|error| error.to_string())?;
        let directory_identity = (before.dev(), before.ino());
        if !before.is_dir() || std::fs::symlink_metadata(parent)
            .map(|metadata| (metadata.dev(), metadata.ino()))
            .map_err(|error| error.to_string())? != directory_identity {
            return Err("watched source directory changed before registration".into());
        }
        let descriptor = inotify::init(CreateFlags::NONBLOCK | CreateFlags::CLOEXEC)
            .map_err(|error| error.to_string())?;
        // The pinned descriptor, not a pathname that can be swapped while the
        // watch is installed, identifies the selected parent directory.
        let pinned = format!("/proc/self/fd/{}", directory.as_raw_fd());
        let watch_id = inotify::add_watch(&descriptor, pinned.as_str(),
            WatchFlags::MOVED_FROM | WatchFlags::MOVED_TO
                | WatchFlags::DELETE_SELF | WatchFlags::MOVE_SELF)
            .map_err(|error| error.to_string())?;
        let after = std::fs::symlink_metadata(parent).map_err(|error| error.to_string())?;
        if !after.is_dir() || (after.dev(), after.ino()) != directory_identity {
            return Err("watched source directory changed during registration".into());
        }
        Ok(Self { descriptor, watch_id, directory, directory_identity, selected,
            basename,
            moved_from_cookies: [0; MAX_RENAME_PAIRS], moved_from_count: 0,
            sequence: 0, pending: false, pending_paired: false, broken: None,
            seals: None,
            #[cfg(feature = "live-test-barrier")]
            fail_next_seal_directory_sync: std::env::var_os(
                "NOBLE_LIVE_TEST_SEAL_FAIL_DIRECTORY_SYNC")
                .as_deref() == Some(std::ffi::OsStr::new("1")) })
    }

    pub(super) fn selected(&self) -> &Path {
        &self.selected
    }

    pub(super) fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(super) fn has_pending(&self) -> bool {
        self.pending && self.broken.is_none()
    }

    pub(super) fn take_pending(&mut self) -> Option<(u64, bool)> {
        if self.pending && self.broken.is_none() {
            self.pending = false;
            Some((self.sequence, self.pending_paired))
        } else {
            None
        }
    }

    /// An explicit reload may have published the very rename that the loop
    /// has not yet taken. Only that committed sequence is satisfied here;
    /// draining a newer rename must leave its nomination pending.
    pub(super) fn consume_published_pending(&mut self, sequence: u64) -> Result<(), String> {
        self.drain()?;
        if self.pending && self.sequence == sequence {
            self.pending = false;
        }
        Ok(())
    }

    fn seal_directory(&mut self) -> Result<&SealDirectory, String> {
        if self.seals.is_none() {
            self.verify_parent()?;
            let parent = pinned(&self.directory);
            let mut created = None;
            for _ in 0..32 {
                let name = format!(".noble-live-seals-{}-{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_err(|error| error.to_string())?.as_nanos(),
                    NEXT_SEAL_DIRECTORY.fetch_add(1, Ordering::Relaxed));
                let path = parent.join(&name);
                match std::fs::DirBuilder::new().mode(0o700).create(&path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => return Err(format!("private source seal directory: {error}")),
                }
                let directory = match std::fs::OpenOptions::new().read(true)
                    .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                    .open(&path)
                {
                    Ok(directory) => directory,
                    Err(error) => {
                        let _ = std::fs::remove_dir(&path);
                        return Err(format!("private source seal directory: {error}"));
                    }
                };
                let metadata = directory.metadata().map_err(|error| {
                    let _ = std::fs::remove_dir(&path);
                    error.to_string()
                })?;
                let parent = self.directory.try_clone().map_err(|error| {
                    let _ = std::fs::remove_dir(&path);
                    error.to_string()
                })?;
                let seals = SealDirectory {
                    named_path: pinned(&parent).join(&name),
                    parent,
                    directory,
                    name,
                    identity: (metadata.dev(), metadata.ino()),
                };
                if !seals.is_current() {
                    return Err("private source seal directory changed during creation".into());
                }
                self.directory.sync_all()
                    .map_err(|error| format!("private source seal parent sync: {error}"))?;
                created = Some(seals);
                break;
            }
            self.seals = Some(created.ok_or("private source seal directory name exhausted")?);
        }
        self.seals.as_ref().ok_or("private source seal directory unavailable".into())
    }

    /// Freeze exactly the bytes of a previously checked selected-file snapshot.
    /// Nothing here replaces or writes the editor's selected source inode.
    pub(super) fn seal_snapshot(
        &mut self,
        candidate: &super::Snapshot,
        sequence: u64,
        generation: u64,
    ) -> Result<SealedSnapshot, String> {
        if candidate.identity.2 != 1 {
            return Err("watched source has a hardlink alias, not a complete-save file".into());
        }
        if candidate.bytes.len() > super::LIMIT as usize
            || self.sequence != sequence || self.broken.is_some()
        {
            return Err("watched source is not a bounded current single-link save".into());
        }
        #[cfg(feature = "live-test-barrier")]
        let fail_directory_sync = std::mem::take(&mut self.fail_next_seal_directory_sync);
        let seals = self.seal_directory()?;
        if !seals.is_current() {
            return Err("private source seal directory changed".into());
        }
        let name = format!("snapshot-{sequence}-{generation}");
        let path = pinned(&seals.directory).join(&name);
        let parent = seals.parent.try_clone().map_err(|error| error.to_string())?;
        let directory = seals.directory.try_clone().map_err(|error| error.to_string())?;
        let file = std::fs::OpenOptions::new().read(true).write(true).create_new(true)
            .mode(0o600).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path).map_err(|error|
                format!("private source seal file creation: {error}"))?;
        let metadata = file.metadata().map_err(|error| {
            let _ = std::fs::remove_file(&path);
            format!("private source seal file metadata: {error}")
        })?;
        let mut seal = SealedSnapshot {
            named_directory: pinned(&parent).join(&seals.name),
            named_file: pinned(&directory).join(&name),
            _parent: parent,
            directory,
            directory_identity: seals.identity,
            file,
            file_identity: super::source_identity(&metadata),
            source_digest: candidate.digest.clone(),
            source_identity: candidate.identity,
            source_directory: candidate.directory,
            sequence,
            generation,
        };
        seal.file.write_all(&candidate.bytes)
            .map_err(|error| format!("private source seal write: {error}"))?;
        seal.file.sync_all()
            .map_err(|error| format!("private source seal file sync: {error}"))?;
        seal.file.set_permissions(std::fs::Permissions::from_mode(0o400))
            .map_err(|error| format!("private source seal permissions: {error}"))?;
        seal.file.sync_all()
            .map_err(|error| format!("private source seal metadata sync: {error}"))?;
        #[cfg(feature = "live-test-barrier")]
        if fail_directory_sync {
            return Err("private source seal directory sync failed (test-injected)".into());
        }
        seal.directory.sync_all()
            .map_err(|error| format!("private source seal directory sync: {error}"))?;
        let read_only = std::fs::OpenOptions::new().read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&seal.named_file)
            .map_err(|error| format!("private source seal read-only handoff: {error}"))?;
        let opened = read_only.metadata().map_err(|error| error.to_string())?;
        if (opened.dev(), opened.ino())
            != (seal.file_identity.0, seal.file_identity.1) {
            return Err("private source seal inode changed during read-only handoff".into());
        }
        seal.file = read_only;
        seal.file_identity = super::source_identity(
            &seal.file.metadata().map_err(|error| error.to_string())?);
        if !seal.matches(candidate, sequence, generation) {
            return Err("private source seal changed after sync".into());
        }
        #[cfg(feature = "live-test-barrier")]
        pause_seal_for_test(candidate)?;
        self.drain()?;
        if self.sequence != sequence || !super::same_snapshot(&self.selected, candidate) {
            return Err("watched source superseded during host byte seal".into());
        }
        Ok(seal)
    }

    pub(super) fn wait(&self, input: &Stdin) -> Result<Ready, String> {
        if let Some(problem) = &self.broken {
            return Err(problem.clone());
        }
        let mut descriptors = [
            PollFd::new(input, PollFlags::IN),
            PollFd::new(&self.descriptor, PollFlags::IN),
        ];
        loop {
            match poll(&mut descriptors, None) {
                Ok(_) => {
                    if descriptors.iter().any(|fd| fd.revents().intersects(
                        PollFlags::ERR | PollFlags::NVAL)) {
                        return Err("live watch descriptor failed".into());
                    }
                    if descriptors[1].revents().contains(PollFlags::HUP) {
                        return Err("selected source directory watch disconnected".into());
                    }
                    // Commands already available before an ACK keep their old
                    // accepted sequence; a selected-byte gate still refuses a
                    // source changed since that old generation's file ACK.
                    if descriptors[0].revents().intersects(PollFlags::IN | PollFlags::HUP) {
                        return Ok(Ready::Input);
                    }
                    if descriptors[1].revents().contains(PollFlags::IN) {
                        return Ok(Ready::Save);
                    }
                }
                Err(Errno::INTR) => continue,
                Err(error) => return Err(error.to_string()),
            }
        }
    }

    pub(super) fn drain(&mut self) -> Result<(), String> {
        if let Some(problem) = &self.broken {
            return Err(problem.clone());
        }
        let mut buffer = [MaybeUninit::uninit(); 4096];
        let mut reader = inotify::Reader::new(&self.descriptor, &mut buffer);
        let mut drained = false;
        let mut problem = None;
        for _ in 0..MAX_EVENTS_PER_DRAIN {
            let event = match reader.next() {
                Ok(event) => event,
                Err(Errno::AGAIN) => {
                    drained = true;
                    break;
                }
                Err(Errno::INTR) => continue,
                Err(error) => {
                    problem = Some(format!("live watch read failed: {error}"));
                    break;
                }
            };
            let mask = event.events();
            if mask.intersects(ReadFlags::QUEUE_OVERFLOW) {
                problem = Some("live watch event queue overflowed".into());
                break;
            }
            if event.wd() != self.watch_id || mask.intersects(
                ReadFlags::IGNORED | ReadFlags::DELETE_SELF | ReadFlags::MOVE_SELF) {
                problem = Some("selected source directory watch was lost".into());
                break;
            }
            let name = event.file_name().map(std::ffi::CStr::to_bytes);
            if mask.contains(ReadFlags::MOVED_FROM)
                && name.is_some_and(|name| name.len() == self.basename.len() + 4
                    && name.starts_with(&self.basename) && name.ends_with(b".tmp"))
                && event.cookie() != 0 {
                if self.moved_from_count == MAX_RENAME_PAIRS
                    || self.moved_from_cookies[..self.moved_from_count]
                        .contains(&event.cookie()) {
                    problem = Some("watched save rename pairs are ambiguous or exhausted".into());
                    break;
                }
                self.moved_from_cookies[self.moved_from_count] = event.cookie();
                self.moved_from_count += 1;
            }
            if mask.contains(ReadFlags::MOVED_TO) && !mask.intersects(ReadFlags::ISDIR)
                && name == Some(self.basename.as_slice()) {
                let paired = (0..self.moved_from_count)
                    .find(|index| self.moved_from_cookies[*index] == event.cookie());
                if let Some(index) = paired {
                    self.moved_from_count -= 1;
                    self.moved_from_cookies[index] = self.moved_from_cookies[self.moved_from_count];
                }
                let Some(sequence) = self.sequence.checked_add(1) else {
                    problem = Some("live watch save sequence exhausted".into());
                    break;
                };
                self.sequence = sequence;
                self.pending = true;
                self.pending_paired = paired.is_some() && event.cookie() != 0;
            }
        }
        drop(reader);
        if let Some(problem) = problem {
            return self.fail(problem);
        }
        if drained {
            self.verify_parent()
        } else {
            self.fail("live watch event batch budget exhausted".into())
        }
    }

    fn verify_parent(&mut self) -> Result<(), String> {
        let parent = self.selected.parent().ok_or("watched source lacks a parent")?;
        let opened = self.directory.metadata().map_err(|error| error.to_string())?;
        let named = std::fs::symlink_metadata(parent).map_err(|error| error.to_string())?;
        if !opened.is_dir() || !named.is_dir()
            || (opened.dev(), opened.ino()) != self.directory_identity
            || (named.dev(), named.ino()) != self.directory_identity {
            return self.fail("watched source directory was replaced".into());
        }
        Ok(())
    }

    fn fail(&mut self, problem: String) -> Result<(), String> {
        self.pending = false;
        self.broken = Some(problem.clone());
        Err(problem)
    }
}

#[cfg(feature = "live-test-barrier")]
fn pause_seal_for_test(candidate: &super::Snapshot) -> Result<(), String> {
    pause_host_for_test(
        "NOBLE_LIVE_TEST_SEAL_BARRIER_DIR",
        "NOBLE_LIVE_TEST_SEAL_BARRIER_SHA256",
        "candidate-sealed",
        &candidate.digest,
        None,
    )
}

#[cfg(feature = "live-test-barrier")]
pub(super) fn pause_accepted_submission_for_test(
    source: &[u8],
    generation: u64,
    sequence: u64,
    selected_source_sha256: &str,
) -> Result<(), String> {
    const DIRECTORY: &str = "NOBLE_LIVE_TEST_ACCEPTED_SUBMISSION_BARRIER_DIR";
    if std::env::var_os(DIRECTORY).is_none() {
        return Ok(());
    }
    let digest = crate::workflow::intrinsic::sha256(source);
    let expected = std::env::var_os("NOBLE_LIVE_TEST_ACCEPTED_SUBMISSION_SHA256")
        .ok_or("private accepted submission barrier lacks its expected SHA-256")?;
    if expected.to_str() != Some(digest.as_str())
        || ACCEPTED_SUBMISSION_PAUSED.swap(true, Ordering::Relaxed)
    {
        return Ok(());
    }
    pause_host_for_test(
        DIRECTORY,
        "NOBLE_LIVE_TEST_ACCEPTED_SUBMISSION_SHA256",
        "accepted-watch-submission",
        &digest,
        Some((generation, sequence, selected_source_sha256)),
    )
}

#[cfg(feature = "live-test-barrier")]
pub(super) fn pause_explicit_reload_for_test(
    point: &str,
    matched: &str,
) -> Result<(), String> {
    let (directory, expected) = match point {
        "before-snapshot" => (
            "NOBLE_LIVE_TEST_RELOAD_BEFORE_SNAPSHOT_DIR",
            "NOBLE_LIVE_TEST_RELOAD_BEFORE_SNAPSHOT_MATCH",
        ),
        "before-reconcile" => (
            "NOBLE_LIVE_TEST_RELOAD_BEFORE_RECONCILE_DIR",
            "NOBLE_LIVE_TEST_RELOAD_BEFORE_RECONCILE_MATCH",
        ),
        _ => return Err("unknown private explicit reload test barrier".into()),
    };
    pause_host_for_test(directory, expected, point, matched, None)
}

#[cfg(feature = "live-test-barrier")]
fn pause_host_for_test(
    directory_key: &str,
    expected_key: &str,
    point: &str,
    matched: &str,
    accepted: Option<(u64, u64, &str)>,
) -> Result<(), String> {
    let Some(directory) = std::env::var_os(directory_key) else {
        return Ok(());
    };
    let expected = std::env::var_os(expected_key)
        .ok_or("private live test barrier lacks its expected value")?;
    if expected.to_str() != Some(matched) {
        return Ok(());
    }
    let directory = PathBuf::from(directory);
    if !directory.is_absolute() {
        return Err("private live test barrier directory is not absolute".into());
    }
    let mut reached = std::fs::OpenOptions::new().write(true)
        .open(directory.join("reached")).map_err(|error| error.to_string())?;
    let mut event = serde_json::json!({
        "point": point,
        "matched": matched,
    });
    if let Some((generation, sequence, selected_source_sha256)) = accepted {
        event["source_generation"] = serde_json::json!(generation);
        event["watch_sequence"] = serde_json::json!(sequence);
        event["selected_source_sha256"] = serde_json::json!(selected_source_sha256);
    }
    writeln!(reached, "{event}").map_err(|error| error.to_string())?;
    let mut release = std::fs::OpenOptions::new().read(true)
        .open(directory.join("release")).map_err(|error| error.to_string())?;
    release.read_exact(&mut [0u8; 1]).map_err(|error| error.to_string())
}
