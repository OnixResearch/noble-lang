/* Bounded non-root exercise of the production Landlock+seccomp policy only.
 * It does NOT prove ext4 quota accounting or the privileged helper.
 */
#define _GNU_SOURCE
#include "stage2_sandbox_policy.h"
#include <errno.h>
#include <fcntl.h>
#include <linux/fs.h>
#include <sched.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/msg.h>
#include <sys/shm.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>

int main(int argc, char **argv) {
    int fd;
    char outside_dir[4096], *slash;
    struct stat outside_stat;
    struct fsxattr unchanged;
    if (argc < 4) {
        fputs("usage: fixture deny|compile|private-metadata|procfd PRIVATE_DIR OUTSIDE_FILE [ARGS]\n", stderr);
        return 2;
    }
    if (stage2_enter_private_write_view(argv[2]) ||
        stage2_install_sandbox(argv[2])) {
        perror("install scoped sandbox");
        return 2;
    }
    if (!strcmp(argv[1], "private-metadata")) {
        struct stat owned;
        if (argc != 5) return 2;
        fd = open(argv[4], O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
        if (fd < 0 || fstat(fd, &owned) ||
            fchmod(fd, owned.st_mode & 07777) || fchown(fd, 0, 0)) {
            perror("private chmod/chown");
            return 1;
        }
        close(fd);
        puts("PRIVATE_METADATA_OK");
        return 0;
    }
    if (!strcmp(argv[1], "procfd")) {
        if (argc != 5) return 2;
        fd = open(argv[4], O_WRONLY | O_CLOEXEC);
        if (fd >= 0) {
            close(fd);
            fputs("foreign process writable FD escaped confinement\n", stderr);
            return 1;
        }
        if (errno != EACCES && errno != EPERM && errno != EROFS) {
            perror("unexpected procfd refusal");
            return 1;
        }
        puts("FOREIGN_PROC_FD_DENIED");
        return 0;
    }
    if (!strcmp(argv[1], "compile")) {
        if (argc != 6) return 2;
        char *const args[] = {"cc", "-c", argv[4], "-o", argv[5], NULL};
        execvp("cc", args);
        perror("exec compiler under sandbox");
        return 2;
    }
    if (strcmp(argv[1], "deny") || argc != 4) return 2;
    fd = open(argv[3], O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
    if (fd >= 0) {
        close(fd);
        (void)unlink(argv[3]); /* Only our just-created O_EXCL file. */
        fputs("outside file creation escaped Landlock\n", stderr);
        return 1;
    }
    if (errno != EACCES && errno != EPERM && errno != EROFS) return 1;
    if (strlen(argv[3]) >= sizeof(outside_dir)) return 2;
    strcpy(outside_dir, argv[3]);
    slash = strrchr(outside_dir, '/');
    if (!slash || slash == outside_dir) return 2;
    *slash = '\0';
    fd = open(outside_dir, O_RDONLY | O_DIRECTORY | O_CLOEXEC);
    if (fd < 0 || fstat(fd, &outside_stat)) return 2;
    if (fchmod(fd, outside_stat.st_mode & 07777) != -1 || errno != EROFS) {
        fputs("outside inode metadata escaped read-only mount\n", stderr);
        close(fd);
        return 1;
    }
    if (fchown(fd, 0, 0) != -1 || errno != EROFS) {
        fputs("outside inode ownership escaped read-only mount\n", stderr);
        close(fd);
        return 1;
    }
    close(fd);
    if (syscall(SYS_shmctl, -1, IPC_STAT, NULL) != -1 || errno != EPERM ||
        syscall(SYS_msgctl, -1, IPC_STAT, NULL) != -1 || errno != EPERM) {
        fputs("SysV IPC escaped seccomp\n", stderr);
        return 1;
    }
    fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd >= 0) { close(fd); fputs("socket escaped seccomp\n", stderr); return 1; }
    if (errno != EPERM) return 1;
    if (unshare(CLONE_NEWUSER) != -1 || errno != EPERM) {
        fputs("namespace escaped seccomp\n", stderr);
        return 1;
    }
    fd = open(argv[2], O_RDONLY | O_DIRECTORY | O_CLOEXEC);
    if (fd < 0) return 1;
    memset(&unchanged, 0, sizeof(unchanged));
    if (ioctl(fd, FS_IOC_FSSETXATTR, &unchanged) != -1 || errno != EPERM) {
        fputs("project retag ioctl escaped seccomp\n", stderr);
        close(fd);
        return 1;
    }
    close(fd);
    puts("LANDLOCK_SECCOMP_DENY_OK");
    return 0;
}
