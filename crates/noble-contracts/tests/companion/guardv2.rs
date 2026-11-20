//! Versioned v2 guard templates: recognition, frozen v1 bytes and domains.
//!
//! These are deterministic core regressions only. They exercise the recognizer,
//! the exact rendered texts and the digest domains; runtime acceptance and
//! rejection through a live invocation are the CLI's `guardv2` smoke control.

/// One contract over the supplied interface and whole-root predicate. The body
/// drops the single input and returns `0`, so only the recognition under test
/// decides whether a guard template exists.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-maintainers; the interface, program and predicate fragments are positional parts of one contract text, so a wrapper type would only rename the same three strings."
)]
fn source_with_program(interface: &str, program: &str, predicate: &str) -> String {
    format!(
        "(contract 1 guard-v2 (input {interface}) (output (y I64)) \
         (program [ {program} ]) (requires {predicate}) (ensures true))"
    )
}

#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-maintainers; interface and predicate are positional parts of one contract text, so a wrapper type would only rename the same two strings."
)]
fn source(interface: &str, predicate: &str) -> String {
    source_with_program(interface, "drop 0", predicate)
}

fn core() -> noble_contracts::companion::Core {
    noble_contracts::companion::Core::new(super::LIMITS)
}

/// Admit one contract with an empty offer: the retained contract record exists
/// while no evidence is minted, which is all recognition needs.
#[expect(
    tigerstyle::ambiguous_params,
    reason = "Owner: noble-maintainers; interface and predicate are positional parts of one contract text, so a wrapper type would only rename the same two strings."
)]
fn recognized(
    interface: &str,
    predicate: &str,
) -> Result<
    (
        noble_contracts::companion::Core,
        noble_contracts::companion::ContractId,
    ),
    String,
> {
    let mut core = core();
    let prepared = super::prepared(source(interface, predicate).as_bytes())?;
    let admission = core.admit(&prepared, super::offer(&[]));
    match admission.contract {
        Some(contract) => Ok((core, contract)),
        None => Err(String::from("contract not retained")),
    }
}

fn list_i64() -> noble_kernel::types::Ty {
    noble_kernel::types::Ty::List(Box::new(noble_kernel::types::Ty::I64))
}

#[test]
fn nonempty_list_template_is_recognized() -> Result<(), String> {
    let (core, contract) = recognized("(xs (List I64))", "(not (is-nil (in xs)))")?;
    assert_eq!(
        core.guard_templates_v2(contract),
        Ok(vec![
            noble_contracts::companion::GuardTemplateV2::NonEmptyList
        ])
    );
    // The frozen v1 set still refuses the list interface.
    assert_eq!(
        core.guard_templates(contract),
        Err(noble_contracts::companion::Refusal::UnsupportedGuardTemplate)
    );
    assert_eq!(core.list_guard_interface(contract), Ok(list_i64()));
    Ok(())
}

#[test]
fn nested_list_element_type_is_type0() -> Result<(), String> {
    let (core, contract) = recognized("(xs (List (List I64)))", "(not (is-nil (in xs)))")?;
    assert_eq!(
        core.guard_templates_v2(contract),
        Ok(vec![
            noble_contracts::companion::GuardTemplateV2::NonEmptyList
        ])
    );
    assert_eq!(
        core.list_guard_interface(contract),
        Ok(noble_kernel::types::Ty::List(Box::new(
            noble_kernel::types::Ty::List(Box::new(noble_kernel::types::Ty::I64))
        )))
    );
    Ok(())
}

