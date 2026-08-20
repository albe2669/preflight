# Standalone evaluation + smoke harness for the preflight home-manager module.
#
# Evaluates the module with a minimal fake `home` module (homeDirectory +
# packages + `systemd.user.services` / `launchd.agents` option stubs) and
# asserts:
#   - option defaults (db state path, server host/port, timezone)
#   - that enabling the module installs `preflight` and `pftui` launchers
#   - the rendered config.toml parses into the exact section/option layout
#     the server's `Config` structs expect.
#   - frontend_dist is omitted when null (installFrontend defaults to true but
#     the fake eval has no frontend package, so the path is null)
#
# Run: nix-instantiate --strict --eval nix/tests/eval-module.nix
let
  pkgs = import <nixpkgs> { };
  lib = pkgs.lib;

  module = import ../modules/home-manager.nix;
  toml = import ../toml.nix { inherit lib; };

  fakeHome = { lib, ... }: {
    options.home = {
      homeDirectory = lib.mkOption {
        type = lib.types.str;
        default = "/home/u";
      };
      packages = lib.mkOption {
        type = lib.types.listOf lib.types.unspecified;
        default = [ ];
      };
    };
    options.xdg = {
      configHome = lib.mkOption {
        type = lib.types.str;
        default = "/home/u/.config";
      };
      configFile = lib.mkOption {
        type = lib.types.attrsOf lib.types.unspecified;
        default = { };
      };
    };
    options.systemd.user.services = lib.mkOption {
      type = lib.types.attrsOf lib.types.unspecified;
      default = { };
    };
    options.launchd.agents = lib.mkOption {
      type = lib.types.attrsOf lib.types.unspecified;
      default = { };
    };
  };

  eval = lib.evalModules {
    specialArgs = { inherit pkgs; };
    modules = [
      fakeHome
      ({ config, lib, ... }: {
        imports = [ module ];
        config.programs.preflight.enable = true;
        config.programs.preflight.installFrontend = false;
        config.programs.preflight.settings.clock.timezone = "America/New_York";
        config.programs.preflight.settings.sync.githubTokenPath = "/run/secrets/gh";
        config.programs.preflight.settings.sync.linearTokenPath = "/run/secrets/lin";
        config.programs.preflight.settings.sync.github.excludeDraftsUnlessAuthoredByMe = true;
      })
    ];
  };

  # Second eval with explicit logging overrides to test custom values
  evalLoggingOverride = lib.evalModules {
    specialArgs = { inherit pkgs; };
    modules = [
      fakeHome
      ({ config, lib, ... }: {
        imports = [ module ];
        config.programs.preflight.enable = true;
        config.programs.preflight.installFrontend = false;
        config.programs.preflight.settings.logging.level = "DEBUG";
        config.programs.preflight.settings.logging.directory = "/var/log/preflight";
        config.programs.preflight.settings.server.corsOrigins = [
          "https://preflight.example.com"
          "http://localhost:5173"
        ];
      })
    ];
  };

  settingsOverride = evalLoggingOverride.config.programs.preflight.settings;
  renderedOverride = toml.renderToml {
    database = settingsOverride.database;
    clock = settingsOverride.clock;
    sync = settingsOverride.sync;
    server = settingsOverride.server;
    logging = settingsOverride.logging;
  };

  settings = eval.config.programs.preflight.settings;
  rendered = toml.renderToml {
    database = settings.database;
    clock = settings.clock;
    sync = settings.sync;
    server = settings.server;
    logging = settings.logging;
  };
  launcherNames = map (p: p.name) eval.config.home.packages;

  # Render assertions (enforced via `assert` below the `in`).
  loggingDefaultOk =
    lib.hasInfix "[logging]" rendered
    && lib.hasInfix "level = \"info\"" rendered
    && lib.hasInfix "directory = \"/home/u/.local/state/preflight/logs\"" rendered;
  loggingOverrideOk =
    lib.hasInfix "level = \"DEBUG\"" renderedOverride
    && lib.hasInfix "directory = \"/var/log/preflight\"" renderedOverride;
  corsDefaultOk = lib.hasInfix "cors_origins = []" rendered;
  corsOverrideOk = lib.hasInfix "cors_origins = [\"https://preflight.example.com\", \"http://localhost:5173\"]" renderedOverride;
  frontendDistDefaultOk = !(lib.hasInfix "frontend_dist" rendered);
in
# Enforce the render assertions: a false value aborts evaluation so a
# toml.nix regression cannot pass silently.
assert loggingDefaultOk;
assert loggingOverrideOk;
assert corsDefaultOk;
assert corsOverrideOk;
assert frontendDistDefaultOk;
{
  dbPath = settings.database.path;
  host = settings.server.host;
  port = settings.server.port;
  tz = settings.clock.timezone;
  launcherNames = launcherNames;
  renderedConfig = rendered;
  inherit
    loggingDefaultOk
    loggingOverrideOk
    corsDefaultOk
    corsOverrideOk
    frontendDistDefaultOk
    ;
}
