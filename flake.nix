{
  description = "soundbox-ii music playback controller";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?rev=35227e5abb956ae2885306ef4769617ed28427e7"; # TODO diagnose `trunk` issues with newer revs of nixpkgs-unstable
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    import-cargo.url = "github:edolstra/import-cargo";
    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, import-cargo, crate2nix, ... }:
    let
      # name must match Cargo.toml
      name = "soundbox-ii";
      rootModuleName = "soundbox-ii";
      rustChannel = "stable";
      rustVersion = "1.60.0";
      makeForSystem = (system: let
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

        projectImportCargo = (import-cargo.builders.importCargo {
            lockFile = ./Cargo.lock;
            inherit pkgs;
          });
        trunkBuild = { buildDirRelative ? ".", pname, version }: pkgs.stdenvNoCC.mkDerivation {
          inherit pname version;

          src = ./.;

          buildInputs = [
            pkgs.rustc
            pkgs.wasm-bindgen-cli
            pkgs.nodePackages.sass
            projectImportCargo.cargoHome
          ];

          buildPhase = ''
            cd "${buildDirRelative}"
            ${pkgs.trunk}/bin/trunk build --dist dist
          '';
          # TODO: need to set mtime of the resulting files to the commit time (but can't use `current time` when unstaged, since that's not pure)
          # OR another bogus time?
          # OR write a file mtime.txt that the server reads?
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
          drv = vlc_script { inherit name visual; };
        };
        vlc_script = { name, visual ? false }: let
          launch = if visual then ''
            echo "!!!NOTE!!! Need to click menu:  View > Add Interface > Web"
            echo ""
            echo "Press enter to launch visual interface"
            read a
            ${pkgs.vlc}/bin/vlc ''${ARGS}
          '' else ''
            ${pkgs.vlc}/bin/cvlc --intf http ''${ARGS}
          '';
        in pkgs.writeShellScriptBin name ''
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
        packages."${name}_vlc" = vlc_script { name = "vlc"; visual = true; };
        packages."${name}_cvlc" = vlc_script { name = "cvlc"; visual = false; };

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
      });
      # packages = flake-utils.lib.eachDefaultSystem makeForSystem;
      packages = flake-utils.lib.eachSystem [ "x86_64-linux" ] makeForSystem;
    in packages // {
      hydraJobs = flake-utils.lib.eachSystem [ "x86_64-linux" ] (system: {
        soundbox-ii = packages.packages.${system}.soundbox-ii;
        soundbox-ii_bin = packages.packages.${system}.soundbox-ii_bin;
        soundbox-ii_frontend = packages.packages.${system}.soundbox-ii_frontend;
        soundbox-ii_vlc = packages.packages.${system}.soundbox-ii_vlc;
        soundbox-ii_cvlc = packages.packages.${system}.soundbox-ii_cvlc;
      });
      nixosModule = { config, lib, pkgs, ... }:
        let
          pkg = self.packages.${pkgs.system}.soundbox-ii;
          pkg_cvlc = self.packages.${pkgs.system}.soundbox-ii_cvlc;
          cfg = config.services.soundbox-ii;
          environment = rec {
            VLC_HOST = "127.0.0.1";
            VLC_BIND_HOST = VLC_HOST;
            VLC_PORT = toString cfg.vlc_port;
            VLC_PASSWORD = cfg.vlc_password;
          };
        in {
          options.services.soundbox-ii = {
            enable = lib.mkEnableOption "soundbox-ii service";
            vlc_port = lib.mkOption {
              type = lib.types.port;
              description = ''
                VLC control port used internally
              '';
              default = 34392;
            };
            vlc_password = lib.mkOption {
              type = lib.types.str;
              description = ''
                VLC control password used internally
              '';
            };
            user = lib.mkOption {
              type = lib.types.str;
              description = ''
                User to run the soundbox-ii and cvlc services
              '';
            };
            group = lib.mkOption {
              type = lib.types.str;
              description = ''
                Group to run the soundbox-ii and cvlc services
              '';
            };
            music_dir = lib.mkOption {
              type = lib.types.path;
              description = ''
                Path to the music folder
              '';
            };
            bind_address = lib.mkOption {
              type = lib.types.str;
              description = ''
                IP-Address to bind the web server to
              '';
            };
            bind_port = lib.mkOption {
              type = lib.types.ints.unsigned;
              default = 3030;
              description = ''
                IP-Address to bind the web server to
              '';
            };
          };
          config = lib.mkIf cfg.enable {
            systemd.services.soundbox-ii = {
              description = "soundbox-ii server";
              serviceConfig = {
                # Type = "forking";
                Type = "simple";
                ExecStart = "${pkg}/bin/soundbox-ii --serve";
                WorkingDirectory = cfg.music_dir;
                User = cfg.user;
                Group = cfg.group;
              };
              requires = [ "soundbox-ii_vlc.service" ];
              after = [ "soundbox-ii_vlc.service" ];
              wantedBy = [ "multiuser.target" ];
              environment = environment // {
                BIND_ADDRESS = "${cfg.bind_address}:${toString cfg.bind_port}";
              };
            };
            systemd.services.soundbox-ii_vlc = {
              description = "vlc instance for soundbox-ii";
              serviceConfig = {
                Type = "simple";
                ExecStart = "${pkg_cvlc}/bin/cvlc";
                WorkingDirectory = cfg.music_dir;
                User = cfg.user;
                Group = cfg.group;
              };
              inherit environment;
            };
            networking.firewall.allowedTCPPorts = [ cfg.bind_port ];
            networking.firewall.allowedUDPPorts = [ cfg.bind_port ];
          };
        };
    };
}
