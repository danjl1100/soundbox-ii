{
  description = "soundbox-ii music playback controller";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crate2nix, ... }:
    let
      # name must match Cargo.toml
      name = "soundbox-ii";
      rootModuleName = "soundbox-ii";
      rustChannel = "stable";
      rustVersion = "1.60.0";
    in flake-utils.lib.eachDefaultSystem (system:
      let
        # Imports
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            rust-overlay.overlay (self: super: {
              # unpack rust-overlay's bundles to inform crate2nix
              rustc = self.rust-bin.${rustChannel}.${rustVersion}.default;
              cargo = self.rust-bin.${rustChannel}.${rustVersion}.default;
            })
          ];
        };
        inherit (import "${crate2nix}/tools.nix" { inherit pkgs; }) generatedCargoNix;

        # Create the cargo2nix project
        project = import
          (generatedCargoNix {
            inherit name;
            src = ./.;
          })
          {
            inherit pkgs;
            # Individual crate overrides
            # reference https://github.com/balsoft/simple-osd-daemons/blob/6f85144934c0c1382c7a4d3a2bbb80106776e270/flake.nix#L28-L50
            defaultCrateOverrides = pkgs.defaultCrateOverrides // {
              ${name} = oldAttrs: {
                inherit buildInputs nativeBuildInputs;
              };
            };
          };

        # Configuration for non-Rust dependencies
        buildInputs = [
          pkgs.openssl.dev
        ];
        nativeBuildInputs = [
          pkgs.rustc
          pkgs.cargo
          pkgs.pkgconfig
        ];
      in rec {
        packages.${name} = project.workspaceMembers.${rootModuleName}.build;

        # `nix build`
        defaultPackage = packages.${name};

        # `nix run`
        apps.${name} = flake-utils.lib.mkApp {
          inherit name;
          drv = packages.${name};
        };
        defaultApp = apps.${name};

        # `nix develop`
        devShell = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.packages.${system};
          buildInputs = buildInputs ++ ([
            # development-only tools go here
            pkgs.nixpkgs-fmt
            pkgs.cargo-watch
            pkgs.bacon
            pkgs.rust-bin.${rustChannel}.${rustVersion}.rust-analysis
            pkgs.rust-bin.${rustChannel}.${rustVersion}.rls
          ]);
          RUST_SRC_PATH = "${pkgs.rust-bin.${rustChannel}.${rustVersion}.rust-src}/lib/rustlib/src/rust/library";
        };
      }
    );

}
