# Offline backend inputs captured on the operator host.
#
# Nix cannot fetch these reproducibly: Lake requires a real `.git` directory in
# every `.lake/packages` entry (a plain tree fetch is rejected), and recursive
# hashing of a fresh clone was not reproducible for most of these repositories.
# Each Lean source value is therefore the store path recorded by
# `nix store add-path` over the pinned clone (regenerate with
# `.pi/m1-offline-backend/store-backend.sh`); `policy/lean-inputs.ncl` records
# the reviewed URLs, revisions, and NAR hashes, and the extraction check
# re-binds every package through `noble-lean-dependencies-check` (remote URL and
# checked-out revision) plus the repository's `lake-manifest.json`.
#
# The Miri sysroot is the `cargo miri setup` output for the selected extraction
# toolchain. Setup resolves and downloads build dependencies, so the built
# sysroot is supplied as an input instead of being rebuilt inside the sandbox.
#
# These are host-bound materialisations of reviewed pins, not authentication.
{
  lean_sources = {
    aeneas = /nix/store/dwb9czpwwgp78x7361353i5akjc6xw0j-aeneas;
    mathlib = /nix/store/mfpq85q33s4hyqy7c5x4prinz45d0341-mathlib;
    plausible = /nix/store/q54nqqrnmsy1w626h37zk2in85ibdqfs-plausible;
    LeanSearchClient = /nix/store/i22n3gkdlv9h9cmkbf4j5wja47d4v473-LeanSearchClient;
    importGraph = /nix/store/ky562jzqplb4qmc1r9wlldqic6vdnmh7-importGraph;
    proofwidgets = /nix/store/lp4l5g9xbkbnlv33p7rsm6m8y5pi2skz-proofwidgets;
    aesop = /nix/store/5cs5c6p7afpmrr2439pay2m7lml55h2b-aesop;
    Qq = /nix/store/p2jg0lwmzxhhc7smhdl5a1cg3n8jwy3d-Qq;
    batteries = /nix/store/gilx1kdl9l006l421k28z99b08cf2qzp-batteries;
    Cli = /nix/store/ixz9ja5v3k47qmnq1y5dgrxl20qy7acn-Cli;
  };
  miri_sysroot = /nix/store/ar0rxjdwv6142iwg2in77xh3jripak0r-miri;
}
