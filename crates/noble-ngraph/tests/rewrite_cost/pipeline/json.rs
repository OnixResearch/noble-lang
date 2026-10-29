//! JSON receipt modules for the pipeline lane.

#[path = "json/comparison.rs"]
mod comparison;
#[path = "json/format.rs"]
mod format;
#[path = "json/receipt.rs"]
mod receipt;
#[path = "json/stages.rs"]
mod stages;

pub(in crate::pipeline) use receipt::render;
