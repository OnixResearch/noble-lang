//! Opt-in, source-bound sequential comparison. Never a default test-time benchmark.

#[path = "benchmark/model.rs"]
mod domain;
#[path = "benchmark/json.rs"]
mod encoding;
#[path = "benchmark/backend.rs"]
mod engines;
#[path = "benchmark/execution.rs"]
mod execution;
#[path = "benchmark/operations.rs"]
mod operations;
#[path = "benchmark/probes.rs"]
mod probes;
#[path = "benchmark/receipt.rs"]
mod reporting;
