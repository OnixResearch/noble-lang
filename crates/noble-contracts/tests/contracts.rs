extern crate alloc;

const LIMITS: noble_contracts::Limits = noble_contracts::Limits {
    bytes: 65_536,
    nodes: 16_384,
    depth: 64,
    work: 2_000_000,
};

fn prepared(source: &[u8]) -> Result<noble_contracts::Prepared, alloc::string::String> {
    match noble_contracts::prepare(source, LIMITS) {
        Ok(prepared) => Ok(prepared),
        Err(error) => Err(alloc::format!("{error:?}")),
    }
}

#[test]
fn capture_row_is_resolved_by_later_composition() -> Result<(), alloc::string::String> {
    let prepared = prepared(include_bytes!("../src/fixtures/capture-family.noble"))?;
    let expected = noble_kernel::types::Ty::program(
        alloc::vec![noble_kernel::types::Ty::I64],
        alloc::vec![noble_kernel::types::Ty::I64],
        noble_kernel::types::EffSet::empty(),
    );
    assert_eq!(
        prepared.checked().interface.stack_out,
        alloc::vec![expected]
    );
    assert_eq!(
        prepared.checked().interface.stack_in,
        alloc::vec![noble_kernel::types::Ty::I64]
    );
    Ok(())
}

#[test]
fn scalar_and_structural_contracts_use_the_same_prepare_boundary(
) -> Result<(), alloc::string::String> {
    let increment = prepared(include_bytes!("../src/fixtures/increment.noble"))?;
    assert_eq!(
        increment.checked().interface.stack_out,
        alloc::vec![noble_kernel::types::Ty::I64]
    );
    let structural = prepared(include_bytes!("../src/fixtures/structural.noble"))?;
    assert_eq!(
        structural.checked().interface.stack_out,
        alloc::vec![noble_kernel::types::Ty::Pair(
            alloc::boxed::Box::new(noble_kernel::types::Ty::Bool),
            alloc::boxed::Box::new(noble_kernel::types::Ty::I64)
        )]
    );
    let maps = prepared(include_bytes!("../src/fixtures/structural-map.noble"))?;
    match maps.checked().interface.stack_out.first() {
        Some(noble_kernel::types::Ty::Program(input, output, _)) => {
            assert_eq!(
                **input,
                alloc::vec![noble_kernel::types::Ty::Pair(
                    alloc::boxed::Box::new(noble_kernel::types::Ty::I64),
                    alloc::boxed::Box::new(noble_kernel::types::Ty::Bool)
                )]
            );
            assert_eq!(**output, structural.checked().interface.stack_out);
        }
        Some(_) | None => {
            return Err(alloc::string::String::from(
                "structural quotation did not produce a program",
            ))
        }
    }
    Ok(())
}

#[test]
fn ghost_parameters_cannot_replace_runtime_capture() -> Result<(), alloc::string::String> {
    let source = b"(contract 1 ghost (input) (output (n I64)) (params (n I64)) (program [ (param n) ]) (requires true) (ensures true))";
    match noble_contracts::prepare(source, LIMITS) {
        Err(error) => {
            assert_eq!(error.kind, noble_contracts::DiagnosticKind::Invalid);
            assert!(error.ordinary_typing().is_none());
        }
        Ok(_) => {
            return Err(alloc::string::String::from(
                "a logical parameter supplied a runtime value",
            ))
        }
    }
    Ok(())
}

#[test]
fn output_dependency_through_definition_cannot_enter_precondition(
) -> Result<(), alloc::string::String> {
    let source = b"(contract 1 hidden-output (input (x I64)) (output (y I64)) (define result I64 (out y)) (program []) (requires (eq (def result) 0)) (ensures true))";
    match noble_contracts::prepare(source, LIMITS) {
        Err(error) => {
            assert_eq!(error.kind, noble_contracts::DiagnosticKind::Invalid);
            assert!(error.ordinary_typing().is_some());
            assert_eq!(
                source.get(
                    usize::try_from(error.span.start).map_err(|error| error.to_string())?
                        ..usize::try_from(error.span.end).map_err(|error| error.to_string())?
                ),
                Some(b"(eq (def result) 0)".as_slice())
            );
        }
        Ok(_) => {
            return Err(alloc::string::String::from(
                "an output-dependent definition entered requires",
            ))
        }
    }
    Ok(())
}

