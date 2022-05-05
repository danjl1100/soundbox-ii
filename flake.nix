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
    trunk-0-15-0-src = {
      url = "github:thedodd/trunk/v0.15.0";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, import-cargo, crate2nix, trunk-0-15-0-src, ... }:
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
          version = "0.15.0";

          src = trunk-0-15-0-src;

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = if pkgs.stdenvNoCC.isDarwin
            then [ pkgs.libiconv pkgs.CoreServices pkgs.Security ]
            else [ pkgs.openssl ];

          # requires network
          checkFlags = [ "--skip=tools::tests::download_and_install_binaries" ];

          cargoSha256 = "sha256-czXe9W+oR1UV7zGZiiHcbydzH6sowa/8upm+5lkPG1U=";
        };
        projectImportCargo = (import-cargo.builders.importCargo {
            lockFile = ./Cargo.lock;
            inherit pkgs;
          });
        trunkBuild = { buildDirRelative ? ".", pname, version }: pkgs.stdenvNoCC.mkDerivation {
          inherit pname version;

          src = ./.;

          buildInputs = [
            pkgs.rustc
            # TODO change trunk-latest to stable `pkgs` version,
            #   once nixpkgs-unstable contains pr: https://github.com/thedodd/trunk/pull/358
            # pkgs.trunk
            trunk-latest
            pkgs.wasm-bindgen-cli
            pkgs.nodePackages.sass
            projectImportCargo.cargoHome
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

        # define packages (for re-use in combined target)
        bin = project.workspaceMembers.${rootModuleName}.build;
        frontend = trunkBuild {
          buildDirRelative = "frontend";
          pname = "${name}_frontend";
          version = "0.1.0";
        };
        bin_wrapped = pkgs.symlinkJoin {
          inherit name;
          paths = [ bin frontend ];
          buildInputs = [ pkgs.makeWrapper ];
          postBuild = ''
            wrapProgram $out/bin/soundbox-ii --add-flags "--static-assets \"${frontend}/share/frontend\""
          '';
        };

        mkVlcApp = { name, visual }: flake-utils.lib.mkApp {
          inherit name;
          drv = pkgs.writeShellScriptBin name (vlc_script { inherit visual; });
        };
        vlc_script = { visual ? false }: let
          launch = if visual then ''
            echo "!!!NOTE!!! Need to click menu:  View > Add Interface > Web"
            echo ""
            echo "Press enter to launch visual interface"
            read a
            ${pkgs.vlc}/bin/vlc ''${ARGS}
          '' else ''
            ${pkgs.vlc}/bin/cvlc --intf http ''${ARGS}
          '';
        in ''
          if [ "''${VLC_BIND_HOST}" = "" ]; then
            echo "VLC_BIND_HOST is not set";
            exit 1;
          fi
          if [ "''${VLC_PORT}" = "" ]; then
            echo "VLC_PORT is not set";
            exit 1;
          fi
          if [ "''${VLC_PASSWORD}" = "" ]; then
            echo "VLC_PASSWORD is not set";
            exit 1;
          fi

          ARGS="--audio-replay-gain-mode track --http-host ''${VLC_BIND_HOST} --http-port ''${VLC_PORT} --http-password ''${VLC_PASSWORD}"

          ${launch}
        '';
      in rec {
        packages.${name} = bin_wrapped;
        packages."${name}_bin" = bin;
        packages."${name}_frontend" = frontend;

        # `nix build`
        defaultPackage = bin_wrapped;

        # `nix run`
        defaultApp = apps.${name};
        apps.${name} = flake-utils.lib.mkApp {
          inherit name;
          drv = packages.${name};
        };
        apps.vlc = mkVlcApp { name = "vlc"; visual = true; };
        apps.cvlc = mkVlcApp { name = "cvlc"; visual = false; };

        # `nix develop`
        devShell = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.packages.${system};
          buildInputs = buildInputs ++ ([
            # development-only tools go here
            pkgs.nixpkgs-fmt
            pkgs.cargo-deny
            pkgs.cargo-edit
            pkgs.cargo-watch
            pkgs.bacon
            pkgs.rust-bin.${rustChannel}.${rustVersion}.rust-analysis
            pkgs.rust-bin.${rustChannel}.${rustVersion}.rls
          ]);
          shellHook = ''
            #!/usr/bin/env bash
            FAKE_CARGO_HOME=$(pwd)/target/.fake-cargo-home
            rm -rf "$FAKE_CARGO_HOME"
            mkdir -p "$FAKE_CARGO_HOME"
            cp -prd ${projectImportCargo.vendorDir}/vendor "$FAKE_CARGO_HOME"
            chmod -R u+w $FAKE_CARGO_HOME
            export CARGO_HOME=''${FAKE_CARGO_HOME}/vendor
          '';
          RUST_SRC_PATH = "${pkgs.rust-bin.${rustChannel}.${rustVersion}.rust-src}/lib/rustlib/src/rust/library";
        };
      }
    );

}