#[test]
fn whole_root_shapes_are_required() -> Result<(), String> {
    // Each row is a hostile near miss: `is-nil` without `not`, a nested
    // negation, length forms, a head selector, a second input and a non-list
    // interface. None of them may select the list template, and none may fall
    // back to an assumed guard.
    let cases = [
        ("(xs (List I64))", "drop 0", "(is-nil (in xs))"),
        (
            "(xs (List I64))",
            "drop 0",
            "(not (not (not (is-nil (in xs)))))",
        ),
        ("(xs (List I64))", "drop 0", "(lt 0 (length (in xs)))"),
        ("(xs (List I64))", "drop 0", "(not (eq (length (in xs)) 0))"),
        ("(xs (List I64))", "drop 0", "(lt 0 (head (in xs)))"),
        (
            "(xs (List I64)) (ys (List I64))",
            "drop drop 0",
            "(not (is-nil (in ys)))",
        ),
        ("(x I64)", "drop 0", "(not (eq (in x) 0))"),
        (
            "(xs (List (Program (I64) (I64))))",
            "drop 0",
            "(not (is-nil (in xs)))",
        ),
    ];
    // An ill-typed selector is refused by ordinary typing before recognition;
    // it is never treated as unsupported-but-prepared.
    for (interface, predicate) in [
        ("(xs (List I64))", "(not (is-nil (head (in xs))))"),
        ("(x I64)", "(not (is-nil (in x)))"),
    ] {
        assert!(super::prepared(source(interface, predicate).as_bytes()).is_err());
    }
    for (interface, program, predicate) in cases {
        let mut core = core();
        let prepared =
            super::prepared(source_with_program(interface, program, predicate).as_bytes())
                .map_err(|error| format!("{interface} {predicate}: {error}"))?;
        let admission = core.admit(&prepared, super::offer(&[]));
        let contract = admission
            .contract
            .ok_or_else(|| format!("{interface}: contract not retained"))?;
        assert_eq!(
            core.guard_templates_v2(contract),
            Err(noble_contracts::companion::Refusal::UnsupportedGuardTemplate),
            "{interface} {predicate}"
        );
        assert_eq!(
            core.list_guard_interface(contract),
            Err(noble_contracts::companion::Refusal::UnsupportedGuardTemplate),
            "{interface} {predicate}"
        );
    }
    Ok(())
}

#[test]
fn v1_template_bytes_are_frozen() -> Result<(), String> {
    let core = core();
    let body = "1 +";
    // Exact v1 renderings, pinned independently of the implementation.
    for (template, source_wrapper, invocation_wrapper) in [
        (
            noble_contracts::companion::GuardTemplate::LtI64Max,
            "[ dup 9223372036854775807 = [ drop 1 inl ] [ [ 1 + ] run inr ] if ]",
            "[ swap dup 9223372036854775807 = [ drop drop 1 inl ] [ swap run inr ] if ]",
        ),
        (
            noble_contracts::companion::GuardTemplate::NeI64Min,
            "[ dup -9223372036854775808 = [ drop 1 inl ] [ [ 1 + ] run inr ] if ]",
            "[ swap dup -9223372036854775808 = [ drop drop 1 inl ] [ swap run inr ] if ]",
        ),
        (
            noble_contracts::companion::GuardTemplate::EqI64Literal(7),
            "[ dup 7 = [ [ 1 + ] run inr ] [ drop 1 inl ] if ]",
            "[ swap dup 7 = [ swap run inr ] [ drop drop 1 inl ] if ]",
        ),
    ] {
        assert_eq!(core.wrapper_source(template, body), source_wrapper);
        assert_eq!(core.invocation_wrapper(template), invocation_wrapper);
        // The v2 set lifts the same three templates and must emit the same
        // bytes for them.
        let lifted = match template {
            noble_contracts::companion::GuardTemplate::LtI64Max => {
                noble_contracts::companion::GuardTemplateV2::LtI64Max
            }
            noble_contracts::companion::GuardTemplate::NeI64Min => {
                noble_contracts::companion::GuardTemplateV2::NeI64Min
            }
            noble_contracts::companion::GuardTemplate::EqI64Literal(value) => {
                noble_contracts::companion::GuardTemplateV2::EqI64Literal(value)
            }
        };
        assert_eq!(core.wrapper_source_v2(lifted, body), source_wrapper);
        assert_eq!(core.invocation_wrapper_v2(lifted), invocation_wrapper);
    }
    Ok(())
}

