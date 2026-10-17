#!/usr/bin/env python3
"""Private source-bound Rust bootstrap launcher; no Nix/Cargo invocation by this script."""
import atexit
import hashlib
import fcntl
import json
import os
import pathlib
import select
import shutil
import secrets
import re
import signal
import stat
import struct
import subprocess
import sys
import tarfile
import time
import tomllib
import stage2_quota_client as quota

R = pathlib.Path.home() / ".cache/noble-octet-rustc-ee4-prep-20261008"
SRC = R / "source/rust-ac7f9ec7da74d37fd28667c86bf117a39ba5b02a"
PYTHON = "/nix/store/l9k0anq0z7zz81zcwy035jfwap9ga6rl-python3-3.13.13/bin/python3.13"
CONFIG = R / "receipts/bootstrap-stage2-vendor.toml"
VENDOR = R / "cargo-home/vendor"
TRIPLE = "x86_64-unknown-linux-gnu"
GCC = "/nix/store/xcnqqnhw9hb4j5rjgds2yjryi8qki5f3-gcc-wrapper-15.2.0/bin"
PATH = ":".join([
    str(R / "tool-bin-build"), GCC,
    "/nix/store/k1b82rs72af124wmq6nb00nhw5i7ikp7-cmake-4.1.2/bin",
    "/nix/store/vj0z9ncypj0qcp8jdcri68dls27inzm6-ninja-1.13.2/bin",
    "/nix/store/l9k0anq0z7zz81zcwy035jfwap9ga6rl-python3-3.13.13/bin",
    "/nix/store/z4zcd87nx2hrsdayd0vl2mx5ncj3ikd8-binutils-wrapper-2.46/bin",
])
GIB = 1024 ** 3
PRIVATE_ROOTS = (
    R, R / "input", R / "source", SRC, SRC / "src/llvm-project",
    R / "build", R / "tmp", R / "receipts", R / "cargo-home",
    R / "cargo-home/vendor", R / "stage0-cache",
)
SOFT_PRIVATE = 90 * GIB
SOFT_FREE = 180 * GIB
SOFT_MEMORY = 12 * GIB
HARD_PRIVATE = 120 * GIB
HARD_FREE = 150 * GIB
HARD_MEMORY = 8 * GIB
SAMPLE_DEADLINE_NS = 2_000_000_000
LOCK_PATH = pathlib.Path("/run/noble-octet-stage2/vendor-build.lock")
SANDBOX_EXEC = pathlib.Path("/run/noble-octet-stage2/stage2-sandbox-exec")
HOST_FS = pathlib.Path("/")
HOST_ROOT_DEV = 66307  # Bonsai /dev/nvme0n1p3, independently pinned from R
EXPECTED_STAGE0 = {
    "rustc": "09f6ad1eac75fdd1fb9fec65896a89230af299764be7035610dbb257ff7db092",
    "rust-std": "bc2ffcdf3a4eae93774ac99397bde4d31a1aa0379b86836d01967e300aad58a7",
    "cargo": "3330ec521c6d283934f756775be28b827ed0e04ad9f584245d14641ccde72dfc",
}


def sha256(path):
    with open(path, "rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def source_inventory(root, excluded=()):
    inventory = {}
    directories = set()
    for directory, dirs, files in os.walk(root, followlinks=False):
        for name in dirs + files:
            path = pathlib.Path(directory) / name
            relative = path.relative_to(root).as_posix()
            if relative in excluded:
                if path.is_symlink() or not path.is_dir():
                    raise RuntimeError("excluded nested source is not a directory: " + str(path))
                dirs.remove(name)
                continue
            mode = path.lstat().st_mode
            if stat.S_ISREG(mode):
                inventory[relative] = ("file", stat.S_IMODE(mode) & 0o111)
            elif stat.S_ISLNK(mode):
                inventory[relative] = ("link", os.readlink(path))
            elif stat.S_ISDIR(mode):
                directories.add(relative)
            else:
                raise RuntimeError("special source entry: " + str(path))
    return inventory, directories


def verify_source_archive(archive, root, prefix, overlays, excluded=()):
    """Compare the actual extracted inputs, not an unrelated Git index."""
    actual, actual_dirs = source_inventory(root, excluded)
    expected = {}
    expected_dirs = set()
    with tarfile.open(archive, "r:gz") as stream:
        for member in stream:
            if member.name == prefix or member.name == prefix + "/":
                continue
            if not member.name.startswith(prefix + "/"):
                raise RuntimeError("source archive path outside expected root: " + member.name)
            relative = member.name[len(prefix) + 1:].rstrip("/")
            parts = pathlib.PurePosixPath(relative).parts
            if not relative or any(part in ("", ".", "..") for part in parts):
                raise RuntimeError("unsafe source archive path: " + member.name)
            if member.isdir():
                expected_dirs.add(relative)
                continue
            if relative in expected:
                raise RuntimeError("duplicate source archive member: " + member.name)
            if member.isfile():
                expected[relative] = ("file", member.mode & 0o111)
            elif member.issym():
                expected[relative] = ("link", member.linkname)
            else:
                raise RuntimeError("unexpected source archive member type: " + member.name)
            if relative in overlays:
                continue
            if actual.get(relative) != expected[relative]:
                raise RuntimeError("source entry type, mode or symlink mismatch: " + str(root / relative))
            if member.isfile():
                archived = stream.extractfile(member)
                if archived is None:
                    raise RuntimeError("cannot read archived source: " + member.name)
                with archived:
                    digest = hashlib.file_digest(archived, "sha256").hexdigest()
                if sha256(root / relative) != digest:
                    raise RuntimeError("live source differs from authenticated archive: " + str(root / relative))
    if set(actual) != set(expected) | set(overlays):
        raise RuntimeError("source path inventory differs from archive and overlays: " +
                           repr({"missing": sorted(set(expected) - set(actual))[:8],
                                 "extra": sorted(set(actual) - set(expected) - set(overlays))[:8]}))
    allowed_dirs = (expected_dirs - set(excluded)) | {
        str(pathlib.PurePosixPath(p).parent) for p in overlays
        if pathlib.PurePosixPath(p).parent != pathlib.PurePosixPath(".")}
    if actual_dirs != allowed_dirs:
        raise RuntimeError("source directory inventory differs from archive: " +
                           repr({"missing": sorted(allowed_dirs - actual_dirs)[:8],
                                 "extra": sorted(actual_dirs - allowed_dirs - set(excluded))[:8]}))
    for relative, identity in overlays.items():
        if actual.get(relative) != identity[0]:
            raise RuntimeError("source overlay type/mode/link mismatch: " + str(root / relative))
        if identity[0][0] == "file" and sha256(root / relative) != identity[1]:
            raise RuntimeError("source overlay checksum mismatch: " + str(root / relative))
    return {"archive": str(archive), "entries": len(actual)}


def verify_llvm_git_tree(root, tree):
    git = str(R / "tool-bin-build/git")
    result = subprocess.run(
        [git, "--git-dir=" + str(R / "receipts/llvm-identity.git"),
         "ls-tree", "-rz", tree], capture_output=True, check=True, timeout=30)
    actual, _ = source_inventory(root)
    tracked = set()
    normalized = []
    for record in result.stdout.split(b"\0"):
        if not record:
            continue
        metadata, path_bytes = record.split(b"\t", 1)
        mode, kind, digest = metadata.split()
        if kind != b"blob":
            raise RuntimeError("LLVM Git tree contains unexpected non-blob: " + repr(path_bytes))
        relative = os.fsdecode(path_bytes)
        tracked.add(relative)
        path = root / relative
        expected_type = ("link", os.readlink(path)) if mode == b"120000" else (
            "file", 0o111 if mode == b"100755" else 0)
        if mode not in (b"100644", b"100755", b"120000") or actual.get(relative) != expected_type:
            raise RuntimeError("live LLVM Git path, mode or symlink differs: " + relative)
        if mode == b"120000":
            raw = os.fsencode(os.readlink(path))
            identity = hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest()
        else:
            size = path.stat().st_size
            blob = hashlib.sha1(b"blob " + str(size).encode() + b"\0")
            with path.open("rb") as stream:
                for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                    blob.update(chunk)
            identity = blob.hexdigest()
        if identity != digest.decode("ascii"):
            if mode == b"120000":
                raise RuntimeError("live LLVM link differs from pinned Git tree: " + relative)
            raw = path.read_bytes()
            canonical = raw.replace(b"\r\n", b"\n")
            alternate = hashlib.sha1(
                b"blob " + str(len(canonical)).encode() + b"\0" + canonical).hexdigest()
            if canonical == raw or alternate != digest.decode("ascii"):
                raise RuntimeError("live LLVM file differs from pinned Git tree: " + relative)
            normalized.append(relative)
    if tracked != set(actual):
        raise RuntimeError("live LLVM Git tree path inventory differs: " +
                           repr({"extra": sorted(set(actual) - tracked)[:8],
                                 "missing": sorted(tracked - set(actual))[:8]}))
    expected_normalized = {
        "lld/test/wasm/Inputs/libstub.so",
        "llvm/test/MC/AsmParser/preserve-comments-crlf.s",
        "llvm/test/tools/llvm-mca/X86/directives-handle-crlf.s",
        "llvm/test/tools/split-file/Inputs/basic-aa.crlf",
        "llvm/test/tools/split-file/Inputs/basic-bb.crlf",
        "llvm/test/tools/split-file/basic.crlf.test",
    }
    if set(normalized) != expected_normalized:
        raise RuntimeError("LLVM Git archive CRLF exception membership changed: " + repr(normalized))
    return {"tracked_entries": len(tracked), "crlf_normalized_paths": sorted(normalized)}


def pin_private_roots():
    pins = {}
    base = R.resolve(strict=True)
    for path in PRIVATE_ROOTS:
        info = path.lstat()
        if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or
            not path.resolve(strict=True).is_relative_to(base)):
            if path != R:
                raise RuntimeError("private directory changed owner, type or location: " + str(path))
            if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid():
                raise RuntimeError("private root changed owner or type: " + str(path))
        pins[str(path)] = (info.st_dev, info.st_ino)
    return pins


