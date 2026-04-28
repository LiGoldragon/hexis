{
  description = "hexis — managed-mutable config reconciliation with per-key modes";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";

    blueprint.url = "github:numtide/blueprint";
    blueprint.inputs.nixpkgs.follows = "nixpkgs";

    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";

    crane.url = "github:ipetkov/crane";
  };

  outputs = inputs:
    inputs.blueprint { inherit inputs; }
    // {
      homeModules.default = import ./nix/home-module.nix;
      lib = import ./nix/wrap.nix;
    };
}
