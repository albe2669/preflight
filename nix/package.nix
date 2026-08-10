# Builds the preflight `server` and `tui` crates into the `preflight` and
# `pftui` binaries.
#
# This is a thin naersk wrapper: it runs `cargo build --release -p server
# -p tui` in the workspace root and renames the produced `server` and `tui`
# executables to `preflight` and `pftui`. Both crates link against the system
# sqlite and openssl dev libraries (sqlx/libsqlite3-sys and openssl-sys are
# not bundled).
{
  naersk,
  src,
  system,
  pkgs,
}:

naersk.lib.${system}.buildPackage {
  inherit src;

  # Build only the deployable `server` and `tui` crates (not migration) and
  # emit the release artifact. `$cargo_build_options` keeps naersk's default
  # `--release -j --message-format=json` flags.
  cargoBuild =
    _: "cargo $cargo_options build -p server -p tui $cargo_build_options >> $cargo_build_output_json";

  # libsqlite3-sys + openssl-sys link against system libs at build time.
  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = [
    pkgs.sqlite
    pkgs.openssl
  ];

  # The crates' binaries are named `server` and `tui`; ship them under the
  # install names `preflight` and `pftui` (used by the home-manager module's
  # `programs.preflight`).
  postInstall = ''
    mv "$out/bin/server" "$out/bin/preflight"
    mv "$out/bin/tui" "$out/bin/pftui"
  '';
}
