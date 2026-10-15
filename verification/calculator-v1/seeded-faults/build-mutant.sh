#!/usr/bin/env bash
# Builds one seeded-fault mutant `noble` binary: applies
# seeded-faults/patches/FAULT.patch to a clean detached worktree at BASE_REV,
# builds the release CLI, and copies the binary out with its SHA-256 and the
# exact patch hash. FAULT=control builds the unpatched base for comparison.
#
#   build-mutant.sh FAULT BASE_REV SCRATCH_WORKTREE CARGO_TARGET_DIR OUT_DIR
#
# The scratch worktree is reset to BASE_REV before each build; it must be a
# dedicated worktree, never a checkout holding someone's changes. Run cargo
# with the repository's pinned toolchain (for example inside `nix develop`).
set -euo pipefail
fault=$1 base=$2 scratch=$3 target=$4 out=$5
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(git -C "$here" rev-parse --show-toplevel)
patch="$here/patches/$fault.patch"
mkdir -p "$out"
if [ ! -e "$scratch/.git" ]; then
  git -C "$repo" worktree add --detach "$scratch" "$base"
fi
git -C "$scratch" checkout --detach --quiet "$base"
git -C "$scratch" reset --hard --quiet "$base"
if [ "$fault" != control ]; then
  test -f "$patch" || { echo "no patch for $fault" >&2; exit 2; }
  git -C "$scratch" apply --index "$patch"
fi
git -C "$scratch" diff --cached --stat
(cd "$scratch" && CARGO_TARGET_DIR="$target" nice -n 10 cargo build --release -p noble-cli -j "${JOBS:-6}")
binary="$out/noble-$fault"
cp "$target/release/noble" "$binary"
chmod a-w "$binary"
{
  echo "fault=$fault"
  echo "base_rev=$(git -C "$scratch" rev-parse HEAD)"
  if [ "$fault" != control ]; then echo "patch_sha256=$(sha256sum "$patch" | cut -d' ' -f1)"; fi
  echo "binary_sha256=$(sha256sum "$binary" | cut -d' ' -f1)"
} > "$binary.build"
cat "$binary.build"
git -C "$scratch" reset --hard --quiet "$base"
