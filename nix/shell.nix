{
  mkShell,
  callPackage,
  just,
  rustPlatform,
  rust-analyzer,
}:
let
  mainPkg = callPackage ./package.nix { };
in
mkShell {
  inputsFrom = [ mainPkg ];

  packages = [
    just
    rust-analyzer
  ];

  RUST_SRC_PATH = "${rustPlatform.rustLibSrc}";
}
