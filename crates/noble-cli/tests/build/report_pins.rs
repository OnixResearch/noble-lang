//! `--report-schema v4` end to end: it observes this build's identities,
//! refuses every mismatch by name, and records the selection it was bound to.
//!
//! The tests learn the values the running build observes from its own refusals,
//! so no identity is transcribed here by hand: a refusal that stopped naming
//! the observed value would fail these tests. They need the pinned Lean release
//! (`NOBLE_LEAN`), the contract library (`proofs/mc1`) and the selected Node and
//! wasm-tools, so they carry the same `#[ignore]` gate as the other lanes that
//! run the real toolchain.

const POLICY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../policy/tool-selection.json"
);
const LIBRARY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../proofs/mc1");
const CONFIG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/core/runtime/config.json");
const SOURCE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../verification/mc2/contracts/q-one.noble"
);
const CONTRACT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../verification/mc2/contracts/increment.contract"
);
const ZERO: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const SCHEMA: &str = "noble-build-report-pins/v1";
const HELPER_COUNT: u64 = 27;

type Error = Box<dyn std::error::Error>;

fn json(path: &str) -> Result<serde_json::Value, Error> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

fn scratch(name: &str) -> Result<std::path::PathBuf, Error> {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(std::format!(
        "noble-build-pins-{}-{name}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

fn tool(bin: &str, policy: &serde_json::Value, name: &str) -> Result<String, Error> {
    let output = policy["tool_paths"][name]["output"]
        .as_str()
        .ok_or("selected tool output")?;
    Ok(std::format!("{output}/bin/{bin}"))
}

fn lean_binary() -> Result<String, Error> {
    tool("lean", &json(POLICY)?, "lean")
}

/// The tool identities the selected configuration declares, cross-checked
/// against the selected policy: the worker verifies the executable it runs
/// against exactly these paths.
fn declared_tools(
    policy: &serde_json::Value,
    config: &serde_json::Value,
) -> Result<serde_json::Value, Error> {
    // The configuration names the optimizer `wasm_opt`; the policy pins the
    // derivation that provides it as `binaryen`.
    // The selected worker records Node's declared version and the printed
    // `--version` output of the other two executables.
    let declared =
        |name: &str, pin: &str, bin: &str, version: String| -> Result<serde_json::Value, Error> {
            let path = tool(bin, policy, pin)?;
            let selected = config["tools"][name]["path"]
                .as_str()
                .ok_or("selected tool path")?;
            assert_eq!(
                path, selected,
                "{name} agrees between policy and configuration"
            );
            Ok(serde_json::json!({
                "path": selected,
                "version": version,
                "sha256": ZERO,
            }))
        };
    let version = |name: &str| -> Result<String, Error> {
        Ok(config["tools"][name]["version"]
            .as_str()
            .ok_or("selected tool version")?
            .to_owned())
    };
    Ok(serde_json::json!({
        "node": declared("node", "node", "node", version("node")?)?,
        "wasm_tools": declared("wasm_tools", "wasm_tools", "wasm-tools",
            std::format!("wasm-tools {}", version("wasm_tools")?))?,
        "wasm_opt": declared("wasm_opt", "binaryen", "wasm-opt",
            std::format!("wasm-opt version {}", version("wasm_opt")?))?,
    }))
}

fn pin_document(
    policy: &serde_json::Value,
    config: &serde_json::Value,
) -> Result<serde_json::Value, Error> {
    Ok(serde_json::json!({
        "schema": SCHEMA,
        "opt_mode": "off",
        "cli": { "sha256": ZERO },
        // A zero count is an empty pin the selection itself refuses; start at
        // one so the mismatch path reports the count this build observes.
        "helpers": { "root": "crates/noble-wasm/runtime", "count": 1, "set_sha256": ZERO },
        "lean": {
            "binary_sha256": ZERO,
            "toolchain": policy["lean"]["toolchain"].clone(),
            "commit": policy["lean"]["rev"].clone(),
            "library_blake3": ZERO,
        },
        "tools": declared_tools(policy, config)?,
    }))
}

struct Run {
    exit: Option<i32>,
    report: serde_json::Value,
}

fn execute(arguments: &[&str], library: &str) -> Result<Run, Error> {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_noble"))
        .args(arguments)
        .env("NOBLE_LEAN", lean_binary()?)
        .env("NOBLE_CONTRACT_LIBRARY", library)
        .output()?;
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        std::format!(
            "build printed no JSON report ({error}): err={}",
            String::from_utf8_lossy(&output.stderr)
        )
    })?;
    Ok(Run {
        exit: output.status.code(),
        report,
    })
}

fn diagnostic(report: &serde_json::Value) -> Result<(String, String), Error> {
    let message = report["diagnostics"][0]["message"]
        .as_str()
        .ok_or("refusal without a diagnostic message")?;
    let identity = message
        .split(':')
        .next()
        .ok_or("refusal without an identity")?;
    let observed = message
        .split_once(" but this build observed ")
        // Tool versions are printed `--version` strings, so the tail of the
        // message is the value rather than its first token.
        .map(|(_, observed)| observed.trim())
        .ok_or_else(|| std::format!("refusal does not name the observed identity: {message}"))?;
    Ok((identity.to_owned(), observed.to_owned()))
}

/// Write one observed value into the pin document at `identity`.
fn set(pins: &mut serde_json::Value, identity: &str, observed: &str) -> Result<(), Error> {
    let steps: Vec<&str> = identity.split('.').collect();
    let (last, rest) = steps.split_last().ok_or("empty identity")?;
    let mut cursor = pins;
    for step in rest {
        cursor = cursor
            .get_mut(step)
            .ok_or_else(|| std::format!("{identity} does not address the pin document"))?;
    }
    let value = match identity.ends_with(".count") {
        true => serde_json::json!(observed.parse::<u64>()?),
        false => serde_json::json!(observed),
    };
    assert!(
        cursor.get(*last).is_some(),
        "{identity} addresses a declared field"
    );
    cursor[*last] = value;
    Ok(())
}

fn v4_arguments<'a>(pins: &'a str, lean: &'a str, destination: Option<&'a str>) -> Vec<&'a str> {
    let mut arguments = vec![
        "build",
        "--require-proof",
        SOURCE,
        "--contract",
        CONTRACT,
        "--report-schema",
        "v4",
        "--lean-binary",
        lean,
        "--library-tree",
        LIBRARY,
        "--pins",
        pins,
    ];
    if let Some(destination) = destination {
        arguments.push("--out");
        arguments.push(destination);
    }
    arguments
}

