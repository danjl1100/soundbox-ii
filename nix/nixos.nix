{packages}: rec {
  nixosModules.default = {
    config,
    lib,
    pkgs,
    ...
  }: let
    pkg = packages.${pkgs.system}.soundbox-ii;
    pkg_cvlc = packages.${pkgs.system}.cvlc;
    cfg = config.services.soundbox-ii;
    environment =
      rec {
        VLC_HOST = "127.0.0.1";
        VLC_BIND_HOST = VLC_HOST;
        VLC_PORT = toString cfg.vlc_port;
        VLC_PASSWORD = cfg.vlc_password;
      }
      // (
        if (isNull cfg.beets_package)
        then {}
        else {
          BEET_CMD = "${cfg.beets_package}/bin/beet";
        }
      );
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

          NOTE: The user must have a HOME folder to cache albumartwork. Otherwise, VLC will refuse to serve artwork.
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
      beets_package = lib.mkOption {
        type = lib.types.nullOr lib.types.package;
        default = null;
        example = "pkgs.beets";
        description = ''
          Package for beets, containing the `/bin/beet` executable
        '';
      };
      beets_readonly_src = lib.mkOption {
        type = lib.types.nullOr lib.types.path;
        default = null;
        example = "/path/to/beets/library.db";
        description = ''
          If set, copy the specified file to a temporary folder (writable by the user)
        '';
      };
    };
    config = lib.mkIf cfg.enable (let
      vlc_service = "soundbox-ii_vlc";
      exec_crate = "${pkg}/bin/soundbox-ii --serve";
      ExecStart =
        if (isNull cfg.beets_readonly_src)
        then exec_crate
        else
          (pkgs.writeShellApplication {
            name = "soundbox-ii-launch";
            runtimeInputs = [
              pkgs.coreutils # mktemp, mkdir, cp, cat
            ];
            text = ''
              TMP_DIR="$(mktemp -d --suffix=soundbox-ii)"
              export HOME="$TMP_DIR"
              BEETS_CONFIG="$HOME/.config/beets"
              CONFIG_YAML="''${BEETS_CONFIG}/config.yaml"

              echo "Creating BEETS_CONFIG folder... $BEETS_CONFIG"
              mkdir -p "$BEETS_CONFIG"

              echo "Creating $CONFIG_YAML"
              echo "directory: ${cfg.music_dir}" > "''${CONFIG_YAML}"
              echo "library: ''${BEETS_CONFIG}/library.db" >> "''${CONFIG_YAML}"
              chmod a-w "''${CONFIG_YAML}"

              echo "Copying beets library from ${cfg.beets_readonly_src}"
              echo " to ''${BEETS_CONFIG}/library.db"
              cp "${cfg.beets_readonly_src}" "''${BEETS_CONFIG}/library.db"
              chmod u+w "''${BEETS_CONFIG}/library.db"

              echo "Launching..."
              ${exec_crate}
            '';
          })
          + "/bin/soundbox-ii-launch";
    in {
      systemd.services.soundbox-ii = {
        description = "soundbox-ii server";
        serviceConfig = {
          Type = "simple";
          inherit ExecStart;
          WorkingDirectory = cfg.music_dir;
          User = cfg.user;
          Group = cfg.group;
        };
        requires = ["${vlc_service}.service"];
        after = ["${vlc_service}.service"];
        wantedBy = ["default.target"];
        environment =
          environment
          // {
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
      networking.firewall.allowedTCPPorts = [cfg.bind_port];
      networking.firewall.allowedUDPPorts = [cfg.bind_port];
    });
  };

  nixosTestSystems = let
    required_nixos = {
      fileSystems."/" = {
        device = "none";
        fsType = "tmpfs";
        options = ["size=2G" "mode=755"];
      };
      boot.loader.systemd-boot.enable = true;
      system.stateVersion = "22.11";
    };
  in
    {
      lib,
      system,
    }: {
      simple-configuration = let
        service_activation = {
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
      in
        lib.nixosSystem {
          inherit system;
          modules = [
            nixosModules.default
            service_activation
            required_nixos
          ];
        };
    };
}
