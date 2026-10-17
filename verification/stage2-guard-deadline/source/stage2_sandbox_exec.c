/* Run inside the unique verified scope; no compiler starts until confinement. */
#define _GNU_SOURCE
#include "stage2_sandbox_policy.h"
#include <errno.h>
#include <fcntl.h>
#include <linux/fs.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <time.h>
#include <unistd.h>

#if !defined(STAGE2_ROOT) || !defined(STAGE2_PROJECT_ID) || \
    !defined(STAGE2_OWNER_UID) || !defined(STAGE2_ROOT_DEV) || \
    !defined(STAGE2_ROOT_INO)
#error "The operator must pin the exact private root, project, UID, device and inode"
#endif
#if STAGE2_PROJECT_ID <= 0 || STAGE2_OWNER_UID <= 0
#error "The private project and owner UID must be nonzero"
#endif

static int checked_project_fd(int fd, bool root, uid_t expected_uid) {
    struct stat st;
    struct fsxattr attr;
    if (fstat(fd, &st) || ioctl(fd, FS_IOC_FSGETXATTR, &attr)) return -1;
    if (st.st_dev != (dev_t)STAGE2_ROOT_DEV ||
        st.st_uid != expected_uid ||
        (root ? (!S_ISDIR(st.st_mode) || st.st_ino != (ino_t)STAGE2_ROOT_INO ||
                 !(attr.fsx_xflags & FS_XFLAG_PROJINHERIT)) :
                !S_ISREG(st.st_mode)) ||
        attr.fsx_projid != STAGE2_PROJECT_ID) {
        errno = ESTALE;
        return -1;
    }
    return 0;
}

static int checked_log_fd(int fd, const char *path) {
    struct stat opened, inherited;
    const char prefix[] = STAGE2_ROOT "/receipts/stage2-union-cgroup-v10-";
    if (strncmp(path, prefix, sizeof(prefix) - 1)) {
        errno = EINVAL;
        return -1;
    }
    int source = open(path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
    if (source < 0) return -1;
    int result = fstat(source, &opened) || fstat(fd, &inherited) ||
        opened.st_dev != inherited.st_dev || opened.st_ino != inherited.st_ino ||
        checked_project_fd(source, false, STAGE2_OWNER_UID) ||
        checked_project_fd(fd, false, STAGE2_OWNER_UID);
    close(source);
    if (result) errno = ESTALE;
    return result ? -1 : 0;
}

static int wait_gate(const char *name) {
    struct timespec start, now, pause = {.tv_nsec = 20 * 1000 * 1000};
    size_t prefix = strlen(STAGE2_ROOT);
    if (strncmp(name, STAGE2_ROOT "/tmp/scope-gate-", prefix +
                sizeof("/tmp/scope-gate-") - 1)) {
        errno = EINVAL;
        return -1;
    }
    if (clock_gettime(CLOCK_MONOTONIC, &start)) return -1;
    do {
        int fd = open(name, O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
        if (fd >= 0) {
            struct stat st;
            char value[4];
            ssize_t n = read(fd, value, sizeof(value));
            bool valid = fstat(fd, &st) == 0 && S_ISREG(st.st_mode) &&
                st.st_uid == (uid_t)STAGE2_OWNER_UID &&
                st.st_dev == (dev_t)STAGE2_ROOT_DEV && n == 3 &&
                memcmp(value, "go\n", 3) == 0;
            close(fd);
            if (!valid) { errno = EACCES; return -1; }
            return 0;
        }
        if (errno != ENOENT) return -1;
        nanosleep(&pause, NULL);
        if (clock_gettime(CLOCK_MONOTONIC, &now)) return -1;
    } while (now.tv_sec - start.tv_sec < 10);
    errno = ETIMEDOUT;
    return -1;
}

int main(int argc, char **argv) {
    int root = -1;
    struct stat null_node, input;
    if (argc < 5) { errno = EINVAL; goto fail; }
    if (geteuid() != STAGE2_OWNER_UID) { errno = EPERM; goto fail; }
    if (wait_gate(argv[1])) goto fail;
    root = open(STAGE2_ROOT, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (root < 0 || checked_project_fd(root, true, STAGE2_OWNER_UID) ||
        checked_log_fd(STDOUT_FILENO, argv[2]) ||
        checked_log_fd(STDERR_FILENO, argv[3]) ||
        fstat(STDIN_FILENO, &input) || stat("/dev/null", &null_node) ||
        !S_ISCHR(input.st_mode) || input.st_rdev != null_node.st_rdev)
        goto fail;
    close(root);
    root = -1;
    /* No inherited network socket, writable foreign FD, or quota helper FD. */
    if (syscall(SYS_close_range, 3, ~0U, 0) ||
        stage2_enter_private_write_view(STAGE2_ROOT)) goto fail;
    root = open(STAGE2_ROOT, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (root < 0 || checked_project_fd(root, true, 0)) goto fail;
    close(root);
    root = -1;
    if (stage2_install_sandbox(STAGE2_ROOT)) goto fail;
    execv(argv[4], argv + 4);
fail:
    { int saved = errno;
      if (root >= 0) close(root);
      errno = saved;
      perror("scoped write boundary failed closed"); }
    return 111;
}
