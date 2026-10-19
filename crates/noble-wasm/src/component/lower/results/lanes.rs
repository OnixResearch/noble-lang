//! Lane reads and captures for canonical result lowering.

pub(super) fn read_lane(
    value: &super::super::Value,
    at: usize,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    let local = attempt!(super::super::slot(value, at));
    super::super::super::abi::get(buffer, local)
}

pub(super) fn capture_lane(
    value: &super::super::Value,
    at: usize,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    let local = attempt!(super::super::slot(value, at));
    super::super::super::abi::set(buffer, local)
}
