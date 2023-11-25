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
  }: let
    target_systems = ["x86_64-linux" "aarch64-darwin"];
    target_systems_nixos = ["x86_64-linux"];
    target_systems_hydra = target_systems_nixos;
    nixos = import ./nix/nixos.nix {
      inherit (self) packages;
    };
    nixosTestSystems = {system}:
      nixos.nixosTestSystems {
        inherit (nixpkgs) lib;
        inherit system;
      };
    nixosTestToplevels = system:
      builtins.mapAttrs (_name: value:
        value.config.system.build.toplevel)
      (nixosTestSystems {inherit system;});
    nixosTests = system:
      (import ./nix/vm-tests) {
        pkgs = import nixpkgs {inherit system;};
        module = self.nixosModules.default;
      };
  in
    {
      inherit (nixos) nixosModules;
      hydraJobs =
        (flake-utils.lib.eachSystem target_systems_hydra (system: {
          soundbox-ii = self.packages.${system}.soundbox-ii;
          soundbox-ii_bin = self.packages.${system}.soundbox-ii_bin;
          soundbox-ii_frontend = self.packages.${system}.soundbox-ii_frontend;
        }))
        // (flake-utils.lib.eachSystem target_systems_nixos nixosTestToplevels)
        // (flake-utils.lib.eachSystem target_systems_nixos nixosTests);
    }
    // flake-utils.lib.eachSystem target_systems (
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

        core = import ./nix/core.nix {
          inherit pkgs system crane advisory-db flake-utils;
        };

        vlc = import ./nix/vlc.nix {
          inherit pkgs flake-utils;
        };

        testSystemsChecks = let
          testSystems = nixosTestSystems {inherit system;};
        in
          if pkgs.stdenv.isDarwin
          then {}
          else
            builtins.listToAttrs (
              pkgs.lib.flatten
              (builtins.map (
                  name:
                    if (testSystems.${name}.config.system ? checks)
                    then
                      (builtins.map (check: {
                          name = "${name}-${check.name}";
                          value = check;
                        })
                        testSystems.${name}.config.system.checks)
                    else []
                )
                (builtins.attrNames testSystems))
            );
      in rec {
        # Combine the outputs from each subsystem,
        #  and pick reasonable defaults.

        checks =
          core.checks
          // vlc.checks
          // testSystemsChecks
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

        packages =
          core.packages
          // vlc.packages
          // {
            default = core.packages.soundbox-ii;
          };

        apps =
          core.apps
          // vlc.apps
          // {
            default = core.apps.soundbox-ii;
          };

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
