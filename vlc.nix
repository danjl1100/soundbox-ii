{
  pkgs,
  flake-utils,
}: let
  # NOTE: VLC packages are currently (nixpkgs-23.05) marked as broken on macos
  # Instead of supplying nothing, generate stub scripts to launch the VLC.app installed separately
  isDarwin = pkgs.stdenv.isDarwin;

  mkVlcApp = {
    name,
    visual,
  }:
    flake-utils.lib.mkApp {
      inherit name;
      drv = vlc_script {inherit name visual;};
    };
  vlc_script = {
    name,
    visual ? false,
  }: let
    launch =
      if isDarwin
      then ''
        echo "!!!NOTE!!! Need to click menu:  VLC media player > Add Interface > Web"
        echo ""
        echo "Press enter to launch visual interface (macos app, external to nix)"
        read a
        open -na /Applications/VLC.app --args ''${ARGS}
      ''
      else
        (
          if visual
          then ''
            echo "!!!NOTE!!! Need to click menu:  View > Add Interface > Web"
            echo ""
            echo "Press enter to launch visual interface"
            read a
            ${pkgs.vlc}/bin/vlc ''${ARGS}
          ''
          else ''
            ${pkgs.vlc}/bin/cvlc --intf http ''${ARGS}
          ''
        );
  in
    pkgs.writeShellScriptBin name ''
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
  not_available = names:
    pkgs.lib.listToAttrs (builtins.map (name: {
        inherit name;
        value = builtins.throw "package \"${name}\" is usually not available on Darwin (blocked in flake.nix)";
      })
      names);

  vlc = vlc_script {
    name = "vlc";
    visual = true;
  };
  cvlc =
    if isDarwin
    then (not_available ["cvlc"]).cvlc
    else
      (vlc_script {
        name = "cvlc";
        visual = false;
      });
in rec {
  packages =
    if isDarwin
    # then (not_available ["vlc" "cvlc"]) #NOTE: errors on `nix flake show`
    then {}
    else {inherit vlc cvlc;};

  apps = {
    vlc = flake-utils.lib.mkApp {
      name = "vlc";
      drv = vlc;
    };
    cvlc = flake-utils.lib.mkApp {
      name = "cvlc";
      drv = cvlc;
    };
  };
}
