/* Unprivileged read-only diagnostic. Never sets limits or changes a project. */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/dqblk_xfs.h>
#include <linux/quota.h>
#include <stdio.h>
#include <string.h>
#include <sys/syscall.h>
#include <unistd.h>

int main(int argc, char **argv) {
    struct fs_quota_statv state = {.qs_version = FS_QSTATV_VERSION1};
    struct if_dqblk quota = {0};
    int fd, state_error = 0, usage_error = 0;
    if (argc != 3) {
        fprintf(stderr, "usage: %s DIRECTORY PROJECT_ID\n", argv[0]);
        return 2;
    }
    int project_id;
    if (sscanf(argv[2], "%d", &project_id) != 1 || project_id <= 0)
        return 2;
    fd = open(argv[1], O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (fd < 0) { perror("open quota directory"); return 2; }
    if (syscall(SYS_quotactl_fd, fd, QCMD(Q_XGETQSTATV, PRJQUOTA), 0, &state) < 0)
        state_error = errno;
    if (syscall(SYS_quotactl_fd, fd, QCMD(Q_GETQUOTA, PRJQUOTA), project_id, &quota) < 0)
        usage_error = errno;
    close(fd);
    printf("Q_XGETQSTATV encoded_op=%#x status=%d:%s project_flags=%#x\n",
           QCMD(Q_XGETQSTATV, PRJQUOTA), state_error, strerror(state_error), state.qs_flags);
    printf("Q_GETQUOTA encoded_op=%#x status=%d:%s valid=%#x used=%llu soft_blocks=%llu hard_blocks=%llu\n",
           QCMD(Q_GETQUOTA, PRJQUOTA), usage_error, strerror(usage_error), quota.dqb_valid,
           (unsigned long long)quota.dqb_curspace,
           (unsigned long long)quota.dqb_bsoftlimit,
           (unsigned long long)quota.dqb_bhardlimit);
    return state_error || usage_error ? 1 : 0;
}
