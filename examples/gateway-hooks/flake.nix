{
  description = "Julius Test project";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/master";
  inputs.devshell.url = "github:numtide/devshell";
  inputs.flake-parts.url = "github:hercules-ci/flake-parts";

  outputs = inputs @ {
    self,
    flake-parts,
    devshell,
    nixpkgs,
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      imports = [
        devshell.flakeModule
      ];

      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "i686-linux"
        "x86_64-darwin"
        "x86_64-linux"
      ];

      perSystem = {
        pkgs,
        system,
        ...
      }: {
        devshells.default = {
          commands = [
            {
              package = pkgs.oha;
              category = "development";
            }
            {
              package = pkgs.jq;
              category = "development";
            }
            {
              package = pkgs.rustup;
              category = "development";
            }
            {
              package = pkgs.cargo-component;
              category = "development";
            }
            {
              package = pkgs.clang;
              category = "development";
            }
          ];
        };
      };
    };
}
