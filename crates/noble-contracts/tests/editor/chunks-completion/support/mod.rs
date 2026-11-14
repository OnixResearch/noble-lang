mod common;
#[path = "receipt.rs"]
mod evidence;

pub(crate) use super::super::chunks::{envelope, library, limits, pinned};
pub(crate) use noble_contracts::source::chunks::{
    complete, Accepted, Budget, Envelope, Library, Node, Op, Recipe, Report, Resource, Response,
    Scalar,
};
pub(crate) use noble_contracts::source::editor::Node as EditorNode;
pub(crate) use noble_kernel::types::Ty;

pub(super) const MEASUREMENT_RECEIPT_PATH: Option<&str> = option_env!("NOBLE_TYPED_CHUNK_RECEIPT");
pub(super) const MEASUREMENT_SOURCE_COMMIT: Option<&str> =
    option_env!("NOBLE_TYPED_CHUNK_SOURCE_COMMIT");

pub(crate) use common::*;
pub(crate) use evidence::maybe_write_measurement_receipt;
