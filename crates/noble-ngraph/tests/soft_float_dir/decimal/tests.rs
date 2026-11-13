use super::{lean_file, sem, COMMITTED, SPELLINGS};

#[test]
fn other_spellings_import_the_same() {
    for (spelling, canonical) in SPELLINGS {
        assert_eq!(
            sem::exact_f32_decimal(spelling),
            sem::exact_f32_decimal(canonical),
            "{spelling} and {canonical}"
        );
    }
}

#[test]
fn the_lean_file_is_the_real_import() {
    assert!(
        lean_file() == COMMITTED,
        "the Rust results changed: regenerate DecimalImportCorpus.lean with `cargo test -p noble-ngraph --test soft_float -- --ignored write_the_lean_file` and rerun the foundation lane"
    );
}

#[test]
#[ignore = "writes proofs/foundation/NobleFoundation/DecimalImportCorpus.lean; run after a deliberate change"]
fn write_the_lean_file() {
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proofs/foundation/NobleFoundation/DecimalImportCorpus.lean"
        ),
        lean_file(),
    )
    .expect("generated file written");
}
