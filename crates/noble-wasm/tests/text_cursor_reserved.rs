//! An untrusted accepted-shape submission must not invoke private profile slots.

use noble_kernel::contracts::Definition;
use noble_kernel::untrusted::{Constraint, Node, Outcome};
use noble_wasm::source::Compiler;

#[test]
fn cursor_profile_refuses_forged_reserved_word_before_wasm_emission() -> Result<(), String> {
    let prepared = noble_contracts::source::Session::new_text_cursor()
        .prepare(
            b"\"ab\" 0 text.byte",
            &[],
            noble_contracts::Limits::default(),
        )
        .map_err(|error| error.diagnostic().message.clone())?;
    let authentic = prepared
        .submission()
        .cloned()
        .ok_or("missing cursor submission")?;
    Compiler::new_text_cursor()
        .prepare(&authentic)
        .map_err(|_| "valid cursor failed backend acceptance")?;

    let mut forged = authentic;
    let invocation = forged
        .body
        .candidate
        .nodes
        .iter_mut()
        .find(|node| matches!(node, Node::Invocation { def, .. } if *def == Definition(26)))
        .ok_or("missing real checked cursor invocation")?;
    if let Node::Invocation { def, .. } = invocation {
        *def = Definition(23);
    }
    match noble_kernel::acceptance::check(
        &forged.environment,
        &forged.request,
        &forged.body.candidate,
    ) {
        Outcome::Invalid(diagnostic)
            if matches!(diagnostic.constraint, Constraint::PrivateDefinition(def)
                if def == Definition(23)) => {}
        other => {
            return Err(format!(
                "reserved operation not rejected at kernel boundary: {other:?}"
            ))
        }
    }
    match Compiler::new_text_cursor().prepare(&forged) {
        Err(noble_wasm::Diagnostic::Invalid) => Ok(()),
        Err(_) => Err("reserved operation gave unexpected backend error".into()),
        Ok(_) => Err("reserved operation reached Wasm emission".into()),
    }
}
