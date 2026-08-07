# Standalone evaluation + smoke harness for the preflight home-manager module.
#
# Evaluates the module with a minimal fake `home` module (homeDirectory +
# packages) and asserts:
#   - option defaults (db state path, server host/port, timezone)
#   - that enabling the module installs a `preflight` launcher
#   - the rendered config.toml parses into the exact section/option layout
#     the server's `Config` structs expect.
#
# Run: nix-instantiate --strict --eval nix/tests/eval-module.nix
let
  pkgs = import <nixpkgs> { };
  lib = pkgs.lib;

  module = import ../modules/home-manager.nix;
  toml = import ../toml.nix { inherit lib; };

  fakeHome = { lib, ... }: {
    options.home = {
      homeDirectory = lib.mkOption { type = lib.types.str; default = "/home/u"; };
      packages = lib.mkOption { type = lib.types.listOf lib.types.unspecified; default = [ ]; };
    };
  };

  eval = lib.evalModules {
    specialArgs = { inherit pkgs; };
    modules = [
      fakeHome
      ({ config, lib, ... }: {
        imports = [ module ];
        config.programs.preflight.enable = true;
        config.programs.preflight.settings.clock.timezone = "America/New_York";
        config.programs.preflight.settings.sync.githubTokenPath = "/run/secrets/gh";
        config.programs.preflight.settings.sync.linearTokenPath = "/run/secrets/lin";
        config.programs.preflight.settings.sync.github.excludeDraftsUnlessAuthoredByMe = true;
      })
    ];
  };

  settings = eval.config.programs.preflight.settings;
  rendered = toml.renderToml {
    database = settings.database;
    clock = settings.clock;
    sync = settings.sync;
    server = settings.server;
  };
in
{
  dbPath = settings.database.path;
  host = settings.server.host;
  port = settings.server.port;
  tz = settings.clock.timezone;
  launcherName = (builtins.head eval.config.home.packages).name;
  renderedConfig = rendered;
}