def check_private_roots(pins):
    if {str(path) for path in PRIVATE_ROOTS} != set(pins):
        raise RuntimeError("private output directory pin membership changed")
    for path in PRIVATE_ROOTS:
        info = path.lstat()
        if (not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or
            (info.st_dev, info.st_ino) != pins[str(path)]):
            raise RuntimeError("private output directory replaced or symlinked: " + str(path))
    return True


def operator_control(path, owner, kind):
    path = pathlib.Path(path)
    if not path.is_absolute():
        raise RuntimeError("operator control path is not absolute: " + str(path))
    for ancestor in path.parents:
        info = ancestor.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            raise RuntimeError("writable or replaced operator-control ancestor: " + str(ancestor))
    info = path.lstat()
    if info.st_uid != owner or info.st_mode & 0o022 or info.st_nlink != 1:
        raise RuntimeError("operator control owner, mode or link count changed: " + str(path))
    if kind == "file" and not stat.S_ISREG(info.st_mode):
        raise RuntimeError("operator control not a regular file: " + str(path))
    return info


def acquire_shared_lease():
    info = operator_control(LOCK_PATH, os.getuid(), "file")
    if stat.S_IMODE(info.st_mode) != 0o600:
        raise RuntimeError("shared vendor/build lease must be mode 0600")
    fd = os.open(LOCK_PATH, os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        info = os.fstat(fd)
        if (info.st_dev, info.st_ino) != (
                LOCK_PATH.lstat().st_dev, LOCK_PATH.lstat().st_ino):
            raise RuntimeError("shared vendor/build lease replaced during open")
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BaseException:
        os.close(fd)
        raise
    # A forked independent stopper holds a duplicate through exact scope death.
    atexit.register(os.close, fd)
    return fd


def pin_host_root(expected_device=HOST_ROOT_DEV):
    info = HOST_FS.stat()
    if (not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or
        info.st_dev != expected_device):
        raise RuntimeError("Bonsai global host root filesystem identity changed")
    return info.st_dev, info.st_ino


def cheap_snapshot(host_pin):
    info = HOST_FS.stat()
    if (info.st_dev, info.st_ino) != host_pin:
        raise RuntimeError("pinned global host root filesystem replaced")
    fs = os.statvfs(HOST_FS)
    free = fs.f_bavail * fs.f_frsize
    with open("/proc/meminfo", encoding="ascii") as stream:
        available = next(int(line.split()[1]) * 1024 for line in stream if line.startswith("MemAvailable:"))
    return {"host_free_bytes": free, "MemAvailable_bytes": available}


def snapshot(pins, host_pin, deadline_ns, quota_identity=None):
    started_ns = time.monotonic_ns()
    before = cheap_snapshot(host_pin)
    check_private_roots(pins)
    private = quota.snapshot(R, deadline_ns, identity=quota_identity)
    quota.verify_directories(PRIVATE_ROOTS, private["private_project_id"])
    check_private_roots(pins)
    after = cheap_snapshot(host_pin)
    completed_ns = time.monotonic_ns()
    if completed_ns >= deadline_ns or completed_ns - started_ns >= SAMPLE_DEADLINE_NS:
        raise RuntimeError("complete resource snapshot missed two-second deadline")
    return {**private, "snapshot_started_ns": started_ns, "snapshot_completed_ns": completed_ns,
            "host_free_bytes": min(before["host_free_bytes"], after["host_free_bytes"]),
            "MemAvailable_bytes": min(before["MemAvailable_bytes"], after["MemAvailable_bytes"])}


def breach(sample):
    if sample["private_allocated_bytes"] >= HARD_PRIVATE:
        return "hard: private >=120GiB"
    if sample["host_free_bytes"] <= HARD_FREE:
        return "hard: host free <=150GiB"
    if sample["MemAvailable_bytes"] <= HARD_MEMORY:
        return "hard: MemAvailable <=8GiB"
    if sample["private_allocated_bytes"] >= SOFT_PRIVATE:
        return "soft: private >=90GiB"
    if sample["host_free_bytes"] <= SOFT_FREE:
        return "soft: host free <=180GiB"
    if sample["MemAvailable_bytes"] <= SOFT_MEMORY:
        return "soft: MemAvailable <=12GiB"
    return None



SYSTEMD_RUN = "/nix/store/rd05syhv5v5999907a2n1r37sgi19vpd-systemd-260.2/bin/systemd-run"
SYSTEMCTL = "/nix/store/rd05syhv5v5999907a2n1r37sgi19vpd-systemd-260.2/bin/systemctl"
SCOPE_MEMORY = 26 * GIB
SCOPE_TASKS = 128
SCOPE_RUNTIME = "6h"
COMPILERS = {"rustc", "cargo", "x.py", "cc1", "cc1plus", "cmake", "ninja",
             "gcc", "g++", "clang", "clang++", "ld", "lld", "nix", "nix-build",
             "nix-store", "nix-shell"}


def proc_stat(pid):
    path = pathlib.Path("/proc") / str(pid)
    try:
        uid = path.stat().st_uid
        raw = (path / "stat").read_text(encoding="ascii")
        before, sep, after = raw.rpartition(") ")
        if not sep:
            raise RuntimeError("malformed /proc stat for " + str(pid))
        fields = after.split()
        return {"pid": pid, "uid": uid, "comm": before.split("(", 1)[1],
                "ppid": int(fields[1]), "pgrp": int(fields[2]),
                "session": int(fields[3]), "starttime": int(fields[19])}
    except (FileNotFoundError, ProcessLookupError):
        return None


def proc_details(pid):
    info = proc_stat(pid)
    if info is None:
        return {"pid": pid, "vanished": True}
    path = pathlib.Path("/proc") / str(pid)
    for field, read_value in (
        ("cmdline", lambda: (path / "cmdline").read_bytes().replace(b"\0", b" ").decode("utf-8", "replace")),
        ("exe", lambda: os.readlink(path / "exe")),
        ("cwd", lambda: os.readlink(path / "cwd")),
        ("cgroup", lambda: (path / "cgroup").read_text().strip()),
    ):
        try:
            info[field] = read_value()
        except (FileNotFoundError, ProcessLookupError, PermissionError) as exc:
            info[field + "_error"] = repr(exc)
    return info


def ancestry(pid):
    chain = []
    seen = set()
    for _ in range(64):
        if pid < 1 or pid in seen:
            break
        seen.add(pid)
        item = proc_details(pid)
        chain.append(item)
        if item.get("vanished"):
            break
        pid = item["ppid"]
    return chain


def outside_compilers(members, cgroup):
    found = []
    for entry in pathlib.Path("/proc").iterdir():
        if not entry.name.isdigit():
            continue
        pid = int(entry.name)
        if pid in members:
            continue
        info = proc_stat(pid)
        if info and info["uid"] == os.getuid() and info["comm"] in COMPILERS:
            chain = ancestry(pid)
            if chain and chain[0].get("cgroup") == "0::" + cgroup:
                continue  # scope membership raced the cgroup.procs read
            found.append({"process": chain[0], "ancestry": chain})
    return found


def same_uid_processes():
    found = {}
    for entry in pathlib.Path("/proc").iterdir():
        if entry.name.isdigit():
            info = proc_stat(int(entry.name))
            if info and info["uid"] == os.getuid():
                details = proc_details(info["pid"])
                if details.get("starttime") == info["starttime"]:
                    info["cgroup"] = details.get("cgroup")
                    found[info["pid"]] = info
    return found


def sshd_service_identity():
    result = subprocess.run(
        [SYSTEMCTL, "show", "sshd.service", "-p", "Id", "-p", "LoadState",
         "-p", "ActiveState", "-p", "MainPID", "-p", "ControlGroup", "--no-pager"],
        capture_output=True, text=True, check=True, timeout=1.5,
    )
    props = dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)
    if (props.get("Id") != "sshd.service" or props.get("LoadState") != "loaded" or
        props.get("ActiveState") != "active" or
        props.get("ControlGroup") != "/system.slice/sshd.service"):
        raise RuntimeError("trusted SSH service identity unavailable: " + repr(props))
    pid = int(props["MainPID"])
    info = proc_stat(pid)
    if (info is None or info["uid"] != 0 or
        proc_details(pid).get("cgroup") != "0::" + props["ControlGroup"]):
        raise RuntimeError("trusted SSH root MainPID unavailable: " + repr(info))
    return {"pid": pid, "starttime": info["starttime"],
            "cgroup": "0::" + props["ControlGroup"]}


