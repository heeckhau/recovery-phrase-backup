# Pinned toolchain for the cross-tool recovery suite (README.md). Python and
# Go come from a fixed nixpkgs revision; the oracle packages themselves are
# pinned by version in requirements-*.txt and (seedkit) in run.py.
let
  nixpkgs = fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/8edc0c72e3a38faf5434e40d1f19431125fe4b30.tar.gz";
    sha256 = "0cwxl7z1x0ad9z8lcljl390y9lk73ql4fvx5lysy4s9c6723p8xn";
  };
  pkgs = import nixpkgs { };
in
pkgs.mkShell {
  packages = [
    pkgs.python312
    pkgs.go
  ];
}
