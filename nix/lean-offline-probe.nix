# Offline acceptance probe for the pinned Lean backend proof root.
# Fails if Lake needs network access to resolve the populated packages.
{
  pkgs,
  leanTool,
  proofRoot,
}:
pkgs.runCommand "lean-offline-probe"
  {
    nativeBuildInputs = [
      leanTool
      pkgs.coreutils
      pkgs.git
    ];
  }
  ''
    mkdir -p "$out"
    cp -r ${proofRoot} work
    chmod -R u+w work
    cd work
    export HOME="$TMPDIR/home"
    mkdir -p "$HOME"
    code=0
    timeout 900 lake --no-cache env true > "$out/lake-env.log" 2>&1 || code=$?
    printf '%s\n' "$code" > "$out/lake-env.exit"
    echo "lake env true exit=$code"
    tail -12 "$out/lake-env.log"
    test "$code" = 0
  ''