def independent_ssh_login(info, chain, members, expected, scope_session, controller_cgroup, trusted):
    """Only a kernel parent chain to the pinned root sshd service is exempt."""
    session_group = f"0::/user.slice/user-{os.getuid()}.slice/session-"
    candidate_cgroup = info.get("cgroup")
    if (info["uid"] != os.getuid() or
        candidate_cgroup is None or not candidate_cgroup.startswith(session_group) or
        not re.fullmatch(r"[0-9]+\.scope", candidate_cgroup[len(session_group):]) or
        candidate_cgroup == controller_cgroup or info["session"] == scope_session or not chain or
        chain[0].get("starttime") != info["starttime"]):
        return False
    root_index = next((index for index, node in enumerate(chain[:9])
                       if node.get("pid") == trusted["pid"]), None)
    if root_index is None or root_index == 0:
        return False
    root = chain[root_index]
    if (root.get("uid") != 0 or root.get("starttime") != trusted["starttime"] or
        root.get("cgroup") != trusted["cgroup"] or
        sshd_service_identity() != trusted):
        return False
    root_children = [index for index, node in enumerate(chain[:root_index])
                     if node.get("uid") == 0]
    if root_children:
        first_root = root_children[0]
        if (first_root == 0 or chain[first_root - 1].get("uid") != os.getuid() or
            chain[first_root - 1].get("session") != chain[first_root].get("session")):
            return False
    for index, node in enumerate(chain[:root_index]):
        if (node.get("pid") in members or node.get("cgroup") != candidate_cgroup or
            node.get("session") == scope_session or node.get("uid") not in (0, os.getuid()) or
            node.get("ppid") != chain[index + 1].get("pid") or
            (node.get("uid") == os.getuid() and
             any(parent.get("uid") == 0 for parent in chain[:index]))):
            return False
    if any(node.get("pid") in members or node.get("cgroup") == "0::" + expected
           for node in chain[:root_index + 1]):
        return False
    # Re-read every process after service interrogation to reject PID reuse or
    # ancestry/cgroup migration between classification and the decision.
    for node in chain[:root_index + 1]:
        now = proc_stat(node["pid"])
        if (now is None or now["starttime"] != node["starttime"] or
            now["uid"] != node["uid"] or now["ppid"] != node["ppid"] or
            now["session"] != node["session"] or
            proc_details(node["pid"]).get("cgroup") != node["cgroup"]):
            return False
    return True


def outside_descendants(members, cgroup, baseline, scope_session,
                        controller_cgroup, trusted_ssh, authenticated_ssh):
    """Fail on new outside processes unless direct external-session origin is proven."""
    found = []
    for pid, info in same_uid_processes().items():
        if pid in members or baseline.get(pid, {}).get("starttime") == info["starttime"]:
            continue
        chain = ancestry(pid)
        if chain and chain[0].get("cgroup") == "0::" + cgroup:
            continue
        if independent_ssh_login(info, chain, members, cgroup, scope_session,
                                 controller_cgroup, trusted_ssh):
            authenticated_ssh.append({"process": chain[0], "ancestry": chain})
            continue
        if info["comm"] == "sshd-session":
            found.append({"process": chain[0], "ancestry": chain})
            continue  # A spoofed SSH name must not inherit the ordinary-session exemption.
        parent = proc_stat(info["ppid"])
        prior = baseline.get(info["ppid"])
        if parent is not None and prior is not None:
            parent_cgroup = proc_details(parent["pid"]).get("cgroup")
            if (parent["starttime"] == prior["starttime"] and
                parent["session"] == prior["session"] == info["session"] and
                parent_cgroup == prior["cgroup"] == info.get("cgroup") and
                parent_cgroup not in (None, "0::" + cgroup)):
                # Direct child of a pre-existing, unchanged external session;
                # a setsid scope member cannot join that unrelated session.
                continue
        found.append({"process": chain[0], "ancestry": chain})
    return found


def unit_properties(unit):
    result = subprocess.run(
        [SYSTEMCTL, "--user", "show", unit, "-p", "Id", "-p", "LoadState",
         "-p", "ActiveState", "-p", "ControlGroup", "-p", "MemoryMax",
         "-p", "TasksMax", "-p", "RuntimeMaxUSec", "-p", "KillMode",
         "--no-pager"], capture_output=True, text=True,
        check=True, timeout=1.5,
    )
    return dict(line.split("=", 1) for line in result.stdout.splitlines() if "=" in line)


def scope_members(cgroup):
    base = pathlib.Path("/sys/fs/cgroup") / cgroup.lstrip("/")
    if not base.is_dir():
        return set()
    members = set()
    for path in base.rglob("cgroup.procs"):
        members.update(int(pid) for pid in path.read_text().split())
    return members


