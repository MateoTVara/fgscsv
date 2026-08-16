{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      packages.${system} = rec {
        fgscsv = pkgs.callPackage ./nix/package.nix { };
        default = fgscsv;
      };

      devShells.${system}.default = pkgs.callPackage ./nix/shell.nix { };

      formatter.${system} = pkgs.callPackage ./nix/formatter.nix { };

      checks.${system} = self.packages.${system};
    };
}
