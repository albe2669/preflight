# home-manager module: `programs.preflight`.
#
# Installs the packaged preflight server binary and manages the `config.toml`
# it launches with. Options are a thin, 1:1 mirror of the app's
# `crates/server/src/config.rs` structs (see `config/default.toml`); the
# rendered file must deserialize with zero Rust changes, so keep sections and
# keys in lockstep. `SyncConfig` is `deny_unknown_fields`, so unknown option
# keys are a real bug, not a warning.
#
# Secrets: a GitHub/Linear `*_token_path` pointing at a decrypted
# sops-nix secrets file (e.g. `config.sops.secrets."<name>".path`) is
# authoritative over the inline `*_token`, matching the app's own
# `resolve_token` precedence.

{
  config,
  lib,
  pkgs,
  ...
}:

let
  toml = import ../toml.nix { inherit lib; };
  cfg = config.programs.preflight;

  # Config tree handed to the renderer. `configFile` (when set) replaces the
  # whole generated file, so settings-rendered content is skipped entirely.
  renderedConfig =
    if cfg.configFile != null then
      null
    else
      {
        database = cfg.settings.database;
        clock = cfg.settings.clock;
        logging = cfg.settings.logging;
        sync = cfg.settings.sync;
        server = cfg.settings.server;
      };

  # TUI launcher that points the tui at the server's configured host/port via
  # the `PREFLIGHT_GRAPHQL_ENDPOINT` env var the client reads at startup
  # (`tui/src/main.rs`). The server's default 127.0.0.1:8000 matches the
  # preflight launcher, so the override only diverges when `settings.server`
  # is customized.
  serverAddr = "${cfg.settings.server.host}:${toString cfg.settings.server.port}";
  tuiLauncher = pkgs.writeShellScriptBin "pftui" ''
    export PREFLIGHT_GRAPHQL_ENDPOINT="http://${serverAddr}/"
    exec "${cfg.package}/bin/pftui" "$@"
  '';

  configToml =
    if renderedConfig == null then
      cfg.configFile
    else
      pkgs.writeText "preflight-config.toml" (toml.renderToml renderedConfig);

  # Launcher that hands the rendered (or `configFile`) config to the binary
  # via the `CONFIG` env var the server reads at startup (`Config::load`).
  launcher = pkgs.writeShellScriptBin "preflight" ''
    export CONFIG="${configToml}"
    exec "${cfg.package}/bin/preflight" "$@"
  '';