/// Build with a pin document until the selection matches what this build
/// observes, learning each identity from the refusal that names it.
fn converge(pins: &mut serde_json::Value, root: &std::path::Path) -> Result<Run, Error> {
    let lean = lean_binary()?;
    let pins_path = root.join("pins.json");
    let pins_path = pins_path.to_str().ok_or("pin path is UTF-8")?;
    let mut learned: Vec<std::string::String> = Vec::new();
    // Eight static and tool identities can be learned, and a matching run must
    // follow the last one.
    for _ in 0..10 {
        std::fs::write(pins_path, pins.to_string())?;
        let run = execute(&v4_arguments(pins_path, &lean, None), LIBRARY)?;
        // The static identities are bound before assembly and the tool
        // identities after it, so only a `complete` record has every value a
        // receipt names; a `partial` refusal still names the identity to learn.
        if run.report["pins"]["status"] == "complete" {
            return Ok(run);
        }
        let (identity, observed) = diagnostic(&run.report)?;
        learned.push(std::format!(
            "{identity}={observed} (exit {:?}, outcome {})",
            run.exit,
            run.report["outcome"]
        ));
        set(pins, &identity, &observed)?;
    }
    Err(std::format!(
        "the refusal walk did not converge on a complete pin document; learned {}",
        learned.join(", ")
    )
    .into())
}

fn refused_with(
    root: &std::path::Path,
    name: &str,
    pins: &serde_json::Value,
    destination: Option<&std::path::Path>,
) -> Result<Run, Error> {
    let lean = lean_binary()?;
    let path = root.join(std::format!("pins-{name}.json"));
    std::fs::write(&path, pins.to_string())?;
    let path = path.to_str().ok_or("pin path is UTF-8")?;
    let destination = match destination {
        Some(destination) => Some(destination.to_str().ok_or("destination is UTF-8")?),
        None => None,
    };
    execute(&v4_arguments(path, &lean, destination), LIBRARY)
}

