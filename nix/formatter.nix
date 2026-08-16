{
  treefmt,
  nixfmt,
  rustfmt,
  keep-sorted,
}:
treefmt.withConfig {
  runtimeInputs = [
    nixfmt
    rustfmt
    keep-sorted
  ];

  settings = {
    on-unmatched = "info";
    tree-root-file = "flake.nix";
    formatter = {
      nixfmt = {
        command = "nixfmt";
        includes = [ "*.nix" ];
      };
      rustfmt = {
        command = "rustfmt";
        options = [
          "--edition"
          "2024"
        ];
        includes = [ "*.rs" ];
      };
      keep-sorted = {
        command = "keep-sorted";
        includes = [ "*" ];
      };
    };
  };
}