#[test]
fn partial_definitions_and_maps_interface_mismatch_retain_ordinary_typing(
) -> Result<(), alloc::string::String> {
    let sources: [&[u8]; 2] = [
        b"(contract 1 partial (input (xs (List I64))) (output (ys (List I64))) (define h I64 (head (in xs))) (program []) (requires true) (ensures true))",
        b"(contract 1 wrong-map (input (p (Program (I64) (I64)))) (output (p (Program (I64) (I64)))) (params (x Bool)) (program []) (requires true) (ensures (maps (out p) (param x) 0)))",
    ];
    let mut at = 0usize;
    while at < sources.len() {
        match noble_contracts::prepare(sources[at], LIMITS) {
            Err(error) => {
                assert_eq!(error.kind, noble_contracts::DiagnosticKind::Invalid);
                assert!(error.ordinary_typing().is_some());
            }
            Ok(_) => {
                return Err(alloc::string::String::from(
                    "an ill-typed or partial definition was accepted",
                ))
            }
        }
        at += 1;
    }
    Ok(())
}

#[test]
fn cyclic_explicit_witness_is_not_erased_by_inference() -> Result<(), alloc::string::String> {
    let source = b"(contract 1 cycle (input (x I64)) (output (x I64) (y I64)) (program [ (word 0 (stack) (ref 1)) ]) (requires true) (ensures true))";
    match noble_contracts::prepare(source, LIMITS) {
        Err(error) => assert_eq!(error.kind, noble_contracts::DiagnosticKind::Invalid),
        Ok(_) => {
            return Err(alloc::string::String::from(
                "a self-referential explicit witness was accepted",
            ))
        }
    }
    Ok(())
}

#[test]
fn finite_limits_and_unsupported_claim_kind_fail_closed() -> Result<(), alloc::string::String> {
    let source =
        b"(contract 1 bounded (input) (output) (program []) (requires true) (ensures true))";
    let limits = noble_contracts::Limits { bytes: 1, ..LIMITS };
    match noble_contracts::prepare(source, limits) {
        Err(error) => assert_eq!(error.kind, noble_contracts::DiagnosticKind::Exhausted),
        Ok(_) => return Err(alloc::string::String::from("byte cap was ignored")),
    }
    let limits = noble_contracts::Limits { depth: 1, ..LIMITS };
    match noble_contracts::prepare(source, limits) {
        Err(error) => assert_eq!(error.kind, noble_contracts::DiagnosticKind::Exhausted),
        Ok(_) => return Err(alloc::string::String::from("depth cap was ignored")),
    }
    let limits = noble_contracts::Limits { nodes: 1, ..LIMITS };
    match noble_contracts::prepare(source, limits) {
        Err(error) => assert_eq!(error.kind, noble_contracts::DiagnosticKind::Exhausted),
        Ok(_) => return Err(alloc::string::String::from("node cap was ignored")),
    }
    let limits = noble_contracts::Limits { work: 1, ..LIMITS };
    match noble_contracts::prepare(source, limits) {
        Err(error) => assert_eq!(error.kind, noble_contracts::DiagnosticKind::Exhausted),
        Ok(_) => return Err(alloc::string::String::from("work cap was ignored")),
    }
    let total = b"(contract 1 total (kind total-correctness) (input) (output) (program []) (requires true) (ensures true))";
    match noble_contracts::prepare(total, LIMITS) {
        Err(error) => {
            assert_eq!(error.kind, noble_contracts::DiagnosticKind::Unsupported);
            assert!(error.ordinary_typing().is_some());
        }
        Ok(_) => {
            return Err(alloc::string::String::from(
                "unsupported total-correctness was silently weakened",
            ))
        }
    }
    Ok(())
}

