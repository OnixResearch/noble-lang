use super::super::monotonic_now_ns;
use super::support::*;

#[test]
fn polymorphic_recipe_is_freshly_instantiated_for_each_value_type() -> Result<(), String> {
    use noble_contracts::source::chunks::{Binding, Instantiation};
    let session = noble_contracts::source::Session::without_test_hosts();
    let duplicate = |ty: Ty| {
        let body = Recipe {
            nodes: vec![Node::Operation {
                op: Op::Dup,
                instantiation: 0,
            }],
            entry: vec![0],
        };
        let output = vec![ty.clone(), ty.clone()];
        let mut chunk = envelope(core::slice::from_ref(&ty), &output, body, b"dup ");
        chunk.instantiations = vec![Instantiation {
            bindings: vec![Binding {
                variable: 1,
                kind: noble_kernel::words::VariableKind::Value,
                ty,
            }],
        }];
        chunk.chunk_id = noble_contracts::source::chunks::chunk_identity_of(&chunk);
        chunk
    };
    let mut entries = library();
    let boolean = duplicate(Ty::Bool);
    let integer = duplicate(Ty::I64);
    entries
        .admit(&session, boolean.clone())
        .map_err(|refusal| format!("bool instantiation: {refusal:?}"))?;
    entries
        .admit(&session, integer.clone())
        .map_err(|refusal| format!("I64 instantiation: {refusal:?}"))?;
    assert_eq!(boolean.source_revision, integer.source_revision);
    assert_ne!(boolean.chunk_id, integer.chunk_id);
    for (input, output, ty, selected_id, expected_bindings) in [
        (
            vec![Ty::Bool],
            vec![Ty::Bool, Ty::Bool],
            Ty::Bool,
            boolean.chunk_id,
            2,
        ),
        (
            vec![Ty::I64],
            vec![Ty::I64, Ty::I64],
            Ty::I64,
            integer.chunk_id,
            1,
        ),
    ] {
        let candidate = request(hole(), vec![0], input.clone(), output.clone());
        let response = complete(&session, &entries, &candidate, monotonic_now_ns);
        let Response::Accepted { accepted, report } = response else {
            return Err(format!("{ty:?}: {}", response.outcome()));
        };
        assert_eq!(accepted.interface.stack_in, input);
        assert_eq!(accepted.interface.stack_out, output);
        assert_eq!(accepted.source, b"dup ");
        assert_eq!(accepted.selected, vec![selected_id]);
        assert_eq!(report.instantiation_bindings, expected_bindings);
    }
    Ok(())
}