def pin_scope(unit, expected):
    uid = os.getuid()
    if expected != f"/user.slice/user-{uid}.slice/user@{uid}.service/app.slice/{unit}":
        raise RuntimeError("scope is not this user's exact unique app.slice child")
    path = pathlib.Path("/sys/fs/cgroup") / expected.lstrip("/")
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        info = os.fstat(fd)
        if info.st_uid != uid or path.lstat().st_ino != info.st_ino or path.lstat().st_dev != info.st_dev:
            raise RuntimeError("scope directory identity/owner mismatch")
        event_fd = os.open("cgroup.events", os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
        return fd, event_fd, (info.st_dev, info.st_ino)
    except BaseException:
        os.close(fd)
        raise


def scope_empty(expected, scope_pin):
    fd, event_fd, identity = scope_pin
    path = pathlib.Path("/sys/fs/cgroup") / expected.lstrip("/")
    info = os.fstat(fd)
    if (info.st_dev, info.st_ino) != identity:
        raise RuntimeError("pinned scope directory identity changed")
    try:
        current = path.lstat()
    except FileNotFoundError:
        # cgroup v2 cannot remove a populated directory; an open events FD may
        # return ENODEV after systemd collects this exact scope.
        return True
    if ((current.st_dev, current.st_ino) != identity or
        current.st_uid != os.getuid() or not stat.S_ISDIR(current.st_mode)):
        raise RuntimeError("exact pinned scope directory changed")
    try:
        values = dict(line.split() for line in os.pread(event_fd, 4096, 0).decode("ascii").splitlines())
    except OSError as exc:
        raise RuntimeError("cgroup.events unavailable while exact scope still exists") from exc
    return values.get("populated") == "0"


def stop_verified_scope(unit, expected, scope_pin, force_fallback=False):
    try:
        if force_fallback:
            raise RuntimeError("injected systemctl stop failure for harmless smoke")
        result = stop_owned_unit(unit, expected)
        if scope_empty(expected, scope_pin):
            return result + "; exact cgroup empty"
        raise RuntimeError("systemctl stop left exact cgroup populated")
    except Exception as stop_error:
        try:
            deadline = time.monotonic() + 5
            signalled = []
            while time.monotonic() < deadline:
                if scope_empty(expected, scope_pin):
                    return (f"exact pidfd fallback signalled {len(signalled)}; cgroup empty; "
                            "systemctl: " + repr(stop_error))
                for pid in scope_members(expected):
                    before = proc_stat(pid)
                    if before is None:
                        continue
                    details = proc_details(pid)
                    if details.get("vanished"):
                        continue
                    if before["uid"] != os.getuid() or details.get("cgroup") != "0::" + expected:
                        raise RuntimeError("refusing to signal a process outside the exact pinned cgroup")
                    try:
                        pidfd = os.pidfd_open(pid)
                    except ProcessLookupError:
                        continue
                    try:
                        after = proc_stat(pid)
                        if after is None:
                            continue
                        if (after["starttime"] != before["starttime"] or
                            after["uid"] != before["uid"] or proc_details(pid).get("cgroup") != "0::" + expected):
                            raise RuntimeError("process changed identity after pidfd_open: " + str(pid))
                        signal.pidfd_send_signal(pidfd, signal.SIGKILL)
                        signalled.append({"pid": pid, "starttime": before["starttime"]})
                    except ProcessLookupError:
                        pass
                    finally:
                        os.close(pidfd)
                time.sleep(0.05)
            raise RuntimeError("bounded pidfd fallback did not empty exact scope")
        except Exception as fallback_error:
            raise RuntimeError("systemctl failed and exact pidfd fallback failed: " +
                               repr(stop_error) + "; " + repr(fallback_error)) from fallback_error


def validate_unit(unit, expected):
    props = unit_properties(unit)
    if (props.get("Id") != unit or props.get("LoadState") != "loaded" or
        props.get("ActiveState") != "active" or
        props.get("ControlGroup") != expected or
        props.get("MemoryMax") != str(SCOPE_MEMORY) or
        props.get("TasksMax") != str(SCOPE_TASKS) or
        props.get("RuntimeMaxUSec") != SCOPE_RUNTIME or
        props.get("KillMode") != "control-group"):
        raise RuntimeError("unique unit identity, cgroup or caps mismatch: " + repr(props))
    return props


def stop_owned_unit(unit, expected):
    # Never stop by PID/PGID, and never stop an unverified or renamed unit.
    props = unit_properties(unit)
    if props.get("LoadState") == "not-found":
        return "already collected"
    if props.get("Id") != unit or props.get("ControlGroup") != expected:
        raise RuntimeError("refusing to stop unverified unit: " + repr(props))
    subprocess.run([SYSTEMCTL, "--user", "stop", unit], capture_output=True,
                   text=True, check=True, timeout=5)
    return "stopped exact unit"


def _failsafe_watch(read_fd, lease_fd, unit, expected, scope_pin, emergency_path):
    # Keep only exact cgroup pins, heartbeat, and shared lease, not parent
    # logs or SSH descriptors after the controller disappears.
    keep = {read_fd, lease_fd, scope_pin[0], scope_pin[1]}
    for name in os.listdir("/proc/self/fd"):
        fd = int(name)
        if fd not in keep:
            try:
                os.close(fd)
            except OSError:
                pass
    deadline_ns = time.monotonic_ns() + SAMPLE_DEADLINE_NS
    reason = None
    try:
        while True:
            now_ns = time.monotonic_ns()
            readable, _, _ = select.select([read_fd], [], [],
                                            max(0, deadline_ns - now_ns) / 1e9)
            # A queued renewal cannot erase an already-expired prior sample.
            if time.monotonic_ns() >= deadline_ns:
                reason = "oldest resource sample expired before renewal"
                if scope_empty(expected, scope_pin):
                    os._exit(0)
                break
            if readable:
                raw = os.read(read_fd, 4096)
                if raw and len(raw) % 8 == 0:
                    acquired_ns = struct.unpack_from("=Q", raw, len(raw) - 8)[0]
                    now_ns = time.monotonic_ns()
                    if (now_ns < deadline_ns and acquired_ns <= now_ns and
                        now_ns < acquired_ns + SAMPLE_DEADLINE_NS):
                        deadline_ns = acquired_ns + SAMPLE_DEADLINE_NS
                        continue
                reason = "controller closed heartbeat or sent stale/invalid heartbeat"
            else:
                reason = "no controller heartbeat within two seconds"
            if scope_empty(expected, scope_pin):
                os._exit(0)
            break
        stop_started_ns = time.monotonic_ns()
        stop = stop_verified_scope(unit, expected, scope_pin)
        failure = {"status": "failsafe-stopped", "reason": reason,
                   "stop_result": stop, "finished_unix": time.time(),
                   "deadline_monotonic_ns": deadline_ns,
                   "stop_started_monotonic_ns": stop_started_ns,
                   "stop_completed_monotonic_ns": time.monotonic_ns(),
                   "unit": unit, "expected_cgroup": expected}
        with emergency_path.open("x") as stream:
            json.dump(failure, stream, sort_keys=True)
            stream.write("\n")
        os._exit(3)
    except BaseException as exc:
        # Failed receipt writing never turns an exact stop into success.
        try:
            stop = stop_verified_scope(unit, expected, scope_pin)
        except BaseException as stop_exc:
            stop = "exact stop unproven: " + repr(stop_exc)
        try:
            with emergency_path.open("x") as stream:
                json.dump({"status": "failsafe-error", "reason": reason,
                           "error": repr(exc), "stop_result": stop,
                           "finished_unix": time.time()}, stream, sort_keys=True)
                stream.write("\n")
        except OSError:
            pass
        os._exit(4)


def start_failsafe(lease_fd, unit, expected, scope_pin, emergency_path):
    read_fd, write_fd = os.pipe2(os.O_NONBLOCK | os.O_CLOEXEC)
    try:
        pid = os.fork()
    except BaseException:
        os.close(read_fd)
        os.close(write_fd)
        raise
    if pid == 0:
        os.close(write_fd)
        _failsafe_watch(read_fd, lease_fd, unit, expected, scope_pin, emergency_path)
        os._exit(4)
    os.close(read_fd)
    return pid, write_fd


def heartbeat(write_fd, expected, scope_pin, acquired_ns):
    now_ns = time.monotonic_ns()
    if not (0 < acquired_ns <= now_ns < acquired_ns + SAMPLE_DEADLINE_NS):
        raise RuntimeError("oldest resource sample expired before heartbeat")
    try:
        os.write(write_fd, struct.pack("=Q", acquired_ns))
    except BrokenPipeError:
        if not scope_empty(expected, scope_pin):
            raise RuntimeError("independent two-second failsafe exited while scope is populated")


def verify_systemd_tools():
    for path in (SYSTEMD_RUN, SYSTEMCTL):
        stat = pathlib.Path(path).stat()
        if stat.st_uid != 0 or not (stat.st_mode & 0o111):
            raise RuntimeError("reviewed systemd tool unavailable: " + path)
    proof = json.loads((R / "receipts/cgroup-probe-noble-octet-probe-2167032-201986155480.scope.json").read_text())
    if (proof["passed"] is not True or proof["systemd_run_realpath"] != SYSTEMD_RUN or
        proof["systemctl_realpath"] != SYSTEMCTL or
        proof["observed"]["setsid_child_cgroup"] != "0::" + proof["expected_cgroup"] or
        proof["stop_issued_to_exact_unit"] is not True):
        raise RuntimeError("private systemd scope containment proof changed")


def verify_tools():
    allowlist = R / "receipts/build-tool-allowlist.json"
    if sha256(allowlist) != "c23889797d4d6df7691792e4cedda28646e07e9c2df6249288f6287c41d8a2d9":
        raise RuntimeError("reviewed tool allowlist changed")
    with allowlist.open("rb") as stream:
        links = json.load(stream)["links"]
    expected = {item["name"]: item["target"] for item in links if item["name"] != "curl"}
    directory = R / "tool-bin-build"
    if {item.name for item in directory.iterdir()} != set(expected):
        raise RuntimeError("private build-tool PATH inventory changed")
    for name, target in expected.items():
        path = directory / name
        if not path.is_symlink() or os.readlink(path) != target:
            raise RuntimeError("private build-tool symlink changed: " + str(path))
    return {"allowlist_sha256": sha256(allowlist), "tool_names": sorted(expected)}


def verify_stage2(env):
    sysroot = R / "build" / TRIPLE / "stage2"
    rustc = sysroot / "bin/rustc"
    result = subprocess.run([str(rustc), "-vV"], env=env, capture_output=True,
                            text=True, check=True, timeout=20)
    commit = "ee4c488718cf1ca865b8978867a05f8622e3d2a2"
    if "commit-hash: " + commit not in result.stdout.splitlines():
        raise RuntimeError("produced stage2 rustc commit differs from patched source: " + result.stdout)
    lib = sysroot / "lib/rustlib" / TRIPLE / "lib"
    for crate in ("rustc_driver", "rustc_hir", "rustc_interface", "rustc_middle", "rustc_span"):
        if not list(lib.glob("lib" + crate + "-*.rlib")) and not list(lib.glob("lib" + crate + "-*.so")):
            raise RuntimeError("stage2 rustc-dev crate missing: " + crate)
    return {"rustc_vV": result.stdout, "rustc_dev_dir": str(lib)}



def verify_vendor():
    with (R / "receipts/rust-lock-vendor-20261008.json").open("rb") as stream:
        root = json.load(stream)
    with (R / "receipts/rust-union-vendor-20261008.json").open("rb") as stream:
        union = json.load(stream)
    records = [
        (r["name"], r["version"], r["archive_sha256"],
         r["checksum_manifest_sha256"], r["archive_file_count"])
        for r in root["records"]
    ] + [
        (r["name"], r["version"], r["archive_sha256"],
         r["checksum_manifest_sha256"], r["file_count"])
        for r in union["new_records"]
    ]
    if len(records) != 622 or sum(r[4] for r in records) != 33513:
        raise RuntimeError("622 archives and 33513 file receipts mismatch")
    dirs = {p.name for p in VENDOR.iterdir() if p.is_dir() and not p.is_symlink()}
    names = {name + "-" + version for name, version, *_ in records}
    if dirs != names or len(list(VENDOR.iterdir())) != 622:
        raise RuntimeError("private union vendor directories differ from lock receipts")
    archive_cache = R / "cargo-home/registry/cache/index.crates.io-1949cf8c6b5b557f"
    if {p.name for p in archive_cache.iterdir()} != {n + ".crate" for n in names}:
        raise RuntimeError("622 cached crate archives differ from lock receipts")
    files = 0
    for name, version, archive_sha, manifest_sha, count in records:
        stem = name + "-" + version
        if sha256(archive_cache / (stem + ".crate")) != archive_sha:
            raise RuntimeError("registry archive hash mismatch: " + stem)
        directory = VENDOR / stem
        manifest_path = directory / ".cargo-checksum.json"
        if sha256(manifest_path) != manifest_sha:
            raise RuntimeError("vendor checksum manifest mismatch: " + stem)
        with manifest_path.open("rb") as stream:
            manifest = json.load(stream)
        if manifest["package"] != archive_sha or len(manifest["files"]) != count:
            raise RuntimeError("vendor manifest package or count mismatch: " + stem)
        listed = set(manifest["files"])
        actual = set()
        for candidate in directory.rglob("*"):
            if candidate.is_symlink():
                raise RuntimeError("unexpected vendor symlink: " + str(candidate))
            if candidate.is_file() and candidate != manifest_path:
                actual.add(candidate.relative_to(directory).as_posix())
        if actual != listed:
            raise RuntimeError("vendored file inventory mismatch: " + stem)
        for relative, digest in manifest["files"].items():
            if sha256(directory / relative) != digest:
                raise RuntimeError("vendored file hash mismatch: " + stem + "/" + relative)
        files += count
    return {"archive_count": len(records), "verified_extracted_files": files}


def check_source():
    expected = {
        R / "input/rustc-ac7f9.tar.gz": "8b58de6533c40b9a725f3ef151859d63258cd234ef8d5df25d1c51ec0b1ca223",
        R / "input/0001-Track-compiler-owned-HIR-syntax-origin-frames-corrected.patch": "1c95a4cf312c9accf37b0d74ced3543106c6a4d6b7895348df4dc28e9537dd12",
        R / "input/ee4-compiler-changed-20261008.sha256": "c26a556a2d6a4666ad4ba7dfb6ca45bc9af1beedd5e4536cb40da7a3c1154711",
        R / "input/llvm-source-41f177ed26a5252a30ea6090a4da66ce0a96bf44.tar.gz": "5ed059e9e8e6eb770ba8dbfbe9859c8f8a8d4bc1d0c901a0a65ebe53dd4b976a",
        R / "input/octet-sourceonly-61a7483-20261008.tar": "aac2dfc005aad5255c76c939b137b6065541b7b1cdb3f991201a8d342f12f662",
        SRC / "Cargo.lock": "5be4009be1a72be464543653b503f7258dc2e8f9ab938e9b98da19adf00d603c",
        SRC / "src/stage0": "9aa08fe0e0935ef42449eb244f438c5697dbb076091f5168c004b115fdaf781d",
        SRC / ".cargo/config": "e3987c0b3ce7c845d7c8dabd72bf638678ffc481abbd446418e4668d0e248a7e",
        R / "receipts/cgroup-probe-noble-octet-probe-2167032-201986155480.scope.json": "c5b05d81f8f3d3a229e47462907f4969f60a815dc827b7f3bc04c4cd2bc1b09a",
        R / "cargo-home/config.toml": "392773e5a71ac51759f2ebe1123f7f26a1ff4e45b42b2b7af4f12edf737cba74",
        R / "stage0-cache/2026-03-05/rustfmt-nightly-x86_64-unknown-linux-gnu.tar.xz": "bde94b7cf7038ce8e8cf9f0215abe66624a22aa64437c02c72331ea86951c685",
        R / "stage0-cache/2026-03-05/rustc-nightly-x86_64-unknown-linux-gnu.tar.xz": "cda6cb941296a57229610ad32fa48c3df8408e8dddafad621f9f3e663e3b9868",
        R / "receipts/rustfmt-nightly-stage0-20261008.json": "af4a7a55e59a4a2b7f792988fe274317fc92858d9a848fd2a5d9734765a02d89",
        SRC / "src/bootstrap/Cargo.lock": "fa183b6e78a81fbff9d0b064a976311733c49693f57c9a87c524787e23ad8b65",
        SRC / "library/Cargo.lock": "24ee3436f747470180e01a14bb9dea54eba96be15f65ab0407922a7132de84b4",
        R / "source/octet/Cargo.lock": "a8dbe7a5cb70f5e966018f2a0f9e660d888648df9313b23cc385245c00d9fd9f",
    }
    for name, digest in EXPECTED_STAGE0.items():
        tar_name = name + "-beta-" + TRIPLE + ".tar.xz"
        expected[R / "input" / tar_name] = digest
        expected[R / "stage0-cache/2026-03-05" / tar_name] = digest
    expected_input = {path.name for path in expected if path.parent == R / "input"}
    expected_input.add("llvm-commit-41f177ed26a5252a30ea6090a4da66ce0a96bf44.json")
    expected_cache = {path.name for path in expected if path.parent == R / "stage0-cache/2026-03-05"}
    expected_cache.update(("rustc-nightly-" + TRIPLE + ".tar.xz",
                           "rustfmt-nightly-" + TRIPLE + ".tar.xz"))
    for directory, names in ((R / "input", expected_input),
                             (R / "stage0-cache/2026-03-05", expected_cache)):
        if {path.name for path in directory.iterdir()} != names:
            raise RuntimeError("authenticated input archive directory membership changed: " + str(directory))
    actual = {str(path): sha256(path) for path in expected}
    if any(actual[str(path)] != digest for path, digest in expected.items()):
        raise RuntimeError("pinned source/input checksum mismatch")
    manifest = (R / "input/ee4-compiler-changed-20261008.sha256").read_text().splitlines()
    if len(manifest) != 14:
        raise RuntimeError("patched source manifest is incomplete")
    overlays = {
        ".cargo/config": (("file", 0), expected[SRC / ".cargo/config"]),
        "git-commit-info": (("file", 0), hashlib.sha256(
            b"ee4c488718cf1ca865b8978867a05f8622e3d2a2\nee4c48871\n2026-10-07\n").hexdigest()),
        "vendor": (("link", "../../cargo-home/vendor"), None),
    }
    for line in manifest:
        digest, relative = line.split("  ", 1)
        if relative in overlays or not relative.startswith(("compiler/", "tests/ui-fulldeps/")):
            raise RuntimeError("unexpected compiler patch manifest path: " + relative)
        if sha256(SRC / relative) != digest:
            raise RuntimeError("patched ee4 compiler source mismatch: " + relative)
        overlays[relative] = (("file", 0), digest)
    rust_source = verify_source_archive(
        R / "input/rustc-ac7f9.tar.gz", SRC,
        "rust-ac7f9ec7da74d37fd28667c86bf117a39ba5b02a",
        overlays, ("src/llvm-project",))
    llvm_source = verify_source_archive(
        R / "input/llvm-source-41f177ed26a5252a30ea6090a4da66ce0a96bf44.tar.gz",
        SRC / "src/llvm-project",
        "llvm-project-41f177ed26a5252a30ea6090a4da66ce0a96bf44",
        {"clang/bindings/python/.git_archival.txt": (("file", 0),
         "19df19404aeee048e5af1b3e5c8090ad1cebbd3316ceed2c232c9b72adb10169")})
    tools = verify_tools()
    commit_info = "ee4c488718cf1ca865b8978867a05f8622e3d2a2\nee4c48871\n2026-10-07\n"
    if (SRC / "git-commit-info").read_text() != commit_info:
        raise RuntimeError("ee4 bootstrap commit metadata mismatch")
    actual[str(SRC / "git-commit-info")] = sha256(SRC / "git-commit-info")
    git_tree = subprocess.run(
        [str(R / "tool-bin-build/git"), "--git-dir=" + str(R / "receipts/llvm-identity.git"), "write-tree"],
        capture_output=True, text=True, check=True, timeout=20,
    ).stdout.strip()
    if git_tree != "724355ef5fd9cd4557f36b87508339d0a8c5b04c":
        raise RuntimeError("pinned LLVM tree changed")
    llvm_git_entries = verify_llvm_git_tree(SRC / "src/llvm-project", git_tree)
    with open(R / "receipts/rust-lock-vendor-20261008.json", "rb") as stream:
        vendor = json.load(stream)
    if vendor["archive_count"] != 526 or vendor["rust_lock_sha256"] != expected[SRC / "Cargo.lock"]:
        raise RuntimeError("private vendor receipt/lock mismatch")
    if sha256(R / "receipts/rust-lock-vendor-20261008.json") != "810680c7a345e67a0854b04a49d96ba86c5d9e1f2803d88006aa64baa1815327":
        raise RuntimeError("original 526-crate root vendor receipt changed")
    union_path = R / "receipts/rust-union-vendor-20261008.json"
    acquisition_path = R / "receipts/union-crate-acquisition-20261008.json"
    if sha256(union_path) != "91f729b27c837f999f0390abaeb457064a6c8a2e00de35764d3b7bcfc2f5553e":
        raise RuntimeError("622-crate union vendor receipt changed")
    if sha256(acquisition_path) != "0994771ab46304736335c55f6e26e6d2cd0c9bf72b7873147546d4db6dad546c":
        raise RuntimeError("exact-SHA acquisition receipt changed")
    with union_path.open("rb") as stream:
        union = json.load(stream)
    with acquisition_path.open("rb") as stream:
        acquisition = json.load(stream)
    if (union["lock_sha256"] != {"root": expected[SRC / "Cargo.lock"],
                                 "bootstrap": expected[SRC / "src/bootstrap/Cargo.lock"],
                                 "library": expected[SRC / "library/Cargo.lock"]} or
        union["union_archive_count"] != 622 or union["new_archive_count"] != 96 or
        union["previous_root_archive_count"] != 526 or len(union["new_records"]) != 96 or
        acquisition["union_distinct_crates"] != 622 or acquisition["fetched_count"] != 96 or
        acquisition["network_payload_bytes"] > 2 * GIB):
        raise RuntimeError("pinned root/bootstrap/library lock union incomplete")
    if len(list(VENDOR.iterdir())) != 622 or not (VENDOR / "cc-1.2.28/.cargo-checksum.json").is_file():
        raise RuntimeError("private union vendor directory incomplete")
    actual[str(R / "receipts/rust-lock-vendor-20261008.json")] = sha256(R / "receipts/rust-lock-vendor-20261008.json")
    actual[str(union_path)] = sha256(union_path)
    actual[str(acquisition_path)] = sha256(acquisition_path)
    if sha256(CONFIG) != "9d2d9bab255566e848305b6025b40781fe86264d59b4b727f2921cc9beaca097":
        raise RuntimeError("private vendor bootstrap config changed")
    link = SRC / "vendor"
    if (not link.is_symlink() or os.readlink(link) != "../../cargo-home/vendor" or
        link.resolve(strict=True) != VENDOR.resolve(strict=True)):
        raise RuntimeError("compiler source vendor symlink is not the authenticated private union")
    config_dir = SRC / ".cargo"
    root_config = config_dir / "config"
    if (config_dir.is_symlink() or root_config.is_symlink() or
        config_dir.stat().st_uid != os.getuid() or
        root_config.stat().st_uid != os.getuid()):
        raise RuntimeError("source-root Cargo mapping ownership or path changed")
    vendor_binding = verify_vendor()
    actual[str(CONFIG)] = sha256(CONFIG)
    return {"input_sha256": actual, "llvm_git_tree": git_tree,
            "live_rust_source": rust_source, "live_llvm_source": llvm_source,
            "live_llvm_git_entries": llvm_git_entries,
            "private_tool_bindings": tools,
            "ee4_commit": commit_info.splitlines()[0], "vendor_crates": 622,
            "source_vendor_symlink": "../../cargo-home/vendor",
            "source_vendor_resolved": str(VENDOR.resolve(strict=True)),
            **vendor_binding}


def check_config(env):
    with open(CONFIG, "rb") as stream:
        conf = tomllib.load(stream)
    build, llvm, rust = conf["build"], conf["llvm"], conf["rust"]
    if (build["jobs"] != 1 or build["build-stage"] != 2 or build["test-stage"] != 2 or
        build["submodules"] is not False or build["locked-deps"] is not True or build["vendor"] is not True or
        build["patch-binaries-for-nix"] is not False or
        "rustc" in build or "cargo" in build or
        llvm["download-ci-llvm"] is not False or
        rust["download-rustc"] is not False):
        raise RuntimeError("bootstrap config allows substitutions, Nix, or unbounded jobs")
    target = conf["target"][TRIPLE]
    if target["cc"] != GCC + "/cc" or target["cxx"] != GCC + "/c++" or target["linker"] != GCC + "/cc":
        raise RuntimeError("bootstrap native compiler/linker does not match reviewed wrapper")
    if target["ar"] != "/run/current-system/sw/bin/ar" or target["ranlib"] != "/run/current-system/sw/bin/ranlib":
        raise RuntimeError("bootstrap binutils path changed")
    with open(R / "cargo-home/config.toml", "rb") as stream:
        cargo = tomllib.load(stream)
    with (SRC / ".cargo/config").open("rb") as stream:
        root_cargo = tomllib.load(stream)
    if root_cargo != {"net": {"offline": True}}:
        raise RuntimeError("source-root Cargo config is not net-only offline")
    if (cargo["source"]["crates-io"]["replace-with"] != "noble-pinned-vendor" or
        cargo["source"]["noble-pinned-vendor"]["directory"] != str(VENDOR) or
        cargo["net"]["offline"] is not True):
        raise RuntimeError("private Cargo source replacement or offline mode changed")
    if (env["CARGO_HOME"] != str(R / "cargo-home") or env["CARGO_NET_OFFLINE"] != "true" or
        env["RUSTUP_DIST_SERVER"] != "http://127.0.0.1:9" or env["NIX_CONFIG"] != "min-free = 0\nmax-free = 0"):
        raise RuntimeError("offline/private or Nix defense environment changed")
    for tool in ("nix", "nix-build", "nix-shell", "nix-store", "curl"):
        if shutil.which(tool, path=env["PATH"]) is not None:
            raise RuntimeError("prohibited command in launch PATH: " + tool)
    for tool in ("python3", "cmake", "ninja", "git", "cc", "c++", "ld", "ar", "ranlib"):
        if shutil.which(tool, path=env["PATH"]) is None:
            raise RuntimeError("missing offline launch tool: " + tool)


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in {"build", "test", "smoke", "smoke-pidfd", "smoke-threshold", "smoke-ssh"}:
        raise SystemExit("usage: stage2-guard-union-cgroup-v10.py build|test|smoke|smoke-pidfd|smoke-threshold|smoke-ssh")
    action = sys.argv[1]
    os.umask(0o077)
    env = {"HOME": str(pathlib.Path.home()), "USER": "brittonr", "LOGNAME": "brittonr",
           "XDG_RUNTIME_DIR": "/run/user/1555",
           "DBUS_SESSION_BUS_ADDRESS": "unix:path=/run/user/1555/bus",
           "LANG": "en_US.UTF-8", "PATH": PATH,
           "CARGO_HOME": str(R / "cargo-home"), "TMPDIR": str(R / "tmp"),
           "CARGO_NET_OFFLINE": "true", "RUSTUP_DIST_SERVER": "http://127.0.0.1:9",
           "NIX_CONFIG": "min-free = 0\nmax-free = 0", "RUSTC_WRAPPER": "",
           "CARGO_INCREMENTAL": "0", "PYTHONDONTWRITEBYTECODE": "1"}
    if not pathlib.Path(env["TMPDIR"]).is_dir():
        raise RuntimeError("private TMPDIR missing")
    lease_fd = acquire_shared_lease()
    host_pin = pin_host_root()
    operator_control(SANDBOX_EXEC, 0, "file")
    if not os.access(SANDBOX_EXEC, os.X_OK):
        raise RuntimeError("reviewed sandbox launcher is not executable")
    sandbox_sha256 = sha256(SANDBOX_EXEC)
    quota_client_sha256 = sha256(pathlib.Path(quota.__file__))
    private_pins = pin_private_roots()
    binding = check_source()
    check_private_roots(private_pins)
    check_config(env)
    verify_systemd_tools()
    if action in ("build", "test"):
        fallback_probe = R / "receipts/stage2-union-cgroup-v10-smoke-pidfd.watchdog.json"
        with fallback_probe.open("rb") as stream:
            proof = json.load(stream)
        if (proof.get("action") != "smoke-pidfd" or proof.get("status") != "success" or
            proof.get("guard_sha256") != sha256(pathlib.Path(__file__)) or
            proof.get("sandbox_sha256") != sandbox_sha256 or
            proof.get("quota_client_sha256") != quota_client_sha256 or
            not re.search(r"exact pidfd fallback signalled [1-9][0-9]*; cgroup empty",
                          proof.get("stop_result") or "")):
            raise RuntimeError("source-bound exact-scope pidfd fallback is unproven; "
                               "stage2 build/test launch prohibited")
        threshold_probe = R / "receipts/stage2-union-cgroup-v10-smoke-threshold.watchdog.json"
        with threshold_probe.open("rb") as stream:
            threshold_proof = json.load(stream)
        if (threshold_proof.get("action") != "smoke-threshold" or
            threshold_proof.get("status") != "success" or
            threshold_proof.get("guard_sha256") != sha256(pathlib.Path(__file__)) or
            threshold_proof.get("sandbox_sha256") != sandbox_sha256 or
            threshold_proof.get("quota_client_sha256") != quota_client_sha256 or
            threshold_proof.get("reason") != "hard: private >=120GiB" or
            threshold_proof.get("synthetic_sensor", {}).get("injected_private_allocated_bytes") != HARD_PRIVATE or
            "exact cgroup empty" not in (threshold_proof.get("stop_result") or "")):
            raise RuntimeError("source-bound synthetic threshold cleanup is unproven; "
                               "stage2 build/test launch prohibited")
        ssh_probe = R / "receipts/stage2-union-cgroup-v10-smoke-ssh.watchdog.json"
        with ssh_probe.open("rb") as stream:
            ssh_proof = json.load(stream)
        if (ssh_proof.get("action") != "smoke-ssh" or ssh_proof.get("status") != "success" or
            ssh_proof.get("guard_sha256") != sha256(pathlib.Path(__file__)) or
            ssh_proof.get("sandbox_sha256") != sandbox_sha256 or
            ssh_proof.get("quota_client_sha256") != quota_client_sha256 or
            not ssh_proof.get("authenticated_ssh_snapshots") or
            "exact cgroup empty" not in (ssh_proof.get("stop_result") or "")):
            raise RuntimeError("source-bound independent SSH session classification is unproven; "
                               "stage2 build/test launch prohibited")
    if action == "test":
        prior = json.loads((R / "receipts/stage2-union-cgroup-v10-build.watchdog.json").read_text())
        if prior["status"] != "success" or prior.get("ee4_commit") != binding["ee4_commit"] or not prior.get("built_compiler_identity"):
            raise RuntimeError("patched stage2 build/rustc-dev identity not proven")
        verify_stage2(env)
    start_sample = snapshot(private_pins, host_pin, time.monotonic_ns() + SAMPLE_DEADLINE_NS)
    quota_identity = tuple(start_sample["quota_service_identity"])
    if action in ("build", "test"):
        for preceding in (proof, threshold_proof, ssh_proof):
            if tuple(preceding.get("quota_service_identity") or ()) != quota_identity:
                raise RuntimeError("source-bound containment probe used a different quota project or service")
    if action == "test" and (
        prior.get("guard_sha256") != sha256(pathlib.Path(__file__)) or
        prior.get("sandbox_sha256") != sandbox_sha256 or
        prior.get("quota_client_sha256") != quota_client_sha256 or
        tuple(prior.get("quota_service_identity") or ()) != quota_identity
    ):
        raise RuntimeError("stage2 build receipt does not bind this guard, sandbox and quota service")
    if (problem := breach(start_sample)) is not None:
        raise RuntimeError("preflight reserve violated: " + problem)
    if (outside := outside_compilers(set(), "not-started")):
        raise RuntimeError("same-UID compiler/Nix contention before launch: " + repr(outside[:3]))
    baseline = same_uid_processes()
    trusted_ssh = sshd_service_identity()
    controller_cgroup = proc_details(os.getpid()).get("cgroup")
    if controller_cgroup is None:
        raise RuntimeError("guard controller cgroup identity unavailable")
    uid = os.getuid()
    if uid != 1555:
        raise RuntimeError("Bonsai user-scope UID differs from reviewed containment probe")
    nonce = secrets.token_hex(8)
    unit = f"noble-octet-stage2-{action}-{os.getpid()}-{nonce}.scope"
    expected = f"/user.slice/user-{uid}.slice/user@{uid}.service/app.slice/{unit}"
    if unit_properties(unit).get("LoadState") != "not-found":
        raise RuntimeError("unique scope unit already exists")
    gate = R / "tmp" / ("scope-gate-" + nonce)
    if gate.exists() or gate.is_symlink():
        raise RuntimeError("private launch gate already exists")
    if action in ("smoke", "smoke-pidfd", "smoke-threshold", "smoke-ssh"):
        seconds = 20 if action in ("smoke-threshold", "smoke-ssh") else 2
        target = ["/bin/sh", "-c", f"/run/current-system/sw/bin/setsid /run/current-system/sw/bin/sleep {seconds} & wait"]
    else:
        path = "tests/ui-fulldeps/syntax-origin.rs" if action == "test" else "compiler/rustc"
        target = [PYTHON, str(SRC / "x.py"), "--config", str(CONFIG), action, "--stage", "2", path]
    logs = R / "receipts"
    prefix = "stage2-union-cgroup-v10-" + ("smoke-reviewed" if action == "smoke" else
                                         "smoke-pidfd" if action == "smoke-pidfd" else
                                         "smoke-threshold" if action == "smoke-threshold" else
                                         "smoke-ssh" if action == "smoke-ssh" else action)
    stdout_path = logs / (prefix + ".stdout")
    stderr_path = logs / (prefix + ".stderr")
    result_path = logs / (prefix + ".watchdog.json")
    emergency_path = logs / (prefix + ".failsafe.json")
    if any(path.exists() for path in (stdout_path, stderr_path, result_path, emergency_path)):
        raise RuntimeError("refusing to overwrite existing raw logs or watchdog receipt")
    launch = [SYSTEMD_RUN, "--user", "--scope", "--collect", "--expand-environment=no",
              "--unit=" + unit, "--property=MemoryMax=26G", "--property=TasksMax=128",
              "--property=RuntimeMaxSec=" + SCOPE_RUNTIME,
              "--property=KillMode=control-group",
              "--", str(SANDBOX_EXEC), str(gate), str(stdout_path), str(stderr_path), *target]
    started = time.time()
    child = None
    reason = None
    status = "failed"
    verified = False
    launch_sample = None
    scope_pin = None
    samples = []
    failsafe_pid = None
    failsafe_fd = None
    scope_sample = None
    outside_snapshots = []
    authenticated_ssh_snapshots = []
    scope_session = None
    built_identity = None
    stop_result = None
    synthetic_sensor = None
    try:
        with stdout_path.open("xb") as out, stderr_path.open("xb") as err:
            child = subprocess.Popen(launch, cwd=SRC, env=env, stdout=out, stderr=err,
                                     stdin=subprocess.DEVNULL, start_new_session=True)
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                if child.poll() is not None:
                    raise RuntimeError("systemd-run exited before the scope launch gate")
                props = unit_properties(unit)
                if props.get("LoadState") == "loaded":
                    scope_sample = validate_unit(unit, expected)
                    members = scope_members(expected)
                    gated = [pid for pid in members if
                             (info := proc_details(pid)).get("uid") == uid and
                             str(gate) in info.get("cmdline", "") and
                             info.get("cgroup") == "0::" + expected]
                    if len(gated) == 1:
                        scope_pin = pin_scope(unit, expected)
                        scope_session = proc_stat(gated[0])["session"]
                        verified = True
                        initial_watch_ns = time.monotonic_ns()
                        failsafe_pid, failsafe_fd = start_failsafe(
                            lease_fd, unit, expected, scope_pin, emergency_path)
                        watcher = proc_stat(failsafe_pid)
                        if watcher is None or watcher["ppid"] != os.getpid():
                            raise RuntimeError("independent failsafe process was not pinned")
                        baseline[failsafe_pid] = {**watcher, "cgroup": controller_cgroup}
                        initial_expiry_ns = initial_watch_ns + SAMPLE_DEADLINE_NS
                        launch_sample = snapshot(private_pins, host_pin, initial_expiry_ns,
                                                 quota_identity)
                        if (problem := breach(launch_sample)) is not None:
                            raise RuntimeError("launch-gate reserve violated: " + problem)
                        if time.monotonic_ns() >= initial_expiry_ns:
                            raise RuntimeError("launch-gate decision exceeded initial stop deadline")
                        heartbeat(failsafe_fd, expected, scope_pin,
                                  launch_sample["snapshot_started_ns"])
                        sample_expiry_ns = launch_sample["snapshot_started_ns"] + SAMPLE_DEADLINE_NS
                        if scope_empty(expected, scope_pin):
                            raise RuntimeError("scope exited before guarded launch gate")
                        gate.write_text("go\n")
                        print("SCOPE_READY unit=" + unit + " gated_pid=" + str(gated[0]), flush=True)
                        break
                time.sleep(0.05)
            else:
                raise RuntimeError("scope identity/membership gate timed out")
            while True:
                tick = time.monotonic()
                try:
                    sample = snapshot(private_pins, host_pin, sample_expiry_ns, quota_identity)
                    if action == "smoke-threshold" and synthetic_sensor is None:
                        members = scope_members(expected)
                        sleeping = [pid for pid in members if
                                    (info := proc_details(pid)).get("comm") == "sleep" and
                                    info.get("cgroup") == "0::" + expected]
                        if sleeping:
                            validate_unit(unit, expected)
                            synthetic_sensor = {
                                "actual_private_allocated_bytes": sample["private_allocated_bytes"],
                                "injected_private_allocated_bytes": HARD_PRIVATE,
                                "scoped_sleep_pid": sleeping[0],
                            }
                            sample = {**sample, "private_allocated_bytes": HARD_PRIVATE}
                    samples.append({"time": time.time(), **sample,
                                    "synthetic_sensor": synthetic_sensor if action == "smoke-threshold" else None})
                    if (reason := breach(sample)) is not None:
                        break
                    props = unit_properties(unit)
                    if props.get("LoadState") == "loaded":
                        validate_unit(unit, expected)
                        members = scope_members(expected)
                        if not members and child.poll() is None:
                            reason = "active scope lost all tracked processes"
                            break
                    else:
                        members = set()
                        if child.poll() is None:
                            reason = "scope vanished before launcher exited"
                            break
                    if (outside := outside_compilers(members, expected)):
                        outside_snapshots.extend(outside)
                        owned = any(any(ancestor.get("pid") in members for ancestor in row["ancestry"][1:]) for row in outside)
                        reason = ("owned descendant escaped unique cgroup: " if owned else
                                  "external/ambiguous same-UID compiler contention: ") + repr([x["process"]["pid"] for x in outside])
                        break
                    if (outside := outside_descendants(
                            members, expected, baseline, scope_session, controller_cgroup, trusted_ssh,
                            authenticated_ssh_snapshots)):
                        outside_snapshots.extend(outside)
                        reason = "new outside-scope same-UID process (possible unnamed escaped descendant): " + repr(
                            [x["process"]["pid"] for x in outside])
                        break
                    if action == "smoke-ssh" and authenticated_ssh_snapshots:
                        reason = "authenticated independent SSH login while scoped sleep active"
                        if not any(proc_details(pid).get("comm") == "sleep" for pid in members):
                            raise RuntimeError("SSH observation did not overlap the scoped sleep")
                        stop_result = stop_verified_scope(unit, expected, scope_pin)
                        status = "success"
                        break
                    if action in ("smoke", "smoke-pidfd"):
                        escaped_pgrp = [pid for pid in members if
                                        (info := proc_details(pid)).get("comm") == "sleep" and
                                        info.get("session") == pid and info.get("pgrp") == pid and
                                        info.get("cgroup") == "0::" + expected]
                        if escaped_pgrp:
                            scope_sample = {**scope_sample, "setsid_child_pid": escaped_pgrp[0],
                                            "same_scope_members": sorted(members)}
                            reason = "verified setsid child retained in unique scope"
                            stop_result = stop_verified_scope(
                                unit, expected, scope_pin, force_fallback=action == "smoke-pidfd")
                            if action == "smoke-pidfd" and not re.search(
                                    r"exact pidfd fallback signalled [1-9][0-9]*; cgroup empty", stop_result):
                                raise RuntimeError("harmless fallback did not exercise pidfd signals")
                            status = "success"
                            break
                    if child.poll() is not None:
                        if child.returncode != 0:
                            reason = "scoped command exited nonzero: " + str(child.returncode)
                        elif action == "build":
                            built_identity = verify_stage2(env)
                            status = "success"
                        elif action == "test":
                            with stdout_path.open("rb") as stream:
                                raw_out = stream.read().decode("utf-8", "replace")
                            with stderr_path.open("rb") as stream:
                                raw_err = stream.read().decode("utf-8", "replace")
                            if re.search(r"test result: ok[^\n]*1 passed; 0 failed; 0 ignored", raw_out + raw_err):
                                status = "success"
                            else:
                                reason = "compiletest did not report actual 1 passed, 0 ignored"
                        else:
                            reason = "smoke exited without scoped setsid proof"
                        break
                    if time.monotonic_ns() >= sample_expiry_ns:
                        raise RuntimeError("complete scope decision exceeded previous sample expiry")
                    heartbeat(failsafe_fd, expected, scope_pin,
                              sample["snapshot_started_ns"])
                    sample_expiry_ns = sample["snapshot_started_ns"] + SAMPLE_DEADLINE_NS
                except Exception as exc:
                    reason = "scope watchdog monitoring failure: " + repr(exc)
                    break
                cadence = 0.05 if action in ("smoke", "smoke-pidfd") else 1.0
                remaining_ns = sample_expiry_ns - time.monotonic_ns()
                if remaining_ns <= 0:
                    raise RuntimeError("resource sample expired before next scope tick")
                time.sleep(max(0, min(cadence - (time.monotonic() - tick),
                                      remaining_ns / 2e9)))
            if status != "success" and verified:
                stop_result = stop_verified_scope(unit, expected, scope_pin)
            if child.poll() is None:
                child.wait(timeout=8)
            if (action == "smoke-threshold" and reason == "hard: private >=120GiB" and
                synthetic_sensor is not None and stop_result is not None and
                "exact cgroup empty" in stop_result and child.returncode is not None):
                status = "success"
            if status == "success" and action not in ("smoke", "smoke-pidfd", "smoke-threshold", "smoke-ssh"):
                if unit_properties(unit).get("LoadState") != "not-found":
                    reason = "scoped command exited with remaining unit processes"
                    status = "failed"
                    stop_result = stop_verified_scope(unit, expected, scope_pin)
            if status == "success":
                check_private_roots(private_pins)
                if (check_source() != binding or
                    sha256(pathlib.Path(quota.__file__)) != quota_client_sha256 or
                    sha256(SANDBOX_EXEC) != sandbox_sha256):
                    status = "failed"
                    reason = "post-run source, sandbox, sensor or toolchain binding changed"
                check_private_roots(private_pins)
    except BaseException as exc:
        reason = reason or "scope guard exception: " + repr(exc)
        status = "failed"
        if verified:
            try:
                stop_result = stop_verified_scope(unit, expected, scope_pin)
                if child is not None and child.poll() is None:
                    child.wait(timeout=8)
            except Exception as cleanup_exc:
                reason += "; exact-unit stop failure: " + repr(cleanup_exc)
    finally:
        if failsafe_fd is not None:
            os.close(failsafe_fd)
        if failsafe_pid is not None:
            deadline_ns = time.monotonic_ns() + 8_000_000_000
            while True:
                done, wait_status = os.waitpid(failsafe_pid, os.WNOHANG)
                if done:
                    if os.waitstatus_to_exitcode(wait_status) != 0:
                        status = "failed"
                        reason = (reason or "") + "; independent two-second failsafe intervened"
                    break
                if time.monotonic_ns() >= deadline_ns:
                    status = "failed"
                    reason = (reason or "") + "; independent failsafe did not finish within 8s"
                    break
                time.sleep(0.05)
        if scope_pin is not None:
            os.close(scope_pin[1])
            os.close(scope_pin[0])
        if gate.exists() and not gate.is_symlink():
            gate.unlink()
    receipt = {"action": action, "status": status, "reason": reason,
               "target_command": target, "scope_command": launch, "cwd": str(SRC),
               "guard_sha256": sha256(pathlib.Path(__file__)),
               "sandbox_sha256": sandbox_sha256,
               "quota_client_sha256": quota_client_sha256,
               "quota_service_identity": quota_identity,
               "global_host_root_pin": {"path": str(HOST_FS), "st_dev": host_pin[0],
                                        "st_ino": host_pin[1]},
               "independent_failsafe_pid": failsafe_pid,
               "failsafe_receipt_path": str(emergency_path),
               "unit": unit, "expected_cgroup": expected,
               "scope_caps": {"MemoryMax_bytes": SCOPE_MEMORY, "TasksMax": SCOPE_TASKS},
               "scope_start_properties": scope_sample, "stop_result": stop_result,
               "systemd_run_pid": child.pid if child else None,
               "systemd_run_exit_code": child.returncode if child else None,
               "started_unix": started, "finished_unix": time.time(),
               "preflight_sample": start_sample,
               "launch_gate_sample": launch_sample,
               "final_sample": samples[-1] if samples else None,
               "synthetic_sensor": synthetic_sensor,
               "sample_count": len(samples), "sample_deadline_ns": SAMPLE_DEADLINE_NS,
               "snapshot_start_gaps_ns": [
                   later["snapshot_started_ns"] - earlier["snapshot_started_ns"]
                   for earlier, later in zip(
                       [launch_sample, *samples[:-1]], samples)
               ] if launch_sample is not None else [],
               "outside_process_snapshots": outside_snapshots,
               "authenticated_ssh_snapshots": authenticated_ssh_snapshots,
               "soft_thresholds_bytes": [SOFT_PRIVATE, SOFT_FREE, SOFT_MEMORY],
               "hard_thresholds_bytes": [HARD_PRIVATE, HARD_FREE, HARD_MEMORY],
               "env_guards": {k: env[k] for k in ("CARGO_HOME", "CARGO_NET_OFFLINE",
                              "RUSTUP_DIST_SERVER", "NIX_CONFIG", "TMPDIR", "RUSTC_WRAPPER")},
               "built_compiler_identity": built_identity, **binding,
               "stdout_path": str(stdout_path), "stdout_sha256": sha256(stdout_path),
               "stderr_path": str(stderr_path), "stderr_sha256": sha256(stderr_path)}
    with result_path.open("x") as stream:
        json.dump(receipt, stream, sort_keys=True, indent=2)
        stream.write("\n")
    print("SCOPE_FINISHED action=" + action + " status=" + status + " reason=" + repr(reason), flush=True)
    return 0 if status == "success" else 2


if __name__ == "__main__":
    raise SystemExit(main())
