{
  pkgs,
  ...
}:
let
  projectRoot = builtins.toString ./.;
  migrationDir = "crates/migration";
  serverDir = ./crates/server;
  entityDir = "crates/entity/src";
in
{
  # https://devenv.sh/packages/
  packages = with pkgs; [
    git
    sqlite-interactive
    openssl # openssl-sys
    pkg-config # openssl-sys
  ];

  # https://devenv.sh/languages/
  languages.rust = {
    enable = true;
    components = [
      "rustc"
      "cargo"
      "clippy"
      "rustfmt"
      "rust-analyzer"
    ];
  };

  env = {
    DATABASE_URL = "sqlite://${projectRoot}/db.sqlite?mode=rwc";
  };

  # https://devenv.sh/processes/
  processes = {
    "server" = {
      exec = "cargo run";
      cwd = builtins.toString serverDir;
      watch = {
        paths = [
          ./crates/server
        ];
        ignore = [
          "**/target/**"
        ];
      };
    };
  };

  # https://devenv.sh/services/
  # services.postgres.enable = true;

  # https://devenv.sh/scripts/
  scripts = {
    checkOrInstallSeaography.exec = ''
      if ! command -v seaography-cli &> /dev/null; then
        echo "seaography-cli not found, installing..."
        cargo install sea-orm-cli@^2.0.0-rc
        cargo install seaography-cli@^2.0.0-rc
      else
        echo "seaography-cli is already installed."
      fi
    '';
  };

  # https://devenv.sh/basics/
  enterShell = ''
    checkOrInstallSeaography
    git --version # Use packages
  '';

  tasks = {
    "db:migrate" = {
      exec = "cargo run -p migration -- up";
    };
    "db:fresh" = {
      exec = "rm -f db.sqlite db.sqlite-* && cargo run -p migration -- fresh";
    };
    # Regenerate entities from the live schema. NOTE: this overwrites the
    # hand-maintained enum typing in crates/entity/src — re-apply the
    # ActiveEnum column types and the sea_orm_active_enums module afterward.
    "gen:entities" = {
      exec = "sea-orm-cli generate entity -o ${entityDir} --with-serde both --seaography";
      after = [ "db:migrate" ];
    };
    "server:run" = {
      exec = "cargo run -p server";
    };
  };

  # https://devenv.sh/tests/
  enterTest = ''
    echo "Running tests"
    git --version | grep --color=auto "${pkgs.git.version}"
  '';

  # https://devenv.sh/git-hooks/
  git-hooks.hooks = {
    rustfmt.enable = true;
    clippy.enable = true;
  };
}
