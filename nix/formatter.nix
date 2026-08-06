{
  treefmt,
  nixfmt,
  rustfmt,
}:
treefmt.withConfig {
  runtimeInputs = [
    nixfmt
    rustfmt
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
        includes = [ "*.rs" ];
      };
    };
  };
}
