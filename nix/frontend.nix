# Builds the preflight web frontend (Vite/React) into a static directory.
#
# The output is a derivation whose `out` is the `dist/` directory produced by
# `vite build`. The home-manager module passes this path to the server's
# `frontend_dist` config option so the server serves the SPA from the same
# origin as the GraphQL endpoint.
#
# `VITE_GRAPHQL_ENDPOINT=/` makes the production build POST to the same
# origin's root path (the server's graphql handler at `/`), replacing the
# Vite dev proxy.
{
  pkgs,
  src,
}:

pkgs.buildNpmPackage {
  pname = "preflight-frontend";
  version = "0.1.0";
  src = "${src}/frontend";

  npmDepsHash = "sha256-fW4VfnDypqmNyJF+YIW/dSd86pJm9ggwlzJVVbJmvys=";

  # Production build: the SPA posts to the same origin.
  npmBuildScript = "build";

  env = {
    VITE_GRAPHQL_ENDPOINT = "/";
  };

  installPhase = ''
    runHook preInstall
    cp -r dist $out
    runHook postInstall
  '';
}
