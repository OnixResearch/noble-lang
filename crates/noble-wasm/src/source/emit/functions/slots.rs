//! Slot-kind guards emitted before dynamic slot access.

pub(super) struct SlotGuard {
    pub(super) shape: u32,
    pub(super) depth: u32,
}

pub(super) fn checked_slot(
    out: &mut crate::output::Buffer,
    guard: SlotGuard,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(if (i32.eqz (call $need "));
    attempt!(out.i32(guard.depth));
    attempt!(out.append(b")) (then (return)))\n(if (i32.eqz (call $t"));
    attempt!(out.number(u64::from(guard.shape)));
    attempt!(out.append(b" (call $slot_kind (i32.sub (global.get $sp) "));
    attempt!(out.i32(guard.depth));
    attempt!(out.append(b")) (call $slot_value (i32.sub (global.get $sp) "));
    attempt!(out.i32(guard.depth));
    out.append(b")))) (then (call $fail (i32.const 4)) (return)))\n")
}

pub(super) fn nominal_call(
    out: &mut crate::output::Buffer,
    function: &[u8],
    id: noble_kernel::types::NominalTypeId,
) -> Result<(), crate::Diagnostic> {
    attempt!(out.append(b"(call "));
    attempt!(out.append(function));
    attempt!(out.append(b" "));
    attempt!(out.i64(id.module as i64));
    attempt!(out.append(b" "));
    out.i32(id.ordinal)
}
