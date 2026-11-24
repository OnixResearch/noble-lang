# HOLD / UNREVIEWED PIN PACKET: C Backend Native Toolchain & QEMU TCG Lane (`CB-88`)

**Status:** HOLD — Unreviewed source-only pin packet (`OWNER DECISION 2026-11-20`, Decision 5).
This packet inventories candidate `octet/nixpkgs` pins, reproducible static-ELF build rules, a bounded QEMU TCG VM profile, and the 8 mandatory negative controls (`CB-NC-01..08`). It does **not** select these pins in `policy/tool-selection*`, admit a native compiler or sandbox runner, authorize `--native` or `execute --target c` (both continue to refuse), or promote any `CB-CASE-01..10` scenario.

Related records:
- Machine-readable inventory: [pins.json](pins.json)
- Design specification & negative-control baseline: [design.md](../../.cairn/changes/c-backend/design.md)
- Active task ledger: [tasks.md](../../.cairn/changes/c-backend/tasks.md)
- Prior canonical-edit review packet: [review-packet.md](../../.cairn/changes/c-backend/review-packet.md)
- Generated canonical spec view: [C-BACKEND.md](../../specs/C-BACKEND.md)
- Conformance scenario designs: [c-backend-cases.json](../../specs/conformance/c-backend-cases.json)
- Source-only emitter & path integrity: [c_backend.rs](../../crates/noble-cli/src/c_backend.rs), [inputs.rs](../../crates/noble-cli/src/c_backend/inputs.rs)

---

## 1. Pinned `octet/nixpkgs` Provenance & Divergence Findings

All proposed tool paths derive from the repository's existing `octet/nixpkgs` flake input (`flake.lock` / `policy/tool-selection.json`):
- **Owner / Repo / Ref:** `NixOS/nixpkgs` (`nixpkgs-unstable`)
- **Revision:** `9cf7092bdd603554bd8b63c216e8943cf9b12512`
- **NAR Hash:** `sha256-9tpvMGFteZnd3gRQZFlRCohVpqooygFuy9yjuyRL2C0=`
- **Target Architecture:** `x86_64-unknown-linux-gnu`

Observed divergences from non-pin host/policy paths:
1. **`policy/tool-selection.json` `linker_bin` mismatch:** `policy/tool-selection.json` records `/nix/store/788mx070y81zjlg5ipcl0cra3afviw9k-gcc-wrapper-15.2.0/bin` (`glibc-2.42-61`, unbound), whereas `octet/nixpkgs` (`9cf7092bdd603554bd8b63c216e8943cf9b12512`) evaluates `pkgs.stdenv.cc` to `/nix/store/kbw2j1vag664b3sj3rjwz9v53cqx87sb-gcc-wrapper-15.2.0` (`glibc-2.42-51`). `CB-88` pins the `octet/nixpkgs` derivations exclusively.
2. **Host QEMU discrepancy:** The historical host path noted in [design.md](../../.cairn/changes/c-backend/design.md) (`/nix/store/7yilgp2sawl51a36g2si9nnwfrliplxz-qemu-11.1.1/bin/qemu-system-x86_64`) is absent on the build host, and `/run/current-system/sw/bin/qemu-system-x86_64` resolves to an unpinned `qemu-11.1.2`. Neither is used; `CB-88` pins `pkgs.qemu` (`qemu-10.2.1`) from `octet/nixpkgs`.

---

## 2. Candidate Derivation & Store Path Inventory

Derivations with `realized: false` have verified `.drv` files in `/nix/store` from offline `octet/nixpkgs` evaluation, but have **not** been built/cached on disk yet; all entries (including `realized: true`) retain `sha256: null` until realized and formally measured.