#[test]
fn v2_list_texts_are_the_designed_texts() {
    let core = core();
    let body = "[ 0 ] [ drop ] list.case";
    assert_eq!(
        core.wrapper_source_v2(
            noble_contracts::companion::GuardTemplateV2::NonEmptyList,
            body
        ),
        "[ dup [ true ] [ drop drop false ] list.case [ drop 1 inl ] [ [ [ 0 ] [ drop ] list.case ] run inr ] if ]"
    );
    assert_eq!(
        core.invocation_wrapper_v2(noble_contracts::companion::GuardTemplateV2::NonEmptyList),
        "[ swap dup [ true ] [ drop drop false ] list.case [ drop drop 1 inl ] [ swap run inr ] if ]"
    );
}

#[test]
fn template_versions_and_digest_domains_are_separate() {
    let core = core();
    assert_eq!(noble_contracts::companion::GUARD_SET_VERSION_V2, 2);
    for template in [
        noble_contracts::companion::GuardTemplateV2::LtI64Max,
        noble_contracts::companion::GuardTemplateV2::NeI64Min,
        noble_contracts::companion::GuardTemplateV2::EqI64Literal(7),
        noble_contracts::companion::GuardTemplateV2::NonEmptyList,
    ] {
        assert_eq!(template.set_version(), 2);
        // A digest is stable for one template ...
        assert_eq!(
            core.guard_template_digest_v2(template),
            core.guard_template_digest_v2(template)
        );
        // ... and the four templates are pairwise distinct.
        let others = [
            noble_contracts::companion::GuardTemplateV2::LtI64Max,
            noble_contracts::companion::GuardTemplateV2::NeI64Min,
            noble_contracts::companion::GuardTemplateV2::EqI64Literal(7),
            noble_contracts::companion::GuardTemplateV2::NonEmptyList,
        ];
        for other in others {
            if other != template {
                assert_ne!(
                    core.guard_template_digest_v2(template),
                    core.guard_template_digest_v2(other)
                );
            }
        }
    }
    // The same scalar template under the two set versions is domain-separated.
    for (v1, v2) in [
        (
            noble_contracts::companion::GuardTemplate::LtI64Max,
            noble_contracts::companion::GuardTemplateV2::LtI64Max,
        ),
        (
            noble_contracts::companion::GuardTemplate::NeI64Min,
            noble_contracts::companion::GuardTemplateV2::NeI64Min,
        ),
        (
            noble_contracts::companion::GuardTemplate::EqI64Literal(7),
            noble_contracts::companion::GuardTemplateV2::EqI64Literal(7),
        ),
    ] {
        assert_ne!(
            core.guard_template_digest(v1),
            core.guard_template_digest_v2(v2)
        );
    }
    // The literal template family stays a family in both sets.
    assert_ne!(
        core.guard_template_digest_v2(noble_contracts::companion::GuardTemplateV2::EqI64Literal(7)),
        core.guard_template_digest_v2(noble_contracts::companion::GuardTemplateV2::EqI64Literal(8))
    );
}

#[test]
fn v2_codes_are_stable() {
    for (template, code) in [
        (
            noble_contracts::companion::GuardTemplateV2::LtI64Max,
            "lt-i64-max",
        ),
        (
            noble_contracts::companion::GuardTemplateV2::NeI64Min,
            "ne-i64-min",
        ),
        (
            noble_contracts::companion::GuardTemplateV2::EqI64Literal(7),
            "eq-i64-literal",
        ),
        (
            noble_contracts::companion::GuardTemplateV2::NonEmptyList,
            "non-empty-list",
        ),
    ] {
        assert_eq!(template.code(), code);
    }
}
