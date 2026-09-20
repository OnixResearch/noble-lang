# Reproducible verification shell; the CLI is built from this pinned workspace.
# Explicit CLI overrides are recorded by the harness, never an ambient fallback.
{
  pkgs,
  src,
  noble,
  node,
  wasmTools,
  binaryen,
}:
pkgs.writeShellApplication {
  name = "m3-wasm";
  text = ''
    if [[ -v NODE_OPTIONS ]]; then
      echo "m3-wasm: unreviewed host override: NODE_OPTIONS" >&2
      exit 2
    fi
    select_tool() {
      local name="$1" expected="$2"
      if [[ -v "$name" && "''${!name}" != "$expected" ]]; then
        echo "m3-wasm: unreviewed tool override: $name" >&2
        exit 2
      fi
      export "$name=$expected"
    }
    select_tool NOBLE_M3_NODE ${node}/bin/node
    select_tool NOBLE_M3_WASM_TOOLS ${wasmTools}/bin/wasm-tools
    select_tool NOBLE_M3_WASM_OPT ${binaryen}/bin/wasm-opt
    export NOBLE_M3_CLI="''${NOBLE_M3_CLI-${noble}/bin/noble}"
    exec ${node}/bin/node ${src}/tools/m3-wasm.mjs "$@"
  '';
}