| Role | `octet/nixpkgs` Attr | Package / Version | `.drv` Path | Output Path | Realized |
|---|---|---|---|---|---|
| Primary C11 Compiler (musl static) | `pkgs.pkgsStatic.stdenv.cc` | `x86_64-unknown-linux-musl-gcc-wrapper-15.2.0` | `/nix/store/l5pd5fv6yc38acziszv6za5rdinx9xwn-x86_64-unknown-linux-musl-gcc-wrapper-15.2.0.drv` | `/nix/store/nz3n5wy8zadq73kkql2xn485l7xdmv46-x86_64-unknown-linux-musl-gcc-wrapper-15.2.0` | `false` |
| Unwrapped GCC (musl static) | `pkgs.pkgsStatic.stdenv.cc.cc` | `x86_64-unknown-linux-musl-gcc-15.2.0` | `/nix/store/kd7882f717fza93pia2h3f4aj1hyqg3s-x86_64-unknown-linux-musl-gcc-15.2.0.drv` | `/nix/store/l04z26p43pa121dbfp2x3zqzkvha33m2-x86_64-unknown-linux-musl-gcc-15.2.0` | `false` |
| Static Linker Wrapper (musl) | `pkgs.pkgsStatic.stdenv.cc.bintools` | `x86_64-unknown-linux-musl-binutils-wrapper-2.44` | `/nix/store/s502ihsw0yvy2h9k579andx3i710cjib-x86_64-unknown-linux-musl-binutils-wrapper-2.44.drv` | `/nix/store/h0pbmj3hw62blpqfwzi7lvq2jkridpxv-x86_64-unknown-linux-musl-binutils-wrapper-2.44` | `false` |
| Unwrapped Static Linker (musl) | `pkgs.pkgsStatic.stdenv.cc.bintools.bintools_bin` | `x86_64-unknown-linux-musl-binutils-2.44` | `/nix/store/7f8kpxqy56laahcs3vimmcxw47by97sz-x86_64-unknown-linux-musl-binutils-2.44.drv` | `/nix/store/6iy5zryd4b8hmwpj9mky2by0ajardffk-x86_64-unknown-linux-musl-binutils-2.44` | `false` |
| Static musl Runtime / Dev | `pkgs.pkgsStatic.musl` | `musl-static-x86_64-unknown-linux-musl-1.2.5` | `/nix/store/xs7qzjhlqqkxkzlb83nh9bw5vgl2nrbg-musl-static-x86_64-unknown-linux-musl-1.2.5.drv` | `/nix/store/05c4skaigahk0yj3z1graj7837c7qm10-musl-static-x86_64-unknown-linux-musl-1.2.5` (`dev`: `/nix/store/rbzk87ywizm0zl9i9hnza1wibjpkhiv5-musl-static-x86_64-unknown-linux-musl-1.2.5-dev`) | `false` |
| Linux Kernel Headers | `pkgs.linuxHeaders` | `linux-headers-6.18.7` | `/nix/store/6ghrbyhhcdadw3l329i1kx1yncwgw094-linux-headers-6.18.7.drv` | `/nix/store/rvpvd6srf1rzkmza1vxbyjcv5c5nawhc-linux-headers-6.18.7` | `true` |
| Host Reference Compiler (glibc) | `pkgs.stdenv.cc` | `gcc-wrapper-15.2.0` | `/nix/store/l8kjykqqnkh48cxjhz1p9wh53mb65dpf-gcc-wrapper-15.2.0.drv` | `/nix/store/kbw2j1vag664b3sj3rjwz9v53cqx87sb-gcc-wrapper-15.2.0` | `true` |
| Host Reference Unwrapped GCC (glibc) | `pkgs.stdenv.cc.cc` | `gcc-15.2.0` | `/nix/store/s17kqafdpsrgbfygig72hgv04vr905yw-gcc-15.2.0.drv` | `/nix/store/sca0pf46jmxva40qahkcwys5c1lvk6n2-gcc-15.2.0` | `true` |
| Host Reference Linker (glibc) | `pkgs.stdenv.cc.bintools` | `binutils-wrapper-2.44` | `/nix/store/94q8md08ip92h7575ln820fzlz0mkp4n-binutils-wrapper-2.44.drv` | `/nix/store/4yi6jj75bb5hhdzpzlxfyf69d35wsf2x-binutils-wrapper-2.44` | `true` |
| Host Reference Unwrapped Linker (glibc) | `pkgs.stdenv.cc.bintools.bintools_bin` | `binutils-2.44` | `/nix/store/ibgm6y86w54yc8hhljfxqwc24ncghqa0-binutils-2.44.drv` | `/nix/store/9nmzd62x45ayp4vmswvn6z45h6bzrsla-binutils-2.44` | `true` |
| Host Reference glibc Static | `pkgs.glibc` | `glibc-2.42-51` | `/nix/store/zf7wrvp7ab5nq6x505nphqjyzv9l28dp-glibc-2.42-51.drv` | `/nix/store/l0l2ll1lmylczj1ihqn351af2kyp5x19-glibc-2.42-51` (`dev`: `/nix/store/j8irrc0mpx029dw0rmadsjylg7h31ync-glibc-2.42-51-dev`, `static`: `/nix/store/dra84lgfpv7lkbq1lz8ngc2sczqnd0js-glibc-2.42-51-static`) | `true` |
| QEMU (full TCG + system) | `pkgs.qemu` | `qemu-10.2.1` | `/nix/store/gw0qjz3ard5zv3csmvhfr8vw6zn0cgw7-qemu-10.2.1.drv` | `/nix/store/0ksasy9cgh24srwfp5lqhgvxm1wal71q-qemu-10.2.1` | `false` |
| QEMU (host CPU only reference) | `pkgs.qemu_kvm` | `qemu-host-cpu-only-10.2.1` | `/nix/store/1nbzjzb4byap2vfz4q6bxh7vjn94744f-qemu-host-cpu-only-10.2.1.drv` | `/nix/store/9h1x6kz210ky9m2ggqfmp0qjx26q6b6s-qemu-host-cpu-only-10.2.1` | `false` |
| SeaBIOS Firmware | `pkgs.seabios` | `seabios-1.17.0` | `/nix/store/m84g9iv2lvyjz0irlp5r7539g3m136nk-seabios-1.17.0.drv` | `/nix/store/xj7g1d1j96cnx6bfvb8bz25v1hxp13sc-seabios-1.17.0` | `false` |
| qboot Firmware (optional) | `pkgs.qboot` | `qboot-unstable-2020-04-23` | `/nix/store/rrb30a4xgsmnc0iywdsjb16830kz80iy-qboot-unstable-2020-04-23.drv` | `/nix/store/5fillk2kix84lbbj70gl6yqzfsabd6z6-qboot-unstable-2020-04-23` | `false` |
| Guest Linux Kernel (default) | `pkgs.linux` | `linux-6.18.18` | `/nix/store/nvzfbg5wi8c0fg230rr9l5bhqzxpif2r-linux-6.18.18.drv` | `/nix/store/hwyx2l9wi4wxsxgkd6yrvs566xpkqdkr-linux-6.18.18` | `false` |
| Guest Linux Kernel (LTS option) | `pkgs.linuxPackages_6_12.kernel` | `linux-6.12.77` | `/nix/store/xc494j1cg9ydjf02am8lr5qjf83zqvrb-linux-6.12.77.drv` | `/nix/store/ilf76d574r5lya0gbrh0jq0kbwp42kap-linux-6.12.77` | `false` |
| Static BusyBox (initrd/rootfs) | `pkgs.pkgsStatic.busybox` | `busybox-static-x86_64-unknown-linux-musl-1.37.0` | `/nix/store/qvzbb5f9jkgiaa88k9wzs044rq8k1gci-busybox-static-x86_64-unknown-linux-musl-1.37.0.drv` | `/nix/store/pic5whsc7f5f7ji3c019pfhhacfr8niq-busybox-static-x86_64-unknown-linux-musl-1.37.0` | `false` |
| initrd `cpio` | `pkgs.cpio` | `cpio-2.15` | `/nix/store/hf8avhllvhf7m3j0vpyi2i6waxdshmc0-cpio-2.15.drv` | `/nix/store/c4xn0489g92bkqmdir5pndrfxl922f2k-cpio-2.15` | `true` |
| initrd `gzip` | `pkgs.gzip` | `gzip-1.14` | `/nix/store/lia9cp27x0s3f9axsjndxi0hxzzjabdn-gzip-1.14.drv` | `/nix/store/w1n7yp2vnldr395hbwbcaw9sflh413bm-gzip-1.14` | `true` |
| initrd `zstd` | `pkgs.zstd` | `zstd-1.5.7` | `/nix/store/blcaz0716y1nk9syz2fb9acwq6s1bd2h-zstd-1.5.7.drv` | `/nix/store/xks3v41w0rqp27a17kwkybcvr39rxpl8-zstd-1.5.7-bin` | `true` |
| Host Outer Confinement | `pkgs.bubblewrap` | `bubblewrap-0.11.0` | `/nix/store/3ml43jyisk1cf03qi1nbhvfhzs39vlky-bubblewrap-0.11.0.drv` | `/nix/store/qn1pfnn2xgfwdr6avw1fnvwsabi83973-bubblewrap-0.11.0` | `true` |
| Host Seccomp Filter Library | `pkgs.libseccomp` | `libseccomp-2.6.0` | `/nix/store/gbnkb0cajg2xcw6za5hl64m4d08d920n-libseccomp-2.6.0.drv` | `/nix/store/2k7zcnnsix84r1vgaf621h3l0hdq4b14-libseccomp-2.6.0` | `true` |

