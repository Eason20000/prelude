{ pkgs, ... }:
{
  projectRootFile = "flake.nix";
  programs.nixfmt.enable = true;
  programs.rustfmt.enable = true;

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