in
{
  options.programs.preflight = {
    enable = lib.mkEnableOption "the preflight todo server (installs the packaged binary)";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.preflight;
      defaultText = "pkgs.preflight (the flake's package, exposed via overlay)";
      description = "The preflight package to install. Defaults to the flake package.";
    };

    installService = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Run the preflight server as a background service: a `systemd` user
        service on Linux or a `launchd` agent on Darwin (selected by the
        host platform). The server runs continuously and the `pftui` client
        connects to it on demand.
      '';
    };

    # A fully-supplied config file takes precedence over Nix-rendered settings.
    configFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        A complete `config.toml` to launch preflight with. When set, it takes
        precedence over any `settings`-rendered content (the module does not
        merge the two).
      '';
    };

    settings = {
      database = lib.mkOption {
        type = lib.types.submodule {
          options.path = lib.mkOption {
            type = lib.types.str;
            default = "${config.home.homeDirectory}/.local/state/preflight/db.sqlite";
            description = "Absolute path to the SQLite database. Packaged mode has no project root, so a relative path is not meaningful.";
          };
        };
        default = { };
      };

      clock = lib.mkOption {
        type = lib.types.submodule {
          options = {
            timezone = lib.mkOption {
              type = lib.types.str;
              default = "America/Los_Angeles";
              description = "IANA timezone; the logical day is computed in this zone.";
            };
            dayStartHour = lib.mkOption {
              type = lib.types.int;
              default = 4;
              description = "Hour (0-23) at which a logical day starts.";
            };
          };
        };
        default = { };
      };
      logging = lib.mkOption {
        type = lib.types.submodule {
          options = {
            level = lib.mkOption {
              type = lib.types.str;
              default = "info";
              description = "Default log level (e.g. info, debug, warn, error).";
            };
            directory = lib.mkOption {
              type = lib.types.str;
              default = "${config.home.homeDirectory}/.local/state/preflight/logs";
              description = "Absolute path to the log directory. Packaged mode has no project root, so a relative path is not meaningful.";
            };
          };
        };
        default = { };
      };

      sync = lib.mkOption {
        type = lib.types.submodule {
          options = {
            githubToken = lib.mkOption {
              type = lib.types.str;
              default = "";
              description = "Inline GitHub token. Ignored when githubTokenPath is set.";
            };
            githubTokenPath = lib.mkOption {
              type = lib.types.nullOr lib.types.str;
              default = null;
              description = ''
                Path to a file whose trimmed contents are the GitHub token
                (sops-nix style). Wins over the inline githubToken. Point this
                at `config.sops.secrets."<name>".path` for a decrypted secret.
              '';
            };
            linearToken = lib.mkOption {
              type = lib.types.str;
              default = "";
              description = "Inline Linear token. Ignored when linearTokenPath is set.";
            };
            linearTokenPath = lib.mkOption {
              type = lib.types.nullOr lib.types.str;
              default = null;
              description = ''
                Path to a file whose trimmed contents are the Linear token
                (sops-nix style). Wins over the inline linearToken. Point this
                at `config.sops.secrets."<name>".path` for a decrypted secret.
              '';
            };
            github = lib.mkOption {
              type = lib.types.submodule {
                options = {
                  excludeDraftsUnlessAuthoredByMe = lib.mkOption {
                    type = lib.types.bool;
                    default = true;
                    description = "Exclude draft PRs unless authored by the authenticated user.";
                  };
                  filters = lib.mkOption {
                    type = lib.types.listOf (
                      lib.types.submodule {
                        options = {
                          repo = lib.mkOption {
                            type = lib.types.nullOr lib.types.str;
                            default = null;
                          };
                          author = lib.mkOption {
                            type = lib.types.nullOr lib.types.str;
                            default = null;
                          };
                          reviewer = lib.mkOption {
                            type = lib.types.nullOr lib.types.str;
                            default = null;
                          };
                          reviewingTeam = lib.mkOption {
                            type = lib.types.nullOr lib.types.str;
                            default = null;
                          };
                          excludeOthersDrafts = lib.mkOption {
                            type = lib.types.bool;
                            default = false;
                            description = "Exclude drafts not authored by the authenticated user.";
                          };
                          excludeMyDrafts = lib.mkOption {
                            type = lib.types.bool;
                            default = false;
                            description = "Exclude drafts authored by the authenticated user.";
                          };
                        };
                      }
                    );
                    default = [ ];
                    description = "PR filter rules (OR-ed; conditions within a rule AND-ed).";
                  };
                };
              };
              default = { };
            };
            linear = lib.mkOption {
              type = lib.types.submodule {
                options.filters = lib.mkOption {
                  type = lib.types.listOf (
                    lib.types.submodule {
                      options = {
                        team = lib.mkOption {
                          type = lib.types.nullOr lib.types.str;
                          default = null;
                        };
                        assignee = lib.mkOption {
                          type = lib.types.nullOr lib.types.str;
                          default = null;
                        };
                        creator = lib.mkOption {
                          type = lib.types.nullOr lib.types.str;
                          default = null;
                        };
                        projectLead = lib.mkOption {
                          type = lib.types.nullOr lib.types.str;
                          default = null;
                        };
                      };
                    }
                  );
                  default = [ ];
                  description = "Issue filter rules.";
                };
              };
              default = { };
            };
          };
        };
        default = { };
      };

      server = lib.mkOption {
        type = lib.types.submodule {
          options = {
            host = lib.mkOption {
              type = lib.types.str;
              default = "127.0.0.1";
            };
            port = lib.mkOption {
              type = lib.types.port;
              default = 8000;
            };
            depthLimit = lib.mkOption {
              type = lib.types.nullOr lib.types.int;
              default = null;
            };
            complexityLimit = lib.mkOption {
              type = lib.types.nullOr lib.types.int;
              default = null;
            };
            corsOrigins = lib.mkOption {
              type = lib.types.listOf lib.types.str;
              default = [ ];
              description = "Origins the web frontend may call from. Empty uses the server's built-in Vite dev defaults.";
            };
          };
        };
        default = { };
      };
    };
  };
  config = lib.mkIf cfg.enable {
    home.packages = [
      launcher
      tuiLauncher
    ];

    systemd.user.services.preflight = lib.mkIf (cfg.installService && pkgs.stdenv.isLinux) {
      Unit = {
        Description = "Preflight todo server";
        After = [ "network.target" ];
      };

      Service = {
        Type = "simple";
        Environment = [ "CONFIG=${configToml}" ];
        ExecStart = "${cfg.package}/bin/preflight";
        Restart = "on-failure";
        RestartSec = 1;
      };

      Install.WantedBy = [ "default.target" ];
    };

    launchd.agents.preflight = lib.mkIf (cfg.installService && pkgs.stdenv.isDarwin) {
      enable = true;
      config = {
        ProgramArguments = [ "${cfg.package}/bin/preflight" ];
        EnvironmentVariables = {
          CONFIG = "${configToml}";
        };
        KeepAlive = {
          Crashed = true;
          SuccessfulExit = false;
        };
        ProcessType = "Background";
        RunAtLoad = true;
      };
    };
  };
}
