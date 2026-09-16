# r[verify VT-M1-01]
# DTO mutations exercise selection only, never compiler execution or acceptance.
{ policy, observation }:
let
  gate =
    p: o:
    import ./tool-selection.nix {
      policy = p;
      observation = o;
    };
  set =
    object: path: value:
    if path == [ ] then
      value
    else
      let
        name = builtins.head path;
      in
      object
      // {
        ${name} = set (object.${name} or { }) (builtins.tail path) value;
      };
  reject =
    name: code: p: o:
    let
      result = gate p o;
    in
    assert !result.valid;
    assert builtins.elem code result.diagnostics;
    assert !(builtins.tryEval result.enforce).success;
    name;
  badObservation =
    name: code: path: value:
    reject name code policy (set observation path value);
  badPolicy =
    name: code: path: value:
    reject name code (set policy path value) observation;
  sourceCases = builtins.concatLists (
    map (name: [
      (badPolicy "missing reviewed pin:${name}" "source-pin-mismatch:${name}" [
        "sources"
        name
        "rev"
      ] null)
      (badObservation "changed revision:${name}" "source-pin-mismatch:${name}" [
        "sources"
        name
        "rev"
      ] "0000000000000000000000000000000000000000")
      (badObservation "changed source content:${name}" "source-pin-mismatch:${name}" [
        "sources"
        name
        "narHash"
      ] "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=")
      (badObservation "local path without revision:${name}" "source-pin-mismatch:${name}" [
        "sources"
        name
        "rev"
      ] null)
    ]) (builtins.attrNames policy.sources)
  );
  toolCases = builtins.concatLists (
    map (name: [
      (badObservation "changed tool recipe:${name}" "tool-build-mismatch:${name}" [
        "tool_paths"
        name
        "derivation"
      ] "/nix/store/00000000000000000000000000000000-replaced.drv")
      (badObservation "changed executable output:${name}" "tool-build-mismatch:${name}" [
        "tool_paths"
        name
        "output"
      ] "/ambient/tool")
    ]) (builtins.attrNames policy.tool_paths)
  );
  fileCases = map (
    name:
    badObservation "stale file:${name}" "file-binding-mismatch:${name}" [
      "file_sha256"
      name
    ] "0000000000000000000000000000000000000000000000000000000000000000"
  ) (import ./tool-selection-files.nix);
  firstBackend = builtins.head observation.lake_config.require;
  firstPackage = builtins.head observation.lake_manifest.packages;
  reversed = builtins.foldl' (acc: p: [ p ] ++ acc) [ ] observation.lake_manifest.packages;
  pass = gate policy observation;
  orderPass = gate policy (set observation [ "lake_manifest" "packages" ] reversed);
  negativeCases = [
    (badObservation "missing selected tool builds" "tool-build-set" [ "tool_paths" ] { })
    (badPolicy "unknown policy schema" "selection-schema" [ "schema_version" ] "legacy")
    (badPolicy "missing reviewed source set" "source-pin-set" [ "sources" ] { })
    (badObservation "missing observed source set" "source-pin-set" [ "sources" ] { })
    (badPolicy "missing reviewed file set" "file-binding-set" [ "file_sha256" ] { })
    (badObservation "missing observed file set" "file-binding-set" [ "file_sha256" ] { })
    (badObservation "wrong upstream Charon requirement" "aeneas-charon-pin-mismatch" [
      "charon_pin_text"
    ] "# mismatch\n0000000000000000000000000000000000000000\n")
    (badObservation "wrong backend revision" "aeneas-backend-config-mismatch"
      [ "lake_config" "require" ]
      [ (firstBackend // { rev = "0000000000000000000000000000000000000000"; }) ]
    )
    (badObservation "backend path alongside valid Git pin" "aeneas-backend-config-mismatch"
      [ "lake_config" "require" ]
      [ (firstBackend // { path = "/ambient/backend"; }) ]
    )
    (badObservation "external Lake package directory" "lean-package-directory" [
      "lake_manifest"
      "packagesDir"
    ] "/ambient/packages")
    (badObservation "manifest path alongside valid Git pin" "lean-dependency-lock-mismatch" [
      "lake_manifest"
      "packages"
    ] (map (p: p // { path = "/ambient/package"; }) observation.lake_manifest.packages))
    (badObservation "backend path override" "aeneas-backend-config-mismatch"
      [ "lake_config" "require" ]
      [
        {
          name = "aeneas";
          path = "/ambient/backend";
        }
      ]
    )
    (badObservation "missing backend dependency" "aeneas-backend-config-mismatch"
      [ "lake_config" "require" ]
      [ ]
    )
    (badObservation "extra backend dependency" "aeneas-backend-config-mismatch"
      [ "lake_config" "require" ]
      [ firstBackend firstBackend ]
    )
    (badObservation "missing Lake packages" "lean-dependency-lock-mismatch" [
      "lake_manifest"
      "packages"
    ] null)
    (badObservation "noncanonical package name" "lean-dependency-lock-mismatch" [
      "lake_manifest"
      "packages"
    ] (map (p: p // { name = "../outside"; }) observation.lake_manifest.packages))
    (badObservation "empty Lake packages" "lean-dependency-lock-mismatch"
      [ "lake_manifest" "packages" ]
      [ ]
    )
    (badObservation "duplicate Lake package" "lean-dependency-lock-mismatch" [
      "lake_manifest"
      "packages"
    ] (observation.lake_manifest.packages ++ [ firstPackage ]))
    (badObservation "floating Lake revision" "lean-dependency-lock-mismatch"
      [ "lake_manifest" "packages" ]
      [ (firstPackage // { rev = "main"; }) ]
    )
    (badObservation "missing transitive dependency" "lean-dependency-lock-mismatch"
      [ "lake_manifest" "packages" ]
      [ firstPackage ]
    )
    (badObservation "wrong Lean version" "lean-toolchain-mismatch" [
      "lean_toolchain_text"
    ] "leanprover/lean4:v4.28.0\n")
    (badObservation "upstream Lean mismatch" "lean-toolchain-mismatch" [
      "upstream_lean_toolchain_text"
    ] "leanprover/lean4:v4.28.0\n")
    (badObservation "quality Rust mismatch" "quality-rust-mismatch" [ "quality_rust" ] "nightly")
    (badObservation "extraction Rust mismatch" "extraction-rust-mismatch" [
      "extraction_rust"
    ] "nightly")
    (badPolicy "Miri Rust mismatch" "quality-rust-mismatch" [ "configuration" "miri_rust" ] "nightly")
    (badObservation "unsupported host" "unsupported-target" [ "system" ] "aarch64-linux")
    (badPolicy "wrong word size" "unsupported-target" [ "configuration" "word_bits" ] 32)
    (badObservation "changed panic strategy" "rust-profile-mismatch" [
      "profiles"
      "dev"
      "panic"
    ] "abort")
    (badObservation "unchecked overflow" "rust-profile-mismatch" [
      "profiles"
      "release"
      "overflow-checks"
    ] false)
    (badObservation "unreviewed feature" "feature-scope-mismatch" [ "features" ] [ "extra" ])
    (badPolicy "missing Lean artifact hash" "lean-artifact-pin" [ "lean" "archive_sha256" ] null)
    (badPolicy "selection promoted to acceptance" "selection-claim-limits"
      [ "claims" ]
      [ "compatible-toolchain" ]
    )
  ];
  cases = [
    "reviewed input set"
    "Lake package order independence"
  ]
  ++ sourceCases
  ++ toolCases
  ++ fileCases
  ++ negativeCases;
in
assert pass.valid && pass.enforce;
assert orderPass.valid && orderPass.enforce;
builtins.deepSeq cases cases
