/* Read-only, fixed-target ext4 project-quota sensor for an operator-owned service.
 * Build with reviewed -DSTAGE2_* pins; never grant capabilities to a user-owned
 * binary.  The service uses only quota GET operations and FS_IOC_FSGETXATTR.
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/dqblk_xfs.h>
#include <linux/fs.h>
#include <linux/magic.h>
#include <linux/quota.h>
#include <signal.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/statfs.h>
#include <sys/syscall.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

#if !defined(STAGE2_ROOT) || !defined(STAGE2_SOCKET) || \
    !defined(STAGE2_PROJECT_ID) || !defined(STAGE2_OWNER_UID) || \
    !defined(STAGE2_ROOT_DEV) || !defined(STAGE2_ROOT_INO)
#error "Pin STAGE2_ROOT, STAGE2_SOCKET, STAGE2_PROJECT_ID, STAGE2_OWNER_UID, STAGE2_ROOT_DEV and STAGE2_ROOT_INO at build time"
#endif
#if STAGE2_PROJECT_ID <= 0 || STAGE2_OWNER_UID <= 0
#error "A dedicated nonzero project ID and unprivileged owner UID are required"
#endif

#define SOFT_BYTES (90ULL * 1024 * 1024 * 1024)
#define HARD_BYTES (120ULL * 1024 * 1024 * 1024)
#define REPLY_MAGIC 0x3151534eU /* NSQ1 in little-endian */
#define PROJECT_FLAGS (FS_QUOTA_PDQ_ACCT | FS_QUOTA_PDQ_ENFD)

/* Wire format is explicitly little-endian on this pinned x86_64 target. */
struct __attribute__((packed)) quota_reply {
    uint32_t magic, status, project_id, flags;
    uint64_t nonce, root_dev, root_ino, used_bytes, soft_bytes, hard_bytes;
    uint64_t started_ns, finished_ns;
};
_Static_assert(sizeof(struct quota_reply) == 80, "quota wire layout changed");

