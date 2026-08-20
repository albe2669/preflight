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
        server = cfg.settings.server // {
          frontendDist =
            if cfg.installFrontend && cfg.settings.server.frontendDist == null then
              "${cfg.frontendPackage}"
            else
              cfg.settings.server.frontendDist;
        };
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

  # XDG path the server, TUI, and services all read. `xdg.configFile` writes
  # the rendered (or `configFile`-sourced) content here; launchers and
  # services point `CONFIG` at it so the user can edit one file.
  configPath = "${config.xdg.configHome}/preflight/config.toml";

  # Launcher that points the server at the XDG config file via the `CONFIG`
  # env var (`Config::load`), so all surfaces read the same editable file.
  launcher = pkgs.writeShellScriptBin "preflight" ''
    export CONFIG="''${XDG_CONFIG_HOME:-$HOME/.config}/preflight/config.toml"
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

    installFrontend = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Build and serve the web frontend from the preflight server. When
        enabled, the frontend_dist config option is set to the built static
        directory and the server serves the SPA at the same origin as the
        GraphQL endpoint.
      '';
    };

    frontendPackage = lib.mkOption {
      type = lib.types.package;
      default =
        pkgs.preflight-frontend
          or (throw "preflight-frontend package not found; import the flake overlay or set frontendPackage explicitly");
      defaultText = "pkgs.preflight-frontend (the flake's frontend package)";
      description = "The preflight frontend package to serve. Defaults to the flake package.";
    };

    installRaycast = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Symlink the preflight Raycast extension into the Raycast extensions
        directory (`$XDG_CONFIG_HOME/raycast/extensions/preflight`). The
        Raycast app picks it up automatically. Only relevant on macOS.
      '';
    };

    raycastPackage = lib.mkOption {
      type = lib.types.package;
      default =
        pkgs.preflight-raycast
          or (throw "preflight-raycast package not found; import the flake overlay or set raycastPackage explicitly");
      defaultText = "pkgs.preflight-raycast (the flake's Raycast package)";
      description = "The preflight Raycast extension package to install. Defaults to the flake package.";
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
            frontendDist = lib.mkOption {
              type = lib.types.nullOr lib.types.str;
              default = null;
              description = "Path to a static directory the server serves as the web frontend. Set automatically when installFrontend is enabled.";
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

    # Write the rendered config to the XDG path all surfaces read. When
    # `configFile` is set, copy its content verbatim; otherwise render from
    # `settings`. Either way the file lives outside the store, editable by
    # the user, and the launchers/services above point `CONFIG` at it.
    xdg.configFile."preflight/config.toml" =
      if renderedConfig == null then
        { source = cfg.configFile; }
      else
        { text = toml.renderToml renderedConfig; };

    systemd.user.services.preflight = lib.mkIf (cfg.installService && pkgs.stdenv.isLinux) {
      Unit = {
        Description = "Preflight todo server";
        After = [ "network.target" ];
      };

      Service = {
        Type = "simple";
        Environment = [ "CONFIG=${configPath}" ];
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
          CONFIG = "${configPath}";
        };
        KeepAlive = {
          Crashed = true;
          SuccessfulExit = false;
        };
        ProcessType = "Background";
        RunAtLoad = true;
      };
    };

    # Symlink the Raycast extension into the Raycast extensions directory so
    # the Raycast app picks it up automatically.
    xdg.configFile."raycast/extensions/preflight" = lib.mkIf cfg.installRaycast {
      source = "${cfg.raycastPackage}";
    };
  };
}
