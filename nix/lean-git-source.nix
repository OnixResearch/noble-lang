# Network-capable fixed-output Git source for the pinned Lean backend.
# Lake requires each .lake/packages entry to be a real Git repository whose
# remote URL and checked-out revision match lake-manifest.json, so a plain
# tree fetch is not sufficient.
{
  pkgs,
  name,
  url,
  rev,
  hash,
}:
pkgs.runCommand "lean-git-${name}"
  {
    outputHashMode = "recursive";
    outputHashAlgo = "sha256";
    outputHash = hash;
    nativeBuildInputs = [
      pkgs.git
      pkgs.cacert
    ];
    SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
    GIT_SSL_CAINFO = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
    GIT_TERMINAL_PROMPT = "0";
  }
  ''
    export HOME="$TMPDIR/home"
    mkdir -p "$HOME"
    git clone --quiet --no-checkout ${url} "$out"
    cd "$out"
    git checkout --quiet ${rev}
    # Reduce non-determinism so the recursive hash is stable.
    rm -rf .git/logs
    rm -f .git/index
    git read-tree HEAD
    git reflog expire --expire=now --all >/dev/null 2>&1 || true
    git remote set-head origin --delete >/dev/null 2>&1 || true
    git config --unset-all remote.origin.fetch >/dev/null 2>&1 || true
    git config --unset-all branch.${rev} >/dev/null 2>&1 || true
    git config core.logallrefupdates false || true
    test "$(git rev-parse HEAD)" = ${rev}
  ''
