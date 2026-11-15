use super::*;

#[path = "labels/collect.rs"]
mod collect;
pub(crate) use collect::*;

#[path = "labels/oracle.rs"]
mod oracle;
pub(crate) use oracle::*;
