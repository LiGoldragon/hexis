{ pkgs, inputs, system, flake, ... }:

let
  toolchain = inputs.fenix.packages.${system}.fromToolchainFile {
    file = flake + "/rust-toolchain.toml";
    sha256 = "sha256-gh/xTkxKHL4eiRXzWv8KP7vfjSk61Iq48x47BEDFgfk=";
  };
in
pkgs.mkShell {
  packages = [
    toolchain
    pkgs.nixfmt-rfc-style
    pkgs.jq
  ];
}
