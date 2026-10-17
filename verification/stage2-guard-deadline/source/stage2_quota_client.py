"""Read the operator-owned, fixed-project ext4 quota service. Never estimate usage."""
import fcntl
import os
import pathlib
import secrets
import socket
import stat
import struct
import time

SOCKET = pathlib.Path("/run/noble-octet-stage2/quota.sock")
SOFT_BYTES = 90 * 1024**3
HARD_BYTES = 120 * 1024**3
PROJECT_FLAGS = (1 << 4) | (1 << 5)
REPLY = struct.Struct("<IIIIQQQQQQQQ")
REQUEST = struct.Struct("<Q")
MAGIC = 0x3151534E
MAX_AGE_NS = 2_000_000_000
FS_IOC_FSGETXATTR = 0x801C581F  # Linux x86_64 _IOR('X',31,struct fsxattr[28])
FS_XFLAG_PROJINHERIT = 0x00000200


class QuotaUnavailable(RuntimeError):
    """A quota sample cannot safely authorize a scoped process."""


def verify_directories(paths, project_id):
    """Ensure every pinned private root has this project and inheritance."""
    for path in paths:
        try:
            fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
            try:
                xflags, _, _, found_id, _, _ = struct.unpack(
                    "<IIIII8s", fcntl.ioctl(fd, FS_IOC_FSGETXATTR, bytes(28)))
            finally:
                os.close(fd)
        except OSError as exc:
            raise QuotaUnavailable("cannot read private project: " + str(path)) from exc
        if found_id != project_id or not xflags & FS_XFLAG_PROJINHERIT:
            raise QuotaUnavailable("private directory not inheriting pinned project: " + str(path))


def _verify_socket_path(path):
    if not path.is_absolute():
        raise QuotaUnavailable("quota socket path is not absolute")
    for ancestor in path.parents:
        info = ancestor.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != 0 or info.st_mode & 0o022:
            raise QuotaUnavailable("quota socket ancestor is writable or replaced")
    node = path.lstat()
    if (not stat.S_ISSOCK(node.st_mode) or
        node.st_uid != os.getuid() or node.st_mode & 0o077):
        raise QuotaUnavailable("quota socket or parent ownership/mode changed")
    return node.st_dev, node.st_ino


def snapshot(root, deadline_ns, *, identity=None, socket_path=SOCKET):
    """Get one fresh reservation-inclusive count before an absolute deadline.

    `socket_path` is only for an unprivileged local negative-path fixture; the
    production guard always calls this with its fixed operator-owned path.
    """
    root = pathlib.Path(root)
    now = time.monotonic_ns()
    if deadline_ns <= now:
        raise QuotaUnavailable("quota sampling deadline already expired")
    try:
        socket_pin = _verify_socket_path(pathlib.Path(socket_path))
        root_pin = root.lstat()
        if not stat.S_ISDIR(root_pin.st_mode) or root_pin.st_uid != os.getuid():
            raise QuotaUnavailable("private root is not owned directory")
        nonce = secrets.randbits(64)
        with socket.socket(socket.AF_UNIX, socket.SOCK_SEQPACKET) as channel:
            channel.settimeout((deadline_ns - time.monotonic_ns()) / 1e9)
            channel.connect(str(socket_path))
            peer_pid, peer_uid, peer_gid = struct.unpack(
                "=iii", channel.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))
            if peer_uid != 0 or peer_pid < 1:
                raise QuotaUnavailable("quota service is not root-owned")
            channel.settimeout(max(0, deadline_ns - time.monotonic_ns()) / 1e9)
            channel.sendall(REQUEST.pack(nonce))
            channel.settimeout(max(0, deadline_ns - time.monotonic_ns()) / 1e9)
            raw = channel.recv(REPLY.size + 1)
        received_ns = time.monotonic_ns()
        if received_ns > deadline_ns or len(raw) != REPLY.size:
            raise QuotaUnavailable("quota reply missed deadline or has wrong size")
        if _verify_socket_path(pathlib.Path(socket_path)) != socket_pin:
            raise QuotaUnavailable("quota service socket replaced during sample")
        root_after = root.lstat()
        if (root_after.st_dev, root_after.st_ino, root_after.st_uid) != (
                root_pin.st_dev, root_pin.st_ino, root_pin.st_uid):
            raise QuotaUnavailable("private root changed during quota sample")
        (magic, status, project_id, flags, returned_nonce, root_dev, root_ino,
         used, soft, hard, started_ns, finished_ns) = REPLY.unpack(raw)
        if magic != MAGIC or returned_nonce != nonce or project_id == 0:
            raise QuotaUnavailable("quota response identity/nonce mismatch")
        if status:
            raise QuotaUnavailable("kernel quota query failed: " + os.strerror(status))
        if (flags & PROJECT_FLAGS) != PROJECT_FLAGS or soft != SOFT_BYTES or hard != HARD_BYTES:
            raise QuotaUnavailable("project quota limits or enforcement changed")
        if (root_dev, root_ino) != (root_pin.st_dev, root_pin.st_ino) or used > hard:
            raise QuotaUnavailable("project quota root identity or usage invalid")
        if not (now <= started_ns <= finished_ns <= received_ns) or received_ns - started_ns > MAX_AGE_NS:
            raise QuotaUnavailable("quota result is stale or has nonmonotonic timestamps")
        service_identity = (project_id, socket_pin[0], socket_pin[1], peer_pid)
        if identity is not None and service_identity != identity:
            raise QuotaUnavailable("pinned project or quota service identity changed")
        sample = {"private_allocated_bytes": used, "private_sample_monotonic_ns": started_ns,
                "private_project_id": project_id, "quota_root_dev": root_dev,
                "quota_root_ino": root_ino, "quota_flags": flags,
                "quota_soft_bytes": soft, "quota_hard_bytes": hard,
                "quota_service_identity": service_identity}
        decided_ns = time.monotonic_ns()
        if decided_ns > deadline_ns or decided_ns - started_ns > MAX_AGE_NS:
            raise QuotaUnavailable("quota sample aged out during postvalidation")
        sample["private_sample_age_seconds"] = (decided_ns - started_ns) / 1e9
        return sample
    except (OSError, OverflowError, ValueError, struct.error) as exc:
        raise QuotaUnavailable("quota service unavailable: " + repr(exc)) from exc
