# Bounded Charon -> Aeneas -> Lean entry point for the actual kernel subject.
# Thin execution and file adapter. Success here is extraction, not refinement.
{
  pkgs,
  aeneas,
  leanTool,
  extractionRust,
  leanDependenciesCheck,
  selection,
}:
let
  config = selection.configuration;
  lean = selection.lean;
in
pkgs.writeShellApplication {
  name = "extract-kernel";
  runtimeInputs = [
    pkgs.coreutils
    pkgs.b3sum
    pkgs.gnugrep
    pkgs.git
    aeneas
    leanTool
    extractionRust
    leanDependenciesCheck
  ];
  text = ''
    set -euo pipefail

    target=${pkgs.lib.escapeShellArg config.target}
    quality_target=${pkgs.lib.escapeShellArg "${config.quality_rust}-${config.target}"}
    extraction_rust=${pkgs.lib.escapeShellArg config.extraction_rust}
    lean_version=${pkgs.lib.escapeShellArg lean.version}
    lean_rev=${pkgs.lib.escapeShellArg lean.rev}
    aeneas_rev=${pkgs.lib.escapeShellArg selection.sources.aeneas.rev}
    charon_rev=${pkgs.lib.escapeShellArg selection.sources.charon.rev}
    subject_manifest=${pkgs.lib.escapeShellArg config.extraction_subject}
    requested_function="noble_kernel::consume_budget"

    usage() {
      echo "usage: extract-kernel --root DIR --selection FILE --inventory FILE --artifact-dir DIR [--proof-dir DIR]" >&2
      echo "       extract-kernel verify --root DIR --selection FILE --inventory FILE --extraction DIR [--function NAME] [--claim smoke|proof-required]" >&2
      exit 2
    }

    verify_main() {
      shift
      local root="" selection_file="" inventory="" extraction="" function_name="noble_kernel::consume_budget" claim="smoke"
      while [ "$#" -gt 0 ]; do
        case "$1" in
          --root) root="$2"; shift 2 ;;
          --selection) selection_file="$2"; shift 2 ;;
          --inventory) inventory="$2"; shift 2 ;;
          --extraction) extraction="$2"; shift 2 ;;
          --function) function_name="$2"; shift 2 ;;
          --claim) claim="$2"; shift 2 ;;
          *) echo "extract-kernel: unknown verify argument $1" >&2; usage ;;
        esac
      done
      [ -n "$root" ] && [ -n "$selection_file" ] && [ -n "$inventory" ] && [ -n "$extraction" ] || usage

      local failures=""
      refuse() {
        failures="$failures $1"
        echo "extract-kernel verify: $1" >&2
      }
      local record="$extraction/extraction.json"
      if [ ! -f "$record" ]; then
        echo "extract-kernel verify: extraction record is missing" >&2
        exit 1
      fi
      field() {
        sed -n "s/.*\"$1\": *\"\([^\"]*\)\".*/\1/p" "$record" | head -1
      }
      sha256_of() { b3sum "$1" | cut -d' ' -f1; }

      if ! grep -q "\"$function_name\"" "$inventory"; then
        refuse "function-not-in-inventory:$function_name"
      fi
      if [ "$claim" = "proof-required" ]; then
        refuse "proof-required-unresolved:no-refinement-proof-exists"
      fi

      local expected actual
      expected=$(field kernel_source_blake3)
      actual=$(sha256_of "$root/crates/noble-kernel/src/lib.rs")
      [ "$expected" = "$actual" ] || refuse "kernel-source-mismatch"
      expected=$(field source_tree)
      actual=$(git -C "$root" rev-parse 'HEAD^{tree}' 2>/dev/null || echo "<no-git-tree>")
      [ "$expected" = "$actual" ] || refuse "source-tree-mismatch"
      expected=$(field inventory_ir_id)
      actual=$(sed -n 's/.*"ir_id": *"\([a-f0-9]\{64\}\)".*/\1/p' "$inventory" | head -1)
      [ "$expected" = "$actual" ] || refuse "inventory-identity-mismatch"
      expected=$(field inventory_coverage_id)
      actual=$(sed -n 's/.*"coverage_id": *"\([a-f0-9]\{64\}\)".*/\1/p' "$inventory" | head -1)
      [ "$expected" = "$actual" ] || refuse "inventory-coverage-mismatch"
      expected=$(field selection_sha256)
      actual=$(sha256_of "$selection_file")
      [ "$expected" = "$actual" ] || refuse "selection-mismatch"
      [ "$(field charon_rev)" = "$charon_rev" ] || refuse "charon-revision-mismatch"
      [ "$(field aeneas_rev)" = "$aeneas_rev" ] || refuse "aeneas-revision-mismatch"
      [ "$(field lean_rev)" = "$lean_rev" ] || refuse "lean-revision-mismatch"
      [ "$(field target)" = "$target" ] || refuse "target-mismatch"
      expected=$(field cargo_lock_blake3)
      actual=$(sha256_of "$root/Cargo.lock")
      [ "$expected" = "$actual" ] || refuse "workspace-lock-mismatch"
      expected=$(field kernel_manifest_blake3)
      actual=$(sha256_of "$root/$subject_manifest")
      [ "$expected" = "$actual" ] || refuse "kernel-manifest-mismatch"

      # The requested function must be a local generated function, not an opaque model.
      if [ -f "$extraction/lean/translation.json" ]; then
        if ! grep -q "\"rust_name\": \"$function_name\"" "$extraction/lean/translation.json"; then
          refuse "function-not-in-generated-output:$function_name"
        fi
      else
        refuse "translation-record-missing"
      fi

      if [ ! -f "$extraction/extraction-blake3.txt" ]; then
        refuse "extraction-ledger-missing"
      else
        while read -r hash path; do
          [ -n "$path" ] || continue
          if [ ! -f "$extraction/$path" ]; then
            refuse "output-missing:$path"
            continue
          fi
          actual=$(sha256_of "$extraction/$path")
          [ "$hash" = "$actual" ] || refuse "output-blake3-mismatch:$path"
        done < "$extraction/extraction-blake3.txt"
      fi

      printf '%s\n' "$failures" > "$extraction/verify-failures.txt"
      if [ -n "$failures" ]; then
        echo "extract-kernel verify: rejected (not M1 acceptance)" >&2
        exit 1
      fi
      echo "extract-kernel verify: binding and outputs consistent (not M1 acceptance, not refinement)"
    }

    main() {
      if [ "''${1:-}" = "verify" ]; then
        verify_main "$@"
        return
      fi
      local root="" selection_file="" inventory="" artifacts="" proof_dir=""
      while [ "$#" -gt 0 ]; do
        case "$1" in
          --root) root="$2"; shift 2 ;;
          --selection) selection_file="$2"; shift 2 ;;
          --inventory) inventory="$2"; shift 2 ;;
          --artifact-dir) artifacts="$2"; shift 2 ;;
          --proof-dir) proof_dir="$2"; shift 2 ;;
          *) echo "extract-kernel: unknown argument $1" >&2; usage ;;
        esac
      done
      [ -n "$root" ] && [ -n "$selection_file" ] && [ -n "$inventory" ] && [ -n "$artifacts" ] || usage
      [ -f "$root/$subject_manifest" ] || { echo "extract-kernel: extraction subject is missing" >&2; exit 2; }
      [ -f "$inventory" ] || { echo "extract-kernel: inventory is missing" >&2; exit 2; }
      grep -q "\"$requested_function\"" "$inventory" || {
        echo "extract-kernel: requested function is not in the compiler-derived inventory" >&2
        exit 1
      }
      [ -f "$root/proofs/m1/lakefile.toml" ] || { echo "extract-kernel: proof root is missing" >&2; exit 2; }

      mkdir -p "$artifacts/llbc" "$artifacts/lean"
      if [ -z "$proof_dir" ]; then proof_dir="$artifacts/proof"; fi
      mkdir -p "$proof_dir"
      cp "$root/proofs/m1/lakefile.toml" "$root/proofs/m1/lake-manifest.json" \
        "$root/proofs/m1/lean-toolchain" "$proof_dir/"
      cmp "$proof_dir/lake-manifest.json" "$root/proofs/m1/lake-manifest.json"

      export PATH="${extractionRust}/bin:$PATH"
      export CARGO_HOME="$artifacts/cargo"
      export CARGO_TARGET_DIR="$artifacts/extraction-target"
      mkdir -p "$CARGO_HOME"

      local bootstrap="passed"
      if [ -d "$proof_dir/.lake/packages" ]; then
        bootstrap="reused"
      else
        (
          cd "$proof_dir"
          timeout 2400 lake --no-cache env true
        ) > "$artifacts/backend-bootstrap.log" 2>&1
        (
          cd "$proof_dir"
          timeout 1800 lake exe cache get
        ) > "$artifacts/backend-cache.log" 2>&1
      fi
      noble-lean-dependencies-check "$proof_dir" > "$artifacts/backend-sources.log" 2>&1
      cmp "$proof_dir/lakefile.toml" "$root/proofs/m1/lakefile.toml"
      cmp "$proof_dir/lean-toolchain" "$root/proofs/m1/lean-toolchain"

      (
        cd "$root"
        timeout 300 charon cargo --preset=aeneas --error-on-warnings \
          --dest-file "$artifacts/llbc/noble_kernel.llbc" -- \
          --manifest-path "$subject_manifest" --lib --all-features --target "$target" --locked --offline
      ) > "$artifacts/charon.log" 2>&1
      timeout 300 aeneas -backend lean -abort-on-error -warnings-as-errors -no-progress-bar -emit-json \
        -dest "$artifacts/lean" "$artifacts/llbc/noble_kernel.llbc" > "$artifacts/aeneas.log" 2>&1

      cp "$artifacts/lean/NobleKernel.lean" "$proof_dir/NobleKernel.lean"
      (
        cd "$proof_dir"
        timeout 1800 lake build NobleKernel
      ) > "$artifacts/lean-build.log" 2>&1
      (
        cd "$proof_dir"
        timeout 1800 lake env lean -DmaxHeartbeats=${toString config.lean_heartbeats} \
          -DmaxRecDepth=${toString config.lean_recursion_depth} \
          -o "$artifacts/lean/NobleKernel.olean" NobleKernel.lean
      ) > "$artifacts/lean-explicit.log" 2>&1
      cmp "$artifacts/lean/NobleKernel.lean" "$proof_dir/NobleKernel.lean"
      noble-lean-dependencies-check "$proof_dir" > "$artifacts/backend-after-lean.log" 2>&1

      (
        cd "$artifacts"
        b3sum llbc/noble_kernel.llbc lean/NobleKernel.lean lean/NobleKernel.olean lean/translation.json
      ) > "$artifacts/extraction-blake3.txt"

      local kernel_source_blake3 source_tree inventory_id coverage_id selection_sha256 cargo_lock_blake3 kernel_manifest_blake3
      kernel_source_blake3=$(b3sum "$root/crates/noble-kernel/src/lib.rs" | cut -d' ' -f1)
      source_tree=$(git -C "$root" rev-parse 'HEAD^{tree}' 2>/dev/null || echo "<no-git-tree>")
      selection_sha256=$(b3sum "$selection_file" | cut -d' ' -f1)
      cargo_lock_blake3=$(b3sum "$root/Cargo.lock" | cut -d' ' -f1)
      kernel_manifest_blake3=$(b3sum "$root/$subject_manifest" | cut -d' ' -f1)
      inventory_id=$(sed -n 's/.*"ir_id": *"\([a-f0-9]\{64\}\)".*/\1/p' "$inventory" | head -1)
      coverage_id=$(sed -n 's/.*"coverage_id": *"\([a-f0-9]\{64\}\)".*/\1/p' "$inventory" | head -1)
      local generated_functions
      generated_functions=$(sed -n 's/^def \([A-Za-z0-9_]*\).*/\1/p' "$artifacts/lean/NobleKernel.lean" | sort -u | tr '\n' ' ')
      printf '%s\n' "$generated_functions" > "$artifacts/generated-functions.txt"
      printf '%s\n' "$inventory_id" > "$artifacts/inventory-id.txt"
      printf '%s\n' "$coverage_id" > "$artifacts/coverage-id.txt"

      cat > "$artifacts/extraction.json" <<JSON
    {
      "schema": "noble-kernel-extraction/v1",
      "status": "extracted",
      "role": "extraction is not a refinement proof",
      "subject": {
        "package": "noble-kernel",
        "manifest": "$subject_manifest",
        "lib": true,
        "all_features": true
      },
      "binding": {
        "kernel_source_blake3": "$kernel_source_blake3",
        "source_tree": "$source_tree",
        "inventory": "$inventory",
        "inventory_ir_id": "$inventory_id",
        "inventory_coverage_id": "$coverage_id",
        "selection_sha256": "$selection_sha256",
        "cargo_lock_blake3": "$cargo_lock_blake3",
        "kernel_manifest_blake3": "$kernel_manifest_blake3",
        "target": "$target",
        "quality_target": "$quality_target",
        "extraction_rust": "$extraction_rust",
        "charon_rev": "$charon_rev",
        "aeneas_rev": "$aeneas_rev",
        "lean_version": "$lean_version",
        "lean_rev": "$lean_rev"
      },
      "function": {
        "requested": "$requested_function",
        "generated": "$generated_functions"
      },
      "outputs": {
        "llbc": "llbc/noble_kernel.llbc",
        "lean": "lean/NobleKernel.lean",
        "olean": "lean/NobleKernel.olean",
        "translation": "lean/translation.json",
        "blake3": "extraction-blake3.txt"
      },
      "phases": {
        "backend_bootstrap": "$bootstrap",
        "backend_cache": "$bootstrap",
        "backend_sources": "passed",
        "charon": "passed",
        "aeneas": "passed",
        "lean_build": "passed",
        "lean_explicit": "passed",
        "byte_comparison": "passed"
      },
      "refinement": "open",
      "non_claims": [
        "extraction-is-not-refinement",
        "generated-lean-compilation-is-not-a-proof",
        "no-wasm-correspondence",
        "cache-authenticity-not-established",
        "not-m1-acceptance"
      ]
    }
    JSON
      echo "extract-kernel: extraction passed (not M1 acceptance)"
    }

    main "$@"
  '';
}
