{
  pkgs,
  system,
  crane,
  advisory-db,
  flake-utils,
}: let
  name = "soundbox-ii";

  rustChannel = "beta";
  rustVersion = "latest";
  # TODO simplify to just one toolchain which includes wasm
  rustToolchain = pkgs.rust-bin.${rustChannel}.${rustVersion}.default;
  rustToolchainForDevshell = rustToolchainWasm.override {
    extensions = ["rust-analyzer" "rust-src"];
  };
  wasmTarget = "wasm32-unknown-unknown";
  rustToolchainWasm = rustToolchain.override {
    targets = [wasmTarget];
  };
  craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
  craneLibWasm = (crane.mkLib pkgs).overrideToolchain rustToolchainWasm;
  craneLibForDevShell = (crane.mkLib pkgs).overrideToolchain rustToolchainForDevshell;

  crates = let
    licenseFilter = path: _type: builtins.match ".*shared/src/license/COPYING.*" path != null;
    webFilter = path: _type: builtins.any (ext: builtins.match ".*${ext}" path != null) ["scss" "html"];
    licenseOrCargo = path: type: (licenseFilter path type) || (craneLib.filterCargoSources path type);
    licenseOrCargoOrWeb = path: type: (licenseOrCargo path type) || (webFilter path type);
  in {
    server = pkgs.callPackage ./crate.nix {
      inherit system advisory-db craneLib;
      src =
        pkgs.lib.cleanSourceWith
        {
          src = craneLib.path ./..;
          filter = licenseOrCargo;
        };
      commonArgOverrides = {
        cargoTestExtraArgs = "--workspace";
      };
    };
    client = pkgs.callPackage ./crate.nix {
      inherit system advisory-db;
      craneLib = craneLibWasm;
      src =
        pkgs.lib.cleanSourceWith
        {
          src = craneLib.path ./..;
          filter = licenseOrCargoOrWeb;
        };
      commonArgOverrides = {
        pname = "${name}-frontend";
        cargoExtraArgs = "--package=frontend";
        # TODO delete this target arg, messes with "doc" and no issues removing it
        # CARGO_BUILD_TARGET = wasmTarget;
        doCheck = false;
      };
      isWasm = true;
    };
    fake-beet = pkgs.callPackage ./crate.nix {
      inherit system advisory-db craneLib;
      src = craneLib.path ./../fake-beet;
    };
  };

  bin = crates.server.package;
  frontend = crates.client.buildTrunkPackage {
    pname = "${name}-frontend";
    trunkIndexPath = "frontend/index.html";
    trunkExtraArgs = "--config frontend/Trunk.toml";
    trunkExtraBuildArgs = "--dist frontend/dist"; # trunk is run from root, expects outputs next to "frontend/index.html"
  };
  fake-beet = crates.fake-beet.package;

  wrap_static_assets = {
    bin,
    frontend,
    name,
  }:
    pkgs.writeShellApplication {
      inherit name;
      text = ''
        export STATIC_ASSETS="${frontend}"
        ${bin}/bin/${name} "$@"
      '';
    };

  trunkOffline = pkgs.writeShellApplication {
    name = "trunk";
    runtimeInputs = frontend.nativeBuildInputs;
    text = ''
      ${frontend.preConfigure}
      ${pkgs.trunk}/bin/trunk "$@"
    '';
  };
in rec {
  packages.${name} = wrap_static_assets {
    inherit bin frontend name;
  };
  packages.${"${name}_bin"} = bin;
  packages.${"${name}_frontend"} = frontend;
  packages.fake-beet = fake-beet;

  apps.${name} = flake-utils.lib.mkApp {
    inherit name;
    drv = packages.${name};
  };
  apps.rust-doc = flake-utils.lib.mkApp {
    drv = crates.server.drv-open-doc.for-crate name;
  };
  apps.rust-doc-std = flake-utils.lib.mkApp {
    drv = crates.server.drv-open-doc.for-std rustToolchainForDevshell;
  };

  devShellFn = {packages ? []} @ inputs:
    crates.client.devShellFn (inputs
      // {
        craneLib = craneLibForDevShell;
        packages =
          packages
          ++ [
            trunkOffline
          ];
      });

  checks = let
    inner_checks = {
      crate_name,
      check_attrs,
    }:
      builtins.map (check_name: {
        name = "${crate_name}--${check_name}";
        value = check_attrs.${check_name};
      }) (builtins.attrNames check_attrs);
    outer_crates = {crates}:
      builtins.listToAttrs (pkgs.lib.flatten (builtins.map (crate_name:
        inner_checks {
          inherit crate_name;
          check_attrs = crates.${crate_name}.checks;
        }) (builtins.attrNames crates)));
  in
    outer_crates {inherit crates;};
}
