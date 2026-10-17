#ifndef STAGE2_SANDBOX_POLICY_H
#define STAGE2_SANDBOX_POLICY_H

/* In a fresh unprivileged user/mount/net/IPC namespace, recursively make all
 * existing mounts read-only except the exact private root's writable bind.
 * Drops all capabilities and sets no_new_privs. Irreversible; fail closed.
 */
int stage2_enter_private_write_view(const char *private_root);

/* Only succeeds after stage2_enter_private_write_view in this process.
 * Installs inherited Landlock filesystem/IPC policy and seccomp denials.
 * The caller must first close/audit every inherited writable descriptor.
 */
int stage2_install_sandbox(const char *private_root);

#endif
