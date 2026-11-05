//! Replays every committed fuzz regression, and every tracked `.noble` and
//! `.wit` fixture under the default budget and three deliberately tiny ones,
//! through the same oracles the libFuzzer targets in `crates/noble-wasm/fuzz`
//! use. A fixed finding therefore stays fixed in the ordinary test suite,
//! without libFuzzer. This is replay of known inputs, not fuzzing.
#![allow(
    tigerstyle::assertion_density,
    tigerstyle::missing_const_fn,
    tigerstyle::non_trait_imports,
    tigerstyle::allocation_in_loop,
    tigerstyle::function_length,
    tigerstyle::no_recursion,
    tigerstyle::ambient_env,
    reason = "Owner: noble-maintainers; this replay walks the committed fixture tree and reports every oracle violation it finds, so recursive directory reads, owned paths and collected reports are the readable form."
)]

/// One oracle and the regression directory its findings are committed under.
type Oracle = fn(&[u8]) -> Result<(), String>;

fn prepare_to_wasm(data: &[u8]) -> Result<(), String> {
    super::oracles::check_prepare_to_wasm(data).map(|_| ())
}

const TARGETS: [(&str, Oracle); 4] = [
    ("prepare", super::oracles::check_prepare),
    ("source", super::oracles::check_source),
    ("prepare_to_wasm", prepare_to_wasm),
    ("wit_world", super::oracles::check_wit_world),
];

/// The default budget, then tiny ones that reach the limit branches. The fifth
/// byte selects the source profile and is replaced per profile below.
const HEADERS: [[u8; super::oracles::HEADER]; 4] = [
    [super::oracles::DEFAULT_LIMITS, 0, 0, 0, 0],
    [0, 255, 63, 7, 0],
    [16, 128, 32, 4, 0],
    [2, 40, 10, 3, 0],
];

/// Source profiles the source oracle selects with the fifth header byte.
const PROFILES: u8 = 4;

fn repository() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files(directory: &std::path::Path, found: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let entries = std::fs::read_dir(directory)
        .map_err(|error| format!("{}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("{}: {error}", directory.display()))?;
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if kind.is_dir() {
            if entry.file_name() != "target" {
                files(&path, found)?;
            }
        } else if kind.is_file() {
            found.push(path);
        }
    }
    Ok(())
}

fn fixtures(extension: &str) -> Result<Vec<std::path::PathBuf>, String> {
    let root = repository();
    let mut found = Vec::new();
    for directory in ["crates", "verification"] {
        files(&root.join(directory), &mut found)?;
    }
    found.retain(|path| path.extension().is_some_and(|value| value == extension));
    found.sort();
    Ok(found)
}

#[test]
fn committed_fuzz_regressions_replay_clean() -> Result<(), String> {
    let mut violations = Vec::new();
    let mut replayed = 0usize;
    for (target, oracle) in TARGETS {
        let directory = repository()
            .join("crates/noble-wasm/fuzz/regressions")
            .join(target);
        if !directory.is_dir() {
            continue;
        }
        let mut inputs = Vec::new();
        files(&directory, &mut inputs)?;
        inputs.sort();
        for input in inputs {
            let data =
                std::fs::read(&input).map_err(|error| format!("{}: {error}", input.display()))?;
            replayed += 1;
            if let Err(violation) = oracle(&data) {
                violations.push(format!("{target} {}: {violation}", input.display()));
            }
        }
    }
    assert!(replayed > 0, "no committed fuzz regression was replayed");
    assert!(violations.is_empty(), "{}", violations.join("\n"));
    Ok(())
}

#[test]
fn tracked_fixtures_satisfy_every_fuzz_oracle() -> Result<(), String> {
    let sources = fixtures("noble")?;
    let worlds = fixtures("wit")?;
    assert!(
        !sources.is_empty() && !worlds.is_empty(),
        "fixture tree is missing"
    );
    let mut violations = Vec::new();
    for (target, oracle) in TARGETS {
        let inputs = if target == "wit_world" {
            &worlds
        } else {
            &sources
        };
        let profiles = if target == "source" { PROFILES } else { 1 };
        for input in inputs {
            let body =
                std::fs::read(input).map_err(|error| format!("{}: {error}", input.display()))?;
            for header in HEADERS {
                for profile in 0..profiles {
                    let mut selected = header;
                    selected[super::oracles::HEADER - 1] = profile;
                    let data = super::oracles::encode(selected, &body);
                    if let Err(violation) = oracle(&data) {
                        violations.push(format!(
                            "{target} {} header {selected:?}: {violation}",
                            input.display()
                        ));
                    }
                }
            }
        }
    }
    assert!(violations.is_empty(), "{}", violations.join("\n"));
    Ok(())
}

/// The oracles are not vacuous: tracked contract fixtures reach WAT emission and
/// produce modules that parse, and the WIT oracle selects the world a fixture
/// really declares, which then parses.
#[test]
fn fuzz_oracles_reach_emission_and_real_worlds() -> Result<(), String> {
    let mut emitted = 0usize;
    for input in fixtures("noble")? {
        let body =
            std::fs::read(&input).map_err(|error| format!("{}: {error}", input.display()))?;
        let data = super::oracles::encode(HEADERS[0], &body);
        emitted += super::oracles::check_prepare_to_wasm(&data)?.len();
    }
    assert!(
        emitted > 0,
        "no tracked contract fixture reached WAT emission"
    );
    let wit = std::fs::read(repository().join("crates/noble-wasm/wit/bounded-region.wit"))
        .map_err(|error| error.to_string())?;
    assert_eq!(super::oracles::world_name(&wit), "bounded");
    noble_contracts::component::World::parse(&wit, "bounded", noble_contracts::Limits::default())
        .map_err(|error| format!("{:?}", error.diagnostic))?;
    Ok(())
}
