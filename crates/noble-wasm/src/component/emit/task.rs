#[expect(
    tigerstyle::assertion_density,
    reason = "Owner: noble-maintainers; task-return names are split at an observed byte index and signatures use checked canonical flattening; every bounded output write preserves a diagnostic instead of asserting on a checked plan."
)]
pub(super) fn write(
    plan: &super::super::lower::Plan,
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    let qualified = plan.name.as_bytes();
    attempt!(buffer.append(b" (import "));
    match qualified.iter().position(|byte| *byte == b'#') {
        Some(at) => {
            attempt!(names(
                &qualified[..at],
                &qualified[at.saturating_add(1)..],
                buffer
            ));
        }
        None => {
            attempt!(names(b"$root", qualified, buffer));
        }
    }
    attempt!(buffer.append(b" (func $task-return-"));
    attempt!(buffer.number(u64::from(plan.ordinal)));
    if let Some(ty) = plan.result {
        attempt!(super::super::abi::parameters(
            core::slice::from_ref(&ty),
            buffer
        ));
    }
    buffer.append(b"))\n")
}

fn names(
    module: &[u8],
    name: &[u8],
    buffer: &mut crate::output::Buffer,
) -> Result<(), crate::Diagnostic> {
    attempt!(super::quoted_prefixed(b"[export]", module, buffer));
    attempt!(buffer.append(b" "));
    super::quoted_prefixed(b"[task-return]", name, buffer)
}
