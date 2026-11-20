//! `wire@1`: the pinned Wiring library fixture (design Decision 1).
//!
//! The library's home is an immutable fixture with an exact-source check, as
//! `result@1` established (OWNER DECISION 2026-11-20: a pinned fixture with an
//! exact-bytes check). Every word is a checked composition of existing
//! builtins, so these tests register the exact bytes, refuse any other bytes,
//! and check that each word links at its published signature.

fn session() -> Result<noble_contracts::source::ModuleSession, String> {
    noble_contracts::source::ModuleSession::new(
        &[],
        noble_contracts::source::RetainedSourceLimit::new(65_536)
            .expect("valid legacy retained source cap"),
    )
    .map_err(|error| format!("{error:?}"))
}

#[test]
fn wire_library_registers_only_its_exact_source_bytes() -> Result<(), String> {
    let mut session = session()?;
    assert_eq!(
        noble_contracts::source::WIRE_LIBRARY_SOURCE.len(),
        2_933,
        "the pinned O11-4 sketch is 2,933 bytes"
    );
    session = super::commit(session, noble_contracts::source::WIRE_LIBRARY_SOURCE)?;
    // The exact bytes register once; a second registration of the same
    // version is a duplicate, not a new library.
    assert!(super::commit(session, noble_contracts::source::WIRE_LIBRARY_SOURCE).is_err());
    Ok(())
}

#[test]
fn wire_library_refuses_any_other_declaration_bytes() -> Result<(), String> {
    let session = session()?;
    let mut altered = noble_contracts::source::WIRE_LIBRARY_SOURCE.to_vec();
    // A comment keeps the altered text a well-formed module declaration, so
    // the refusal proves the byte check and not a parse or typing failure.
    altered.splice(15..15, b" # altered".iter().copied());
    assert_ne!(
        altered.as_slice(),
        noble_contracts::source::WIRE_LIBRARY_SOURCE
    );
    let refused = session
        .prepare(&altered, &[], super::LIMITS)
        .expect_err("a changed wire@1 body must be refused");
    assert_eq!(
        refused.stage(),
        noble_contracts::source::Stage::Link,
        "the exact-source check refuses at link: {refused:?}"
    );
    assert_eq!(
        refused.diagnostic().message,
        "wire@1 requires the exact checked library source"
    );
    Ok(())
}

#[test]
fn wire_library_words_link_at_their_published_signatures() -> Result<(), String> {
    let mut session = session()?;
    session = super::commit(session, noble_contracts::source::WIRE_LIBRARY_SOURCE)?;
    let split = session
        .prepare(
            b"10 20 pair [ 1 + ] [ 2 * ] wire@1.split unpair",
            &[],
            super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    let routed = session
        .prepare(
            b"10 20 pair [ 1 + ] wire@1.first [ 2 * ] wire@1.second unpair",
            &[],
            super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        split.output(),
        routed.output(),
        "split and the first/second route publish the same interface"
    );
    let fanout = session
        .prepare(
            b"7 [ 1 + ] [ 2 * ] wire@1.fanout unpair",
            &[],
            super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(fanout.output(), split.output());
    for program in [
        b"5 inl [ 1 + ] wire@1.left".as_slice(),
        b"5 inr [ 1 + ] wire@1.right",
        b"5 inr [ 1 + ] [ 2 * ] wire@1.choose",
        b"5 inl [ 1 + ] [ drop 0 ] wire@1.join",
        b"1 2 pair 3 pair wire@1.assoc_pair",
        b"1 2 pair wire@1.swap_pair",
        b"1 2 pair wire@1.project_first",
        b"5 wire@1.identity",
    ] {
        session
            .prepare(program, &[], super::LIMITS)
            .map_err(|error| format!("{program:?}: {error:?}"))?;
    }
    let applied = session
        .prepare(
            b"5 wire@1.identity [ 1 + ] wire@1.then wire@1.apply",
            &[],
            super::LIMITS,
        )
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(applied.output(), &[noble_kernel::types::Ty::I64]);
    let under = session
        .prepare(b"5 7 [ 1 + ] wire@1.under", &[], super::LIMITS)
        .map_err(|error| format!("{error:?}"))?;
    assert_eq!(
        under.output(),
        &[noble_kernel::types::Ty::I64, noble_kernel::types::Ty::I64],
        "the frame restores the held value above the program result"
    );
    // `rot` is deliberately unexported: `first` and `second` use it, but the
    // published surface does not, so a direct use is an unbound word.
    assert!(session
        .prepare(b"1 2 3 wire@1.rot", &[], super::LIMITS)
        .is_err());
    Ok(())
}

#[test]
fn wire_fanout_at_a_resource_names_the_library_word() -> Result<(), String> {
    let mut session = session()?;
    session = super::commit(
        session,
        b"module ledger@1 [ opaque CounterOwner Pair<I64,Resource<test.counter>> private export CounterOwner ]",
    )?;
    session = super::commit(session, noble_contracts::source::WIRE_LIBRARY_SOURCE)?;
    let owner = session
        .resolve_type("ledger@1.CounterOwner")
        .map_err(|error| format!("{error:?}"))?;
    // O11-7: `dup` inside `fanout` meets a non-capturable value, so the
    // specialization is refused at kernel acceptance.
    let refused = session
        .prepare(
            b"[ ] [ ] wire@1.fanout",
            core::slice::from_ref(&owner),
            super::LIMITS,
        )
        .expect_err("fanout at a resource must be refused");
    assert_eq!(
        refused.stage(),
        noble_contracts::source::Stage::Acceptance,
        "eligibility is enforced by the kernel: {refused:?}"
    );
    let message = &refused.diagnostic().message;
    assert!(
        message.contains("wire@1.fanout via dup"),
        "the refusal names the applied library word and the inner word: {message}"
    );
    assert!(
        message.contains("required S Data"),
        "the refusal keeps the required Data predicate: {message}"
    );
    // The bare word keeps its unqualified diagnostic when no library word is
    // applied, so existing refusals are unchanged.
    let bare = session
        .prepare(b"dup", core::slice::from_ref(&owner), super::LIMITS)
        .expect_err("dup at a resource must be refused");
    let bare_message = &bare.diagnostic().message;
    assert!(
        bare_message.contains("; word dup;"),
        "a bare dup is not attributed to a library word: {bare_message}"
    );
    assert!(!bare_message.contains(" via "));
    Ok(())
}
