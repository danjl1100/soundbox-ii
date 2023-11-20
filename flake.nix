{
  description = "soundbox-ii";

  inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
    # decrease total count of flake dependencies by following versions from "rust-overlay" input
    flake-utils.follows = "rust-overlay/flake-utils";
    nixpkgs.follows = "rust-overlay/nixpkgs";
    nixpkgs-unstable-for-vlc.url = "github:nixos/nixpkgs?branch=nixpkgs-unstable";
    crane.inputs.rust-overlay.follows = "rust-overlay";
    crane.inputs.nixpkgs.follows = "nixpkgs";
    crane.inputs.flake-utils.follows = "flake-utils";
  };

  outputs = {
    # common
    self,
    flake-utils,
    nixpkgs,
    nixpkgs-unstable-for-vlc,
    # rust
    rust-overlay,
    crane,
    advisory-db,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        nixpkgs-unstable = import nixpkgs-unstable-for-vlc {
          inherit system;
        };
        overlays = [
          rust-overlay.overlays.default
          (next: prev: {
            vlc = nixpkgs-unstable.vlc;
          })
        ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        core = import ./core.nix {
          inherit pkgs system crane advisory-db flake-utils;
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

        packages = let
          # not_available = names: pkgs.lib.listToAttrs (builtins.map (name: {
          #   inherit name;
          #   value = builtins.throw "package \"${name}\" is not available on Darwin";
          # }) names);
          vlc_packages =
            if pkgs.stdenv.isDarwin
            then {}
            else {
              # TODO wrap the vlc commands as before
              vlc = pkgs.vlc;
              # cvlc = #TODO;
            };
        in
          core.packages // vlc_packages;

        apps = core.apps;

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
