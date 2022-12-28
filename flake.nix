{
  description = "soundbox-ii music playback controller";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?rev=35227e5abb956ae2885306ef4769617ed28427e7"; # TODO diagnose `trunk` issues with newer revs of nixpkgs-unstable
    flake-utils.url = "github:numtide/flake-utils";
    nix-filter_input.url = "github:numtide/nix-filter";
    rust-overlay.url = "github:oxalica/rust-overlay";
    import-cargo.url = "github:edolstra/import-cargo";
    crate2nix = {
      url = "github:kolloch/crate2nix";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, nix-filter_input, rust-overlay, import-cargo, crate2nix, ... }:
    let
      sources = let
        nix-filter = nix-filter_input.lib;
        rust-filter = rec {
          __functor = self: filter;
          filter = { root, include-lib-crates ? [], include-bin-crates ? [], include ? [], exclude ? [] }:
            nix-filter {
              inherit root exclude;
              include = include
                ++ (lib-crates include-lib-crates)
                ++ (bin-crates include-bin-crates);
            };
          _or_root = d: if builtins.isString d then d else ".";
          rs-src-dir = d: (nix-filter.and
            (nix-filter.inDirectory "${_or_root d}/src")
            (nix-filter.or_
              (nix-filter.matchExt "rs")
              (nix-filter.isDirectory) # include directories, for sub-folder traversal (required by the outer AND)
            )
          );
          cargo-toml-dir = d: "${_or_root d}/Cargo.toml";
          cargo-lock-dir = d: "${_or_root d}/Cargo.lock";
          lib-crate = d: [ (rs-src-dir d) (cargo-toml-dir d) ];
          bin-crate = d: [ (rs-src-dir d) (cargo-toml-dir d) (cargo-lock-dir d) ];
          lib-crates = dirs: builtins.concatLists (builtins.map lib-crate dirs);
          bin-crates = dirs: builtins.concatLists (builtins.map bin-crate dirs);
        };
        src-license = [
          ./shared/src/license/COPYING.REDISTRIBUTION
          ./shared/src/license/COPYING.WARRANTY
          ./shared/src/license/COPYING.WELCOME
        ];
      in {
        bin = rust-filter {
          root = ./.;
          include-bin-crates = [ false ];
          include-lib-crates = ["q-filter-tree" "vlc-http" "frontend" "sequencer" "shared" "arg_split"];
          include = src-license;
        };
        frontend = rust-filter {
          root = ./.;
          include-lib-crates = [ "frontend" "shared" ];
          include = [
            ./frontend/index.html
            ./frontend/index.scss
            ./frontend/Trunk.toml
          ] ++ src-license;
        };
        bin-lock-file = ./Cargo.lock;
      };

      # name must match Cargo.toml
      name = "soundbox-ii";
      rootModuleName = "soundbox-ii";
      rustChannel = "stable";
      rustVersion = "1.62.0";
      makeForSystem = (system: let
        # Imports
        pkgs = import nixpkgs {
          inherit system;
          overlays = [
            rust-overlay.overlays.default (self: super: let
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
            src = sources.bin;
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
        ] ++ (if pkgs.stdenv.hostPlatform.isDarwin then
        let
          frameworks = pkgs.darwin.apple_sdk.frameworks;
        in [
          frameworks.CoreServices
        ] else []);

        projectImportCargo = (import-cargo.builders.importCargo {
            lockFile = sources.bin-lock-file;
            inherit pkgs;
          });
        trunkBuild = { pname, version }: pkgs.stdenvNoCC.mkDerivation {
          inherit pname version;

          src = sources.frontend;

          buildInputs = [
            pkgs.rustc
            pkgs.wasm-bindgen-cli
            pkgs.trunk
            pkgs.nodePackages.sass
            projectImportCargo.cargoHome
          ];

          buildPhase = let
            buildDirRelative = "frontend";
          in ''
            cd "${buildDirRelative}"
            trunk build --dist dist
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
        bin = pkgs.symlinkJoin { # NOTE: artificial redirection, seemed to help with `nix flake show` speediness
          name = rootModuleName;
          paths = [
            project.workspaceMembers.${rootModuleName}.build
          ];
        };
        frontend = trunkBuild {
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
          # NOTE: do not include dependencies for `vlc` (broken on darwin systems)
          # inputsFrom = builtins.attrValues self.packages.${system};
          inputsFrom = [ self.defaultPackage.${system} frontend ];
          buildInputs = nativeBuildInputs ++ buildInputs ++ ([
            # development-only tools go here
            pkgs.nixpkgs-fmt
            pkgs.cargo-deny
            pkgs.cargo-edit
            # pkgs.cargo-watch
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
      test_nixosConfiguration = system: let
        service_activation = { ... }: {
          # initialize the soundbox-ii service
          services.soundbox-ii = {
            enable = true;
            vlc_password = "notsecure";
            user = "soundbox-ii";
            group = "soundbox-ii";
            music_dir = "/nonexistent";
            bind_address = "127.0.0.1";
            bind_port = 1234;
          };
        };
        required_nixos = { ... }: {
          fileSystems."/" =
            { device = "none";
              fsType = "tmpfs";
              options = [ "size=2G" "mode=755" ];
            };
          boot.loader.systemd-boot.enable = true;
        };
      in nixpkgs.lib.nixosSystem {
          inherit system;
          modules = [
            self.nixosModules.default
            service_activation
            required_nixos
          ];
        };
      # packages = flake-utils.lib.eachDefaultSystem makeForSystem;
      packages = flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-darwin" ] makeForSystem;
    in packages // {
      hydraJobs = flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-darwin" ] (system: {
        soundbox-ii = self.packages.${system}.soundbox-ii;
        soundbox-ii_bin = self.packages.${system}.soundbox-ii_bin;
        soundbox-ii_frontend = self.packages.${system}.soundbox-ii_frontend;
        # soundbox-ii_vlc = self.packages.${system}.soundbox-ii_vlc;
        # soundbox-ii_cvlc = self.packages.${system}.soundbox-ii_cvlc;
        nixosModule = let
          pkgs = import nixpkgs {
            inherit system;
          };
          nixos_toplevel = (test_nixosConfiguration system).config.system.build.toplevel;
        in pkgs.symlinkJoin { # NOTE: artificial redirection, seemed to help with `nix flake show` speediness
          name = "nixosConfiguration test-${system}";
          paths = [ nixos_toplevel ];
        };
      });
      # nixosConfigurations.test-x86_64-linux = test_nixosConfiguration "x86_64-linux";
      # nixosConfigurations.test-aarch-darwin = test_nixosConfiguration "aarch-darwin";
      # nixosConfigurations.default = self.nixosConfigurations.test-x86_64-linux;
      nixosModules.default = { config, lib, pkgs, ... }:
        let
          pkg = self.packages.${pkgs.system}.soundbox-ii;
          pkg_cvlc = self.apps.${pkgs.system}.cvlc.program;
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
          config = lib.mkIf cfg.enable (let
            vlc_service = "soundbox-ii_vlc";
          in {
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
              requires = [ "${vlc_service}.service" ];
              after = [ "${vlc_service}.service" ];
              wantedBy = [ "multiuser.target" ];
              environment = environment // {
                BIND_ADDRESS = "${cfg.bind_address}:${toString cfg.bind_port}";
              };
            };
            systemd.services.${vlc_service} = {
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
          });
        };
    };
}
