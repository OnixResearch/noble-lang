# Pure selection checks. Success is NOT an executed compatibility receipt.
# r[impl VT-M1-01]
{ policy, observation }:
let
  inherit (builtins)
    all
    attrNames
    concatLists
    concatStringsSep
    filter
    isAttrs
    isList
    isString
    length
    map
    match
    sort
    ;
  get = value: name: if isAttrs value then value.${name} or null else null;
  at =
    value: path:
    if path == [ ] then value else at (get value (builtins.head path)) (builtins.tail path);
  equalKeys = value: keys: isAttrs value && attrNames value == sort builtins.lessThan keys;
  hex = size: value: isString value && match "[0-9a-f]{${toString size}}" value != null;
  pinValid =
    value:
    equalKeys value [
      "rev"
      "narHash"
    ]
    && hex 40 value.rev
    && isString value.narHash
    && match "sha256-[A-Za-z0-9+/]{43}=" value.narHash != null;
  require = condition: diagnostic: if condition then [ ] else [ diagnostic ];
  sourceNames = [
    "aeneas"
    "aeneas_nixpkgs"
    "cairn"
    "charon"
    "extraction_nixpkgs"
    "nixpkgs"
    "octet"
    "rust_overlay"
  ];
  fileNames = import ./tool-selection-files.nix;
  toolNames = [
    "aeneas"
    "binaryen"
    "charon"
    "lean"
    "quality_rust"
    "extraction_rust"
    "octet"
    "octet_standards"
    "nickel"
    "node"
    "cairn"
    "upstream_pin_check"
    "wasm_tools"
  ];
  verificationTools = {
    node = {
      source = "nixpkgs";
      attribute = "nodejs_24";
      role = "core-bootstrap-and-verification-wasm-host";
    };
    wasm_tools = {
      source = "nixpkgs";
      attribute = "wasm-tools";
      role = "core-bootstrap-and-verification-wasm-assembler";
    };
    binaryen = {
      source = "nixpkgs";
      attribute = "binaryen";
      role = "core-bootstrap-and-verification-wasm-optimizer";
    };
  };
  verificationChecks = concatLists (
    map (
      name:
      let
        expected = at policy [ "verification_tools" name ];
        version = get expected "version";
      in
      require (
        equalKeys expected [ "source" "attribute" "role" "version" ]
        && builtins.removeAttrs expected [ "version" ] == verificationTools.${name}
      ) "verification-tool-scope:${name}"
      ++ require (
        isString version && version != ""
        && version == at observation [ "verification_tool_versions" name ]
      ) "verification-tool-version-mismatch:${name}"
    ) (attrNames verificationTools)
  );
  wasmAssets = filter (name: match "crates/noble-wasm/runtime/[^/]+\\.wat" name != null) fileNames;
  toolChecks = concatLists (
    map (
      name:
      let
        expected = at policy [
          "tool_paths"
          name
        ];
        actual = at observation [
          "tool_paths"
          name
        ];
      in
      require (
        equalKeys expected [
          "derivation"
          "output"
        ]
        && isString expected.derivation
        && isString expected.output
        && match "/nix/store/[0-9a-z]{32}-[^/]+\\.drv" expected.derivation != null
        && match "/nix/store/[0-9a-z]{32}-[^/]+" expected.output != null
        && expected == actual
      ) "tool-build-mismatch:${name}"
    ) toolNames
  );
  selected = get policy "sources";
  observed = get observation "sources";
  config = get policy "configuration";
  lean = get policy "lean";
  sourceChecks = concatLists (
    map (
      name:
      require (
        pinValid (get selected name) && get selected name == get observed name
      ) "source-pin-mismatch:${name}"
    ) sourceNames
  );
  fileChecks = concatLists (
    map (
      name:
      require (
        hex 64 (
          at policy [
            "file_sha256"
            name
          ]
        )
        &&
          at policy [
            "file_sha256"
            name
          ] == at observation [
            "file_sha256"
            name
          ]
      ) "file-binding-mismatch:${name}"
    ) fileNames
  );
  aeneasRev = at policy [
    "sources"
    "aeneas"
    "rev"
  ];
  charonRev = at policy [
    "sources"
    "charon"
    "rev"
  ];
  nonempty = value: isString value && value != "";
  line = text: value: nonempty value && text == value + "\n";
  charonPin = get observation "charon_pin_text";
  # The upstream file has one comment and one exact revision line.
  charonPinMatches =
    hex 40 charonRev && isString charonPin && match "#[^\n]*\n${charonRev}\n" charonPin != null;
  requires = at observation [
    "lake_config"
    "require"
  ];
  backendMatches =
    isList requires
    && length requires == 1
    && (
      let
        backend = builtins.head requires;
      in
      equalKeys backend [
        "name"
        "git"
        "rev"
      ]
      && equalKeys (get backend "git") [
        "url"
        "subDir"
      ]
      && get backend "name" == "aeneas"
      && get backend "rev" == aeneasRev
      &&
        at backend [
          "git"
          "url"
        ] == get lean "backend_url"
      &&
        at backend [
          "git"
          "subDir"
        ] == get lean "backend_subdir"
    );
  packages = at observation [
    "lake_manifest"
    "packages"
  ];
  upstream = at observation [
    "upstream_lake_manifest"
    "packages"
  ];
  packageValid =
    p:
    equalKeys p [
      "name"
      "type"
      "url"
      "rev"
      "subDir"
      "configFile"
      "manifestFile"
      "scope"
      "inputRev"
      "inherited"
    ]
    && get p "type" == "git"
    && hex 40 (get p "rev")
    && nonempty (get p "name")
    && match "[A-Za-z0-9_][A-Za-z0-9_-]*" p.name != null
    && nonempty (get p "url");
  normalizePackage =
    p:
    map (name: get p name) [
      "name"
      "type"
      "url"
      "rev"
      "subDir"
      "configFile"
      "manifestFile"
    ];
  normalized = ps: map normalizePackage (sort (a: b: a.name < b.name) ps);
  namesUnique =
    ps:
    length ps == length (
      attrNames (
        builtins.listToAttrs (
          map (p: {
            name = p.name;
            value = true;
          }) ps
        )
      )
    );
  backendPackages = if isList packages then filter (p: get p "name" == "aeneas") packages else [ ];
  dependencyPackages = if isList packages then filter (p: get p "name" != "aeneas") packages else [ ];
  manifestMatches =
    isList packages
    && isList upstream
    && packages != [ ]
    && length packages <= 128
    && all packageValid packages
    && all packageValid upstream
    && namesUnique packages
    && namesUnique upstream
    && length backendPackages == 1
    && get (builtins.head backendPackages) "rev" == aeneasRev
    && get (builtins.head backendPackages) "url" == get lean "backend_url"
    && get (builtins.head backendPackages) "subDir" == get lean "backend_subdir"
    && normalized dependencyPackages == normalized upstream;
  profileMatches =
    name:
    at observation [
      "profiles"
      name
      "panic"
    ] == get config "panic"
    &&
      at observation [
        "profiles"
        name
        "overflow-checks"
      ] == get config "overflow_checks";
  diagnostics = concatLists [
    (require (get policy "schema_version" == "noble-tool-selection-policy/v1") "selection-schema")
    (require (equalKeys selected sourceNames && equalKeys observed sourceNames) "source-pin-set")
    sourceChecks
    (require (
      equalKeys (get policy "tool_paths") toolNames && equalKeys (get observation "tool_paths") toolNames
    ) "tool-build-set")
    toolChecks
    (require (
      equalKeys (get policy "verification_tools") (attrNames verificationTools)
      && equalKeys (get observation "verification_tool_versions") (attrNames verificationTools)
    ) "verification-tool-set")
    verificationChecks
    (require (
      equalKeys (get policy "owned_wasm_runtime") [ "assets" "assurance" "non_claims" ]
      && at policy [ "owned_wasm_runtime" "assets" ] == wasmAssets
      && at policy [ "owned_wasm_runtime" "assurance" ] == "byte-bound-runtime-semantics-unproved"
      && at policy [ "owned_wasm_runtime" "non_claims" ] == [
        "rust-compiler-inventory-coverage"
        "rust-refinement"
        "wasm-lowering-correspondence"
        "loader-binding-proof"
        "component-support"
      ]
    ) "owned-wasm-runtime-boundary")
    (require (
      get policy "future_unselected" == [ "Verus" "byte-view dependency" ]
    ) "unselected-tool-set")
    (require (
      get policy "component_sync" == get observation "component_sync"
      && at policy [ "component_sync" "schema" ] == "noble-m5-tool-pins/v1"
      && at policy [ "component_sync" "profile" ] == "Component-Sync-Bootstrap"
      && at policy [ "component_sync" "world" ] == "noble-test:sync/bootstrap@1.0.0"
      && at policy [ "component_sync" "wasmtime_version" ] == "40.0.2"
      && at policy [ "component_sync" "wasm_tools_version" ] == at policy [ "verification_tools" "wasm_tools" "version" ]
      && at policy [ "component_sync" "canonical_abi" ] == "memory32-sync-utf8"
      && at policy [ "component_sync" "non_claims" ] == [
        "Component-Draft"
        "WASI profile implementation"
        "native async/future/stream execution"
        "universal Canonical ABI or engine refinement"
      ]
    ) "component-sync-selection")
    (require (
      equalKeys (get policy "file_sha256") fileNames
      && equalKeys (get observation "file_sha256") fileNames
    ) "file-binding-set")
    fileChecks
    (require charonPinMatches "aeneas-charon-pin-mismatch")
    (require backendMatches "aeneas-backend-config-mismatch")
    (require manifestMatches "lean-dependency-lock-mismatch")
    (require (
      at observation [
        "lake_manifest"
        "packagesDir"
      ] == ".lake/packages"
      &&
        at observation [
          "lake_manifest"
          "lakeDir"
        ] == ".lake"
    ) "lean-package-directory")
    (require (
      line (get observation "lean_toolchain_text") (get lean "toolchain")
      && line (get observation "upstream_lean_toolchain_text") (get lean "toolchain")
    ) "lean-toolchain-mismatch")
    (require (
      get observation "quality_rust" == get config "quality_rust"
      && get config "miri_rust" == get observation "extraction_rust"
    ) "quality-rust-mismatch")
    (require (
      nonempty (get config "extraction_rust")
      && get observation "extraction_rust" == get config "extraction_rust"
    ) "extraction-rust-mismatch")
    (require (
      get observation "system" == "x86_64-linux"
      && get config "host" == "x86_64-linux"
      && get config "target" == "x86_64-unknown-linux-gnu"
      && get config "word_bits" == 64
    ) "unsupported-target")
    (require (
      get config "panic" == "unwind"
      && get config "overflow_checks" == true
      && profileMatches "dev"
      && profileMatches "release"
    ) "rust-profile-mismatch")
    (require (
      get config "features" == [ ]
      && get observation "features" == [ ]
      && get config "all_features" == true
      && get config "default_features" == true
    ) "feature-scope-mismatch")
    (require (
      hex 40 (get lean "rev")
      && hex 64 (get lean "archive_sha256")
      && get lean "backend_subdir" == "backends/lean"
      && get lean "backend_url" == "https://github.com/AeneasVerif/aeneas"
    ) "lean-artifact-pin")
    (require (
      get policy "claims" == [
        "selection-policy-only"
        "execution-required-for-compatibility"
        "refinement-open"
        "m1-acceptance-open"
      ]
    ) "selection-claim-limits")
  ];
in
{
  inherit diagnostics;
  valid = diagnostics == [ ];
  enforce =
    if diagnostics == [ ] then
      true
    else
      throw ("Noble tool selection rejected: " + concatStringsSep ", " diagnostics);
}
