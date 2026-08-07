{
  description = "preflight — a todo planner with GitHub/Linear sync";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    # naersk builds the workspace's `server` crate into the `preflight` binary.
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, naersk }: let
    systems = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
    forAllSystems = nixpkgs.lib.genAttrs systems;
    src = ./.;
    makePreflight = { system, pkgs }:
      import ./nix/package.nix { inherit naersk src system pkgs; };
  in {
    packages = forAllSystems (system: let
      pkgs = nixpkgs.legacyPackages.${system};
      preflight = makePreflight { inherit system pkgs; };
    in {
      default = preflight;
      preflight = preflight;
    });

    # Overlay that exposes `preflight` as `pkgs.preflight`. The home-manager
    # module's `package` option defaults to this. Importing the overlay is
    # optional: the module honors an explicit `package` override.
    overlays.default = final: prev: {
      preflight = makePreflight {
        system = final.stdenv.hostPlatform.system;
        pkgs = final;
      };
    };

    # home-manager module: `programs.preflight`. Import this in a
    # home-manager config via `homeManagerModules.preflight`.
    homeManagerModules = {
      preflight = import ./nix/modules/home-manager.nix;
    };
  };
}
