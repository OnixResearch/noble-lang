{ pkgs, src, revision }:
let
  site = pkgs.runCommand "noble-static-site" { nativeBuildInputs = [ pkgs.bun ]; } ''
    cd ${src}
    bun tools/site.mjs build --out "$out" --revision ${pkgs.lib.escapeShellArg revision}
  '';
  check = pkgs.runCommand "noble-static-site-check" { nativeBuildInputs = [ pkgs.bun ]; } ''
    export TMPDIR="$TMPDIR/site-tests"
    mkdir -p "$TMPDIR"
    cd ${src}
    bun test tools/site.test.mjs
    # Realize the complete static tree as part of the check, not only fixtures.
    test -s ${site}/spec/language/index.html
    test -s ${site}/spec/core-bootstrap/index.html
    mkdir -p "$out"
    printf '%s\n' 'site: canonical family, link checks and reproducibility tests passed' > "$out/validated.txt"
  '';
in
{
  inherit site check;
}
