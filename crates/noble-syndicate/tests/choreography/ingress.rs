#[test]
fn malformed_descriptors_never_create_facets() -> Result<(), noble_syndicate::choreography::Error> {
    let malformed: &[&[u8]] = &[
    b"{}", b"[]", b"\xff", br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[]}"#,
    br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","ready":"false","exit":"withdraw"}]}"#,
    br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","ready":false,"exit":"withdraw","name":"b"}]}"#,
    br#"{"protocol":"noble:choreography/service@1.0.0","protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}"#,
    br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","ready":false,"exit":"withdraw","extra":[0]}]}"#,
    br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","ready":false,"exit":"trap"},{"name":"b","ready":true,"exit":"withdraw"}]}"#,
];
    for &bytes in malformed {
        let profile = noble_syndicate::Profile::new(noble_syndicate::SELECTED_LIMITS)?;
        assert!(noble_syndicate::choreography::Session::admit(
            profile.clone(),
            bytes,
            noble_syndicate::choreography::WitBinding::SERVICE
        )
        .is_err());
        assert_eq!(profile.counts()?, (0, 0, 0));
    }
    assert_eq!(
        noble_syndicate::choreography::project(&[b' '; 513]).err(),
        Some(noble_syndicate::choreography::Error::ByteLimit)
    );
    assert_eq!(noble_syndicate::choreography::project(br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}{}"#).err(), Some(noble_syndicate::choreography::Error::ParserEnd));
    assert_eq!(noble_syndicate::choreography::project(br#"{"protocol":"noble:choreography/service@2.0.0","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}"#).err(), Some(noble_syndicate::choreography::Error::Protocol));
    assert!(noble_syndicate::choreography::project(br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"\u0061","ready":false,"exit":"withdraw"}]}"#).is_ok());
    let one = noble_syndicate::choreography::project(br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","ready":false,"exit":"trap"}]}"#)?;
    assert_eq!(one.publisher_steps(), &[1]);
    assert_eq!(one.subscriber_steps(), &[0, 2]);
    Ok(())
}

#[test]
fn decoded_keys_escapes_and_unicode_are_checked_before_projection(
) -> Result<(), noble_syndicate::choreography::Error> {
    let escaped = br#"{"pro\u0074ocol":"noble:choreography\/service@1.0.0","round\u0073":[{"na\u006de":"cl\u006fck","rea\u0064y":false,"exi\u0074":"withdraw"}]}"#;
    let plan = noble_syndicate::choreography::project(escaped)?;
    assert_eq!(plan.name(0), Some("clock"));
    assert!(noble_syndicate::choreography::project(br#"{"rounds":[{"exit":"withdraw","ready":false,"name":"a"}],"protocol":"noble:choreography/service@1.0.0"}"#).is_ok());
    for bytes in [
    &br#"{"protocol":"noble:choreography/service@1.0.0","pro\u0074ocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}"#[..],
    &br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","na\u006de":"b","ready":false,"exit":"withdraw"}]}"#[..],
    &br#"{"protocol":"\uD83D","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}"#[..],
    &br#"{"protocol":"\uDE00","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}"#[..],
    &br#"{"protocol":"\uD83D\u0041","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}"#[..],
    &br#"{"protocol":"\u00GG","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}"#[..],
    &br#"{"protocol":"\x","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}"#[..],
    &br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"\u0000","ready":false,"exit":"withdraw"}]}"#[..],
    &b"{\"protocol\":\"a\nb\",\"rounds\":[]}"[..],
    &b"{\"protocol\":\"a\0b\",\"rounds\":[]}"[..],
] {
    assert_eq!(noble_syndicate::choreography::project(bytes).err(), Some(noble_syndicate::choreography::Error::Descriptor), "{bytes:?}");
}
    assert_eq!(
    noble_syndicate::choreography::project(br#"{"protocol":"\uD83D\uDE00","rounds":[{"name":"a","ready":false,"exit":"withdraw"}]}"#).err(),
    Some(noble_syndicate::choreography::Error::Protocol)
);
    let depth_four = br#"{"protocol":"noble:choreography/service@1.0.0","rounds":[{"name":"a","ready":false,"exit":"withdraw","extra":[0]}]}"#;
    assert_eq!(
        noble_syndicate::choreography::project(depth_four).err(),
        Some(noble_syndicate::choreography::Error::DepthLimit)
    );
    assert!(noble_syndicate::choreography::project(b"{\"protocol\":\"noble:choreography/service@1.0.0\",\"rounds\":[{\"name\":\"a\",\"ready\":false,\"exit\":\"withdraw\"}]} \t\r\n").is_ok());
    Ok(())
}
