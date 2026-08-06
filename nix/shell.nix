{
  mkShell,
  callPackage,
  just,
}:
let
  mainPkg = callPackage ./package.nix { };
in
mkShell {
  inputsFrom = [ mainPkg ];
  packages = [ just ];
}
