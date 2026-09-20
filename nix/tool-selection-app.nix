# Thin process boundary. No environment variable can select replacement tools.
{
  pkgs,
  policy,
  rust,
  extractionRust,
  charon,
  aeneas,
  lean,
  upstreamPinCheck,
  node,
  wasmTools,
  binaryen,
}:
pkgs.writeShellApplication {
  name = "noble-toolchain-check";
  runtimeInputs = [
    pkgs.coreutils
    pkgs.gnugrep
    pkgs.b3sum
  ];
  text = ''
    if [ "$#" -ne 0 ]; then
      echo "tool-selection: no arguments or manual overrides are supported" >&2
      exit 2
    fi
    for name in CHARON_EXE AENEAS_EXE LEAN_PATH LEAN_SRC_PATH LEAN_SYSROOT \
      LAKE_HOME RUSTC RUSTC_WRAPPER RUSTC_WORKSPACE_WRAPPER RUSTFLAGS \
      CARGO_ENCODED_RUSTFLAGS RUSTUP_TOOLCHAIN CHARON_IS_SYMLINK \
      MIRI MIRI_SYSROOT MIRIFLAGS NOBLE_M3_CLI NOBLE_M3_NODE \
      NOBLE_M3_WASM_TOOLS NOBLE_M3_WASM_OPT NODE_OPTIONS; do
      if [[ -v "$name" ]]; then
        echo "tool-selection: forbidden tool override: $name" >&2
        exit 2
      fi
    done
    test -e ${upstreamPinCheck}
    ${rust}/bin/rustc -Vv
    ${extractionRust}/bin/rustc -Vv
    PATH=${extractionRust}/bin:$PATH RUSTC=${extractionRust}/bin/rustc \
      ${extractionRust}/bin/cargo miri --version
    ${charon}/bin/charon version
    ${aeneas}/bin/aeneas -version
    version=$(${lean}/bin/lean --version)
    printf '%s\n' "$version"
    printf '%s\n' "$version" | grep -F ${pkgs.lib.escapeShellArg "version ${policy.lean.version},"}
    printf '%s\n' "$version" | grep -F ${pkgs.lib.escapeShellArg "commit ${policy.lean.rev},"}
    ${lean}/bin/lake --version
    version=$(${node}/bin/node --version)
    printf '%s\n' "$version"
    test "$version" = ${pkgs.lib.escapeShellArg "v${policy.verification_tools.node.version}"}
    version=$(${wasmTools}/bin/wasm-tools --version)
    printf '%s\n' "$version"
    test "$version" = ${pkgs.lib.escapeShellArg "wasm-tools ${policy.verification_tools.wasm_tools.version}"}
    version=$(${binaryen}/bin/wasm-opt --version)
    printf '%s\n' "$version"
    # Binaryen may append its immutable release tag after the numeric version.
    [[ "$version" = ${pkgs.lib.escapeShellArg "wasm-opt version ${policy.verification_tools.binaryen.version}"} \
      || "$version" = ${pkgs.lib.escapeShellArg "wasm-opt version ${policy.verification_tools.binaryen.version} ("}*")" ]]
    b3sum ${rust}/bin/rustc ${extractionRust}/bin/rustc \
      ${extractionRust}/bin/cargo-miri ${extractionRust}/bin/miri ${charon}/bin/charon \
      ${aeneas}/bin/aeneas ${lean}/bin/lean ${lean}/bin/lake \
      ${node}/bin/node ${wasmTools}/bin/wasm-tools ${binaryen}/bin/wasm-opt
    echo "tool-selection: pinned tools available; compatibility execution still required"
  '';
}
