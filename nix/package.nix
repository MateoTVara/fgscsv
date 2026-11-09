{
  lib,
  rustPlatform,
}:
let
  inherit (lib) fakeHash;
in 
rustPlatform.buildRustPackage {
  pname = "fgscsv";
  version = "0.0.0";

  src = ../.;

  # cargoHash = fakeHash;
  cargoHash = "sha256-/nc4gxtDbP948YVC/ZTz3Re2KSS9pd5LQ+F4e9dFnG0=";
}
