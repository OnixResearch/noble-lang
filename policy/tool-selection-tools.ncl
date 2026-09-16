# Reviewed Nix recipe/output identities for the selected tool closure.
# These interoperate with Nix identities; they are not build authenticity proofs.
{
  aeneas = {
    derivation = "/nix/store/1x90ighanbdkhaxyc33dd2kv7j8pgpxh-ocaml5.2.1-aeneas-0.1.0.drv",
    output = "/nix/store/4a6mwly7yrbpr675xq9r399nf75d7d71-ocaml5.2.1-aeneas-0.1.0",
  },
  cairn = {
    derivation = "/nix/store/c34azbr57788nd8d7yi83rxxnb143pdb-cairn-0.1.0.drv",
    output = "/nix/store/bvz6kl88qkfi15n8ml1xr7p22syykggd-cairn-0.1.0",
  },
  charon = {
    derivation = "/nix/store/4cbk4z12wcjr68zrryb7wim5vlh719l2-charon.drv",
    output = "/nix/store/ka61j6d6zy8zg91ynlbd6nmjssyynrpl-charon",
  },
  extraction_rust = {
    derivation = "/nix/store/jjyd99rqsd7zbkwzyqma7qcc61l71kfw-rust-default-1.100.0-nightly-2026-08-18.drv",
    output = "/nix/store/wq92i79ivn1w1nrdhmzl1mqx0wcyrawi-rust-default-1.100.0-nightly-2026-08-18",
  },
  lean = {
    derivation = "/nix/store/93d3dz41ps7wqq9lbs1cysnx34d43pqr-noble-lean-release-4.31.0.drv",
    output = "/nix/store/hbxwgcbpkyqllqzq7cidhnvhgb60jb9q-noble-lean-release-4.31.0",
  },
  nickel = {
    derivation = "/nix/store/fnvv96qkzwb6rfxh7mxkxgx9akcjgqyv-nickel-lang-rev-fixup-1.17.0.drv",
    output = "/nix/store/ck3a4k9az87z5rjwgy8nmpcs34xp9cc5-nickel-lang-rev-fixup-1.17.0",
  },
  octet = {
    derivation = "/nix/store/7iqww35i0mvd8bkv0gckq826q6b2kf21-cargo-octet-0.1.0.drv",
    output = "/nix/store/vj0sg5rn291z78im7gd2cqzqzd0bq3zy-cargo-octet-0.1.0",
  },
  octet_standards = {
    derivation = "/nix/store/ackprds206cvvx31x6b1dwarwc812jpv-octet-standards-0.1.0.drv",
    output = "/nix/store/fh39gx0hwply5mblzd11g7xysc1ihh14-octet-standards-0.1.0",
  },
  quality_rust = {
    derivation = "/nix/store/zxvqck0x496vcj2qn1r0wil6bb85zj7b-rust-default-1.96.0-nightly-2026-03-21.drv",
    output = "/nix/store/1yvh3d6y3fj3xk2dgwczrp1dj5svd92c-rust-default-1.96.0-nightly-2026-03-21",
  },
  upstream_pin_check = {
    derivation = "/nix/store/abf9q3r42bql84jmywvw8q0r9q7ya84n-aeneas-check-charon-pin.drv",
    output = "/nix/store/b34wcdj3gbjf03dzi5y8ghv23bsc3aya-aeneas-check-charon-pin",
  },
}
