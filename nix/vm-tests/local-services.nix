{
  pkgs,
  module,
}: let
  bind_address = "127.0.0.1";
  bind_port = 1234;
  user = "user1";
  group = "group1";
  music_dir = "/music";
in
  pkgs.nixosTest {
    name = "local-services-test";
    nodes.machine = {pkgs, ...}: {
      imports = [
        module
      ];
      # user/group for the daemon
      users.groups.${group} = {};
      users.users.${user} = {
        isSystemUser = true;
        inherit group;
      };
      # folders for music directory (TEMPORARY, for this test)
      # Real users will likely create a mount to existing files
      systemd.tmpfiles.rules = [
        "d ${music_dir} 1777 ${user} ${group} 9999d"
      ];
      # initialize the soundbox-ii service
      services.soundbox-ii = {
        enable = true;
        vlc_password = "notsecure";
        inherit user group;
        inherit music_dir;
        inherit bind_address bind_port;
        beets_package = pkgs.beets;
        # empty file
        # TODO add test for querying specific songs from the synthetic "library.db"
        # (e.g. query some GET paths from a client node for available items)
        beets_readonly_src = pkgs.runCommand "beets-readonly-src" {} ''
          touch $out
        '';
      };
      # curl for executing the test
      environment.systemPackages = [
        pkgs.curl
      ];
    };

    testScript = ''
      machine.wait_for_unit("soundbox-ii.service")
      machine.wait_for_unit("soundbox-ii_vlc.service")
      machine.wait_for_open_port(${toString bind_port})
      machine.succeed("systemctl status soundbox-ii.service")
      machine.succeed("curl http://${bind_address}:${toString bind_port}/app/index.html | grep -o \"soundbox-ii\"")
    '';
  }
