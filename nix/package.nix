# Builds the preflight `server` crate into a binary named `preflight`.
#
# This is a thin naersk wrapper: it runs `cargo build --release -p server`
# in the workspace root and renames the produced `server` executable to
# `preflight`. The server links against the system sqlite and openssl dev
# libraries (sqlx/libsqlite3-sys and openssl-sys are not bundled).
{ naersk, src, system, pkgs }:

naersk.lib.${system}.buildPackage {
  inherit src;

  # Build only the deployable `server` crate (not tui/migration) and emit the
  # release artifact. `$cargo_build_options` keeps naersk's default
  # `--release -j --message-format=json` flags.
  cargoBuild = _: "cargo $cargo_options build -p server $cargo_build_options >> $cargo_build_output_json";

  # libsqlite3-sys + openssl-sys link against system libs at build time.
  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = [ pkgs.sqlite pkgs.openssl ];

  # The crate's binary is named `server`; ship it under the install name
  # `preflight` (used by the home-manager module's `programs.preflight`).
  postInstall = ''
    mv "$out/bin/server" "$out/bin/preflight"
  '';
}