`pkgs.pkgsStatic.stdenv.cc` (`musl-1.2.5`) is the recommended guest ELF compiler because it produces static ELFs with zero `PT_INTERP`, zero dynamic loader, and zero glibc NSS `dlopen` surface; `pkgs.stdenv.cc` + `pkgs.glibc.static` (`glibc-2.42-51`) is recorded as the realized `octet/nixpkgs` reference toolchain.

---

## 3. Reproducible Static-ELF Build Procedure (Proposed)

Under a future bounded, networkless compiler service (never invoked by current `build --target c`):

1. **Sanitized build environment:**
   `env -i PATH=<PINNED_TOOLCHAIN_BIN> LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0 ZERO_AR_DATE=1`
2. **Exact C11 static compilation flags (run for both `-O0` and `-O2`):**
   ```text
   <PINNED_CC> -std=c11 -static -O0 \
     -fno-asynchronous-unwind-tables -fno-ident -frandom-seed=noble-c11-v1 \
     -ffile-prefix-map=<WORK_DIR>=/noble-src -fdebug-prefix-map=<WORK_DIR>=/noble-src \
     -Wall -Wextra -Werror \
     -Wl,--build-id=none -Wl,-z,noexecstack -Wl,-z,relro -Wl,-z,now \
     <WORK_DIR>/artifact.c -o <WORK_DIR>/artifact.O0.elf
   ```
   Repeat with `-O2` for `<WORK_DIR>/artifact.O2.elf`.