#[test]
fn signed_integer_boundary_is_not_wrapped_during_parsing() -> Result<(), alloc::string::String> {
    let minimum = b"(contract 1 minimum (input) (output (x I64)) (program [ -9223372036854775808 ]) (requires true) (ensures (eq (out x) -9223372036854775808)))";
    let prepared = prepared(minimum)?;
    match prepared.candidate().nodes.first() {
        Some(noble_kernel::untrusted::Node::Literal {
            lit: noble_kernel::untrusted::Lit::I64(value),
            ..
        }) => assert_eq!(*value, i64::MIN),
        Some(_) | None => {
            return Err(alloc::string::String::from(
                "minimum I64 did not remain an integer literal",
            ))
        }
    }
    let overflow = b"(contract 1 overflow (input) (output) (program []) (requires true) (ensures (eq 9223372036854775808 0)))";
    match noble_contracts::prepare(overflow, LIMITS) {
        Err(error) => {
            assert_eq!(error.kind, noble_contracts::DiagnosticKind::Invalid);
            assert!(error.ordinary_typing().is_some());
        }
        Ok(_) => {
            return Err(alloc::string::String::from(
                "out-of-range literal was wrapped",
            ))
        }
    }
    Ok(())
}

#[test]
fn recipe_identity_is_not_a_logical_structural_observation() -> Result<(), alloc::string::String> {
    let sources: [&[u8]; 2] = [
        b"(contract 1 program-equality (input (p (Program (I64) (I64)))) (output (p (Program (I64) (I64)))) (program []) (requires true) (ensures (eq (in p) (out p))))",
        b"(contract 1 nested-recipe (input (p (List (Pair Syntax (Program (I64) (I64)))))) (output (p (List (Pair Syntax (Program (I64) (I64)))))) (program []) (requires true) (ensures (eq (in p) (out p))))",
    ];
    for source in sources {
        match noble_contracts::prepare(source, LIMITS) {
            Err(error) => {
                assert_eq!(error.kind, noble_contracts::DiagnosticKind::Unsupported);
                assert!(error.ordinary_typing().is_some());
            }
            Ok(_) => {
                return Err(alloc::string::String::from(
                    "recipe identity entered a structural predicate",
                ))
            }
        }
    }
    Ok(())
}

#[test]
fn skipped_comments_still_require_valid_utf8() -> Result<(), alloc::string::String> {
    let valid = "(contract 1 utf8-comment (input) (output) (program []) (requires true) (ensures true)) ; λ\n";
    assert_eq!(prepared(valid.as_bytes())?.name(), "utf8-comment");
    let invalid = b"(contract 1 bad-comment (input) (output) (program []) (requires true) (ensures true)) ;\xff\n";
    match noble_contracts::prepare(invalid, LIMITS) {
        Err(error) => {
            assert_eq!(error.kind, noble_contracts::DiagnosticKind::Invalid);
            assert!(error.ordinary_typing().is_none());
            assert_eq!(
                invalid.get(
                    usize::try_from(error.span.start).map_err(|error| error.to_string())?
                        ..usize::try_from(error.span.end).map_err(|error| error.to_string())?
                ),
                Some(b"\xff".as_slice())
            );
        }
        Ok(_) => {
            return Err(alloc::string::String::from(
                "invalid UTF-8 was hidden by a comment",
            ))
        }
    }
    Ok(())
}

#[test]
fn unknown_predicate_preserves_typing_as_unsupported() -> Result<(), alloc::string::String> {
    let source = b"(contract 1 future (input) (output) (program []) (requires true) (ensures (future-predicate true)))";
    match noble_contracts::prepare(source, LIMITS) {
        Err(error) => {
            assert_eq!(error.kind, noble_contracts::DiagnosticKind::Unsupported);
            assert!(error.ordinary_typing().is_some());
        }
        Ok(_) => {
            return Err(alloc::string::String::from(
                "unknown predicate was silently accepted",
            ))
        }
    }
    Ok(())
}
