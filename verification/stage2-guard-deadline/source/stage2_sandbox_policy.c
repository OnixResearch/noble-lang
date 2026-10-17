/* Linux x86_64: restrict the scoped compiler before exec, not systemd-run.
 * No untrusted inherited writable FDs are allowed (enforced by the caller).
 */
#define _GNU_SOURCE
#include "stage2_sandbox_policy.h"
#include <errno.h>
#include <fcntl.h>
#include <linux/audit.h>
#include <linux/capability.h>
#include <linux/filter.h>
#include <linux/landlock.h>
#include <linux/mount.h>
#include <linux/seccomp.h>
#include <stddef.h>
#include <sched.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <sys/mount.h>
#include <sys/prctl.h>
#include <sys/statvfs.h>
#include <sys/syscall.h>
#include <unistd.h>

#ifndef __x86_64__
#error "Review syscall numbers and BPF architecture before using on another ABI"
#endif

#define DENY_SYSCALL(number) \
    BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, number, 0, 1), \
    BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EPERM)

static int write_map(const char *path, const char *value) {
    int fd = open(path, O_WRONLY | O_CLOEXEC);
    if (fd < 0) return -1;
    size_t len = strlen(value);
    ssize_t count = write(fd, value, len);
    int saved = errno;
    close(fd);
    if (count != (ssize_t)len) { errno = count < 0 ? saved : EIO; return -1; }
    return 0;
}

static int write_view_ready;

int stage2_enter_private_write_view(const char *private_root) {
    char map[96];
    struct mount_attr ro = {.attr_set = MOUNT_ATTR_RDONLY};
    struct statvfs outside, inside;
    struct __user_cap_header_struct header = {.version = _LINUX_CAPABILITY_VERSION_3};
    struct __user_cap_data_struct empty_caps[2] = {{0}, {0}};
    uid_t host_uid = getuid();
    gid_t host_gid = getgid();
    if (write_view_ready || !private_root || private_root[0] != '/' ||
        host_uid == 0) { errno = EPERM; return -1; }
    if (unshare(CLONE_NEWUSER)) return -1;
    if (write_map("/proc/self/setgroups", "deny\n")) return -1;
    snprintf(map, sizeof(map), "0 %u 1\n", (unsigned)host_uid);
    if (write_map("/proc/self/uid_map", map)) return -1;
    snprintf(map, sizeof(map), "0 %u 1\n", (unsigned)host_gid);
    if (write_map("/proc/self/gid_map", map)) return -1;
    if (unshare(CLONE_NEWNS | CLONE_NEWNET | CLONE_NEWIPC) ||
        mount(NULL, "/", NULL, MS_REC | MS_PRIVATE, NULL) ||
        mount(private_root, private_root, NULL, MS_BIND, NULL) ||
        syscall(SYS_mount_setattr, AT_FDCWD, "/", AT_RECURSIVE, &ro, sizeof(ro)) ||
        mount(NULL, private_root, NULL, MS_BIND | MS_REMOUNT, NULL) ||
        statvfs("/", &outside) || statvfs(private_root, &inside))
        return -1;
    if (!(outside.f_flag & ST_RDONLY) || (inside.f_flag & ST_RDONLY)) {
        errno = EROFS;
        return -1;
    }
    if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) ||
        prctl(PR_CAP_AMBIENT, PR_CAP_AMBIENT_CLEAR_ALL, 0, 0, 0) ||
        syscall(SYS_capset, &header, empty_caps)) return -1;
    write_view_ready = 1;
    return 0;
}

