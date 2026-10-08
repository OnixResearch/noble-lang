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
    "verification/worker-send/peer/Cargo.lock"
  ];
  expectedLocks = {
    "Cargo.lock" = "414f95bc62d57849a6411aed8d848bd93fe00111dba2c19be4a77e1b6e0161aa";
    "verification/m5/peer/Cargo.lock" = "0d5f4ffc5f42646cc25c539e46c565b95903813ef4cc0fe5256358b3cbf42435";
    "verification/m6/peer/Cargo.lock" = "1280a5944c2a3cd3c37cddd660ac789cfadf165d863a3c71d67e46fad8eb8fd5";
    "verification/m7/peer/Cargo.lock" = "6a90a8cccc740d6c6f8d4126ee3ca7190cdb948d13b364ed91f7af8c300b8b80";
    "verification/intrinsic-proofs/peer/Cargo.lock" = "0fb144cbc8c5a96dd2b1c6d91ecfd74e4dd5268e53a273e96d148ea4ee356feb";
    "verification/intrinsic-named-v2/peer/Cargo.lock" = "64a919d5caefef95e0b4cf1c545a7808161b0299e3f22cfb3aba03a22cc3ba75";
    "verification/result-library/peer/Cargo.lock" = "576bd4877d45d0ba2e305d31ff05a38d030f82d14bf4a45df06f405658115260";
    "verification/declared-modules-v1/peer/Cargo.lock" = "bf925f1c8c9d73f29571c5d63995ae0c93424cd9386ea334ab57882f4cb332b7";
    "verification/worker-send/peer/Cargo.lock" = "91e75cfc90112d55765e7598527f151677076902c2b3434cc5af77069931f7c2";
  };
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
    else if prior != null && lib.elem package.lock prior.locks then
      throw "reviewed vendor: duplicate registry package in ${package.lock}: ${id}"
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
assert builtins.length lockNames == 9;
assert builtins.attrNames expectedLocks == builtins.sort builtins.lessThan lockNames;
assert builtins.all (lock: lock.sha256 == expectedLocks.${lock.name}) locks;
assert builtins.length registry == 615;
assert builtins.length tuples == 241;
pkgs.runCommand "noble-reviewed-offline-vendor" {
  nativeBuildInputs = [ pkgs.python3 ];
  archiveInputs = map (row: row.archive) tuples;
  manifest = builtins.toJSON provenance;
  passAsFile = [ "manifest" ];
  preferLocalBuild = true;
  allowSubstitutes = false;
} ''
  set -euo pipefail
  python3 - "$manifestPath" "$out" <<'PY'
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tarfile

def require(condition, message):
    if not condition:
        raise ValueError(message)

manifest_path, output = map(Path, sys.argv[1:])
manifest = json.loads(manifest_path.read_bytes())
packages = manifest["packages"]
require(manifest["schema"] == "noble-reviewed-offline-vendor/v1",
        "wrong vendor provenance schema")
require(len(manifest["locks"]) == 9 and len(packages) == 241,
        "wrong vendor provenance size")
registry = output / "source-registry-0"
registry.mkdir(parents=True)
for row in packages:
    name, version, checksum = row["name"], row["version"], row["checksum"]
    package = f"{name}-{version}"
    archive = Path(row["archive"])
    with archive.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    require(digest == checksum, f"wrong archive checksum: {package}")
    dest = registry / package
    dest.mkdir()
    seen = set()
    total_bytes = 0
    with tarfile.open(archive, "r:gz") as source:
        for member in source:
            parts = member.name.split("/")
            if parts[-1] == "":
                parts.pop()
            require(len(parts) >= 1 and parts[0] == package
                    and all(part not in ("", ".", "..") for part in parts),
                    f"unsafe archive path: {package}")
            relative = tuple(parts[1:])
            if not relative:
                require(member.isdir(), f"non-directory package root: {package}")
                continue
            require(relative not in seen, f"duplicate archive path: {package}")
            seen.add(relative)
            require(member.isdir() or member.isfile(), f"unsafe archive member type: {package}")
            target = dest.joinpath(*relative)
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                require(member.size >= 0, f"negative archive member size: {package}")
                total_bytes += member.size
                require(total_bytes <= 268435456, f"oversized archive: {package}")
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as content, target.open("xb") as result:
                    shutil.copyfileobj(content, result)
                target.chmod(0o755 if member.mode & 0o111 else 0o644)
    checksum_file = dest / ".cargo-checksum.json"
    with checksum_file.open("x") as result:
        json.dump({"files": {}, "package": checksum}, result, separators=(",", ":"))
        result.write("\n")
require(len(list(registry.iterdir())) == 241, "incomplete extracted vendor")
shutil.copyfile(manifest_path, output / "vendor-provenance.json")
PY
''
