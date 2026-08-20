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
  settings.formatter.blueprint = {
    command = "${pkgs.blueprint-compiler}/bin/blueprint-compiler";
    options = [
      "format"
      "--fix"
      "--no-diff"
    ];
    includes = [ "*.blp" ];
  };
}
