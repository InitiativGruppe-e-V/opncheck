{
  pkgs,
  lib,
  config,
  ...
}:
{
  languages.rust = {
    enable = true;
    channel = "stable";
  };
}