3. **Full-ELF reproducibility & structural admission checks (`CB-ADMIT-01`):**
   - Compile each optimization mode twice from distinct work directories and assert full-ELF SHA-256 byte equality (including inert sections).
   - Verify ELF64 `EM_X86_64`, zero `PT_INTERP` headers, zero `PT_DYNAMIC` / `DT_NEEDED` entries, zero `PF_W | PF_X` segments, and non-executable `PT_GNU_STACK`.
   - Snapshot the admitted ELF into a sealed read-only descriptor; launch from that sealed identity rather than re-opening an output pathname.

---

## 4. QEMU TCG VM & Host Confinement Profile (Proposed)

To evaluate a virtualized deterministic clock/hardware boundary under `CB-SANDBOX-01`:

1. **QEMU launch arguments (`qemu-10.2.1` `bin/qemu-system-x86_64`):**
   - Machine & CPU: `-machine q35,accel=tcg,usb=off,vmport=off,smm=off -cpu qemu64 -smp 1,sockets=1,cores=1,threads=1`
   - Deterministic TCG & RTC flags: `-icount shift=5,sleep=off,align=off -rtc base=2026-01-01T00:00:00,clock=vm`
   - Bounded resources & headless defaults: `-m 128M -no-user-config -nodefaults -nographic -no-reboot`
   - Kernel & read-only initramfs: `-kernel <KERNEL_BZIMAGE> -initrd <INITRD_IMAGE> -append "console=ttyS0 panic=1 oops=panic quiet nokaslr vsyscall=none init=/init"`
2. **Enumerated allowed devices:**
   - `isa-serial` (`ttyS0` console)
   - `virtio-serial-pci` / `virtserialport` (bounded private request/response IPC FD only, when mediator is enabled)
3. **Explicitly forbidden flags & devices:**
   - `-enable-kvm`, `/dev/kvm`
   - `shift=auto`, `sleep=on`, `align=on`, `clock=host`
   - `-net`, `-netdev`, `virtio-net`, `-fsdev`, `virtio-9p`, `virtiofs`, writable `-drive`, `spice`, `vnc`, `audio`, `usb`
4. **Host confinement of the QEMU process (`bubblewrap-0.11.0` + `libseccomp-2.6.0`):**
   - Verify mount, user, and network namespaces actually exist (fail closed; never rely on `--unshare-*-try`, `--share-net`, or `--not-a-security-boundary`).
   - Close all unrelated FDs; pass only explicit private protocol and output FDs.
   - Apply a measured, fail-closed post-load seccomp allowlist denying subsequent `execve`/`execveat`, network, arbitrary file opens, and namespace changes.
   - Enforce parent-owned cgroup memory/CPU/task/output caps and monotonic wall-clock timeout with full process-tree kill and reap.

---

## 5. Negative Control Matrix (`CB-NC-01..08`)

Every control below is **not executed** in Iteration 1 and MUST be verified before any native GO decision:

| ID | Negative Control (not executed) | Insufficient Shortcut | Required Gate | Spec / Case Mapping |
|---|---|---|---|---|
| `CB-NC-01` | Cached `bubblewrap`/`libseccomp` or the Lean proof sandbox | Package availability is not native runner selection | Reviewed separate default-disabled C policy, exact pins and owner approval | `CB-SANDBOX-01` / `CB-CASE-08`, `CB-CASE-10` |
| `CB-NC-02` | `../`, ancestor symlink, hardlink or swapped parent | `canonicalize`, final-component `O_NOFOLLOW` and pathname digest race | Descriptor-anchored no-symlink walk and sealed byte snapshot | `CB-ADMIT-01` / `CB-CASE-05` |
| `CB-NC-03` | Forged C comment or ELF inert-section change | Syntax, manifest flag or `.text` hash cannot establish native correspondence | Trusted C re-emission and reproducible full-ELF byte equality | `CB-ADMIT-01` / `CB-CASE-05` |
| `CB-NC-04` | `PT_INTERP`, `LD_PRELOAD`, writable loader or executable mapping | Outside code runs despite admitted ELF bytes | Reviewed static startup or exact immutable loader/ABI closure and post-load filter | `CB-SANDBOX-01`, `CB-ADMIT-01` / `CB-CASE-08` |
| `CB-NC-05` | Deny all `execve` before initial load, or allow it afterward | First prevents launch; second permits substitution | Trusted initial-load versus final no-exec stage before guest instructions | `CB-SANDBOX-01` / `CB-CASE-08` |
| `CB-NC-06` | vDSO `clock_gettime`/`time`/`gettimeofday`, coarse clocks, direct vvar reads, x86 `rdtsc`/`rdtscp` | All bypass syscall `seccomp`/bubblewrap namespace flags; TSC-only trap/offset leaves host time or elapsed time | Reviewed complete clock/vvar/TSC barrier or equivalent; otherwise no native launch | `CB-SANDBOX-01` / `CB-CASE-08` |
| `CB-NC-07` | Inherited secret FD, `/proc/self/fd` or revoked `test.emit` grant | Direct disclosure or forged/stale protected request | Fixed private FD allowlist, bounded IPC, independent mediator checking current authority on every request | `CB-HOST-01`, `CB-SANDBOX-01` / `CB-CASE-07`, `CB-CASE-08` |
| `CB-NC-08` | Crash, timeout, leaked descendant or partial JSON | Partial output is not Noble completion | Parent-owned cgroup limits, kill/reap entire worker set and typed failure without post-failure effects | `CB-LIMIT-01`, `CB-SANDBOX-01` / `CB-CASE-06` |

---

## 6. Owner / Independent-Reviewer Sign-Off Checklist

`CB-88` remains **UNCHECKED (`[ ]`)** until every item below is independently verified and recorded:

- [ ] **Pin Realization & Digest Recording:** Build unrealized `octet/nixpkgs` derivations (`pkgsStatic` musl toolchain, `qemu-10.2.1`, `seabios-1.17.0`, `linux-6.18.18`, `pkgsStatic.busybox`) and record exact store output SHA-256 digests (for both newly realized and already-realized pins) in [pins.json](pins.json).
- [ ] **Kernel & TCG Clock Audit:** Verify guest kernel config (`vsyscall=none`, `CONFIG_SECCOMP`, `vvar`/TSC behavior) and run sealed hostile probes (`__vdso_clock_gettime`, `__vdso_time`, `__vdso_gettimeofday`, `CLOCK_MONOTONIC_COARSE`, direct `vvar`, `rdtsc`, `rdtscp`) inside the pinned `qemu-10.2.1` `-icount shift=5,sleep=off,align=off` `-rtc base=2026-01-01T00:00:00,clock=vm` guest (`CB-CASE-08`).
- [ ] **Host Seccomp & Namespace Measurement:** Measure and verify fail-closed namespace presence and post-load seccomp allowlist on the confined QEMU host process (`CB-NC-01`, `CB-NC-04`, `CB-NC-05`, `CB-NC-07`, `CB-NC-08`).
- [ ] **Reproducible Static-ELF & Differential Evidence:** Demonstrate deterministic `-O0` and `-O2` full-ELF byte equality (`CB-NC-03`) and C/Wasm differential parity (`CB-CASE-01..10`) under independent review.
- [ ] **Explicit Owner Selection Decision:** Separate owner authorization to wire reviewed pins into a default-disabled C-native policy (`policy/tool-selection*` remains unchanged until that decision).
