use super::*;

#[path = "fixtures/tasks.rs"]
mod tasks;
pub(crate) use tasks::{generate, Target, TaskCase, TaskSpec};

#[path = "fixtures/libraries.rs"]
mod libraries;
pub(crate) use libraries::*;

#[path = "fixtures/manifest.rs"]
mod manifest;
pub(crate) use manifest::{hex, scalar_json, snapshot};
