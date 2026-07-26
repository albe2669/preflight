{
  pkgs,
  ...
}:
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
    DATABASE_URL = "sqlite://db.sqlite?mode=rwc";
  };

  # https://devenv.sh/processes/
  # processes.dev.exec = "${lib.getExe pkgs.watchexec} -n -- ls -la";

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

  # https://devenv.sh/tasks/
  # tasks = {
  #   "myproj:setup".exec = "mytool build";
  #   "devenv:enterShell".after = [ "myproj:setup" ];
  # };

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
