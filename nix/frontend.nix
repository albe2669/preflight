# Builds the preflight web frontend (Vite/React) into a static directory.
#
# The output is a derivation whose `out` is the `dist/` directory produced by
# `vite build`. The home-manager module passes this path to the server's
# `frontend_dist` config option so the server serves the SPA from the same
# origin as the GraphQL endpoint.
#
# `VITE_GRAPHQL_ENDPOINT=/graphql` makes the production build POST to the
# `/graphql` path on the same origin, matching the server route.
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
    VITE_GRAPHQL_ENDPOINT = "/graphql";
  };

  installPhase = ''
    runHook preInstall
    cp -r dist $out
    runHook postInstall
  '';
}
