# WI03 independent component peer

`peer/` is a standalone Wasmtime 40.0.2 host, not linked to Noble crates. It accepts exactly one argument: the path to a **real component binary** emitted by `noble component compile-checked-u64 WIT WORLD NEW_DIR echo=SOURCE` for an import-free world with `export echo: func(value: u64) -> u64`.

Run `noble-wi03-peer COMPONENT_PATH`. The peer checks the component export type, calls a fresh instance for each `Val::U64` value `0`, `42`, `i64::MAX`, `i64::MAX+1`, and `u64::MAX`, and writes one JSON object to stdout. Fresh instances prevent one trap from poisoning subsequent probes. Each observation contains the actual call outcome, including trap text or the returned component `Val` variant (`u64` versus `s64`), and the report includes a SHA256 of the exact bytes loaded. No import callbacks or guest-body counters are claimed.

Build with the repository's selected Rust at `/nix/store/1yvh3d6y3fj3xk2dgwczrp1dj5svd92c-rust-default-1.96.0-nightly-2026-03-21/bin/cargo` using `--manifest-path verification/wi03/peer/Cargo.toml --locked --offline`, the M5 vendor source replacement in `verification/m5/build.mjs`, `RUSTC_WRAPPER=`, and an external `CARGO_TARGET_DIR`. The peer source and lockfile are separate from the archived M5 peer.
