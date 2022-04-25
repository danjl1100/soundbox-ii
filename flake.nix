{
  description = "soundbox-ii music playback controller";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    import-cargo.url = "github:edolstra/import-cargo";
    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };
    trunk-latest-src = {
      url = "github:thedodd/trunk";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, import-cargo, crate2nix, trunk-latest-src, ... }:
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
            rust-overlay.overlay (self: super: let
              rust-bundle = self.rust-bin.${rustChannel}.${rustVersion}.default.override {
                # include wasm32 for frontend compilation via trunk
                targets = [ "wasm32-unknown-unknown" ];
              };
            in {
              # unpack rust-overlay's bundles to inform crate2nix
              rustc = rust-bundle;
              cargo = rust-bundle;
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

        # Trunk builder for frontend
        # TODO remove this, replace with stable `pkgs` version,
        #    once nixpkgs-unstable incorporates: https://github.com/thedodd/trunk/pull/358
        trunk-latest = pkgs.rustPlatform.buildRustPackage rec {
          pname = "trunk";
          version = "0.14.0-pre-git";

          src = trunk-latest-src;

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = if pkgs.stdenvNoCC.isDarwin
            then [ pkgs.libiconv pkgs.CoreServices pkgs.Security ]
            else [ pkgs.openssl ];

          # requires network
          checkFlags = [ "--skip=tools::tests::download_and_install_binaries" ];

          cargoSha256 = "sha256-DBL8fvC1pVsGsIEjNouFbeAsbH/TNfcrIrds2EWD53Q=";
        };
        trunkBuild = { src, buildDirRelative ? ".", pname, version }: pkgs.stdenvNoCC.mkDerivation {
          inherit src pname version;

          buildInputs = [
            pkgs.rustc
            # TODO change trunk-latest to stable `pkgs` version,
            #   once nixpkgs-unstable contains pr: https://github.com/thedodd/trunk/pull/358
            # pkgs.trunk
            trunk-latest
            pkgs.wasm-bindgen-cli
            pkgs.nodePackages.sass
            (import-cargo.builders.importCargo {
              lockFile = "${src}/Cargo.lock";
              inherit pkgs;
            }).cargoHome
          ];

          buildPhase = ''
            cd "${buildDirRelative}"
            trunk build --dist dist
          '';
          installPhase = ''
            mkdir -p "$out/share/frontend"
            cp -r dist/* "$out/share/frontend"
          '';
        };
      in rec {
        packages.${name} = pkgs.symlinkJoin {
          inherit name;
          paths = [
            packages."${name}_bin"
            packages."${name}_frontend"
          ];
        };
        packages."${name}_bin" = project.workspaceMembers.${rootModuleName}.build;
        packages."${name}_frontend" = trunkBuild {
          src = ./.;
          buildDirRelative = "frontend";
          pname = "${name}_frontend";
          version = "0.1.0";
        };

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
