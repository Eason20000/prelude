{ pkgs, ... }:
{
  projectRootFile = "flake.nix";
  programs.nixfmt.enable = true;
  programs.rustfmt.enable = true;
  programs.taplo.enable = true;
  programs.mdformat = {
    enable = true;
    settings.wrap = 80;
    plugins = ps: [ ps.mdformat-gfm ];
  };
  # Meson build files. muon only: the programs.meson module matches the same
  # includes, so enabling both would fight over rewrites.
  programs.muon.enable = true;
  # GitHub Actions workflows. Note: yamlfmt folds the MSYS2 `install: >-`
  # package list into one line; keep it folded, setup-msys2 requires a
  # string here, not a YAML list.
  programs.yamlfmt.enable = true;
  # GResource/schema XML plus SVG icons (covered by the module defaults).
  programs.xmllint.enable = true;
  # Stylesheet only. Prettier's defaults also match md/yaml, which belong to
  # mdformat/yamlfmt, so restrict it to CSS here.
  programs.prettier.enable = true;
  settings.formatter.prettier.includes = [ "*.css" ];
  settings.formatter.blueprint = {
    command = "${pkgs.blueprint-compiler}/bin/blueprint-compiler";
    options = [
      "format"
      "--fix"
      "--no-diff"
    ];
    includes = [ "*.blp" ];
  };
  # Generated or canonical files: never format.
  settings.excludes = [
    "*.lock"
    "LICENSE"
  ];
}
