alias r := run
alias b := build
alias f := fmt

default: run

run subcommand="run":
    cargo run -- {{ subcommand }}

build:
    cargo build

alias nb := nix-build
alias nc := nix-check

[group('nix')]
nix-build:
    nix build

[group('nix')]
nix-check:
    nix flake check

[group('nix')]
fmt:
    nix fmt
