# Upstream release bytes are fixed by SHA-256; Nix patches the ELF interpreter.
# This package is a tool dependency, not Noble-owned verified code.
{ pkgs, selection }:
pkgs.stdenvNoCC.mkDerivation {
  pname = "noble-lean-release";
  version = selection.lean.version;
  src = pkgs.fetchurl {
    url = selection.lean.archive_url;
    sha256 = selection.lean.archive_sha256;
  };
  nativeBuildInputs = [
    pkgs.zstd
    pkgs.autoPatchelfHook
  ];
  buildInputs = [ pkgs.stdenv.cc.cc.lib ];
  dontConfigure = true;
  dontBuild = true;
  dontStrip = true;
  installPhase = ''
    runHook preInstall
    mkdir -p "$out"
    cp -r ./. "$out/"
    runHook postInstall
  '';
}