fn hex(value: &serde_json::Value) -> bool {
    value
        .as_str()
        .is_some_and(|text| text.len() == 64 && text.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

#[test]
#[ignore = "requires the pinned Lean release, proofs/mc1, and the selected Node and wasm-tools"]
fn v1_and_v3_reports_keep_their_exact_field_sets() -> Result<(), Error> {
    let v1 = execute(
        &["build", "--require-proof", SOURCE, "--contract", CONTRACT],
        LIBRARY,
    )?;
    assert_eq!(v1.exit, Some(0), "{}", v1.report["diagnostics"]);
    assert_eq!(v1.report["schema"], "noble-mc2-build/v1");
    assert!(v1.report.get("pins").is_none());
    assert!(v1.report.get("selected_toolchain").is_none());
    assert_eq!(
        v1.report["artifact_correspondence"]["status"],
        "recorded-trusted-build"
    );

    let lean = lean_binary()?;
    let v3 = execute(
        &[
            "build",
            "--require-proof",
            SOURCE,
            "--contract",
            CONTRACT,
            "--report-schema",
            "v3",
            "--lean-binary",
            &lean,
            "--library-tree",
            LIBRARY,
        ],
        LIBRARY,
    )?;
    assert_eq!(v3.exit, Some(0), "{}", v3.report["diagnostics"]);
    assert_eq!(v3.report["schema"], "noble-mc2-build/v3");
    assert!(v3.report.get("pins").is_none());
    assert!(hex(
        &v3.report["selected_toolchain"]["lean"]["binary_sha256"]
    ));
    assert!(hex(
        &v3.report["selected_toolchain"]["library_tree"]["blake3"]
    ));
    Ok(())
}

#[test]
#[ignore = "requires the pinned Lean release, proofs/mc1, and the selected Node and wasm-tools"]
fn v4_binds_every_identity_and_refuses_a_changed_one() -> Result<(), Error> {
    let root = scratch("v4")?;
    let policy = json(POLICY)?;
    let config = json(CONFIG)?;
    let mut pins = pin_document(&policy, &config)?;
    let run = converge(&mut pins, &root)?;
    let report = &run.report;
    assert_eq!(report["schema"], "noble-mc2-build/v4");
    assert_eq!(report["pins"]["schema"], SCHEMA);
    assert_eq!(report["pins"]["status"], "complete");
    assert!(hex(&report["pins"]["selection_sha256"]));
    assert!(hex(&report["pins"]["cli"]["sha256"]));
    assert_eq!(report["pins"]["cli"]["sha256"], pins["cli"]["sha256"]);
    assert_eq!(report["pins"]["helpers"]["count"], HELPER_COUNT);
    assert_eq!(
        report["pins"]["helpers"]["assets"]
            .as_array()
            .ok_or("helper rows")?
            .len(),
        HELPER_COUNT as usize
    );
    assert_eq!(
        report["pins"]["helpers"]["set_sha256"],
        pins["helpers"]["set_sha256"]
    );
    assert_eq!(
        report["pins"]["lean"]["toolchain"],
        policy["lean"]["toolchain"]
    );
    assert_eq!(
        report["pins"]["lean"]["library_blake3"],
        pins["lean"]["library_blake3"]
    );
    assert_eq!(
        report["pins"]["tools"]["node"]["path"],
        config["tools"]["node"]["path"]
    );
    assert_eq!(
        report["pins"]["tools"]["wasm_tools"]["sha256"],
        pins["tools"]["wasm_tools"]["sha256"]
    );
    assert!(report["pins"]["tools"]["node"]["v8"].is_string());
    assert_eq!(report["pins"]["opt_mode"]["mode"], "off");
    assert_eq!(report["pins"]["opt_mode"]["flags"], serde_json::json!([]));
    // No evidence was offered, so the build compiles and assembles but never
    // releases: the pin record does not claim a build was admitted.
    assert_eq!(report["outcome"], "not-run");
    assert_eq!(report["release"]["allowed"], false);

    // A pin that names another CLI is refused before anything is assembled.
    let mut other = pins.clone();
    other["cli"]["sha256"] = serde_json::json!(ZERO);
    let refused = refused_with(&root, "cli", &other, None)?;
    assert_eq!(refused.exit, Some(4));
    assert_eq!(
        refused.report["diagnostics"][0]["code"],
        "build-pin-mismatch"
    );
    assert_eq!(diagnostic(&refused.report)?.0, "cli.sha256");

    // A pin that names another library tree is refused before assembly too.
    let mut other = pins.clone();
    other["lean"]["library_blake3"] = serde_json::json!(ZERO);
    let refused = refused_with(&root, "library", &other, None)?;
    assert_eq!(refused.exit, Some(4));
    assert_eq!(diagnostic(&refused.report)?.0, "lean.library_blake3");

    // A pin that names another optimizer tool is refused after assembly, and
    // the refusal leaves no artifact behind.
    let mut other = pins.clone();
    other["tools"]["wasm_opt"]["sha256"] = serde_json::json!(ZERO);
    let destination = root.join("refused-out");
    let refused = refused_with(&root, "wasm-opt", &other, Some(&destination))?;
    assert_eq!(refused.exit, Some(4));
    assert_eq!(
        refused.report["diagnostics"][0]["code"],
        "build-pin-mismatch"
    );
    assert_eq!(diagnostic(&refused.report)?.0, "tools.wasm_opt.sha256");
    assert!(
        !destination.exists(),
        "a refused pin must not leave an artifact directory"
    );
    Ok(())
}
