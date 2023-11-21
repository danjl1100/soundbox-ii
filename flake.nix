{
  description = "soundbox-ii";

  inputs = {
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
    # decrease total count of flake dependencies by following versions from "rust-overlay" input
    flake-utils.follows = "rust-overlay/flake-utils";
    # nixpkgs.follows = "rust-overlay/nixpkgs";
    nixpkgs.url = "github:nixos/nixpkgs?branch=nixos-23.05";
    nixpkgs-for-vlc.url = "github:nixos/nixpkgs?branch=nixpkgs-unstable";
    nixpkgs-for-wasm-bindgen.url = "github:nixos/nixpkgs/34bfa9403e42eece93d1a3740e9d8a02fceafbca";
    crane.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    # common
    self,
    flake-utils,
    nixpkgs,
    nixpkgs-for-vlc,
    nixpkgs-for-wasm-bindgen,
    # rust
    rust-overlay,
    crane,
    advisory-db,
  }:
    flake-utils.lib.eachSystem ["x86_64-linux" "aarch64-darwin"] (
      system: let
        pkgs-for-vlc = import nixpkgs-for-vlc {
          inherit system;
        };
        pkgs-for-wasm-bindgen = import nixpkgs-for-wasm-bindgen {
          inherit system;
        };
        overlays = [
          rust-overlay.overlays.default
          (next: prev: {
            inherit (pkgs-for-vlc) vlc;
            inherit (pkgs-for-wasm-bindgen) wasm-bindgen-cli;
          })
        ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        core = import ./core.nix {
          inherit pkgs system crane advisory-db flake-utils;
        };

        vlc = import ./vlc.nix {
          inherit pkgs flake-utils;
        };
      in rec {
        # Combine the outputs from each subsystem,
        #  and pick reasonable defaults.

        checks =
          (
            core.checks
          )
          // {
            nix-alejandra = pkgs.stdenvNoCC.mkDerivation {
              name = "nix-alejandra";
              src = pkgs.lib.cleanSourceWith {
                src = ./.;
                filter = path: type: ((type == "directory") || (pkgs.lib.hasSuffix ".nix" path));
              };
              phases = ["buildPhase"];
              nativeBuildInputs = [pkgs.alejandra];
              buildPhase = "(alejandra -qc $src || alejandra -c $src) > $out";
            };
          };

        packages = core.packages;

        apps = core.apps // vlc.apps;

        devShells = {
          default = core.devShellFn {
            packages = [
              pkgs.alejandra
              pkgs.bacon
            ];
          };
        };
      }
    );
}
