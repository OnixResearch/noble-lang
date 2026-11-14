let
  nixpkgsSource = builtins.fetchTree {
    type = "github";
    owner = "NixOS";
    repo = "nixpkgs";
    rev = "9cf7092bdd603554bd8b63c216e8943cf9b12512";
    narHash = "sha256-9tpvMGFteZnd3gRQZFlRCohVpqooygFuy9yjuyRL2C0=";
    lastModified = 1773840656;
  };
  pkgs = import nixpkgsSource { system = "x86_64-linux"; };
in
pkgs.stdenv.mkDerivation {
  pname = "noble-independent-replay-ocaml4lean";
  version = "6f94226df5b91e51af692d05e00b001ee8e0f2fd";
  src = pkgs.fetchFromGitHub {
    owner = "yoonwoojeong";
    repo = "ocaml4lean";
    rev = "6f94226df5b91e51af692d05e00b001ee8e0f2fd";
    hash = "sha256-JMMcTdpEpYpXYxT7OLIAL1yJ76Fh/etp1+QOwzcGtww=";
  };
  nativeBuildInputs = [ pkgs.ocaml pkgs.bash ];
  dontConfigure = true;
  buildPhase = ''
    runHook preBuild
    bash ./build.sh
    runHook postBuild
  '';
  installPhase = ''
    runHook preInstall
    install -D -m 0755 ocaml4lean "$out/bin/ocaml4lean"
    runHook postInstall
  '';
  meta = {
    description = "Independent OCaml Lean 4 export checker for Noble replay measurements";
    license = pkgs.lib.licenses.asl20;
    mainProgram = "ocaml4lean";
    platforms = pkgs.lib.platforms.linux;
  };
}
