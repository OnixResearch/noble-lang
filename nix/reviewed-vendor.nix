{ pkgs, root }:
let
  inherit (pkgs) lib;
  lockNames = [
    "Cargo.lock"
    "verification/m5/peer/Cargo.lock"
    "verification/m6/peer/Cargo.lock"
    "verification/m7/peer/Cargo.lock"
    "verification/intrinsic-proofs/peer/Cargo.lock"
    "verification/intrinsic-named-v2/peer/Cargo.lock"
    "verification/result-library/peer/Cargo.lock"
    "verification/declared-modules-v1/peer/Cargo.lock"
  ];
  locks = map (name: {
    inherit name;
    path = root + "/${name}";
    sha256 = builtins.hashFile "sha256" (root + "/${name}");
  }) lockNames;
  packages = lib.concatMap (lock: map (package: package // { lock = lock.name; })
    (builtins.fromTOML (builtins.readFile lock.path)).package) locks;
  registry = builtins.filter (package:
    (package.source or null) == "registry+https://github.com/rust-lang/crates.io-index") packages;
  checked = lib.foldl' (acc: package:
    let
      id = "${package.name}-${package.version}";
      prior = acc.${id} or null;
    in
    if !(builtins.match "[0-9a-f]{64}" (package.checksum or "") != null) then
      throw "reviewed vendor: invalid lock checksum for ${id}"
    else if prior != null && prior.checksum != package.checksum then
      throw "reviewed vendor: conflicting registry checksums for ${id}"
    else acc // {
      "${id}" = {
        inherit (package) name version checksum;
        locks = (if prior == null then [ ] else prior.locks) ++ [ package.lock ];
      };
    }) { } registry;
  tuples = map (id:
    let row = checked.${id}; in row // {
      archive = pkgs.fetchurl {
        name = "${id}.crate";
        url = "https://static.crates.io/crates/${row.name}/${id}.crate";
        sha256 = row.checksum;
      };
    }) (builtins.attrNames checked);
  provenance = {
    schema = "noble-reviewed-offline-vendor/v1";
    locks = builtins.listToAttrs (map (lock: { name = lock.name; value = lock.sha256; }) locks);
    packages = map (row: {
      inherit (row) name version checksum locks;
      archive = toString row.archive;
    }) tuples;
  };
in
assert builtins.length lockNames == 8;
assert builtins.length tuples == 237;
pkgs.runCommand "noble-reviewed-offline-vendor" {
  nativeBuildInputs = [ pkgs.gnutar pkgs.gzip pkgs.coreutils pkgs.jq ];
  archiveInputs = map (row: row.archive) tuples;
  manifest = builtins.toJSON provenance;
  passAsFile = [ "manifest" ];
  preferLocalBuild = true;
  allowSubstitutes = false;
} ''
  set -euo pipefail
  mkdir -p "$out/source-registry-0"
  jq -r '.packages[] | [.name, .version, .checksum, .archive] | @tsv' "$manifestPath" |
  while IFS="$(printf '\t')" read -r name version checksum archive; do
    id="$name-$version"
    printf '%s  %s\n' "$checksum" "$archive" | sha256sum --check --status
    tar -tzf "$archive" | while IFS= read -r entry; do
      case "$entry" in
        "$id"|"$id/"|"$id/"*) ;;
        *) printf 'unexpected archive entry: %s in %s\n' "$entry" "$id" >&2; exit 1 ;;
      esac
    done
    dest="$out/source-registry-0/$id"
    mkdir -p "$dest"
    tar -xzf "$archive" --strip-components=1 --no-same-owner --no-same-permissions -C "$dest"
    printf '{"files":{},"package":"%s"}\n' "$checksum" > "$dest/.cargo-checksum.json"
  done
  test "$(find "$out/source-registry-0" -mindepth 1 -maxdepth 1 -type d | wc -l)" -eq 237
  cp "$manifestPath" "$out/vendor-provenance.json"
''
