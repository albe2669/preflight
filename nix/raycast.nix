# Builds the preflight Raycast extension into the extension directory format
# that the Raycast app loads from `~/.config/raycast/extensions/<name>/`.
#
# Each command (defined in package.json `commands`) is bundled with esbuild
# into a single `.js` file, matching what `ray build` produces. The output is
# a directory with the built JS, package.json, and assets.
{
  pkgs,
  src,
}:

pkgs.buildNpmPackage {
  pname = "preflight-raycast";
  version = "0.1.0";
  inherit src;

  sourceRoot = "${src.name}/raycast";

  npmDepsHash = pkgs.lib.fakeHash;

  # Install dependencies without running the build script (which needs `ray`,
  # the Raycast CLI — not available in Nix). We bundle with esbuild instead.
  dontNpmBuild = true;

  nativeBuildInputs = [ pkgs.esbuild ];

  # Bundle each command entry point with esbuild, matching `ray build` output.
  buildPhase = ''
    runHook preBuild

    commands=(
      "src/today.tsx:today.js"
      "src/inbox.tsx:inbox.js"
      "src/review.tsx:review.js"
      "src/quick-create.tsx:quick-create.js"
      "src/menubar.tsx:menubar.js"
    )

    for entry in "''${commands[@]}"; do
      src=''${entry%%:*}
      out=''${entry##*:}
      esbuild "$src" \
        --bundle \
        --outfile="$out" \
        --format=esm \
        --platform=node \
        --target=node20 \
        --jsx=automatic \
        --loader:.tsx=tsx \
        --loader:.ts=ts \
        --external:@raycast/api \
        --external:@raycast/utils \
        --external:react \
        --external:react-dom \
        --sourcemap
    done

    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    mkdir $out
    cp package.json $out/
    cp *.js *.js.map $out/ 2>/dev/null || true
    cp -r assets $out/ 2>/dev/null || true
    runHook postInstall
  '';
}