static uint64_t monotonic_ns(void) {
    struct timespec ts;
    if (clock_gettime(CLOCK_MONOTONIC, &ts)) return 0;
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

static int state(int fd, uint32_t *flags) {
    struct fs_quota_statv info = {.qs_version = FS_QSTATV_VERSION1};
    /* quotactl_fd expects QCMD(op,type); the unused id argument is zero. */
    if (syscall(SYS_quotactl_fd, fd, QCMD(Q_XGETQSTATV, PRJQUOTA), 0, &info) < 0)
        return errno;
    *flags = info.qs_flags;
    return ((*flags & PROJECT_FLAGS) == PROJECT_FLAGS) ? 0 : ENODATA;
}

static int read_quota(int fd, struct quota_reply *reply) {
    struct stat st;
    struct statfs fs;
    struct fsxattr attr;
    struct if_dqblk q = {0};
    uint32_t before, after;
    int err;

    if (fstat(fd, &st) || fstatfs(fd, &fs)) return errno;
    if (st.st_dev != (dev_t)STAGE2_ROOT_DEV ||
        st.st_ino != (ino_t)STAGE2_ROOT_INO ||
        !S_ISDIR(st.st_mode) || st.st_uid != (uid_t)STAGE2_OWNER_UID ||
        fs.f_type != EXT4_SUPER_MAGIC)
        return ESTALE;
    if (ioctl(fd, FS_IOC_FSGETXATTR, &attr) < 0) return errno;
    if (attr.fsx_projid != STAGE2_PROJECT_ID ||
        !(attr.fsx_xflags & FS_XFLAG_PROJINHERIT)) return ESTALE;
    if ((err = state(fd, &before))) return err;
    if (syscall(SYS_quotactl_fd, fd, QCMD(Q_GETQUOTA, PRJQUOTA),
                STAGE2_PROJECT_ID, &q) < 0) return errno;
    if ((q.dqb_valid & (QIF_BLIMITS | QIF_SPACE)) !=
        (QIF_BLIMITS | QIF_SPACE)) return ENODATA;
    if (q.dqb_bsoftlimit != SOFT_BYTES / QIF_DQBLKSIZE ||
        q.dqb_bhardlimit != HARD_BYTES / QIF_DQBLKSIZE ||
        q.dqb_curspace > HARD_BYTES) return ERANGE;
    if ((err = state(fd, &after))) return err;
    reply->flags = before & after;
    reply->root_dev = st.st_dev;
    reply->root_ino = st.st_ino;
    reply->used_bytes = q.dqb_curspace; /* generic ext4 includes reservations */
    reply->soft_bytes = SOFT_BYTES;
    reply->hard_bytes = HARD_BYTES;
    return 0;
}

static int ancestors_are_protected(const char *path) {
    char prefix[sizeof(((struct sockaddr_un *)0)->sun_path)];
    struct stat st;
    size_t len = strlen(path);
    if (path[0] != '/' || len >= sizeof(prefix) || strstr(path, "//"))
        return 0;
    for (size_t i = 1; i < len; ++i) {
        if (path[i] != '/') continue;
        memcpy(prefix, path, i);
        prefix[i] = '\0';
        if (lstat(prefix, &st) || !S_ISDIR(st.st_mode) ||
            st.st_uid != 0 || (st.st_mode & 022))
            return 0;
    }
    return true;
}

static volatile sig_atomic_t stopped;
static void shutdown_signal(int sig) { (void)sig; stopped = 1; }

int main(void) {
    int root_fd = -1, listen_fd = -1, rc = 1;
    struct sockaddr_un addr = {.sun_family = AF_UNIX};
    struct quota_reply probe = {0};
    struct stat st, socket_pin;
    struct sigaction sa = {.sa_handler = shutdown_signal};
    bool bound = false;
    const char *root = STAGE2_ROOT, *socket_path = STAGE2_SOCKET;
    if (geteuid() != 0 || strlen(socket_path) >= sizeof(addr.sun_path) ||
        !ancestors_are_protected(socket_path)) {
        fputs("quota service requires root identity and protected socket ancestors\n", stderr);
        return 1;
    }
    root_fd = open(root, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (root_fd < 0) goto done;
    /* An unprovisioned backend must not even advertise a usable socket. */
    if ((errno = read_quota(root_fd, &probe)) != 0) {
        goto done;
    }
    /* Refuse to replace a socket left by another service instance. */
    if (lstat(socket_path, &st) == 0 || errno != ENOENT) goto done;
    listen_fd = socket(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0);
    if (listen_fd < 0) goto done;
    memcpy(addr.sun_path, socket_path, strlen(socket_path) + 1);
    umask(077);
    if (bind(listen_fd, (struct sockaddr *)&addr,
             (socklen_t)(offsetof(struct sockaddr_un, sun_path) + strlen(socket_path) + 1)))
        goto done;
    if (lstat(socket_path, &socket_pin)) goto done;
    bound = true;
    if (chown(socket_path, STAGE2_OWNER_UID, -1) || chmod(socket_path, 0600) ||
        listen(listen_fd, 4) || sigemptyset(&sa.sa_mask) ||
        sigaction(SIGTERM, &sa, NULL) || sigaction(SIGINT, &sa, NULL))
        goto done;
    while (!stopped) {
        int conn = accept4(listen_fd, NULL, NULL, SOCK_CLOEXEC);
        struct ucred peer;
        socklen_t peer_len = sizeof(peer);
        struct timeval io_timeout = {.tv_sec = 0, .tv_usec = 250000};
        uint64_t nonce;
        struct iovec payload = {.iov_base = &nonce, .iov_len = sizeof(nonce)};
        struct msghdr request = {.msg_iov = &payload, .msg_iovlen = 1};
        struct quota_reply reply = {.magic = REPLY_MAGIC,
                                    .project_id = STAGE2_PROJECT_ID};
        if (conn < 0) {
            if (errno == EINTR) continue;
            break;
        }
        if (setsockopt(conn, SOL_SOCKET, SO_RCVTIMEO, &io_timeout, sizeof(io_timeout)) ||
            setsockopt(conn, SOL_SOCKET, SO_SNDTIMEO, &io_timeout, sizeof(io_timeout)) ||
            getsockopt(conn, SOL_SOCKET, SO_PEERCRED, &peer, &peer_len) ||
            peer_len != sizeof(peer) || peer.uid != STAGE2_OWNER_UID ||
            recvmsg(conn, &request, MSG_TRUNC) != sizeof(nonce) ||
            (request.msg_flags & MSG_TRUNC)) {
            close(conn);
            continue;
        }
        reply.nonce = nonce;
        reply.started_ns = monotonic_ns();
        reply.status = (uint32_t)read_quota(root_fd, &reply);
        reply.finished_ns = monotonic_ns();
        (void)send(conn, &reply, sizeof(reply), MSG_NOSIGNAL);
        close(conn);
    }
    rc = 0;
done:
    if (listen_fd >= 0) close(listen_fd);
    if (root_fd >= 0) close(root_fd);
    /* Only remove the exact socket this instance created, never another owner. */
    if (bound && lstat(socket_path, &st) == 0 && S_ISSOCK(st.st_mode) &&
        st.st_dev == socket_pin.st_dev && st.st_ino == socket_pin.st_ino)
        (void)unlink(socket_path);
    if (rc) perror("quota service");
    return rc;
}