static int deny_escape_syscalls(void) {
    const uint32_t new_ns = CLONE_NEWNS | CLONE_NEWUTS | CLONE_NEWIPC |
                            CLONE_NEWUSER | CLONE_NEWPID | CLONE_NEWNET |
                            CLONE_NEWCGROUP;
    struct sock_filter program[] = {
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, arch)),
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, AUDIT_ARCH_X86_64, 1, 0),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, nr)),
        /* The x32 syscall bit shares the architecture ID: reject it. */
        BPF_JUMP(BPF_JMP | BPF_JGE | BPF_K, 0x40000000, 0, 1),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EPERM),
        /* Ordinary fork/threads work; no clone() namespaces. */
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, __NR_clone, 0, 4),
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, args[0])),
        BPF_STMT(BPF_ALU | BPF_AND | BPF_K, new_ns),
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, 0, 1, 0),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EPERM),
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, nr)),
        /* Prevent project retagging and other mutation/device ioctls. */
        DENY_SYSCALL(__NR_ioctl),
        DENY_SYSCALL(__NR_quotactl),
        DENY_SYSCALL(__NR_quotactl_fd),
        /* All mounts except the private writable bind are read-only, which
         * covers metadata mutation not mediated by Landlock write rights.
         * This permits chmod/utimensat of actual private compiler artifacts.
         * No SysV/POSIX IPC channels to an actor outside this scope. */
        DENY_SYSCALL(__NR_shmget),
        DENY_SYSCALL(__NR_shmat),
        DENY_SYSCALL(__NR_shmdt),
        DENY_SYSCALL(__NR_shmctl),
        DENY_SYSCALL(__NR_semget),
        DENY_SYSCALL(__NR_semop),
        DENY_SYSCALL(__NR_semtimedop),
        DENY_SYSCALL(__NR_semctl),
        DENY_SYSCALL(__NR_msgget),
        DENY_SYSCALL(__NR_msgsnd),
        DENY_SYSCALL(__NR_msgrcv),
        DENY_SYSCALL(__NR_msgctl),
        DENY_SYSCALL(__NR_mq_open),
        DENY_SYSCALL(__NR_mq_unlink),
        DENY_SYSCALL(__NR_mq_timedsend),
        DENY_SYSCALL(__NR_mq_timedreceive),
        DENY_SYSCALL(__NR_mq_notify),
        DENY_SYSCALL(__NR_mq_getsetattr),
        DENY_SYSCALL(__NR_add_key),
        DENY_SYSCALL(__NR_request_key),
        DENY_SYSCALL(__NR_keyctl),
        /* No new outward service/network IPC. socketpair stays intra-scope. */
        DENY_SYSCALL(__NR_socket),
        DENY_SYSCALL(__NR_connect),
        DENY_SYSCALL(__NR_bind),
        DENY_SYSCALL(__NR_listen),
        DENY_SYSCALL(__NR_accept),
        DENY_SYSCALL(__NR_accept4),
        DENY_SYSCALL(__NR_sendto),
        DENY_SYSCALL(__NR_sendmsg),
        DENY_SYSCALL(__NR_sendmmsg),
        DENY_SYSCALL(__NR_unshare),
        DENY_SYSCALL(__NR_setns),
        DENY_SYSCALL(__NR_mount),
        DENY_SYSCALL(__NR_umount2),
        DENY_SYSCALL(__NR_pivot_root),
        DENY_SYSCALL(__NR_chroot),
        DENY_SYSCALL(__NR_open_tree),
        DENY_SYSCALL(__NR_move_mount),
        DENY_SYSCALL(__NR_fsopen),
        DENY_SYSCALL(__NR_fsconfig),
        DENY_SYSCALL(__NR_fspick),
        DENY_SYSCALL(__NR_fsmount),
        DENY_SYSCALL(__NR_mount_setattr),
        DENY_SYSCALL(__NR_bpf),
        DENY_SYSCALL(__NR_ptrace),
        DENY_SYSCALL(__NR_process_vm_writev),
        DENY_SYSCALL(__NR_pidfd_getfd),
        DENY_SYSCALL(__NR_pidfd_send_signal),
        /* Force libc's older clone fallback; never accept unchecked clone3 flags. */
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, __NR_clone3, 0, 1),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | ENOSYS),
        DENY_SYSCALL(__NR_io_uring_setup),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
    };
    struct sock_fprog fprog = {.len = sizeof(program) / sizeof(program[0]),
                               .filter = program};
    return prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &fprog);
}

int stage2_install_sandbox(const char *private_root) {
    int ruleset = -1, root_fd = -1, null_fd = -1, abi, saved;
    const uint64_t writes = LANDLOCK_ACCESS_FS_WRITE_FILE |
        LANDLOCK_ACCESS_FS_REMOVE_DIR | LANDLOCK_ACCESS_FS_REMOVE_FILE |
        LANDLOCK_ACCESS_FS_MAKE_CHAR | LANDLOCK_ACCESS_FS_MAKE_DIR |
        LANDLOCK_ACCESS_FS_MAKE_REG | LANDLOCK_ACCESS_FS_MAKE_SOCK |
        LANDLOCK_ACCESS_FS_MAKE_FIFO | LANDLOCK_ACCESS_FS_MAKE_BLOCK |
        LANDLOCK_ACCESS_FS_MAKE_SYM | LANDLOCK_ACCESS_FS_REFER |
        LANDLOCK_ACCESS_FS_TRUNCATE;
    struct landlock_ruleset_attr attrs = {
        .handled_access_fs = writes,
        .scoped = LANDLOCK_SCOPE_ABSTRACT_UNIX_SOCKET | LANDLOCK_SCOPE_SIGNAL,
    };
    struct landlock_path_beneath_attr path = {.allowed_access = writes &
        ~(LANDLOCK_ACCESS_FS_MAKE_CHAR | LANDLOCK_ACCESS_FS_MAKE_BLOCK)};
    if (!write_view_ready) { errno = EPERM; return -1; }
    abi = syscall(SYS_landlock_create_ruleset, NULL, 0, LANDLOCK_CREATE_RULESET_VERSION);
    if (abi < 6) { errno = EOPNOTSUPP; return -1; }
    ruleset = syscall(SYS_landlock_create_ruleset, &attrs, sizeof(attrs), 0);
    if (ruleset < 0) return -1;
    root_fd = open(private_root, O_PATH | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (root_fd < 0) goto fail;
    path.parent_fd = root_fd;
    if (syscall(SYS_landlock_add_rule, ruleset, LANDLOCK_RULE_PATH_BENEATH, &path, 0))
        goto fail;
    /* Toolchains can explicitly open /dev/null for output. No other device. */
    null_fd = open("/dev/null", O_PATH | O_NOFOLLOW | O_CLOEXEC);
    if (null_fd < 0) goto fail;
    path.parent_fd = null_fd;
    path.allowed_access = LANDLOCK_ACCESS_FS_WRITE_FILE;
    if (syscall(SYS_landlock_add_rule, ruleset, LANDLOCK_RULE_PATH_BENEATH, &path, 0))
        goto fail;
    if (prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) ||
        syscall(SYS_landlock_restrict_self, ruleset, 0) ||
        deny_escape_syscalls()) goto fail;
    close(null_fd);
    close(root_fd);
    close(ruleset);
    return 0;
fail:
    saved = errno;
    if (null_fd >= 0) close(null_fd);
    if (root_fd >= 0) close(root_fd);
    if (ruleset >= 0) close(ruleset);
    errno = saved;
    return -1;
}
