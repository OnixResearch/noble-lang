//! Linux-only cooperative save completion: a selected .tmp file and paired
//! atomic rename select a candidate; byte snapshots decide
//! whether it can be published. Inotify does not authenticate in-place writes,
//! hardlinks, shared mappings, or the writer's preceding fsync.

use std::io::Stdin;
use std::mem::MaybeUninit;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use rustix::event::{poll, PollFd, PollFlags};
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use rustix::io::Errno;

const MAX_EVENTS_PER_DRAIN: usize = 4096;
const MAX_RENAME_PAIRS: usize = 64;

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
    pending_trusted: bool,
    broken: Option<String>,
}

impl Watcher {
    pub(super) fn new(path: &Path) -> Result<Self, String> {
        use std::os::fd::AsRawFd;

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
            sequence: 0, pending: false, pending_trusted: false, broken: None })
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
            Some((self.sequence, self.pending_trusted))
        } else {
            None
        }
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
                self.pending_trusted = paired.is_some() && event.cookie() != 0;
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
