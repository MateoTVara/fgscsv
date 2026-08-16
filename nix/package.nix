{
  lib,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "fgscsv";
  version = "0.0.0";

  src = ../.;

  # cargoHash = lib.fakeHash;
  cargoHash = "sha256-hZpMfrM/3evCjKWVyx9EleKlvsVdSOKg2vacSLhOlRI=";
}
