{
  lib,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "fgscsv";
  version = "0.0.0";

  src = ../.;

  # cargoHash = lib.fakeHash;
  cargoHash = "sha256-roCYIIhD3tjU/IAGSiWCIJ5wdSJJgqId9ET4iMlqsgY=";
}
