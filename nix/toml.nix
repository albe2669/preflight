# Renders the home-manager module's `settings` tree into a `config.toml`
# matching the structs in `crates/server/src/config.rs`.
#
# The app's `SyncConfig` is `#[serde(deny_unknown_fields)]`, so the rendered
# `database`, `clock`, `logging`, `sync`, `server`. `[sync]` carries the
# scalar `*_token` / `*_token_path` keys plus the nested `github` and `linear`
# tables; `github.filters` and `linear.filters` are arrays of tables
# (`[[sync.github.filters]]`).
#
# This file is a deliberate, thin mirror of the Rust config schema — keep it
# in lockstep when `config.rs` changes.

{ lib }:

let
  # Convert a camelCase Nix option name to the snake_case TOML key the Rust
  # `Config` structs expect (e.g. `dayStartHour` -> `day_start_hour`,
  # `githubTokenPath` -> `github_token_path`). Section names like `database`,
  # `clock`, `sync`, `github` are already lowercase and pass through unchanged.

  snakeCaseStr = s:
    let
      chars = lib.stringToCharacters s;
      go = acc: prevLower: cs:
        if cs == [ ] then acc
        else let
          c = builtins.head cs;
          rest = builtins.tail cs;
          isUpper = c >= "A" && c <= "Z";
          lower = lib.toLower c;
          # Insert '_' before an uppercase letter that follows a lowercase
          # letter or digit (camelCase boundary), or a digit after lowercase.
          needsSep = isUpper && prevLower;
          acc' = acc + (if needsSep then "_" else "") + lower;
          prevLower' = !isUpper;
        in go acc' prevLower' rest;
    in go "" false chars;

  # TOML literal escaping for strings (the only scalar values we emit are
  # strings, bools, and ints).
  escapeStr = s:
    let s' = builtins.toString s;
    in ''"${lib.replaceStrings [ "\\" "\"" ] [ "\\\\" "\\\"" ] s'}"'';

  # `null` marks an absent optional (e.g. `*_token_path`); it is skipped so
  # the field is omitted from the emitted TOML.
  isNullValue = v: v == null;
  scalar = v:
    if builtins.isString v then escapeStr v
    else if builtins.isBool v then (if v then "true" else "false")
    else if builtins.isInt v then toString v
    else throw "preflight: unsupported config value for TOML: ${builtins.toJSON v}";

  # `key = value` for a single scalar leaf (skips nulls); key is snake_cased.
  scalarLine = key: value:
    if isNullValue value then ""
    else "${snakeCaseStr key} = ${scalar value}\n";

  # Render one array-of-tables value under `[[<header>]]` sections.
  # `filters` rows are flat scalar attrsets (option strings + bools).
  arrayOfTables = header: rows:
    lib.concatStrings (map (row:
      "[[${snakeCaseStr header}]]\n"
      + (lib.concatStrings (lib.mapAttrsToList scalarLine row))
    ) rows);

  # Render a section (attrset) given its full dotted header. Each entry is:
  #   - an attrset  -> nested subsection `[<header>.<name>]` (recursed)
  #   - a list      -> array-of-tables `[[<header>.<name>]]`
  #   - a scalar    -> `name = value` (nulls omitted)
  renderSection = header: attrs:
    let
      scalars = lib.filterAttrs (n: v: !(builtins.isAttrs v) && !(builtins.isList v)) attrs;
      subsections = lib.filterAttrs (n: v: builtins.isAttrs v) attrs;
      arrays = lib.filterAttrs (n: v: builtins.isList v) attrs;
    in
      "[${snakeCaseStr header}]\n"
      + (lib.concatStrings (lib.mapAttrsToList scalarLine scalars))
      + (lib.concatStrings (lib.mapAttrsToList (n: v:
          renderSection "${header}.${snakeCaseStr n}" v
        ) subsections))
      + (lib.concatStrings (lib.mapAttrsToList (n: rows:
          arrayOfTables "${header}.${snakeCaseStr n}" rows
        ) arrays));


  # Render a full config tree (top-level section names) into a TOML string.
  # `cfg` is `{ database = {..}; clock = {..}; sync = {..}; server = {..}; }`.
  renderToml = cfg:
    lib.concatStrings (lib.mapAttrsToList (section: attrs:
      renderSection section attrs
    ) cfg);

in
{
  inherit renderToml;
}
