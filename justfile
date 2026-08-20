alias r := run
alias b := build
alias t := test

default: run

run subcommand="run":
    cargo run -- {{ subcommand }}

build:
    cargo build

test:
    cargo test

alias nb := nix-build
alias nc := nix-check
alias f := fmt

[group('nix')]
nix-build:
    nix build

[group('nix')]
nix-check:
    nix flake check

[group('nix')]
fmt:
    nix fmt
