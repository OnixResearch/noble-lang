//! Production-table controls for RA-CASE-01,03,05-09 and WI-06.
//! Accounting assertions concern local decisions, not physical/native release.

#[path = "m5-resources/boundaries.rs"]
mod boundaries;
#[path = "m5-resources/budgets.rs"]
mod budgets;
#[path = "m5-resources/handoff.rs"]
mod handoff;
#[path = "m5-resources/lifecycle.rs"]
mod lifecycle;
#[path = "m5-resources/retirement.rs"]
mod retirement;
#[path = "m5-resources/support.rs"]
mod support;
#[path = "m5-resources/transfer.rs"]
mod transfer;
