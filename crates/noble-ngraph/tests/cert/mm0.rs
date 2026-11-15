//! Rust-produced subject bytes for the optional MM0 constant-fold pilot.
//! The ordinary test regenerates and checks every retained byte and admits both
//! endpoints. MM0 proof production and verification are separate Node commands.

const LANE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../verification/ngraph/mm0/subjects"
);

fn fixture(first: i64, second: i64) -> [(String, String); 3] {
    let ids = crate::differential::TestIds;
    let before =
        crate::differential::constant_sum(crate::differential::SumOperands { first, second });
    let (derivation, after) = crate::differential::fold(&before);
    assert_eq!(
        derivation.steps.len(),
        1,
        "exactly one FoldAddConstants step"
    );
    assert_eq!(
        derivation.steps[0].rule,
        noble_ngraph::cert::Rule::FoldAddConstants
    );
    assert_eq!(
        noble_ngraph::cert::check(&ids, &before, &after, &derivation),
        Ok(()),
        "the Rust checker admits the source-generated certificate"
    );
    assert!(noble_ngraph::check::check(&ids, before.clone()).is_ok());
    assert!(noble_ngraph::check::check(&ids, after.clone()).is_ok());
    [
        (
            "before.graph".to_owned(),
            noble_ngraph::format::graph::print(&before),
        ),
        (
            "after.graph".to_owned(),
            noble_ngraph::format::graph::print(&after),
        ),
        (
            "certificate.wire".to_owned(),
            noble_ngraph::cert::encode(&derivation),
        ),
    ]
}

fn fixtures() -> [(&'static str, [(String, String); 3]); 2] {
    [("small", fixture(3, 5)), ("boundary", fixture(i64::MAX, 1))]
}

#[test]
fn retained_subjects_equal_source_generated_admitted_pairs() {
    let ids = crate::differential::TestIds;
    for (name, files) in fixtures() {
        let path = |part: &str| format!("{LANE}/{name}/{part}");
        for (part, generated) in files {
            assert_eq!(
                std::fs::read_to_string(path(&part)).expect("retained source byte is readable"),
                generated,
                "the {name}/{part} retained bytes match Rust emission"
            );
        }
        let before = noble_ngraph::format::graph::parse(
            &ids,
            &std::fs::read_to_string(path("before.graph")).expect("original graph is retained"),
        )
        .expect("original graph parses");
        let after = noble_ngraph::format::graph::parse(
            &ids,
            &std::fs::read_to_string(path("after.graph")).expect("folded graph is retained"),
        )
        .expect("folded graph parses");
        let cert = noble_ngraph::cert::decode(
            &std::fs::read_to_string(path("certificate.wire")).expect("certificate is retained"),
        )
        .expect("certificate decodes")
        .bind(before.logical_id(&ids), after.logical_id(&ids));
        assert!(noble_ngraph::check::check(&ids, before.clone()).is_ok());
        assert!(noble_ngraph::check::check(&ids, after.clone()).is_ok());
        assert_eq!(
            noble_ngraph::cert::check(&ids, &before, &after, &cert),
            Ok(()),
            "retained bytes pass the real independent Rust checker"
        );
    }
}

#[test]
#[ignore = "write Rust-produced pilot subjects after a deliberate source change"]
fn write_retained_subjects() {
    for (name, files) in fixtures() {
        let dir = format!("{LANE}/{name}");
        std::fs::create_dir_all(&dir).expect("fixture directory is created");
        for (part, body) in files {
            std::fs::write(format!("{dir}/{part}"), body).expect("subject bytes are written");
        }
    }
}
