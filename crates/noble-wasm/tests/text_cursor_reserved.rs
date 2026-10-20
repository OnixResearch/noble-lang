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
    if Compiler::new().prepare(&authentic).is_ok() {
        return Err("ordinary compiler accepted a selected cursor environment".into());
    }

    for private in 22..=25 {
        let mut forged = authentic.clone();
        let invocation = forged
            .body
            .candidate
            .nodes
            .iter_mut()
            .find(|node| matches!(node, Node::Invocation { def, .. } if *def == Definition(26)))
            .ok_or("missing real checked cursor invocation")?;
        if let Node::Invocation { def, .. } = invocation {
            *def = Definition(private);
        }
        match noble_kernel::acceptance::check(
            &forged.environment,
            &forged.request,
            &forged.body.candidate,
        ) {
            Outcome::Invalid(diagnostic)
                if matches!(diagnostic.constraint, Constraint::PrivateDefinition(def)
                    if def == Definition(private)) => {}
            other => {
                return Err(format!(
                    "private operation {private} not rejected at kernel boundary: {other:?}"
                ))
            }
        }
        match Compiler::new_text_cursor().prepare(&forged) {
            Err(noble_wasm::Diagnostic::Invalid) => {}
            Err(_) => return Err("reserved operation gave unexpected backend error".into()),
            Ok(_) => return Err("reserved operation reached Wasm emission".into()),
        }
    }
    Ok(())
}
