# Deterministic fixed-output archive of a pinned Git source.
# Lake requires real Git metadata (remote URL + exact revision) in each
# .lake/packages entry, so the archive keeps .git while removing the sources of
# build-to-build variation: index stat data, reflogs, and default refs.
# outputHashMode = "flat" hashes the tar file itself, not a directory tree.
{
  pkgs,
  name,
  url,
  rev,
  hash,
}:
pkgs.runCommand "lean-git-tar-${name}"
  {
    outputHashMode = "flat";
    outputHashAlgo = "sha256";
    outputHash = hash;
    # The archive carries .git metadata, so allow store-path-looking content.
    unsafeDiscardReferences = {
      out = true;
    };
    nativeBuildInputs = [
      pkgs.git
      pkgs.cacert
      pkgs.gnutar
      pkgs.coreutils
    ];
    SSL_CERT_FILE = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
    GIT_SSL_CAINFO = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
    GIT_TERMINAL_PROMPT = "0";
  }
  ''
    export HOME="$TMPDIR/home"
    mkdir -p "$HOME"
    work="$TMPDIR/${name}"
    git clone --quiet --no-checkout ${url} "$work"
    cd "$work"
    git checkout --quiet ${rev}
    test "$(git rev-parse HEAD)" = ${rev}
    # Remove index stat data and ref-log variation.
    rm -rf .git/logs
    rm -f .git/index
    git read-tree HEAD
    git config --unset-all remote.origin.fetch >/dev/null 2>&1 || true
    git remote set-head origin --delete >/dev/null 2>&1 || true
    git config core.logallrefupdates false >/dev/null 2>&1 || true
    find .git -type f -newermt '1970-01-02' -exec touch -t 197001010000.00 {} + 2>/dev/null || true
    tar --mtime=@0 --sort=name --owner=0 --group=0 --numeric-owner --format=gnu \
      -cf "$out" .
  ''
