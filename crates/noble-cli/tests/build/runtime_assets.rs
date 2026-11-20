//! The declared owned-runtime helper list must equal the runtime directory.
//!
//! `tools/m3-wasm-policy.mjs` owns the rule: the M3 harness applies it to the
//! tree it snapshots, and this target drives that same function so a helper
//! that is added to `crates/noble-wasm/runtime` without a declaration fails
//! here instead of surfacing as an incomplete source manifest at harness run.

const CONFIG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/core/runtime/config.json");
const RUNTIME: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../noble-wasm/runtime");
const POLICY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tools/m3-wasm-policy.mjs"
);
const ASSET_ROOT: &str = "crates/noble-wasm/runtime";

fn configuration() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(&std::fs::read_to_string(CONFIG)?)?)
}

fn declared(configuration: &serde_json::Value) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let entries = configuration["runtime_assets"]
        .as_array()
        .ok_or("runtime_assets is an array")?;
    entries
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "runtime asset entry is a path string".into())
        })
        .collect()
}

fn selected_node(configuration: &serde_json::Value) -> Result<&str, Box<dyn std::error::Error>> {
    Ok(configuration["tools"]["node"]["path"]
        .as_str()
        .ok_or("selected node path")?)
}

fn observed() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(RUNTIME)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "runtime asset name is UTF-8")?;
        if name.ends_with(".wat") {
            files.push(std::format!("{ASSET_ROOT}/{name}"));
        }
    }
    files.sort();
    Ok(files)
}

/// Run the policy module's own check against one declared/observed pair.
fn policy_check(
    node: &str,
    declared: &[String],
    observed: &[String],
) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(root)?;
    let script = root.join(std::format!(
        "noble-runtime-assets-{}.mjs",
        std::process::id()
    ));
    let module = serde_json::to_string(&std::format!("file://{POLICY}"))?;
    std::fs::write(
        &script,
        std::format!(
            "const policy = await import({module});\n\
             policy.validateRuntimeAssets({{ runtime_assets: JSON.parse(process.argv[2]) }},\n\
             JSON.parse(process.argv[3]));\n"
        ),
    )?;
    Ok(std::process::Command::new(node)
        .arg(&script)
        .arg(serde_json::to_string(declared)?)
        .arg(serde_json::to_string(observed)?)
        .output()?)
}

#[test]
fn the_declared_helper_list_equals_the_runtime_directory() -> Result<(), Box<dyn std::error::Error>>
{
    let configuration = configuration()?;
    let declared = declared(&configuration)?;
    let observed = observed()?;
    let mut sorted = declared.clone();
    sorted.sort();
    assert_eq!(
        sorted, observed,
        "the declaration and {RUNTIME} must name the same helpers"
    );
    let accepted = policy_check(selected_node(&configuration)?, &declared, &observed)?;
    assert!(
        accepted.status.success(),
        "the policy check must accept the real tree: {}",
        String::from_utf8_lossy(&accepted.stderr)
    );
    Ok(())
}

#[test]
fn a_helper_missing_from_the_declaration_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let configuration = configuration()?;
    let observed = observed()?;
    // The declaration that drifted before this test existed: `experiment-cleanup`
    // is emitted and embedded, so its absence must be a refusal, not a warning.
    let missing = std::format!("{ASSET_ROOT}/experiment-cleanup.wat");
    let mut incomplete = declared(&configuration)?;
    incomplete.retain(|entry| entry != &missing);
    assert!(incomplete.len() + 1 == observed.len());
    let refused = policy_check(selected_node(&configuration)?, &incomplete, &observed)?;
    assert_eq!(refused.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains(&std::format!(
            "owned runtime asset is not declared: {missing}"
        )),
        "{stderr}"
    );
    Ok(())
}

#[test]
fn a_declared_helper_missing_from_the_tree_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let configuration = configuration()?;
    let declared = declared(&configuration)?;
    let observed = observed()?;
    let phantom = std::format!("{ASSET_ROOT}/absent-helper.wat");
    let mut extra = declared;
    extra.push(phantom.clone());
    let refused = policy_check(selected_node(&configuration)?, &extra, &observed)?;
    assert_eq!(refused.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains(&std::format!(
            "declared runtime asset is missing from the tree: {phantom}"
        )),
        "{stderr}"
    );
    Ok(())
}
