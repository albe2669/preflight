{
  pkgs,
  ...
}:
let
  projectRoot = builtins.toString ./.;
  migrationDir = "migration";
  serverDir = ./server;
  entityDir = "./src/entities";
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
          ./server
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
    "server:migrate" = {
      exec = "sea-orm-cli migrate";
    };
    "server:clean-server" = {
      exec = "mkdir -p ./server";
    };
    "server:generate-entities" = {
      exec = "sea-orm-cli generate entity -o ${entityDir} --seaography";
      after = [
        "server:migrate"
        "server:clean-server"
      ];
      cwd = builtins.toString serverDir;
    };
    "server:generate-server" = {
      exec = ''
        seaography-cli -o . -e ${entityDir} --framework axum server
      '';
      after = [ "server:generate-entities" ];
      cwd = builtins.toString serverDir;
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
